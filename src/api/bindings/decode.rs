#[pyfunction]
pub fn detect_version(path: &str) -> PyResult<String> {
    let tag = file_open::read_version_tag(path).map_err(to_py_err)?;
    let version = version::detect_version(&tag).map_err(to_py_err)?;
    Ok(version.as_str().to_string())
}

#[pyfunction]
pub fn list_section_locators(path: &str) -> PyResult<Vec<SectionLocatorRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let directory = decoder.section_directory().map_err(to_py_err)?;
    let result = directory
        .records
        .into_iter()
        .map(|record| {
            let label = record.name.clone().unwrap_or_else(|| record.kind().label());
            (label, record.offset, record.size)
        })
        .collect();
    Ok(result)
}

#[pyfunction]
pub fn read_section_bytes(path: &str, index: usize) -> PyResult<Vec<u8>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let directory = decoder.section_directory().map_err(to_py_err)?;
    let section = decoder
        .load_section_by_index(&directory, index)
        .map_err(to_py_err)?;
    Ok(section.data.as_ref().to_vec())
}

#[pyfunction(signature = (path, limit=None))]
pub fn list_object_map_entries(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectMapEntryRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut entries: Vec<ObjectMapEntryRow> = index
        .objects
        .iter()
        .map(|obj| (obj.handle.0, obj.offset))
        .collect();
    if let Some(limit) = limit {
        if entries.len() > limit {
            entries.truncate(limit);
        }
    }
    Ok(entries)
}

#[pyfunction(signature = (path, limit=None))]
pub fn list_object_headers(path: &str, limit: Option<usize>) -> PyResult<Vec<ObjectHeaderRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((obj.handle.0, obj.offset, header.data_size, header.type_code));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn list_object_headers_with_type(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectHeaderWithTypeRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        let type_class = resolved_type_class(header.type_code, &type_name);
        result.push((
            obj.handle.0,
            obj.offset,
            header.data_size,
            header.type_code,
            type_name,
            type_class,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, type_codes, limit=None))]
