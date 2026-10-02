type LinetypeRow = (u64, String, String, f64, Vec<f64>);
type LayerLinetypeRow = (u64, u64);
type EntityLinetypeRow = (u64, u64, u8, Option<u64>, f64);
type EntityLineweightRow = (u64, Option<i16>, bool);
type LayerStateRow = (u64, bool, bool, bool, bool, bool, i16);
type DimStyleSizeRow = (u64, String, f64, f64, f64);

/// A linetype definition holds at most 12 dash specifications in AutoCAD.
const MAX_LINETYPE_DASHES: usize = 32;

/// Fixed-layout start of a table record (LAYER, LTYPE, ...), read after the type code.
struct TableRecordPreamble {
    /// End of the object data, which is where the handle stream starts, in record-body bits.
    data_end_bit: Option<u32>,
    handle: u64,
    num_reactors: u32,
    xdic_missing: bool,
}

/// Reads the common object data in front of the entry name of a table record.
///
/// The layout differs by version (ODA specification, "Common non-entity object format"):
/// R13/R14 store the object size after the EED, R2000-R2007 before the handle and
/// R2010+ not at all (the handle stream size in the object header replaces it).
/// The "XDic Missing Flag" exists from R2004 on and the data store flag from R2013 on.
fn read_table_record_preamble(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
) -> crate::core::result::Result<TableRecordPreamble> {
    let r13_r14 = matches!(version, version::DwgVersion::R13 | version::DwgVersion::R14);
    let r2010_plus = is_r2010_plus_version(version);

    let mut data_end_bit = None;
    if !r13_r14 && !r2010_plus {
        data_end_bit = Some(reader.read_rl(Endian::Little)?);
    }
    let handle = reader.read_h()?.value;
    skip_eed(reader)?;
    if r13_r14 {
        data_end_bit = Some(reader.read_rl(Endian::Little)?);
    }
    if r2010_plus {
        data_end_bit = resolve_r2010_object_data_end_bit_exact(api_header)
            .or_else(|| resolve_r2010_object_data_end_bit(api_header).ok());
    }

    let num_reactors = reader.read_bl()?;
    let xdic_missing = match version {
        version::DwgVersion::R13 | version::DwgVersion::R14 | version::DwgVersion::R2000 => false,
        _ => reader.read_b()? != 0,
    };
    if matches!(
        version,
        version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _has_ds_binary_data = reader.read_b()?;
    }

    Ok(TableRecordPreamble {
        data_end_bit,
        handle,
        num_reactors,
        xdic_missing,
    })
}

/// R2007+ keep the text fields of an object in its string stream, in field order.
fn table_record_strings_in_stream(version: &version::DwgVersion) -> bool {
    matches!(
        version,
        version::DwgVersion::R2007
            | version::DwgVersion::R2010
            | version::DwgVersion::R2013
            | version::DwgVersion::R2018
    )
}

/// The first `count` strings of the object's string stream (missing ones are empty).
fn read_table_record_stream_strings(
    base_reader: &BitReader<'_>,
    data_end_bit: Option<u32>,
    count: usize,
) -> Vec<String> {
    let mut strings = vec![String::new(); count];
    let Some(end_bit) = data_end_bit else {
        return strings;
    };
    let Some((start_bit, stream_end_bit)) =
        resolve_r2010_string_stream_range_oda(base_reader, end_bit)
    else {
        return strings;
    };
    let mut reader = base_reader.clone();
    reader.set_bit_pos(start_bit);
    for slot in strings.iter_mut() {
        let Ok(text) = reader.read_tu() else {
            break;
        };
        if reader.tell_bits() > u64::from(stream_end_bit) {
            break;
        }
        *slot = text;
    }
    strings
}

fn is_plausible_table_text(text: &str) -> bool {
    text.chars().count() <= 512 && !text.chars().any(|ch| ch.is_control() || ch == '\u{FFFD}')
}

