// ============================================================================
// LAYER table parsing.
//
// One deterministic pass per record (name + color + flags together, real
// field order per dwg.spec's COMMON_TABLE_FLAGS + LAYER fields), cached
// per-file so the four public functions below share a single real parse.
// No scoring/guessing in this section -- ground-truth verified for
// R2013/R2018 (LibreDWG TRACE + ACadSharp cross-check), spec-derived for
// R14/R2000/R2004/R2007 (see per-branch FIXMEs for what's unverified).
//
// Entity->layer handle *association* (a different, unverified problem --
// resolving which layer an entity is on, not layer table records) is
// relocated to the bottom of this file for organization, logic unchanged.
// ============================================================================


// --- Cache: one real file parse serves all public LAYER functions below ---
// Key + FIFO policy live in cache.rs (FileKey / PathFifoCache). Do not add
// another process-global static for a second table; extend PathFifoCache
// usage (or a future FileDecodeState) instead.

fn layer_records_cache() -> &'static PathFifoCache<Vec<LayerRecord>> {
    static CACHE: OnceLock<PathFifoCache<Vec<LayerRecord>>> = OnceLock::new();
    CACHE.get_or_init(PathFifoCache::new)
}

/// Called from clear_decode_cache() in cache.rs.
fn clear_layer_records_cache() {
    layer_records_cache().clear();
}

/// Every LAYER record in `path`, decoded once and cached by (path, size,
/// mtime) -- calling more than one of the four functions below on the same
/// file triggers exactly one real parse, not one each.
fn get_all_layer_records(path: &str) -> PyResult<Arc<Vec<LayerRecord>>> {
    let key = file_key(path);
    if let Some(records) = layer_records_cache().get(&key) {
        return Ok(records);
    }

    let mut records_vec = parse_all_layer_records(path)?;
    let _ = resolve_layer_names_from_tables(path, &mut records_vec);
    let records = Arc::new(records_vec);
    layer_records_cache().insert(key, records.clone());
    Ok(records)
}

fn parse_all_layer_records(path: &str) -> PyResult<Vec<LayerRecord>> {
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
        match parse_layer_record(&record, &header, &mut reader, decoder.version(), obj.handle.0) {
            Ok(rec) => result.push(rec),
            // A LAYER object that's unrecoverably corrupt still can't
            // produce a row -- there's no field left to anchor one to.
            // Everything else (a field that's merely unresolved) is
            // handled inside parse_layer_record as Option::None instead.
            Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
            Err(err) => return Err(to_py_err(err)),
        }
    }

    Ok(result)
}

// --- Public API (signatures unchanged) --------------------------------------

