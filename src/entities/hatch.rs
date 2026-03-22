use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct HatchPath {
    pub closed: bool,
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct HatchEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub name: String,
    pub solid_fill: bool,
    pub associative: bool,
    pub elevation: f64,
    pub extrusion: (f64, f64, f64),
    pub paths: Vec<HatchPath>,
}

pub fn decode_hatch(reader: &mut BitReader<'_>) -> Result<HatchEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_hatch_with_header(reader, header, false, false, false, false)
}

pub fn decode_hatch_r2004(reader: &mut BitReader<'_>) -> Result<HatchEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_hatch_with_header(reader, header, false, false, true, false)
}

pub fn decode_hatch_r2007(reader: &mut BitReader<'_>) -> Result<HatchEntity> {
    decode_hatch_with_header_start_candidates(
        reader,
        parse_common_entity_header_r2007,
        true,
        true,
        true,
        true,
    )
}

pub fn decode_hatch_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<HatchEntity> {
    decode_hatch_with_header_start_candidates(
        reader,
        |attempt_reader| {
            let mut header = parse_common_entity_header_r2010(attempt_reader, object_data_end_bit)?;
            header.handle = object_handle;
            Ok(header)
        },
        true,
        true,
        true,
        true,
    )
}

pub fn decode_hatch_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<HatchEntity> {
    decode_hatch_with_header_start_candidates(
        reader,
        |attempt_reader| {
            let mut header = parse_common_entity_header_r2013(attempt_reader, object_data_end_bit)?;
            header.handle = object_handle;
            Ok(header)
        },
        true,
        true,
        true,
        true,
    )
}

