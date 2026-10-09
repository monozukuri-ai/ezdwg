// ============================================================================
// Layer States — Phase 1 raw discovery API
//
// Walks LAYER_CONTROL → xdic → DICTIONARY "ACAD_LAYERSTATES" → XRECORDs
// and also falls back to a full scan of every DICTIONARY for the same key.
//
// Public functions (registered on ezdwg.raw):
//   decode_layer_state_names(path) -> list[str]
//   decode_layer_states(path) / decode_layer_state_xrecords(path)
//   decode_plotstyles(path) -> list[(name, handle)]
//   decode_layer_vp_overrides(path) -> list[(layer_h, name, prop, vp_h, value)]
// ============================================================================

/// One raw layer-state row (Phase 1 contract).
///
/// Shape (frozen after first successful dump of an AC1032 fixture):
///   (name, xrecord_handle, mask, description, xdata_groups, objid_handles, xdata_raw)
/// where:
///   name:            str
///   xrecord_handle:  int (absolute handle of the XRECORD)
///   mask:            Optional[int]  (group 90/91 if present in xdata)
///   description:     Optional[str] (group 301/1 if present)
///   xdata_groups:    list[(code: int, value: Any)]
///   objid_handles:   list[int]
///   xdata_raw:       bytes
type LayerStateRawRow = (
    String,               // name
    u64,                  // xrecord_handle
    Option<i64>,          // mask
    Option<String>,       // description
    Vec<(i16, PyObject)>, // xdata groups (code, py value)
    Vec<u64>,             // objid handles
    Vec<u8>,              // raw xdata bytes
);

