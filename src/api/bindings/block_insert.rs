fn prepare_insert_name_resolution_state(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
) -> PyResult<InsertNameResolutionState> {
    let block_header_entries =
        collect_block_header_name_entries_in_order(decoder, dynamic_types, index, best_effort)?;
    let mut known_block_handles: HashSet<u64> = HashSet::new();
    let mut block_header_names: HashMap<u64, String> = HashMap::new();
    let mut block_header_decoded_by_raw: HashMap<u64, u64> = HashMap::new();
    for (raw_handle, decoded_handle, name) in block_header_entries {
        block_header_decoded_by_raw.insert(raw_handle, decoded_handle);
        known_block_handles.insert(raw_handle);
        known_block_handles.insert(decoded_handle);
        if name.is_empty() {
            continue;
        }
        block_header_names
            .entry(raw_handle)
            .or_insert_with(|| name.clone());
        block_header_names.entry(decoded_handle).or_insert(name);
    }
    let (block_name_aliases, recovered_header_names) = collect_block_name_aliases_in_order(
        decoder,
        dynamic_types,
        index,
        best_effort,
        &block_header_names,
    )?;
    for (header_handle, name) in recovered_header_names {
        if name.is_empty() {
            continue;
        }
        known_block_handles.insert(header_handle);
        block_header_names
            .entry(header_handle)
            .or_insert_with(|| name.clone());
        if let Some(decoded_handle) = block_header_decoded_by_raw.get(&header_handle).copied() {
            known_block_handles.insert(decoded_handle);
            block_header_names.entry(decoded_handle).or_insert(name);
        }
    }
    for (alias_handle, name) in block_name_aliases {
        known_block_handles.insert(alias_handle);
        block_header_names.entry(alias_handle).or_insert(name);
    }
    let block_record_aliases = collect_block_record_handle_aliases_in_order(
        decoder,
        dynamic_types,
        index,
        best_effort,
        &block_header_names,
    )?;
    for (alias_handle, name) in block_record_aliases {
        known_block_handles.insert(alias_handle);
        block_header_names.entry(alias_handle).or_insert(name);
    }
    let object_type_codes = collect_object_type_codes(decoder, index, best_effort)?;
    let known_layer_handles: HashSet<u64> =
        collect_known_layer_handles_in_order(decoder, dynamic_types, index, best_effort)?
            .into_iter()
            .collect();
    let stream_aliases = collect_block_header_stream_aliases_in_order(
        decoder,
        dynamic_types,
        index,
        best_effort,
        &block_header_names,
        &object_type_codes,
        &known_layer_handles,
    )?;
    for (alias_handle, name) in stream_aliases {
        known_block_handles.insert(alias_handle);
        block_header_names.entry(alias_handle).or_insert(name);
    }
    let named_block_handles: HashSet<u64> = block_header_names.keys().copied().collect();
    Ok(InsertNameResolutionState {
        known_block_handles,
        block_header_names,
        named_block_handles,
    })
}

fn decode_insert_entities_with_state(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    state: &mut InsertNameResolutionState,
    limit: Option<usize>,
) -> PyResult<Vec<InsertEntityRow>> {
    let mut decoded_rows: Vec<(u64, f64, f64, f64, f64, f64, f64, f64, Option<u64>)> = Vec::new();
    let mut unresolved_insert_candidates: HashMap<u64, Vec<u64>> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x07, "INSERT", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_insert_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let resolved_block_handle = recover_insert_block_header_handle_r2010_plus(
            &record,
            decoder.version(),
            &header,
            obj.handle.0,
            entity.block_header_handle,
            &state.known_block_handles,
            &state.named_block_handles,
        );
        decoded_rows.push((
            entity.handle,
            entity.position.0,
            entity.position.1,
            entity.position.2,
            entity.scale.0,
            entity.scale.1,
            entity.scale.2,
            entity.rotation,
            resolved_block_handle,
        ));
        if let Some(limit) = limit {
            if decoded_rows.len() >= limit {
                break;
            }
        }
    }
    let unresolved_handles: HashSet<u64> = decoded_rows
        .iter()
        .filter_map(|row| row.8)
        .filter(|handle| !state.block_header_names.contains_key(handle))
        .collect();
    if !unresolved_handles.is_empty() {
        let targeted_aliases = collect_block_header_targeted_aliases_in_order(
            decoder,
            dynamic_types,
            index,
            best_effort,
            &state.block_header_names,
            &unresolved_handles,
        )?;
        for (alias_handle, name) in targeted_aliases {
            state.known_block_handles.insert(alias_handle);
            state.block_header_names.entry(alias_handle).or_insert(name);
        }
    }
    let unresolved_insert_handles: HashSet<u64> = decoded_rows
        .iter()
        .filter_map(|row| {
            let missing = row
                .8
                .and_then(|handle| state.block_header_names.get(&handle))
                .is_none();
            if missing {
                Some(row.0)
            } else {
                None
            }
        })
        .collect();
    if !unresolved_insert_handles.is_empty() {
        let mut extra_targets: HashSet<u64> = HashSet::new();
        for obj in index.objects.iter() {
            if !unresolved_insert_handles.contains(&obj.handle.0) {
                continue;
            }
            let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
            else {
                continue;
            };
            if !matches_type_name(header.type_code, 0x07, "INSERT", dynamic_types) {
                continue;
            }
            let mut reader = record.bit_reader();
            if skip_object_type_prefix(&mut reader, decoder.version()).is_err() {
                continue;
            }
            let Ok(entity) =
                decode_insert_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
            else {
                continue;
            };
            let candidates = collect_insert_block_handle_candidates_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.block_header_handle,
                Some(&state.known_block_handles),
                8,
            );
            if candidates.is_empty() {
                continue;
            }
            for candidate in candidates.iter().copied().take(4) {
                if !state.block_header_names.contains_key(&candidate) {
                    extra_targets.insert(candidate);
                }
            }
            unresolved_insert_candidates.insert(obj.handle.0, candidates);
        }
        if !extra_targets.is_empty() {
            let targeted_aliases = collect_block_header_targeted_aliases_in_order(
                decoder,
                dynamic_types,
                index,
                best_effort,
                &state.block_header_names,
                &extra_targets,
            )?;
            for (alias_handle, name) in targeted_aliases {
                state.known_block_handles.insert(alias_handle);
                state.block_header_names.entry(alias_handle).or_insert(name);
            }
        }
    }

    let available_named_handles: Vec<u64> = state.block_header_names.keys().copied().collect();
    let mut result = Vec::with_capacity(decoded_rows.len());
    let debug_insert_names = std::env::var("EZDWG_DEBUG_INSERT_NAMES")
        .ok()
        .is_some_and(|v| v != "0");
    for (handle, px, py, pz, sx, sy, sz, rotation, block_handle) in decoded_rows {
        let mut resolved_name =
            block_handle.and_then(|h| state.block_header_names.get(&h).cloned());
        if resolved_name.is_none() {
            if let Some(candidates) = unresolved_insert_candidates.get(&handle) {
                resolved_name = candidates
                    .iter()
                    .find_map(|candidate| state.block_header_names.get(candidate).cloned());
                if resolved_name.is_none() {
                    let mut nearby_names: HashSet<String> = HashSet::new();
                    for candidate in candidates {
                        for known in &available_named_handles {
                            if known.abs_diff(*candidate) <= 8 {
                                if let Some(name) = state.block_header_names.get(known) {
                                    nearby_names.insert(name.clone());
                                }
                            }
                        }
                    }
                    if nearby_names.len() == 1 {
                        resolved_name = nearby_names.into_iter().next();
                    }
                }
            }
        }
        if debug_insert_names {
            let candidate_debug = unresolved_insert_candidates.get(&handle);
            eprintln!(
                "[insert-name] insert={} block_handle={:?} name={:?} candidates={:?}",
                handle, block_handle, resolved_name, candidate_debug
            );
        }
        result.push((handle, px, py, pz, sx, sy, sz, rotation, resolved_name));
    }
    Ok(result)
}