/// LTYPE record (ODA specification 20.4.58): name, description, pattern length and
/// the dash lengths (positive = dash, negative = gap, 0 = dot, in drawing units).
fn decode_linetype_record(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
    expected_handle: u64,
) -> crate::core::result::Result<LinetypeRow> {
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version)?;
    let preamble = read_table_record_preamble(&mut reader, version, api_header)?;

    let strings_in_stream = table_record_strings_in_stream(version);
    let (name, description) = if strings_in_stream {
        let mut strings =
            read_table_record_stream_strings(&reader, preamble.data_end_bit, 2).into_iter();
        let name = strings.next().unwrap_or_default();
        let description = strings.next().unwrap_or_default();
        // R2007+ fold the 64-flag and the xref dependency bit into this one value.
        let _xref_index_plus_one = reader.read_bs()?;
        (name, description)
    } else {
        let name = reader.read_tv()?;
        let _flag_64 = reader.read_b()?;
        let _xref_index_plus_one = reader.read_bs()?;
        let _xdep = reader.read_b()?;
        let description = reader.read_tv()?;
        (name, description)
    };

    let pattern_length = reader.read_bd()?;
    let alignment = reader.read_rc()?;
    let num_dashes = reader.read_rc()? as usize;
    // The alignment code is 'A' in AutoCAD files and 'S' in files of some other
    // writers. Anything but a letter, or more dashes than a linetype can hold,
    // means the fields before it were not where this layout expects them.
    if !alignment.is_ascii_uppercase() || num_dashes > MAX_LINETYPE_DASHES {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("LTYPE record is misaligned (alignment {alignment:#04x}, {num_dashes} dashes)"),
        ));
    }
    let mut dashes = Vec::with_capacity(num_dashes);
    for _ in 0..num_dashes {
        let length = reader.read_bd()?;
        let _complex_shape_code = reader.read_bs()?;
        let _x_offset = reader.read_rd(Endian::Little)?;
        let _y_offset = reader.read_rd(Endian::Little)?;
        let _scale = reader.read_bd()?;
        let _rotation = reader.read_bd()?;
        let _shape_flag = reader.read_bs()?;
        dashes.push(length);
    }
    if !pattern_length.is_finite()
        || dashes.iter().any(|length| !length.is_finite())
        || !is_plausible_table_text(&name)
        || !is_plausible_table_text(&description)
    {
        return Err(DwgError::new(
            ErrorKind::Format,
            "LTYPE record has implausible values",
        ));
    }

    let handle = if preamble.handle != 0 {
        preamble.handle
    } else {
        expected_handle
    };
    Ok((handle, name, description, pattern_length, dashes))
}

/// Linetype table: `(handle, name, description, pattern_length, dash_lengths)`.
///
/// Dash lengths follow the DXF convention (positive = dash, negative = gap, 0 = dot)
/// and are in drawing units at linetype scale 1. The table includes the three
/// built-in entries "ByBlock", "ByLayer" and "Continuous".
#[pyfunction(signature = (path, limit=None))]
pub fn decode_linetypes(path: &str, limit: Option<usize>) -> PyResult<Vec<LinetypeRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result: Vec<LinetypeRow> = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x39, "LTYPE", &dynamic_types) {
            continue;
        }
        let row = match decode_linetype_record(&record, &header, decoder.version(), obj.handle.0) {
            Ok(row) => row,
            Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        // The object map can list a handle more than once; keep the first good record.
        if !seen.insert(row.0) {
            continue;
        }
        result.push(row);
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

fn collect_object_handles_by_type(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    builtin_code: u16,
    builtin_name: &str,
) -> PyResult<HashSet<u64>> {
    let mut handles = HashSet::new();
    for obj in index.objects.iter() {
        let Some((_record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, builtin_code, builtin_name, dynamic_types) {
            handles.insert(obj.handle.0);
        }
    }
    Ok(handles)
}

/// The linetype handle of a LAYER record, taken from its handle stream.
///
/// The stream holds, in order: layer control, reactors, xdictionary, xref block,
/// plotstyle (R2000+), material (R2007+) and the linetype. Rather than trust the
/// optional members, take the first handle that is a LTYPE object: no other
/// member of the stream can be one.
fn decode_layer_linetype_handle(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
    layer_handle: u64,
    linetype_handles: &HashSet<u64>,
) -> Option<u64> {
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version).ok()?;
    let preamble = read_table_record_preamble(&mut reader, version, api_header).ok()?;
    let start_bit = preamble.data_end_bit?;
    let base_handle = if preamble.handle != 0 {
        preamble.handle
    } else {
        layer_handle
    };
    let total_bits = reader.total_bits();
    if u64::from(start_bit) >= total_bits {
        return None;
    }
    reader.set_bit_pos(start_bit);
    // control + xdictionary + xref block + plotstyle + material + linetype + unknown.
    let max_refs = preamble.num_reactors as usize + usize::from(!preamble.xdic_missing) + 6;
    for _ in 0..max_refs.min(64) {
        if reader.tell_bits() >= total_bits {
            break;
        }
        let Ok(value) = entities::common::read_handle_reference(&mut reader, base_handle) else {
            break;
        };
        if linetype_handles.contains(&value) {
            return Some(value);
        }
    }
    None
}

/// Linetype of every layer: `(layer_handle, linetype_handle)`.
///
/// Layers whose linetype handle cannot be read are omitted.
#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_linetypes(path: &str, limit: Option<usize>) -> PyResult<Vec<LayerLinetypeRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let linetype_handles = collect_object_handles_by_type(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        0x39,
        "LTYPE",
    )?;
    let mut result = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x33, "LAYER", &dynamic_types) {
            continue;
        }
        let Some(linetype_handle) = decode_layer_linetype_handle(
            &record,
            &header,
            decoder.version(),
            obj.handle.0,
            &linetype_handles,
        ) else {
            continue;
        };
        if !seen.insert(obj.handle.0) {
            continue;
        }
        result.push((obj.handle.0, linetype_handle));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

