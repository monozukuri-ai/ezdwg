fn start_delta_candidates_for_type(type_code: u16) -> &'static [i32] {
    const DEFAULT: &[i32] = &[-8, -4, 0, 4, 8];
    const HEADER_LIKE: &[i32] = &[-16, -8, -4, 0, 4, 8, 16];
    const PAYLOAD_LIKE: &[i32] = &[-32, -24, -16, -8, -4, 0, 4, 8, 16, 24, 32];
    match type_code {
        0x214 | 0x221 => HEADER_LIKE,
        0x222 | 0x223 | 0x224 | 0x225 => PAYLOAD_LIKE,
        _ => DEFAULT,
    }
}

fn preferred_ref_type_codes_for_acis_unknown(type_code: u16) -> &'static [u16] {
    match type_code {
        // HEADER-like records usually point back to owner 3DSOLID/BODY/REGION or link table.
        0x221 => &[0x26, 0x27, 0x25, 0x214],
        // Link table records are expected to link to header/payload records.
        0x214 => &[0x221, 0x222, 0x223, 0x224, 0x225],
        // Payload chunks often refer to link table/header and sometimes sibling payload chunks.
        0x222 | 0x223 | 0x224 | 0x225 => &[0x214, 0x221, 0x222, 0x223, 0x224, 0x225],
        _ => &[],
    }
}