fn decode_minsert_entities_with_state(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    state: &mut InsertNameResolutionState,
    limit: Option<usize>,
) -> PyResult<Vec<MInsertEntityRow>> {
    let mut decoded_rows: Vec<(
        u64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        u16,
        u16,
        f64,
        f64,
        Option<u64>,
    )> = Vec::new();
    let mut unresolved_minsert_candidates: HashMap<u64, Vec<u64>> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x08, "MINSERT", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match entities::decode_minsert(&mut reader) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let block_handle = recover_insert_block_header_handle_r2010_plus(
            &record,
            decoder.version(),
            &header,
            obj.handle.0,
            None,
            &state.known_block_handles,
            &state.named_block_handles,
        );
        decoded_rows.push((
            entity.handle,
            entity.position.0,
            entity.position.1,
            entity.position.2,
            entity.scale.0,
            entity.scale.1,
            entity.scale.2,
            entity.rotation,
            entity.num_columns,
            entity.num_rows,
            entity.column_spacing,
            entity.row_spacing,
            block_handle,
        ));
        if let Some(limit) = limit {
            if decoded_rows.len() >= limit {
                break;
            }
        }
    }

    let unresolved_handles: HashSet<u64> = decoded_rows
        .iter()
        .filter_map(|row| row.12)
        .filter(|handle| !state.block_header_names.contains_key(handle))
        .collect();
    if !unresolved_handles.is_empty() {
        let targeted_aliases = collect_block_header_targeted_aliases_in_order(
            decoder,
            dynamic_types,
            index,
            best_effort,
            &state.block_header_names,
            &unresolved_handles,
        )?;
        for (alias_handle, name) in targeted_aliases {
            state.known_block_handles.insert(alias_handle);
            state.block_header_names.entry(alias_handle).or_insert(name);
        }
    }
    let unresolved_minsert_handles: HashSet<u64> = decoded_rows
        .iter()
        .filter_map(|row| {
            let missing = row
                .12
                .and_then(|handle| state.block_header_names.get(&handle))
                .is_none();
            if missing {
                Some(row.0)
            } else {
                None
            }
        })
        .collect();
    if !unresolved_minsert_handles.is_empty() {
        let mut extra_targets: HashSet<u64> = HashSet::new();
        for obj in index.objects.iter() {
            if !unresolved_minsert_handles.contains(&obj.handle.0) {
                continue;
            }
            let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
            else {
                continue;
            };
            if !matches_type_name(header.type_code, 0x08, "MINSERT", dynamic_types) {
                continue;
            }
            let mut reader = record.bit_reader();
            if skip_object_type_prefix(&mut reader, decoder.version()).is_err() {
                continue;
            }
            let Ok(_entity) = entities::decode_minsert(&mut reader) else {
                continue;
            };
            let candidates = collect_insert_block_handle_candidates_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                None,
                Some(&state.known_block_handles),
                8,
            );
            if candidates.is_empty() {
                continue;
            }
            for candidate in candidates.iter().copied().take(4) {
                if !state.block_header_names.contains_key(&candidate) {
                    extra_targets.insert(candidate);
                }
            }
            unresolved_minsert_candidates.insert(obj.handle.0, candidates);
        }
        if !extra_targets.is_empty() {
            let targeted_aliases = collect_block_header_targeted_aliases_in_order(
                decoder,
                dynamic_types,
                index,
                best_effort,
                &state.block_header_names,
                &extra_targets,
            )?;
            for (alias_handle, name) in targeted_aliases {
                state.known_block_handles.insert(alias_handle);
                state.block_header_names.entry(alias_handle).or_insert(name);
            }
        }
    }

    let available_named_handles: Vec<u64> = state.block_header_names.keys().copied().collect();
    let mut result = Vec::with_capacity(decoded_rows.len());
    for (
        handle,
        px,
        py,
        pz,
        sx,
        sy,
        sz,
        rotation,
        num_columns,
        num_rows,
        column_spacing,
        row_spacing,
        block_handle,
    ) in decoded_rows
    {
        let mut resolved_name =
            block_handle.and_then(|h| state.block_header_names.get(&h).cloned());
        if resolved_name.is_none() {
            if let Some(candidates) = unresolved_minsert_candidates.get(&handle) {
                resolved_name = candidates
                    .iter()
                    .find_map(|candidate| state.block_header_names.get(candidate).cloned());
                if resolved_name.is_none() {
                    let mut nearby_names: HashSet<String> = HashSet::new();
                    for candidate in candidates {
                        for known in &available_named_handles {
                            if known.abs_diff(*candidate) <= 8 {
                                if let Some(name) = state.block_header_names.get(known) {
                                    nearby_names.insert(name.clone());
                                }
                            }
                        }
                    }
                    if nearby_names.len() == 1 {
                        resolved_name = nearby_names.into_iter().next();
                    }
                }
            }
        }
        result.push((
            handle,
            px,
            py,
            pz,
            sx,
            sy,
            sz,
            rotation,
            (
                num_columns,
                num_rows,
                column_spacing,
                row_spacing,
                resolved_name,
            ),
        ));
    }

    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_insert_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<InsertEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut state =
        prepare_insert_name_resolution_state(&decoder, &dynamic_types, &index, best_effort)?;
    decode_insert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_minsert_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<MInsertEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut state =
        prepare_insert_name_resolution_state(&decoder, &dynamic_types, &index, best_effort)?;
    decode_minsert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_insert_minsert_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<InsertMInsertRows> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut state =
        prepare_insert_name_resolution_state(&decoder, &dynamic_types, &index, best_effort)?;
    let inserts = decode_insert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )?;
    let minserts = decode_minsert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )?;
    Ok((inserts, minserts))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_insert_minsert_dimension_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<InsertMInsertDimensionRows> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut state =
        prepare_insert_name_resolution_state(&decoder, &dynamic_types, &index, best_effort)?;
    let inserts = decode_insert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )?;
    let minserts = decode_minsert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )?;
    let dimensions = decode_dimension_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &state,
        limit,
    )?;
    Ok((inserts, minserts, dimensions))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_block_header_names(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<BlockHeaderNameRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let state =
        prepare_insert_name_resolution_state(&decoder, &dynamic_types, &index, best_effort)?;
    let mut rows: Vec<BlockHeaderNameRow> = state.block_header_names.into_iter().collect();
    rows.sort_by_key(|(handle, _)| *handle);
    if let Some(limit) = limit {
        rows.truncate(limit);
    }
    Ok(rows)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_block_entity_names(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<BlockEntityNameRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut ordered_objects: Vec<_> = index.objects.iter().collect();
    ordered_objects.sort_by_key(|obj| obj.offset);
    let is_r2010_plus = matches!(
        decoder.version(),
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    );
    let header_entries =
        collect_block_header_name_entries_in_order(&decoder, &dynamic_types, &index, best_effort)?;
    let mut header_entry_name_by_handle: HashMap<u64, String> = HashMap::new();
    for (raw_handle, decoded_handle, name) in header_entries.iter() {
        if name.is_empty() {
            continue;
        }
        header_entry_name_by_handle
            .entry(*raw_handle)
            .or_insert_with(|| name.clone());
        header_entry_name_by_handle
            .entry(*decoded_handle)
            .or_insert_with(|| name.clone());
    }
    let block_header_names =
        collect_block_header_names_in_order(&decoder, &dynamic_types, &index, best_effort, None)?;
    let (mut block_aliases, mut endblk_aliases) = collect_block_and_endblk_handle_aliases_in_order(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &block_header_names,
    )?;
    let header_names_in_order: Vec<String> = if is_r2010_plus {
        header_entries
            .into_iter()
            .filter_map(
                |(_raw_handle, _decoded_handle, name)| {
                    if name.is_empty() {
                        None
                    } else {
                        Some(name)
                    }
                },
            )
            .collect()
    } else {
        let mut names = Vec::new();
        for obj in ordered_objects.iter().copied() {
            let Some((_record, header)) =
                parse_record_and_header(&decoder, obj.offset, best_effort)?
            else {
                continue;
            };
            if !matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", &dynamic_types) {
                continue;
            }
            if let Some(name) = block_header_names
                .get(&obj.handle.0)
                .cloned()
                .filter(|value| !value.is_empty())
            {
                names.push(name);
                continue;
            }
            if let Some(name) = header_entry_name_by_handle
                .get(&obj.handle.0)
                .cloned()
                .filter(|value| !value.is_empty())
            {
                names.push(name);
            }
        }
        names
    };
    let mut block_handles_in_order: Vec<u64> = Vec::new();
    let mut endblk_handles_in_order: Vec<u64> = Vec::new();
    for obj in ordered_objects.iter().copied() {
        let Some((_record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x04, "BLOCK", &dynamic_types) {
            block_handles_in_order.push(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x05, "ENDBLK", &dynamic_types) {
            endblk_handles_in_order.push(obj.handle.0);
        }
    }

    if is_r2010_plus {
        let block_targets: HashSet<u64> = block_handles_in_order.iter().copied().collect();
        let endblk_targets: HashSet<u64> = endblk_handles_in_order.iter().copied().collect();
        if !block_targets.is_empty() {
            for (handle, name) in collect_block_header_targeted_aliases_in_order(
                &decoder,
                &dynamic_types,
                &index,
                best_effort,
                &block_header_names,
                &block_targets,
            )? {
                if !name.is_empty() {
                    block_aliases.insert(handle, name);
                }
            }
        }
        if !endblk_targets.is_empty() {
            for (handle, name) in collect_block_header_targeted_aliases_in_order(
                &decoder,
                &dynamic_types,
                &index,
                best_effort,
                &block_header_names,
                &endblk_targets,
            )? {
                if !name.is_empty() {
                    endblk_aliases.insert(handle, name);
                }
            }
        }
    }

    if !header_names_in_order.is_empty() {
        if !is_r2010_plus && block_handles_in_order.len() == header_names_in_order.len() {
            block_aliases = HashMap::new();
            for (handle, name) in block_handles_in_order
                .iter()
                .copied()
                .zip(header_names_in_order.iter())
            {
                block_aliases.insert(handle, name.clone());
            }
        } else {
            for (index, handle) in block_handles_in_order.iter().copied().enumerate() {
                if block_aliases.contains_key(&handle) {
                    continue;
                }
                if let Some(name) = header_names_in_order.get(index) {
                    block_aliases.insert(handle, name.clone());
                }
            }
        }

        if !is_r2010_plus && endblk_handles_in_order.len() == header_names_in_order.len() {
            endblk_aliases = HashMap::new();
            for (handle, name) in endblk_handles_in_order
                .iter()
                .copied()
                .zip(header_names_in_order.iter())
            {
                endblk_aliases.insert(handle, name.clone());
            }
        } else {
            for (index, handle) in endblk_handles_in_order.iter().copied().enumerate() {
                if endblk_aliases.contains_key(&handle) {
                    continue;
                }
                if let Some(name) = header_names_in_order.get(index) {
                    endblk_aliases.insert(handle, name.clone());
                }
            }
        }
    }

    let mut rows: Vec<BlockEntityNameRow> = Vec::new();
    rows.reserve(block_aliases.len().saturating_add(endblk_aliases.len()));
    for (handle, name) in block_aliases {
        rows.push((handle, "BLOCK".to_string(), name));
    }
    for (handle, name) in endblk_aliases {
        rows.push((handle, "ENDBLK".to_string(), name));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    if let Some(limit) = limit {
        rows.truncate(limit);
    }
    Ok(rows)
}

fn decode_insert_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::InsertEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_insert_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_insert_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_insert_r2007(reader),
        _ => entities::decode_insert(reader),
    }
}

fn collect_block_header_name_entries_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
) -> PyResult<Vec<(u64, u64, String)>> {
    let mut entries: Vec<(u64, u64, String)> = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            continue;
        }
        let prefer_prefixed = matches!(
            decoder.version(),
            version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
        );
        let mut decoded_handle_fallback = obj.handle.0;
        if prefer_prefixed {
            let mut prefixed_reader = record.bit_reader();
            if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                if let Ok(handle) =
                    decode_block_header_record_handle(&mut prefixed_reader, obj.handle.0)
                {
                    decoded_handle_fallback = handle;
                }
            } else {
                let mut reader = record.bit_reader();
                if let Ok(handle) = decode_block_header_record_handle(&mut reader, obj.handle.0) {
                    decoded_handle_fallback = handle;
                }
            }
        } else {
            let mut reader = record.bit_reader();
            if let Ok(handle) = decode_block_header_record_handle(&mut reader, obj.handle.0) {
                decoded_handle_fallback = handle;
            }
        }

        let mut parsed = if prefer_prefixed {
            let mut prefixed_reader = record.bit_reader();
            if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                decode_block_header_name_record(
                    &mut prefixed_reader,
                    decoder.version(),
                    obj.handle.0,
                    Some(&header),
                )
            } else {
                let mut reader = record.bit_reader();
                decode_block_header_name_record(
                    &mut reader,
                    decoder.version(),
                    obj.handle.0,
                    Some(&header),
                )
            }
        } else {
            let mut reader = record.bit_reader();
            decode_block_header_name_record(
                &mut reader,
                decoder.version(),
                obj.handle.0,
                Some(&header),
            )
        };

        let retry_alternate = parsed
            .as_ref()
            .map(|(_handle, name)| name.is_empty())
            .unwrap_or(true);
        if retry_alternate {
            if prefer_prefixed {
                let mut reader = record.bit_reader();
                if let Ok(row) = decode_block_header_name_record(
                    &mut reader,
                    decoder.version(),
                    obj.handle.0,
                    Some(&header),
                ) {
                    parsed = Ok(row);
                }
            } else {
                let mut prefixed_reader = record.bit_reader();
                if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                    if let Ok(row) = decode_block_header_name_record(
                        &mut prefixed_reader,
                        decoder.version(),
                        obj.handle.0,
                        Some(&header),
                    ) {
                        parsed = Ok(row);
                    }
                }
            }
        }
        let (decoded_handle, mut name) = match parsed {
            Ok(row) => row,
            Err(err) if best_effort || is_recoverable_decode_error(&err) => {
                let recovered_name =
                    recover_block_header_name_from_record(&record, decoder.version(), &header)
                        .unwrap_or_default();
                entries.push((obj.handle.0, decoded_handle_fallback, recovered_name));
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        if name.is_empty() {
            if let Some(recovered_name) =
                recover_block_header_name_from_record(&record, decoder.version(), &header)
            {
                name = recovered_name;
            }
        }
        entries.push((obj.handle.0, decoded_handle, name));
    }
    Ok(entries)
}