fn decode_hatch_with_header_start_candidates<F>(
    reader: &mut BitReader<'_>,
    mut parse_header: F,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
    has_gradient_payload: bool,
    use_unicode_text: bool,
) -> Result<HatchEntity>
where
    F: FnMut(&mut BitReader<'_>) -> Result<CommonEntityHeader>,
{
    let start = reader.get_pos();
    let align_candidates = if start.1 == 0 {
        [false, false]
    } else {
        [false, true]
    };
    let mut first_err: Option<DwgError> = None;
    let mut best: Option<(i32, BitReader<'_>, HatchEntity)> = None;

    for align_after_prefix in align_candidates {
        if align_after_prefix && start.1 == 0 {
            continue;
        }
        let mut attempt_reader = reader.clone();
        if align_after_prefix {
            attempt_reader.align_byte();
        }
        let header = match parse_header(&mut attempt_reader) {
            Ok(header) => header,
            Err(err) => {
                if first_err.is_none() {
                    first_err = Some(err);
                }
                continue;
            }
        };
        match decode_hatch_with_header(
            &mut attempt_reader,
            header,
            allow_handle_decode_failure,
            r2007_layer_only,
            has_gradient_payload,
            use_unicode_text,
        ) {
            Ok(entity) => {
                let score = score_hatch_candidate(&entity);
                match &best {
                    Some((best_score, ..)) if score <= *best_score => {}
                    _ => best = Some((score, attempt_reader, entity)),
                }
            }
            Err(err) => {
                if first_err.is_none() {
                    first_err = Some(err);
                }
            }
        }
    }

    if let Some((_score, chosen_reader, entity)) = best {
        *reader = chosen_reader;
        return Ok(entity);
    }

    Err(first_err.unwrap_or_else(|| {
        DwgError::new(
            ErrorKind::Format,
            "failed to decode HATCH header-alignment candidates",
        )
    }))
}

fn decode_hatch_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
    has_gradient_payload: bool,
    use_unicode_text: bool,
) -> Result<HatchEntity> {
    let base_reader = reader.clone();
    if !has_gradient_payload {
        return decode_hatch_body(
            reader,
            header,
            allow_handle_decode_failure,
            r2007_layer_only,
            false,
            use_unicode_text,
        );
    }

    let mut first_err: Option<DwgError> = None;
    let mut best: Option<(i32, BitReader<'_>, HatchEntity)> = None;

    for skip_gradient in [false, true] {
        for string_is_unicode in [use_unicode_text, !use_unicode_text] {
            let mut attempt_reader = reader.clone();
            match decode_hatch_body(
                &mut attempt_reader,
                header.clone(),
                allow_handle_decode_failure,
                r2007_layer_only,
                skip_gradient,
                string_is_unicode,
            ) {
                Ok(entity) => {
                    let score = score_hatch_candidate(&entity);
                    match &best {
                        Some((best_score, ..)) if score <= *best_score => {}
                        _ => best = Some((score, attempt_reader, entity)),
                    }
                }
                Err(err) => {
                    if first_err.is_none() {
                        first_err = Some(err);
                    }
                }
            }
        }
    }

    if let Ok((candidate_reader, entity)) = decode_hatch_with_polyline_path_scan(
        &base_reader,
        &header,
        allow_handle_decode_failure,
        r2007_layer_only,
    ) {
        let score = score_hatch_candidate(&entity);
        match &best {
            Some((best_score, ..)) if score <= *best_score => {}
            _ => best = Some((score, candidate_reader, entity)),
        }
    }

    if let Some((_score, chosen_reader, entity)) = best {
        *reader = chosen_reader;
        return Ok(entity);
    }

    Err(first_err.unwrap_or_else(|| {
        DwgError::new(
            ErrorKind::Format,
            "failed to decode HATCH with all candidates",
        )
    }))
}

fn decode_hatch_with_polyline_path_scan<'a>(
    base_reader: &BitReader<'a>,
    header: &CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<(BitReader<'a>, HatchEntity)> {
    let body_start_bit = base_reader.tell_bits().min(u64::from(u32::MAX)) as u32;
    let total_bits = base_reader.total_bits().min(u64::from(u32::MAX)) as u32;
    let search_end_bit = header.obj_size.min(total_bits);
    if search_end_bit <= body_start_bit.saturating_add(96) {
        return Err(DwgError::new(
            ErrorKind::Format,
            "HATCH scan fallback has no room for path data",
        ));
    }

    let max_start_bit = search_end_bit
        .saturating_sub(96)
        .min(body_start_bit.saturating_add(2048));
    let mut best: Option<(i32, BitReader<'_>, HatchEntity)> = None;

    for start_bit in body_start_bit..=max_start_bit {
        let mut attempt_reader = base_reader.clone();
        attempt_reader.set_bit_pos(start_bit);
        let Ok(paths) = scan_hatch_polyline_paths(&mut attempt_reader, search_end_bit) else {
            continue;
        };

        let solid_fill = hatch_flag_bit_before(base_reader, start_bit, 2).unwrap_or(false);
        let associative = hatch_flag_bit_before(base_reader, start_bit, 1).unwrap_or(false);

        let mut handle_reader = attempt_reader.clone();
        handle_reader.set_bit_pos(header.obj_size);
        let layer_handle = match if r2007_layer_only {
            parse_common_entity_layer_handle(&mut handle_reader, header)
        } else {
            parse_common_entity_handles(&mut handle_reader, header)
                .map(|common_handles| common_handles.layer)
        } {
            Ok(layer_handle) => layer_handle,
            Err(err)
                if allow_handle_decode_failure
                    && matches!(
                        err.kind,
                        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                    ) =>
            {
                0
            }
            Err(err) => return Err(err),
        };

        let entity = HatchEntity {
            handle: header.handle,
            color_index: header.color.index,
            true_color: header.color.true_color,
            layer_handle,
            name: String::new(),
            solid_fill,
            associative,
            elevation: 0.0,
            extrusion: (0.0, 0.0, 1.0),
            paths,
        };

        let start_penalty =
            i32::try_from(start_bit.saturating_sub(body_start_bit) / 8).unwrap_or(0);
        let score = score_hatch_candidate(&entity).saturating_sub(start_penalty);
        match &best {
            Some((best_score, ..)) if score <= *best_score => {}
            _ => best = Some((score, handle_reader, entity)),
        }
    }

    best.map(|(_, reader, entity)| (reader, entity))
        .ok_or_else(|| {
            DwgError::new(
                ErrorKind::Format,
                "failed to recover HATCH polyline paths by scanning",
            )
        })
}

fn decode_hatch_body(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
    skip_gradient: bool,
    use_unicode_text: bool,
) -> Result<HatchEntity> {
    if skip_gradient {
        skip_gradient_payload(reader, use_unicode_text)?;
    }

    let elevation = reader.read_bd()?;
    let extrusion = reader.read_3bd()?;
    let name = if use_unicode_text {
        reader.read_tu()?
    } else {
        reader.read_tv()?
    };
    let solid_fill = reader.read_b()? != 0;
    let associative = reader.read_b()? != 0;

    let num_paths = bounded_count(reader.read_bl()?, "hatch paths")?;
    let mut paths = Vec::with_capacity(num_paths);
    let mut any_path_uses_pixel_size = false;

    for _ in 0..num_paths {
        let path_flag = reader.read_bl()?;
        any_path_uses_pixel_size |= (path_flag & 0x04) != 0;

        if (path_flag & 0x02) == 0 {
            let num_segments = bounded_count(reader.read_bl()?, "hatch edge path segments")?;
            let mut path_points: Vec<(f64, f64)> = Vec::new();
            for _ in 0..num_segments {
                let segment_type = reader.read_rc()?;
                match segment_type {
                    1 => {
                        let start = read_point2rd(reader)?;
                        let end = read_point2rd(reader)?;
                        append_segment_points(&mut path_points, &[start, end]);
                    }
                    2 => {
                        let center = read_point2rd(reader)?;
                        let radius = reader.read_bd()?;
                        let start_angle = reader.read_bd()?;
                        let end_angle = reader.read_bd()?;
                        let is_ccw = reader.read_b()? != 0;
                        let segment =
                            circular_arc_points(center, radius, start_angle, end_angle, is_ccw, 64);
                        append_segment_points(&mut path_points, &segment);
                    }
                    3 => {
                        let center = read_point2rd(reader)?;
                        let major_endpoint = read_point2rd(reader)?;
                        let ratio = reader.read_bd()?;
                        let start_angle = reader.read_bd()?;
                        let end_angle = reader.read_bd()?;
                        let is_ccw = reader.read_b()? != 0;
                        let segment = elliptical_arc_points(
                            center,
                            major_endpoint,
                            ratio,
                            start_angle,
                            end_angle,
                            is_ccw,
                            96,
                        );
                        append_segment_points(&mut path_points, &segment);
                    }
                    4 => {
                        return Err(DwgError::new(
                            ErrorKind::NotImplemented,
                            "HATCH spline edge is not supported yet",
                        ));
                    }
                    _ => {
                        return Err(DwgError::new(
                            ErrorKind::Format,
                            format!("unsupported HATCH edge segment type: {segment_type}"),
                        ));
                    }
                }
            }
            let _num_boundary_obj_handles = reader.read_bl()?;
            close_path_if_needed(&mut path_points);
            paths.push(HatchPath {
                closed: true,
                points: path_points,
            });
            continue;
        }

        let bulges_present = reader.read_b()? != 0;
        let closed = reader.read_b()? != 0;
        let num_vertices = bounded_count(reader.read_bl()?, "hatch polyline vertices")?;
        let mut vertices: Vec<(f64, f64)> = Vec::with_capacity(num_vertices);
        let mut bulges: Vec<f64> = Vec::with_capacity(num_vertices);
        for _ in 0..num_vertices {
            vertices.push(read_point2rd(reader)?);
            if bulges_present {
                bulges.push(reader.read_bd()?);
            }
        }
        let _num_boundary_obj_handles = reader.read_bl()?;

        let mut points = if bulges_present {
            polyline_with_bulges_points(&vertices, &bulges, closed, 64)
        } else {
            vertices
        };
        if closed {
            close_path_if_needed(&mut points);
        }
        paths.push(HatchPath { closed, points });
    }

    if let Err(err) = skip_hatch_definition_payload(reader, solid_fill, any_path_uses_pixel_size) {
        if !matches!(
            err.kind,
            ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
        ) {
            return Err(err);
        }
    }

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let layer_handle = match if r2007_layer_only {
        parse_common_entity_layer_handle(reader, &header)
    } else {
        parse_common_entity_handles(reader, &header).map(|common_handles| common_handles.layer)
    } {
        Ok(layer_handle) => layer_handle,
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            0
        }
        Err(err) => return Err(err),
    };

    Ok(HatchEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        name,
        solid_fill,
        associative,
        elevation,
        extrusion,
        paths,
    })
}

fn scan_hatch_polyline_paths(
    reader: &mut BitReader<'_>,
    search_end_bit: u32,
) -> Result<Vec<HatchPath>> {
    let num_paths = bounded_count(reader.read_bl()?, "hatch paths")?;
    if !(1..=32).contains(&num_paths) {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("unsupported scanned HATCH path count: {num_paths}"),
        ));
    }

    let mut paths = Vec::with_capacity(num_paths);
    for _ in 0..num_paths {
        let path_flag = reader.read_bl()?;
        if path_flag > 0x1F || (path_flag & 0x02) == 0 {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!("unsupported scanned HATCH path flag: {path_flag}"),
            ));
        }

        let bulges_present = reader.read_b()? != 0;
        let closed = reader.read_b()? != 0;
        let num_vertices = bounded_count(reader.read_bl()?, "hatch polyline vertices")?;
        if !(3..=4096).contains(&num_vertices) {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!("unsupported scanned HATCH vertex count: {num_vertices}"),
            ));
        }

        let mut vertices: Vec<(f64, f64)> = Vec::with_capacity(num_vertices);
        let mut bulges: Vec<f64> = Vec::with_capacity(num_vertices);
        for _ in 0..num_vertices {
            let point = read_point2rd(reader)?;
            if !is_plausible_hatch_point(point) {
                return Err(DwgError::new(
                    ErrorKind::Format,
                    format!(
                        "implausible scanned HATCH point: ({}, {})",
                        point.0, point.1
                    ),
                ));
            }
            vertices.push(point);
            if bulges_present {
                let bulge = reader.read_bd()?;
                if !bulge.is_finite() || bulge.abs() > 1.0e6 {
                    return Err(DwgError::new(
                        ErrorKind::Format,
                        format!("implausible scanned HATCH bulge: {bulge}"),
                    ));
                }
                bulges.push(bulge);
            }
        }
        let _num_boundary_obj_handles = reader.read_bl()?;

        if reader.tell_bits() > u64::from(search_end_bit) {
            return Err(DwgError::new(
                ErrorKind::Format,
                "scanned HATCH path exceeds object data boundary",
            ));
        }

        let mut points = if bulges_present {
            polyline_with_bulges_points(&vertices, &bulges, closed, 64)
        } else {
            vertices
        };
        if closed {
            close_path_if_needed(&mut points);
        }
        if points.len() < 3 {
            return Err(DwgError::new(
                ErrorKind::Format,
                "scanned HATCH path is too short",
            ));
        }
        paths.push(HatchPath { closed, points });
    }

    Ok(paths)
}