fn resolve_handle_stream_start_candidates(
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    type_code: u16,
) -> Vec<u32> {
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return Vec::new();
    }
    let total_bits = header.data_size.saturating_mul(8);
    let mut bases = resolve_r2010_object_data_end_bit_candidates(header);
    if let Ok(canonical) = resolve_r2010_object_data_end_bit(header) {
        bases.push(canonical);
    }
    let mut out = Vec::new();
    for base in bases {
        for delta in start_delta_candidates_for_type(type_code).iter().copied() {
            let candidate_i64 = i64::from(base) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            if candidate >= total_bits {
                continue;
            }
            out.push(candidate);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[derive(Default)]
struct KnownHandleRefsDecode {
    refs: Vec<u64>,
    confidence: u8,
}

fn derive_known_handle_refs_confidence(
    refs_len: usize,
    quality_score: i64,
    best_score: i64,
    second_score: Option<i64>,
) -> u8 {
    if refs_len == 0 {
        return 0;
    }
    let mut confidence = 8i64;
    confidence = confidence.saturating_add(i64::try_from(refs_len.min(8)).unwrap_or(0) * 7);
    if quality_score > 0 {
        confidence = confidence.saturating_add(quality_score.min(12) * 3);
    }
    if let Some(second) = second_score {
        let margin = best_score.saturating_sub(second);
        let margin_boost = if margin >= 48 {
            26
        } else if margin >= 24 {
            18
        } else if margin >= 12 {
            12
        } else if margin >= 6 {
            7
        } else if margin > 0 {
            3
        } else {
            0
        };
        confidence = confidence.saturating_add(margin_boost);
    } else {
        // Only one candidate decoded successfully: moderate confidence, not maximal.
        confidence = confidence.saturating_add(14);
    }
    confidence.clamp(0, 100) as u8
}

fn acis_unknown_role_hint_from_type_code(type_code: u16, data_size: u32) -> &'static str {
    match type_code {
        0x214 => "acis-link-table",
        0x221 => "acis-header",
        0x222 => "acis-payload-chunk",
        0x223 | 0x224 | 0x225 => {
            if data_size >= 128 {
                "acis-payload-main"
            } else {
                "acis-payload-chunk"
            }
        }
        0x215..=0x220 => "acis-aux",
        _ if (0x214..=0x225).contains(&type_code) => "acis-aux",
        _ => "unknown",
    }
}

fn is_plausible_line_entity_candidate(entity: &entities::LineEntity) -> bool {
    let values = [
        entity.start.0,
        entity.start.1,
        entity.start.2,
        entity.end.0,
        entity.end.1,
        entity.end.2,
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return false;
    }
    let max_abs = values
        .iter()
        .fold(0.0_f64, |acc, value| acc.max(value.abs()));
    if max_abs > 1.0e8 {
        return false;
    }
    true
}

fn decode_attrib_like_entities_by_type<F>(
    path: &str,
    limit: Option<usize>,
    type_code: u16,
    type_name: &str,
    mut decode_entity: F,
) -> PyResult<Vec<AttribEntityRow>>
where
    F: FnMut(
        &mut BitReader<'_>,
        &version::DwgVersion,
        &ApiObjectHeader,
        u64,
    ) -> crate::core::result::Result<entities::AttribEntity>,
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
        let entity = match decode_entity(&mut reader, decoder.version(), &header, obj.handle.0) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.text,
            entity.tag,
            entity.prompt,
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
            entity.flags,
            entity.lock_position,
            (entity.style_handle, entity.owner_handle),
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

fn decode_dim_entities_by_type<F>(
    path: &str,
    limit: Option<usize>,
    type_code: u16,
    type_name: &str,
    allow_minimal_fallback: bool,
    mut decode_entity_row: F,
) -> PyResult<Vec<DimEntityRow>>
where
    F: FnMut(
        &mut BitReader<'_>,
        &version::DwgVersion,
        &ApiObjectHeader,
        u64,
    ) -> crate::core::result::Result<entities::DimLinearEntity>,
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

        let row = match decode_dim_linear_like_entity_with_prefix_fallback(
            &record,
            decoder.version(),
            &header,
            obj.handle.0,
            allow_minimal_fallback,
            &mut decode_entity_row,
        ) {
            Ok(entity) => dim_entity_row_from_linear_like(&entity),
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push(row);

        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn decode_dim_linear_like_entity_with_prefix_fallback<F>(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
    allow_minimal_fallback: bool,
    mut decode_entity: F,
) -> crate::core::result::Result<entities::DimLinearEntity>
where
    F: FnMut(
        &mut BitReader<'_>,
        &version::DwgVersion,
        &ApiObjectHeader,
        u64,
    ) -> crate::core::result::Result<entities::DimLinearEntity>,
{
    let mut best_candidate: Option<entities::DimLinearEntity> = None;
    let mut best_candidate_score = u64::MAX;
    let mut last_error: Option<DwgError> = None;

    for with_prefix in [true, false] {
        let mut reader = record.bit_reader();
        if with_prefix {
            if let Err(err) = skip_object_type_prefix(&mut reader, version) {
                last_error = Some(err);
                continue;
            }
        }

        match decode_entity(&mut reader, version, header, object_handle) {
            Ok(entity) => {
                let score = dim_linear_entity_plausibility_score(&entity);
                match &best_candidate {
                    Some(_) if score >= best_candidate_score => {}
                    _ => {
                        best_candidate_score = score;
                        best_candidate = Some(entity);
                    }
                }
            }
            Err(err) => last_error = Some(err),
        }
    }

    if let Some(entity) = best_candidate {
        return Ok(entity);
    }

    if allow_minimal_fallback {
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix && skip_object_type_prefix(&mut reader, version).is_err() {
                continue;
            }
            if let Ok(entity) = decode_dim_linear_like_entity_minimal_for_version(
                &mut reader,
                version,
                header,
                object_handle,
            ) {
                return Ok(entity);
            }
        }
        return Ok(build_ultra_minimal_dim_linear_entity(object_handle));
    }

    Err(last_error.unwrap_or_else(|| {
        DwgError::new(
            ErrorKind::Decode,
            "failed to decode dimension entity with prefix variants",
        )
    }))
}

fn parse_dim_common_header_r2010_plus_with_candidates<F>(
    reader: &mut BitReader<'_>,
    header: &ApiObjectHeader,
    mut parse: F,
) -> Option<entities::common::CommonEntityHeader>
where
    F: FnMut(
        &mut BitReader<'_>,
        u32,
    ) -> crate::core::result::Result<entities::common::CommonEntityHeader>,
{
    let start = reader.get_pos();
    let canonical_end_bit = resolve_r2010_object_data_end_bit(header).ok();
    let mut end_bit_candidates = resolve_r2010_object_data_end_bit_candidates(header);
    if let Some(canonical) = canonical_end_bit {
        for delta in (-256i32..=256).step_by(8) {
            let candidate_i64 = i64::from(canonical) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            if let Ok(candidate) = u32::try_from(candidate_i64) {
                end_bit_candidates.push(candidate);
            }
        }
    }
    let mut size_reader = reader.clone();
    if let Ok(obj_size_bits) = size_reader.read_rl(Endian::Little) {
        for delta in (-128i32..=128).step_by(8) {
            let candidate_i64 = i64::from(obj_size_bits) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            if let Ok(candidate) = u32::try_from(candidate_i64) {
                end_bit_candidates.push(candidate);
            }
        }
    }
    end_bit_candidates.push(header.data_size.saturating_mul(8));
    if let Some(canonical) = canonical_end_bit {
        if !end_bit_candidates.contains(&canonical) {
            end_bit_candidates.push(canonical);
        }
        end_bit_candidates.sort_by_key(|candidate| candidate.abs_diff(canonical));
    } else {
        end_bit_candidates.sort_unstable();
    }
    end_bit_candidates.retain(|candidate| *candidate <= header.data_size.saturating_mul(8));
    end_bit_candidates.dedup();
    for end_bit in end_bit_candidates {
        reader.set_pos(start.0, start.1);
        if let Ok(candidate) = parse(reader, end_bit) {
            return Some(candidate);
        }
    }
    None
}

fn point_plausibility_score(point: (f64, f64, f64)) -> u64 {
    let mut score = 0u64;
    for value in [point.0, point.1, point.2] {
        score = score.saturating_add(value_plausibility_score(value));
    }
    score
}

fn value_plausibility_score(value: f64) -> u64 {
    if !value.is_finite() {
        return 1_000_000;
    }
    let abs = value.abs();
    if abs > 1.0e12 {
        100_000
    } else if abs > 1.0e9 {
        10_000
    } else if abs > 1.0e6 {
        500
    } else {
        0
    }
}

#[derive(Debug, Clone)]
struct Polyline3dVertexRow {
    handle: u64,
    flags_70_bits: u8,
    closed: bool,
    vertices: Vec<entities::Vertex3dEntity>,
}

#[derive(Debug, Clone)]
struct PolylineMeshVertexRow {
    handle: u64,
    flags: u16,
    m_vertex_count: u16,
    n_vertex_count: u16,
    closed: bool,
    vertices: Vec<entities::Vertex3dEntity>,
}

#[derive(Debug, Clone)]
struct PolylinePFaceRow {
    handle: u64,
    num_vertices: u16,
    num_faces: u16,
    vertices: Vec<entities::Vertex3dEntity>,
    faces: Vec<entities::VertexPFaceFaceEntity>,
}

#[derive(Debug, Clone)]
struct PolylineVertexRow {
    handle: u64,
    flags: u16,
    flags_info: entities::PolylineFlagsInfo,
    curve_type_info: entities::PolylineCurveType,
    elevation: f64,
    vertices: Vec<entities::Vertex2dEntity>,
}

fn has_large_adjacent_jump(vertices: &[entities::Vertex2dEntity], index: usize) -> bool {
    let mut max_jump = 0.0f64;
    if index > 0 {
        max_jump = max_jump.max(distance_2d(
            vertices[index - 1].position,
            vertices[index].position,
        ));
    }
    if index + 1 < vertices.len() {
        max_jump = max_jump.max(distance_2d(
            vertices[index].position,
            vertices[index + 1].position,
        ));
    }
    max_jump >= 1000.0
}

fn distance_2d(a: Point3, b: Point3) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolylineSequenceKind {
    Polyline2d,
    Polyline3d,
    PolylineMesh,
    PolylinePFace,
}

impl PolylineSequenceKind {
    fn label(self) -> &'static str {
        match self {
            Self::Polyline2d => "POLYLINE_2D",
            Self::Polyline3d => "POLYLINE_3D",
            Self::PolylineMesh => "POLYLINE_MESH",
            Self::PolylinePFace => "POLYLINE_PFACE",
        }
    }
}