fn collect_block_header_names_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    only_handles: Option<&HashSet<u64>>,
) -> PyResult<HashMap<u64, String>> {
    let entries =
        collect_block_header_name_entries_in_order(decoder, dynamic_types, index, best_effort)?;
    let mut block_names: HashMap<u64, String> = HashMap::new();
    let mut raw_to_decoded: HashMap<u64, u64> = HashMap::new();
    for (raw_handle, decoded_handle, name) in entries {
        raw_to_decoded.insert(raw_handle, decoded_handle);
        if name.is_empty() {
            continue;
        }
        if let Some(handles) = only_handles {
            if !handles.contains(&raw_handle) && !handles.contains(&decoded_handle) {
                continue;
            }
        }
        block_names
            .entry(raw_handle)
            .or_insert_with(|| name.clone());
        block_names.entry(decoded_handle).or_insert(name);
    }
    let (_aliases, recovered_header_names) = collect_block_name_aliases_in_order(
        decoder,
        dynamic_types,
        index,
        best_effort,
        &block_names,
    )?;
    for (raw_handle, name) in recovered_header_names {
        if name.is_empty() {
            continue;
        }
        let decoded_handle = raw_to_decoded
            .get(&raw_handle)
            .copied()
            .unwrap_or(raw_handle);
        if let Some(handles) = only_handles {
            if !handles.contains(&raw_handle) && !handles.contains(&decoded_handle) {
                continue;
            }
        }
        block_names
            .entry(raw_handle)
            .or_insert_with(|| name.clone());
        block_names.entry(decoded_handle).or_insert(name);
    }
    Ok(block_names)
}

