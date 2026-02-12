use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, parse_common_entity_layer_handle, CommonEntityHeader,
};
use std::sync::atomic::{AtomicU32, Ordering};

static R14_CIRCLE_PREFERRED_DELTA: AtomicU32 = AtomicU32::new(64);

#[derive(Debug, Clone)]
pub struct CircleEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub center: (f64, f64, f64),
    pub radius: f64,
}

pub fn decode_circle(reader: &mut BitReader<'_>) -> Result<CircleEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_circle_with_header(reader, header, false, false)
}

pub fn decode_circle_r14(reader: &mut BitReader<'_>, object_handle: u64) -> Result<CircleEntity> {
    let saved = reader.get_pos();
    if let Ok(header) = parse_common_entity_header_r14(reader) {
        if let Ok(mut entity) = decode_circle_with_header(reader, header, true, false) {
            if entity.handle == 0 {
                entity.handle = object_handle;
            }
            return Ok(entity);
        }
    }

    reader.set_pos(saved.0, saved.1);
    if let Ok(mut entity) = decode_circle(reader) {
        if entity.handle == 0 {
            entity.handle = object_handle;
        }
        return Ok(entity);
    }

    reader.set_pos(saved.0, saved.1);
    if let Ok(header) = parse_common_entity_header(reader) {
        if let Ok(mut entity) = decode_circle_with_header(reader, header, true, false) {
            if entity.handle == 0 {
                entity.handle = object_handle;
            }
            return Ok(entity);
        }
    }
    reader.set_pos(saved.0, saved.1);
    decode_circle_r14_fallback(reader, object_handle)
}

pub fn decode_circle_r2007(reader: &mut BitReader<'_>) -> Result<CircleEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_circle_with_header(reader, header, true, true)
}

pub fn decode_circle_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<CircleEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_circle_with_header(reader, header, true, true)
}

pub fn decode_circle_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<CircleEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_circle_with_header(reader, header, true, true)
}

fn decode_circle_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<CircleEntity> {
    let center = reader.read_3bd()?;
    let radius = reader.read_bd()?;
    let _thickness = reader.read_bt()?;
    let _extrusion = reader.read_be()?;
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

    Ok(CircleEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        center,
        radius,
    })
}