#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_colors(path: &str, limit: Option<usize>) -> PyResult<Vec<LayerColorRow>> {
    let records = get_all_layer_records(path)?;
    Ok(records
        .iter()
        .take(limit.unwrap_or(usize::MAX))
        .map(|r| (r.handle, r.color_index, r.true_color))
        .collect())
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_flags(path: &str, limit: Option<usize>) -> PyResult<Vec<LayerFlagsRow>> {
    let records = get_all_layer_records(path)?;
    Ok(records
        .iter()
        .take(limit.unwrap_or(usize::MAX))
        .map(|r| {
            (
                r.handle,
                r.frozen,
                r.off,
                r.frozen_in_new,
                r.locked,
                r.plotflag,
                r.lineweight_idx,
            )
        })
        .collect())
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_color_details(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<LayerColorDetailRow>> {
    let records = get_all_layer_records(path)?;
    Ok(records
        .iter()
        .take(limit.unwrap_or(usize::MAX))
        .map(|r| {
            (
                r.handle,
                r.color_index,
                r.true_color,
                r.color_name.clone(),
                r.book_name.clone(),
            )
        })
        .collect())
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_names(path: &str, limit: Option<usize>) -> PyResult<Vec<LayerNameRow>> {
    let records = get_all_layer_records(path)?;
    // Handle is the canonical reference (every LAYER object in the file
    // gets a row) -- a layer whose name couldn't be resolved now returns
    // "" instead of vanishing from the list. See FIXME on LayerRecord::name.
    Ok(records
        .iter()
        .take(limit.unwrap_or(usize::MAX))
        .map(|r| (r.handle, r.name.clone().unwrap_or_default()))
        .collect())
}

// --- Core record -------------------------------------------------------------

/// One LAYER table record, fully decoded in a single pass.
struct LayerRecord {
    handle: u64,
    name: Option<String>,
    color_index: u16,
    true_color: Option<u32>,
    color_name: Option<String>,
    book_name: Option<String>,
    frozen: bool,
    off: bool,
    frozen_in_new: bool,
    locked: bool,
    plotflag: bool,
    lineweight_idx: u8,
    eed: Vec<EedBlock>,
    owner_handle: Option<u64>,
    xdic_handle: Option<u64>,
    plotstyle_handle: Option<u64>,
    material_handle: Option<u64>,
    ltype_handle: Option<u64>,
    visualstyle_handle: Option<u64>,
    linetype: Option<String>,
    eed_app_names: Vec<Option<String>>,
    description: Option<String>,
    handle_warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct EedBlock {
    size: u16,
    app_handle: u64,
    items: Vec<EedItem>,
}

#[derive(Debug, Clone)]
struct EedItem {
    code: u8,
    kind: EedValue,
}

#[derive(Debug, Clone)]
enum EedValue {
    String(String),
    Control(u8),
    LayerRef(u64),
    Binary(Vec<u8>),
    EntityRef(u64),
    Point(f64, f64, f64),
    Real(f64),
    Short(i16),
    Long(i32),
    Unknown(Vec<u8>),
}

/// Single walk of one LAYER object's bitstream: prologue, name, xref
/// fields, state flags, color -- in real field order (dwg.spec
/// COMMON_TABLE_FLAGS + LAYER's own fields), not the old two-separate-
/// passes-that-can-drift design.
fn decode_eed(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
) -> crate::core::result::Result<Vec<EedBlock>> {
    let mut blocks = Vec::new();
    let mut ext_size = reader.read_bs()?;
    while ext_size > 0 {
        let app_handle = reader.read_h()?.value;
        let raw = reader.read_rcs(ext_size as usize)?;
        let items = parse_eed_payload(&raw, version);
        blocks.push(EedBlock { size: ext_size, app_handle, items });
        ext_size = reader.read_bs()?;
    }
    Ok(blocks)
}

fn parse_eed_payload(data: &[u8], version: &version::DwgVersion) -> Vec<EedItem> {
    let mut items = Vec::new();
    let mut pos = 0usize;
    let unicode = matches!(
        version,
        version::DwgVersion::R2007
            | version::DwgVersion::R2010
            | version::DwgVersion::R2013
            | version::DwgVersion::R2018
    );
    while pos < data.len() {
        let code = data[pos];
        pos += 1;
        let remaining = data.len() - pos;
        let item = match code {
            0 if unicode => {
                if remaining < 2 { break; }
                let len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
                pos += 2;
                let byte_len = len.saturating_mul(2);
                if pos + byte_len > data.len() { break; }
                let mut units = Vec::with_capacity(len);
                for i in 0..len {
                    units.push(u16::from_le_bytes([data[pos + i * 2], data[pos + i * 2 + 1]]));
                }
                pos += byte_len;
                EedItem { code, kind: EedValue::String(String::from_utf16_lossy(&units)) }
            }
            0 => {
                if remaining < 1 { break; }
                let len = data[pos] as usize;
                pos += 1;
                if pos + 2 + len > data.len() { break; }
                pos += 2;
                let value = String::from_utf8_lossy(&data[pos..pos + len]).into_owned();
                pos += len;
                EedItem { code, kind: EedValue::String(value) }
            }
            2 => {
                if remaining < 1 { break; }
                let v = data[pos]; pos += 1;
                EedItem { code, kind: EedValue::Control(v) }
            }
            3 | 5 => {
                if remaining < 8 { break; }
                let v = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                pos += 8;
                EedItem { code, kind: if code == 3 { EedValue::LayerRef(v) } else { EedValue::EntityRef(v) } }
            }
            4 => {
                if remaining < 1 { break; }
                let len = data[pos] as usize; pos += 1;
                if pos + len > data.len() { break; }
                let bytes = data[pos..pos + len].to_vec(); pos += len;
                EedItem { code, kind: EedValue::Binary(bytes) }
            }
            10 | 11 | 12 | 13 => {
                if remaining < 24 { break; }
                let x = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                let y = f64::from_le_bytes(data[pos + 8..pos + 16].try_into().unwrap());
                let z = f64::from_le_bytes(data[pos + 16..pos + 24].try_into().unwrap());
                pos += 24;
                EedItem { code, kind: EedValue::Point(x, y, z) }
            }
            40 | 41 | 42 => {
                if remaining < 8 { break; }
                let v = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                pos += 8;
                EedItem { code, kind: EedValue::Real(v) }
            }
            70 => {
                if remaining < 2 { break; }
                let v = i16::from_le_bytes([data[pos], data[pos + 1]]); pos += 2;
                EedItem { code, kind: EedValue::Short(v) }
            }
            71 => {
                if remaining < 4 { break; }
                let v = i32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()); pos += 4;
                EedItem { code, kind: EedValue::Long(v) }
            }
            _ => {
                let rest = data[pos..].to_vec(); pos = data.len();
                EedItem { code, kind: EedValue::Unknown(rest) }
            }
        };
        items.push(item);
    }
    items
}

#[derive(Default, Debug, Clone)]
struct LayerHandleStream {
    owner: Option<u64>,
    xdic: Option<u64>,
    plotstyle: Option<u64>,
    material: Option<u64>,
    ltype: Option<u64>,
    visualstyle: Option<u64>,
    /// Structured notes when a slot was cleared after type validation.
    warnings: Vec<String>,
}

/// Resolve the single, deterministic bit position where the LAYER handle
/// stream starts. No candidate sweeps.
///
/// Geometry (LibreDWG `obj_handle_stream` / ODA bitsize):
/// * **R2010+** — `data_start + data_size*8 - handle_stream_size_bits`
///   (`resolve_r2010_object_data_end_bit_exact`).
/// * **R14–R2007** — the object **bitsize** field (RL): absolute bit index
///   from the start of the object where handle data begins
///   (`obj->hdlpos = obj->bitsize`). R2000–R2007 store bitsize immediately
///   after the type prefix; R14 stores it after EED (same absolute meaning).
///
/// `handlestream_size = size*8 - bitsize` is not required to *start* the
/// stream; it only bounds available bits.
fn layer_handle_stream_start_bit(
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
    obj_size: Option<u32>,
) -> Option<u32> {
    use version::DwgVersion::*;
    match version {
        R2010 | R2013 | R2018 => resolve_r2010_object_data_end_bit_exact(api_header),
        R14 | R2000 | R2004 | R2007 => {
            let bitsize = obj_size.filter(|&s| s > 0)?;
            // LibreDWG rejects bitsize > size*8; mirror that guard when size
            // is known. Prefer exact bitsize over any candidate search.
            let total_bits = api_header.data_size.saturating_mul(8);
            if total_bits > 0 && bitsize >= total_bits {
                return None;
            }
            Some(bitsize)
        }
        _ => None,
    }
}

fn read_layer_handle_stream(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
    base_handle: u64,
    num_reactors: u32,
    is_xdic_missing: bool,
    obj_size: Option<u32>,
) -> LayerHandleStream {
    let mut out = LayerHandleStream::default();
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return out;
    }

    let Some(start) = layer_handle_stream_start_bit(api_header, version, obj_size) else {
        return out;
    };
    reader.set_bit_pos(start);

    // Fixed slot order (common_object_handle_data + LAYER extras), versioned:
    //   owner, reactors[num], [xdic if present], [plotstyle R2000+],
    //   [material R2007+], ltype, [visualstyle R2013+]
    // Leading zeros in the post-xdic tail are padding nulls in the stream,
    // not alternate candidates — consume them without rescoring.
    let mut refs: Vec<u64> = Vec::new();
    for _ in 0..16 {
        match entities::common::read_handle_reference(&mut reader, base_handle) {
            Ok(h) => refs.push(h),
            Err(_) => break,
        }
    }

    let mut i = 0usize;
    let next = |i: &mut usize| -> Option<u64> {
        if *i < refs.len() {
            let v = refs[*i];
            *i += 1;
            Some(v)
        } else {
            None
        }
    };

    out.owner = next(&mut i).filter(|&h| h != 0);
    for _ in 0..num_reactors {
        let _ = next(&mut i);
    }
    if !is_xdic_missing {
        out.xdic = next(&mut i).filter(|&h| h != 0);
    }

    let mut rest: Vec<u64> = refs[i..].to_vec();
    let take = |rest: &mut Vec<u64>| -> Option<u64> {
        while rest.first().copied() == Some(0) {
            rest.remove(0);
        }
        if rest.is_empty() {
            None
        } else {
            Some(rest.remove(0))
        }
    };

    use version::DwgVersion::*;
    // plotstyle: present from R2000 onward in the LAYER handle tail
    if matches!(
        version,
        R2000 | R2004 | R2007 | R2010 | R2013 | R2018
    ) {
        out.plotstyle = take(&mut rest);
    }
    // material: R2007+
    if matches!(version, R2007 | R2010 | R2013 | R2018) {
        out.material = take(&mut rest);
    }
    out.ltype = take(&mut rest);
    if matches!(version, R2013 | R2018) {
        out.visualstyle = take(&mut rest);
    }
    out
}

/// Fixed type-code / DXF-name expectations for LAYER handle slots.
const LAYER_CONTROL_TYPE: u16 = 0x32;
const LTYPE_TYPE: u16 = 0x39;

fn type_name_matches(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

/// After positional decode: drop any handle whose object type does not match
/// the slot. Never pick an alternate ref — mismatch → None + warning.
fn validate_layer_handle_types(
    records: &mut [LayerRecord],
    type_map: &HashMap<u64, (u16, String)>,
) {
    for r in records.iter_mut() {
        if let Some(h) = r.owner_handle {
            match type_map.get(&h) {
                Some((code, name))
                    if *code == LAYER_CONTROL_TYPE || type_name_matches(name, "LAYER_CONTROL") => {}
                Some((code, name)) => {
                    r.owner_handle = None;
                    r.handle_warnings.push(format!(
                        "owner handle {h} type {code}/{name} is not LAYER_CONTROL"
                    ));
                }
                None => {
                    // Unknown handle: keep value (may be external/soft-pointer
                    // not present in the object map) without inventing a substitute.
                }
            }
        }

        if let Some(h) = r.ltype_handle {
            match type_map.get(&h) {
                Some((code, name))
                    if *code == LTYPE_TYPE || type_name_matches(name, "LTYPE") => {}
                Some((code, name)) => {
                    r.ltype_handle = None;
                    r.linetype = None;
                    r.handle_warnings.push(format!(
                        "ltype handle {h} type {code}/{name} is not LTYPE"
                    ));
                }
                None => {}
            }
        }

        if let Some(h) = r.material_handle {
            match type_map.get(&h) {
                Some((_, name)) if type_name_matches(name, "MATERIAL") => {}
                Some((code, name)) => {
                    r.material_handle = None;
                    r.handle_warnings.push(format!(
                        "material handle {h} type {code}/{name} is not MATERIAL"
                    ));
                }
                None => {}
            }
        }

        if let Some(h) = r.plotstyle_handle {
            match type_map.get(&h) {
                Some((_, name))
                    if type_name_matches(name, "PLOTSTYLENAME")
                        || type_name_matches(name, "PLACEHOLDER")
                        || name.to_ascii_uppercase().contains("PLOTSTYLE") => {}
                Some((code, name)) => {
                    // Soft pointers to missing dictionary entries are common;
                    // only clear when the target is a *known* wrong type.
                    if *code != 0 {
                        r.plotstyle_handle = None;
                        r.handle_warnings.push(format!(
                            "plotstyle handle {h} type {code}/{name} is not a plotstyle"
                        ));
                    }
                }
                None => {}
            }
        }

        if let Some(h) = r.visualstyle_handle {
            match type_map.get(&h) {
                Some((_, name)) if type_name_matches(name, "VISUALSTYLE") => {}
                Some((code, name)) => {
                    r.visualstyle_handle = None;
                    r.handle_warnings.push(format!(
                        "visualstyle handle {h} type {code}/{name} is not VISUALSTYLE"
                    ));
                }
                None => {}
            }
        }
    }
}

fn build_object_type_map(path: &str) -> PyResult<HashMap<u64, (u16, String)>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut map = HashMap::new();
    for obj in index.objects.iter() {
        let Some((_record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        let name = dynamic_types
            .get(&header.type_code)
            .cloned()
            .unwrap_or_else(|| objects::object_type_name(header.type_code));
        map.insert(obj.handle.0, (header.type_code, name));
    }
    Ok(map)
}

fn parse_table_object_name(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
    _expected_handle: u64,
) -> Option<String> {
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version).ok()?;
    use version::DwgVersion::*;
    let obj_size = if matches!(version, R2000 | R2004 | R2007) {
        reader.read_rl(Endian::Little).ok()
    } else { None };
    let _h = reader.read_h().ok()?;
    let mut ext = reader.read_bs().ok()?;
    while ext > 0 {
        let _ = reader.read_h().ok()?;
        let _ = reader.read_rcs(ext as usize).ok()?;
        ext = reader.read_bs().ok()?;
    }
    if matches!(version, R14) { let _ = reader.read_rl(Endian::Little).ok()?; }
    let _ = reader.read_bl().ok()?;
    if !matches!(version, R14 | R2000) { let _ = reader.read_b().ok()?; }
    if matches!(version, R2013 | R2018) { let _ = reader.read_b().ok()?; }
    if matches!(version, R14 | R2000 | R2004) {
        let name = reader.read_tv().ok()?;
        let t = name.trim().to_string();
        return if t.is_empty() { None } else { Some(t) };
    }
    let exact_end = if matches!(version, R2007) { obj_size } else { None };
    for (mut sr, _) in locate_layer_string_stream_starts(record, api_header, version, exact_end) {
        if let Ok(n) = sr.read_tu() {
            let t = n.trim().to_string();
            if !t.is_empty() { return Some(t); }
        }
    }
    None
}

fn build_table_name_map(path: &str, type_code: u16, type_name: &str) -> PyResult<HashMap<u64, String>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut map = HashMap::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)? else { continue; };
        if !matches_type_name(header.type_code, type_code, type_name, &dynamic_types) { continue; }
        if let Some(name) = parse_table_object_name(&record, &header, decoder.version(), obj.handle.0) {
            map.insert(obj.handle.0, name);
        }
    }
    Ok(map)
}

