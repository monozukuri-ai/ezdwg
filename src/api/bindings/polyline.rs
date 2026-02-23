#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_2d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline2dEntityRow>> {
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
        let declared_match =
            matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types);
        if !declared_match
            && !is_r14_polyline_2d_speculative_type(decoder.version(), header.type_code)
        {
            continue;
        }
        let mut entity = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_polyline_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(value) => {
                    entity = Some(value);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let entity = match entity {
            Some(entity) => entity,
            None if !declared_match => continue,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        if !declared_match
            && std::env::var("EZDWG_DEBUG_R14_POLY2D")
                .ok()
                .is_some_and(|value| value != "0")
        {
            eprintln!(
                concat!(
                    "[r14-poly2d] handle={} type=0x{:X} flags={} curve_type={} ",
                    "owned={} width=({:.6},{:.6}) thickness={:.6} elevation={:.6}",
                ),
                obj.handle.0,
                header.type_code,
                entity.flags,
                entity.curve_type,
                entity.owned_handles.len(),
                entity.width_start,
                entity.width_end,
                entity.thickness,
                entity.elevation,
            );
        }
        if !declared_match && !is_plausible_polyline_2d_entity(&entity) {
            continue;
        }
        result.push((
            entity.handle,
            entity.flags,
            entity.curve_type,
            entity.width_start,
            entity.width_end,
            entity.thickness,
            entity.elevation,
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
pub fn decode_polyline_2d_entities_interpreted(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline2dInterpretedRow>> {
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
        let declared_match =
            matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types);
        if !declared_match
            && !is_r14_polyline_2d_speculative_type(decoder.version(), header.type_code)
        {
            continue;
        }
        let mut entity = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_polyline_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(value) => {
                    entity = Some(value);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let entity = match entity {
            Some(entity) => entity,
            None if !declared_match => continue,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        if !declared_match
            && std::env::var("EZDWG_DEBUG_R14_POLY2D")
                .ok()
                .is_some_and(|value| value != "0")
        {
            eprintln!(
                concat!(
                    "[r14-poly2d] handle={} type=0x{:X} flags={} curve_type={} ",
                    "owned={} width=({:.6},{:.6}) thickness={:.6} elevation={:.6}",
                ),
                obj.handle.0,
                header.type_code,
                entity.flags,
                entity.curve_type,
                entity.owned_handles.len(),
                entity.width_start,
                entity.width_end,
                entity.thickness,
                entity.elevation,
            );
        }
        if !declared_match && !is_plausible_polyline_2d_entity(&entity) {
            continue;
        }
        let info = entity.flags_info;
        let curve_label = entity.curve_type_info.label().to_string();
        result.push((
            entity.handle,
            entity.flags,
            entity.curve_type,
            curve_label,
            info.closed,
            info.curve_fit,
            info.spline_fit,
            info.is_3d_polyline,
            info.is_3d_mesh,
            info.is_closed_mesh,
            info.is_polyface_mesh,
            info.continuous_linetype,
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
pub fn decode_lwpolyline_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<LwPolylineEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x4D, "LWPOLYLINE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_lwpolyline_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => {
                if std::env::var("EZDWG_DEBUG_LWPOLYLINE")
                    .ok()
                    .is_some_and(|value| value != "0")
                {
                    eprintln!(
                        "[lwpolyline] skip handle={} type=0x{:X} err={}",
                        obj.handle.0, header.type_code, err
                    );
                }
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.flags,
            entity.vertices,
            entity.bulges,
            entity.widths,
            entity.const_width,
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
pub fn decode_polyline_3d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline3dEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x10, "POLYLINE_3D", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_polyline_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle, entity.flags_75_bits, entity.flags_70_bits));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_vertex_3d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Vertex3dEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x0B, "VERTEX_3D", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_vertex_3d_for_version(
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
            entity.flags,
            entity.position.0,
            entity.position.1,
            entity.position.2,
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
pub fn decode_polyline_3d_with_vertices(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline3dVerticesRow>> {
    let decoded_rows = decode_polyline_3d_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());
    for row in decoded_rows {
        let mut vertices: Vec<Point3> = row.vertices.iter().map(|vertex| vertex.position).collect();
        if row.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }
        result.push((row.handle, row.flags_70_bits, row.closed, vertices));
    }
    Ok(result)
}

fn decode_polyline_3d_vertex_rows(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline3dVertexRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let vertex_map = build_vertex_3d_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let Some((record, header)) =
            parse_record_and_header(&decoder, sorted[i].offset, best_effort)?
        else {
            i += 1;
            continue;
        };
        if !matches_type_name(header.type_code, 0x10, "POLYLINE_3D", &dynamic_types) {
            i += 1;
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                i += 1;
                continue;
            }
            return Err(to_py_err(err));
        }
        let poly = match decode_polyline_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            sorted[i].handle.0,
        ) {
            Ok(poly) => poly,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let (vertices, next_i) = collect_polyline_3d_vertices(
            &decoder,
            &sorted,
            &dynamic_types,
            &vertex_map,
            &poly,
            i,
            best_effort,
        )?;
        i = next_i;
        result.push(Polyline3dVertexRow {
            handle: poly.handle,
            flags_70_bits: poly.flags_70_bits,
            closed: (poly.flags_70_bits & 0x01) != 0,
            vertices,
        });
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn build_vertex_3d_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::Vertex3dEntity>> {
    let mut vertex_map = HashMap::new();
    for obj in sorted {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0B, "VERTEX_3D", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let vertex = match decode_vertex_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(vertex) => vertex,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        vertex_map.insert(vertex.handle, vertex);
    }
    Ok(vertex_map)
}

fn collect_polyline_3d_vertices(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    vertex_map: &HashMap<u64, entities::Vertex3dEntity>,
    poly: &entities::Polyline3dEntity,
    start_index: usize,
    best_effort: bool,
) -> PyResult<(Vec<entities::Vertex3dEntity>, usize)> {
    let mut vertices = Vec::new();

    if !poly.owned_handles.is_empty() {
        for handle in &poly.owned_handles {
            if let Some(vertex) = vertex_map.get(handle) {
                vertices.push(vertex.clone());
            }
        }
        return Ok((vertices, start_index + 1));
    }

    let mut next_i = start_index + 1;
    while next_i < sorted.len() {
        let Some((next_record, next_header)) =
            parse_record_and_header(decoder, sorted[next_i].offset, best_effort)?
        else {
            next_i += 1;
            continue;
        };
        let mut next_reader = next_record.bit_reader();
        if matches_type_name(next_header.type_code, 0x0B, "VERTEX_3D", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            let vertex = match decode_vertex_3d_for_version(
                &mut next_reader,
                decoder.version(),
                &next_header,
                sorted[next_i].handle.0,
            ) {
                Ok(vertex) => vertex,
                Err(err) if best_effort => {
                    next_i += 1;
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
            vertices.push(vertex);
            next_i += 1;
            continue;
        }
        if matches_type_name(next_header.type_code, 0x06, "SEQEND", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            if let Err(err) = entities::decode_seqend(&mut next_reader) {
                if !best_effort {
                    return Err(to_py_err(err));
                }
            }
            next_i += 1;
        }
        break;
    }

    Ok((vertices, next_i))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_mesh_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineMeshEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x1E, "POLYLINE_MESH", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_polyline_mesh_for_version(
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
            entity.flags,
            entity.curve_type,
            entity.m_vertex_count,
            entity.n_vertex_count,
            entity.m_density,
            entity.n_density,
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
pub fn decode_vertex_mesh_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<VertexMeshEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x0C, "VERTEX_MESH", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_vertex_3d_for_version(
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
            entity.flags,
            entity.position.0,
            entity.position.1,
            entity.position.2,
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
pub fn decode_polyline_mesh_with_vertices(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineMeshVerticesRow>> {
    let decoded_rows = decode_polyline_mesh_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());
    for row in decoded_rows {
        let mut vertices: Vec<Point3> = row.vertices.iter().map(|vertex| vertex.position).collect();
        if row.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }
        result.push((
            row.handle,
            row.flags,
            row.m_vertex_count,
            row.n_vertex_count,
            row.closed,
            vertices,
        ));
    }
    Ok(result)
}

fn decode_polyline_mesh_vertex_rows(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineMeshVertexRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let vertex_map = build_vertex_mesh_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let Some((record, header)) =
            parse_record_and_header(&decoder, sorted[i].offset, best_effort)?
        else {
            i += 1;
            continue;
        };
        if !matches_type_name(header.type_code, 0x1E, "POLYLINE_MESH", &dynamic_types) {
            i += 1;
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                i += 1;
                continue;
            }
            return Err(to_py_err(err));
        }
        let poly = match decode_polyline_mesh_for_version(
            &mut reader,
            decoder.version(),
            &header,
            sorted[i].handle.0,
        ) {
            Ok(poly) => poly,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let (vertices, next_i) = collect_polyline_mesh_vertices(
            &decoder,
            &sorted,
            &dynamic_types,
            &vertex_map,
            &poly,
            i,
            best_effort,
        )?;
        i = next_i;
        result.push(PolylineMeshVertexRow {
            handle: poly.handle,
            flags: poly.flags,
            m_vertex_count: poly.m_vertex_count,
            n_vertex_count: poly.n_vertex_count,
            closed: (poly.flags & 0x01) != 0,
            vertices,
        });
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn build_vertex_mesh_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::Vertex3dEntity>> {
    let mut vertex_map = HashMap::new();
    for obj in sorted {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0C, "VERTEX_MESH", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let vertex = match decode_vertex_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(vertex) => vertex,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        vertex_map.insert(vertex.handle, vertex);
    }
    Ok(vertex_map)
}

fn collect_polyline_mesh_vertices(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    vertex_map: &HashMap<u64, entities::Vertex3dEntity>,
    poly: &entities::PolylineMeshEntity,
    start_index: usize,
    best_effort: bool,
) -> PyResult<(Vec<entities::Vertex3dEntity>, usize)> {
    let mut vertices = Vec::new();

    if !poly.owned_handles.is_empty() {
        for handle in &poly.owned_handles {
            if let Some(vertex) = vertex_map.get(handle) {
                vertices.push(vertex.clone());
            }
        }
        return Ok((vertices, start_index + 1));
    }

    let mut next_i = start_index + 1;
    while next_i < sorted.len() {
        let Some((next_record, next_header)) =
            parse_record_and_header(decoder, sorted[next_i].offset, best_effort)?
        else {
            next_i += 1;
            continue;
        };
        let mut next_reader = next_record.bit_reader();
        if matches_type_name(next_header.type_code, 0x0C, "VERTEX_MESH", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            let vertex = match decode_vertex_3d_for_version(
                &mut next_reader,
                decoder.version(),
                &next_header,
                sorted[next_i].handle.0,
            ) {
                Ok(vertex) => vertex,
                Err(err) if best_effort => {
                    next_i += 1;
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
            vertices.push(vertex);
            next_i += 1;
            continue;
        }
        if matches_type_name(next_header.type_code, 0x06, "SEQEND", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            if let Err(err) = entities::decode_seqend(&mut next_reader) {
                if !best_effort {
                    return Err(to_py_err(err));
                }
            }
            next_i += 1;
        }
        break;
    }

    Ok((vertices, next_i))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_pface_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylinePFaceEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x1D, "POLYLINE_PFACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_polyline_pface_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle, entity.num_vertices, entity.num_faces));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_vertex_pface_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<VertexPFaceEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x0D, "VERTEX_PFACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_vertex_3d_for_version(
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
            entity.flags,
            entity.position.0,
            entity.position.1,
            entity.position.2,
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
pub fn decode_vertex_pface_face_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<VertexPFaceFaceEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x0E, "VERTEX_PFACE_FACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_vertex_pface_face_for_version(
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
            entity.index1,
            entity.index2,
            entity.index3,
            entity.index4,
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
pub fn decode_polyline_pface_with_faces(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylinePFaceFacesRow>> {
    let decoded_rows = decode_polyline_pface_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());
    for row in decoded_rows {
        let vertices: Vec<Point3> = row.vertices.iter().map(|vertex| vertex.position).collect();
        let faces: Vec<PFaceFaceRow> = row
            .faces
            .iter()
            .map(|face| (face.index1, face.index2, face.index3, face.index4))
            .collect();
        result.push((row.handle, row.num_vertices, row.num_faces, vertices, faces));
    }
    Ok(result)
}

fn decode_polyline_pface_rows(path: &str, limit: Option<usize>) -> PyResult<Vec<PolylinePFaceRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let vertex_map = build_vertex_pface_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let face_map = build_vertex_pface_face_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let Some((record, header)) =
            parse_record_and_header(&decoder, sorted[i].offset, best_effort)?
        else {
            i += 1;
            continue;
        };
        if !matches_type_name(header.type_code, 0x1D, "POLYLINE_PFACE", &dynamic_types) {
            i += 1;
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                i += 1;
                continue;
            }
            return Err(to_py_err(err));
        }
        let poly = match decode_polyline_pface_for_version(
            &mut reader,
            decoder.version(),
            &header,
            sorted[i].handle.0,
        ) {
            Ok(poly) => poly,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let (vertices, faces, next_i) = collect_polyline_pface_data(
            &decoder,
            &sorted,
            &dynamic_types,
            &vertex_map,
            &face_map,
            &poly,
            i,
            best_effort,
        )?;
        i = next_i;
        result.push(PolylinePFaceRow {
            handle: poly.handle,
            num_vertices: poly.num_vertices,
            num_faces: poly.num_faces,
            vertices,
            faces,
        });
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn build_vertex_pface_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::Vertex3dEntity>> {
    let mut vertex_map = HashMap::new();
    for obj in sorted {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0D, "VERTEX_PFACE", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let vertex = match decode_vertex_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(vertex) => vertex,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        vertex_map.insert(vertex.handle, vertex);
    }
    Ok(vertex_map)
}

fn build_vertex_pface_face_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::VertexPFaceFaceEntity>> {
    let mut face_map = HashMap::new();
    for obj in sorted {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0E, "VERTEX_PFACE_FACE", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let face = match decode_vertex_pface_face_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(face) => face,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        face_map.insert(face.handle, face);
    }
    Ok(face_map)
}

fn collect_polyline_pface_data(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    vertex_map: &HashMap<u64, entities::Vertex3dEntity>,
    face_map: &HashMap<u64, entities::VertexPFaceFaceEntity>,
    poly: &entities::PolylinePFaceEntity,
    start_index: usize,
    best_effort: bool,
) -> PyResult<(
    Vec<entities::Vertex3dEntity>,
    Vec<entities::VertexPFaceFaceEntity>,
    usize,
)> {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();

    if !poly.owned_handles.is_empty() {
        for handle in &poly.owned_handles {
            if let Some(vertex) = vertex_map.get(handle) {
                vertices.push(vertex.clone());
                continue;
            }
            if let Some(face) = face_map.get(handle) {
                faces.push(face.clone());
            }
        }
        return Ok((vertices, faces, start_index + 1));
    }

    let mut next_i = start_index + 1;
    while next_i < sorted.len() {
        let Some((next_record, next_header)) =
            parse_record_and_header(decoder, sorted[next_i].offset, best_effort)?
        else {
            next_i += 1;
            continue;
        };
        let mut next_reader = next_record.bit_reader();
        if matches_type_name(next_header.type_code, 0x0D, "VERTEX_PFACE", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            let vertex = match decode_vertex_3d_for_version(
                &mut next_reader,
                decoder.version(),
                &next_header,
                sorted[next_i].handle.0,
            ) {
                Ok(vertex) => vertex,
                Err(err) if best_effort => {
                    next_i += 1;
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
            vertices.push(vertex);
            next_i += 1;
            continue;
        }
        if matches_type_name(
            next_header.type_code,
            0x0E,
            "VERTEX_PFACE_FACE",
            dynamic_types,
        ) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            let face = match decode_vertex_pface_face_for_version(
                &mut next_reader,
                decoder.version(),
                &next_header,
                sorted[next_i].handle.0,
            ) {
                Ok(face) => face,
                Err(err) if best_effort => {
                    next_i += 1;
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
            faces.push(face);
            next_i += 1;
            continue;
        }
        if matches_type_name(next_header.type_code, 0x06, "SEQEND", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            if let Err(err) = entities::decode_seqend(&mut next_reader) {
                if !best_effort {
                    return Err(to_py_err(err));
                }
            }
            next_i += 1;
        }
        break;
    }

    Ok((vertices, faces, next_i))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_2d_with_vertices(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineVerticesRow>> {
    let decoded_rows = decode_polyline_2d_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());

    for row in decoded_rows {
        let use_vertex_z = polyline_uses_vertex_z(row.flags_info);
        let mut vertices: Vec<Point3> = row
            .vertices
            .iter()
            .map(|vertex| vertex_position_for_polyline(vertex, row.elevation, use_vertex_z))
            .collect();
        if row.flags_info.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }
        result.push((row.handle, row.flags, vertices));
    }

    Ok(result)
}

#[pyfunction(signature = (path, segments_per_span=8, limit=None))]
pub fn decode_polyline_2d_with_vertices_interpolated(
    path: &str,
    segments_per_span: usize,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineInterpolatedRow>> {
    let decoded_rows = decode_polyline_2d_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());

    for row in decoded_rows {
        let use_vertex_z = polyline_uses_vertex_z(row.flags_info);
        let mut vertices: Vec<Point3> = row
            .vertices
            .iter()
            .map(|vertex| vertex_position_for_polyline(vertex, row.elevation, use_vertex_z))
            .collect();
        let mut applied = false;
        let should_interpolate = row.flags_info.curve_fit
            || row.flags_info.spline_fit
            || matches!(
                row.curve_type_info,
                entities::PolylineCurveType::QuadraticBSpline
                    | entities::PolylineCurveType::CubicBSpline
                    | entities::PolylineCurveType::Bezier
            );

        if should_interpolate && vertices.len() > 1 {
            let base = strip_closure(vertices);
            let interpolated =
                entities::catmull_rom_spline(&base, row.flags_info.closed, segments_per_span)
                    .map_err(to_py_err)?;
            vertices = interpolated;
            applied = true;
        } else if row.flags_info.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }

        result.push((row.handle, row.flags, applied, vertices));
    }

    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_vertex_2d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Vertex2dEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
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
        if !matches_type_name(header.type_code, 0x0A, "VERTEX_2D", &dynamic_types) {
            continue;
        }
        let mut decoded: Option<entities::Vertex2dEntity> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_vertex_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(vertex) => {
                    decoded = Some(vertex);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let vertex = match decoded {
            Some(vertex) => vertex,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        result.push((
            vertex.handle,
            vertex.flags,
            vertex.position.0,
            vertex.position.1,
            vertex.position.2,
            vertex.start_width,
            vertex.end_width,
            vertex.bulge,
            vertex.tangent_dir,
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
pub fn decode_polyline_2d_with_vertex_data(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineVertexDataRow>> {
    let decoded_rows = decode_polyline_2d_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());

    for row in decoded_rows {
        let use_vertex_z = polyline_uses_vertex_z(row.flags_info);
        let mut vertices: Vec<VertexDataRow> = row
            .vertices
            .iter()
            .map(|vertex| vertex_data_for_polyline(vertex, row.elevation, use_vertex_z))
            .collect();
        if row.flags_info.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d_with_data(first, last) {
                vertices.push(first);
            }
        }
        result.push((row.handle, row.flags, vertices));
    }

    Ok(result)
}

fn decode_polyline_2d_vertex_rows(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineVertexRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let vertex_map = build_vertex_2d_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let mut vertices_by_owner: HashMap<u64, Vec<entities::Vertex2dEntity>> = HashMap::new();
    for vertex in vertex_map.values() {
        let Some(owner_handle) = vertex.owner_handle else {
            continue;
        };
        vertices_by_owner
            .entry(owner_handle)
            .or_default()
            .push(vertex.clone());
    }
    for owned_vertices in vertices_by_owner.values_mut() {
        owned_vertices.sort_by_key(|vertex| vertex.handle);
    }
    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let obj = sorted[i];
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let declared_match =
            matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types);
        if !declared_match
            && !is_r14_polyline_2d_speculative_type(decoder.version(), header.type_code)
        {
            i += 1;
            continue;
        }

        let mut poly: Option<entities::Polyline2dEntity> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            let decoded = decode_polyline_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            );
            match decoded {
                Ok(entity) => {
                    poly = Some(entity);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let poly = match poly {
            Some(poly) => poly,
            None if !declared_match => {
                i += 1;
                continue;
            }
            None if best_effort => {
                i += 1;
                continue;
            }
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                i += 1;
                continue;
            }
        };
        if !declared_match
            && std::env::var("EZDWG_DEBUG_R14_POLY2D")
                .ok()
                .is_some_and(|value| value != "0")
        {
            eprintln!(
                concat!(
                    "[r14-poly2d] handle={} type=0x{:X} flags={} curve_type={} ",
                    "owned={} width=({:.6},{:.6}) thickness={:.6} elevation={:.6}",
                ),
                obj.handle.0,
                header.type_code,
                poly.flags,
                poly.curve_type,
                poly.owned_handles.len(),
                poly.width_start,
                poly.width_end,
                poly.thickness,
                poly.elevation,
            );
        }
        if !declared_match && !is_plausible_polyline_2d_entity(&poly) {
            i += 1;
            continue;
        }
        let (vertices, next_i) = collect_polyline_vertices(
            &decoder,
            &sorted,
            &dynamic_types,
            &vertex_map,
            &vertices_by_owner,
            &poly,
            i,
            best_effort,
        )?;
        let vertices = sanitize_polyline_2d_vertices(vertices);
        i = next_i;

        result.push(PolylineVertexRow {
            handle: poly.handle,
            flags: poly.flags,
            flags_info: poly.flags_info,
            curve_type_info: poly.curve_type_info,
            elevation: poly.elevation,
            vertices,
        });
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn build_vertex_2d_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::Vertex2dEntity>> {
    let mut vertex_map = HashMap::new();
    for obj in sorted {
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
        if !matches_type_name(header.type_code, 0x0A, "VERTEX_2D", dynamic_types) {
            continue;
        }
        let mut decoded: Option<entities::Vertex2dEntity> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_vertex_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(vertex) => {
                    decoded = Some(vertex);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let vertex = match decoded {
            Some(vertex) => vertex,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        vertex_map.insert(vertex.handle, vertex);
    }
    Ok(vertex_map)
}

fn collect_polyline_vertices(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    vertex_map: &HashMap<u64, entities::Vertex2dEntity>,
    vertices_by_owner: &HashMap<u64, Vec<entities::Vertex2dEntity>>,
    poly: &entities::Polyline2dEntity,
    start_index: usize,
    best_effort: bool,
) -> PyResult<(Vec<entities::Vertex2dEntity>, usize)> {
    let mut vertices = Vec::new();

    if !poly.owned_handles.is_empty() {
        for handle in &poly.owned_handles {
            if let Some(vertex) = vertex_map.get(handle) {
                vertices.push(vertex.clone());
            }
        }
        return Ok((vertices, start_index + 1));
    }

    if let Some(owned_vertices) = vertices_by_owner.get(&poly.handle) {
        return Ok((owned_vertices.clone(), start_index + 1));
    }

    // Legacy POLYLINE_2D often stores VERTEX/SEQEND far from parent in object-offset
    // order, but keeps handle adjacency: POLYLINE -> VERTEX* -> SEQEND.
    let mut handle_cursor = poly.handle.saturating_add(1);
    while let Some(vertex) = vertex_map.get(&handle_cursor) {
        vertices.push(vertex.clone());
        handle_cursor = handle_cursor.saturating_add(1);
    }
    if !vertices.is_empty() {
        return Ok((vertices, start_index + 1));
    }

    let mut next_i = start_index + 1;
    while next_i < sorted.len() {
        let next = sorted[next_i];
        let next_record = match decoder.parse_object_record(next.offset) {
            Ok(record) => record,
            Err(err) if best_effort => {
                next_i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let next_header = match parse_object_header_for_version(&next_record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => {
                next_i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let mut next_reader = next_record.bit_reader();
        if matches_type_name(next_header.type_code, 0x0A, "VERTEX_2D", dynamic_types) {
            let mut decoded: Option<entities::Vertex2dEntity> = None;
            let mut last_err = None;
            for with_prefix in [true, false] {
                let mut candidate_reader = if with_prefix {
                    let mut prefixed = next_record.bit_reader();
                    if let Err(err) = skip_object_type_prefix(&mut prefixed, decoder.version()) {
                        last_err = Some(err);
                        continue;
                    }
                    prefixed
                } else {
                    next_record.bit_reader()
                };
                match decode_vertex_2d_for_version(
                    &mut candidate_reader,
                    decoder.version(),
                    &next_header,
                    next.handle.0,
                ) {
                    Ok(vertex) => {
                        decoded = Some(vertex);
                        break;
                    }
                    Err(err) => last_err = Some(err),
                }
            }
            let vertex = match decoded {
                Some(vertex) => vertex,
                None if best_effort => {
                    next_i += 1;
                    continue;
                }
                None => {
                    if let Some(err) = last_err {
                        return Err(to_py_err(err));
                    }
                    next_i += 1;
                    continue;
                }
            };
            vertices.push(vertex);
            next_i += 1;
            continue;
        }
        if matches_type_name(next_header.type_code, 0x06, "SEQEND", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            if let Err(err) = entities::decode_seqend(&mut next_reader) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            next_i += 1;
        }
        break;
    }

    Ok((vertices, next_i))
}

fn sanitize_polyline_2d_vertices(
    vertices: Vec<entities::Vertex2dEntity>,
) -> Vec<entities::Vertex2dEntity> {
    if vertices.len() < 3 {
        return vertices;
    }

    let far_count = vertices
        .iter()
        .filter(|vertex| polyline_vertex_extent(vertex) >= 1000.0)
        .count();
    if far_count == 0 {
        return vertices;
    }

    let candidate_count = vertices
        .iter()
        .filter(|vertex| is_origin_like_polyline_vertex(vertex))
        .count();
    if candidate_count == 0 {
        return vertices;
    }

    let mut remove = vec![false; vertices.len()];
    for index in 0..vertices.len() {
        if !is_origin_like_polyline_vertex(&vertices[index]) {
            continue;
        }
        if candidate_count >= 2 || has_large_adjacent_jump(&vertices, index) {
            remove[index] = true;
        }
    }

    let mut cleaned: Vec<entities::Vertex2dEntity> = vertices
        .iter()
        .enumerate()
        .filter_map(|(index, vertex)| {
            if remove[index] {
                None
            } else {
                Some(vertex.clone())
            }
        })
        .collect();
    if cleaned.len() < 2 {
        if candidate_count >= vertices.len().saturating_sub(1) {
            return Vec::new();
        }
        return vertices;
    }

    let mut deduped: Vec<entities::Vertex2dEntity> = Vec::with_capacity(cleaned.len());
    for vertex in cleaned.drain(..) {
        if deduped
            .last()
            .is_some_and(|prev| points_equal_3d(prev.position, vertex.position))
        {
            continue;
        }
        deduped.push(vertex);
    }
    if deduped.len() < 2 {
        if candidate_count >= vertices.len().saturating_sub(1) {
            return Vec::new();
        }
        return vertices;
    }
    deduped
}

fn polyline_vertex_extent(vertex: &entities::Vertex2dEntity) -> f64 {
    vertex.position.0.abs().max(vertex.position.1.abs())
}

fn is_origin_like_polyline_vertex(vertex: &entities::Vertex2dEntity) -> bool {
    let x = vertex.position.0;
    let y = vertex.position.1;
    if !x.is_finite() || !y.is_finite() {
        return true;
    }
    if x.abs() + y.abs() <= 1.0e-120 {
        return true;
    }
    x.abs() <= 1.5 && y.abs() <= 1.5
}

fn polyline_uses_vertex_z(flags_info: entities::PolylineFlagsInfo) -> bool {
    flags_info.is_3d_polyline || flags_info.is_3d_mesh || flags_info.is_polyface_mesh
}

fn vertex_position_for_polyline(
    vertex: &entities::Vertex2dEntity,
    polyline_elevation: f64,
    use_vertex_z: bool,
) -> Point3 {
    let z = if use_vertex_z {
        vertex.position.2
    } else {
        polyline_elevation
    };
    (vertex.position.0, vertex.position.1, z)
}

fn vertex_data_for_polyline(
    vertex: &entities::Vertex2dEntity,
    polyline_elevation: f64,
    use_vertex_z: bool,
) -> VertexDataRow {
    let z = if use_vertex_z {
        vertex.position.2
    } else {
        polyline_elevation
    };
    (
        vertex.position.0,
        vertex.position.1,
        z,
        vertex.start_width,
        vertex.end_width,
        vertex.bulge,
        vertex.tangent_dir,
        vertex.flags,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_sequence_members(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineSequenceMembersRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let mut vertex_2d_handles: HashSet<u64> = HashSet::new();
    let mut vertex_3d_handles: HashSet<u64> = HashSet::new();
    let mut vertex_mesh_handles: HashSet<u64> = HashSet::new();
    let mut vertex_pface_handles: HashSet<u64> = HashSet::new();
    let mut vertex_pface_face_handles: HashSet<u64> = HashSet::new();
    let mut seqend_handles: HashSet<u64> = HashSet::new();

    for obj in sorted.iter() {
        let Some((_record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x0A, "VERTEX_2D", &dynamic_types) {
            vertex_2d_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x0B, "VERTEX_3D", &dynamic_types) {
            vertex_3d_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x0C, "VERTEX_MESH", &dynamic_types) {
            vertex_mesh_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x0D, "VERTEX_PFACE", &dynamic_types) {
            vertex_pface_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x0E, "VERTEX_PFACE_FACE", &dynamic_types) {
            vertex_pface_face_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x06, "SEQEND", &dynamic_types) {
            seqend_handles.insert(obj.handle.0);
        }
    }

    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let Some((record, header)) =
            parse_record_and_header(&decoder, sorted[i].offset, best_effort)?
        else {
            i += 1;
            continue;
        };
        let kind = if matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types) {
            Some(PolylineSequenceKind::Polyline2d)
        } else if matches_type_name(header.type_code, 0x10, "POLYLINE_3D", &dynamic_types) {
            Some(PolylineSequenceKind::Polyline3d)
        } else if matches_type_name(header.type_code, 0x1E, "POLYLINE_MESH", &dynamic_types) {
            Some(PolylineSequenceKind::PolylineMesh)
        } else if matches_type_name(header.type_code, 0x1D, "POLYLINE_PFACE", &dynamic_types) {
            Some(PolylineSequenceKind::PolylinePFace)
        } else {
            None
        };
        let Some(kind) = kind else {
            i += 1;
            continue;
        };

        let polyline_handle = sorted[i].handle.0;
        let mut vertex_handles: Vec<u64> = Vec::new();
        let mut face_handles: Vec<u64> = Vec::new();
        let mut seqend_handle: Option<u64> = None;

        let mut owned_handles: Option<Vec<u64>> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            let decoded = match kind {
                PolylineSequenceKind::Polyline2d => decode_polyline_2d_for_version(
                    &mut reader,
                    decoder.version(),
                    &header,
                    polyline_handle,
                )
                .map(|polyline| polyline.owned_handles),
                PolylineSequenceKind::Polyline3d => decode_polyline_3d_for_version(
                    &mut reader,
                    decoder.version(),
                    &header,
                    polyline_handle,
                )
                .map(|polyline| polyline.owned_handles),
                PolylineSequenceKind::PolylineMesh => decode_polyline_mesh_for_version(
                    &mut reader,
                    decoder.version(),
                    &header,
                    polyline_handle,
                )
                .map(|polyline| polyline.owned_handles),
                PolylineSequenceKind::PolylinePFace => decode_polyline_pface_for_version(
                    &mut reader,
                    decoder.version(),
                    &header,
                    polyline_handle,
                )
                .map(|polyline| polyline.owned_handles),
            };
            match decoded {
                Ok(handles) => {
                    owned_handles = Some(handles);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        if owned_handles.is_none() && !best_effort {
            if let Some(err) = last_err {
                return Err(to_py_err(err));
            }
        }

        let mut next_i = i + 1;
        if let Some(owned_handles) = owned_handles {
            for owned_handle in owned_handles {
                match kind {
                    PolylineSequenceKind::Polyline2d => {
                        if vertex_2d_handles.contains(&owned_handle) {
                            vertex_handles.push(owned_handle);
                            continue;
                        }
                    }
                    PolylineSequenceKind::Polyline3d => {
                        if vertex_3d_handles.contains(&owned_handle) {
                            vertex_handles.push(owned_handle);
                            continue;
                        }
                    }
                    PolylineSequenceKind::PolylineMesh => {
                        if vertex_mesh_handles.contains(&owned_handle) {
                            vertex_handles.push(owned_handle);
                            continue;
                        }
                    }
                    PolylineSequenceKind::PolylinePFace => {
                        if vertex_pface_handles.contains(&owned_handle) {
                            vertex_handles.push(owned_handle);
                            continue;
                        }
                        if vertex_pface_face_handles.contains(&owned_handle) {
                            face_handles.push(owned_handle);
                            continue;
                        }
                    }
                }
                if seqend_handle.is_none() && seqend_handles.contains(&owned_handle) {
                    seqend_handle = Some(owned_handle);
                }
            }
        } else {
            while next_i < sorted.len() {
                let Some((_next_record, next_header)) =
                    parse_record_and_header(&decoder, sorted[next_i].offset, best_effort)?
                else {
                    next_i += 1;
                    continue;
                };
                let next_handle = sorted[next_i].handle.0;
                let is_member = match kind {
                    PolylineSequenceKind::Polyline2d => {
                        matches_type_name(next_header.type_code, 0x0A, "VERTEX_2D", &dynamic_types)
                    }
                    PolylineSequenceKind::Polyline3d => {
                        matches_type_name(next_header.type_code, 0x0B, "VERTEX_3D", &dynamic_types)
                    }
                    PolylineSequenceKind::PolylineMesh => matches_type_name(
                        next_header.type_code,
                        0x0C,
                        "VERTEX_MESH",
                        &dynamic_types,
                    ),
                    PolylineSequenceKind::PolylinePFace => matches_type_name(
                        next_header.type_code,
                        0x0D,
                        "VERTEX_PFACE",
                        &dynamic_types,
                    ),
                };
                if is_member {
                    vertex_handles.push(next_handle);
                    next_i += 1;
                    continue;
                }
                if kind == PolylineSequenceKind::PolylinePFace
                    && matches_type_name(
                        next_header.type_code,
                        0x0E,
                        "VERTEX_PFACE_FACE",
                        &dynamic_types,
                    )
                {
                    face_handles.push(next_handle);
                    next_i += 1;
                    continue;
                }
                if matches_type_name(next_header.type_code, 0x06, "SEQEND", &dynamic_types) {
                    seqend_handle = Some(next_handle);
                    next_i += 1;
                }
                break;
            }
        }

        result.push((
            polyline_handle,
            kind.label().to_string(),
            vertex_handles,
            face_handles,
            seqend_handle,
        ));
        i = next_i;
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn decode_lwpolyline_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::LwPolylineEntity> {
    match version {
        version::DwgVersion::R14 => {
            entities::decode_lwpolyline_r14(reader, object_handle, header.type_code)
        }
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_lwpolyline_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_lwpolyline_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_lwpolyline_r2007(reader),
        _ => entities::decode_lwpolyline(reader),
    }
}

fn decode_polyline_2d_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Polyline2dEntity> {
    let start = reader.get_pos();
    match version {
        version::DwgVersion::R14 => entities::decode_polyline_2d_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            match entities::decode_polyline_2d_r2010(reader, object_data_end_bit, object_handle) {
                Ok(entity) => Ok(entity),
                Err(primary_err) => {
                    reader.set_pos(start.0, start.1);
                    entities::decode_polyline_2d(reader).or(Err(primary_err))
                }
            }
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            match entities::decode_polyline_2d_r2013(reader, object_data_end_bit, object_handle) {
                Ok(entity) => Ok(entity),
                Err(primary_err) => {
                    reader.set_pos(start.0, start.1);
                    entities::decode_polyline_2d(reader).or(Err(primary_err))
                }
            }
        }
        version::DwgVersion::R2007 => match entities::decode_polyline_2d_r2007(reader) {
            Ok(entity) => Ok(entity),
            Err(primary_err) => {
                reader.set_pos(start.0, start.1);
                entities::decode_polyline_2d(reader).or(Err(primary_err))
            }
        },
        _ => entities::decode_polyline_2d(reader),
    }
}

fn is_r14_polyline_2d_speculative_type(version: &version::DwgVersion, type_code: u16) -> bool {
    matches!(version, version::DwgVersion::R14) && type_code >= 0x01F4
}

fn is_plausible_polyline_2d_entity(entity: &entities::Polyline2dEntity) -> bool {
    if entity.handle == 0 {
        return false;
    }
    if !matches!(entity.curve_type, 0 | 5 | 6 | 8) {
        return false;
    }
    if !entity.width_start.is_finite()
        || !entity.width_end.is_finite()
        || !entity.thickness.is_finite()
        || !entity.elevation.is_finite()
    {
        return false;
    }
    if entity.width_start.abs() > 1.0e9
        || entity.width_end.abs() > 1.0e9
        || entity.thickness.abs() > 1.0e9
        || entity.elevation.abs() > 1.0e9
    {
        return false;
    }
    if entity.flags > 0x03FF {
        return false;
    }
    let owned_len = entity.owned_handles.len();
    owned_len > 0 && owned_len <= 4096
}

fn decode_polyline_3d_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Polyline3dEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_3d_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_3d_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_polyline_3d_r2007(reader),
        _ => entities::decode_polyline_3d(reader),
    }
}

fn decode_vertex_3d_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Vertex3dEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_3d_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_3d_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_vertex_3d_r2007(reader),
        _ => entities::decode_vertex_3d(reader),
    }
}

fn decode_vertex_2d_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Vertex2dEntity> {
    let start = reader.get_pos();
    let primary = match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_2d_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_2d_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_vertex_2d_r2007(reader),
        _ => entities::decode_vertex_2d(reader),
    };
    if let Ok(entity) = primary {
        return Ok(entity);
    }
    let primary_err = primary.unwrap_err();

    reader.set_pos(start.0, start.1);
    if let Ok(entity) = entities::decode_vertex_2d(reader) {
        return Ok(entity);
    }

    // Some drawings tag 2D vertices with legacy 3D-like payloads.
    reader.set_pos(start.0, start.1);
    if let Ok(vertex3d) = decode_vertex_3d_for_version(reader, version, header, object_handle) {
        return Ok(entities::Vertex2dEntity {
            handle: vertex3d.handle,
            flags: u16::from(vertex3d.flags),
            position: vertex3d.position,
            start_width: 0.0,
            end_width: 0.0,
            bulge: 0.0,
            tangent_dir: 0.0,
            owner_handle: None,
        });
    }
    Err(primary_err)
}

fn decode_polyline_mesh_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::PolylineMeshEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_mesh_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_mesh_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_polyline_mesh_r2007(reader),
        _ => entities::decode_polyline_mesh(reader),
    }
}

fn decode_polyline_pface_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::PolylinePFaceEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_pface_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_pface_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_polyline_pface_r2007(reader),
        _ => entities::decode_polyline_pface(reader),
    }
}

fn decode_vertex_pface_face_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::VertexPFaceFaceEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_pface_face_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_pface_face_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_vertex_pface_face_r2007(reader),
        _ => entities::decode_vertex_pface_face(reader),
    }
}
