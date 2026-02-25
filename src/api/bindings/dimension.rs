#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_linear_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x15,
        "DIM_LINEAR",
        true,
        decode_dim_linear_for_version,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_ordinate_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x14,
        "DIM_ORDINATE",
        true,
        decode_dim_linear_for_version,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_diameter_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x1A,
        "DIM_DIAMETER",
        true,
        decode_dim_diameter_for_version,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_aligned_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x16,
        "DIM_ALIGNED",
        true,
        decode_dim_linear_for_version,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_ang3pt_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x17,
        "DIM_ANG3PT",
        true,
        decode_dim_linear_for_version,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_ang2ln_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x18,
        "DIM_ANG2LN",
        true,
        decode_dim_linear_for_version,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_radius_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x19,
        "DIM_RADIUS",
        true,
        decode_dim_radius_for_version,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dimension_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<DimTypedEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let insert_name_state =
        prepare_insert_name_resolution_state(&decoder, &dynamic_types, &index, best_effort)?;
    decode_dimension_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &insert_name_state,
        limit,
    )
}

fn decode_dimension_entities_with_state(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    insert_name_state: &InsertNameResolutionState,
    limit: Option<usize>,
) -> PyResult<Vec<DimTypedEntityRow>> {
    let mut result: Vec<DimTypedEntityRow> = Vec::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };

        if let Some((dimtype, row)) = decode_dimension_typed_row(
            &record,
            &header,
            obj.handle.0,
            decoder.version(),
            dynamic_types,
            best_effort,
            insert_name_state,
        )? {
            result.push((dimtype.to_string(), row));
            if let Some(limit) = limit {
                if result.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(result)
}

fn decode_dimension_typed_row(
    record: &objects::ObjectRecord<'_>,
    header: &ApiObjectHeader,
    object_handle: u64,
    version: &version::DwgVersion,
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
    insert_name_state: &InsertNameResolutionState,
) -> PyResult<Option<(&'static str, DimEntityRow)>> {
    for spec in DIM_DECODE_SPECS.iter() {
        if !matches_type_name(
            header.type_code,
            spec.type_code,
            spec.type_name,
            dynamic_types,
        ) {
            continue;
        }

        let mut entity = match decode_dim_linear_like_entity_with_prefix_fallback(
            record,
            version,
            header,
            object_handle,
            true,
            spec.decode_entity,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => return Ok(None),
            Err(err) => return Err(to_py_err(err)),
        };
        entity.common.anonymous_block_handle = recover_dimension_anonymous_block_handle_r2010_plus(
            record,
            version,
            header,
            object_handle,
            entity.common.anonymous_block_handle,
            &insert_name_state.known_block_handles,
            &insert_name_state.named_block_handles,
            &insert_name_state.block_header_names,
        );
        return Ok(Some((
            spec.dimtype,
            dim_entity_row_from_linear_like(&entity),
        )));
    }

    Ok(None)
}

fn build_ultra_minimal_dim_linear_entity(object_handle: u64) -> entities::DimLinearEntity {
    let common = entities::DimensionCommonData {
        handle: object_handle,
        color_index: None,
        true_color: None,
        layer_handle: 0,
        extrusion: (0.0, 0.0, 1.0),
        text_midpoint: (0.0, 0.0, 0.0),
        elevation: 0.0,
        dim_flags: 0,
        user_text: String::new(),
        text_rotation: 0.0,
        horizontal_direction: 0.0,
        insert_scale: (1.0, 1.0, 1.0),
        insert_rotation: 0.0,
        attachment_point: None,
        line_spacing_style: None,
        line_spacing_factor: None,
        actual_measurement: None,
        insert_point: None,
        dimstyle_handle: None,
        anonymous_block_handle: None,
    };
    entities::DimLinearEntity {
        common,
        point13: (0.0, 0.0, 0.0),
        point14: (0.0, 0.0, 0.0),
        point10: (0.0, 0.0, 0.0),
        ext_line_rotation: 0.0,
        dim_rotation: 0.0,
    }
}

fn decode_dim_linear_like_entity_minimal_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::DimLinearEntity> {
    let start = reader.get_pos();
    let mut parsed_header = match version {
        version::DwgVersion::R2010 => parse_dim_common_header_r2010_plus_with_candidates(
            reader,
            header,
            entities::common::parse_common_entity_header_r2010,
        ),
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            parse_dim_common_header_r2010_plus_with_candidates(
                reader,
                header,
                entities::common::parse_common_entity_header_r2013,
            )
        }
        version::DwgVersion::R2007 => {
            entities::common::parse_common_entity_header_r2007(reader).ok()
        }
        version::DwgVersion::R14 => entities::common::parse_common_entity_header_r14(reader).ok(),
        _ => entities::common::parse_common_entity_header(reader).ok(),
    };

    if parsed_header.is_none() {
        for parser in [
            entities::common::parse_common_entity_header_r2007
                as fn(
                    &mut BitReader<'_>,
                )
                    -> crate::core::result::Result<entities::common::CommonEntityHeader>,
            entities::common::parse_common_entity_header,
            entities::common::parse_common_entity_header_r14,
        ] {
            reader.set_pos(start.0, start.1);
            if let Ok(candidate) = parser(reader) {
                parsed_header = Some(candidate);
                break;
            }
        }
    }

    let mut common_header = parsed_header.ok_or_else(|| {
        DwgError::new(
            ErrorKind::Decode,
            "failed to minimally decode dimension common header",
        )
    })?;
    if matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        common_header.handle = object_handle;
    }

    reader.set_bit_pos(common_header.obj_size);
    let read_optional_handle = |reader: &mut BitReader<'_>| -> Option<u64> {
        let pos = reader.get_pos();
        match entities::common::read_handle_reference(reader, common_header.handle) {
            Ok(handle) => Some(handle),
            Err(_) => {
                reader.set_pos(pos.0, pos.1);
                None
            }
        }
    };
    let dimstyle_handle = read_optional_handle(reader);
    let anonymous_block_handle = read_optional_handle(reader);
    let layer_handle =
        entities::common::parse_common_entity_layer_handle(reader, &common_header).unwrap_or(0);

    let common = entities::DimensionCommonData {
        handle: common_header.handle,
        color_index: common_header.color.index,
        true_color: common_header.color.true_color,
        layer_handle,
        extrusion: (0.0, 0.0, 1.0),
        text_midpoint: (0.0, 0.0, 0.0),
        elevation: 0.0,
        dim_flags: 0,
        user_text: String::new(),
        text_rotation: 0.0,
        horizontal_direction: 0.0,
        insert_scale: (1.0, 1.0, 1.0),
        insert_rotation: 0.0,
        attachment_point: None,
        line_spacing_style: None,
        line_spacing_factor: None,
        actual_measurement: None,
        insert_point: None,
        dimstyle_handle,
        anonymous_block_handle,
    };

    Ok(entities::DimLinearEntity {
        common,
        point13: (0.0, 0.0, 0.0),
        point14: (0.0, 0.0, 0.0),
        point10: (0.0, 0.0, 0.0),
        ext_line_rotation: 0.0,
        dim_rotation: 0.0,
    })
}

