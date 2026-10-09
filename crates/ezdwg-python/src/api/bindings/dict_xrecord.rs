// ============================================================================
// Shared DICTIONARY / XRECORD decode helpers
//
// Used by layer_states and layer_filters. All bindings/*.rs files are
// include!-ed into one module, so these helpers are file-private to that
// module and visible to every subsequent include.
// ============================================================================

const LS_LAYER_CONTROL_TYPE: u16 = 0x32;
const LS_DICTIONARY_TYPE: u16 = 0x2A;
const ACAD_LAYERSTATES_KEYS: &[&str] = &["ACAD_LAYERSTATES", "ACAD_LAYERSTATE"];

fn handle_stream_start_bit(
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
    bitsize: Option<u32>,
) -> Option<u32> {
    use version::DwgVersion::*;
    match version {
        R2010 | R2013 | R2018 => resolve_r2010_object_data_end_bit_exact(api_header),
        R14 | R2000 | R2004 | R2007 => {
            let bitsize = bitsize.filter(|&s| s > 0)?;
            let total_bits = api_header.data_size.saturating_mul(8);
            if total_bits > 0 && bitsize >= total_bits {
                return None;
            }
            Some(bitsize)
        }
        _ => None,
    }
}

/// Read optional bitsize (RL) that lives right after the type prefix on R2000–R2007.
fn read_optional_bitsize(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
) -> Option<u32> {
    use version::DwgVersion::*;
    if matches!(version, R2000 | R2004 | R2007) {
        reader.read_rl(Endian::Little).ok()
    } else {
        None
    }
}

fn is_dictionary_type(code: u16, dynamic: &HashMap<u16, String>) -> bool {
    matches_type_name(code, LS_DICTIONARY_TYPE, "DICTIONARY", dynamic)
        || matches_type_name(code, LS_DICTIONARY_TYPE, "ACDBDICTIONARYWDFLT", dynamic)
}

fn is_layer_control_type(code: u16, dynamic: &HashMap<u16, String>) -> bool {
    matches_type_name(code, LS_LAYER_CONTROL_TYPE, "LAYER_CONTROL", dynamic)
}



/// Skip common non-entity object **data-section** preamble after OT.
///
/// LibreDWG dump of multi-page AC1032 maps handle 0x200 (AC1032 DICTIONARY):
///   handle H → num_eed=0 → **num_reactors BL** → **is_xdic_missing B** →
///   **has_ds_data B** (R2013+) → numitems BL → cloning BS → is_hardowner RC
///
/// Owner / reactor / xdic **handles** are in the handle stream; the counts and
/// flags above are in the data section (same order as LAYER after EED).
/// Confirmed: after H+empty EED the remaining 12 bits to numitems=12 are
/// BL(num_reactors=1)=10 bits + B + B.
/// Returns `(num_reactors, is_xdic_missing, has_ds_data)` from the data section.
fn skip_dictionary_common_preamble(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
) -> Result<(u32, bool, bool), crate::core::error::DwgError> {
    let _object_handle = reader.read_h()?;
    let mut ext_size = reader.read_bs()?;
    let mut blocks = 0u32;
    while ext_size > 0 {
        if blocks >= 64 || ext_size > 16_384 {
            return Err(crate::core::error::DwgError::new(
                crate::core::error::ErrorKind::Format,
                format!("DICTIONARY EED size {ext_size} or block count unreasonable"),
            ));
        }
        let _app = reader.read_h()?;
        for _ in 0..ext_size {
            let _ = reader.read_rc()?;
        }
        blocks += 1;
        ext_size = reader.read_bs()?;
    }
    let num_reactors = reader.read_bl()?;
    let is_xdic_missing = if !matches!(version, version::DwgVersion::R14 | version::DwgVersion::R2000)
    {
        reader.read_b()? != 0
    } else {
        false
    };
    let has_ds_data = if matches!(
        version,
        version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        reader.read_b()? != 0
    } else {
        false
    };
    Ok((num_reactors, is_xdic_missing, has_ds_data))
}

/// After OT + object H + EED + num_reactors/is_xdic_missing/has_ds_data,
/// DICTIONARY `numitems` follows (LibreDWG dump of multi-page AC1032 maps 0x200).
/// When `expected_numitems` is set and the BL does not match, return None
/// rather than scanning the body for BL==expected.