fn score_hatch_candidate(entity: &HatchEntity) -> i32 {
    let mut score = 0i32;
    if !entity.name.is_empty() {
        score += 8;
    }
    if entity.paths.is_empty() {
        return i32::MIN / 4;
    }
    score += (entity.paths.len().min(32) as i32) * 4;
    for path in &entity.paths {
        if path.points.len() >= 3 {
            score += 3;
        }
        if path.closed {
            score += 2;
        }
        for (x, y) in &path.points {
            if !x.is_finite() || !y.is_finite() {
                return i32::MIN / 4;
            }
            if x.abs() > 1.0e9 || y.abs() > 1.0e9 {
                return i32::MIN / 4;
            }
        }
    }
    score
}

fn hatch_flag_bit_before(base_reader: &BitReader<'_>, start_bit: u32, offset: u32) -> Option<bool> {
    if start_bit < offset {
        return None;
    }
    let bit_pos = start_bit.saturating_sub(offset);
    if u64::from(bit_pos) >= base_reader.total_bits() {
        return None;
    }
    let mut reader = base_reader.clone();
    reader.set_bit_pos(bit_pos);
    reader.read_b().ok().map(|bit| bit != 0)
}

fn is_plausible_hatch_point(point: (f64, f64)) -> bool {
    point.0.is_finite() && point.1.is_finite() && point.0.abs() <= 1.0e8 && point.1.abs() <= 1.0e8
}