fn dim_linear_entity_plausibility_score(entity: &entities::DimLinearEntity) -> u64 {
    let mut score = 0u64;
    let common = &entity.common;

    for point in [
        entity.point10,
        entity.point13,
        entity.point14,
        common.text_midpoint,
    ] {
        score = score.saturating_add(point_plausibility_score(point));
    }
    if let Some(insert_point) = common.insert_point {
        score = score.saturating_add(point_plausibility_score(insert_point));
    }
    score = score.saturating_add(point_plausibility_score(common.extrusion));
    score = score.saturating_add(point_plausibility_score(common.insert_scale));

    if common.extrusion.0 == 0.0 && common.extrusion.1 == 0.0 && common.extrusion.2 == 0.0 {
        score = score.saturating_add(50_000);
    }

    for angle in [
        common.text_rotation,
        common.horizontal_direction,
        entity.ext_line_rotation,
        entity.dim_rotation,
        common.insert_rotation,
    ] {
        if !angle.is_finite() {
            score = score.saturating_add(100_000);
        } else if angle.abs() > 1.0e6 {
            score = score.saturating_add(10_000);
        } else if angle.abs() > 1.0e4 {
            score = score.saturating_add(100);
        }
    }

    if let Some(actual_measurement) = common.actual_measurement {
        score = score.saturating_add(value_plausibility_score(actual_measurement));
    }
    if let Some(line_spacing_factor) = common.line_spacing_factor {
        score = score.saturating_add(value_plausibility_score(line_spacing_factor));
    }

    if let Some(attachment_point) = common.attachment_point {
        if attachment_point > 9 {
            score = score.saturating_add(10_000);
        }
    }
    if let Some(line_spacing_style) = common.line_spacing_style {
        if line_spacing_style > 2 {
            score = score.saturating_add(10_000);
        }
    }
    if common.dim_flags > 0x7F {
        score = score.saturating_add(1_000);
    }

    score
}