fn resolve_layer_names_from_tables(path: &str, records: &mut [LayerRecord]) -> PyResult<()> {
    // Type-check handle slots before name resolution (2c).
    if let Ok(type_map) = build_object_type_map(path) {
        validate_layer_handle_types(records, &type_map);
    }

    let ltype_map = build_table_name_map(path, 0x39, "LTYPE")?;
    let appid_map = build_table_name_map(path, 0x43, "APPID")?;
    for r in records.iter_mut() {
        if let Some(h) = r.ltype_handle {
            r.linetype = ltype_map.get(&h).cloned();
        }
        r.eed_app_names = r.eed.iter().map(|b| appid_map.get(&b.app_handle).cloned()).collect();
        for (block, app_name) in r.eed.iter().zip(r.eed_app_names.iter()) {
            if app_name.as_deref() != Some("AcAecLayerStandard") { continue; }
            let strings: Vec<&str> = block.items.iter().filter_map(|it| match &it.kind {
                EedValue::String(s) => Some(s.as_str()),
                _ => None,
            }).collect();
            if strings.len() >= 2 && !strings[1].is_empty() {
                r.description = Some(strings[1].to_string());
            } else if strings.len() == 1 && !strings[0].is_empty() {
                r.description = Some(strings[0].to_string());
            }
        }
    }
    Ok(())
}