fn skip_gradient_payload(reader: &mut BitReader<'_>, use_unicode_text: bool) -> Result<()> {
    let _is_gradient = reader.read_bl()?;
    let _reserved = reader.read_bl()?;
    let _gradient_angle = reader.read_bd()?;
    let _gradient_shift = reader.read_bd()?;
    let _single_color = reader.read_bl()?;
    let _gradient_tint = reader.read_bd()?;
    let num_colors = bounded_count(reader.read_bl()?, "hatch gradient colors")?;
    for _ in 0..num_colors {
        let _unknown_double = reader.read_bd()?;
        let _unknown_short = reader.read_bs()?;
        let _rgb_color = reader.read_bl()?;
        let _ignored_color_byte = reader.read_rc()?;
    }
    let _gradient_name = if use_unicode_text {
        reader.read_tu()?
    } else {
        reader.read_tv()?
    };
    Ok(())
}

fn skip_hatch_definition_payload(
    reader: &mut BitReader<'_>,
    solid_fill: bool,
    any_path_uses_pixel_size: bool,
) -> Result<()> {
    let _style = reader.read_bs()?;
    let _pattern_type = reader.read_bs()?;

    if !solid_fill {
        let _pattern_angle = reader.read_bd()?;
        let _pattern_scale = reader.read_bd()?;
        let _double_hatch = reader.read_b()?;
        let num_def_lines =
            bounded_count(reader.read_bs()? as u32, "hatch pattern definition lines")?;
        for _ in 0..num_def_lines {
            let _line_angle = reader.read_bd()?;
            let _line_origin = (reader.read_bd()?, reader.read_bd()?);
            let _line_offset = (reader.read_bd()?, reader.read_bd()?);
            let num_dashes = bounded_count(reader.read_bs()? as u32, "hatch pattern dashes")?;
            for _ in 0..num_dashes {
                let _dash_length = reader.read_bd()?;
            }
        }
    }

    let num_seed_points = if any_path_uses_pixel_size {
        let _pixel_size = reader.read_bd()?;
        bounded_count(reader.read_bl()?, "hatch seed points")?
    } else {
        0usize
    };
    for _ in 0..num_seed_points {
        let _seed = read_point2rd(reader)?;
    }
    Ok(())
}