fn decode_circle_r14_fallback(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<CircleEntity> {
    let base_bit = reader.tell_bits();
    let mut best: Option<(u64, u64, CircleEntity)> = None;

    let preferred_delta = R14_CIRCLE_PREFERRED_DELTA.load(Ordering::Relaxed) as i64;
    let preferred_start = preferred_delta.saturating_sub(8).max(0) as u64;
    let preferred_end = preferred_delta.saturating_add(8).max(0) as u64;

    for delta in preferred_start..=preferred_end.min(256) {
        if let Some(entity) =
            consider_circle_r14_delta(reader, base_bit, delta, object_handle, &mut best)
        {
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
        if let Some(entity) =
            consider_circle_r14_delta(reader, base_bit, delta, object_handle, &mut best)
        {
            return Ok(entity);
        }
    }

    if let Some((_, best_delta, entity)) = best {
        R14_CIRCLE_PREFERRED_DELTA.store(best_delta as u32, Ordering::Relaxed);
        return Ok(entity);
    }

    Err(DwgError::new(
        ErrorKind::Decode,
        "failed to decode R14 CIRCLE entity",
    ))
}

fn parse_circle_body_no_common(
    reader: &mut BitReader<'_>,
) -> Result<((f64, f64, f64), f64, (f64, f64, f64))> {
    let center = reader.read_3bd()?;
    let radius = reader.read_bd()?;
    let _thickness = reader.read_bt()?;
    let extrusion = reader.read_be()?;
    Ok((center, radius, extrusion))
}

fn parse_circle_body_no_common_r14(
    reader: &mut BitReader<'_>,
) -> Result<((f64, f64, f64), f64, (f64, f64, f64))> {
    let center = reader.read_3bd()?;
    let radius = reader.read_bd()?;
    let _thickness = reader.read_bd()?;
    let extrusion = reader.read_3bd()?;
    Ok((center, radius, extrusion))
}

fn consider_circle_r14_delta(
    reader: &BitReader<'_>,
    base_bit: u64,
    delta: u64,
    object_handle: u64,
    best: &mut Option<(u64, u64, CircleEntity)>,
) -> Option<CircleEntity> {
    let target = base_bit.saturating_add(delta);
    let Ok(target_u32) = u32::try_from(target) else {
        return None;
    };
    for parser_kind in 0..2u8 {
        let mut probe = reader.clone();
        probe.set_bit_pos(target_u32);

        let parsed = if parser_kind == 0 {
            parse_circle_body_no_common(&mut probe)
        } else {
            parse_circle_body_no_common_r14(&mut probe)
        };
        let Ok((center, radius, extrusion)) = parsed else {
            continue;
        };
        let Some(mut score) = score_circle_candidate(delta, center, radius, extrusion) else {
            continue;
        };
        if parser_kind == 1 {
            // Prefer the explicit R13/R14 thickness/extrusion decoding.
            score = score.saturating_sub(8);
        }

        let candidate = CircleEntity {
            handle: object_handle,
            color_index: None,
            true_color: None,
            layer_handle: 0,
            center,
            radius,
        };

        if is_high_confidence_circle_candidate(delta, center, radius, extrusion, score) {
            R14_CIRCLE_PREFERRED_DELTA.store(delta as u32, Ordering::Relaxed);
            return Some(candidate);
        }

        match best {
            Some((best_score, _, _)) if *best_score <= score => {}
            _ => *best = Some((score, delta, candidate)),
        }
    }
    None
}

fn score_circle_candidate(
    delta: u64,
    center: (f64, f64, f64),
    radius: f64,
    extrusion: (f64, f64, f64),
) -> Option<u64> {
    let values = [
        center.0,
        center.1,
        center.2,
        radius,
        extrusion.0,
        extrusion.1,
        extrusion.2,
    ];
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    if radius <= 1.0e-9 || radius > 1.0e9 {
        return None;
    }

    let max_abs = values.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
    if max_abs > 1.0e12 {
        return None;
    }

    let ex_norm =
        (extrusion.0 * extrusion.0 + extrusion.1 * extrusion.1 + extrusion.2 * extrusion.2).sqrt();
    if !ex_norm.is_finite() || ex_norm > 1.0e6 {
        return None;
    }

    let mut score = delta;
    if ex_norm < 1.0e-9 {
        score = score.saturating_add(128);
    } else {
        let norm_penalty = ((ex_norm - 1.0).abs() * 64.0).round() as u64;
        score = score.saturating_add(norm_penalty);
    }

    if center.2.abs() > 1.0e-6 {
        score = score.saturating_add(16);
    }
    if center.0.abs() < 1.0e-9 && center.1.abs() < 1.0e-9 && center.2.abs() < 1.0e-9 {
        score = score.saturating_add(96);
    }
    if (radius - 1.0).abs() < 1.0e-9 {
        score = score.saturating_add(64);
    }
    if (extrusion.0.abs() + extrusion.1.abs()) < 1.0e-6 && (extrusion.2 - 1.0).abs() < 1.0e-6 {
        score = score.saturating_sub(8);
    }

    Some(score)
}

fn is_high_confidence_circle_candidate(
    delta: u64,
    center: (f64, f64, f64),
    radius: f64,
    extrusion: (f64, f64, f64),
    score: u64,
) -> bool {
    if delta < 16 || score > 80 {
        return false;
    }
    if !radius.is_finite() || radius < 1.0e-3 {
        return false;
    }
    if center.2.abs() > 1.0e-6 {
        return false;
    }
    if center.0.abs().max(center.1.abs()).max(radius) < 2.0 {
        return false;
    }
    let extrusion_err = extrusion.0.abs() + extrusion.1.abs() + (extrusion.2 - 1.0).abs();
    extrusion_err < 1.0e-6
}