/// Decode DICTIONARY data + handles once the readers are positioned.
///
/// For R2007+ with a string stream, pass `expected_numitems` = TU count so
/// `numitems` can be located when it does not sit immediately after OT.
fn assemble_dictionary_from_record<'a>(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    object_handle: u64,
    handle_start: u32,
    string_reader: Option<&'a mut BitReader<'a>>,
    expected_numitems: Option<u32>,
) -> Option<objects::Dictionary> {
    // ACadSharp readCommonData + readCommonDictionary:
    //   after OT → object H → EED → numitems (no free bit scan).
    let mut data_reader = record.bit_reader();
    skip_object_type_prefix(&mut data_reader, version).ok()?;
    let _ = read_optional_bitsize(&mut data_reader, version);
    let common_counts = skip_dictionary_common_preamble(&mut data_reader, version).ok()?;

    // Sanity: when expected TU count is known, numitems must match immediately.
    if let Some(exp) = expected_numitems {
        let pos = data_reader.tell_bits();
        let mut probe = record.bit_reader();
        probe.set_bit_pos(pos as u32);
        match probe.read_bl() {
            Ok(n) if n == exp => {}
            Ok(_n) => return None,
            Err(_) => return None,
        }
    }

    let mut ctx = objects::DictionaryDecodeCtx {
        version: version.clone(),
        object_handle,
        handle_stream_bit_start: Some(u64::from(handle_start)),
        string_reader,
    };
    let data = objects::decode_dictionary_data(&mut data_reader, &mut ctx).ok()?;

    // Handle stream: owner + reactors + xdic refs only (counts from data section).
    let mut handle_reader = record.bit_reader();
    handle_reader.set_bit_pos(handle_start);
    match objects::decode_dictionary_handles(
        &mut handle_reader,
        version,
        object_handle,
        data.numitems,
        Some(common_counts),
    ) {
        Ok(h) => objects::assemble_dictionary(object_handle, data, h).ok(),
        Err(_err) => {
            // BUG(dictionary-handle-stream): geometric handle_start did not
            // decode COMMON + numitems soft-pointers. Observed on AC1032
            // states dict (multi-page AC1032 maps handle 0x200): end_bit from
            // resolve_r2010_object_data_end_bit_exact yields invalid item
            // refs; names remain valid from the string stream / numitems BL.
            // Do not invent alternate bit positions here — fix end_bit or
            // COMMON_OBJECT_HANDLE_DATA positioning instead.
            let numitems = data.numitems;
            let entries = data
                .names
                .into_iter()
                .map(|name| objects::DictionaryEntry {
                    name,
                    value_handle: None,
                })
                .collect();
            Some(objects::Dictionary {
                handle: objects::Handle(object_handle),
                numitems,
                cloning: data.cloning,
                is_hardowner: data.is_hardowner,
                entries,
                num_reactors: 0,
                is_xdic_missing: true,
                has_ds_data: false,
                owner_handle: None,
                reactors: Vec::new(),
                xdic_handle: None,
            })
        }
    }
}

/// Decode one DICTIONARY object at the given index entry.
///
/// Deterministic R2007+ path (LibreDWG `obj_string_stream`):
///   1. `end_bit` = exact object-data end (`handle_stream_size` geometry)
///   2. String stream = presence bit @ end_bit-1 + RS size → TU texts
///   3. Handle stream @ end_bit → COMMON_OBJECT_HANDLE_DATA + item handles
///
/// When the data-section `numitems` BL is unreadable (compact AC1032 dicts),
/// `numitems` is taken as the number of TUs read from the string stream
/// (stream bounds are exact; this is not a scan/guess).
fn decode_one_dictionary(
    decoder: &decoder::Decoder<'_>,
    obj: &objects::ObjectRef,
    best_effort: bool,
) -> Option<objects::Dictionary> {
    let (record, header) = parse_record_and_header(decoder, obj.offset, best_effort).ok()??;
    let version = decoder.version();
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version).ok()?;
    let bitsize = read_optional_bitsize(&mut reader, version);
    let handle_start = handle_stream_start_bit(&header, version, bitsize)?;

    use version::DwgVersion::*;
    if matches!(version, R2007 | R2010 | R2013 | R2018) {
        let exact_end = if matches!(version, R2007) {
            bitsize
        } else {
            resolve_r2010_object_data_end_bit_exact(&header)
        };
        if let Some(eb) = exact_end {
            let stream_names =
                read_dictionary_string_stream_names(&record, &header, version, eb);
            let expected = if stream_names.is_empty() {
                None
            } else {
                Some(stream_names.len() as u32)
            };

            // Classic sparse path: locate numitems BL (= expected TU count),
            // then read names from the string stream at the normal field order.
            for (mut sr, _) in
                locate_layer_string_stream_starts(&record, &header, version, Some(eb))
            {
                if let Some(dict) = assemble_dictionary_from_record(
                    &record,
                    version,
                    obj.handle.0,
                    handle_start,
                    Some(&mut sr),
                    expected,
                ) {
                    if dict.entries.iter().any(|e| !e.name.is_empty()) {
                        return Some(dict);
                    }
                }
            }

            // Fallback: names from string stream only; handles by count.
            if let Some(dict) =
                assemble_dictionary_from_string_stream(&record, &header, version, obj.handle.0, eb)
            {
                return Some(dict);
            }
        }
        return assemble_dictionary_from_record(
            &record,
            version,
            obj.handle.0,
            handle_start,
            None,
            None,
        );
    }
    assemble_dictionary_from_record(&record, version, obj.handle.0, handle_start, None, None)
}