fn read_point2rd(reader: &mut BitReader<'_>) -> Result<(f64, f64)> {
    Ok((
        reader.read_rd(Endian::Little)?,
        reader.read_rd(Endian::Little)?,
    ))
}

fn append_segment_points(points: &mut Vec<(f64, f64)>, segment: &[(f64, f64)]) {
    if segment.is_empty() {
        return;
    }
    if points.is_empty() {
        points.extend_from_slice(segment);
        return;
    }
    let mut start = 0usize;
    if points_equal_2d(*points.last().unwrap(), segment[0]) {
        start = 1;
    }
    points.extend_from_slice(&segment[start..]);
}

fn close_path_if_needed(points: &mut Vec<(f64, f64)>) {
    if points.len() <= 1 {
        return;
    }
    let first = points[0];
    let last = *points.last().unwrap();
    if !points_equal_2d(first, last) {
        points.push(first);
    }
}

fn circular_arc_points(
    center: (f64, f64),
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    is_ccw: bool,
    arc_segments: usize,
) -> Vec<(f64, f64)> {
    if radius.abs() <= 1.0e-12 {
        return vec![];
    }
    let sweep = normalized_sweep(start_angle, end_angle, is_ccw);
    let segs = ((sweep.abs() / std::f64::consts::TAU) * (arc_segments.max(8) as f64)).ceil();
    let segments = segs.max(2.0) as usize;
    let mut out = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = (i as f64) / (segments as f64);
        let angle = start_angle + sweep * t;
        out.push((
            center.0 + radius * angle.cos(),
            center.1 + radius * angle.sin(),
        ));
    }
    out
}