fn is_best_effort_compat_version(decoder: &decoder::Decoder<'_>) -> bool {
    matches!(
        decoder.version(),
        version::DwgVersion::R14
            | version::DwgVersion::R2000
            | version::DwgVersion::R2010
            | version::DwgVersion::R2013
            | version::DwgVersion::R2018
    )
}

fn resolve_r2010_object_data_end_bit(header: &ApiObjectHeader) -> crate::core::result::Result<u32> {
    let total_bits = header
        .data_size
        .checked_mul(8)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "object size bits overflow"))?;
    let handle_bits = header
        .handle_stream_size_bits
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "missing R2010 handle stream size"))?;
    total_bits.checked_sub(handle_bits).ok_or_else(|| {
        DwgError::new(
            ErrorKind::Format,
            "R2010 handle stream exceeds object data size",
        )
    })
}

fn resolve_r2010_object_data_end_bit_candidates(header: &ApiObjectHeader) -> Vec<u32> {
    let total_bits = header.data_size.saturating_mul(8);
    let Some(handle_bits) = header.handle_stream_size_bits else {
        return Vec::new();
    };

    let bases = [
        total_bits.saturating_sub(handle_bits),
        total_bits.saturating_sub(handle_bits.saturating_sub(8)),
    ];
    let deltas = [-16i32, -8, 0, 8, 16];

    let mut out = Vec::new();
    for base in bases {
        for delta in deltas {
            let candidate_i64 = i64::from(base) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            if candidate > total_bits {
                continue;
            }
            out.push(candidate);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn skip_object_type_prefix(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
) -> crate::core::result::Result<u16> {
    match version {
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let _handle_stream_size_bits = reader.read_umc()?;
            let type_code = reader.read_ot_r2010()?;
            if type_code == 0 {
                return Err(DwgError::new(ErrorKind::Format, "object type code is zero"));
            }
            Ok(type_code)
        }
        _ => {
            let type_code = reader.read_bs()?;
            if type_code == 0 {
                return Err(DwgError::new(ErrorKind::Format, "object type code is zero"));
            }
            Ok(type_code)
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ApiObjectHeader {
    data_size: u32,
    type_code: u16,
    handle_stream_size_bits: Option<u32>,
}

fn parse_object_header_for_version(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
) -> crate::core::result::Result<ApiObjectHeader> {
    match version {
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let header = objects::object_header_r2010::parse_from_record(record)?;
            Ok(ApiObjectHeader {
                data_size: header.data_size,
                type_code: header.type_code,
                handle_stream_size_bits: Some(header.handle_stream_size_bits),
            })
        }
        _ => {
            let header = objects::object_header_r2000::parse_from_record(record)?;
            Ok(ApiObjectHeader {
                data_size: header.data_size,
                type_code: header.type_code,
                handle_stream_size_bits: None,
            })
        }
    }
}

fn parse_record_and_header<'a>(
    decoder: &decoder::Decoder<'a>,
    offset: u32,
    best_effort: bool,
) -> PyResult<Option<(objects::ObjectRecord<'a>, ApiObjectHeader)>> {
    let record = match decoder.parse_object_record(offset) {
        Ok(record) => record,
        Err(err) if best_effort => return Ok(None),
        Err(err) => return Err(to_py_err(err)),
    };
    let header = match parse_object_header_for_version(&record, decoder.version()) {
        Ok(header) => header,
        Err(err) if best_effort => return Ok(None),
        Err(err) => return Err(to_py_err(err)),
    };
    Ok(Some((record, header)))
}

fn load_dynamic_types(
    decoder: &decoder::Decoder<'_>,
    best_effort: bool,
) -> PyResult<HashMap<u16, String>> {
    match decoder.dynamic_type_map() {
        Ok(map) => Ok(map),
        Err(_) if best_effort => Ok(HashMap::new()),
        Err(err) => Err(to_py_err(err)),
    }
}

fn collect_object_type_codes(
    decoder: &decoder::Decoder<'_>,
    index: &objects::ObjectIndex,
    best_effort: bool,
) -> PyResult<HashMap<u64, u16>> {
    let mut object_types: HashMap<u64, u16> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((_record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        object_types.insert(obj.handle.0, header.type_code);
    }
    Ok(object_types)
}

fn resolve_r2010_string_stream_ranges(
    base_reader: &BitReader<'_>,
    end_bit: u32,
) -> Vec<(u32, u32)> {
    if end_bit <= 1 {
        return Vec::new();
    }
    let mut present_reader = base_reader.clone();
    present_reader.set_bit_pos(end_bit.saturating_sub(1));
    let Ok(has_string_stream) = present_reader.read_b() else {
        return Vec::new();
    };
    if has_string_stream == 0 {
        return Vec::new();
    }

    let mut size_field_start = end_bit.saturating_sub(1);
    if size_field_start < 16 {
        return Vec::new();
    }
    size_field_start = size_field_start.saturating_sub(16);
    let mut size_reader = base_reader.clone();
    size_reader.set_bit_pos(size_field_start);
    let Ok(low_size_signed) = size_reader.read_rs(Endian::Little) else {
        return Vec::new();
    };
    let mut stream_size = u32::from(low_size_signed as u16);
    if (stream_size & 0x8000) != 0 {
        if size_field_start < 16 {
            return Vec::new();
        }
        size_field_start = size_field_start.saturating_sub(16);
        let mut hi_reader = base_reader.clone();
        hi_reader.set_bit_pos(size_field_start);
        let Ok(high_size_signed) = hi_reader.read_rs(Endian::Little) else {
            return Vec::new();
        };
        let high_size = u32::from(high_size_signed as u16);
        stream_size = (stream_size & 0x7FFF) | (high_size << 15);
    }

    let mut ranges = Vec::new();
    for multiplier in [1u32, 8u32] {
        let Some(size_bits) = stream_size.checked_mul(multiplier) else {
            continue;
        };
        if size_field_start < size_bits {
            continue;
        }
        let start_bit = size_field_start.saturating_sub(size_bits);
        if start_bit >= size_field_start {
            continue;
        }
        ranges.push((start_bit, size_field_start));
    }
    ranges.sort_unstable();
    ranges.dedup();
    ranges
}

fn recover_r2010_mtext_text(
    reader_after_prefix: &BitReader<'_>,
    header: &ApiObjectHeader,
    inline_text: &str,
) -> Option<String> {
    let total_bits = header.data_size.saturating_mul(8);
    let start_bit = reader_after_prefix.tell_bits() as u32;
    if total_bits <= start_bit.saturating_add(16) {
        return None;
    }

    let mut end_bit_candidates = resolve_r2010_object_data_end_bit_candidates(header);
    end_bit_candidates.push(total_bits);
    end_bit_candidates.retain(|candidate| *candidate > start_bit && *candidate <= total_bits);
    end_bit_candidates.sort_unstable();
    end_bit_candidates.dedup();
    if end_bit_candidates.is_empty() {
        return None;
    }

    let current_score = score_mtext_text_quality(inline_text);
    let canonical_end_bit = resolve_r2010_object_data_end_bit(header).ok();
    let mut best: Option<(u64, String)> = None;
    for end_bit in end_bit_candidates {
        for (stream_start_bit, stream_end_bit) in
            resolve_r2010_string_stream_ranges(reader_after_prefix, end_bit)
        {
            if let Some((mut score, text)) = scan_mtext_text_in_string_stream(
                reader_after_prefix,
                stream_start_bit,
                stream_end_bit,
            ) {
                if let Some(canonical) = canonical_end_bit {
                    score = score.saturating_add(canonical.abs_diff(end_bit) as u64);
                }
                match &best {
                    Some((best_score, _)) if score >= *best_score => {}
                    _ => best = Some((score, text)),
                }
            }
        }
    }
    let Some((best_score, best_text)) = best else {
        return None;
    };
    if best_score.saturating_add(32) < current_score {
        Some(best_text)
    } else {
        None
    }
}

fn scan_mtext_text_in_string_stream(
    base_reader: &BitReader<'_>,
    start_bit: u32,
    end_bit: u32,
) -> Option<(u64, String)> {
    if start_bit >= end_bit {
        return None;
    }
    let mut best: Option<(u64, String)> = None;
    let mut bit = start_bit;
    let mut tried = 0u32;
    let max_tries = end_bit
        .saturating_sub(start_bit)
        .saturating_div(8)
        .saturating_add(2)
        .min(65_536);
    while bit + 16 <= end_bit && tried < max_tries {
        let mut candidate_reader = base_reader.clone();
        candidate_reader.set_bit_pos(bit);
        let Ok(candidate) = read_tu(&mut candidate_reader) else {
            bit = bit.saturating_add(8);
            tried = tried.saturating_add(1);
            continue;
        };
        if candidate_reader.tell_bits() > end_bit as u64 || !is_plausible_mtext_text(&candidate) {
            bit = bit.saturating_add(8);
            tried = tried.saturating_add(1);
            continue;
        }

        let trailing_gap_bits = end_bit as u64 - candidate_reader.tell_bits();
        let score = score_mtext_text_quality(&candidate).saturating_add(trailing_gap_bits / 64);
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, candidate)),
        }
        bit = bit.saturating_add(8);
        tried = tried.saturating_add(1);
    }
    best
}

fn is_plausible_mtext_text(text: &str) -> bool {
    let len = text.chars().count();
    if !(2..=4096).contains(&len) {
        return false;
    }
    if text.contains('\u{0000}') || text.contains('\u{fffd}') {
        return false;
    }
    let mut has_meaningful = false;
    for ch in text.chars() {
        if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
            return false;
        }
        if ch.is_alphanumeric() || ch.is_whitespace() || ch.is_ascii_punctuation() {
            has_meaningful = true;
        }
    }
    has_meaningful
}