fn collect_block_name_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
) -> PyResult<(HashMap<u64, String>, HashMap<u64, String>)> {
    let mut aliases: HashMap<u64, String> = HashMap::new();
    let mut recovered_header_names: HashMap<u64, String> = HashMap::new();
    let mut pending_name: Option<String> = None;
    let mut pending_header_handle: Option<u64> = None;
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            pending_name = block_header_names.get(&obj.handle.0).cloned();
            pending_header_handle = Some(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x04, "BLOCK", dynamic_types) {
            let mut block_name = pending_name.clone();
            if block_name.is_none() || block_name.as_ref().is_some_and(|name| name.is_empty()) {
                block_name =
                    recover_block_name_from_block_record(&record, decoder.version(), &header);
                if let (Some(header_handle), Some(name)) =
                    (pending_header_handle, block_name.clone())
                {
                    if !name.is_empty() {
                        recovered_header_names.entry(header_handle).or_insert(name);
                    }
                }
            }
            if let Some(name) = block_name {
                aliases.insert(obj.handle.0, name);
            }
            continue;
        }
        if matches_type_name(header.type_code, 0x05, "ENDBLK", dynamic_types) {
            pending_name = None;
            pending_header_handle = None;
        }
    }
    Ok((aliases, recovered_header_names))
}

fn decode_block_record_handle(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    expected_handle: u64,
) -> crate::core::result::Result<u64> {
    let decoded = match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(api_header)?;
            entities::common::parse_common_entity_header_r2010(reader, object_data_end_bit)?.handle
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(api_header)?;
            entities::common::parse_common_entity_header_r2013(reader, object_data_end_bit)?.handle
        }
        _ => entities::common::parse_common_entity_header(reader)?.handle,
    };
    Ok(if decoded != 0 {
        decoded
    } else {
        expected_handle
    })
}

fn collect_block_record_handle_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
) -> PyResult<HashMap<u64, String>> {
    let mut aliases: HashMap<u64, String> = HashMap::new();
    let mut pending_name: Option<String> = None;
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            pending_name = block_header_names.get(&obj.handle.0).cloned();
            if pending_name.is_none() || pending_name.as_ref().is_some_and(|name| name.is_empty()) {
                let prefer_prefixed = matches!(
                    decoder.version(),
                    version::DwgVersion::R2010
                        | version::DwgVersion::R2013
                        | version::DwgVersion::R2018
                );
                let parsed = if prefer_prefixed {
                    let mut prefixed_reader = record.bit_reader();
                    if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                        decode_block_header_name_record(
                            &mut prefixed_reader,
                            decoder.version(),
                            obj.handle.0,
                            Some(&header),
                        )
                    } else {
                        let mut reader = record.bit_reader();
                        decode_block_header_name_record(
                            &mut reader,
                            decoder.version(),
                            obj.handle.0,
                            Some(&header),
                        )
                    }
                } else {
                    let mut reader = record.bit_reader();
                    decode_block_header_name_record(
                        &mut reader,
                        decoder.version(),
                        obj.handle.0,
                        Some(&header),
                    )
                };
                if let Ok((decoded_handle, decoded_name)) = parsed {
                    let mapped = block_header_names.get(&decoded_handle).cloned();
                    if mapped.is_some() {
                        pending_name = mapped;
                    } else if !decoded_name.is_empty() {
                        pending_name = Some(decoded_name);
                    }
                }
            }
            continue;
        }
        if matches_type_name(header.type_code, 0x04, "BLOCK", dynamic_types) {
            let Some(name) = pending_name.clone() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            aliases.entry(obj.handle.0).or_insert_with(|| name.clone());

            let prefer_prefixed = matches!(
                decoder.version(),
                version::DwgVersion::R2010
                    | version::DwgVersion::R2013
                    | version::DwgVersion::R2018
            );
            let decoded_handle = if prefer_prefixed {
                let mut prefixed_reader = record.bit_reader();
                if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                    decode_block_record_handle(
                        &mut prefixed_reader,
                        decoder.version(),
                        &header,
                        obj.handle.0,
                    )
                } else {
                    let mut reader = record.bit_reader();
                    decode_block_record_handle(
                        &mut reader,
                        decoder.version(),
                        &header,
                        obj.handle.0,
                    )
                }
            } else {
                let mut reader = record.bit_reader();
                decode_block_record_handle(&mut reader, decoder.version(), &header, obj.handle.0)
            };
            if let Ok(decoded_handle) = decoded_handle {
                aliases.entry(decoded_handle).or_insert(name);
            }
            continue;
        }
        if matches_type_name(header.type_code, 0x05, "ENDBLK", dynamic_types) {
            pending_name = None;
        }
    }
    Ok(aliases)
}

