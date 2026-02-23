#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_colors(path: &str, limit: Option<usize>) -> PyResult<Vec<LayerColorRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x33, "LAYER", &dynamic_types) {
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let (handle, color_index, true_color) =
            match decode_layer_color_record(&mut reader, decoder.version(), obj.handle.0) {
                Ok(decoded) => decoded,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((handle, color_index, true_color));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn collect_known_layer_handles_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
) -> PyResult<Vec<u64>> {
    let mut layer_handles = Vec::new();
    for obj in index.objects.iter() {
        let Some((_record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x33, "LAYER", dynamic_types) {
            layer_handles.push(obj.handle.0);
        }
    }
    Ok(layer_handles)
}

fn recover_entity_layer_handle_r2010_plus(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
    parsed_layer_handle: u64,
    known_layer_handles: &HashSet<u64>,
) -> u64 {
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return parsed_layer_handle;
    }
    if known_layer_handles.is_empty() {
        return parsed_layer_handle;
    }

    let expected_layer_index =
        parse_expected_entity_layer_ref_index(record, version, api_header, object_handle);
    let common_parsed_layer =
        parse_common_entity_layer_handle_from_common_header(record, version, api_header);
    let mut parsed_score = layer_handle_score(parsed_layer_handle, known_layer_handles);
    if known_layer_handles.contains(&parsed_layer_handle) {
        // Allow handle-stream candidates to override parsed value.
        parsed_score = parsed_score.saturating_add(1);
    }
    let mut best = (parsed_score, parsed_layer_handle);
    let default_layer = known_layer_handles.iter().copied().min();
    let debug_entity_handle = std::env::var("EZDWG_DEBUG_ENTITY_LAYER")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let debug_this = debug_entity_handle == Some(object_handle);
    if debug_this {
        eprintln!(
            "[entity-layer] handle={} parsed_layer={} parsed_score={}",
            object_handle, parsed_layer_handle, parsed_score
        );
        if let Some(layer) = common_parsed_layer {
            eprintln!(
                "[entity-layer] handle={} common_header_layer={}",
                object_handle, layer
            );
        }
    }
    if let Some(layer) = common_parsed_layer {
        let score = layer_handle_score(layer, known_layer_handles);
        if score < best.0 {
            best = (score, layer);
        }
    }
    let mut base_handles = vec![object_handle];
    if object_handle > 1 {
        base_handles.push(object_handle - 1);
    }
    base_handles.push(object_handle.saturating_add(1));
    let mut base_reader = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader, version).is_ok() {
        if let Ok(record_handle) = base_reader.read_h() {
            let record_base = record_handle.value;
            if record_base != 0 && record_base != object_handle {
                base_handles.push(record_base);
                if record_base > 1 {
                    base_handles.push(record_base - 1);
                }
                base_handles.push(record_base.saturating_add(1));
            }
        }
    }
    let mut base_reader_with_size = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader_with_size, version).is_ok()
        && base_reader_with_size.read_rl(Endian::Little).is_ok()
    {
        if let Ok(record_handle) = base_reader_with_size.read_h() {
            let record_base = record_handle.value;
            if record_base != 0 && !base_handles.contains(&record_base) {
                base_handles.push(record_base);
                if record_base > 1 {
                    base_handles.push(record_base - 1);
                }
                base_handles.push(record_base.saturating_add(1));
            }
        }
    }
    let mut ordered_base_handles = Vec::with_capacity(base_handles.len());
    let mut seen_base_handles = HashSet::with_capacity(base_handles.len());
    for handle in base_handles {
        if seen_base_handles.insert(handle) {
            ordered_base_handles.push(handle);
        }
    }

    let mut expanded_end_bits = Vec::new();
    for base in resolve_r2010_object_data_end_bit_candidates(api_header) {
        for delta in (-256i32..=256).step_by(8) {
            let candidate_i64 = i64::from(base) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            expanded_end_bits.push(candidate);
        }
    }
    let mut stream_size_reader = record.bit_reader();
    if skip_object_type_prefix(&mut stream_size_reader, version).is_ok() {
        if let Ok(obj_size_bits) = stream_size_reader.read_rl(Endian::Little) {
            for delta in (-128i32..=128).step_by(8) {
                let candidate_i64 = i64::from(obj_size_bits) + i64::from(delta);
                if candidate_i64 < 0 {
                    continue;
                }
                if let Ok(candidate) = u32::try_from(candidate_i64) {
                    expanded_end_bits.push(candidate);
                }
            }
        }
    }
    expanded_end_bits.sort_unstable();
    expanded_end_bits.dedup();

    for object_data_end_bit in expanded_end_bits {
        for base_handle in ordered_base_handles.iter().copied() {
            for chained_base in [false, true] {
                let mut reader = record.bit_reader();
                if skip_object_type_prefix(&mut reader, version).is_err() {
                    continue;
                }
                reader.set_bit_pos(object_data_end_bit);
                let mut prev_handle = base_handle;
                let mut handle_index = 0u64;
                while handle_index < 64 {
                    let layer_handle = if chained_base {
                        match read_handle_reference_chained(&mut reader, &mut prev_handle) {
                            Ok(handle) => handle,
                            Err(_) => break,
                        }
                    } else {
                        match entities::common::read_handle_reference(&mut reader, base_handle) {
                            Ok(handle) => handle,
                            Err(_) => break,
                        }
                    };
                    let mut score = layer_handle_score(layer_handle, known_layer_handles)
                        .saturating_add(handle_index);
                    if let Some(expected) = expected_layer_index {
                        let distance = handle_index.abs_diff(expected as u64);
                        score = score.saturating_add(distance.saturating_mul(16));
                        if handle_index == expected as u64 {
                            score = score.saturating_sub(120);
                        }
                    }
                    if handle_index == 0 {
                        // First handle is often owner-related; avoid overfitting to it.
                        score = score.saturating_add(200);
                    }
                    if chained_base {
                        // Relative-to-previous mode is speculative; keep fixed-base preference.
                        score = score.saturating_add(20);
                    }
                    if layer_handle == parsed_layer_handle
                        && known_layer_handles.contains(&layer_handle)
                    {
                        score = score.saturating_sub(80);
                    }
                    if Some(layer_handle) == default_layer {
                        score = score.saturating_add(150);
                    }
                    if debug_this && known_layer_handles.contains(&layer_handle) {
                        eprintln!(
                            "[entity-layer] handle={} end_bit={} base={} chained={} idx={} layer={} score={}",
                            object_handle,
                            object_data_end_bit,
                            base_handle,
                            chained_base,
                            handle_index,
                            layer_handle,
                            score
                        );
                    } else if debug_this && handle_index < 16 {
                        eprintln!(
                            "[entity-layer] handle={} end_bit={} base={} chained={} idx={} raw_layer={} score={}",
                            object_handle,
                            object_data_end_bit,
                            base_handle,
                            chained_base,
                            handle_index,
                            layer_handle,
                            score
                        );
                    }
                    if score < best.0 {
                        best = (score, layer_handle);
                        if score == 0 {
                            break;
                        }
                    }
                    handle_index += 1;
                }
                if best.0 == 0 {
                    break;
                }
            }
            if best.0 == 0 {
                break;
            }
        }
        if best.0 == 0 {
            break;
        }
    }

    if known_layer_handles.contains(&best.1) {
        if debug_this {
            eprintln!(
                "[entity-layer] handle={} selected={}",
                object_handle, best.1
            );
        }
        return best.1;
    }
    if known_layer_handles.contains(&parsed_layer_handle) {
        return parsed_layer_handle;
    }
    if let Some(default_layer) = known_layer_handles.iter().copied().min() {
        return default_layer;
    }
    best.1
}

