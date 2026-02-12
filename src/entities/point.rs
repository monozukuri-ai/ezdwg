use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, parse_common_entity_layer_handle, CommonEntityHeader,
};
use std::sync::atomic::{AtomicU32, Ordering};

static R14_POINT_PREFERRED_DELTA: AtomicU32 = AtomicU32::new(64);

#[derive(Debug, Clone)]
pub struct PointEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub location: (f64, f64, f64),
    pub x_axis_angle: f64,
}

pub fn decode_point(reader: &mut BitReader<'_>) -> Result<PointEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_point_with_header(reader, header, false, false)
}

pub fn decode_point_r14(reader: &mut BitReader<'_>, object_handle: u64) -> Result<PointEntity> {
    let saved = reader.get_pos();
    if let Ok(entity) = decode_point_r14_fallback(reader, object_handle) {
        return Ok(entity);
    }

    reader.set_pos(saved.0, saved.1);
    if let Ok(mut header) = parse_common_entity_header_r14(reader) {
        if header.handle == 0 {
            header.handle = object_handle;
        }
        if let Ok(entity) = decode_point_with_header(reader, header, true, false) {
            return Ok(entity);
        }
    }

    reader.set_pos(saved.0, saved.1);
    if let Ok(mut entity) = decode_point(reader) {
        if entity.handle == 0 {
            entity.handle = object_handle;
        }
        return Ok(entity);
    }

    reader.set_pos(saved.0, saved.1);
    decode_point_r14_fallback(reader, object_handle)
}

fn decode_point_r14_fallback(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<PointEntity> {
    let base_bit = reader.tell_bits();
    let mut best: Option<(u64, u64, PointEntity)> = None;
    let debug_enabled = std::env::var("EZDWG_DEBUG_R14_POINT")
        .ok()
        .is_some_and(|value| value != "0");
    let mut debug_candidates: Vec<(u64, u64, (f64, f64, f64), (f64, f64, f64), f64)> = Vec::new();

    let preferred_delta = R14_POINT_PREFERRED_DELTA.load(Ordering::Relaxed) as i64;
    let preferred_start = preferred_delta.saturating_sub(8).max(0) as u64;
    let preferred_end = preferred_delta.saturating_add(8).max(0) as u64;

    for delta in preferred_start..=preferred_end.min(256) {
        if let Some(entity) = consider_point_r14_delta(
            reader,
            base_bit,
            delta,
            object_handle,
            &mut best,
            if debug_enabled {
                Some(&mut debug_candidates)
            } else {
                None
            },
        ) {
            return Ok(entity);
        }
    }

    for delta in 0..=1024u64 {
        if delta <= 256 && delta >= preferred_start && delta <= preferred_end {
            continue;
        }
        if delta == 257 && matches!(best, Some((score, _, _)) if score <= 96) {
            break;
        }
        if let Some(entity) = consider_point_r14_delta(
            reader,
            base_bit,
            delta,
            object_handle,
            &mut best,
            if debug_enabled {
                Some(&mut debug_candidates)
            } else {
                None
            },
        ) {
            return Ok(entity);
        }
    }

    if let Some((_, best_delta, entity)) = best {
        R14_POINT_PREFERRED_DELTA.store(best_delta as u32, Ordering::Relaxed);
        if debug_enabled {
            debug_candidates.sort_by_key(|item| item.0);
            for (idx, (score, delta, location, extrusion, x_axis_angle)) in
                debug_candidates.iter().take(128).enumerate()
            {
                eprintln!(
                    "[r14-point] rank={} score={} delta={} location=({:.9},{:.9},{:.9}) extrusion=({:.9},{:.9},{:.9}) x_axis={:.9}",
                    idx,
                    score,
                    delta,
                    location.0,
                    location.1,
                    location.2,
                    extrusion.0,
                    extrusion.1,
                    extrusion.2,
                    x_axis_angle,
                );
            }
        }
        return Ok(entity);
    }

    Err(DwgError::new(
        ErrorKind::Decode,
        "failed to decode R14 POINT entity",
    ))
}

pub fn decode_point_r2007(reader: &mut BitReader<'_>) -> Result<PointEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_point_with_header(reader, header, true, true)
}

pub fn decode_point_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<PointEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_point_with_header(reader, header, true, true)
}

pub fn decode_point_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<PointEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_point_with_header(reader, header, true, true)
}

fn decode_point_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<PointEntity> {
    let location = reader.read_3bd()?;
    let _thickness = reader.read_bt()?;
    let _extrusion = reader.read_be()?;
    let x_axis_angle = reader.read_bd()?;
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

    Ok(PointEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        location,
        x_axis_angle,
    })
}

