// Standard version-dispatch helper shared by most entity decoders.
// - `with_r14`: entity has a dedicated R14 decoder taking `(reader, object_handle)`
// - `no_r14`: entity skips R14 and falls through to the pre-R2010 default decoder
//
// R2010 / R2013 / R2018 resolve the object-data end bit via
// `resolve_r2010_object_data_end_bit` and pass it to the version-specific decoder.
// R2007 and the default branch take only the reader (no end-bit / handle params).
macro_rules! impl_version_dispatch {
    (
        with_r14;
        fn $fn_name:ident -> $entity_ty:ty;
        r14: $r14_fn:path;
        r2010: $r2010_fn:path;
        r2013: $r2013_fn:path;
        r2007: $r2007_fn:path;
        default: $default_fn:path $(;)?
    ) => {
        fn $fn_name(
            reader: &mut BitReader<'_>,
            version: &version::DwgVersion,
            header: &ApiObjectHeader,
            object_handle: u64,
        ) -> crate::core::result::Result<$entity_ty> {
            match version {
                version::DwgVersion::R14 => $r14_fn(reader, object_handle),
                version::DwgVersion::R2010 => {
                    let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
                    $r2010_fn(reader, object_data_end_bit, object_handle)
                }
                version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
                    let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
                    $r2013_fn(reader, object_data_end_bit, object_handle)
                }
                version::DwgVersion::R2007 => $r2007_fn(reader),
                _ => $default_fn(reader),
            }
        }
    };
    (
        no_r14;
        fn $fn_name:ident -> $entity_ty:ty;
        r2010: $r2010_fn:path;
        r2013: $r2013_fn:path;
        r2007: $r2007_fn:path;
        default: $default_fn:path $(;)?
    ) => {
        fn $fn_name(
            reader: &mut BitReader<'_>,
            version: &version::DwgVersion,
            header: &ApiObjectHeader,
            object_handle: u64,
        ) -> crate::core::result::Result<$entity_ty> {
            match version {
                version::DwgVersion::R2010 => {
                    let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
                    $r2010_fn(reader, object_data_end_bit, object_handle)
                }
                version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
                    let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
                    $r2013_fn(reader, object_data_end_bit, object_handle)
                }
                version::DwgVersion::R2007 => $r2007_fn(reader),
                _ => $default_fn(reader),
            }
        }
    };
}