fn parse_expected_entity_layer_ref_index(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
) -> Option<usize> {
    let object_data_end_bit = resolve_r2010_object_data_end_bit(api_header).ok()?;
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let header = match version {
        version::DwgVersion::R2010 => {
            entities::common::parse_common_entity_header_r2010(&mut reader, object_data_end_bit)
                .ok()?
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            entities::common::parse_common_entity_header_r2013(&mut reader, object_data_end_bit)
                .ok()?
        }
        _ => return None,
    };

    let mut index = 0usize;
    if header.entity_mode == 0 {
        index = index.saturating_add(1);
    }
    index = index.saturating_add(header.num_of_reactors as usize);
    if header.xdic_missing_flag == 0 {
        index = index.saturating_add(1);
    }
    if matches!(api_header.type_code, 0x15 | 0x19 | 0x1A) {
        // R2010+ dimensions keep dimstyle and anonymous block handles
        // before common entity handles.
        index = index.saturating_add(2);
    }

    let debug_entity_handle = std::env::var("EZDWG_DEBUG_ENTITY_LAYER")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    if debug_entity_handle == Some(object_handle) {
        eprintln!(
            concat!(
                "[entity-layer] handle={} expected_index={} entity_mode={} reactors={} ",
                "xdic_missing={} ltype_flags={} plotstyle_flags={} material_flags={} type=0x{:X}",
            ),
            object_handle,
            index,
            header.entity_mode,
            header.num_of_reactors,
            header.xdic_missing_flag,
            header.ltype_flags,
            header.plotstyle_flags,
            header.material_flags,
            api_header.type_code
        );
    }

    Some(index)
}