/// Build a DICTIONARY from exact string-stream + handle-stream geometry.
///
/// Reads every TU until the string-stream end bit, then reads the same number
/// of soft-pointer item handles after COMMON_OBJECT_HANDLE_DATA. Fully
/// determined by LibreDWG object layout — no body scans or bit-shift trials.
fn assemble_dictionary_from_string_stream(
    record: &objects::ObjectRecord<'_>,
    header: &ApiObjectHeader,
    version: &version::DwgVersion,
    object_handle: u64,
    end_bit: u32,
) -> Option<objects::Dictionary> {
    let starts = locate_layer_string_stream_starts(record, header, version, Some(end_bit));
    let (mut sr, stream_end) = starts.into_iter().next()?;

    let mut names: Vec<String> = Vec::new();
    while (sr.tell_bits() as u32) < stream_end {
        match sr.read_tu() {
            Ok(s) => names.push(s),
            Err(_) => break,
        }
    }
    if names.is_empty() {
        return None;
    }
    let numitems = names.len() as u32;

    // Counts/flags live in the data section (after OT + H + EED).
    let mut data_reader = record.bit_reader();
    skip_object_type_prefix(&mut data_reader, version).ok()?;
    let _ = read_optional_bitsize(&mut data_reader, version);
    let common_counts = skip_dictionary_common_preamble(&mut data_reader, version).ok()?;

    // Handle stream: owner + reactors + xdic refs, then itemhandles.
    let mut handle_reader = record.bit_reader();
    handle_reader.set_bit_pos(end_bit);
    let handles = match objects::decode_dictionary_handles(
        &mut handle_reader,
        version,
        object_handle,
        numitems,
        Some(common_counts),
    ) {
        Ok(h) => h,
        Err(_err) => {
            // BUG(dictionary-handle-stream): same as assemble_dictionary_from_record.
            // Names from the string stream are kept; value handles left unset.
            let entries = names
                .into_iter()
                .map(|name| objects::DictionaryEntry {
                    name,
                    value_handle: None,
                })
                .collect();
            return Some(objects::Dictionary {
                handle: objects::Handle(object_handle),
                numitems,
                cloning: None,
                is_hardowner: None,
                entries,
                num_reactors: 0,
                is_xdic_missing: true,
                has_ds_data: false,
                owner_handle: None,
                reactors: Vec::new(),
                xdic_handle: None,
            });
        }
    };
    let data = objects::DictionaryData {
        numitems,
        cloning: None,
        is_hardowner: None,
        names,
    };
    objects::assemble_dictionary(object_handle, data, handles).ok()
}


/// Read all TU names from the exact R2010+ string stream for a dictionary.
fn read_dictionary_string_stream_names(
    record: &objects::ObjectRecord<'_>,
    header: &ApiObjectHeader,
    version: &version::DwgVersion,
    end_bit: u32,
) -> Vec<String> {
    let starts = locate_layer_string_stream_starts(record, header, version, Some(end_bit));
    let Some((mut sr, stream_end)) = starts.into_iter().next() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    while (sr.tell_bits() as u32) < stream_end {
        match sr.read_tu() {
            Ok(s) => names.push(s),
            Err(_) => break,
        }
    }
    names
}

