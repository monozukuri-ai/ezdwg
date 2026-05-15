use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, parse_common_entity_layer_handle, CommonEntityHeader,
};
use std::sync::atomic::{AtomicU32, Ordering};

static R14_ELLIPSE_PREFERRED_DELTA: AtomicU32 = AtomicU32::new(64);

#[derive(Debug, Clone)]
pub struct EllipseEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub center: (f64, f64, f64),
    pub major_axis: (f64, f64, f64),
    pub extrusion: (f64, f64, f64),
    pub axis_ratio: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

pub fn decode_ellipse(reader: &mut BitReader<'_>) -> Result<EllipseEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_ellipse_with_header(reader, header, false, false)
}

pub fn decode_ellipse_r14(reader: &mut BitReader<'_>, object_handle: u64) -> Result<EllipseEntity> {
    let saved = reader.get_pos();
    if let Ok(header) = parse_common_entity_header_r14(reader) {
        if let Ok(mut entity) = decode_ellipse_with_header(reader, header, true, false) {
            if entity.handle == 0 {
                entity.handle = object_handle;
            }
            if is_plausible_ellipse_entity(&entity) {
                return Ok(entity);
            }
        }
    }

    reader.set_pos(saved.0, saved.1);
    if let Ok(mut entity) = decode_ellipse(reader) {
        if entity.handle == 0 {
            entity.handle = object_handle;
        }
        if is_plausible_ellipse_entity(&entity) {
            return Ok(entity);
        }
    }

    reader.set_pos(saved.0, saved.1);
    if let Ok(header) = parse_common_entity_header(reader) {
        if let Ok(mut entity) = decode_ellipse_with_header(reader, header, true, false) {
            if entity.handle == 0 {
                entity.handle = object_handle;
            }
            if is_plausible_ellipse_entity(&entity) {
                return Ok(entity);
            }
        }
    }

    reader.set_pos(saved.0, saved.1);
    decode_ellipse_r14_fallback(reader, object_handle)
}

pub fn decode_ellipse_r2007(reader: &mut BitReader<'_>) -> Result<EllipseEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_ellipse_with_header(reader, header, true, true)
}

pub fn decode_ellipse_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<EllipseEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_ellipse_with_header(reader, header, true, true)
}

pub fn decode_ellipse_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<EllipseEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_ellipse_with_header(reader, header, true, true)
}

fn decode_ellipse_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<EllipseEntity> {
    let center = reader.read_3bd()?;
    let major_axis = reader.read_3bd()?;
    let extrusion = reader.read_3bd()?;
    let axis_ratio = reader.read_bd()?;
    let start_angle = reader.read_bd()?;
    let end_angle = reader.read_bd()?;
    let layer_handle = decode_ellipse_layer_handle(
        reader,
        &header,
        allow_handle_decode_failure,
        r2007_layer_only,
    )?;

    Ok(EllipseEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        center,
        major_axis,
        extrusion,
        axis_ratio,
        start_angle,
        end_angle,
    })
}