/// Common entity data and handles of any entity record, whatever its type.
fn decode_common_entity_header_and_handles(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
) -> Option<(
    entities::common::CommonEntityHeader,
    entities::common::CommonEntityHandles,
)> {
    read_common_entity_header_and_handles(record, version, header)
        .map(|(_reader, common, handles)| (common, handles))
}

/// Like `decode_common_entity_header_and_handles`, and also returns the reader,
/// left at the first handle after the common ones (the type-specific handles).
fn read_common_entity_header_and_handles<'a>(
    record: &'a objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
) -> Option<(
    BitReader<'a>,
    entities::common::CommonEntityHeader,
    entities::common::CommonEntityHandles,
)> {
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version).ok()?;
    let common = match version {
        version::DwgVersion::R13 | version::DwgVersion::R14 => {
            entities::common::parse_common_entity_header_r14(&mut reader).ok()?
        }
        version::DwgVersion::R2000 | version::DwgVersion::R2004 => {
            entities::common::parse_common_entity_header(&mut reader).ok()?
        }
        version::DwgVersion::R2007 => {
            entities::common::parse_common_entity_header_r2007(&mut reader).ok()?
        }
        version::DwgVersion::R2010 => parse_dim_common_header_r2010_plus_with_candidates(
            &mut reader,
            header,
            |candidate_reader, end_bit| {
                entities::common::parse_common_entity_header_r2010(candidate_reader, end_bit)
            },
        )?,
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            parse_dim_common_header_r2010_plus_with_candidates(
                &mut reader,
                header,
                |candidate_reader, end_bit| {
                    entities::common::parse_common_entity_header_r2013(candidate_reader, end_bit)
                },
            )?
        }
        _ => return None,
    };

    reader.set_bit_pos(common.obj_size);
    let handles = entities::common::parse_common_entity_handles(&mut reader, &common).ok()?;
    Some((reader, common, handles))
}