fn parse_point_body_no_common(
    reader: &mut BitReader<'_>,
) -> Result<((f64, f64, f64), (f64, f64, f64), f64)> {
    let location = reader.read_3bd()?;
    let _thickness = reader.read_bt()?;
    let extrusion = reader.read_be()?;
    let x_axis_angle = reader.read_bd()?;
    Ok((location, extrusion, x_axis_angle))
}

fn consider_point_r14_delta(
    reader: &BitReader<'_>,
    base_bit: u64,
    delta: u64,
    object_handle: u64,
    best: &mut Option<(u64, u64, PointEntity)>,
    debug_candidates: Option<&mut Vec<(u64, u64, (f64, f64, f64), (f64, f64, f64), f64)>>,
) -> Option<PointEntity> {
    let target = base_bit.saturating_add(delta);
    let Ok(target_u32) = u32::try_from(target) else {
        return None;
    };
    let mut probe = reader.clone();
    probe.set_bit_pos(target_u32);
    let Ok((location, extrusion, x_axis_angle)) = parse_point_body_no_common(&mut probe) else {
        return None;
    };
    let Some(score) = score_point_candidate(delta, location, extrusion, x_axis_angle) else {
        return None;
    };
    if let Some(list) = debug_candidates {
        list.push((score, delta, location, extrusion, x_axis_angle));
    }

    let candidate = PointEntity {
        handle: object_handle,
        color_index: None,
        true_color: None,
        layer_handle: 0,
        location,
        x_axis_angle,
    };

    if is_high_confidence_point_candidate(delta, location, extrusion, x_axis_angle, score) {
        R14_POINT_PREFERRED_DELTA.store(delta as u32, Ordering::Relaxed);
        return Some(candidate);
    }

    match best {
        Some((best_score, _, _)) if *best_score <= score => {}
        _ => *best = Some((score, delta, candidate)),
    }
    None
}

fn score_point_candidate(
    delta: u64,
    location: (f64, f64, f64),
    extrusion: (f64, f64, f64),
    x_axis_angle: f64,
) -> Option<u64> {
    let values = [
        location.0,
        location.1,
        location.2,
        extrusion.0,
        extrusion.1,
        extrusion.2,
        x_axis_angle,
    ];
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let max_abs = values.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
    if max_abs > 1.0e12 {
        return None;
    }
    if x_axis_angle.abs() > 1.0e6 {
        return None;
    }

    let ex_norm =
        (extrusion.0 * extrusion.0 + extrusion.1 * extrusion.1 + extrusion.2 * extrusion.2).sqrt();
    if !ex_norm.is_finite() || ex_norm < 1.0e-9 || ex_norm > 1.0e3 {
        return None;
    }

    let mut score = delta;
    let norm_penalty = ((ex_norm - 1.0).abs() * 64.0).round() as u64;
    score = score.saturating_add(norm_penalty);

    let geom_max = [location.0, location.1, location.2]
        .iter()
        .fold(0.0_f64, |acc, v| acc.max(v.abs()));
    let near_zero_or_one = [location.0, location.1, location.2]
        .iter()
        .filter(|value| {
            let v = **value;
            v.abs() < 1.0e-9 || (v - 1.0).abs() < 1.0e-9 || (v + 1.0).abs() < 1.0e-9
        })
        .count();
    if geom_max < 1.0e-6 {
        score = score.saturating_add(64);
    }
    if geom_max <= 1.0 + 1.0e-9 {
        score = score.saturating_add(256);
    }
    if near_zero_or_one >= 2 {
        score = score.saturating_add(192);
    }
    if geom_max >= 10.0 {
        score = score.saturating_sub(16);
    }
    if location.2.abs() > 1.0e4 {
        score = score.saturating_add(256);
    }
    if x_axis_angle.abs() < 1.0e-9 {
        score = score.saturating_sub(8);
    }
    if (x_axis_angle - 1.0).abs() < 1.0e-9 || (x_axis_angle + 1.0).abs() < 1.0e-9 {
        score = score.saturating_add(32);
    }
    if (extrusion.0.abs() + extrusion.1.abs()) < 1.0e-6 && (extrusion.2 - 1.0).abs() < 1.0e-6 {
        score = score.saturating_sub(8);
    }

    Some(score)
}

fn is_high_confidence_point_candidate(
    delta: u64,
    location: (f64, f64, f64),
    extrusion: (f64, f64, f64),
    x_axis_angle: f64,
    score: u64,
) -> bool {
    if delta < 24 || score > 80 {
        return false;
    }
    if !x_axis_angle.is_finite() || x_axis_angle.abs() > 1.0e-3 {
        return false;
    }
    let geom_max = [location.0, location.1, location.2]
        .iter()
        .fold(0.0_f64, |acc, v| acc.max(v.abs()));
    if geom_max < 1.0 {
        return false;
    }
    let extrusion_err = extrusion.0.abs() + extrusion.1.abs() + (extrusion.2 - 1.0).abs();
    extrusion_err < 1.0e-6
}