fn decode_ellipse_layer_handle(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<u64> {
    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let layer_handle = match if r2007_layer_only {
        parse_common_entity_layer_handle(reader, header)
    } else {
        parse_common_entity_handles(reader, header).map(|common_handles| common_handles.layer)
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
    Ok(layer_handle)
}

fn decode_ellipse_r14_fallback(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<EllipseEntity> {
    let base_bit = reader.tell_bits();
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    let target_end = header.obj_size as u64;
    if target_end <= base_bit {
        return Err(DwgError::new(
            ErrorKind::Format,
            "invalid R14 ELLIPSE object stream boundary",
        ));
    }

    let preferred_delta = R14_ELLIPSE_PREFERRED_DELTA.load(Ordering::Relaxed) as i64;
    let preferred_start = preferred_delta.saturating_sub(8).max(0) as u64;
    let preferred_end = preferred_delta.saturating_add(8).max(0) as u64;
    let mut best: Option<(u64, u64, EllipseEntity)> = None;

    for delta in preferred_start..=preferred_end.min(2048) {
        if let Some(entity) =
            consider_ellipse_r14_delta(reader, base_bit, target_end, &header, delta)
        {
            R14_ELLIPSE_PREFERRED_DELTA.store(delta as u32, Ordering::Relaxed);
            return Ok(entity);
        }
        update_best_ellipse_candidate(reader, base_bit, target_end, &header, delta, &mut best);
    }
    for delta in 0..=2048u64 {
        if delta >= preferred_start && delta <= preferred_end {
            continue;
        }
        if let Some(entity) =
            consider_ellipse_r14_delta(reader, base_bit, target_end, &header, delta)
        {
            R14_ELLIPSE_PREFERRED_DELTA.store(delta as u32, Ordering::Relaxed);
            return Ok(entity);
        }
        update_best_ellipse_candidate(reader, base_bit, target_end, &header, delta, &mut best);
    }

    if let Some((_, best_delta, entity)) = best {
        R14_ELLIPSE_PREFERRED_DELTA.store(best_delta as u32, Ordering::Relaxed);
        return Ok(entity);
    }

    Err(DwgError::new(
        ErrorKind::Decode,
        "failed to decode R14 ELLIPSE entity",
    ))
}

fn parse_ellipse_body_no_common(
    reader: &mut BitReader<'_>,
) -> Result<(
    (f64, f64, f64),
    (f64, f64, f64),
    (f64, f64, f64),
    f64,
    f64,
    f64,
)> {
    let center = reader.read_3bd()?;
    let major_axis = reader.read_3bd()?;
    let extrusion = reader.read_3bd()?;
    let axis_ratio = reader.read_bd()?;
    let start_angle = reader.read_bd()?;
    let end_angle = reader.read_bd()?;
    Ok((
        center,
        major_axis,
        extrusion,
        axis_ratio,
        start_angle,
        end_angle,
    ))
}

fn consider_ellipse_r14_delta(
    reader: &BitReader<'_>,
    base_bit: u64,
    target_end: u64,
    header: &CommonEntityHeader,
    delta: u64,
) -> Option<EllipseEntity> {
    let target = base_bit.saturating_add(delta);
    let Ok(target_u32) = u32::try_from(target) else {
        return None;
    };
    if target >= target_end {
        return None;
    }
    let mut probe = reader.clone();
    probe.set_bit_pos(target_u32);
    let Ok((center, major_axis, extrusion, axis_ratio, start_angle, end_angle)) =
        parse_ellipse_body_no_common(&mut probe)
    else {
        return None;
    };
    if probe.tell_bits() != target_end {
        return None;
    }
    let Some(score) = score_ellipse_candidate(
        delta,
        center,
        major_axis,
        extrusion,
        axis_ratio,
        start_angle,
        end_angle,
    ) else {
        return None;
    };
    if !is_high_confidence_ellipse_candidate(
        center,
        major_axis,
        extrusion,
        axis_ratio,
        start_angle,
        end_angle,
        score,
    ) {
        return None;
    }

    let mut layer_reader = reader.clone();
    let layer_handle = match decode_ellipse_layer_handle(&mut layer_reader, header, true, false) {
        Ok(layer) => layer,
        Err(_) => 0,
    };
    Some(EllipseEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        center,
        major_axis,
        extrusion,
        axis_ratio,
        start_angle,
        end_angle,
    })
}

fn update_best_ellipse_candidate(
    reader: &BitReader<'_>,
    base_bit: u64,
    target_end: u64,
    header: &CommonEntityHeader,
    delta: u64,
    best: &mut Option<(u64, u64, EllipseEntity)>,
) {
    let target = base_bit.saturating_add(delta);
    let Ok(target_u32) = u32::try_from(target) else {
        return;
    };
    if target >= target_end {
        return;
    }
    let mut probe = reader.clone();
    probe.set_bit_pos(target_u32);
    let Ok((center, major_axis, extrusion, axis_ratio, start_angle, end_angle)) =
        parse_ellipse_body_no_common(&mut probe)
    else {
        return;
    };
    if probe.tell_bits() != target_end {
        return;
    }
    let Some(score) = score_ellipse_candidate(
        delta,
        center,
        major_axis,
        extrusion,
        axis_ratio,
        start_angle,
        end_angle,
    ) else {
        return;
    };

    let mut layer_reader = reader.clone();
    let layer_handle = match decode_ellipse_layer_handle(&mut layer_reader, header, true, false) {
        Ok(layer) => layer,
        Err(_) => 0,
    };
    let candidate = EllipseEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        center,
        major_axis,
        extrusion,
        axis_ratio,
        start_angle,
        end_angle,
    };
    match best {
        Some((best_score, _, _)) if *best_score <= score => {}
        _ => *best = Some((score, delta, candidate)),
    }
}

fn score_ellipse_candidate(
    delta: u64,
    center: (f64, f64, f64),
    major_axis: (f64, f64, f64),
    extrusion: (f64, f64, f64),
    axis_ratio: f64,
    start_angle: f64,
    end_angle: f64,
) -> Option<u64> {
    let values = [
        center.0,
        center.1,
        center.2,
        major_axis.0,
        major_axis.1,
        major_axis.2,
        extrusion.0,
        extrusion.1,
        extrusion.2,
        axis_ratio,
        start_angle,
        end_angle,
    ];
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    if values.iter().any(|v| v.abs() > 1.0e9) {
        return None;
    }
    let major_len =
        (major_axis.0 * major_axis.0 + major_axis.1 * major_axis.1 + major_axis.2 * major_axis.2)
            .sqrt();
    if !major_len.is_finite() || major_len < 1.0e-9 {
        return None;
    }
    let extrusion_norm =
        (extrusion.0 * extrusion.0 + extrusion.1 * extrusion.1 + extrusion.2 * extrusion.2).sqrt();
    if !extrusion_norm.is_finite() || extrusion_norm < 1.0e-9 || extrusion_norm > 1.0e3 {
        return None;
    }
    if axis_ratio <= 0.0 {
        return None;
    }

    let mut score = delta;
    if axis_ratio > 1.0 {
        score = score.saturating_add(32);
    }
    if start_angle.abs() > std::f64::consts::PI * 64.0
        || end_angle.abs() > std::f64::consts::PI * 64.0
    {
        score = score.saturating_add(64);
    }
    let extrusion_penalty = ((extrusion_norm - 1.0).abs() * 64.0).round() as u64;
    score = score.saturating_add(extrusion_penalty);
    Some(score)
}

fn is_high_confidence_ellipse_candidate(
    _center: (f64, f64, f64),
    major_axis: (f64, f64, f64),
    extrusion: (f64, f64, f64),
    axis_ratio: f64,
    start_angle: f64,
    end_angle: f64,
    score: u64,
) -> bool {
    if score > 128 {
        return false;
    }
    let major_len =
        (major_axis.0 * major_axis.0 + major_axis.1 * major_axis.1 + major_axis.2 * major_axis.2)
            .sqrt();
    if major_len < 1.0 {
        return false;
    }
    if !(0.0 < axis_ratio && axis_ratio <= 1.0) {
        return false;
    }
    if start_angle.abs() > std::f64::consts::PI * 8.0
        || end_angle.abs() > std::f64::consts::PI * 8.0
    {
        return false;
    }
    let extrusion_err = extrusion.0.abs() + extrusion.1.abs() + (extrusion.2 - 1.0).abs();
    extrusion_err < 1.0e-6
}

fn is_plausible_ellipse_entity(entity: &EllipseEntity) -> bool {
    score_ellipse_candidate(
        0,
        entity.center,
        entity.major_axis,
        entity.extrusion,
        entity.axis_ratio,
        entity.start_angle,
        entity.end_angle,
    )
    .is_some()
}