fn score_mtext_text_quality(text: &str) -> u64 {
    if text.is_empty() {
        return 1_000_000;
    }
    let len = text.chars().count() as u64;
    let mut score = 0u64;
    if len <= 1 {
        score = score.saturating_add(50_000);
    } else if len == 2 {
        score = score.saturating_add(5_000);
    }
    if len > 4096 {
        score = score.saturating_add((len - 4096) * 10);
    }

    let mut meaningful = 0u64;
    for ch in text.chars() {
        if ch == '\u{fffd}' || ch == '\u{0000}' {
            score = score.saturating_add(10_000);
            continue;
        }
        if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
            score = score.saturating_add(5_000);
            continue;
        }
        if ch.is_alphanumeric() || ch.is_whitespace() || ch.is_ascii_punctuation() {
            meaningful = meaningful.saturating_add(1);
        } else if !ch.is_control() {
            // Treat non-ASCII printable glyphs (e.g. CJK, symbols) as meaningful.
            meaningful = meaningful.saturating_add(1);
        }
    }
    if meaningful == 0 {
        score = score.saturating_add(25_000);
    }
    score
}

#[derive(Clone, Copy)]
struct LayerColorParseVariant {
    pre_flag_bits: u8,
    post_flag_bits: u8,
    pre_values_bits: u8,
}

