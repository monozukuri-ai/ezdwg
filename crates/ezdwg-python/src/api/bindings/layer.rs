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

#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_names(path: &str, limit: Option<usize>) -> PyResult<Vec<LayerNameRow>> {
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
        let (handle, name) =
            match decode_layer_name_record(&record, &header, decoder.version(), obj.handle.0) {
                Ok(decoded) => decoded,
                Err(_err)
                    if matches!(
                        decoder.version(),
                        version::DwgVersion::R2010
                            | version::DwgVersion::R2013
                            | version::DwgVersion::R2018
                    ) =>
                {
                    match decode_layer_name_record_from_shifted_utf16_fallback(
                        &record,
                        obj.handle.0,
                    ) {
                        Ok(decoded) => decoded,
                        Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                        Err(err) => return Err(to_py_err(err)),
                    }
                }
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((handle, name));
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
    let allow_exact_zero_layer_bonus =
        parse_allow_exact_zero_layer_bonus(record, version, api_header).unwrap_or(false);
    let canonical_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
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
                    let score = layer_handle_candidate_score(
                        layer_handle,
                        handle_index,
                        expected_layer_index,
                        object_data_end_bit,
                        canonical_end_bit,
                        chained_base,
                        parsed_layer_handle,
                        default_layer,
                        allow_exact_zero_layer_bonus,
                        known_layer_handles,
                    );
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
    if best.1 == 0 {
        if debug_this {
            eprintln!("[entity-layer] handle={} selected=0", object_handle);
        }
        return 0;
    }
    if known_layer_handles.contains(&parsed_layer_handle) {
        return parsed_layer_handle;
    }
    if parsed_layer_handle == 0 {
        return 0;
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

fn parse_allow_exact_zero_layer_bonus(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
) -> Option<bool> {
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

    Some(
        header.ltype_flags != 3
            && header.plotstyle_flags != 3
            && header.material_flags != 3
            && !header.has_full_visual_style
            && !header.has_face_visual_style
            && !header.has_edge_visual_style,
    )
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

fn layer_handle_candidate_score(
    layer_handle: u64,
    handle_index: u64,
    expected_layer_index: Option<usize>,
    object_data_end_bit: u32,
    canonical_end_bit: Option<u32>,
    chained_base: bool,
    parsed_layer_handle: u64,
    default_layer: Option<u64>,
    allow_exact_zero_layer_bonus: bool,
    known_layer_handles: &HashSet<u64>,
) -> u64 {
    let mut score = layer_handle_score(layer_handle, known_layer_handles).saturating_add(handle_index);
    if let Some(expected) = expected_layer_index {
        let expected_index = expected as u64;
        let distance = handle_index.abs_diff(expected_index);
        score = score.saturating_add(distance.saturating_mul(48));
        if handle_index == expected_index {
            score = score.saturating_sub(120);
            if layer_handle == 0 && allow_exact_zero_layer_bonus {
                // Layer 0 is valid; don't force it behind a later known layer candidate.
                score = score.saturating_sub(9_880);
            }
        }
    }
    if handle_index == 0 && expected_layer_index != Some(0) {
        // The first handle is often owner-related unless the entity header says layer is first.
        score = score.saturating_add(200);
    }
    if let Some(canonical) = canonical_end_bit {
        score = score.saturating_add(u64::from(canonical.abs_diff(object_data_end_bit) / 2));
    }
    if chained_base {
        // Relative-to-previous mode is speculative; keep fixed-base preference.
        score = score.saturating_add(20);
    }
    if layer_handle == parsed_layer_handle && known_layer_handles.contains(&layer_handle) {
        score = score.saturating_sub(80);
    }
    if Some(layer_handle) == default_layer {
        score = score.saturating_add(150);
    }
    score
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

fn decode_layer_name_record(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
    expected_handle: u64,
) -> crate::core::result::Result<(u64, String)> {
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version)?;
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _obj_size = reader.read_rl(Endian::Little)?;
    }
    let record_handle = reader.read_h()?.value;
    skip_eed(&mut reader)?;

    let _num_reactors = reader.read_bl()?;
    let _xdic_missing_flag = reader.read_b()?;
    if matches!(
        version,
        version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _has_ds_binary_data = reader.read_b()?;
    }

    let name = if matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        decode_layer_name_from_string_stream(record, api_header, version)?
    } else {
        reader.read_tv()?
    };

    let handle = if record_handle != 0 {
        record_handle
    } else {
        expected_handle
    };
    Ok((handle, name))
}

fn decode_layer_name_record_from_shifted_utf16_fallback(
    record: &objects::ObjectRecord<'_>,
    expected_handle: u64,
) -> crate::core::result::Result<(u64, String)> {
    let mut best: Option<(u64, String)> = None;
    scan_shifted_utf16_layer_name_candidates(record.raw.as_ref(), &mut best);
    best.filter(|(score, _)| *score <= 1_536)
        .map(|(_score, name)| (expected_handle, name))
        .ok_or_else(|| {
            DwgError::new(
                ErrorKind::Format,
                "failed to decode layer name from shifted utf16 fallback",
            )
        })
}

fn decode_layer_name_from_string_stream(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
) -> crate::core::result::Result<String> {
    let total_bits = api_header.data_size.saturating_mul(8);
    let canonical_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
    let mut base_reader = record.bit_reader();
    skip_object_type_prefix(&mut base_reader, version)?;

    let mut best: Option<(u64, String)> = None;
    let mut end_bit_candidates = resolve_r2010_object_data_end_bit_candidates(api_header);
    end_bit_candidates.push(total_bits);
    end_bit_candidates.retain(|candidate| *candidate > 0 && *candidate <= total_bits);
    end_bit_candidates.sort_unstable();
    end_bit_candidates.dedup();

    for object_data_end_bit in end_bit_candidates {
        for (stream_start_bit, stream_end_bit) in
            resolve_r2010_string_stream_ranges(&base_reader, object_data_end_bit)
        {
            scan_layer_name_range(
                &base_reader,
                stream_start_bit,
                stream_end_bit,
                canonical_end_bit.map(|canonical| canonical.abs_diff(object_data_end_bit) as u64),
                0,
                false,
                &mut best,
            );
        }
    }

    if best.is_none() {
        let scan_start_bit = base_reader.tell_bits() as u32;
        if scan_start_bit < total_bits {
            scan_layer_name_range(
                &base_reader,
                scan_start_bit,
                total_bits,
                canonical_end_bit.map(|canonical| canonical.abs_diff(total_bits) as u64),
                64,
                true,
                &mut best,
            );
        }
    }

    if best
        .as_ref()
        .map(|(_, name)| layer_name_needs_shifted_utf16_fallback(name))
        .unwrap_or(true)
    {
        if let Some((score, name)) = best.as_mut() {
            if layer_name_needs_shifted_utf16_fallback(name) {
                *score = score.saturating_add(2_048);
            }
        }
        scan_shifted_utf16_layer_name_candidates(record.raw.as_ref(), &mut best);
    }

    best.filter(|(score, _)| *score <= 1_536)
        .map(|(_score, name)| name)
        .ok_or_else(|| {
        DwgError::new(
            ErrorKind::Format,
            "failed to decode layer name from string stream",
        )
    })
}

fn scan_layer_name_range(
    base_reader: &BitReader<'_>,
    start_bit: u32,
    end_bit: u32,
    end_bit_penalty: Option<u64>,
    fallback_bias: u64,
    allow_tv: bool,
    best: &mut Option<(u64, String)>,
) {
    if start_bit >= end_bit {
        return;
    }
    let mut bit = start_bit;
    while bit.saturating_add(16) <= end_bit {
        let decoders = if allow_tv {
            [false, true]
        } else {
            [false, false]
        };
        for (decoder_index, prefer_tv) in decoders.into_iter().enumerate() {
            if !allow_tv && decoder_index > 0 {
                break;
            }
            let mut reader = base_reader.clone();
            reader.set_bit_pos(bit);
            let name = if prefer_tv {
                reader.read_tv()
            } else {
                reader.read_tu()
            };
            let Ok(name) = name else {
                continue;
            };
            if reader.tell_bits() > u64::from(end_bit) {
                continue;
            }
            let trimmed = name.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut score = layer_name_candidate_score(trimmed);
            if let Some(penalty) = end_bit_penalty {
                score = score.saturating_add(penalty);
            }
            score = score.saturating_add((u64::from(end_bit) - reader.tell_bits()).saturating_div(128));
            score = score.saturating_add(fallback_bias);
            if prefer_tv {
                score = score.saturating_add(4);
            }
            update_best_layer_name_candidate(best, score, trimmed);
        }
        bit = bit.saturating_add(1);
    }
}

fn layer_name_needs_shifted_utf16_fallback(name: &str) -> bool {
    if name.is_empty() {
        return true;
    }
    if name.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    let trimmed = name.trim();
    trimmed.chars().count() <= 2
}

fn scan_shifted_utf16_layer_name_candidates(raw: &[u8], best: &mut Option<(u64, String)>) {
    if raw.len() < 6 {
        return;
    }
    for shift in 0..8u8 {
        let shifted = shift_bits_bytes(raw, shift);
        for parity in 0..=1usize {
            let mut index = parity;
            while index + 6 <= shifted.len() {
                let mut cursor = index;
                let mut units = Vec::new();
                while cursor + 1 < shifted.len() {
                    let code = u16::from_le_bytes([shifted[cursor], shifted[cursor + 1]]);
                    if code == 0 {
                        break;
                    }
                    if code == 0x3000 || (32..=0x9FFF).contains(&code) {
                        units.push(code);
                        cursor += 2;
                        continue;
                    }
                    break;
                }
                if units.len() >= 3 {
                    let name = String::from_utf16_lossy(&units);
                    let Some(fragment) = extract_plausible_layer_name_fragment(&name) else {
                        index = cursor;
                        continue;
                    };
                    if !fragment.is_empty() {
                        let mut score = layer_name_candidate_score(&fragment);
                        score = score.saturating_add(u64::from(shift).saturating_mul(8));
                        score = score.saturating_add(shifted_utf16_layer_name_candidate_penalty(
                            &fragment, shift,
                        ));
                        if parity != 0 {
                            score = score.saturating_add(4);
                        }
                        update_best_layer_name_candidate(best, score, &fragment);
                    }
                    index = cursor;
                } else {
                    index += 2;
                }
            }
        }
    }
}

fn shift_bits_bytes(raw: &[u8], shift: u8) -> Vec<u8> {
    if shift == 0 {
        return raw.to_vec();
    }
    let mut out = vec![0u8; raw.len()];
    let mut carry = 0u8;
    for (index, value) in raw.iter().copied().enumerate() {
        out[index] = ((value >> shift) | carry) & 0xFF;
        carry = value.wrapping_shl((8 - shift) as u32);
    }
    out
}

fn is_plausible_layer_name_fragment_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(ch, '_' | '-' | '.' | '$' | '*' | ' ' | '/' | '(' | ')' | '[' | ']')
        || ('\u{FF61}'..='\u{FF9F}').contains(&ch)
        || ('\u{3040}'..='\u{30FF}').contains(&ch)
        || ('\u{4E00}'..='\u{9FFF}').contains(&ch)
}