/// Linetype of every entity:
/// `(handle, layer_handle, linetype_flags, linetype_handle, linetype_scale)`.
///
/// `linetype_flags` is 0 = BYLAYER (the linetype of `layer_handle`, see
/// `decode_layer_linetypes`), 1 = BYBLOCK, 2 = CONTINUOUS, 3 = the linetype named
/// by `linetype_handle` (a LTYPE record, see `decode_linetypes`).
/// `linetype_scale` is the entity's own scale (DXF group 48), 1.0 by default.
///
/// The rows come from the common entity data alone, so they cover every entity
/// type, including the ones without a geometry decoder.
#[pyfunction(signature = (path, limit=None))]
pub fn decode_entity_linetypes(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<EntityLinetypeRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let dynamic_type_classes = load_dynamic_type_classes(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        if resolved_type_class(header.type_code, &type_name, &dynamic_type_classes) != "E" {
            continue;
        }
        let Some((common, handles)) =
            decode_common_entity_header_and_handles(&record, decoder.version(), &header)
        else {
            continue;
        };
        if !seen.insert(obj.handle.0) {
            continue;
        }
        let scale = if common.ltype_scale.is_finite() && common.ltype_scale > 0.0 {
            common.ltype_scale
        } else {
            1.0
        };
        result.push((
            obj.handle.0,
            handles.layer,
            common.ltype_flags,
            handles.ltype,
            scale,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

/// Lineweight and visibility of every entity: `(handle, lineweight, invisible)`.
///
/// `lineweight` is the value of DXF group 370: hundredths of a millimetre,
/// -1 = BYLAYER, -2 = BYBLOCK, -3 = the default lineweight. It is `None` when
/// the file stores none (R13/R14) or the stored value is not a lineweight.
/// `invisible` is DXF group 60: the entity is not displayed.
///
/// Like `decode_entity_linetypes`, the rows come from the common entity data
/// alone and cover every entity type.
#[pyfunction(signature = (path, limit=None))]
pub fn decode_entity_lineweights(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<EntityLineweightRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let dynamic_type_classes = load_dynamic_type_classes(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        if resolved_type_class(header.type_code, &type_name, &dynamic_type_classes) != "E" {
            continue;
        }
        let Some((common, _handles)) =
            decode_common_entity_header_and_handles(&record, decoder.version(), &header)
        else {
            continue;
        };
        if !seen.insert(obj.handle.0) {
            continue;
        }
        result.push((
            obj.handle.0,
            common
                .line_weight
                .and_then(entities::common::lineweight_from_index),
            common.invisible,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

/// LAYER record (ODA specification 20.4.53): the state bits and the lineweight.
///
/// R2000+ pack them into one "Values" BS: frozen (bit 0), off (bit 1), frozen in
/// new viewports (bit 2), locked (bit 3), plot (bit 4) and the lineweight index
/// (bits 5-9). R13/R14 store the first four as single bits and have neither a
/// plot flag nor lineweights.
fn decode_layer_state_record(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
) -> crate::core::result::Result<(bool, bool, bool, bool, bool, i16)> {
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version)?;
    let _preamble = read_table_record_preamble(&mut reader, version, api_header)?;
    if table_record_strings_in_stream(version) {
        // R2007+ fold the 64-flag and the xref dependency bit into this one value.
        let _xref_index_plus_one = reader.read_bs()?;
    } else {
        let _name = reader.read_tv()?;
        let _flag_64 = reader.read_b()?;
        let _xref_index_plus_one = reader.read_bs()?;
        let _xdep = reader.read_b()?;
    }
    if matches!(version, version::DwgVersion::R13 | version::DwgVersion::R14) {
        let frozen = reader.read_b()? != 0;
        let off = reader.read_b()? != 0;
        let frozen_in_new_viewports = reader.read_b()? != 0;
        let locked = reader.read_b()? != 0;
        return Ok((frozen, off, frozen_in_new_viewports, locked, true, -3));
    }
    let values = reader.read_bs()?;
    let lineweight =
        entities::common::lineweight_from_index(((values & 0x03E0) >> 5) as u8).unwrap_or(-3);
    Ok((
        values & 0x01 != 0,
        values & 0x02 != 0,
        values & 0x04 != 0,
        values & 0x08 != 0,
        values & 0x10 != 0,
        lineweight,
    ))
}

/// State of every layer:
/// `(layer_handle, frozen, off, frozen_in_new_viewports, locked, plot, lineweight)`.
///
/// A frozen or off layer is not displayed; `plot` is false for a layer that is
/// displayed but not plotted (DXF group 290 = 0). `lineweight` is the value of
/// DXF group 370: hundredths of a millimetre, or -3 for the default lineweight.
/// R13/R14 have no plot flag and no lineweights: `plot` is true and
/// `lineweight` is -3 there. Layers whose record cannot be read are omitted.
#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_states(path: &str, limit: Option<usize>) -> PyResult<Vec<LayerStateRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x33, "LAYER", &dynamic_types) {
            continue;
        }
        let Ok((frozen, off, frozen_in_new_viewports, locked, plot, lineweight)) =
            decode_layer_state_record(&record, &header, decoder.version())
        else {
            continue;
        };
        if !seen.insert(obj.handle.0) {
            continue;
        }
        result.push((
            obj.handle.0,
            frozen,
            off,
            frozen_in_new_viewports,
            locked,
            plot,
            lineweight,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}


/// DIMSTYLE record (ODA specification 20.4.68), as far as the sizes that scale
/// a dimension: `(name, DIMSCALE, DIMASZ, DIMTXT)`.
///
/// R13/R14 store the flags and codes of the style in front of the sizes;
/// R2000+ start with DIMPOST and DIMAPOST (strings, which R2007+ keep in the
/// string stream) and put a second group of flags between DIMTM and DIMTXT.
fn decode_dimstyle_record(
    record: &objects::ObjectRecord<'_>,
    api_header: &ApiObjectHeader,
    version: &version::DwgVersion,
) -> crate::core::result::Result<(String, f64, f64, f64)> {
    let mut reader = record.bit_reader();
    skip_object_type_prefix(&mut reader, version)?;
    let preamble = read_table_record_preamble(&mut reader, version, api_header)?;
    let strings_in_stream = table_record_strings_in_stream(version);
    let name = if strings_in_stream {
        // R2007+ fold the 64-flag and the xref dependency bit into this one value.
        let _xref_index_plus_one = reader.read_bs()?;
        read_table_record_stream_strings(&reader, preamble.data_end_bit, 1)
            .into_iter()
            .next()
            .unwrap_or_default()
    } else {
        let name = reader.read_tv()?;
        let _flag_64 = reader.read_b()?;
        let _xref_index_plus_one = reader.read_bs()?;
        let _xdep = reader.read_b()?;
        name
    };

    if matches!(version, version::DwgVersion::R13 | version::DwgVersion::R14) {
        // DIMTOL .. DIMSOXD
        for _ in 0..11 {
            let _flag = reader.read_b()?;
        }
        let _dimaltd = reader.read_rc()?;
        let _dimzin = reader.read_rc()?;
        let _dimsd1 = reader.read_b()?;
        let _dimsd2 = reader.read_b()?;
        let _dimtolj = reader.read_rc()?;
        let _dimjust = reader.read_rc()?;
        let _dimfit = reader.read_rc()?;
        let _dimupt = reader.read_b()?;
        // DIMTZIN, DIMALTZ, DIMALTTZ, DIMTAD
        for _ in 0..4 {
            let _code = reader.read_rc()?;
        }
        // DIMUNIT .. DIMALTTD
        for _ in 0..6 {
            let _code = reader.read_bs()?;
        }
    } else if !strings_in_stream {
        let _dimpost = reader.read_tv()?;
        let _dimapost = reader.read_tv()?;
    }

    let dimscale = reader.read_bd()?;
    let dimasz = reader.read_bd()?;
    // DIMEXO, DIMDLI, DIMEXE, DIMRND, DIMDLE, DIMTP, DIMTM
    for _ in 0..7 {
        let _size = reader.read_bd()?;
    }
    if !matches!(version, version::DwgVersion::R13 | version::DwgVersion::R14) {
        if strings_in_stream {
            let _dimfxl = reader.read_bd()?;
            let _dimjogang = reader.read_bd()?;
            let _dimtfill = reader.read_bs()?;
            // DIMTFILLCLR (CMC): index, RGB and the color byte; its names are
            // in the string stream.
            let _color_index = reader.read_bs()?;
            let _color_rgb = reader.read_bl()?;
            let _color_byte = reader.read_rc()?;
        }
        // DIMTOL, DIMLIM, DIMTIH, DIMTOH, DIMSE1, DIMSE2
        for _ in 0..6 {
            let _flag = reader.read_b()?;
        }
        let _dimtad = reader.read_bs()?;
        let _dimzin = reader.read_bs()?;
        let _dimazin = reader.read_bs()?;
        if strings_in_stream {
            let _dimarcsym = reader.read_bs()?;
        }
    }
    let dimtxt = reader.read_bd()?;

    if ![dimscale, dimasz, dimtxt]
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0 && *value < 1.0e12)
    {
        return Err(DwgError::new(
            ErrorKind::Format,
            "DIMSTYLE sizes are out of range",
        ));
    }
    Ok((name, dimscale, dimasz, dimtxt))
}

/// Sizes of every dimension style: `(handle, name, dimscale, dimasz, dimtxt)`.
///
/// `dimtxt` is the text height and `dimasz` the arrow size of the style, in
/// drawing units before `dimscale` (the overall scale; 0 for a style that is
/// scaled by the viewport or annotatively).
#[pyfunction(signature = (path, limit=None))]
pub fn decode_dimstyles(path: &str, limit: Option<usize>) -> PyResult<Vec<DimStyleSizeRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    let mut seen: HashSet<u64> = HashSet::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x45, "DIMSTYLE", &dynamic_types) {
            continue;
        }
        let Ok((name, dimscale, dimasz, dimtxt)) =
            decode_dimstyle_record(&record, &header, decoder.version())
        else {
            continue;
        };
        if !seen.insert(obj.handle.0) {
            continue;
        }
        result.push((obj.handle.0, name, dimscale, dimasz, dimtxt));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}
