/// Resolve explicit owned arrays or legacy first/last and next links. Never infer
/// write order from object offsets or numeric handles.
fn polyline_vertex_references(
    decoder: &decoder::Decoder<'_>,
    objects: &[objects::ObjectRef],
    parent: u64,
    count: Option<usize>,
    vertex_code: u16,
) -> Option<(Vec<u64>, u64)> {
    let by_handle: HashMap<_, _> = objects.iter().map(|o| (o.handle.0, o.offset)).collect();
    polyline_vertex_references_indexed(decoder, &by_handle, parent, count, vertex_code)
}

/// Same as `polyline_vertex_references`, with the handle → offset index built once by the
/// caller. Callers that resolve many polylines must use this one: building the index per
/// polyline is O(objects) each time, and a drawing with 23k POLYLINE_2D among 163k objects
/// spent 65 s of its 78 s read here (2026-10-17).
fn polyline_vertex_references_indexed(
    decoder: &decoder::Decoder<'_>,
    by_handle: &HashMap<u64, u32>,
    parent: u64,
    count: Option<usize>,
    vertex_code: u16,
) -> Option<(Vec<u64>, u64)> {
    let get = |h| {
        let offset = *by_handle.get(&h)?;
        parse_record_and_header(decoder, offset, true)
            .ok()
            .flatten()
    };
    let (record, header) = get(parent)?;
    let (mut refs, _, _) =
        read_common_entity_header_and_handles(&record, decoder.version(), &header)?;
    let mut handles = Vec::new();
    if let Some(count) = count {
        if count > by_handle.len() {
            return None;
        }
        for _ in 0..count {
            handles.push(entities::common::read_handle_reference(&mut refs, parent).ok()?);
        }
    } else {
        let first = entities::common::read_handle_reference(&mut refs, parent).ok()?;
        let last = entities::common::read_handle_reference(&mut refs, parent).ok()?;
        let mut h = first;
        let mut seen = HashSet::new();
        while h != 0 {
            if !seen.insert(h) {
                return None;
            }
            handles.push(h);
            if h == last {
                break;
            }
            let (record, header) = get(h)?;
            if header.type_code != vertex_code {
                return None;
            }
            let (_, links) =
                decode_common_entity_header_and_handles(&record, decoder.version(), &header)?;
            h = links.next.unwrap_or(h.checked_add(1)?);
        }
        if handles.last().copied().unwrap_or(0) != last {
            return None;
        }
    }
    let seqend = entities::common::read_handle_reference(&mut refs, parent).ok()?;
    let mut seen = HashSet::new();
    for (h, code) in handles
        .iter()
        .map(|&h| (h, vertex_code))
        .chain(std::iter::once((seqend, 6)))
    {
        if !seen.insert(h) {
            return None;
        }
        let (record, header) = get(h)?;
        if header.type_code != code {
            return None;
        }
        let (_, links) =
            decode_common_entity_header_and_handles(&record, decoder.version(), &header)?;
        if links.owner_ref != Some(parent) {
            return None;
        }
    }
    Some((handles, seqend))
}