fn extract_plausible_layer_name_fragment(text: &str) -> Option<String> {
    let mut best = String::new();
    let mut current = String::new();
    for ch in text.chars() {
        if is_plausible_layer_name_fragment_char(ch) {
            if should_split_ascii_layer_token_before_cjk(&current, ch) {
                update_best_layer_name_fragment(&mut best, &current);
                current.clear();
            }
            current.push(ch);
            continue;
        }
        update_best_layer_name_fragment(&mut best, &current);
        current.clear();
    }
    update_best_layer_name_fragment(&mut best, &current);
    if best.is_empty() {
        None
    } else {
        Some(best)
    }
}

fn should_split_ascii_layer_token_before_cjk(current: &str, next: char) -> bool {
    if !('\u{4E00}'..='\u{9FFF}').contains(&next) {
        return false;
    }
    if current.chars().count() < 3 {
        return false;
    }
    if !current
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '$' | '*' | ' ' | '/' | '(' | ')' | '[' | ']'))
    {
        return false;
    }
    if !current.chars().any(|ch| ch.is_ascii_alphabetic()) {
        return false;
    }
    !matches!(
        current.chars().last(),
        Some('_' | '-' | '.' | '$' | '*' | ' ' | '/' | '(' | ')' | '[' | ']')
    )
}