fn dim_entity_row_from_linear_like(entity: &entities::DimLinearEntity) -> DimEntityRow {
    let common = &entity.common;
    (
        common.handle,
        common.user_text.clone(),
        entity.point10,
        entity.point13,
        entity.point14,
        common.text_midpoint,
        common.insert_point,
        (common.extrusion, common.insert_scale),
        (
            common.text_rotation,
            common.horizontal_direction,
            entity.ext_line_rotation,
            entity.dim_rotation,
        ),
        (
            common.dim_flags,
            common.actual_measurement,
            common.attachment_point,
            common.line_spacing_style,
            common.line_spacing_factor,
            common.insert_rotation,
        ),
        (common.dimstyle_handle, common.anonymous_block_handle),
    )
}

fn decode_dim_linear_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::DimLinearEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_linear_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_linear_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_dim_linear_r2007(reader),
        _ => entities::decode_dim_linear(reader),
    }
}

fn decode_dim_radius_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::DimRadiusEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_radius_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_radius_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_dim_radius_r2007(reader),
        _ => entities::decode_dim_radius(reader),
    }
}

fn decode_dim_diameter_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::DimDiameterEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_diameter_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_diameter_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_dim_diameter_r2007(reader),
        _ => entities::decode_dim_diameter(reader),
    }
}