fn collect_block_and_endblk_handle_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
) -> PyResult<(HashMap<u64, String>, HashMap<u64, String>)> {
    let is_r2010_plus = matches!(
        decoder.version(),
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    );
    let mut block_aliases: HashMap<u64, String> = HashMap::new();
    let mut endblk_aliases: HashMap<u64, String> = HashMap::new();
    let mut pending_name: Option<String> = None;
    let mut current_block_name: Option<String> = None;
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            pending_name = block_header_names.get(&obj.handle.0).cloned();
            continue;
        }
        if matches_type_name(header.type_code, 0x04, "BLOCK", dynamic_types) {
            let recovered_name =
                recover_block_name_from_block_record(&record, decoder.version(), &header);
            let mut block_name = pending_name.clone();
            if !is_r2010_plus && recovered_name.as_ref().is_some_and(|name| !name.is_empty()) {
                block_name = recovered_name.clone();
            }
            if block_name.is_none() || block_name.as_ref().is_some_and(|name| name.is_empty()) {
                block_name = recovered_name;
            }
            if let Some(name) = block_name {
                if !name.is_empty() {
                    current_block_name = Some(name.clone());
                    block_aliases
                        .entry(obj.handle.0)
                        .or_insert_with(|| name.clone());
                    let prefer_prefixed = matches!(
                        decoder.version(),
                        version::DwgVersion::R2010
                            | version::DwgVersion::R2013
                            | version::DwgVersion::R2018
                    );
                    let decoded_handle = if prefer_prefixed {
                        let mut prefixed_reader = record.bit_reader();
                        if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok()
                        {
                            decode_block_record_handle(
                                &mut prefixed_reader,
                                decoder.version(),
                                &header,
                                obj.handle.0,
                            )
                        } else {
                            let mut reader = record.bit_reader();
                            decode_block_record_handle(
                                &mut reader,
                                decoder.version(),
                                &header,
                                obj.handle.0,
                            )
                        }
                    } else {
                        let mut reader = record.bit_reader();
                        decode_block_record_handle(
                            &mut reader,
                            decoder.version(),
                            &header,
                            obj.handle.0,
                        )
                    };
                    if let Ok(decoded_handle) = decoded_handle {
                        block_aliases.entry(decoded_handle).or_insert(name);
                    }
                } else {
                    current_block_name = None;
                }
            } else {
                current_block_name = None;
            }
            continue;
        }
        if matches_type_name(header.type_code, 0x05, "ENDBLK", dynamic_types) {
            let mut endblk_name = current_block_name.clone();
            if endblk_name.is_none() || endblk_name.as_ref().is_some_and(|name| name.is_empty()) {
                endblk_name = pending_name.clone();
            }
            let Some(name) = endblk_name else {
                pending_name = None;
                current_block_name = None;
                continue;
            };
            if !name.is_empty() {
                endblk_aliases
                    .entry(obj.handle.0)
                    .or_insert_with(|| name.clone());
                let prefer_prefixed = matches!(
                    decoder.version(),
                    version::DwgVersion::R2010
                        | version::DwgVersion::R2013
                        | version::DwgVersion::R2018
                );
                let decoded_handle = if prefer_prefixed {
                    let mut prefixed_reader = record.bit_reader();
                    if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                        decode_block_record_handle(
                            &mut prefixed_reader,
                            decoder.version(),
                            &header,
                            obj.handle.0,
                        )
                    } else {
                        let mut reader = record.bit_reader();
                        decode_block_record_handle(
                            &mut reader,
                            decoder.version(),
                            &header,
                            obj.handle.0,
                        )
                    }
                } else {
                    let mut reader = record.bit_reader();
                    decode_block_record_handle(
                        &mut reader,
                        decoder.version(),
                        &header,
                        obj.handle.0,
                    )
                };
                if let Ok(decoded_handle) = decoded_handle {
                    endblk_aliases.entry(decoded_handle).or_insert(name);
                }
            }
            pending_name = None;
            current_block_name = None;
        }
    }
    Ok((block_aliases, endblk_aliases))
}

fn collect_block_header_stream_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
    object_type_codes: &HashMap<u64, u16>,
    known_layer_handles: &HashSet<u64>,
) -> PyResult<HashMap<u64, String>> {
    if !matches!(
        decoder.version(),
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return Ok(HashMap::new());
    }
    let mut aliases: HashMap<u64, String> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            continue;
        }
        let Some(name) = block_header_names.get(&obj.handle.0).cloned() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }

        let expected_end_bit = resolve_r2010_object_data_end_bit(&header).ok();
        let mut end_bit_candidates = resolve_r2010_object_data_end_bit_candidates(&header);
        if end_bit_candidates.is_empty() {
            if let Some(expected) = expected_end_bit {
                end_bit_candidates.push(expected);
            }
        }
        let mut best_known: Option<(u64, u64)> = None;
        let mut best_unknown: Option<(u64, u64)> = None;
        for end_bit in end_bit_candidates {
            let mut reader = record.bit_reader();
            if skip_object_type_prefix(&mut reader, decoder.version()).is_err() {
                continue;
            }
            reader.set_bit_pos(end_bit);
            for index in 0..96u64 {
                let Ok(candidate) =
                    entities::common::read_handle_reference(&mut reader, obj.handle.0)
                else {
                    break;
                };
                if candidate == 0 || candidate == obj.handle.0 {
                    continue;
                }
                if known_layer_handles.contains(&candidate) {
                    continue;
                }

                let mut score = index.saturating_mul(16);
                if let Some(expected) = expected_end_bit {
                    score = score.saturating_add(expected.abs_diff(end_bit) as u64);
                }
                if let Some(type_code) = object_type_codes.get(&candidate) {
                    score = score.saturating_add(match *type_code {
                        0x04 => 0,   // BLOCK entity
                        0x05 => 40,  // ENDBLK
                        0x31 => 120, // BLOCK_HEADER itself
                        0x30 => 160, // BLOCK_CONTROL
                        0x33 => 240, // LAYER
                        _ => 80,
                    });
                } else {
                    // Unknown handle ids may still point to valid block-related records.
                    score = score.saturating_add(48);
                }
                if block_header_names.contains_key(&candidate) {
                    score = score.saturating_add(120);
                }

                let known_like = object_type_codes.contains_key(&candidate)
                    || block_header_names.contains_key(&candidate);
                if known_like {
                    match best_known {
                        Some((best_score, _)) if best_score <= score => {}
                        _ => best_known = Some((score, candidate)),
                    }
                } else {
                    match best_unknown {
                        Some((best_score, _)) if best_score <= score => {}
                        _ => best_unknown = Some((score, candidate)),
                    }
                }
            }
        }

        if let Some((_, alias_handle)) = best_known {
            aliases.entry(alias_handle).or_insert_with(|| name.clone());
        }
        if let Some((unknown_score, alias_handle)) = best_unknown {
            let allow_unknown = match best_known {
                Some((known_score, _)) => unknown_score <= known_score.saturating_add(128),
                None => true,
            };
            if allow_unknown {
                aliases.entry(alias_handle).or_insert(name);
            }
        }
    }
    Ok(aliases)
}