fn parse_common_entity_layer_handle_from_common_header(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
) -> Option<u64> {
    let object_data_end_bit = resolve_r2010_object_data_end_bit(api_header).ok()?;
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let header = match version {
        version::DwgVersion::R2010 => {
            entities::common::parse_common_entity_header_r2010(&mut reader, object_data_end_bit)
                .ok()?
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            entities::common::parse_common_entity_header_r2013(&mut reader, object_data_end_bit)
                .ok()?
        }
        _ => return None,
    };
    reader.set_bit_pos(header.obj_size);
    entities::common::parse_common_entity_layer_handle(&mut reader, &header).ok()
}

fn layer_handle_score(layer_handle: u64, known_layer_handles: &HashSet<u64>) -> u64 {
    if known_layer_handles.contains(&layer_handle) {
        0
    } else if layer_handle == 0 {
        10_000
    } else {
        50_000
    }
}

fn decode_layer_color_record(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    expected_handle: u64,
) -> crate::core::result::Result<(u64, u16, Option<u32>)> {
    // R2010+/R2013 objects start with handle directly after OT prefix.
    // Older versions keep ObjSize (RL) before handle.
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _obj_size = reader.read_rl(Endian::Little)?;
    }
    let record_handle = reader.read_h()?.value;
    skip_eed(reader)?;

    let _num_reactors = reader.read_bl()?;
    let _xdic_missing_flag = reader.read_b()?;
    if matches!(
        version,
        version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _has_ds_binary_data = reader.read_b()?;
    }
    // R2010+ stores entry name in string stream. The data stream directly
    // continues with layer state flags and color data.
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _entry_name = reader.read_tv()?;
    }

    let style_start = reader.get_pos();
    let variants = [
        LayerColorParseVariant {
            pre_flag_bits: 0,
            post_flag_bits: 0,
            pre_values_bits: 0,
        },
        LayerColorParseVariant {
            pre_flag_bits: 2,
            post_flag_bits: 0,
            pre_values_bits: 0,
        },
        LayerColorParseVariant {
            pre_flag_bits: 0,
            post_flag_bits: 2,
            pre_values_bits: 0,
        },
        LayerColorParseVariant {
            pre_flag_bits: 0,
            post_flag_bits: 0,
            pre_values_bits: 2,
        },
        LayerColorParseVariant {
            pre_flag_bits: 2,
            post_flag_bits: 2,
            pre_values_bits: 0,
        },
        LayerColorParseVariant {
            pre_flag_bits: 2,
            post_flag_bits: 0,
            pre_values_bits: 2,
        },
        LayerColorParseVariant {
            pre_flag_bits: 0,
            post_flag_bits: 2,
            pre_values_bits: 2,
        },
        LayerColorParseVariant {
            pre_flag_bits: 2,
            post_flag_bits: 2,
            pre_values_bits: 2,
        },
    ];

    let mut best: Option<(u64, (u16, Option<u32>))> = None;
    for variant in variants {
        reader.set_pos(style_start.0, style_start.1);
        let Ok((color_index, true_color, color_byte)) = decode_layer_color_cmc(reader, variant)
        else {
            continue;
        };
        let score = layer_color_candidate_score(color_index, true_color, color_byte);
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, (color_index, true_color))),
        }
    }

    if let Some((_, (color_index, true_color))) = best {
        let handle = if record_handle != 0 {
            record_handle
        } else {
            expected_handle
        };
        return Ok((handle, color_index, true_color));
    }

    // Last resort: parse in the simplest form to keep progress.
    reader.set_pos(style_start.0, style_start.1);
    let (color_index, true_color, _) = decode_layer_color_cmc(reader, variants[0])?;
    let handle = if record_handle != 0 {
        record_handle
    } else {
        expected_handle
    };
    Ok((handle, color_index, true_color))
}