fn recover_dimension_anonymous_block_handle_r2010_plus(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
    parsed_block_handle: Option<u64>,
    known_block_handles: &HashSet<u64>,
    named_block_handles: &HashSet<u64>,
    block_header_names: &HashMap<u64, String>,
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
    base_handles.sort_unstable();
    base_handles.dedup();

    let mut end_bits = resolve_r2010_object_data_end_bit_candidates(api_header);
    if let Ok(canonical) = resolve_r2010_object_data_end_bit(api_header) {
        for delta in (-256i32..=256).step_by(8) {
            let candidate_i64 = i64::from(canonical) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            if let Ok(candidate) = u32::try_from(candidate_i64) {
                end_bits.push(candidate);
            }
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
                    end_bits.push(candidate);
                }
            }
        }
    }
    let total_bits = api_header.data_size.saturating_mul(8);
    end_bits.push(total_bits);
    end_bits.retain(|candidate| *candidate <= total_bits);
    end_bits.sort_unstable();
    end_bits.dedup();

    let expected_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
    let mut best: Option<(u64, u64)> = None;
    if let Some(handle) = parsed_block_handle {
        if known_block_handles.contains(&handle) {
            let mut score = dimension_block_name_penalty(handle, block_header_names);
            if !named_block_handles.is_empty() && named_block_handles.contains(&handle) {
                score = score.saturating_sub(8);
            }
            best = Some((score, handle));
        }
    }
    for end_bit in end_bits.iter().copied() {
        for base_handle in base_handles.iter().copied() {
            let mut reader = record.bit_reader();
            if skip_object_type_prefix(&mut reader, version).is_err() {
                continue;
            }
            let mut common_header = match version {
                version::DwgVersion::R2010 => {
                    match entities::common::parse_common_entity_header_r2010(&mut reader, end_bit) {
                        Ok(header) => header,
                        Err(_) => continue,
                    }
                }
                version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
                    match entities::common::parse_common_entity_header_r2013(&mut reader, end_bit) {
                        Ok(header) => header,
                        Err(_) => continue,
                    }
                }
                _ => continue,
            };
            common_header.handle = base_handle;
            reader.set_bit_pos(common_header.obj_size);
            if entities::common::read_handle_reference(&mut reader, common_header.handle).is_err() {
                continue;
            }
            let block_handle =
                match entities::common::read_handle_reference(&mut reader, common_header.handle) {
                    Ok(handle) => handle,
                    Err(_) => continue,
                };
            if !known_block_handles.contains(&block_handle) {
                continue;
            }
            let mut score = expected_end_bit
                .map(|expected| expected.abs_diff(end_bit) as u64)
                .unwrap_or(0)
                .saturating_mul(4);
            if base_handle != object_handle {
                score = score.saturating_add(24);
            }
            if Some(block_handle) == parsed_block_handle {
                score = score.saturating_sub(16);
            }
            if !named_block_handles.is_empty() && named_block_handles.contains(&block_handle) {
                score = score.saturating_sub(8);
            }
            score = score.saturating_add(
                dimension_block_name_penalty(block_handle, block_header_names),
            );
            match best {
                Some((best_score, _)) if best_score <= score => {}
                _ => best = Some((score, block_handle)),
            }
        }
    }

    let expected_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
    for end_bit in end_bits {
        for base_handle in base_handles.iter().copied() {
            for chained_base in [false, true] {
                let mut reader = record.bit_reader();
                if skip_object_type_prefix(&mut reader, version).is_err() {
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
                        match entities::common::read_handle_reference(&mut reader, base_handle) {
                            Ok(value) => value,
                            Err(_) => break,
                        }
                    };
                    if !known_block_handles.contains(&candidate) {
                        continue;
                    }
                    let mut score = index.saturating_mul(64);
                    if let Some(expected) = expected_end_bit {
                        score = score.saturating_add(expected.abs_diff(end_bit) as u64);
                    }
                    if base_handle != object_handle {
                        score = score.saturating_add(24);
                    }
                    if Some(candidate) == parsed_block_handle {
                        score = score.saturating_sub(12);
                    }
                    if !named_block_handles.is_empty() && named_block_handles.contains(&candidate) {
                        score = score.saturating_sub(8);
                    }
                    score =
                        score.saturating_add(dimension_block_name_penalty(candidate, block_header_names));
                    if !chained_base {
                        score = score.saturating_add(6);
                    } else {
                        score = score.saturating_add(18);
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

fn dimension_block_name_penalty(
    handle: u64,
    block_header_names: &HashMap<u64, String>,
) -> u64 {
    let Some(raw_name) = block_header_names.get(&handle) else {
        return 96;
    };
    let name = raw_name.trim();
    if name.is_empty() {
        return 96;
    }
    let upper = name.to_ascii_uppercase();
    if upper == "*D" {
        // Many drawings contain placeholder duplicate "*D" names; selecting
        // them eagerly collapses distinct anonymous dimension graphics.
        return 1024;
    }
    if let Some(suffix) = upper.strip_prefix("*D") {
        if !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
            return 0;
        }
        return 80;
    }
    if upper.starts_with('*') {
        return 160;
    }
    240
}