fn collect_block_header_targeted_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
    targets: &HashSet<u64>,
) -> PyResult<HashMap<u64, String>> {
    if targets.is_empty()
        || !matches!(
            decoder.version(),
            version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
        )
    {
        return Ok(HashMap::new());
    }
    let mut best: HashMap<u64, (u64, String)> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            continue;
        }
        let mut block_name = block_header_names.get(&obj.handle.0).cloned();
        if block_name.is_none() {
            let mut prefixed_reader = record.bit_reader();
            if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                if let Ok((decoded_handle, _)) = decode_block_header_name_record(
                    &mut prefixed_reader,
                    decoder.version(),
                    obj.handle.0,
                    Some(&header),
                ) {
                    block_name = block_header_names.get(&decoded_handle).cloned();
                }
            }
        }
        let Some(block_name) = block_name else {
            continue;
        };
        if block_name.is_empty() {
            continue;
        }

        let expected_end_bit = resolve_r2010_object_data_end_bit(&header).ok();
        let mut end_bit_candidates = resolve_r2010_object_data_end_bit_candidates(&header);
        if let Some(expected) = expected_end_bit {
            for delta in (-64i32..=64).step_by(8) {
                let candidate_i64 = i64::from(expected) + i64::from(delta);
                if candidate_i64 < 0 {
                    continue;
                }
                if let Ok(candidate) = u32::try_from(candidate_i64) {
                    end_bit_candidates.push(candidate);
                }
            }
        }
        end_bit_candidates.sort_unstable();
        end_bit_candidates.dedup();

        let mut base_handles = vec![obj.handle.0];
        let mut prefixed_reader = record.bit_reader();
        if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
            if let Ok(record_handle) = prefixed_reader.read_h() {
                if record_handle.value != 0 {
                    base_handles.push(record_handle.value);
                }
            }
        }
        base_handles.sort_unstable();
        base_handles.dedup();

        for end_bit in end_bit_candidates {
            for base_handle in base_handles.iter().copied() {
                for chained_base in [false, true] {
                    let mut reader = record.bit_reader();
                    if skip_object_type_prefix(&mut reader, decoder.version()).is_err() {
                        continue;
                    }
                    reader.set_bit_pos(end_bit);
                    let mut prev_handle = base_handle;
                    for index in 0..256u64 {
                        let candidate = if chained_base {
                            match read_handle_reference_chained(&mut reader, &mut prev_handle) {
                                Ok(value) => value,
                                Err(_) => break,
                            }
                        } else {
                            match entities::common::read_handle_reference(&mut reader, base_handle)
                            {
                                Ok(value) => value,
                                Err(_) => break,
                            }
                        };
                        if candidate == 0 {
                            continue;
                        }
                        if !targets.contains(&candidate) {
                            continue;
                        }
                        let mut score = index.saturating_mul(8);
                        if let Some(expected) = expected_end_bit {
                            score = score.saturating_add(expected.abs_diff(end_bit) as u64);
                        }
                        if base_handle != obj.handle.0 {
                            score = score.saturating_add(12);
                        }
                        if chained_base {
                            score = score.saturating_add(8);
                        }
                        match best.get(&candidate) {
                            Some((best_score, _)) if *best_score <= score => {}
                            _ => {
                                best.insert(candidate, (score, block_name.clone()));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(best
        .into_iter()
        .map(|(handle, (_score, name))| (handle, name))
        .collect())
}

fn collect_insert_block_handle_candidates_r2010_plus(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
    parsed_block_handle: Option<u64>,
    known_block_handles: Option<&HashSet<u64>>,
    limit: usize,
) -> Vec<u64> {
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return parsed_block_handle
            .filter(|handle| *handle != 0)
            .into_iter()
            .collect();
    }

    let parsed_block_handle = parsed_block_handle.filter(|handle| *handle != 0);
    let mut candidate_scores: HashMap<u64, u64> = HashMap::new();

    let mut base_handles = vec![object_handle];
    if object_handle > 1 {
        base_handles.push(object_handle - 1);
    }
    base_handles.push(object_handle.saturating_add(1));
    if object_handle > 2 {
        base_handles.push(object_handle - 2);
    }
    base_handles.push(object_handle.saturating_add(2));

    let mut base_reader = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader, version).is_ok() {
        if let Ok(record_handle) = base_reader.read_h() {
            if record_handle.value != 0 {
                base_handles.push(record_handle.value);
                if record_handle.value > 1 {
                    base_handles.push(record_handle.value - 1);
                }
                base_handles.push(record_handle.value.saturating_add(1));
            }
        }
    }
    let mut base_reader_with_size = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader_with_size, version).is_ok()
        && base_reader_with_size.read_rl(Endian::Little).is_ok()
    {
        if let Ok(record_handle) = base_reader_with_size.read_h() {
            if record_handle.value != 0 {
                base_handles.push(record_handle.value);
                if record_handle.value > 1 {
                    base_handles.push(record_handle.value - 1);
                }
                base_handles.push(record_handle.value.saturating_add(1));
            }
        }
    }
    base_handles.sort_unstable();
    base_handles.dedup();

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
    if let Some(expected) = resolve_r2010_object_data_end_bit(api_header).ok() {
        for delta in -48i32..=48i32 {
            let candidate_i64 = i64::from(expected) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            if let Ok(candidate) = u32::try_from(candidate_i64) {
                expanded_end_bits.push(candidate);
            }
        }
    }
    expanded_end_bits.sort_unstable();
    expanded_end_bits.dedup();

    let expected_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
    let is_known = |candidate: u64| -> bool {
        known_block_handles
            .map(|handles| handles.contains(&candidate))
            .unwrap_or(false)
    };
    for object_data_end_bit in expanded_end_bits.iter().copied() {
        for base_handle in base_handles.iter().copied() {
            let Some(candidate) = parse_insert_block_header_handle_from_common_header(
                record,
                version,
                object_data_end_bit,
                base_handle,
            ) else {
                continue;
            };
            if candidate == 0 || candidate == object_handle {
                continue;
            }
            let mut score = expected_end_bit
                .map(|expected| expected.abs_diff(object_data_end_bit) as u64)
                .unwrap_or(0)
                .saturating_mul(4);
            if base_handle != object_handle {
                score = score.saturating_add(32);
            }
            if Some(candidate) == parsed_block_handle {
                score = score.saturating_sub(16);
            }
            if is_known(candidate) {
                score = score.saturating_sub(80);
            }
            match candidate_scores.get(&candidate).copied() {
                Some(best_score) if best_score <= score => {}
                _ => {
                    candidate_scores.insert(candidate, score);
                }
            }
        }
    }

    for object_data_end_bit in expanded_end_bits {
        for base_handle in base_handles.iter().copied() {
            for chained_base in [false, true] {
                let mut reader = record.bit_reader();
                if skip_object_type_prefix(&mut reader, version).is_err() {
                    continue;
                }
                reader.set_bit_pos(object_data_end_bit);
                let mut prev_handle = base_handle;
                for index in 0..64u64 {
                    let candidate = if chained_base {
                        match read_handle_reference_chained(&mut reader, &mut prev_handle) {
                            Ok(value) => value,
                            Err(_) => break,
                        }
                    } else {
                        match entities::common::read_handle_reference(&mut reader, base_handle) {
                            Ok(value) => value,
                            Err(_) => break,
                        }
                    };
                    if candidate == 0 || candidate == object_handle {
                        continue;
                    }
                    let mut score = index.saturating_mul(64);
                    if let Some(expected) = expected_end_bit {
                        score = score.saturating_add(expected.abs_diff(object_data_end_bit) as u64);
                    }
                    if base_handle != object_handle {
                        score = score.saturating_add(40);
                    }
                    if !chained_base {
                        score = score.saturating_add(8);
                    } else {
                        score = score.saturating_add(24);
                    }
                    if Some(candidate) == parsed_block_handle {
                        score = score.saturating_sub(12);
                    }
                    if is_known(candidate) {
                        score = score.saturating_sub(72);
                    }
                    match candidate_scores.get(&candidate).copied() {
                        Some(best_score) if best_score <= score => {}
                        _ => {
                            candidate_scores.insert(candidate, score);
                        }
                    }
                }
            }
        }
    }

    let mut scored: Vec<(u64, u64)> = candidate_scores.into_iter().collect();
    scored.sort_by_key(|(candidate, score)| (*score, *candidate));
    if let Some(parsed) = parsed_block_handle {
        if !scored.iter().any(|(candidate, _)| *candidate == parsed) {
            scored.insert(0, (parsed, 0));
        }
    }
    scored
        .into_iter()
        .take(limit.max(1))
        .map(|(candidate, _score)| candidate)
        .collect()
}

fn recover_insert_block_header_handle_r2010_plus(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
    parsed_block_handle: Option<u64>,
    known_block_handles: &HashSet<u64>,
    named_block_handles: &HashSet<u64>,
) -> Option<u64> {
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return parsed_block_handle;
    }
    if known_block_handles.is_empty() {
        return parsed_block_handle;
    }
    let parsed_block_handle = parsed_block_handle.filter(|handle| *handle != 0);
    let mut best: Option<(u64, u64)> = None;
    if let Some(handle) = parsed_block_handle {
        if known_block_handles.contains(&handle) {
            if named_block_handles.is_empty() || named_block_handles.contains(&handle) {
                return Some(handle);
            }
            best = Some((10, handle));
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
            if record_handle.value != 0 {
                base_handles.push(record_handle.value);
                if record_handle.value > 1 {
                    base_handles.push(record_handle.value - 1);
                }
                base_handles.push(record_handle.value.saturating_add(1));
            }
        }
    }
    let mut base_reader_with_size = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader_with_size, version).is_ok()
        && base_reader_with_size.read_rl(Endian::Little).is_ok()
    {
        if let Ok(record_handle) = base_reader_with_size.read_h() {
            if record_handle.value != 0 {
                base_handles.push(record_handle.value);
                if record_handle.value > 1 {
                    base_handles.push(record_handle.value - 1);
                }
                base_handles.push(record_handle.value.saturating_add(1));
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

    let expected_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
    for object_data_end_bit in expanded_end_bits.iter().copied() {
        for base_handle in ordered_base_handles.iter().copied() {
            let Some(candidate) = parse_insert_block_header_handle_from_common_header(
                record,
                version,
                object_data_end_bit,
                base_handle,
            ) else {
                continue;
            };
            let mut score = expected_end_bit
                .map(|expected| expected.abs_diff(object_data_end_bit) as u64)
                .unwrap_or(0)
                .saturating_mul(4);
            if base_handle != object_handle {
                score = score.saturating_add(32);
            }
            if named_block_handles.contains(&candidate) {
                score = score.saturating_sub(24);
            }
            if Some(candidate) == parsed_block_handle {
                score = score.saturating_sub(16);
            }
            if !known_block_handles.contains(&candidate) {
                continue;
            }
            match best {
                Some((best_score, _)) if best_score <= score => {}
                _ => best = Some((score, candidate)),
            }
        }
    }

    for object_data_end_bit in expanded_end_bits {
        for base_handle in ordered_base_handles.iter().copied() {
            for chained_base in [false, true] {
                let mut reader = record.bit_reader();
                if skip_object_type_prefix(&mut reader, version).is_err() {
                    continue;
                }
                reader.set_bit_pos(object_data_end_bit);
                let mut prev_handle = base_handle;
                for index in 0..64u64 {
                    let candidate = if chained_base {
                        match read_handle_reference_chained(&mut reader, &mut prev_handle) {
                            Ok(value) => value,
                            Err(_) => break,
                        }
                    } else {
                        match entities::common::read_handle_reference(&mut reader, base_handle) {
                            Ok(value) => value,
                            Err(_) => break,
                        }
                    };
                    let mut score = index.saturating_mul(64);
                    if let Some(expected) = expected_end_bit {
                        score = score.saturating_add(expected.abs_diff(object_data_end_bit) as u64);
                    }
                    if base_handle != object_handle {
                        score = score.saturating_add(40);
                    }
                    if !chained_base {
                        score = score.saturating_add(8);
                    } else {
                        score = score.saturating_add(24);
                    }
                    if named_block_handles.contains(&candidate) {
                        score = score.saturating_sub(20);
                    }
                    if Some(candidate) == parsed_block_handle {
                        score = score.saturating_sub(12);
                    }
                    if !known_block_handles.contains(&candidate) {
                        continue;
                    }
                    match best {
                        Some((best_score, _)) if best_score <= score => {}
                        _ => best = Some((score, candidate)),
                    }
                }
            }
        }
    }
    best.map(|(_, handle)| handle).or(parsed_block_handle)
}

fn parse_insert_block_header_handle_from_common_header(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    object_data_end_bit: u32,
    base_handle: u64,
) -> Option<u64> {
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let mut header = match version {
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
    header.handle = base_handle;
    reader.set_bit_pos(header.obj_size);
    entities::common::parse_common_entity_handles(&mut reader, &header).ok()?;
    entities::common::read_handle_reference(&mut reader, header.handle).ok()
}

fn decode_block_header_record_handle(
    reader: &mut BitReader<'_>,
    expected_handle: u64,
) -> crate::core::result::Result<u64> {
    let _obj_size_bits = reader.read_rl(Endian::Little)?;
    let record_handle = reader.read_h()?.value;
    Ok(if record_handle != 0 {
        record_handle
    } else {
        expected_handle
    })
}

fn recover_block_header_name_from_record(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
) -> Option<String> {
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let start_bit = reader.tell_bits() as u32;
    let total_bits = api_header.data_size.saturating_mul(8);
    if start_bit >= total_bits {
        return None;
    }

    let mut end_bit_candidates: Vec<u32> = Vec::new();
    end_bit_candidates.extend(resolve_r2010_object_data_end_bit_candidates(api_header));
    let mut size_reader = reader.clone();
    if let Ok(obj_size_bits) = size_reader.read_rl(Endian::Little) {
        for delta in (-64i32..=64).step_by(8) {
            let candidate_i64 = i64::from(obj_size_bits) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            end_bit_candidates.push(candidate);
        }
    }
    end_bit_candidates.push(total_bits);
    end_bit_candidates.retain(|candidate| *candidate > start_bit && *candidate <= total_bits);
    end_bit_candidates.sort_unstable();
    end_bit_candidates.dedup();
    if end_bit_candidates.is_empty() {
        return None;
    }

    let canonical_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
    let mut best: Option<(u64, String)> = None;
    for end_bit in end_bit_candidates {
        let Some(name) = scan_block_header_name_in_string_stream(&reader, start_bit, end_bit)
        else {
            continue;
        };
        let mut score = score_block_name_candidate(&name);
        if let Some(canonical_end) = canonical_end_bit {
            score = score.saturating_add(canonical_end.abs_diff(end_bit) as u64);
        }
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, name)),
        }
    }
    best.map(|(_, name)| name)
}

fn recover_block_name_from_block_record(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
) -> Option<String> {
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let start_bit = reader.tell_bits() as u32;
    let total_bits = api_header.data_size.saturating_mul(8);
    if start_bit >= total_bits {
        return None;
    }

    let mut best: Option<(u64, String)> = None;
    let mut consider = |name: String, score_bias: u64| {
        if !is_plausible_block_name(&name) {
            return;
        }
        let score = score_block_name_candidate(&name).saturating_add(score_bias);
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, name)),
        }
    };

    if matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let mut end_bit_candidates: Vec<u32> = Vec::new();
        end_bit_candidates.extend(resolve_r2010_object_data_end_bit_candidates(api_header));
        end_bit_candidates.push(total_bits);
        end_bit_candidates.retain(|candidate| *candidate > start_bit && *candidate <= total_bits);
        end_bit_candidates.sort_unstable();
        end_bit_candidates.dedup();

        let canonical_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
        let base_reader = reader.clone();
        for end_bit in end_bit_candidates {
            for (stream_start_bit, stream_end_bit) in
                resolve_r2010_string_stream_ranges(&base_reader, end_bit)
            {
                if let Some(name) = scan_block_header_name_in_string_stream(
                    &base_reader,
                    stream_start_bit,
                    stream_end_bit,
                ) {
                    let end_bias = canonical_end_bit
                        .map(|canonical| canonical.abs_diff(end_bit) as u64)
                        .unwrap_or(0);
                    consider(name, end_bias);
                }
            }
        }
    }

    if let Some(name) = scan_block_header_name_in_string_stream(&reader, start_bit, total_bits) {
        consider(name, 32);
    }

    best.map(|(_, name)| name)
}