pub fn list_object_headers_by_type(
    path: &str,
    type_codes: Vec<u16>,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectHeaderWithTypeRow>> {
    if type_codes.is_empty() {
        return Ok(Vec::new());
    }
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let filter: HashSet<u16> = type_codes.into_iter().collect();
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        if !matches_type_filter(&filter, header.type_code, &type_name) {
            continue;
        }
        let type_class = resolved_type_class(header.type_code, &type_name);
        result.push((
            obj.handle.0,
            obj.offset,
            header.data_size,
            header.type_code,
            type_name,
            type_class,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, type_codes, limit=None))]
pub fn read_object_records_by_type(
    path: &str,
    type_codes: Vec<u16>,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectRecordBytesRow>> {
    if type_codes.is_empty() {
        return Ok(Vec::new());
    }
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let filter: HashSet<u16> = type_codes.into_iter().collect();
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        if !matches_type_filter(&filter, header.type_code, &type_name) {
            continue;
        }
        let record = record.raw.as_ref().to_vec();
        result.push((
            obj.handle.0,
            obj.offset,
            header.data_size,
            header.type_code,
            record,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, handles, limit=None))]
pub fn read_object_records_by_handle(
    path: &str,
    handles: Vec<u64>,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectRecordBytesRow>> {
    if handles.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let target_handles: HashSet<u64> = handles.iter().copied().collect();
    let mut found_rows: HashMap<u64, ObjectRecordBytesRow> = HashMap::new();

    for obj in index.objects.iter() {
        let handle = obj.handle.0;
        if !target_handles.contains(&handle) || found_rows.contains_key(&handle) {
            continue;
        }
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        found_rows.insert(
            handle,
            (
                handle,
                obj.offset,
                header.data_size,
                header.type_code,
                record.raw.as_ref().to_vec(),
            ),
        );
        if found_rows.len() >= target_handles.len() {
            break;
        }
    }

    let mut result = Vec::new();
    for handle in handles {
        if let Some(row) = found_rows.remove(&handle) {
            result.push(row);
            if let Some(limit) = limit {
                if result.len() >= limit {
                    break;
                }
            }
        }
    }
    Ok(result)
}

fn decode_known_handle_refs_from_object_record(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
    known_handles: &HashSet<u64>,
    object_type_codes: Option<&HashMap<u64, u16>>,
    max_refs: usize,
) -> KnownHandleRefsDecode {
    let total_bits = u64::from(header.data_size.saturating_mul(8));
    let start_candidates =
        resolve_handle_stream_start_candidates(version, header, header.type_code);
    if start_candidates.is_empty() {
        return KnownHandleRefsDecode::default();
    }
    let canonical_start = resolve_r2010_object_data_end_bit(header).ok();
    let preferred_ref_types = preferred_ref_type_codes_for_acis_unknown(header.type_code);
    let mut best: Option<(i64, i64, usize, u32, Vec<u64>)> = None;
    let mut second_score: Option<i64> = None;

    for start_bit in start_candidates {
        let mut reader = record.bit_reader();
        if skip_object_type_prefix(&mut reader, version).is_err() {
            continue;
        }
        reader.set_bit_pos(start_bit);

        let mut refs: Vec<u64> = Vec::new();
        let mut seen: HashSet<u64> = HashSet::new();
        let mut quality_score: i64 = 0;
        for _ in 0..128usize {
            if reader.tell_bits() >= total_bits {
                break;
            }
            let before_bits = reader.tell_bits();
            let value = match entities::common::read_handle_reference(&mut reader, object_handle) {
                Ok(value) => value,
                Err(_) => break,
            };
            if reader.tell_bits() <= before_bits {
                break;
            }
            if value == 0 || value == object_handle || !known_handles.contains(&value) {
                continue;
            }
            if seen.insert(value) {
                refs.push(value);
                if let Some(type_codes) = object_type_codes {
                    if let Some(ref_type_code) = type_codes.get(&value) {
                        if preferred_ref_types.contains(ref_type_code) {
                            quality_score += 6;
                        } else if (0x214..=0x225).contains(ref_type_code) {
                            quality_score += 3;
                        } else if matches!(*ref_type_code, 0x25 | 0x26 | 0x27) {
                            quality_score += 2;
                        } else if *ref_type_code == 0x33 {
                            quality_score -= 2;
                        }
                    }
                }
                if refs.len() >= max_refs {
                    break;
                }
            }
        }

        let delta = canonical_start
            .map(|canonical| canonical.abs_diff(start_bit))
            .unwrap_or(0);
        let score = quality_score
            .saturating_mul(32)
            .saturating_add((refs.len() as i64).saturating_mul(4))
            .saturating_sub(i64::from(delta));

        let should_replace_best = match &best {
            Some((best_score, _best_quality, best_len, best_delta, _))
                if score < *best_score
                    || (score == *best_score
                        && (refs.len() < *best_len
                            || (refs.len() == *best_len && delta >= *best_delta))) =>
            {
                false
            }
            _ => true,
        };
        if should_replace_best {
            if let Some((prev_best_score, _, _, _, _)) = &best {
                second_score = Some(
                    second_score
                        .map(|value| value.max(*prev_best_score))
                        .unwrap_or(*prev_best_score),
                );
            }
            best = Some((score, quality_score, refs.len(), delta, refs));
        } else {
            second_score = Some(second_score.map(|value| value.max(score)).unwrap_or(score));
        }
    }

    if let Some((best_score, quality_score, _len, _delta, refs)) = best {
        let confidence = derive_known_handle_refs_confidence(
            refs.len(),
            quality_score,
            best_score,
            second_score,
        );
        KnownHandleRefsDecode { refs, confidence }
    } else {
        KnownHandleRefsDecode::default()
    }
}

#[pyfunction(signature = (path, handles, limit=None))]
pub fn decode_object_handle_stream_refs(
    path: &str,
    handles: Vec<u64>,
    limit: Option<usize>,
) -> PyResult<Vec<HandleStreamRefsRow>> {
    if handles.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let object_offsets: HashMap<u64, u32> = index
        .objects
        .iter()
        .map(|obj| (obj.handle.0, obj.offset))
        .collect();

    let mut result = Vec::new();
    for handle in handles {
        let Some(offset) = object_offsets.get(&handle).copied() else {
            continue;
        };
        let Some((record, header)) = parse_record_and_header(&decoder, offset, best_effort)? else {
            continue;
        };
        let decoded = decode_known_handle_refs_from_object_record(
            &record,
            decoder.version(),
            &header,
            handle,
            &known_handles,
            None,
            16,
        );
        result.push((handle, decoded.refs));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, handles, limit=None))]
pub fn decode_acis_candidate_infos(
    path: &str,
    handles: Vec<u64>,
    limit: Option<usize>,
) -> PyResult<Vec<AcisCandidateInfoRow>> {
    if handles.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let object_type_codes = collect_object_type_codes(&decoder, &index, best_effort)?;
    let object_offsets: HashMap<u64, u32> = index
        .objects
        .iter()
        .map(|obj| (obj.handle.0, obj.offset))
        .collect();

    let mut result = Vec::new();
    for handle in handles {
        let Some(offset) = object_offsets.get(&handle).copied() else {
            continue;
        };
        let Some((record, header)) = parse_record_and_header(&decoder, offset, best_effort)? else {
            continue;
        };
        let decoded = decode_known_handle_refs_from_object_record(
            &record,
            decoder.version(),
            &header,
            handle,
            &known_handles,
            Some(&object_type_codes),
            16,
        );
        let role = acis_unknown_role_hint_from_type_code(header.type_code, header.data_size);
        result.push((
            handle,
            header.type_code,
            header.data_size,
            role.to_string(),
            decoded.refs,
            decoded.confidence,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_entity_styles(path: &str, limit: Option<usize>) -> PyResult<Vec<EntityStyleRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let decoded_layer_rows = decode_layer_colors(path, None)?;
    let decoded_layer_handles: Vec<u64> = decoded_layer_rows.iter().map(|(h, _, _)| *h).collect();
    let raw_layer_handles =
        collect_known_layer_handles_in_order(&decoder, &dynamic_types, &index, best_effort)?;
    let mut layer_handle_remap = HashMap::new();
    if raw_layer_handles.len() == decoded_layer_handles.len() {
        for (raw, decoded) in raw_layer_handles
            .iter()
            .copied()
            .zip(decoded_layer_handles.iter().copied())
        {
            layer_handle_remap.insert(raw, decoded);
        }
    }
    let mut known_layer_handles: HashSet<u64> = decoded_layer_handles.into_iter().collect();
    known_layer_handles.extend(raw_layer_handles.iter().copied());
    let mut result = Vec::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        if matches_type_name(header.type_code, 0x13, "LINE", &dynamic_types) {
            let entity = match decode_line_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1B, "POINT", &dynamic_types) {
            let entity = match decode_point_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x11, "ARC", &dynamic_types) {
            let entity =
                match decode_arc_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
                {
                    Ok(entity) => entity,
                    Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                    Err(err) => return Err(to_py_err(err)),
                };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x12, "CIRCLE", &dynamic_types) {
            let entity = match decode_circle_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x23, "ELLIPSE", &dynamic_types) {
            let entity = match decode_ellipse_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x24, "SPLINE", &dynamic_types) {
            let entity = match decode_spline_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x01, "TEXT", &dynamic_types) {
            let entity = match decode_text_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x02, "ATTRIB", &dynamic_types) {
            let entity = match decode_attrib_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x03, "ATTDEF", &dynamic_types) {
            let entity = match decode_attdef_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2C, "MTEXT", &dynamic_types) {
            let entity = match decode_mtext_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2D, "LEADER", &dynamic_types) {
            let entity = match decode_leader_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x4E, "HATCH", &dynamic_types) {
            let entity = match decode_hatch_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2E, "TOLERANCE", &dynamic_types) {
            let entity = match decode_tolerance_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2F, "MLINE", &dynamic_types) {
            let entity = match decode_mline_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x4D, "LWPOLYLINE", &dynamic_types) {
            let entity = match decode_lwpolyline_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x10, "POLYLINE_3D", &dynamic_types) {
            let entity = match decode_polyline_3d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1E, "POLYLINE_MESH", &dynamic_types) {
            let entity = match decode_polyline_mesh_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1D, "POLYLINE_PFACE", &dynamic_types) {
            let entity = match decode_polyline_pface_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1C, "3DFACE", &dynamic_types) {
            let entity = match decode_3dface_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1F, "SOLID", &dynamic_types) {
            let entity = match decode_solid_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x20, "TRACE", &dynamic_types) {
            let entity = match decode_trace_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x21, "SHAPE", &dynamic_types) {
            let entity = match decode_shape_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x22, "VIEWPORT", &dynamic_types) {
            let entity = match decode_viewport_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2B, "OLEFRAME", &dynamic_types) {
            let entity = match decode_oleframe_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x4A, "OLE2FRAME", &dynamic_types) {
            let entity = match decode_ole2frame_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x4C, "LONG_TRANSACTION", &dynamic_types) {
            let entity = match decode_long_transaction_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x25, "REGION", &dynamic_types) {
            let entity = match decode_region_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x26, "3DSOLID", &dynamic_types) {
            let entity = match decode_3dsolid_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x27, "BODY", &dynamic_types) {
            let entity = match decode_body_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x28, "RAY", &dynamic_types) {
            let entity =
                match decode_ray_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
                {
                    Ok(entity) => entity,
                    Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                    Err(err) => return Err(to_py_err(err)),
                };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x29, "XLINE", &dynamic_types) {
            let entity = match decode_xline_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x15, "DIM_LINEAR", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x14, "DIM_ORDINATE", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x16, "DIM_ALIGNED", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x17, "DIM_ANG3PT", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x18, "DIM_ANG2LN", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1A, "DIM_DIAMETER", &dynamic_types) {
            let entity = match decode_dim_diameter_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x19, "DIM_RADIUS", &dynamic_types) {
            let entity = match decode_dim_radius_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else {
            continue;
        }

        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_line_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<LineEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x13, "LINE", &dynamic_types) {
            continue;
        }
        let mut entity: Option<entities::LineEntity> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_line_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(decoded) => {
                    if !is_plausible_line_entity_candidate(&decoded) {
                        continue;
                    }
                    entity = Some(decoded);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let entity = match entity {
            Some(entity) => entity,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        result.push((
            entity.handle,
            entity.start.0,
            entity.start.1,
            entity.start.2,
            entity.end.0,
            entity.end.1,
            entity.end.2,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_point_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<PointEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x1B, "POINT", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_point_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => {
                    if std::env::var("EZDWG_DEBUG_POINT_DECODE")
                        .ok()
                        .is_some_and(|value| value != "0")
                    {
                        eprintln!(
                            "[point-decode] handle={} type=0x{:X} offset={} error={}",
                            obj.handle.0, header.type_code, obj.offset, err
                        );
                    }
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.location.0,
            entity.location.1,
            entity.location.2,
            entity.x_axis_angle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_3dface_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<Face3dEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x1C, "3DFACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_3dface_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.p1,
            entity.p2,
            entity.p3,
            entity.p4,
            entity.invisible_edge_flags,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_arc_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<ArcEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x11, "ARC", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_arc_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.center.0,
            entity.center.1,
            entity.center.2,
            entity.radius,
            entity.angle_start,
            entity.angle_end,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_circle_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<CircleEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x12, "CIRCLE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_circle_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.center.0,
            entity.center.1,
            entity.center.2,
            entity.radius,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_line_arc_circle_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<LineArcCircleRows> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut lines: Vec<LineEntityRow> = Vec::new();
    let mut arcs: Vec<ArcEntityRow> = Vec::new();
    let mut circles: Vec<CircleEntityRow> = Vec::new();
    let mut total = 0usize;

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        let is_line = matches_type_name(header.type_code, 0x13, "LINE", &dynamic_types);
        let is_arc = matches_type_name(header.type_code, 0x11, "ARC", &dynamic_types);
        let is_circle = matches_type_name(header.type_code, 0x12, "CIRCLE", &dynamic_types);
        if !(is_line || is_arc || is_circle) {
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }

        if is_line {
            let entity = match decode_line_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            lines.push((
                entity.handle,
                entity.start.0,
                entity.start.1,
                entity.start.2,
                entity.end.0,
                entity.end.1,
                entity.end.2,
            ));
            total += 1;
        } else if is_arc {
            let entity =
                match decode_arc_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
                {
                    Ok(entity) => entity,
                    Err(err) if best_effort => continue,
                    Err(err) => return Err(to_py_err(err)),
                };
            arcs.push((
                entity.handle,
                entity.center.0,
                entity.center.1,
                entity.center.2,
                entity.radius,
                entity.angle_start,
                entity.angle_end,
            ));
            total += 1;
        } else if is_circle {
            let entity = match decode_circle_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            circles.push((
                entity.handle,
                entity.center.0,
                entity.center.1,
                entity.center.2,
                entity.radius,
            ));
            total += 1;
        }

        if let Some(limit) = limit {
            if total >= limit {
                break;
            }
        }
    }

    Ok((lines, arcs, circles))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_ellipse_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<EllipseEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x23, "ELLIPSE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_ellipse_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
            {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.center,
            entity.major_axis,
            entity.extrusion,
            entity.axis_ratio,
            entity.start_angle,
            entity.end_angle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_spline_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<SplineEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x24, "SPLINE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_spline_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            (
                entity.scenario,
                entity.degree,
                entity.rational,
                entity.closed,
                entity.periodic,
            ),
            (
                entity.fit_tolerance,
                entity.knot_tolerance,
                entity.ctrl_tolerance,
            ),
            entity.knots,
            entity.control_points,
            entity.weights,
            entity.fit_points,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_text_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<TextEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x01, "TEXT", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_text_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.text,
            entity.insertion,
            entity.alignment,
            entity.extrusion,
            (
                entity.thickness,
                entity.oblique_angle,
                entity.height,
                entity.rotation,
                entity.width_factor,
            ),
            (
                entity.generation,
                entity.horizontal_alignment,
                entity.vertical_alignment,
            ),
            entity.style_handle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_attrib_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<AttribEntityRow>> {
    decode_attrib_like_entities_by_type(
        path,
        limit,
        0x02,
        "ATTRIB",
        |reader, version, header, object_handle| {
            decode_attrib_for_version(reader, version, header, object_handle)
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_attdef_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<AttribEntityRow>> {
    decode_attrib_like_entities_by_type(
        path,
        limit,
        0x03,
        "ATTDEF",
        |reader, version, header, object_handle| {
            decode_attdef_for_version(reader, version, header, object_handle)
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_mtext_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<MTextEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x2C, "MTEXT", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let reader_after_prefix = reader.clone();
        let mut entity =
            match decode_mtext_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        if matches!(
            decoder.version(),
            version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
        ) {
            if let Some(recovered_text) =
                recover_r2010_mtext_text(&reader_after_prefix, &header, entity.text.as_str())
            {
                entity.text = recovered_text;
            }
        }
        result.push((
            entity.handle,
            entity.text,
            entity.insertion,
            entity.extrusion,
            entity.x_axis_dir,
            entity.rect_width,
            entity.text_height,
            entity.attachment,
            entity.drawing_dir,
            (
                entity.background_flags,
                entity.background_scale_factor,
                entity.background_color_index,
                entity.background_true_color,
                entity.background_transparency,
            ),
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_leader_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<LeaderEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x2D, "LEADER", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_leader_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.annotation_type,
            entity.path_type,
            entity.points,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_hatch_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<HatchEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x4E, "HATCH", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_hatch_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        let paths: Vec<HatchPathRow> = entity
            .paths
            .into_iter()
            .map(|path| (path.closed, path.points))
            .collect();
        result.push((
            entity.handle,
            entity.name,
            entity.solid_fill,
            entity.associative,
            entity.elevation,
            entity.extrusion,
            paths,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_tolerance_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ToleranceEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x2E, "TOLERANCE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_tolerance_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.text,
            entity.insertion,
            entity.x_direction,
            entity.extrusion,
            entity.height,
            entity.dimgap,
            entity.dimstyle_handle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_mline_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<MLineEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x2F, "MLINE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_mline_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        let vertices: Vec<MLineVertexRow> = entity
            .vertices
            .iter()
            .map(|vertex| {
                (
                    vertex.position,
                    vertex.vertex_direction,
                    vertex.miter_direction,
                )
            })
            .collect();
        result.push((
            entity.handle,
            entity.scale,
            entity.justification,
            entity.base_point,
            entity.extrusion,
            entity.open_closed,
            entity.lines_in_style,
            vertices,
            entity.mlinestyle_handle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_solid_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<SolidEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x1F, "SOLID", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_solid_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.p1,
            entity.p2,
            entity.p3,
            entity.p4,
            entity.thickness,
            entity.extrusion,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_trace_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<TraceEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x20, "TRACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_trace_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.p1,
            entity.p2,
            entity.p3,
            entity.p4,
            entity.thickness,
            entity.extrusion,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_shape_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<ShapeEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x21, "SHAPE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_shape_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.insertion,
            entity.scale,
            entity.rotation,
            entity.width_factor,
            entity.oblique,
            entity.thickness,
            entity.shape_no,
            entity.extrusion,
            entity.shapefile_handle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_viewport_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ViewportEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x22, "VIEWPORT", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_viewport_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle,));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_oleframe_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<OleFrameEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x2B, "OLEFRAME", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_oleframe_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle,));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_ole2frame_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<OleFrameEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x4A, "OLE2FRAME", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_ole2frame_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle,));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_long_transaction_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<LongTransactionEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x4C, "LONG_TRANSACTION", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_long_transaction_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.owner_handle,
            entity.reactor_handles,
            entity.xdic_obj_handle,
            entity.ltype_handle,
            entity.plotstyle_handle,
            entity.material_handle,
            entity.extra_handles,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_region_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<RegionEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_layer_handles: HashSet<u64> =
        collect_known_layer_handles_in_order(&decoder, &dynamic_types, &index, best_effort)?
            .into_iter()
            .collect();
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x25, "REGION", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_region_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let layer_handle = entity.layer_handle;
        let mut acis_handles = entity.acis_handles;
        acis_handles.retain(|handle| {
            *handle != layer_handle
                && known_handles.contains(handle)
                && !known_layer_handles.contains(handle)
        });
        result.push((entity.handle, acis_handles));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_3dsolid_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Solid3dEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_layer_handles: HashSet<u64> =
        collect_known_layer_handles_in_order(&decoder, &dynamic_types, &index, best_effort)?
            .into_iter()
            .collect();
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x26, "3DSOLID", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_3dsolid_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
            {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        let layer_handle = entity.layer_handle;
        let mut acis_handles = entity.acis_handles;
        acis_handles.retain(|handle| {
            *handle != layer_handle
                && known_handles.contains(handle)
                && !known_layer_handles.contains(handle)
        });
        result.push((entity.handle, acis_handles));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_body_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<BodyEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_layer_handles: HashSet<u64> =
        collect_known_layer_handles_in_order(&decoder, &dynamic_types, &index, best_effort)?
            .into_iter()
            .collect();
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x27, "BODY", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_body_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        let layer_handle = entity.layer_handle;
        let mut acis_handles = entity.acis_handles;
        acis_handles.retain(|handle| {
            *handle != layer_handle
                && known_handles.contains(handle)
                && !known_layer_handles.contains(handle)
        });
        result.push((entity.handle, acis_handles));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_ray_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<RayEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x28, "RAY", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_ray_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((entity.handle, entity.start, entity.unit_vector));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_xline_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<XLineEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x29, "XLINE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_xline_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((entity.handle, entity.start, entity.unit_vector));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

fn decode_line_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::LineEntity> {
    let start = reader.get_pos();
    let primary = match version {
        version::DwgVersion::R14 => entities::decode_line_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_line_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_line_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_line_r2007(reader),
        _ => entities::decode_line(reader),
    };
    if let Ok(entity) = primary {
        return Ok(entity);
    }
    let primary_err = primary.unwrap_err();

    reader.set_pos(start.0, start.1);
    if let Ok(entity) = entities::decode_line(reader) {
        return Ok(entity);
    }

    reader.set_pos(start.0, start.1);
    if let Ok(entity) = entities::decode_line_r14(reader, object_handle) {
        return Ok(entity);
    }

    Err(primary_err)
}

fn decode_point_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::PointEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_point_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_point_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_point_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_point_r2007(reader),
        _ => entities::decode_point(reader),
    }
}

fn decode_arc_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::ArcEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_arc_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_arc_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_arc_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_arc_r2007(reader),
        _ => entities::decode_arc(reader),
    }
}

fn decode_circle_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::CircleEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_circle_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_circle_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_circle_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_circle_r2007(reader),
        _ => entities::decode_circle(reader),
    }
}

fn decode_ellipse_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::EllipseEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_ellipse_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ellipse_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ellipse_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_ellipse_r2007(reader),
        _ => entities::decode_ellipse(reader),
    }
}

fn decode_spline_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::SplineEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_spline_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_spline_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_spline_r2007(reader),
        _ => entities::decode_spline(reader),
    }
}

fn decode_text_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::TextEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_text_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_text_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_text_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_text_r2007(reader),
        _ => entities::decode_text(reader),
    }
}

fn decode_attrib_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::AttribEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_attrib_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_attrib_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_attrib_r2007(reader),
        _ => entities::decode_attrib(reader),
    }
}

fn decode_attdef_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::AttribEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_attdef_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_attdef_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_attdef_r2007(reader),
        _ => entities::decode_attdef(reader),
    }
}

fn decode_mtext_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::MTextEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_mtext_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_mtext_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_mtext_r2007(reader),
        version::DwgVersion::R2004 => entities::decode_mtext_r2004(reader),
        _ => entities::decode_mtext(reader),
    }
}