fn decode_layer_color_cmc(
    reader: &mut BitReader<'_>,
    variant: LayerColorParseVariant,
) -> crate::core::result::Result<(u16, Option<u32>, u8)> {
    if variant.pre_flag_bits > 0 {
        let _unknown = reader.read_bits_msb(variant.pre_flag_bits)?;
    }
    let _flag_64 = reader.read_b()?;
    if variant.post_flag_bits > 0 {
        let _unknown = reader.read_bits_msb(variant.post_flag_bits)?;
    }
    let _xref_index_plus_one = reader.read_bs()?;
    let _xdep = reader.read_b()?;
    let _frozen = reader.read_b()?;
    let _on = reader.read_b()?;
    let _frozen_new = reader.read_b()?;
    let _locked = reader.read_b()?;
    if variant.pre_values_bits > 0 {
        let _unknown = reader.read_bits_msb(variant.pre_values_bits)?;
    }
    let _values = reader.read_bs()?;

    let color_index = reader.read_bs()?;
    let color_rgb = reader.read_bl()?;
    let color_byte = reader.read_rc()?;
    if (color_byte & 0x01) != 0 {
        let _color_name = reader.read_tv()?;
    }
    if (color_byte & 0x02) != 0 {
        let _book_name = reader.read_tv()?;
    }

    let true_color = if color_rgb == 0 || (color_rgb >> 24) == 0 {
        // Keep only true 24-bit payload with marker byte present.
        // If high byte is zero, treat as unset to prefer indexed color.
        None
    } else {
        let rgb = color_rgb & 0x00FF_FFFF;
        if rgb == 0 {
            None
        } else {
            Some(rgb)
        }
    };
    Ok((color_index, true_color, color_byte))
}

fn layer_color_candidate_score(color_index: u16, true_color: Option<u32>, color_byte: u8) -> u64 {
    let mut score = 0u64;

    if color_index <= 257 {
        score += 0;
    } else if color_index <= 4096 {
        score += 1_000;
    } else {
        score += 100_000;
    }

    if color_byte <= 3 {
        score += 0;
    } else {
        score += 10_000;
    }

    if let Some(rgb) = true_color {
        if rgb == 0 || rgb > 0x00FF_FFFF {
            score += 10_000;
        }
    }

    score
}