fn decode_block_header_name_record(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    expected_handle: u64,
    api_header: Option<&ApiObjectHeader>,
) -> crate::core::result::Result<(u64, String)> {
    let obj_size_bits = reader.read_rl(Endian::Little)?;
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

    let entry_name = if matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let start_bit = reader.tell_bits() as u32;
        let total_bits = api_header
            .map(|header| header.data_size.saturating_mul(8))
            .unwrap_or(u32::MAX);
        let mut end_bit_candidates: Vec<u32> = Vec::new();
        if obj_size_bits > 0 {
            for delta in (-64i32..=64).step_by(8) {
                let candidate_i64 = i64::from(obj_size_bits) + i64::from(delta);
                if candidate_i64 < 0 {
                    continue;
                }
                let Ok(candidate) = u32::try_from(candidate_i64) else {
                    continue;
                };
                end_bit_candidates.push(candidate);
            }
        }
        if let Some(header) = api_header {
            for base in resolve_r2010_object_data_end_bit_candidates(header) {
                for delta in (-64i32..=64).step_by(8) {
                    let candidate_i64 = i64::from(base) + i64::from(delta);
                    if candidate_i64 < 0 {
                        continue;
                    }
                    let Ok(candidate) = u32::try_from(candidate_i64) else {
                        continue;
                    };
                    end_bit_candidates.push(candidate);
                }
            }
        }
        end_bit_candidates.retain(|candidate| *candidate > start_bit && *candidate <= total_bits);
        end_bit_candidates.sort_unstable();
        end_bit_candidates.dedup();
        if end_bit_candidates.is_empty() {
            if obj_size_bits > start_bit {
                end_bit_candidates.push(obj_size_bits.min(total_bits));
            } else if total_bits > start_bit {
                end_bit_candidates.push(total_bits);
            }
        }

        let canonical_end_bit =
            api_header.and_then(|header| resolve_r2010_object_data_end_bit(header).ok());
        let base_reader = reader.clone();
        let mut best_name: Option<(u64, String)> = None;
        for end_bit in end_bit_candidates {
            let mut candidate_name = String::new();
            let mut stream_best: Option<(u64, String)> = None;
            for (stream_start_bit, stream_end_bit) in
                resolve_r2010_string_stream_ranges(&base_reader, end_bit)
            {
                let mut stream_reader = base_reader.clone();
                stream_reader.set_bit_pos(stream_start_bit);
                if let Ok(name) = read_tu(&mut stream_reader) {
                    if stream_reader.tell_bits() <= stream_end_bit as u64
                        && is_plausible_block_name(&name)
                    {
                        let score = score_block_name_candidate(&name);
                        match &stream_best {
                            Some((best_score, _)) if score >= *best_score => {}
                            _ => stream_best = Some((score, name)),
                        }
                    }
                }
                if let Some(name) = scan_block_header_name_in_string_stream(
                    &base_reader,
                    stream_start_bit,
                    stream_end_bit,
                ) {
                    let score = score_block_name_candidate(&name);
                    match &stream_best {
                        Some((best_score, _)) if score >= *best_score => {}
                        _ => stream_best = Some((score, name)),
                    }
                }
            }
            if let Some((_, name)) = stream_best {
                candidate_name = name;
            }

            if candidate_name.is_empty() {
                let mut parsed_reader = reader.clone();
                if parse_block_header_nonstring_data_r2010_plus(&mut parsed_reader).is_ok() {
                    if let Ok(name) = read_tu(&mut parsed_reader) {
                        if parsed_reader.tell_bits() <= end_bit as u64
                            && is_plausible_block_name(&name)
                        {
                            candidate_name = name;
                        }
                    }
                }
            }
            if candidate_name.is_empty() {
                if let Some(name) =
                    scan_block_header_name_in_string_stream(&base_reader, start_bit, end_bit)
                {
                    candidate_name = name;
                }
            }
            if candidate_name.is_empty() {
                continue;
            }
            let mut score = score_block_name_candidate(&candidate_name);
            if let Some(canonical_end) = canonical_end_bit {
                score = score.saturating_add(canonical_end.abs_diff(end_bit) as u64);
            }
            match &best_name {
                Some((best_score, _)) if score >= *best_score => {}
                _ => best_name = Some((score, candidate_name)),
            }
        }
        best_name.map(|(_, name)| name).unwrap_or_default()
    } else {
        reader.read_tv()?
    };

    let handle = if record_handle != 0 {
        record_handle
    } else {
        expected_handle
    };
    Ok((handle, entry_name))
}

