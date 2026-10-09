// ============================================================================
// Layer Filters — ACAD_LAYERFILTERS (property) + ACLYDICTIONARY (nested AcLy)
//
// Property filters: XRECORDs under ACAD_LAYERFILTERS (legacy group-1 patterns).
// Nested filters: AcLyLayerFilter XRECORDs under ACLYDICTIONARY; children hang
// off each filter's extension dictionary → nested ACLYDICTIONARY.
//
// Public functions (registered on ezdwg.raw):
//   decode_layer_filter_names(path) -> list[str]          // property only
//   decode_layer_filter_xrecords(path) -> list of rows    // property only
//   decode_layer_filter_tree(path) -> list of tree nodes  // ACLY nested tree
// ============================================================================

/// One raw property-filter row (ACAD_LAYERFILTERS).
type LayerFilterRawRow = (
    String,                 // name
    u64,                    // xrecord_handle
    Option<String>,         // expression
    Option<i64>,            // flags (group 70)
    Vec<(i16, PyObject)>,   // xdata groups
    Vec<u64>,               // objid handles
    Vec<u8>,                // raw xdata bytes
);

/// One node in the ACLY nested filter tree.
type LayerFilterTreeNode = (
    String,                 // display name (group 300)
    u64,                    // xrecord handle
    Option<String>,         // expression (group 301)
    String,                 // synthetic dict key (*A1, …) or ""
    u32,                    // depth (0 = root)
    u32,                    // number of direct children
    Vec<(i16, PyObject)>,   // xdata groups
);

const ACAD_LAYERFILTERS_KEYS: &[&str] = &["ACAD_LAYERFILTERS", "ACAD_LAYERFILTER"];
const ACLY_DICTIONARY_KEYS: &[&str] = &["ACLYDICTIONARY", "ACLY_DICTIONARY"];

fn extract_filter_expression_and_flags(
    groups: &[objects::XDataGroup],
) -> (Option<String>, Option<i64>) {
    let mut expression = None;
    let mut flags = None;
    let mut strings: Vec<&str> = Vec::new();
    for g in groups {
        match g.code {
            70 if flags.is_none() => match &g.value {
                objects::XDataValue::Int16(v) => flags = Some(*v as i64),
                objects::XDataValue::Int32(v) => flags = Some(*v as i64),
                objects::XDataValue::Int64(v) => flags = Some(*v),
                _ => {}
            },
            301 | 300 => {
                if let objects::XDataValue::String(s) = &g.value {
                    if !s.is_empty() && expression.is_none() {
                        if g.code == 301 || s.contains("NAME") || s.starts_with('(') {
                            expression = Some(s.clone());
                        }
                    }
                }
            }
            1 => {
                if let objects::XDataValue::String(s) = &g.value {
                    strings.push(s.as_str());
                }
            }
            _ => {}
        }
    }
    if expression.is_none() {
        if let Some(pat) = strings.get(1) {
            if *pat != "*" && !pat.is_empty() {
                expression = Some(format!("( NAME == \"{pat}\" )"));
            }
        }
    }
    (expression, flags)
}

fn extract_acly_name_and_expression(
    groups: &[objects::XDataGroup],
) -> (Option<String>, Option<String>) {
    let mut name = None;
    let mut expression = None;
    for g in groups {
        match g.code {
            300 => {
                if let objects::XDataValue::String(s) = &g.value {
                    if !s.is_empty() && name.is_none() {
                        name = Some(s.clone());
                    }
                }
            }
            301 => {
                if let objects::XDataValue::String(s) = &g.value {
                    if !s.is_empty() && expression.is_none() {
                        expression = Some(s.clone());
                    }
                }
            }
            _ => {}
        }
    }
    (name, expression)
}