#[pyfunction]
pub fn decode_write_polylines(py: Python<'_>, path: &str) -> PyResult<Vec<PyObject>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let version = decoder.version();
    let legacy = matches!(
        version,
        DwgVersion::R13 | DwgVersion::R14 | DwgVersion::R2000
    );
    let r14 = matches!(version, DwgVersion::R13 | DwgVersion::R14);
    let modern_id = matches!(
        version,
        DwgVersion::R2010 | DwgVersion::R2013 | DwgVersion::R2018
    );
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let offsets: HashMap<_, _> = index
        .objects
        .iter()
        .map(|o| (o.handle.0, o.offset))
        .collect();
    let mut children_by_owner: HashMap<u64, HashSet<u64>> = HashMap::new();
    for obj in &index.objects {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, true)? else {
            continue;
        };
        if matches!(header.type_code, 6 | 0x0a | 0x0b) {
            if let Some((_, links)) =
                decode_common_entity_header_and_handles(&record, version, &header)
            {
                if let Some(owner) = links.owner_ref {
                    children_by_owner
                        .entry(owner)
                        .or_default()
                        .insert(obj.handle.0);
                }
            }
        }
    }
    let mut result = Vec::new();
    for obj in &index.objects {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, true)? else {
            continue;
        };
        if !matches!(header.type_code, 0x0f | 0x10) {
            continue;
        }
        let decoded = (|| -> PyResult<PyObject> {
            let (mut body, _) = read_common_entity_body(&record, version, &header)
                .ok_or_else(|| PyValueError::new_err("undecoded polyline header"))?;
            let (_, parent_links) =
                decode_common_entity_header_and_handles(&record, version, &header)
                    .ok_or_else(|| PyValueError::new_err("undecoded polyline owner"))?;
            let is_3d = header.type_code == 0x10;
            let (flags, curve_type, width_start, width_end, thickness, elevation, extrusion) =
                if is_3d {
                    let curve = body.read_rc().map_err(to_py_err)?;
                    let closed = body.read_rc().map_err(to_py_err)?;
                    if closed & !1 != 0 || curve & !3 != 0 {
                        return Err(PyValueError::new_err("unsupported 3D polyline flags"));
                    }
                    (
                        8 | u16::from(closed & 1) | if curve != 0 { 4 } else { 0 },
                        u16::from(curve),
                        0.,
                        0.,
                        0.,
                        0.,
                        (0., 0., 1.),
                    )
                } else {
                    let flags = body.read_bs().map_err(to_py_err)?;
                    let curve = body.read_bs().map_err(to_py_err)?;
                    let ws = body.read_bd().map_err(to_py_err)?;
                    let we = body.read_bd().map_err(to_py_err)?;
                    let thickness =
                        if r14 { body.read_bd() } else { body.read_bt() }.map_err(to_py_err)?;
                    let elevation = body.read_bd().map_err(to_py_err)?;
                    let normal =
                        if r14 { body.read_3bd() } else { body.read_be() }.map_err(to_py_err)?;
                    (flags, curve, ws, we, thickness, elevation, normal)
                };
            let count = if legacy {
                None
            } else {
                Some(body.read_bl().map_err(to_py_err)? as usize)
            };
            let refs = polyline_vertex_references_indexed(
                &decoder,
                &offsets,
                obj.handle.0,
                count,
                if is_3d { 0x0b } else { 0x0a },
            );
            let mut losses = Vec::new();
            let mut complete = refs.is_some();
            let (handles, seqend) = refs.unwrap_or_default();
            let referenced: HashSet<_> = handles
                .iter()
                .copied()
                .chain(std::iter::once(seqend))
                .collect();
            if children_by_owner.get(&obj.handle.0) != Some(&referenced) {
                complete = false;
                losses.push("unreferenced or inconsistent owned VERTEX/SEQEND objects".into());
            }
            let mut vertices = Vec::new();
            for &h in handles
                .iter()
                .chain(std::iter::once(&seqend))
                .filter(|&&h| h != 0)
            {
                let (record, header) = parse_record_and_header(&decoder, offsets[&h], true)?
                    .ok_or_else(|| PyValueError::new_err("undecoded polyline child"))?;
                let (mut r, common) = read_common_entity_body(&record, version, &header)
                    .ok_or_else(|| PyValueError::new_err("undecoded vertex body"))?;
                let (_, links) = decode_common_entity_header_and_handles(&record, version, &header)
                    .ok_or_else(|| PyValueError::new_err("undecoded vertex properties"))?;
                if links.layer != parent_links.layer
                    || common.color.index != Some(256)
                    || common.color.true_color.is_some()
                    || common.ltype_flags != 0
                    || common.ltype_scale != 1.
                    || common.invisible
                    || common.line_weight.is_some_and(|w| w != 29)
                {
                    losses.push(format!(
                        "child {h}: individual display properties cannot be preserved"
                    ));
                }
                if h == seqend {
                    continue;
                }
                let flags = r.read_rc().map_err(to_py_err)?;
                let p = r.read_3bd().map_err(to_py_err)?;
                let (ws, we, bulge, tangent) = if is_3d {
                    (0., 0., 0., 0.)
                } else {
                    let ws = r.read_bd().map_err(to_py_err)?;
                    let we = if ws < 0. {
                        -ws
                    } else {
                        r.read_bd().map_err(to_py_err)?
                    };
                    let bulge = r.read_bd().map_err(to_py_err)?;
                    if modern_id && r.read_bl().map_err(to_py_err)? != 0 {
                        losses.push(format!(
                            "vertex {h}: vertex ID cannot be preserved by AC1015"
                        ));
                    }
                    (ws.abs(), we, bulge, r.read_bd().map_err(to_py_err)?)
                };
                if flags != if is_3d { 32 } else { 0 } || tangent != 0. || (!is_3d && p.2 != 0.) {
                    complete = false;
                    losses.push(format!(
                        "vertex {h}: fit flags, tangent or nonzero 2D Z are unsupported"
                    ));
                }
                vertices.push((p, ws, we, bulge));
            }
            let d = PyDict::new_bound(py);
            d.set_item("handle", obj.handle.0)?;
            d.set_item("flags", flags)?;
            d.set_item("curve_type", curve_type)?;
            d.set_item("width_start", width_start)?;
            d.set_item("width_end", width_end)?;
            d.set_item("thickness", thickness)?;
            d.set_item("elevation", elevation)?;
            d.set_item("extrusion", extrusion)?;
            d.set_item("vertices", vertices)?;
            d.set_item("vertex_handles", handles)?;
            d.set_item("seqend_handle", seqend)?;
            d.set_item("complete", complete)?;
            d.set_item("losses", losses)?;
            Ok(d.into_any().unbind())
        })();
        if let Ok(row) = decoded {
            result.push(row);
        }
    }
    Ok(result)
}