// Standard entity collection loop used by most `decode_*_entities` /
// `decode_*_owner_handles` PyFunction wrappers.
//
// Walks the object index, filters by type_code/type_name, skips the
// object-type prefix, calls the version dispatch fn, and pushes a caller-built
// row. Best-effort mode swallows per-object errors and continues.
fn collect_entity_rows<E, R, F>(
    path: &str,
    limit: Option<usize>,
    type_code: u16,
    type_name: &'static str,
    decode_for_version: fn(
        &mut BitReader<'_>,
        &version::DwgVersion,
        &ApiObjectHeader,
        u64,
    ) -> crate::core::result::Result<E>,
    mut build_row: F,
) -> PyResult<Vec<R>>
where
    F: FnMut(E) -> R,
{
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
        if !matches_type_name(header.type_code, type_code, type_name, &dynamic_types) {
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
            match decode_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(_) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push(build_row(entity));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

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
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let dynamic_type_classes = load_dynamic_type_classes(&decoder, best_effort)?;
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
        let type_class =
            resolved_type_class(header.type_code, &type_name, &dynamic_type_classes);
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
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let dynamic_type_classes = load_dynamic_type_classes(&decoder, best_effort)?;
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
        let type_class =
            resolved_type_class(header.type_code, &type_name, &dynamic_type_classes);
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
    let mut object_offsets: HashMap<u64, u32> = HashMap::new();
    for obj in index.objects.iter() {
        object_offsets
            .entry(obj.handle.0)
            .and_modify(|offset| {
                if obj.offset > *offset {
                    *offset = obj.offset;
                }
            })
            .or_insert(obj.offset);
    }

    let mut found_rows: HashMap<u64, ObjectRecordBytesRow> = HashMap::new();
    for handle in handles.iter().copied() {
        if !target_handles.contains(&handle) {
            continue;
        }
        let Some(offset) = object_offsets.get(&handle).copied() else {
            continue;
        };
        let record = decoder.parse_object_record(offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        found_rows.insert(
            handle,
            (
                handle,
                offset,
                header.data_size,
                header.type_code,
                record.raw.as_ref().to_vec(),
            ),
        );
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

#[pyfunction(signature = (path, offsets, limit=None))]
pub fn read_object_records_by_offset(
    path: &str,
    offsets: Vec<u32>,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectRecordBytesRow>> {
    if offsets.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let mut found_rows: HashMap<u32, ObjectRecordBytesRow> = HashMap::new();

    for offset in offsets.iter().copied() {
        let record = decoder.parse_object_record(offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        found_rows.insert(
            offset,
            (
                0,
                offset,
                header.data_size,
                header.type_code,
                record.raw.as_ref().to_vec(),
            ),
        );
    }

    let mut result = Vec::new();
    for offset in offsets {
        if let Some(row) = found_rows.remove(&offset) {
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

#[pyfunction(signature = (path, handles, limit=None))]
pub fn decode_object_entity_layer_handles(
    path: &str,
    handles: Vec<u64>,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectLayerHandleRow>> {
    if handles.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_layer_handles: HashSet<u64> =
        collect_known_layer_handles_in_order(&decoder, &dynamic_types, &index, best_effort)?
            .into_iter()
            .collect();
    let target_handles: HashSet<u64> = handles.iter().copied().collect();
    let mut object_offsets: HashMap<u64, u32> = HashMap::new();
    for obj in index.objects.iter() {
        if !target_handles.contains(&obj.handle.0) {
            continue;
        }
        object_offsets
            .entry(obj.handle.0)
            .and_modify(|offset| {
                if obj.offset > *offset {
                    *offset = obj.offset;
                }
            })
            .or_insert(obj.offset);
    }

    let mut found_rows: HashMap<u64, ObjectLayerHandleRow> = HashMap::new();
    for handle in handles.iter().copied() {
        let Some(offset) = object_offsets.get(&handle).copied() else {
            continue;
        };
        let Some((record, header)) = parse_record_and_header(&decoder, offset, best_effort)? else {
            continue;
        };
        let Some(layer_handle) = decode_object_entity_layer_handle_from_record(
            &record,
            decoder.version(),
            &header,
            handle,
            &known_layer_handles,
        ) else {
            continue;
        };
        found_rows.insert(handle, (handle, layer_handle));
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

fn decode_object_entity_layer_handle_from_record(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
    known_layer_handles: &HashSet<u64>,
) -> Option<u64> {
    let default_layer = known_layer_handles.iter().copied().min();
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return recover_object_entity_layer_handle_without_common_header(
            record,
            version,
            header,
            object_handle,
            known_layer_handles,
            default_layer,
        );
    }

    let parsed_layer_handle = match version {
        version::DwgVersion::R14 => {
            let common = entities::common::parse_common_entity_header_r14(&mut reader).ok()?;
            reader.set_bit_pos(common.obj_size);
            entities::common::parse_common_entity_layer_handle(&mut reader, &common).ok()?
        }
        version::DwgVersion::R2000
        | version::DwgVersion::R2004
        | version::DwgVersion::R2007 => {
            let common = entities::common::parse_common_entity_header_r2007(&mut reader).ok()?;
            reader.set_bit_pos(common.obj_size);
            entities::common::parse_common_entity_layer_handle(&mut reader, &common).ok()?
        }
        version::DwgVersion::R2010 => {
            let Some(common) = parse_dim_common_header_r2010_plus_with_candidates(
                &mut reader,
                header,
                |candidate_reader, end_bit| {
                    entities::common::parse_common_entity_header_r2010(
                        candidate_reader,
                        end_bit,
                    )
                },
            ) else {
                return recover_object_entity_layer_handle_without_common_header(
                    record,
                    version,
                    header,
                    object_handle,
                    known_layer_handles,
                    default_layer,
                );
            };
            reader.set_bit_pos(common.obj_size);
            let Some(parsed) =
                entities::common::parse_common_entity_layer_handle(&mut reader, &common).ok()
            else {
                return recover_object_entity_layer_handle_without_common_header(
                    record,
                    version,
                    header,
                    object_handle,
                    known_layer_handles,
                    default_layer,
                );
            };
            if known_layer_handles.contains(&parsed) {
                return Some(parsed);
            }
            let recovered = recover_entity_layer_handle_r2010_plus(
                record,
                version,
                header,
                object_handle,
                parsed,
                known_layer_handles,
            );
            return accept_recovered_object_entity_layer_handle(
                recovered,
                known_layer_handles,
                default_layer,
            );
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let Some(common) = parse_dim_common_header_r2010_plus_with_candidates(
                &mut reader,
                header,
                |candidate_reader, end_bit| {
                    entities::common::parse_common_entity_header_r2013(
                        candidate_reader,
                        end_bit,
                    )
                },
            ) else {
                return recover_object_entity_layer_handle_without_common_header(
                    record,
                    version,
                    header,
                    object_handle,
                    known_layer_handles,
                    default_layer,
                );
            };
            reader.set_bit_pos(common.obj_size);
            let Some(parsed) =
                entities::common::parse_common_entity_layer_handle(&mut reader, &common).ok()
            else {
                return recover_object_entity_layer_handle_without_common_header(
                    record,
                    version,
                    header,
                    object_handle,
                    known_layer_handles,
                    default_layer,
                );
            };
            if known_layer_handles.contains(&parsed) {
                return Some(parsed);
            }
            let recovered = recover_entity_layer_handle_r2010_plus(
                record,
                version,
                header,
                object_handle,
                parsed,
                known_layer_handles,
            );
            return accept_recovered_object_entity_layer_handle(
                recovered,
                known_layer_handles,
                default_layer,
            );
        }
        _ => return None,
    };

    if known_layer_handles.contains(&parsed_layer_handle) {
        Some(parsed_layer_handle)
    } else {
        None
    }
}

fn recover_object_entity_layer_handle_without_common_header(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
    known_layer_handles: &HashSet<u64>,
    default_layer: Option<u64>,
) -> Option<u64> {
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return None;
    }

    let recovered = recover_entity_layer_handle_r2010_plus(
        record,
        version,
        header,
        object_handle,
        0,
        known_layer_handles,
    );
    accept_recovered_object_entity_layer_handle(
        recovered,
        known_layer_handles,
        default_layer,
    )
}

fn accept_recovered_object_entity_layer_handle(
    recovered: u64,
    known_layer_handles: &HashSet<u64>,
    default_layer: Option<u64>,
) -> Option<u64> {
    if known_layer_handles.contains(&recovered) && Some(recovered) != default_layer {
        Some(recovered)
    } else {
        None
    }
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

fn is_r2010_plus_version(version: &version::DwgVersion) -> bool {
    matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    )
}

fn is_block_header_handle(
    handle: Option<u64>,
    object_type_codes: &HashMap<u64, u16>,
) -> bool {
    handle
        .and_then(|value| object_type_codes.get(&value).copied())
        .is_some_and(|type_code| type_code == 0x31)
}

fn is_text_style_handle(
    handle: Option<u64>,
    object_type_codes: &HashMap<u64, u16>,
) -> bool {
    handle
        .and_then(|value| object_type_codes.get(&value).copied())
        .is_some_and(|type_code| type_code == 0x35)
}

fn recover_textish_owner_and_style_handles(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
    parsed_owner_handle: Option<u64>,
    parsed_style_handle: Option<u64>,
    known_handles: &HashSet<u64>,
    object_type_codes: &HashMap<u64, u16>,
) -> (Option<u64>, Option<u64>) {
    if !is_r2010_plus_version(version) {
        return (parsed_owner_handle, parsed_style_handle);
    }

    let owner_is_plausible = is_block_header_handle(parsed_owner_handle, object_type_codes);
    let style_is_plausible = parsed_style_handle.is_none()
        || is_text_style_handle(parsed_style_handle, object_type_codes);
    if owner_is_plausible && style_is_plausible {
        return (parsed_owner_handle, parsed_style_handle);
    }

    let decoded = decode_known_handle_refs_from_object_record(
        record,
        version,
        header,
        object_handle,
        known_handles,
        Some(object_type_codes),
        8,
    );

    let mut recovered_owner_handle = if owner_is_plausible {
        parsed_owner_handle
    } else {
        None
    };
    let mut recovered_style_handle = if style_is_plausible {
        parsed_style_handle
    } else {
        None
    };

    for candidate in decoded.refs {
        let Some(type_code) = object_type_codes.get(&candidate).copied() else {
            continue;
        };
        if recovered_owner_handle.is_none() && type_code == 0x31 {
            recovered_owner_handle = Some(candidate);
            continue;
        }
        if recovered_style_handle.is_none() && type_code == 0x35 {
            recovered_style_handle = Some(candidate);
        }
    }

    (recovered_owner_handle, recovered_style_handle)
}

fn extract_proxy_graphics_from_object_record(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
) -> Option<Vec<u8>> {
    if !is_r2010_plus_version(version) {
        return None;
    }

    let mut end_bits = resolve_r2010_object_data_end_bit_candidates(header);
    if let Ok(primary) = resolve_r2010_object_data_end_bit(header) {
        end_bits.retain(|candidate| *candidate != primary);
        end_bits.insert(0, primary);
    }

    for object_data_end_bit in end_bits {
        let mut reader = record.bit_reader();
        if skip_object_type_prefix(&mut reader, version).is_err() {
            return None;
        }
        let decoded = match version {
            version::DwgVersion::R2010 => {
                entities::common::parse_common_entity_header_with_proxy_graphics_r2010(
                    &mut reader,
                    object_data_end_bit,
                )
            }
            version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
                entities::common::parse_common_entity_header_with_proxy_graphics_r2013(
                    &mut reader,
                    object_data_end_bit,
                )
            }
            _ => return None,
        };
        let Ok((_header, graphics)) = decoded else {
            continue;
        };
        let Some(graphics) = graphics else {
            continue;
        };
        if !graphics.is_empty() {
            return Some(graphics);
        }
    }

    None
}

fn parse_proxy_graphic_text_chunks(
    data: &[u8],
    codepage: Option<u16>,
) -> Vec<ProxyGraphicTextCandidate> {
    let mut reader = crate::io::ByteReader::new(data);
    let mut out = Vec::new();

    while reader.remaining() >= 8 {
        let Ok(size_raw) = reader.read_i32_le() else {
            break;
        };
        let Ok(chunk_type_raw) = reader.read_i32_le() else {
            break;
        };
        if size_raw <= 0 || chunk_type_raw < 0 {
            break;
        }
        let Ok(size) = usize::try_from(size_raw) else {
            break;
        };
        let Ok(chunk_type) = u32::try_from(chunk_type_raw) else {
            break;
        };
        if size > reader.remaining() {
            break;
        }
        let Ok(chunk) = reader.read_bytes(size) else {
            break;
        };
        let parsed = match chunk_type {
            10 => parse_proxy_graphic_text_chunk(chunk, codepage, false),
            11 => parse_proxy_graphic_text2_chunk(chunk, codepage, false),
            36 => parse_proxy_graphic_text_chunk(chunk, codepage, true),
            38 => parse_proxy_graphic_text2_chunk(chunk, codepage, true),
            _ => None,
        };
        if let Some(candidate) = parsed.filter(|candidate| !candidate.text.trim().is_empty()) {
            out.push(candidate);
        }
    }

    out
}

fn parse_proxy_graphic_text_chunk(
    chunk: &[u8],
    codepage: Option<u16>,
    unicode: bool,
) -> Option<ProxyGraphicTextCandidate> {
    let mut reader = crate::io::ByteReader::new(chunk);
    let insertion = read_proxy_graphic_point3(&mut reader).ok()?;
    let _normal = read_proxy_graphic_point3(&mut reader).ok()?;
    let text_direction = read_proxy_graphic_point3(&mut reader).ok()?;
    let height = reader.read_f64_le().ok()?;
    let width_factor = reader.read_f64_le().ok()?;
    let oblique_angle = reader.read_f64_le().ok()?;
    let text = if unicode {
        read_proxy_graphic_padded_unicode_string(&mut reader).ok()?
    } else {
        read_proxy_graphic_padded_codepage_string(&mut reader, codepage).ok()?
    };
    Some(ProxyGraphicTextCandidate {
        text,
        insertion,
        text_direction,
        height,
        width_factor,
        oblique_angle,
    })
}

fn parse_proxy_graphic_text2_chunk(
    chunk: &[u8],
    codepage: Option<u16>,
    unicode: bool,
) -> Option<ProxyGraphicTextCandidate> {
    let mut reader = crate::io::ByteReader::new(chunk);
    let insertion = read_proxy_graphic_point3(&mut reader).ok()?;
    let _normal = read_proxy_graphic_point3(&mut reader).ok()?;
    let text_direction = read_proxy_graphic_point3(&mut reader).ok()?;
    let text = if unicode {
        read_proxy_graphic_padded_unicode_string(&mut reader).ok()?
    } else {
        read_proxy_graphic_padded_codepage_string(&mut reader, codepage).ok()?
    };
    let _length = reader.read_i32_le().ok()?;
    let _raw = reader.read_i32_le().ok()?;
    let height = reader.read_f64_le().ok()?;
    let width_factor = reader.read_f64_le().ok()?;
    let oblique_angle = reader.read_f64_le().ok()?;
    Some(ProxyGraphicTextCandidate {
        text,
        insertion,
        text_direction,
        height,
        width_factor,
        oblique_angle,
    })
}

fn read_proxy_graphic_point3(reader: &mut crate::io::ByteReader<'_>) -> crate::core::result::Result<Point3> {
    Ok((
        reader.read_f64_le()?,
        reader.read_f64_le()?,
        reader.read_f64_le()?,
    ))
}

fn read_proxy_graphic_padded_codepage_string(
    reader: &mut crate::io::ByteReader<'_>,
    codepage: Option<u16>,
) -> crate::core::result::Result<String> {
    let start = reader.tell() as usize;
    let mut bytes = Vec::new();
    loop {
        let byte = reader.read_u8()?;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    align_proxy_graphic_reader_to_4(reader, start)?;
    Ok(crate::bit::bit_reader::decode_tv_bytes(&bytes, codepage))
}

fn read_proxy_graphic_padded_unicode_string(
    reader: &mut crate::io::ByteReader<'_>,
) -> crate::core::result::Result<String> {
    let start = reader.tell() as usize;
    let mut words = Vec::new();
    loop {
        let value = reader.read_u16_le()?;
        if value == 0 {
            break;
        }
        words.push(value);
    }
    align_proxy_graphic_reader_to_4(reader, start)?;
    Ok(String::from_utf16_lossy(&words))
}

fn align_proxy_graphic_reader_to_4(
    reader: &mut crate::io::ByteReader<'_>,
    start: usize,
) -> crate::core::result::Result<()> {
    let consumed = (reader.tell() as usize).saturating_sub(start);
    let padding = (4usize.wrapping_sub(consumed % 4)) % 4;
    if padding > 0 {
        reader.skip(padding)?;
    }
    Ok(())
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
    let mut object_offsets: HashMap<u64, u32> = HashMap::new();
    for obj in index.objects.iter() {
        object_offsets
            .entry(obj.handle.0)
            .and_modify(|offset| {
                if obj.offset > *offset {
                    *offset = obj.offset;
                }
            })
            .or_insert(obj.offset);
    }

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
    let mut object_offsets: HashMap<u64, u32> = HashMap::new();
    for obj in index.objects.iter() {
        object_offsets
            .entry(obj.handle.0)
            .and_modify(|offset| {
                if obj.offset > *offset {
                    *offset = obj.offset;
                }
            })
            .or_insert(obj.offset);
    }

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

#[derive(Debug, Clone)]
struct ProxyGraphicTextCandidate {
    text: String,
    insertion: Point3,
    text_direction: Point3,
    height: f64,
    width_factor: f64,
    oblique_angle: f64,
}

fn parse_proxy_graphic_chunk_infos(data: &[u8]) -> Vec<(u32, u32)> {
    let mut reader = crate::io::ByteReader::new(data);
    let mut out = Vec::new();

    while reader.remaining() >= 8 {
        let Ok(size_raw) = reader.read_i32_le() else {
            break;
        };
        let Ok(chunk_type_raw) = reader.read_i32_le() else {
            break;
        };
        if size_raw <= 0 || chunk_type_raw < 0 {
            break;
        }
        let Ok(size) = usize::try_from(size_raw) else {
            break;
        };
        let Ok(chunk_type) = u32::try_from(chunk_type_raw) else {
            break;
        };
        if size > reader.remaining() {
            break;
        }
        out.push((chunk_type, size_raw as u32));
        if reader.skip(size).is_err() {
            break;
        }
    }

    out
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_proxy_graphic_chunk_infos(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ProxyGraphicChunkInfoRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let dynamic_type_classes = load_dynamic_type_classes(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();

    'objects: for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if header.type_code != 0x1F2 && header.type_code < 500 {
            continue;
        }
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        let type_class =
            resolved_type_class(header.type_code, &type_name, &dynamic_type_classes);
        if type_class == "O" {
            continue;
        }
        if header.type_code != 0x1F2 && !type_name.starts_with("UNKNOWN(") {
            continue;
        }
        let Some(graphics) =
            extract_proxy_graphics_from_object_record(&record, decoder.version(), &header)
        else {
            continue;
        };
        for (chunk_index, (chunk_type, chunk_size)) in
            parse_proxy_graphic_chunk_infos(&graphics).into_iter().enumerate()
        {
            result.push((
                obj.handle.0,
                header.type_code,
                chunk_index as u32,
                chunk_type,
                chunk_size,
            ));
            if let Some(limit) = limit {
                if result.len() >= limit {
                    break 'objects;
                }
            }
        }
    }

    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_proxy_graphic_text_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ProxyGraphicTextRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let dynamic_type_classes = load_dynamic_type_classes(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();

    'objects: for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if header.type_code != 0x1F2 && header.type_code < 500 {
            continue;
        }
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        let type_class =
            resolved_type_class(header.type_code, &type_name, &dynamic_type_classes);
        if type_class == "O" {
            continue;
        }
        if header.type_code != 0x1F2 && !type_name.starts_with("UNKNOWN(") {
            continue;
        }
        let Some(graphics) =
            extract_proxy_graphics_from_object_record(&record, decoder.version(), &header)
        else {
            continue;
        };
        let candidates = parse_proxy_graphic_text_chunks(&graphics, decoder.codepage());
        for (chunk_index, candidate) in candidates.into_iter().enumerate() {
            result.push((
                obj.handle.0,
                header.type_code,
                chunk_index as u32,
                candidate.text,
                candidate.insertion,
                candidate.text_direction,
                candidate.height,
                candidate.width_factor,
                candidate.oblique_angle,
            ));
            if let Some(limit) = limit {
                if result.len() >= limit {
                    break 'objects;
                }
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
pub fn decode_line_owner_handles(path: &str, limit: Option<usize>) -> PyResult<Vec<InsertOwnerRow>> {
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
        result.push((entity.handle, entity.owner_handle));
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
pub fn decode_point_owner_handles(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<InsertOwnerRow>> {
    collect_entity_rows(
        path,
        limit,
        0x1B,
        "POINT",
        decode_point_for_version,
        |entity| (entity.handle, entity.owner_handle),
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_3dface_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<Face3dEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x1C,
        "3DFACE",
        decode_3dface_for_version,
        |entity| {
            (
                entity.handle,
                entity.p1,
                entity.p2,
                entity.p3,
                entity.p4,
                entity.invisible_edge_flags,
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_arc_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<ArcEntityRow>> {
    collect_entity_rows(path, limit, 0x11, "ARC", decode_arc_for_version, |entity| {
        (
            entity.handle,
            entity.center.0,
            entity.center.1,
            entity.center.2,
            entity.radius,
            entity.angle_start,
            entity.angle_end,
        )
    })
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_arc_owner_handles(path: &str, limit: Option<usize>) -> PyResult<Vec<InsertOwnerRow>> {
    collect_entity_rows(path, limit, 0x11, "ARC", decode_arc_for_version, |entity| {
        (entity.handle, entity.owner_handle)
    })
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_circle_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<CircleEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x12,
        "CIRCLE",
        decode_circle_for_version,
        |entity| {
            (
                entity.handle,
                entity.center.0,
                entity.center.1,
                entity.center.2,
                entity.radius,
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_circle_owner_handles(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<InsertOwnerRow>> {
    collect_entity_rows(
        path,
        limit,
        0x12,
        "CIRCLE",
        decode_circle_for_version,
        |entity| (entity.handle, entity.owner_handle),
    )
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
    collect_entity_rows(
        path,
        limit,
        0x23,
        "ELLIPSE",
        decode_ellipse_for_version,
        |entity| {
            (
                entity.handle,
                entity.center,
                entity.major_axis,
                entity.extrusion,
                entity.axis_ratio,
                entity.start_angle,
                entity.end_angle,
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_spline_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<SplineEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x24,
        "SPLINE",
        decode_spline_for_version,
        |entity| {
            (
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
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_text_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<TextEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_handles: HashSet<u64> = if is_r2010_plus_version(decoder.version()) {
        index.objects.iter().map(|obj| obj.handle.0).collect()
    } else {
        HashSet::new()
    };
    let object_type_codes = if is_r2010_plus_version(decoder.version()) {
        collect_object_type_codes(&decoder, &index, best_effort)?
    } else {
        HashMap::new()
    };
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
        let mut entity =
            match decode_text_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        if is_r2010_plus_version(decoder.version()) {
            let (owner_handle, style_handle) = recover_textish_owner_and_style_handles(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.owner_handle,
                entity.style_handle,
                &known_handles,
                &object_type_codes,
            );
            entity.owner_handle = owner_handle;
            entity.style_handle = style_handle;
        }
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
            entity.owner_handle,
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
    let debug_mtext = std::env::var("EZDWG_DEBUG_MTEXT")
        .ok()
        .is_some_and(|value| value != "0");
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_handles: HashSet<u64> = if is_r2010_plus_version(decoder.version()) {
        index.objects.iter().map(|obj| obj.handle.0).collect()
    } else {
        HashSet::new()
    };
    let object_type_codes = if is_r2010_plus_version(decoder.version()) {
        collect_object_type_codes(&decoder, &index, best_effort)?
    } else {
        HashMap::new()
    };
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
                if debug_mtext {
                    eprintln!(
                        "[mtext-decode] handle={} stage=prefix err={:?}",
                        obj.handle.0,
                        err
                    );
                }
                continue;
            }
            return Err(to_py_err(err));
        }
        let reader_after_prefix = reader.clone();
        let mut entity =
            match decode_mtext_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => {
                    if debug_mtext {
                        eprintln!(
                            "[mtext-decode] handle={} offset={} size={} err={:?}",
                            obj.handle.0,
                            obj.offset,
                            header.data_size,
                            err
                        );
                    }
                    continue;
                }
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
            let (owner_handle, _style_handle) = recover_textish_owner_and_style_handles(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.owner_handle,
                None,
                &known_handles,
                &object_type_codes,
            );
            entity.owner_handle = owner_handle;
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
            entity.owner_handle,
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
    collect_entity_rows(
        path,
        limit,
        0x2D,
        "LEADER",
        decode_leader_for_version,
        |entity| {
            (
                entity.handle,
                entity.annotation_type,
                entity.path_type,
                entity.points,
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_hatch_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<HatchEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let debug_hatch = std::env::var("EZDWG_DEBUG_HATCH")
        .ok()
        .is_some_and(|value| value != "0");
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
                if debug_hatch {
                    eprintln!(
                        "[hatch-decode] handle={} stage=prefix err={:?}",
                        obj.handle.0,
                        err
                    );
                }
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_hatch_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => {
                    if debug_hatch {
                        eprintln!(
                            "[hatch-decode] handle={} offset={} size={} err={:?}",
                            obj.handle.0,
                            obj.offset,
                            header.data_size,
                            err
                        );
                    }
                    continue;
                }
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
    collect_entity_rows(
        path,
        limit,
        0x2E,
        "TOLERANCE",
        decode_tolerance_for_version,
        |entity| {
            (
                entity.handle,
                entity.text,
                entity.insertion,
                entity.x_direction,
                entity.extrusion,
                entity.height,
                entity.dimgap,
                entity.dimstyle_handle,
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_mline_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<MLineEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x2F,
        "MLINE",
        decode_mline_for_version,
        |entity| {
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
            (
                entity.handle,
                entity.scale,
                entity.justification,
                entity.base_point,
                entity.extrusion,
                entity.open_closed,
                entity.lines_in_style,
                vertices,
                entity.mlinestyle_handle,
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_solid_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<SolidEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x1F,
        "SOLID",
        decode_solid_for_version,
        |entity| {
            (
                entity.handle,
                entity.p1,
                entity.p2,
                entity.p3,
                entity.p4,
                entity.thickness,
                entity.extrusion,
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_trace_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<TraceEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x20,
        "TRACE",
        decode_trace_for_version,
        |entity| {
            (
                entity.handle,
                entity.p1,
                entity.p2,
                entity.p3,
                entity.p4,
                entity.thickness,
                entity.extrusion,
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_shape_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<ShapeEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x21,
        "SHAPE",
        decode_shape_for_version,
        |entity| {
            (
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
            )
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_viewport_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ViewportEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x22,
        "VIEWPORT",
        decode_viewport_for_version,
        |entity| (entity.handle,),
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_oleframe_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<OleFrameEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x2B,
        "OLEFRAME",
        decode_oleframe_for_version,
        |entity| (entity.handle,),
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_ole2frame_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<OleFrameEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x4A,
        "OLE2FRAME",
        decode_ole2frame_for_version,
        |entity| (entity.handle,),
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_long_transaction_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<LongTransactionEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x4C,
        "LONG_TRANSACTION",
        decode_long_transaction_for_version,
        |entity| {
            (
                entity.handle,
                entity.owner_handle,
                entity.reactor_handles,
                entity.xdic_obj_handle,
                entity.ltype_handle,
                entity.plotstyle_handle,
                entity.material_handle,
                entity.extra_handles,
            )
        },
    )
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
    collect_entity_rows(path, limit, 0x28, "RAY", decode_ray_for_version, |entity| {
        (entity.handle, entity.start, entity.unit_vector)
    })
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_xline_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<XLineEntityRow>> {
    collect_entity_rows(
        path,
        limit,
        0x29,
        "XLINE",
        decode_xline_for_version,
        |entity| (entity.handle, entity.start, entity.unit_vector),
    )
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

impl_version_dispatch! {
    with_r14;
    fn decode_point_for_version -> entities::PointEntity;
    r14: entities::decode_point_r14;
    r2010: entities::decode_point_r2010;
    r2013: entities::decode_point_r2013;
    r2007: entities::decode_point_r2007;
    default: entities::decode_point;
}

impl_version_dispatch! {
    with_r14;
    fn decode_arc_for_version -> entities::ArcEntity;
    r14: entities::decode_arc_r14;
    r2010: entities::decode_arc_r2010;
    r2013: entities::decode_arc_r2013;
    r2007: entities::decode_arc_r2007;
    default: entities::decode_arc;
}

impl_version_dispatch! {
    with_r14;
    fn decode_circle_for_version -> entities::CircleEntity;
    r14: entities::decode_circle_r14;
    r2010: entities::decode_circle_r2010;
    r2013: entities::decode_circle_r2013;
    r2007: entities::decode_circle_r2007;
    default: entities::decode_circle;
}

impl_version_dispatch! {
    with_r14;
    fn decode_ellipse_for_version -> entities::EllipseEntity;
    r14: entities::decode_ellipse_r14;
    r2010: entities::decode_ellipse_r2010;
    r2013: entities::decode_ellipse_r2013;
    r2007: entities::decode_ellipse_r2007;
    default: entities::decode_ellipse;
}

impl_version_dispatch! {
    no_r14;
    fn decode_spline_for_version -> entities::SplineEntity;
    r2010: entities::decode_spline_r2010;
    r2013: entities::decode_spline_r2013;
    r2007: entities::decode_spline_r2007;
    default: entities::decode_spline;
}

fn decode_text_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::TextEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_text_r14(reader, object_handle),
        version::DwgVersion::R2010 => decode_r2010_entity_with_end_bit_candidates(
            reader,
            header,
            |attempt_reader, object_data_end_bit| {
                entities::decode_text_r2010(attempt_reader, object_data_end_bit, object_handle)
            },
        ),
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            decode_r2010_entity_with_end_bit_candidates(
                reader,
                header,
                |attempt_reader, object_data_end_bit| {
                    entities::decode_text_r2013(attempt_reader, object_data_end_bit, object_handle)
                },
            )
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
        version::DwgVersion::R2010 => decode_r2010_entity_with_start_and_end_bit_candidates_scored(
            reader,
            header,
            |attempt_reader, object_data_end_bit| {
                entities::decode_attrib_r2010(attempt_reader, object_data_end_bit, object_handle)
            },
            score_attrib_entity_candidate,
        ),
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            decode_r2010_entity_with_start_and_end_bit_candidates_scored(
                reader,
                header,
                |attempt_reader, object_data_end_bit| {
                    entities::decode_attrib_r2013(
                        attempt_reader,
                        object_data_end_bit,
                        object_handle,
                    )
                },
                score_attrib_entity_candidate,
            )
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
        version::DwgVersion::R2010 => decode_r2010_entity_with_start_and_end_bit_candidates_scored(
            reader,
            header,
            |attempt_reader, object_data_end_bit| {
                entities::decode_attdef_r2010(attempt_reader, object_data_end_bit, object_handle)
            },
            score_attrib_entity_candidate,
        ),
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            decode_r2010_entity_with_start_and_end_bit_candidates_scored(
                reader,
                header,
                |attempt_reader, object_data_end_bit| {
                    entities::decode_attdef_r2013(
                        attempt_reader,
                        object_data_end_bit,
                        object_handle,
                    )
                },
                score_attrib_entity_candidate,
            )
        }
        version::DwgVersion::R2007 => entities::decode_attdef_r2007(reader),
        _ => entities::decode_attdef(reader),
    }
}

fn score_attrib_entity_candidate(entity: &entities::AttribEntity) -> i64 {
    fn is_finite_point3(point: (f64, f64, f64)) -> bool {
        point.0.is_finite()
            && point.1.is_finite()
            && point.2.is_finite()
            && point.0.abs() <= 1.0e12
            && point.1.abs() <= 1.0e12
            && point.2.abs() <= 1.0e12
    }

    fn score_point3(point: (f64, f64, f64)) -> i64 {
        if !is_finite_point3(point) {
            return -200;
        }
        let max_abs = point.0.abs().max(point.1.abs()).max(point.2.abs());
        if max_abs < 1.0e-12 {
            -32
        } else if max_abs < 1.0e-6 {
            -12
        } else if max_abs <= 1.0e7 {
            16
        } else {
            4
        }
    }

    fn score_text(text: &str) -> i64 {
        if text.is_empty() {
            return -40;
        }
        let mut score = 0i64;
        for ch in text.chars() {
            score += if ch == '\u{FFFD}' || ('\u{E000}'..='\u{F8FF}').contains(&ch) {
                -6
            } else if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
                -5
            } else if ch.is_ascii_alphanumeric() {
                2
            } else if ch.is_ascii_punctuation() || ch.is_ascii_whitespace() {
                1
            } else if matches!(
                ch,
                '\u{3000}'..='\u{303F}'
                    | '\u{3040}'..='\u{309F}'
                    | '\u{30A0}'..='\u{30FF}'
                    | '\u{3400}'..='\u{4DBF}'
                    | '\u{4E00}'..='\u{9FFF}'
                    | '\u{FF01}'..='\u{FF60}'
                    | '\u{FFE0}'..='\u{FFE6}'
            ) {
                2
            } else if ch.is_alphabetic() || ch.is_numeric() || ch.is_whitespace() {
                1
            } else {
                -2
            };
        }
        score
    }

    let mut score = score_text(&entity.text);
    if let Some(tag) = entity.tag.as_deref() {
        score = score.saturating_add(score_text(tag) / 2);
        score = score.saturating_add(if tag.is_empty() { -16 } else { 16 });
    } else {
        score = score.saturating_sub(8);
    }
    if let Some(prompt) = entity.prompt.as_deref() {
        score = score.saturating_add(score_text(prompt) / 2);
        if !prompt.is_empty() {
            score = score.saturating_add(8);
        }
    }

    score += score_point3(entity.insertion).saturating_mul(2);
    score += entity.alignment.map(score_point3).unwrap_or(0);
    score += if is_finite_point3(entity.extrusion) {
        let norm_sq = entity.extrusion.0 * entity.extrusion.0
            + entity.extrusion.1 * entity.extrusion.1
            + entity.extrusion.2 * entity.extrusion.2;
        if (norm_sq - 1.0).abs() <= 1.0e-6 {
            16
        } else if norm_sq > 0.25 && norm_sq < 4.0 {
            8
        } else {
            -16
        }
    } else {
        -40
    };
    score += if entity.thickness.is_finite() && entity.thickness.abs() <= 1.0e12 {
        2
    } else {
        -40
    };
    score += if entity.oblique_angle.is_finite() && entity.oblique_angle.abs() <= 1.0e12 {
        2
    } else {
        -20
    };
    score += if entity.rotation.is_finite() && entity.rotation.abs() <= 1.0e12 {
        2
    } else {
        -20
    };
    score += if entity.height.is_finite() && entity.height >= 1.0e-4 && entity.height <= 1.0e4 {
        32
    } else if entity.height.is_finite() && entity.height > 0.0 && entity.height <= 1.0e6 {
        4
    } else {
        -120
    };
    score += if entity.width_factor.is_finite() && entity.width_factor >= 1.0e-3 && entity.width_factor <= 100.0 {
        16
    } else if entity.width_factor.is_finite()
        && entity.width_factor > 0.0
        && entity.width_factor <= 1.0e4
    {
        4
    } else {
        -80
    };
    score += if entity.generation <= 6 && entity.generation % 2 == 0 {
        8
    } else {
        -24
    };
    score += if entity.horizontal_alignment <= 6 {
        8
    } else {
        -24
    };
    score += if entity.vertical_alignment <= 6 {
        8
    } else {
        -24
    };
    score += if entity.flags <= 15 { 8 } else { -32 };
    score += if entity.layer_handle != 0 { 8 } else { -20 };
    score += if entity.owner_handle.is_some() { 8 } else { 0 };
    score += if entity.style_handle.is_some() { 4 } else { 0 };
    score
}

fn decode_r2010_entity_with_start_and_end_bit_candidates_scored<T, F, S>(
    reader: &mut BitReader<'_>,
    header: &ApiObjectHeader,
    mut decode_entity: F,
    mut score_entity: S,
) -> crate::core::result::Result<T>
where
    F: FnMut(&mut BitReader<'_>, u32) -> crate::core::result::Result<T>,
    S: FnMut(&T) -> i64,
{
    let total_bits = header.data_size.saturating_mul(8);
    let start_bit = reader.tell_bits() as u32;
    if start_bit >= total_bits {
        return Err(DwgError::new(
            ErrorKind::Format,
            "entity body start bit exceeds object size",
        ));
    }

    let mut candidate_bits: Vec<u32> = Vec::new();
    if let Ok(primary) = resolve_r2010_object_data_end_bit(header) {
        candidate_bits.push(primary);
    }
    for candidate in resolve_r2010_object_data_end_bit_candidates(header) {
        if !candidate_bits.contains(&candidate) {
            candidate_bits.push(candidate);
        }
    }
    if candidate_bits.is_empty() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "no R2010 object data end-bit candidates",
        ));
    }

    let mut start_candidates = vec![start_bit];
    for delta_bits in 1..=64u32 {
        let candidate = start_bit.saturating_sub(delta_bits);
        if candidate < total_bits {
            start_candidates.push(candidate);
        }
    }
    for delta_bits in 1..=64u32 {
        let candidate = start_bit.saturating_add(delta_bits);
        if candidate < total_bits {
            start_candidates.push(candidate);
        }
    }
    start_candidates.sort_unstable();
    start_candidates.dedup();

    let canonical_end_bit = resolve_r2010_object_data_end_bit(header).ok();
    let original_reader = reader.clone();
    let mut best: Option<(i64, T, BitReader<'_>)> = None;
    let mut first_err: Option<DwgError> = None;

    for start_candidate in start_candidates {
        let mut start_reader = original_reader.clone();
        start_reader.set_bit_pos(start_candidate);
        for object_data_end_bit in candidate_bits.iter().copied() {
            let mut attempt_reader = start_reader.clone();
            match decode_entity(&mut attempt_reader, object_data_end_bit) {
                Ok(entity) => {
                    let mut score = score_entity(&entity);
                    if let Some(canonical) = canonical_end_bit {
                        score = score.saturating_sub(canonical.abs_diff(object_data_end_bit) as i64);
                    }
                    score = score.saturating_sub(i64::from(start_candidate.abs_diff(start_bit)) * 6);
                    match &best {
                        Some((best_score, _, _)) if score <= *best_score => {}
                        _ => best = Some((score, entity, attempt_reader)),
                    }
                }
                Err(err) => {
                    if first_err.is_none() {
                        first_err = Some(err);
                    }
                }
            }
        }
    }

    if let Some((_score, entity, attempt_reader)) = best {
        *reader = attempt_reader;
        return Ok(entity);
    }

    Err(first_err.unwrap_or_else(|| {
        DwgError::new(
            ErrorKind::Format,
            "failed to decode R2010 entity for all start/end-bit candidates",
        )
    }))
}

fn decode_r2010_entity_with_end_bit_candidates<T, F>(
    reader: &mut BitReader<'_>,
    header: &ApiObjectHeader,
    mut decode_entity: F,
) -> crate::core::result::Result<T>
where
    F: FnMut(&mut BitReader<'_>, u32) -> crate::core::result::Result<T>,
{
    let mut candidate_bits: Vec<u32> = Vec::new();
    if let Ok(primary) = resolve_r2010_object_data_end_bit(header) {
        candidate_bits.push(primary);
    }
    for candidate in resolve_r2010_object_data_end_bit_candidates(header) {
        if !candidate_bits.contains(&candidate) {
            candidate_bits.push(candidate);
        }
    }
    if candidate_bits.is_empty() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "no R2010 object data end-bit candidates",
        ));
    }

    let mut first_err: Option<DwgError> = None;
    for object_data_end_bit in candidate_bits {
        let mut attempt_reader = reader.clone();
        match decode_entity(&mut attempt_reader, object_data_end_bit) {
            Ok(entity) => {
                *reader = attempt_reader;
                return Ok(entity);
            }
            Err(err) => {
                if first_err.is_none() {
                    first_err = Some(err);
                }
            }
        }
    }

    Err(first_err.unwrap_or_else(|| {
        DwgError::new(
            ErrorKind::Format,
            "failed to decode R2010 entity for all end-bit candidates",
        )
    }))
}

fn decode_r2010_entity_with_end_bit_candidates_scored<T, F, S>(
    reader: &mut BitReader<'_>,
    header: &ApiObjectHeader,
    mut decode_entity: F,
    mut score_entity: S,
) -> crate::core::result::Result<T>
where
    F: FnMut(&mut BitReader<'_>, u32) -> crate::core::result::Result<T>,
    S: FnMut(&T) -> i64,
{
    let mut candidate_bits: Vec<u32> = Vec::new();
    if let Ok(primary) = resolve_r2010_object_data_end_bit(header) {
        candidate_bits.push(primary);
    }
    for candidate in resolve_r2010_object_data_end_bit_candidates(header) {
        if !candidate_bits.contains(&candidate) {
            candidate_bits.push(candidate);
        }
    }
    if candidate_bits.is_empty() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "no R2010 object data end-bit candidates",
        ));
    }

    let canonical_end_bit = resolve_r2010_object_data_end_bit(header).ok();
    let mut best: Option<(i64, T, BitReader<'_>)> = None;
    let mut first_err: Option<DwgError> = None;
    for object_data_end_bit in candidate_bits {
        let mut attempt_reader = reader.clone();
        match decode_entity(&mut attempt_reader, object_data_end_bit) {
            Ok(entity) => {
                let mut score = score_entity(&entity);
                if let Some(canonical) = canonical_end_bit {
                    score = score.saturating_sub(canonical.abs_diff(object_data_end_bit) as i64);
                }
                match &best {
                    Some((best_score, _, _)) if score <= *best_score => {}
                    _ => best = Some((score, entity, attempt_reader)),
                }
            }
            Err(err) => {
                if first_err.is_none() {
                    first_err = Some(err);
                }
            }
        }
    }

    if let Some((_score, entity, attempt_reader)) = best {
        *reader = attempt_reader;
        return Ok(entity);
    }

    Err(first_err.unwrap_or_else(|| {
        DwgError::new(
            ErrorKind::Format,
            "failed to decode R2010 entity for all end-bit candidates",
        )
    }))
}

fn decode_mtext_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::MTextEntity> {
    match version {
        version::DwgVersion::R2010 => decode_r2010_entity_with_end_bit_candidates_scored(
            reader,
            header,
            |attempt_reader, object_data_end_bit| {
                entities::decode_mtext_r2010(attempt_reader, object_data_end_bit, object_handle)
            },
            score_mtext_entity_candidate,
        ),
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            decode_r2010_entity_with_end_bit_candidates_scored(
                reader,
                header,
                |attempt_reader, object_data_end_bit| {
                    entities::decode_mtext_r2013(attempt_reader, object_data_end_bit, object_handle)
                },
                score_mtext_entity_candidate,
            )
        }
        version::DwgVersion::R2007 => entities::decode_mtext_r2007(reader),
        version::DwgVersion::R2004 => entities::decode_mtext_r2004(reader),
        _ => entities::decode_mtext(reader),
    }
}

fn score_mtext_entity_candidate(entity: &entities::MTextEntity) -> i64 {
    fn is_finite_point3(point: (f64, f64, f64)) -> bool {
        point.0.is_finite()
            && point.1.is_finite()
            && point.2.is_finite()
            && point.0.abs() <= 1.0e12
            && point.1.abs() <= 1.0e12
            && point.2.abs() <= 1.0e12
    }

    fn score_text(text: &str) -> i64 {
        if text.is_empty() {
            return -200;
        }
        let mut score = 0i64;
        for ch in text.chars() {
            score += if ch == '\u{FFFD}' || ('\u{E000}'..='\u{F8FF}').contains(&ch) {
                -6
            } else if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
                -5
            } else if ch.is_ascii_alphanumeric() {
                2
            } else if ch.is_ascii_punctuation() || ch.is_ascii_whitespace() {
                1
            } else if matches!(
                ch,
                '\u{3000}'..='\u{303F}'
                    | '\u{3040}'..='\u{309F}'
                    | '\u{30A0}'..='\u{30FF}'
                    | '\u{3400}'..='\u{4DBF}'
                    | '\u{4E00}'..='\u{9FFF}'
                    | '\u{FF01}'..='\u{FF60}'
                    | '\u{FFE0}'..='\u{FFE6}'
            ) {
                2
            } else if ch.is_alphabetic() || ch.is_numeric() || ch.is_whitespace() {
                1
            } else {
                -2
            };
        }
        score
    }

    let mut score = score_text(&entity.text);
    score += if is_finite_point3(entity.insertion) { 32 } else { -200 };
    score += if is_finite_point3(entity.extrusion) { 8 } else { -40 };
    score += if is_finite_point3(entity.x_axis_dir) { 8 } else { -40 };
    score += if entity.text_height.is_finite() && entity.text_height > 0.0 && entity.text_height <= 1.0e6 {
        32
    } else {
        -120
    };
    score += if entity.rect_width.is_finite() && entity.rect_width >= 0.0 && entity.rect_width <= 1.0e9 {
        6
    } else {
        -20
    };
    score += if (1..=9).contains(&entity.attachment) {
        12
    } else {
        -24
    };
    score += if matches!(entity.drawing_dir, 1 | 3 | 5) {
        8
    } else {
        -16
    };
    score
}


impl_version_dispatch! {
    no_r14;
    fn decode_leader_for_version -> entities::LeaderEntity;
    r2010: entities::decode_leader_r2010;
    r2013: entities::decode_leader_r2013;
    r2007: entities::decode_leader_r2007;
    default: entities::decode_leader;
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

impl_version_dispatch! {
    no_r14;
    fn decode_tolerance_for_version -> entities::ToleranceEntity;
    r2010: entities::decode_tolerance_r2010;
    r2013: entities::decode_tolerance_r2013;
    r2007: entities::decode_tolerance_r2007;
    default: entities::decode_tolerance;
}

impl_version_dispatch! {
    no_r14;
    fn decode_mline_for_version -> entities::MLineEntity;
    r2010: entities::decode_mline_r2010;
    r2013: entities::decode_mline_r2013;
    r2007: entities::decode_mline_r2007;
    default: entities::decode_mline;
}

impl_version_dispatch! {
    no_r14;
    fn decode_3dface_for_version -> entities::Face3dEntity;
    r2010: entities::decode_3dface_r2010;
    r2013: entities::decode_3dface_r2013;
    r2007: entities::decode_3dface_r2007;
    default: entities::decode_3dface;
}

impl_version_dispatch! {
    no_r14;
    fn decode_solid_for_version -> entities::SolidEntity;
    r2010: entities::decode_solid_r2010;
    r2013: entities::decode_solid_r2013;
    r2007: entities::decode_solid_r2007;
    default: entities::decode_solid;
}

impl_version_dispatch! {
    no_r14;
    fn decode_trace_for_version -> entities::TraceEntity;
    r2010: entities::decode_trace_r2010;
    r2013: entities::decode_trace_r2013;
    r2007: entities::decode_trace_r2007;
    default: entities::decode_trace;
}

impl_version_dispatch! {
    no_r14;
    fn decode_shape_for_version -> entities::ShapeEntity;
    r2010: entities::decode_shape_r2010;
    r2013: entities::decode_shape_r2013;
    r2007: entities::decode_shape_r2007;
    default: entities::decode_shape;
}

impl_version_dispatch! {
    with_r14;
    fn decode_viewport_for_version -> entities::ViewportEntity;
    r14: entities::decode_viewport_r14;
    r2010: entities::decode_viewport_r2010;
    r2013: entities::decode_viewport_r2013;
    r2007: entities::decode_viewport_r2007;
    default: entities::decode_viewport;
}

impl_version_dispatch! {
    with_r14;
    fn decode_oleframe_for_version -> entities::OleFrameEntity;
    r14: entities::decode_oleframe_r14;
    r2010: entities::decode_oleframe_r2010;
    r2013: entities::decode_oleframe_r2013;
    r2007: entities::decode_oleframe_r2007;
    default: entities::decode_oleframe;
}

impl_version_dispatch! {
    with_r14;
    fn decode_ole2frame_for_version -> entities::OleFrameEntity;
    r14: entities::decode_ole2frame_r14;
    r2010: entities::decode_ole2frame_r2010;
    r2013: entities::decode_ole2frame_r2013;
    r2007: entities::decode_ole2frame_r2007;
    default: entities::decode_ole2frame;
}

impl_version_dispatch! {
    with_r14;
    fn decode_long_transaction_for_version -> entities::LongTransactionEntity;
    r14: entities::decode_long_transaction_r14;
    r2010: entities::decode_long_transaction_r2010;
    r2013: entities::decode_long_transaction_r2013;
    r2007: entities::decode_long_transaction_r2007;
    default: entities::decode_long_transaction;
}

impl_version_dispatch! {
    with_r14;
    fn decode_region_for_version -> entities::RegionEntity;
    r14: entities::decode_region_r14;
    r2010: entities::decode_region_r2010;
    r2013: entities::decode_region_r2013;
    r2007: entities::decode_region_r2007;
    default: entities::decode_region;
}

impl_version_dispatch! {
    with_r14;
    fn decode_3dsolid_for_version -> entities::Solid3dEntity;
    r14: entities::decode_3dsolid_r14;
    r2010: entities::decode_3dsolid_r2010;
    r2013: entities::decode_3dsolid_r2013;
    r2007: entities::decode_3dsolid_r2007;
    default: entities::decode_3dsolid;
}

impl_version_dispatch! {
    with_r14;
    fn decode_body_for_version -> entities::BodyEntity;
    r14: entities::decode_body_r14;
    r2010: entities::decode_body_r2010;
    r2013: entities::decode_body_r2013;
    r2007: entities::decode_body_r2007;
    default: entities::decode_body;
}

impl_version_dispatch! {
    with_r14;
    fn decode_ray_for_version -> entities::RayEntity;
    r14: entities::decode_ray_r14;
    r2010: entities::decode_ray_r2010;
    r2013: entities::decode_ray_r2013;
    r2007: entities::decode_ray_r2007;
    default: entities::decode_ray;
}

impl_version_dispatch! {
    with_r14;
    fn decode_xline_for_version -> entities::XLineEntity;
    r14: entities::decode_xline_r14;
    r2010: entities::decode_xline_r2010;
    r2013: entities::decode_xline_r2013;
    r2007: entities::decode_xline_r2007;
    default: entities::decode_xline;
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

#[cfg(test)]
mod proxy_chunk_tests {
    use super::parse_proxy_graphic_chunk_infos;

    #[test]
    fn parse_proxy_graphic_chunk_infos_reads_well_formed_chunks() {
        let mut data = Vec::new();
        data.extend_from_slice(&4_i32.to_le_bytes());
        data.extend_from_slice(&10_i32.to_le_bytes());
        data.extend_from_slice(&[1, 2, 3, 4]);
        data.extend_from_slice(&8_i32.to_le_bytes());
        data.extend_from_slice(&38_i32.to_le_bytes());
        data.extend_from_slice(&[5, 6, 7, 8, 9, 10, 11, 12]);

        let infos = parse_proxy_graphic_chunk_infos(&data);
        assert_eq!(infos, vec![(10, 4), (38, 8)]);
    }

    #[test]
    fn parse_proxy_graphic_chunk_infos_stops_on_truncated_chunk() {
        let mut data = Vec::new();
        data.extend_from_slice(&16_i32.to_le_bytes());
        data.extend_from_slice(&10_i32.to_le_bytes());
        data.extend_from_slice(&[1, 2, 3, 4]);

        let infos = parse_proxy_graphic_chunk_infos(&data);
        assert!(infos.is_empty());
    }
}