fn discover_named_dict_entries(path: &str, keys: &[&str]) -> PyResult<Vec<(String, u64)>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;

    let mut by_handle: HashMap<u64, usize> = HashMap::new();
    for (i, obj) in index.objects.iter().enumerate() {
        by_handle.insert(obj.handle.0, i);
    }

    let mut entries: Vec<(String, u64)> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();

    let push_dict = |filt_dict: &objects::Dictionary,
                     entries: &mut Vec<(String, u64)>,
                     seen: &mut HashSet<u64>| {
        for e in &filt_dict.entries {
            if e.name.is_empty() {
                continue;
            }
            if let Some(xh) = e.value_handle {
                if seen.insert(xh.0) {
                    entries.push((e.name.clone(), xh.0));
                }
            } else if !entries.iter().any(|(n, _)| n == &e.name) {
                entries.push((e.name.clone(), 0));
            }
        }
    };

    for obj in index.objects.iter() {
        let Some((_record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !is_layer_control_type(header.type_code, &dynamic) {
            continue;
        }
        let Some(xdic_h) = layer_control_xdic_handle(&decoder, obj, best_effort) else {
            continue;
        };
        let Some(&idx) = by_handle.get(&xdic_h) else {
            continue;
        };
        let Some(xdic) = decode_one_dictionary(&decoder, &index.objects[idx], best_effort) else {
            continue;
        };
        for key in keys {
            if let Some(entry) = xdic.get_ignore_ascii_case(key) {
                if let Some(filt_h) = entry.value_handle {
                    if let Some(&fidx) = by_handle.get(&filt_h.0) {
                        if let Some(filt_dict) =
                            decode_one_dictionary(&decoder, &index.objects[fidx], best_effort)
                        {
                            push_dict(&filt_dict, &mut entries, &mut seen);
                        }
                    }
                }
            }
        }
    }
    if entries.is_empty() {
        for obj in index.objects.iter() {
            let Some((_record, header)) =
                parse_record_and_header(&decoder, obj.offset, best_effort)?
            else {
                continue;
            };
            if !is_dictionary_type(header.type_code, &dynamic) {
                continue;
            }
            let Some(dict) = decode_one_dictionary(&decoder, obj, best_effort) else {
                continue;
            };
            for key in keys {
                if let Some(entry) = dict.get_ignore_ascii_case(key) {
                    if let Some(filt_h) = entry.value_handle {
                        if let Some(&fidx) = by_handle.get(&filt_h.0) {
                            if let Some(filt_dict) =
                                decode_one_dictionary(&decoder, &index.objects[fidx], best_effort)
                            {
                                push_dict(&filt_dict, &mut entries, &mut seen);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(entries)
}

fn discover_layer_filter_entries(path: &str) -> PyResult<Vec<(String, u64)>> {
    discover_named_dict_entries(path, ACAD_LAYERFILTERS_KEYS)
}

#[pyfunction]
pub fn decode_layer_filter_names(path: &str) -> PyResult<Vec<String>> {
    let entries = discover_layer_filter_entries(path)?;
    Ok(entries.into_iter().map(|(n, _)| n).collect())
}

#[pyfunction]
pub fn decode_layer_filter_xrecords(
    py: Python<'_>,
    path: &str,
) -> PyResult<Vec<LayerFilterRawRow>> {
    let entries = discover_layer_filter_entries(path)?;
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let index = decoder.build_object_index().map_err(to_py_err)?;

    let mut by_handle: HashMap<u64, usize> = HashMap::new();
    for (i, obj) in index.objects.iter().enumerate() {
        by_handle.insert(obj.handle.0, i);
    }

    let mut rows = Vec::with_capacity(entries.len());
    for (name, xh) in entries {
        let Some(&idx) = by_handle.get(&xh) else {
            continue;
        };
        let Some(xr) = decode_one_xrecord(&decoder, &index.objects[idx], best_effort) else {
            continue;
        };
        let (expression, flags) = extract_filter_expression_and_flags(&xr.groups);
        let mut groups_py = Vec::with_capacity(xr.groups.len());
        for g in &xr.groups {
            groups_py.push((g.code, xdata_value_to_py(py, &g.value)?));
        }
        let objids: Vec<u64> = xr.objid_handles.iter().map(|h| h.0).collect();
        rows.push((name, xh, expression, flags, groups_py, objids, xr.xdata));
    }
    Ok(rows)
}

fn nested_acly_children(
    decoder: &decoder::Decoder<'_>,
    by_handle: &HashMap<u64, usize>,
    index_objects: &[objects::ObjectRef],
    xr: &objects::XRecord,
    best_effort: bool,
) -> Vec<(String, u64)> {
    let Some(xdic_h) = xr.xdic_handle else {
        return Vec::new();
    };
    let Some(&xdic_idx) = by_handle.get(&xdic_h.0) else {
        return Vec::new();
    };
    let Some(xdic) = decode_one_dictionary(decoder, &index_objects[xdic_idx], best_effort) else {
        return Vec::new();
    };
    let mut child_dict_h = None;
    for key in ACLY_DICTIONARY_KEYS {
        if let Some(entry) = xdic.get_ignore_ascii_case(key) {
            if let Some(h) = entry.value_handle {
                child_dict_h = Some(h.0);
                break;
            }
        }
    }
    let dict_h = child_dict_h.unwrap_or(xdic_h.0);
    let Some(&didx) = by_handle.get(&dict_h) else {
        return Vec::new();
    };
    let Some(child_dict) = decode_one_dictionary(decoder, &index_objects[didx], best_effort) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in &child_dict.entries {
        if e.name.is_empty() {
            continue;
        }
        if let Some(h) = e.value_handle {
            out.push((e.name.clone(), h.0));
        }
    }
    out
}

fn walk_acly_tree(
    py: Python<'_>,
    decoder: &decoder::Decoder<'_>,
    by_handle: &HashMap<u64, usize>,
    index_objects: &[objects::ObjectRef],
    best_effort: bool,
    id_key: &str,
    handle: u64,
    depth: u32,
    out: &mut Vec<LayerFilterTreeNode>,
    visited: &mut HashSet<u64>,
) -> PyResult<()> {
    if handle == 0 || !visited.insert(handle) {
        return Ok(());
    }
    if depth > 32 {
        return Ok(());
    }
    let Some(&idx) = by_handle.get(&handle) else {
        return Ok(());
    };
    let Some(xr) = decode_one_xrecord(decoder, &index_objects[idx], best_effort) else {
        return Ok(());
    };
    let (name_opt, expression) = extract_acly_name_and_expression(&xr.groups);
    let name = name_opt.unwrap_or_else(|| {
        if !id_key.is_empty() {
            id_key.to_string()
        } else {
            format!("0x{handle:X}")
        }
    });
    let children = nested_acly_children(decoder, by_handle, index_objects, &xr, best_effort);
    let child_count = children.len() as u32;
    let mut groups_py = Vec::with_capacity(xr.groups.len());
    for g in &xr.groups {
        groups_py.push((g.code, xdata_value_to_py(py, &g.value)?));
    }
    out.push((
        name,
        handle,
        expression,
        id_key.to_string(),
        depth,
        child_count,
        groups_py,
    ));
    for (ck, ch) in children {
        walk_acly_tree(
            py,
            decoder,
            by_handle,
            index_objects,
            best_effort,
            &ck,
            ch,
            depth + 1,
            out,
            visited,
        )?;
    }
    Ok(())
}

/// Nested AcLy filter tree under ACLYDICTIONARY (pre-order flat list).
///
/// Each node: (name, handle, expression, id_key, depth, child_count, xdata_groups)
#[pyfunction]
pub fn decode_layer_filter_tree(
    py: Python<'_>,
    path: &str,
) -> PyResult<Vec<LayerFilterTreeNode>> {
    let roots = discover_named_dict_entries(path, ACLY_DICTIONARY_KEYS)?;
    if roots.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let index = decoder.build_object_index().map_err(to_py_err)?;

    let mut by_handle: HashMap<u64, usize> = HashMap::new();
    for (i, obj) in index.objects.iter().enumerate() {
        by_handle.insert(obj.handle.0, i);
    }

    let mut out = Vec::new();
    let mut visited: HashSet<u64> = HashSet::new();
    for (id_key, handle) in roots {
        walk_acly_tree(
            py,
            &decoder,
            &by_handle,
            &index.objects,
            best_effort,
            &id_key,
            handle,
            0,
            &mut out,
            &mut visited,
        )?;
    }
    Ok(out)
}