fn eed_value_to_py(py: Python<'_>, value: &EedValue) -> PyResult<PyObject> {
    match value {
        EedValue::String(s) => Ok(s.clone().into_py(py)),
        EedValue::Control(v) => Ok((*v).into_py(py)),
        EedValue::LayerRef(v) | EedValue::EntityRef(v) => Ok((*v).into_py(py)),
        EedValue::Binary(b) => Ok(pyo3::types::PyBytes::new_bound(py, b).into_py(py)),
        EedValue::Point(x, y, z) => Ok((*x, *y, *z).into_py(py)),
        EedValue::Real(v) => Ok((*v).into_py(py)),
        EedValue::Short(v) => Ok((*v as i64).into_py(py)),
        EedValue::Long(v) => Ok((*v as i64).into_py(py)),
        EedValue::Unknown(b) => Ok(pyo3::types::PyBytes::new_bound(py, b).into_py(py)),
    }
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_eed(
    py: Python<'_>,
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, Vec<(u16, u64, Option<String>, Vec<(u8, PyObject)>)>)>> {
    let records = get_all_layer_records(path)?;
    let mut out = Vec::new();
    for r in records.iter().take(limit.unwrap_or(usize::MAX)) {
        let mut blocks = Vec::new();
        for (i, block) in r.eed.iter().enumerate() {
            let mut items = Vec::new();
            for item in &block.items {
                items.push((item.code, eed_value_to_py(py, &item.kind)?));
            }
            let app_name = r.eed_app_names.get(i).cloned().flatten();
            blocks.push((block.size, block.app_handle, app_name, items));
        }
        out.push((r.handle, blocks));
    }
    Ok(out)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_handles(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, Option<u64>, Option<u64>, Option<u64>, Option<u64>, Option<u64>, Option<u64>, Option<String>)>> {
    let records = get_all_layer_records(path)?;
    Ok(records.iter().take(limit.unwrap_or(usize::MAX)).map(|r| {
        (r.handle, r.owner_handle, r.xdic_handle, r.plotstyle_handle, r.material_handle, r.ltype_handle, r.visualstyle_handle, r.linetype.clone())
    }).collect())
}

fn parse_layer_record(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    expected_handle: u64,
) -> crate::core::result::Result<LayerRecord> {
    use version::DwgVersion::*;

    // R2000-R2007 read a 4-byte object size (RL) directly after the type
    // prefix, before the handle. R2010+ has no inline size field at all.
    // R14 is neither: it has the field, but positioned AFTER EED instead
    // (LibreDWG decode.c: VERSIONS(R_13b1,R_14) reads bitsize right before
    // common_object_handle_data.spec starts, i.e. after dwg_decode_eed).
    // FIX: previously grouped with R2000-R2007 (read before the handle),
    // which corrupted the handle read for every R14 object -- confirmed
    // via trace: garbage obj_size value, handle read as all-zero, cascading
    // into an EOF error a few fields later. R14 must skip this read here
    // and take it after EED instead (below).
    // Captured (not just discarded) because R2007 needs this exact value
    // later to locate its string stream -- see the string-stream section
    // below.
    let obj_size = if matches!(version, R2000 | R2004 | R2007) {
        Some(reader.read_rl(Endian::Little)?)
    } else {
        None
    };
    let record_handle = reader.read_h()?.value;
    let eed = decode_eed(reader, version)?;
    // R14: bitsize is after EED (LibreDWG decode.c VERSIONS(R_13b1,R_14)).
    // Same absolute bit index from object start as R2000–R2007 bitsize.
    let obj_size = if matches!(version, R14) {
        Some(reader.read_rl(Endian::Little)?)
    } else {
        obj_size
    };

    let num_reactors = reader.read_bl()?;
    // FIX: SINCE R_2004a only (dwg.spec) -- R14/R2000 have no such field.
    // Reading it unconditionally (the old behavior) misaligned every
    // R14/R2000 record from here on; root cause of the long-standing
    // "some R2000 files return an empty layer name" issue.
    let is_xdic_missing = match version {
        R14 | R2000 => false,
        _ => reader.read_b()? != 0,
    };
    if matches!(version, R2013 | R2018) {
        let _has_ds_binary_data = reader.read_b()?;
    }

    // Name: inline TV for R14/R2000/R2004, in its real position (before
    // the xref-resolution fields -- confirmed against dwg.spec's
    // COMMON_TABLE_FLAGS macro directly). R2007+ stores it in the string
    // stream instead, resolved separately below.
    // FIX: R2007 was previously grouped with R2004 (inline) -- wrong.
    // LibreDWG decode_r2007.c's obj_stream_position is explicitly gated
    // `SINCE (R_2007a)`, and empirically the inline read here just landed
    // on a coincidental 2-bit "empty string" BS encoding, corrupting
    // every field read after it (confirmed: name came back "", flags came
    // back all-false, lineweight 0, on every R2007 fixture).
    let inline_name = if matches!(version, R14 | R2000 | R2004) {
        Some(reader.read_tv()?)
    } else {
        None
    };

    // xref-resolution: <=R2004 is the 3-field form, >R2004 (R2007+) is the
    // 1-field form (dwg.spec: UNTIL(R_2004), inclusive of R2004 itself).
    // No handle read in either branch -- owner/xref/xdicobj handles all
    // live in the separate handle stream for every version (LibreDWG TRACE).
    if matches!(version, R14 | R2000 | R2004) {
        let _is_xref_ref = reader.read_b()?;
        let _is_xref_resolved = reader.read_bs()?;
        let _is_xref_dep = reader.read_b()?;
    } else {
        let _is_xref_resolved = reader.read_bs()?;
    }

    // State flags: R14 is four separate bits; R2000+ packs
    // frozen/off/frozen_in_new/locked/plotflag/lineweight into one
    // bitshort ("flag0").
    let (frozen, mut off, frozen_in_new, locked, plotflag, lineweight_idx) = match version {
        R14 => {
            let frozen = reader.read_b()? != 0;
            let off = reader.read_b()? != 0;
            let frozen_in_new = reader.read_b()? != 0;
            let locked = reader.read_b()? != 0;
            (frozen, off, frozen_in_new, locked, false, 0u8)
        }
        _ => {
            let flag0 = reader.read_bs()?;
            (
                flag0 & 0x0001 != 0,
                flag0 & 0x0002 != 0,
                flag0 & 0x0004 != 0,
                flag0 & 0x0008 != 0,
                flag0 & 0x0010 != 0,
                ((flag0 & 0x03E0) >> 5) as u8,
            )
        }
    };

        // Color (CMC) — exact bitstream fields only.
    //
    // Layout (ODA / LibreDWG bit_read_CMC):
    //   BS  index          — ACI (or signed on R14; negative means off)
    //   R2004+:
    //   BL  rgb            — high byte = method (0xC0 ByLayer, 0xC1 ByBlock,
    //                        0xC2 RGB, 0xC3 ACI/palette RGB payload, …);
    //                        low 24 bits = 0xRRGGBB when method carries RGB
    //   RC  flag           — bit0 color_name present, bit1 book_name present
    //   (+ TV name/book on R2004 main stream; R2007+ on string stream)
    //
    // We never re-derive ACI via palette search or rgb&0xFF fallback.
    // color_index is always the BS just read. true_color is the low-24 RGB
    // when method is true-color (0xC2) or when method is ACI (0xC3) and the
    // file stored a non-zero RGB payload (exact bits, not a lookup result).
    let raw_index = reader.read_bs()?;
    let (color_index, true_color, color_byte, mut color_name, mut book_name) = match version {
        R14 | R2000 => {
            // R14: off is encoded as a negative color index (dwg.spec DECODER
            // after FIELD_CMC). R2000 uses flag0 for off; index stays as-is.
            if matches!(version, R14) {
                off = (raw_index as i16) < 0;
            }
            // Exact ACI: absolute value for display when R14 encoded off in sign
            let color_index = if matches!(version, R14) {
                (raw_index as i16).unsigned_abs()
            } else {
                raw_index
            };
            (color_index, None, 0u8, None, None)
        }
        _ => {
            let color_rgb = reader.read_bl()?;
            let color_byte = reader.read_rc()?;
            let method = (color_rgb >> 24) as u8;
            let rgb24 = color_rgb & 0x00FF_FFFF;

            // Exact ACI — deterministic from file fields only (no palette search):
            //   • Prefer the BS index when non-zero (spec-compliant writers).
            //   • R2004+ often writes BS=0 and places ACI in rgb24 low byte
            //     with method 0xC3 (ACadSharp / many Autodesk files). That is
            //     still exact: low byte of the BL, not a palette lookup.
            //   • 0xC0/0xC1: ByLayer/ByBlock sentinels — keep BS as-is.
            let color_index = match method {
                0xC0 => 256u16, // ByLayer
                0xC1 => 0u16,   // ByBlock
                0xC3 if raw_index == 0 && (rgb24 & 0x00FFFF00) == 0 => {
                    (rgb24 & 0xFF) as u16
                }
                _ if raw_index != 0 => raw_index,
                _ if method == 0xC3 && (rgb24 & 0x00FFFF00) == 0 => (rgb24 & 0xFF) as u16,
                _ => raw_index,
            };

            // true_color only for true-color method 0xC2 (24-bit RGB).
            // Method 0xC3 stores palette RGB or ACI-in-low-byte — not a
            // free true-color; leave true_color None so ACI is the color.
            let true_color = if method == 0xC2 && rgb24 != 0 {
                Some(rgb24)
            } else {
                None
            };

            let (color_name, book_name) = if matches!(version, R2004) {
                let color_name = if color_byte & 0x01 != 0 {
                    Some(reader.read_tv()?)
                } else {
                    None
                };
                let book_name = if color_byte & 0x02 != 0 {
                    Some(reader.read_tv()?)
                } else {
                    None
                };
                (color_name, book_name)
            } else {
                (None, None)
            };

            (color_index, true_color, color_byte, color_name, book_name)
        }
    };

    // Name/color_name/book_name for R2007+ live in a separate string-
    // stream cursor, independent of everything read above -- a failure
    // here can't invalidate flags/color, already fully decoded by now.
    //
    // R2007 vs R2010+ locate this cursor differently: R2007's anchor is
    // exact (obj_size, already in hand -- LibreDWG decode_r2007.c:
    // obj_string_stream's presence bit sits at exactly obj_size - 1, no
    // guessing needed). R2010+ has no inline obj_size field at all, so it
    // still goes through the multi-candidate sweep in
    // locate_layer_string_stream_starts.
    let mut name = inline_name;
    if matches!(version, R2007 | R2010 | R2013 | R2018) {
        // Exact end-of-data only: R2007 uses obj_size; R2010+ uses
        // handle_stream geometry (no multi-candidate sweep).
        let exact_end_bit = if matches!(version, R2007) {
            obj_size
        } else {
            resolve_r2010_object_data_end_bit_exact(api_header)
        };
        for (mut sr, _end_bit) in
            locate_layer_string_stream_starts(record, api_header, version, exact_end_bit)
        {
            let Ok(candidate_name) = sr.read_tu() else {
                continue;
            };
            // FIX: an empty read doesn't error, so a wrong candidate can
            // "succeed" before the real one is ever tried -- this was the
            // root cause of color_name/book_name always coming back None
            // (the layer's own name always resolves fine independently,
            // since it already rejected empty candidates; this string-
            // stream lookup didn't). Reject and keep looking.
            let trimmed = candidate_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            name = Some(trimmed.to_string());
            if color_byte & 0x01 != 0 {
                color_name = sr.read_tu().ok().map(|s| s.trim().to_string());
            }
            if color_byte & 0x02 != 0 {
                book_name = sr.read_tu().ok().map(|s| s.trim().to_string());
            }
            break;
        }
    }

    let handle = if record_handle != 0 {
        record_handle
    } else {
        expected_handle
    };

    let hs = read_layer_handle_stream(
        record,
        api_header,
        version,
        handle,
        num_reactors,
        is_xdic_missing,
        obj_size,
    );

    Ok(LayerRecord {
        handle,
        name,
        color_index,
        true_color,
        color_name,
        book_name,
        frozen,
        off,
        frozen_in_new,
        locked,
        plotflag,
        lineweight_idx,
        eed,
        owner_handle: hs.owner,
        xdic_handle: hs.xdic,
        plotstyle_handle: hs.plotstyle,
        material_handle: hs.material,
        ltype_handle: hs.ltype,
        visualstyle_handle: hs.visualstyle,
        linetype: None,
        eed_app_names: Vec::new(),
        description: None,
        handle_warnings: hs.warnings,
    })
}

/// Deterministic string-stream candidate finder. For R2010+, tries a
/// handful of end_bit candidates derived from handle_stream_size_bits
/// (LibreDWG doesn't give us an exact anchor there without re-deriving its
/// own header format). For R2007, `exact_end_bit` is *not* a guess -- it's
/// `obj_size` (LibreDWG decode_r2007.c: obj_string_stream's presence bit
/// sits at exactly `obj->bitsize - 1`), already read earlier in the same
/// pass, so there's exactly one real candidate, not several to try.
/// Either way: for each candidate, checks the object's literal last bit is
/// a real presence flag before trusting it. No scoring.
fn locate_layer_string_stream_starts<'a>(
    record: &'a objects::ObjectRecord<'a>,
    _api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
    exact_end_bit: Option<u32>,
) -> Vec<(BitReader<'a>, u32)> {
    let mut base_reader = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader, version).is_err() {
        return Vec::new();
    }

    // Exact only: never scan alternate end_bit candidates for layer strings
    // (name / color_name / book_name). Missing exact geometry → no strings.
    let end_bit_candidates = match exact_end_bit {
        Some(end_bit) => vec![end_bit],
        None => return Vec::new(),
    };

    let mut starts = Vec::new();
    for end_bit in end_bit_candidates {
        let mut presence_reader = base_reader.clone();
        presence_reader.set_bit_pos(end_bit.saturating_sub(1));
        if !matches!(presence_reader.read_b(), Ok(1)) {
            continue;
        }

        let Some((start_bit, stream_end_bit)) =
            resolve_r2010_string_stream_range_spec(&base_reader, end_bit)
        else {
            continue;
        };

        let mut stream_reader = base_reader.clone();
        stream_reader.set_bit_pos(start_bit);
        starts.push((stream_reader, stream_end_bit));
    }
    starts
}