fn extract_mask_and_description(groups: &[objects::XDataGroup]) -> (Option<i64>, Option<String>) {
    let header_end = groups
        .iter()
        .position(|g| matches!(g.code, 8 | 330))
        .unwrap_or(groups.len());
    let groups = &groups[..header_end];
    // LibreDWG / .las layout observed on AC1032 layer-state XRECORDs:
    //   91  global LayerStateMasks (prefer over per-layer 90)
    //   301 description (state-level only; group 1 is per-layer plotstyle)
    let mut mask = None;
    let mut description = None;
    for g in groups {
        if g.code == 91 && mask.is_none() {
            match &g.value {
                objects::XDataValue::Int32(v) => mask = Some(*v as i64),
                objects::XDataValue::Int16(v) => mask = Some(*v as i64),
                objects::XDataValue::Int64(v) => mask = Some(*v),
                _ => {}
            }
        }
    }
    if mask.is_none() {
        for g in groups {
            if g.code == 90 {
                match &g.value {
                    objects::XDataValue::Int32(v) => {
                        mask = Some(*v as i64);
                        break;
                    }
                    objects::XDataValue::Int16(v) => {
                        mask = Some(*v as i64);
                        break;
                    }
                    objects::XDataValue::Int64(v) => {
                        mask = Some(*v);
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    for g in groups {
        if matches!(g.code, 301 | 300) {
            if let objects::XDataValue::String(s) = &g.value {
                // Prefer 301; skip empty so we do not invent a description.
                if !s.is_empty() {
                    description = Some(s.clone());
                }
                break;
            }
        }
    }
    (mask, description)
}

fn discover_layer_state_entries(path: &str) -> PyResult<Vec<(String, u64)>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;

    let mut by_handle: HashMap<u64, usize> = HashMap::new();
    for (i, obj) in index.objects.iter().enumerate() {
        by_handle.insert(obj.handle.0, i);
    }

    let mut states: Vec<(String, u64)> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();

    // Include every dictionary entry. Soft-deleted XRECORDs (handle not in
    // the object map) still contribute their name so callers see all keys
    // LibreDWG reports; decode_layer_states skips body parse when missing.
    // Entries whose handle stream failed (value_handle=None) become name-only
    // rows with synthetic handle 0 so the name is not dropped.
    let push_states_dict = |states_dict: &objects::Dictionary,
                            states: &mut Vec<(String, u64)>,
                            seen: &mut HashSet<u64>| {
        for e in &states_dict.entries {
            if e.name.is_empty() {
                continue;
            }
            if let Some(xh) = e.value_handle {
                if seen.insert(xh.0) {
                    states.push((e.name.clone(), xh.0));
                }
            } else if !states.iter().any(|(n, _)| n == &e.name) {
                states.push((e.name.clone(), 0));
            }
        }
    };

    // Path A: LAYER_CONTROL → xdic
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
        for key in ACAD_LAYERSTATES_KEYS {
            if let Some(entry) = xdic.get_ignore_ascii_case(key) {
                if let Some(states_h) = entry.value_handle {
                    if let Some(&sidx) = by_handle.get(&states_h.0) {
                        if let Some(states_dict) =
                            decode_one_dictionary(&decoder, &index.objects[sidx], best_effort)
                        {
                            push_states_dict(&states_dict, &mut states, &mut seen);
                        }
                    }
                }
            }
        }
    }
    // Path B: scan all dictionaries for ACAD_LAYERSTATES key
    if states.is_empty() {
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
            for key in ACAD_LAYERSTATES_KEYS {
                if let Some(entry) = dict.get_ignore_ascii_case(key) {
                    if let Some(states_h) = entry.value_handle {
                        if let Some(&sidx) = by_handle.get(&states_h.0) {
                            if let Some(states_dict) =
                                decode_one_dictionary(&decoder, &index.objects[sidx], best_effort)
                            {
                                push_states_dict(&states_dict, &mut states, &mut seen);
                            }
                        }
                    }
                }
            }
        }
    }
    // Path C (UTF-16 body scan + name-plausibility filter) removed: Path A/B
    // are exact once XRECORD preamble + data-section counts are correct.

    Ok(states)
}

/// List layer-state names present in the drawing (cheap path).
#[pyfunction]
pub fn decode_layer_state_names(path: &str) -> PyResult<Vec<String>> {
    let entries = discover_layer_state_entries(path)?;
    Ok(entries.into_iter().map(|(n, _)| n).collect())
}

/// Full raw rows for reverse-engineering / Phase 2 structured parse.
///
/// Each row:
///   (name, xrecord_handle, mask, description, xdata_groups, objid_handles, xdata_raw)
#[pyfunction]
pub fn decode_layer_state_xrecords(py: Python<'_>, path: &str) -> PyResult<Vec<LayerStateRawRow>> {
    let entries = discover_layer_state_entries(path)?;
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
            // Soft-deleted XRECORD: name known, no body in the object map.
            // Callers (Phase 2 table) materialise name-only rows separately.
            continue;
        };
        let Some(xr) = decode_one_xrecord(&decoder, &index.objects[idx], best_effort) else {
            // Deterministic decode failed — do not fall back to body scanning.
            continue;
        };
        let (mask, description) = extract_mask_and_description(&xr.groups);
        let mut groups_py = Vec::with_capacity(xr.groups.len());
        for g in &xr.groups {
            groups_py.push((g.code, xdata_value_to_py(py, &g.value)?));
        }
        let objids: Vec<u64> = xr.objid_handles.iter().map(|h| h.0).collect();
        rows.push((
            name,
            xh,
            mask,
            description,
            groups_py,
            objids,
            xr.xdata, // real xdata payload (not the whole object body)
        ));
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Plot-style name table (ACAD_PLOTSTYLENAME dictionary)
// ---------------------------------------------------------------------------

const ACAD_PLOTSTYLENAME_KEYS: &[&str] = &["ACAD_PLOTSTYLENAME", "ACAD_PLOTSTYLE"];

/// Discover name → handle entries under the drawing's plot-style dictionary.
///
/// Path A: named-objects dictionary (or any DICTIONARY) key `ACAD_PLOTSTYLENAME`
/// pointing at a DICTIONARY / DICTIONARYWDFLT whose items are the plot styles.
/// Path B: fall back to scanning for DICTIONARYWDFLT objects that look like the
/// plot-style table (typically a single `"Normal"` entry in CTB mode).
///
/// Returns `(name, value_handle)` pairs. Handles may point at PLACEHOLDER or
/// PLOTSTYLENAME objects; callers map names for layer-state diff/apply.
fn discover_plotstyle_entries(path: &str) -> PyResult<Vec<(String, u64)>> {
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

    let push_dict =
        |dict: &objects::Dictionary, entries: &mut Vec<(String, u64)>, seen: &mut HashSet<u64>| {
            for e in &dict.entries {
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

    // Path A: any DICTIONARY containing ACAD_PLOTSTYLENAME → child dict entries
    for obj in index.objects.iter() {
        let Some((_record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !is_dictionary_type(header.type_code, &dynamic) {
            continue;
        }
        let Some(dict) = decode_one_dictionary(&decoder, obj, best_effort) else {
            continue;
        };
        for key in ACAD_PLOTSTYLENAME_KEYS {
            if let Some(entry) = dict.get_ignore_ascii_case(key) {
                if let Some(ps_h) = entry.value_handle {
                    if let Some(&pidx) = by_handle.get(&ps_h.0) {
                        if let Some(ps_dict) =
                            decode_one_dictionary(&decoder, &index.objects[pidx], best_effort)
                        {
                            push_dict(&ps_dict, &mut entries, &mut seen);
                        }
                    }
                }
            }
        }
    }

    // Path B: DICTIONARYWDFLT / plot-style dict not reached via Path A
    // (e.g. header handle points here directly). Prefer objects whose
    // sole/first entry is "Normal" — the CTB default table shape.
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
            let has_normal = dict
                .entries
                .iter()
                .any(|e| e.name.eq_ignore_ascii_case("Normal"));
            if has_normal {
                push_dict(&dict, &mut entries, &mut seen);
                break;
            }
        }
    }

    Ok(entries)
}

/// List plot-style dictionary entries as `(name, handle)` pairs.
///
/// In color-dependent (CTB / PSTYLEMODE=1) drawings the table usually has a
/// single `"Normal"` entry pointing at an `ACDBPLACEHOLDER`. Named plot-style
/// (STB) drawings list each style name. Layer-state XRECORD group 1 stores
/// plot-style **strings**; resolve them against this table for diff/apply.
#[pyfunction]
pub fn decode_plotstyles(path: &str) -> PyResult<Vec<(String, u64)>> {
    discover_plotstyle_entries(path)
}

// ---------------------------------------------------------------------------
// Viewport layer property overrides (LAYER xdict ADSK_XREC_LAYER_*_OVR)
// ---------------------------------------------------------------------------
//
// VP Freeze lives on the VIEWPORT entity. VP Color / Linetype / Lineweight /
// Transparency / Plot Style live on each LAYER's extension dictionary as
// XRECORDs. See ezdwg_vplayer_scope.md §13.
//
// XRECORD xdata shape (repeated per viewport):
//   102 "{ADSK_LYR_COLOR_OVERRIDE" | …LINETYPE… | …
//   335 <viewport handle>
//   420|343|370|440|…  <value>
//   102 "}"

const OVR_KEY_COLOR: &str = "ADSK_XREC_LAYER_COLOR_OVR";
const OVR_KEY_LINETYPE: &str = "ADSK_XREC_LAYER_LINETYPE_OVR";
const OVR_KEY_LINEWT: &str = "ADSK_XREC_LAYER_LINEWT_OVR";
const OVR_KEY_ALPHA: &str = "ADSK_XREC_LAYER_ALPHA_OVR";
const OVR_KEY_PLOTSTYLE: &str = "ADSK_XREC_LAYER_PLOTSTYLE_OVR";

/// One VP property override row:
/// `(layer_handle, layer_name, property, viewport_handle, value)`.
///
/// `property` is one of `"color"`, `"linetype"`, `"lineweight"`,
/// `"transparency"`, `"plot_style"`. `value` is a signed int (encoded color,
/// linetype handle, lineweight index, transparency, or plot-style handle).
type LayerVpOverrideRow = (u64, String, String, u64, i64);

#[cfg(test)]
mod layer_state_header_tests {
    use super::*;

    #[test]
    fn per_layer_flags_do_not_become_state_mask() {
        let groups = vec![
            objects::XDataGroup {
                code: 330,
                value: objects::XDataValue::Int64(16),
            },
            objects::XDataGroup {
                code: 90,
                value: objects::XDataValue::Int32(8),
            },
        ];
        assert_eq!(extract_mask_and_description(&groups), (None, None));
    }
}

fn ovr_property_for_dict_key(key: &str) -> Option<&'static str> {
    let k = key.to_ascii_uppercase();
    if k == OVR_KEY_COLOR {
        Some("color")
    } else if k == OVR_KEY_LINETYPE {
        Some("linetype")
    } else if k == OVR_KEY_LINEWT {
        Some("lineweight")
    } else if k == OVR_KEY_ALPHA {
        Some("transparency")
    } else if k == OVR_KEY_PLOTSTYLE {
        Some("plot_style")
    } else {
        None
    }
}

fn xdata_i64(value: &objects::XDataValue) -> Option<i64> {
    use objects::XDataValue::*;
    match value {
        Int16(v) => Some(*v as i64),
        Int32(v) => Some(*v as i64),
        Int64(v) => Some(*v),
        Handle(href) => Some(href.value as i64),
        Real(v) => Some(*v as i64),
        _ => None,
    }
}

fn xdata_handle_u64(value: &objects::XDataValue) -> Option<u64> {
    use objects::XDataValue::*;
    match value {
        Handle(href) => Some(href.value as u64),
        Int32(v) if *v >= 0 => Some(*v as u64),
        Int64(v) if *v >= 0 => Some(*v as u64),
        _ => None,
    }
}

/// Parse one override XRECORD into `(viewport_handle, value)` pairs.
fn parse_ovr_xrecord_groups(groups: &[objects::XDataGroup], property: &str) -> Vec<(u64, i64)> {
    // Value group codes per property (ezdxf / LibreDWG observations).
    let value_codes: &[i16] = match property {
        "color" => &[420, 62],
        "linetype" => &[343, 6],
        "lineweight" => &[370, 371],
        "transparency" => &[440],
        "plot_style" => &[390, 1],
        _ => &[],
    };

    let mut out: Vec<(u64, i64)> = Vec::new();
    let mut in_block = false;
    let mut vp: Option<u64> = None;
    let mut val: Option<i64> = None;

    for g in groups {
        match g.code {
            102 => {
                if let objects::XDataValue::String(s) = &g.value {
                    if s.starts_with('{') {
                        in_block = true;
                        vp = None;
                        val = None;
                    } else if s.starts_with('}') {
                        if in_block {
                            if let (Some(vph), Some(v)) = (vp, val) {
                                out.push((vph, v));
                            }
                        }
                        in_block = false;
                        vp = None;
                        val = None;
                    }
                }
            }
            335 if in_block => {
                if let Some(h) = xdata_handle_u64(&g.value) {
                    vp = Some(h);
                }
            }
            code if in_block && value_codes.contains(&code) => {
                if val.is_none() {
                    val = xdata_i64(&g.value);
                }
            }
            _ => {}
        }
    }
    // Trailing block without closing "}"
    if in_block {
        if let (Some(vph), Some(v)) = (vp, val) {
            out.push((vph, v));
        }
    }
    out
}

/// Decode all LAYER xdict viewport property overrides in `path`.
///
/// Returns rows `(layer_handle, layer_name, property, viewport_handle, value)`.
/// Empty when the drawing has no ADSK override XRECORDs.
#[pyfunction]
pub fn decode_layer_vp_overrides(path: &str) -> PyResult<Vec<LayerVpOverrideRow>> {
    let records = get_all_layer_records(path)?;
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let index = decoder.build_object_index().map_err(to_py_err)?;

    let mut by_handle: HashMap<u64, usize> = HashMap::new();
    for (i, obj) in index.objects.iter().enumerate() {
        by_handle.insert(obj.handle.0, i);
    }

    let mut rows: Vec<LayerVpOverrideRow> = Vec::new();

    for rec in records.iter() {
        let Some(xdic_h) = rec.xdic_handle.filter(|&h| h != 0) else {
            continue;
        };
        let Some(&xdic_idx) = by_handle.get(&xdic_h) else {
            continue;
        };
        let Some(dict) = decode_one_dictionary(&decoder, &index.objects[xdic_idx], best_effort)
        else {
            continue;
        };

        let layer_name = rec.name.clone().unwrap_or_default();
        for entry in &dict.entries {
            let Some(prop) = ovr_property_for_dict_key(&entry.name) else {
                continue;
            };
            let Some(xr_h) = entry.value_handle else {
                continue;
            };
            let Some(&xr_idx) = by_handle.get(&xr_h.0) else {
                continue;
            };
            let Some(xr) = decode_one_xrecord(&decoder, &index.objects[xr_idx], best_effort) else {
                continue;
            };
            for (vp_h, value) in parse_ovr_xrecord_groups(&xr.groups, prop) {
                rows.push((
                    rec.handle,
                    layer_name.clone(),
                    prop.to_string(),
                    vp_h,
                    value,
                ));
            }
        }
    }

    Ok(rows)
}