fn update_best_layer_name_fragment(best: &mut String, current: &str) {
    let candidate = current.trim().to_string();
    if !candidate.is_empty()
        && (best.is_empty()
            || layer_name_candidate_score(&candidate) < layer_name_candidate_score(best)
            || (layer_name_candidate_score(&candidate) == layer_name_candidate_score(best)
                && candidate.chars().count() > best.chars().count()))
    {
        *best = candidate;
    }
}

fn shifted_utf16_layer_name_candidate_penalty(name: &str, _shift: u8) -> u64 {
    let mut score = 0u64;
    let char_count = name.chars().count();
    let has_ascii_alpha = name.chars().any(|ch| ch.is_ascii_alphabetic());
    let has_non_ascii = name.chars().any(|ch| !ch.is_ascii());
    let has_separator = name
        .chars()
        .any(|ch| matches!(ch, '_' | '-' | '.' | '$' | '*' | ' ' | '/' | '(' | ')' | '[' | ']'));

    if char_count <= 2 && !name.chars().all(|ch| ch.is_ascii_digit()) {
        score = score.saturating_add(1_024);
    }
    if has_non_ascii && !has_ascii_alpha && !has_separator && char_count < 4 {
        score = score.saturating_add(512);
    }
    if name
        .chars()
        .all(|ch| ('\u{4E00}'..='\u{9FFF}').contains(&ch))
        && char_count <= 6
    {
        score = score.saturating_add(1_536);
    }
    score
}