fn elliptical_arc_points(
    center: (f64, f64),
    major_endpoint: (f64, f64),
    ratio: f64,
    start_angle: f64,
    end_angle: f64,
    is_ccw: bool,
    arc_segments: usize,
) -> Vec<(f64, f64)> {
    let mx = major_endpoint.0;
    let my = major_endpoint.1;
    if mx.abs() <= 1.0e-12 && my.abs() <= 1.0e-12 {
        return vec![];
    }
    let vx = -my * ratio;
    let vy = mx * ratio;
    let sweep = normalized_sweep(start_angle, end_angle, is_ccw);
    let segs = ((sweep.abs() / std::f64::consts::TAU) * (arc_segments.max(16) as f64)).ceil();
    let segments = segs.max(4.0) as usize;
    let mut out = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = (i as f64) / (segments as f64);
        let angle = start_angle + sweep * t;
        let c = angle.cos();
        let s = angle.sin();
        out.push((center.0 + mx * c + vx * s, center.1 + my * c + vy * s));
    }
    out
}

fn polyline_with_bulges_points(
    points: &[(f64, f64)],
    bulges: &[f64],
    closed: bool,
    arc_segments: usize,
) -> Vec<(f64, f64)> {
    if points.len() <= 1 {
        return points.to_vec();
    }
    let mut bulge_values = vec![0.0f64; points.len()];
    for (idx, bulge) in bulges.iter().enumerate().take(points.len()) {
        bulge_values[idx] = *bulge;
    }

    let seg_count = if closed {
        points.len()
    } else {
        points.len().saturating_sub(1)
    };
    let mut out: Vec<(f64, f64)> = Vec::new();
    for idx in 0..seg_count {
        let start = points[idx];
        let end = points[(idx + 1) % points.len()];
        let bulge = bulge_values[idx];
        let segment = bulge_segment_points(start, end, bulge, arc_segments);
        append_segment_points(&mut out, &segment);
    }
    out
}

fn bulge_segment_points(
    start: (f64, f64),
    end: (f64, f64),
    bulge: f64,
    arc_segments: usize,
) -> Vec<(f64, f64)> {
    if bulge.abs() <= 1.0e-12 {
        return vec![start, end];
    }

    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let chord = (dx * dx + dy * dy).sqrt();
    if chord <= 1.0e-12 {
        return vec![start, end];
    }

    let theta = 4.0 * bulge.atan();
    if theta.abs() <= 1.0e-12 {
        return vec![start, end];
    }

    let normal = (-dy / chord, dx / chord);
    let center_offset = chord * (1.0 - bulge * bulge) / (4.0 * bulge);
    let mid = ((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5);
    let center = (
        mid.0 + normal.0 * center_offset,
        mid.1 + normal.1 * center_offset,
    );
    let radius = ((start.0 - center.0).powi(2) + (start.1 - center.1).powi(2)).sqrt();
    if radius <= 1.0e-12 {
        return vec![start, end];
    }

    let start_angle = (start.1 - center.1).atan2(start.0 - center.0);
    let segs = ((theta.abs() / std::f64::consts::TAU) * (arc_segments.max(8) as f64)).ceil();
    let segments = segs.max(2.0) as usize;
    let mut out = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = (i as f64) / (segments as f64);
        let angle = start_angle + theta * t;
        out.push((
            center.0 + radius * angle.cos(),
            center.1 + radius * angle.sin(),
        ));
    }
    if let Some(first) = out.first_mut() {
        *first = start;
    }
    if let Some(last) = out.last_mut() {
        *last = end;
    }
    out
}

fn normalized_sweep(start_angle: f64, end_angle: f64, is_ccw: bool) -> f64 {
    let mut sweep = end_angle - start_angle;
    if is_ccw {
        if sweep < 0.0 {
            sweep += std::f64::consts::TAU;
        }
    } else if sweep > 0.0 {
        sweep -= std::f64::consts::TAU;
    }
    sweep
}

fn points_equal_2d(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() <= 1.0e-9 && (a.1 - b.1).abs() <= 1.0e-9
}

fn bounded_count(raw: u32, label: &str) -> Result<usize> {
    let count = raw as usize;
    if count > 1_000_000 {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("{} count is too large: {}", label, count),
        ));
    }
    Ok(count)
}