fn skip_eed(reader: &mut BitReader<'_>) -> crate::core::result::Result<()> {
    let mut ext_size = reader.read_bs()?;
    while ext_size > 0 {
        let _app_handle = reader.read_h()?;
        for _ in 0..ext_size {
            let _ = reader.read_rc()?;
        }
        ext_size = reader.read_bs()?;
    }
    Ok(())
}

fn is_recoverable_decode_error(err: &DwgError) -> bool {
    matches!(
        err.kind,
        ErrorKind::NotImplemented | ErrorKind::Decode | ErrorKind::Format
    )
}

fn build_decoder(bytes: &[u8]) -> crate::core::result::Result<decoder::Decoder<'_>> {
    decoder::Decoder::new(bytes, Default::default())
}

fn to_py_err(err: DwgError) -> PyErr {
    let message = err.to_string();
    match err.kind {
        ErrorKind::Io => PyIOError::new_err(message),
        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Resolve | ErrorKind::Unsupported => {
            PyValueError::new_err(message)
        }
        ErrorKind::NotImplemented => PyNotImplementedError::new_err(message),
    }
}

fn points_equal_3d(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
    const EPS: f64 = 1e-9;
    (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS && (a.2 - b.2).abs() < EPS
}

fn strip_closure(mut points: Vec<(f64, f64, f64)>) -> Vec<(f64, f64, f64)> {
    if points.len() > 1 {
        let first = points[0];
        let last = *points.last().unwrap();
        if points_equal_3d(first, last) {
            points.pop();
        }
    }
    points
}

fn points_equal_3d_with_data(
    a: (f64, f64, f64, f64, f64, f64, f64, u16),
    b: (f64, f64, f64, f64, f64, f64, f64, u16),
) -> bool {
    points_equal_3d((a.0, a.1, a.2), (b.0, b.1, b.2))
}

fn resolved_type_name(type_code: u16, dynamic_types: &HashMap<u16, String>) -> String {
    dynamic_types
        .get(&type_code)
        .cloned()
        .unwrap_or_else(|| objects::object_type_name(type_code))
}

fn resolved_type_class(type_code: u16, resolved_name: &str) -> String {
    let class = objects::object_type_class(type_code).as_str();
    if !class.is_empty() {
        return class.to_string();
    }
    if is_known_entity_type_name(resolved_name) {
        return "E".to_string();
    }
    String::new()
}

fn matches_type_name(
    type_code: u16,
    builtin_code: u16,
    builtin_name: &str,
    dynamic_types: &HashMap<u16, String>,
) -> bool {
    if type_code == builtin_code {
        return true;
    }
    dynamic_types
        .get(&type_code)
        .map(|name| name == builtin_name)
        .unwrap_or(false)
}

fn matches_type_filter(filter: &HashSet<u16>, type_code: u16, resolved_name: &str) -> bool {
    if filter.contains(&type_code) {
        return true;
    }
    if let Some(builtin_code) = builtin_code_from_name(resolved_name) {
        return filter.contains(&builtin_code);
    }
    false
}

fn builtin_code_from_name(name: &str) -> Option<u16> {
    match name {
        "TEXT" => Some(0x01),
        "SEQEND" => Some(0x06),
        "INSERT" => Some(0x07),
        "VERTEX_2D" => Some(0x0A),
        "CIRCLE" => Some(0x12),
        "POLYLINE_2D" => Some(0x0F),
        "ARC" => Some(0x11),
        "LINE" => Some(0x13),
        "POINT" => Some(0x1B),
        "ELLIPSE" => Some(0x23),
        "MTEXT" => Some(0x2C),
        "LWPOLYLINE" => Some(0x4D),
        "DIM_LINEAR" => Some(0x15),
        "DIM_RADIUS" => Some(0x19),
        "DIM_DIAMETER" => Some(0x1A),
        "DIMENSION" => Some(0x15),
        _ => None,
    }
}

fn is_known_entity_type_name(name: &str) -> bool {
    builtin_code_from_name(name).is_some()
}