fn update_best_layer_name_candidate(
    best: &mut Option<(u64, String)>,
    score: u64,
    candidate: &str,
) {
    match best {
        Some((best_score, best_name))
            if score > *best_score
                || (score == *best_score
                    && candidate.chars().count() <= best_name.chars().count()) => {}
        _ => *best = Some((score, candidate.to_string())),
    }
}

fn layer_name_candidate_score(name: &str) -> u64 {
    let mut score = 0u64;
    if name.is_empty() {
        return 1_000_000;
    }
    if name.len() > 255 {
        score = score.saturating_add(10_000);
    }
    if name.chars().any(|ch| ch.is_control()) {
        score = score.saturating_add(10_000);
    }
    if name.chars().any(|ch| ch == '\u{FFFD}') {
        score = score.saturating_add(10_000);
    }
    let has_visible = name.chars().any(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(ch, '_' | '-' | '.' | '$' | '*' | ' ')
            || matches!(ch, '/' | '(' | ')' | '[' | ']' | '、' | '・')
            || ('\u{FF61}'..='\u{FF9F}').contains(&ch)
            || ('\u{3040}'..='\u{30FF}').contains(&ch)
            || ('\u{4E00}'..='\u{9FFF}').contains(&ch)
    });
    if !has_visible {
        score = score.saturating_add(100_000);
    }
    let disallowed = name
        .chars()
        .filter(|&ch| {
            !ch.is_ascii_alphanumeric()
                && !matches!(ch, '_' | '-' | '.' | '$' | '*' | ' ' | '/' | '(' | ')' | '[' | ']' | '、' | '・')
                && !('\u{FF61}'..='\u{FF9F}').contains(&ch)
                && !('\u{3040}'..='\u{30FF}').contains(&ch)
                && !('\u{4E00}'..='\u{9FFF}').contains(&ch)
        })
        .count();
    score = score.saturating_add((disallowed as u64).saturating_mul(1_024));
    if name.chars().all(|ch| ch.is_ascii_digit()) {
        score = score.saturating_add(500);
    }
    score.saturating_add(name.len() as u64 / 64)
}

#[cfg(test)]
mod layer_name_tests {
    use super::{
        extract_plausible_layer_name_fragment, layer_handle_candidate_score,
        layer_name_needs_shifted_utf16_fallback, scan_shifted_utf16_layer_name_candidates,
        shifted_utf16_layer_name_candidate_penalty,
    };
    use std::collections::HashSet;