/// Decode one XRECORD at the given index entry.
///
/// Layout (LibreDWG non-entity object, R2004+):
///   OT → [bitsize] → object H → EED → num_reactors BL → is_xdic B →
///   has_ds B (R2013+) → **BL xdata_size** → XDATA → BS cloning →
///   [1 pad bit] → handle stream (owner + reactors + xdic + objid Hs)
///
/// The common preamble must be skipped before reading `xdata_size`. Skipping
/// it is what makes the declared size trustworthy; there is no recovery path.
fn decode_one_xrecord(
    decoder: &decoder::Decoder<'_>,
    obj: &objects::ObjectRef,
    best_effort: bool,
) -> Option<objects::XRecord> {
    let (record, header) = parse_record_and_header(decoder, obj.offset, best_effort).ok()??;
    let version = decoder.version();
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version).ok()?;
    let bitsize = read_optional_bitsize(&mut reader, version);
    let handle_start = handle_stream_start_bit(&header, version, bitsize)?;

    let mut data_reader = record.bit_reader();
    skip_object_type_prefix(&mut data_reader, version).ok()?;
    let _ = read_optional_bitsize(&mut data_reader, version);
    // Common non-entity preamble: H + EED + counts (same as DICTIONARY).
    let common_counts = skip_dictionary_common_preamble(&mut data_reader, version).ok()?;

    let ctx = objects::XRecordDecodeCtx {
        version: version.clone(),
        object_handle: obj.handle.0,
    };
    let data = objects::decode_xrecord_data_with_end(
        &mut data_reader,
        &ctx,
        Some(u64::from(handle_start)),
    )
    .ok()?;

    let total_bits = u64::from(header.data_size.saturating_mul(8));
    let handle_end = match version {
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            u64::from(header.data_start_bit.unwrap_or(0)).saturating_add(total_bits)
        }
        _ => total_bits,
    };

    let mut handle_reader = record.bit_reader();
    handle_reader.set_bit_pos(handle_start);
    let handles = objects::decode_xrecord_handles(
        &mut handle_reader,
        version,
        obj.handle.0,
        handle_end,
        Some(common_counts),
    )
    .ok()?;
    Some(objects::assemble_xrecord(obj.handle.0, data, handles))
}

/// Read xdic from LAYER_CONTROL using data-section counts (R2004+).
///
/// Table control objects (LibreDWG `dwg_obj_is_control`) do **not** carry an
/// ownerhandle. Data section still has num_reactors / is_xdic_missing /
/// has_ds; the handle stream is reactors then xdic only.
fn layer_control_xdic_handle(
    decoder: &decoder::Decoder<'_>,
    obj: &objects::ObjectRef,
    best_effort: bool,
) -> Option<u64> {
    let (record, header) = parse_record_and_header(decoder, obj.offset, best_effort).ok()??;
    let version = decoder.version();
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version).ok()?;
    let bitsize = read_optional_bitsize(&mut reader, version);
    let handle_start = handle_stream_start_bit(&header, version, bitsize)?;

    // Counts live in the data section on R2004+.
    let mut data_reader = record.bit_reader();
    skip_object_type_prefix(&mut data_reader, version).ok()?;
    let _ = read_optional_bitsize(&mut data_reader, version);
    let (num_reactors, is_xdic_missing, _has_ds) =
        skip_dictionary_common_preamble(&mut data_reader, version).ok()?;

    if is_xdic_missing {
        return None;
    }

    let mut handle_reader = record.bit_reader();
    handle_reader.set_bit_pos(handle_start);
    // Control layout: no ownerhandle — reactors then xdic.
    const MAX_REACTORS: u32 = 10_000;
    if num_reactors > MAX_REACTORS {
        return None;
    }
    for _ in 0..num_reactors {
        let _ = handle_reader.read_h().ok()?;
    }
    let href = handle_reader.read_h().ok()?;
    objects::resolve_handle_ref(&href, obj.handle.0)
        .ok()
        .flatten()
        .filter(|&h| h != 0)
}

fn xdata_value_to_py(py: Python<'_>, value: &objects::XDataValue) -> PyResult<PyObject> {
    use objects::XDataValue::*;
    match value {
        String(s) => Ok(s.clone().into_py(py)),
        Real(v) | Real2(v) => Ok((*v).into_py(py)),
        Int16(v) => Ok((*v as i64).into_py(py)),
        Int32(v) => Ok((*v as i64).into_py(py)),
        Int64(v) => Ok((*v).into_py(py)),
        Bool(v) => Ok((*v).into_py(py)),
        Binary(b) => Ok(pyo3::types::PyBytes::new_bound(py, b).into_py(py)),
        // Absolute 8-byte XDATA object ids (LibreDWG): expose the integer
        // handle so Python can resolve LAYER names without string parsing.
        Handle(href) => Ok((href.value as u64).into_py(py)),
        Raw(b) => Ok(pyo3::types::PyBytes::new_bound(py, b).into_py(py)),
    }
}