fn decode_leader_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::LeaderEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_leader_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_leader_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_leader_r2007(reader),
        _ => entities::decode_leader(reader),
    }
}

fn decode_hatch_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::HatchEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_hatch_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_hatch_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_hatch_r2007(reader),
        version::DwgVersion::R2004 => entities::decode_hatch_r2004(reader),
        _ => entities::decode_hatch(reader),
    }
}

fn decode_tolerance_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::ToleranceEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_tolerance_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_tolerance_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_tolerance_r2007(reader),
        _ => entities::decode_tolerance(reader),
    }
}

fn decode_mline_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::MLineEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_mline_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_mline_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_mline_r2007(reader),
        _ => entities::decode_mline(reader),
    }
}

fn decode_3dface_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Face3dEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_3dface_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_3dface_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_3dface_r2007(reader),
        _ => entities::decode_3dface(reader),
    }
}

fn decode_solid_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::SolidEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_solid_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_solid_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_solid_r2007(reader),
        _ => entities::decode_solid(reader),
    }
}

fn decode_trace_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::TraceEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_trace_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_trace_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_trace_r2007(reader),
        _ => entities::decode_trace(reader),
    }
}

fn decode_shape_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::ShapeEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_shape_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_shape_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_shape_r2007(reader),
        _ => entities::decode_shape(reader),
    }
}