// ============================================================================
// Entity -> layer handle association (2d).
//
// Deterministic path only:
//   1. Parse common entity header using the exact object-data end bit.
//   2. Seek to header.obj_size (handle-stream start).
//   3. Read the layer slot defined by common_entity_handle_data
//      (after optional owner, reactors, xdic, legacy links).
//
// No multi-end_bit candidate grid, no score tables, no "default = min
// known layer" substitution. If the fixed slot cannot be read, return the
// already-parsed value when it is a known LAYER handle, otherwise 0.
// ============================================================================

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

    let debug_entity_handle = std::env::var("EZDWG_DEBUG_ENTITY_LAYER")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let debug_this = debug_entity_handle == Some(object_handle);

    // Primary: fixed common-entity-handle_data layer slot at exact end bit.
    if let Some(layer) =
        parse_common_entity_layer_handle_from_common_header(record, version, api_header)
    {
        if debug_this {
            eprintln!(
                "[entity-layer] handle={} fixed_slot_layer={}",
                object_handle, layer
            );
        }
        // Accept zero (explicit "no layer" / BYLAYER edge) and known LAYER
        // handles. Unknown non-zero is still returned when the fixed slot
        // decoded cleanly — the object map may lag the handle stream.
        if layer == 0 || known_layer_handles.is_empty() || known_layer_handles.contains(&layer) {
            return layer;
        }
        // Fixed slot produced a non-layer object handle: treat as failure
        // rather than scoring alternate indices.
        if debug_this {
            eprintln!(
                "[entity-layer] handle={} fixed_slot={} not a known LAYER",
                object_handle, layer
            );
        }
    }

    // Secondary: same exact stream origin, but take the layer by the
    // versioned fixed index (covers dimstyle prefix on dimensions).
    if let Some(layer) =
        read_entity_layer_handle_at_fixed_index(record, version, api_header, object_handle)
    {
        if debug_this {
            eprintln!(
                "[entity-layer] handle={} fixed_index_layer={}",
                object_handle, layer
            );
        }
        if layer == 0 || known_layer_handles.is_empty() || known_layer_handles.contains(&layer) {
            return layer;
        }
    }

    // Fallbacks: trust prior parse only if it names a known LAYER; else 0.
    // Never substitute min(known_layers) — that was a heuristic.
    if known_layer_handles.contains(&parsed_layer_handle) {
        if debug_this {
            eprintln!(
                "[entity-layer] handle={} fallback_parsed={}",
                object_handle, parsed_layer_handle
            );
        }
        return parsed_layer_handle;
    }
    if parsed_layer_handle == 0 {
        return 0;
    }
    if debug_this {
        eprintln!(
            "[entity-layer] handle={} unresolved parsed={}",
            object_handle, parsed_layer_handle
        );
    }
    0
}