fn parse_block_header_nonstring_data_r2010_plus(
    reader: &mut BitReader<'_>,
) -> crate::core::result::Result<()> {
    let _flag_64 = reader.read_b()?;
    let _xref_index_plus1 = reader.read_bs()?;
    let _xdep = reader.read_b()?;
    let _anonymous = reader.read_b()?;
    let _has_atts = reader.read_b()?;
    let _blk_is_xref = reader.read_b()?;
    let _xref_overlaid = reader.read_b()?;
    let _loaded_bit = reader.read_b()?;
    let _owned_obj_count = reader.read_bl()?;
    let _base_pt = reader.read_3bd()?;
    loop {
        let marker = reader.read_rc()?;
        if marker == 0 {
            break;
        }
    }
    let preview_data_size = reader.read_bl()? as usize;
    let _preview_data = reader.read_rcs(preview_data_size)?;
    let _insert_units = reader.read_bs()?;
    let _explodable = reader.read_b()?;
    let _block_scaling = reader.read_rc()?;
    Ok(())
}

fn is_plausible_block_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 {
        return false;
    }
    let mut has_meaningful = false;
    for ch in name.chars() {
        if ch.is_control() {
            return false;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '*' | '-') {
            has_meaningful = true;
            continue;
        }
        if ch == ' ' {
            continue;
        }
        if !ch.is_ascii_graphic() {
            return false;
        }
    }
    has_meaningful
}

fn score_block_name_candidate(name: &str) -> u64 {
    let mut score = 0u64;
    if name.len() <= 2 {
        score = score.saturating_add(24);
    } else if name.len() <= 4 {
        score = score.saturating_add(8);
    }
    if name.len() > 96 {
        score = score.saturating_add((name.len() - 96) as u64);
    }
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '*' | '-' | '.') {
            continue;
        }
        if ch == ' ' {
            score = score.saturating_add(120);
        } else if ch.is_ascii_graphic() {
            score = score.saturating_add(240);
        } else {
            score = score.saturating_add(500);
        }
    }
    if name.starts_with('*') {
        score = score.saturating_add(8);
    }
    if name.chars().all(|ch| ch.is_ascii_digit()) {
        score = score.saturating_add(64);
    }
    score
}

fn scan_block_header_name_in_string_stream(
    base_reader: &BitReader<'_>,
    start_bit: u32,
    end_bit: u32,
) -> Option<String> {
    if start_bit >= end_bit {
        return None;
    }
    // String stream alignment is byte-based; scanning the full range is cheap
    // and more robust than relying on a fixed tail window.
    let scan_start = start_bit;
    let mut best: Option<(u64, String)> = None;
    let mut bit = scan_start;
    let mut tried = 0u32;
    let max_tries = end_bit
        .saturating_sub(scan_start)
        .saturating_div(8)
        .saturating_add(2)
        .min(65_536);
    while bit + 16 <= end_bit && tried < max_tries {
        let mut candidate_reader = base_reader.clone();
        candidate_reader.set_bit_pos(bit);
        let Ok(name) = read_tu(&mut candidate_reader) else {
            bit = bit.saturating_add(8);
            tried = tried.saturating_add(1);
            continue;
        };
        if candidate_reader.tell_bits() > end_bit as u64 || !is_plausible_block_name(&name) {
            bit = bit.saturating_add(8);
            tried = tried.saturating_add(1);
            continue;
        }
        let trailing_gap_bits = end_bit as u64 - candidate_reader.tell_bits();
        let mut score = score_block_name_candidate(&name);
        score = score.saturating_add(trailing_gap_bits / 128);
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, name)),
        }
        bit = bit.saturating_add(8);
        tried = tried.saturating_add(1);
    }
    best.map(|(_, name)| name)
}