fn decode_viewport_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::ViewportEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_viewport_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_viewport_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_viewport_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_viewport_r2007(reader),
        _ => entities::decode_viewport(reader),
    }
}

fn decode_oleframe_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::OleFrameEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_oleframe_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_oleframe_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_oleframe_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_oleframe_r2007(reader),
        _ => entities::decode_oleframe(reader),
    }
}

fn decode_ole2frame_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::OleFrameEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_ole2frame_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ole2frame_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ole2frame_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_ole2frame_r2007(reader),
        _ => entities::decode_ole2frame(reader),
    }
}

fn decode_long_transaction_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::LongTransactionEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_long_transaction_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_long_transaction_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_long_transaction_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_long_transaction_r2007(reader),
        _ => entities::decode_long_transaction(reader),
    }
}

fn decode_region_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::RegionEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_region_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_region_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_region_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_region_r2007(reader),
        _ => entities::decode_region(reader),
    }
}

fn decode_3dsolid_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Solid3dEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_3dsolid_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_3dsolid_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_3dsolid_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_3dsolid_r2007(reader),
        _ => entities::decode_3dsolid(reader),
    }
}

fn decode_body_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::BodyEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_body_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_body_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_body_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_body_r2007(reader),
        _ => entities::decode_body(reader),
    }
}

fn decode_ray_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::RayEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_ray_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ray_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ray_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_ray_r2007(reader),
        _ => entities::decode_ray(reader),
    }
}

fn decode_xline_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::XLineEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_xline_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_xline_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_xline_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_xline_r2007(reader),
        _ => entities::decode_xline(reader),
    }
}

fn read_handle_reference_chained(
    reader: &mut BitReader<'_>,
    prev_handle: &mut u64,
) -> crate::core::result::Result<u64> {
    let handle = reader.read_h()?;
    let absolute = match handle.code {
        0x06 => prev_handle.saturating_add(1),
        0x08 => prev_handle.saturating_sub(1),
        0x0A => prev_handle.saturating_add(handle.value),
        0x0C => prev_handle.saturating_sub(handle.value),
        0x02..=0x05 => handle.value,
        _ => handle.value,
    };
    *prev_handle = absolute;
    Ok(absolute)
}

fn read_tu(reader: &mut BitReader<'_>) -> crate::core::result::Result<String> {
    reader.read_tu()
}