/// Exact object-data end bit only (no candidate list).
fn entity_object_data_end_bit(api_header: &ApiObjectHeader) -> Option<u32> {
    resolve_r2010_object_data_end_bit_exact(api_header)
        .or_else(|| resolve_r2010_object_data_end_bit(api_header).ok())
}

fn parse_common_entity_header_at_end(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    _api_header: &ApiObjectHeader,
    object_data_end_bit: u32,
) -> Option<entities::common::CommonEntityHeader> {
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    match version {
        version::DwgVersion::R2010 => {
            entities::common::parse_common_entity_header_r2010(&mut reader, object_data_end_bit)
                .ok()
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            entities::common::parse_common_entity_header_r2013(&mut reader, object_data_end_bit)
                .ok()
        }
        _ => None,
    }
}

fn parse_expected_entity_layer_ref_index(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
) -> Option<usize> {
    let object_data_end_bit = entity_object_data_end_bit(api_header)?;
    let header =
        parse_common_entity_header_at_end(record, version, api_header, object_data_end_bit)?;

    let mut index = 0usize;
    if header.entity_mode == 0 {
        index = index.saturating_add(1);
    }
    index = index.saturating_add(header.num_of_reactors as usize);
    if header.xdic_missing_flag == 0 {
        index = index.saturating_add(1);
    }
    if header.has_legacy_entity_links {
        index = index.saturating_add(2);
    }
    // R2010+ dimensions keep dimstyle and anonymous block handles before
    // common entity handles in some layouts.
    if matches!(api_header.type_code, 0x15 | 0x19 | 0x1A) {
        index = index.saturating_add(2);
    }

    let debug_entity_handle = std::env::var("EZDWG_DEBUG_ENTITY_LAYER")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    if debug_entity_handle == Some(object_handle) {
        eprintln!(
            "[entity-layer] handle={} expected_index={} entity_mode={} reactors={} xdic_missing={} type=0x{:X}",
            object_handle,
            index,
            header.entity_mode,
            header.num_of_reactors,
            header.xdic_missing_flag,
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
    let object_data_end_bit = entity_object_data_end_bit(api_header)?;
    let header =
        parse_common_entity_header_at_end(record, version, api_header, object_data_end_bit)?;
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    reader.set_bit_pos(header.obj_size);
    entities::common::parse_common_entity_layer_handle(&mut reader, &header).ok()
}

/// Read the handle at the fixed layer index in the entity handle stream
/// (exact stream start = header.obj_size after common header parse).
fn read_entity_layer_handle_at_fixed_index(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
) -> Option<u64> {
    let object_data_end_bit = entity_object_data_end_bit(api_header)?;
    let header =
        parse_common_entity_header_at_end(record, version, api_header, object_data_end_bit)?;
    let expected_index =
        parse_expected_entity_layer_ref_index(record, version, api_header, object_handle)?;

    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    reader.set_bit_pos(header.obj_size);

    let mut last = 0u64;
    for i in 0..=expected_index {
        match entities::common::read_handle_reference(&mut reader, header.handle) {
            Ok(h) => {
                if i == expected_index {
                    return Some(h);
                }
                last = h;
            }
            Err(_) => {
                if i == expected_index {
                    return None;
                }
                break;
            }
        }
    }
    let _ = last;
    None
}


#[cfg(test)]
fn layer_handle_score(layer_handle: u64, known_layer_handles: &HashSet<u64>) -> u64 {
    if known_layer_handles.contains(&layer_handle) {
        0
    } else if layer_handle == 0 {
        10_000
    } else {
        50_000
    }
}

/// Historical score function retained only for unit tests of relative ordering.
#[cfg(test)]
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
        let distance = expected.abs_diff(handle_index as usize) as u64;
        score = score.saturating_add(distance.saturating_mul(48));
        if handle_index as usize == expected {
            score = score.saturating_sub(120);
            if layer_handle == 0 && allow_exact_zero_layer_bonus {
                score = score.saturating_sub(9_880);
            }
        }
    }
    if handle_index == 0 && expected_layer_index != Some(0) {
        score = score.saturating_add(200);
    }
    if let Some(canonical) = canonical_end_bit {
        score = score.saturating_add(u64::from(canonical.abs_diff(object_data_end_bit) / 2));
    }
    if chained_base {
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

/// LAYER record of R13/R14/R2000 exactly as specified (ODA 20.4.54): these versions
/// have no "XDic Missing Flag", keep the entry name inline and store the color as a
/// plain index. R13/R14 carry four state bits where R2000 has the "Values" BS.
fn decode_layer_name_record(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
    expected_handle: u64,
) -> crate::core::result::Result<(u64, String)> {
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version)?;
    let preamble = read_table_record_preamble(&mut reader, version, api_header)?;
    let record_handle = preamble.handle;

    let name = if matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        // The name is the first string of the string stream. The scan below looks
        // for the most plausible string instead, and takes the color book name of
        // a layer whose color comes from a color book.
        match read_block_name_from_exact_string_stream(&reader, Some(api_header)) {
            Some(name) => name,
            None => decode_layer_name_from_string_stream(record, api_header, version)?,
        }
    } else if matches!(version, version::DwgVersion::R2007) {
        // R2007 already keeps the entry name in the string stream; the data
        // stream continues with the flags, so there is no inline text to read.
        let name = read_table_record_stream_strings(&reader, preamble.data_end_bit, 1)
            .into_iter()
            .next()
            .unwrap_or_default();
        if name.is_empty() || !is_plausible_table_text(&name) {
            return Err(DwgError::new(
                ErrorKind::Format,
                "failed to decode layer name from the R2007 string stream",
            ));
        }
        name
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
            160, 0, Some(0), 549, Some(509), false, 81, Some(130), false, &known,
        );
        let later_score = layer_handle_candidate_score(
            160, 1, Some(0), 533, Some(509), false, 81, Some(130), false, &known,
        );
        assert!(exact_first_score < later_score);
    }

    #[test]
    fn layer_candidate_score_does_not_prefer_zero_without_zero_bonus() {
        let known = HashSet::from([160u64]);
        let zero_score = layer_handle_candidate_score(
            0, 0, Some(0), 493, Some(509), false, 81, Some(130), false, &known,
        );
        let known_score = layer_handle_candidate_score(
            160, 0, Some(0), 549, Some(509), false, 81, Some(130), false, &known,
        );
        assert!(known_score < zero_score);
    }
}