    #[test]
    fn shifted_utf16_layer_name_scan_recovers_utf16_name_run() {
        let utf16: Vec<u8> = "SD-FRAME_TEXT\0"
            .encode_utf16()
            .flat_map(|unit| unit.to_le_bytes())
            .collect();
        let mut best = None;

        scan_shifted_utf16_layer_name_candidates(&utf16, &mut best);

        assert_eq!(
            best.map(|(_, name)| name),
            Some("SD-FRAME_TEXT".to_string())
        );
    }

    #[test]
    fn plausible_layer_name_fragment_drops_garbage_prefix_and_suffix() {
        assert_eq!(
            extract_plausible_layer_name_fragment("ఃഁSD-FRAME_TEXTÚ膠⠨"),
            Some("SD-FRAME_TEXT".to_string())
        );
        assert_eq!(
            extract_plausible_layer_name_fragment("ఃЁAODGJ膠⠨"),
            Some("AODGJ".to_string())
        );
        assert_eq!(
            extract_plausible_layer_name_fragment("ఃँA14-躯体-点線\u{009A}膠⠨"),
            Some("A14-躯体-点線".to_string())
        );
    }

    #[test]
    fn layer_name_shifted_fallback_only_for_suspicious_short_names() {
        assert!(layer_name_needs_shifted_utf16_fallback("喏"));
        assert!(!layer_name_needs_shifted_utf16_fallback("0"));
        assert!(!layer_name_needs_shifted_utf16_fallback("SD-FRAME"));
    }

    #[test]
    fn shifted_utf16_layer_name_candidate_penalty_prefers_longer_ascii_name() {
        assert!(
            shifted_utf16_layer_name_candidate_penalty("胀", 0)
                > shifted_utf16_layer_name_candidate_penalty("AODGJ", 6)
        );
        assert!(
            shifted_utf16_layer_name_candidate_penalty("懀轪", 0)
                > shifted_utf16_layer_name_candidate_penalty("SD-FRAME_TEXT", 6)
        );
        assert!(
            shifted_utf16_layer_name_candidate_penalty("袂詺蠺育", 2)
                > shifted_utf16_layer_name_candidate_penalty("SD-FRAME_TEXT", 6)
        );
        assert!(
            shifted_utf16_layer_name_candidate_penalty("興鸀蠀踀鐀", 5)
                > shifted_utf16_layer_name_candidate_penalty("AODGJ", 6)
        );
    }

    #[test]
    fn layer_candidate_score_allows_exact_layer_zero_to_beat_misaligned_known_layer() {
        let known = HashSet::from([160u64]);
        let zero_score = layer_handle_candidate_score(
            0,
            0,
            Some(0),
            413,
            Some(397),
            false,
            81,
            Some(130),
            true,
            &known,
        );
        let known_score = layer_handle_candidate_score(
            160,
            1,
            Some(0),
            413,
            Some(397),
            false,
            81,
            Some(130),
            true,
            &known,
        );

        assert!(zero_score < known_score);
    }

    #[test]
    fn layer_candidate_score_prefers_expected_first_handle_when_entity_mode_has_no_owner() {
        let known = HashSet::from([160u64]);
        let exact_first_score = layer_handle_candidate_score(
            160,
            0,
            Some(0),
            549,
            Some(509),
            false,
            81,
            Some(130),
            false,
            &known,
        );
        let later_score = layer_handle_candidate_score(
            160,
            1,
            Some(0),
            533,
            Some(509),
            false,
            81,
            Some(130),
            false,
            &known,
        );

        assert!(exact_first_score < later_score);
    }

    #[test]
    fn layer_candidate_score_does_not_prefer_zero_without_zero_bonus() {
        let known = HashSet::from([160u64]);
        let zero_score = layer_handle_candidate_score(
            0,
            0,
            Some(0),
            493,
            Some(509),
            false,
            81,
            Some(130),
            false,
            &known,
        );
        let known_score = layer_handle_candidate_score(
            160,
            0,
            Some(0),
            549,
            Some(509),
            false,
            81,
            Some(130),
            false,
            &known,
        );

        assert!(known_score < zero_score);
    }
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
