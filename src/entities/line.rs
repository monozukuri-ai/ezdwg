use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, parse_common_entity_layer_handle, CommonEntityHeader,
};
use std::sync::atomic::{AtomicU32, Ordering};

static R14_LINE_PREFERRED_DELTA: AtomicU32 = AtomicU32::new(64);

#[derive(Debug, Clone)]
pub struct LineEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub start: (f64, f64, f64),
    pub end: (f64, f64, f64),
}

pub fn decode_line(reader: &mut BitReader<'_>) -> Result<LineEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_line_with_header(reader, header, false, false)
}

pub fn decode_line_r14(reader: &mut BitReader<'_>, object_handle: u64) -> Result<LineEntity> {
    let saved = reader.get_pos();
    if let Ok(header) = parse_common_entity_header_r14(reader) {
        if let Ok(mut entity) = decode_line_with_header_r14(reader, header, true) {
            if entity.handle == 0 {
                entity.handle = object_handle;
            }
            return Ok(entity);
        }
    }

    reader.set_pos(saved.0, saved.1);
    if let Ok(header) = parse_common_entity_header_r14(reader) {
        if let Ok(mut entity) = decode_line_with_header(reader, header, true, false) {
            if entity.handle == 0 {
                entity.handle = object_handle;
            }
            return Ok(entity);
        }
    }

    reader.set_pos(saved.0, saved.1);
    if let Ok(mut entity) = decode_line(reader) {
        if entity.handle == 0 {
            entity.handle = object_handle;
        }
        return Ok(entity);
    }

    // R14 samples often decode geometry correctly but fail in the handle stream.
    // Retry with relaxed handle parsing before using expensive bit-scan fallback.
    reader.set_pos(saved.0, saved.1);
    if let Ok(header) = parse_common_entity_header(reader) {
        if let Ok(mut entity) = decode_line_with_header(reader, header, true, false) {
            if entity.handle == 0 {
                entity.handle = object_handle;
            }
            return Ok(entity);
        }
    }

    reader.set_pos(saved.0, saved.1);
    decode_line_r14_fallback(reader, object_handle)
}

pub fn decode_line_r2007(reader: &mut BitReader<'_>) -> Result<LineEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_line_with_header(reader, header, true, true)
}

fn decode_line_r14_fallback(reader: &mut BitReader<'_>, object_handle: u64) -> Result<LineEntity> {
    let base_bit = reader.tell_bits();
    let mut best: Option<(u64, u64, LineEntity)> = None;
    let debug_enabled = std::env::var("EZDWG_DEBUG_R14_LINE")
        .ok()
        .is_some_and(|value| value != "0");
    let mut debug_candidates: Vec<(u64, u64, (f64, f64, f64), (f64, f64, f64), (f64, f64, f64))> =
        Vec::new();

    let preferred_delta = R14_LINE_PREFERRED_DELTA.load(Ordering::Relaxed) as i64;
    let preferred_start = preferred_delta.saturating_sub(6).max(0) as u64;
    let preferred_end = preferred_delta.saturating_add(6).max(0) as u64;

    let mut consider_delta = |delta: u64| -> Option<LineEntity> {
        let target = base_bit.saturating_add(delta);
        let Ok(target_u32) = u32::try_from(target) else {
            return None;
        };
        for parser_kind in 0..3u8 {
            let mut probe = reader.clone();
            probe.set_bit_pos(target_u32);
            let parsed = if parser_kind == 0 {
                parse_line_body_no_common(&mut probe)
            } else if parser_kind == 1 {
                parse_line_body_no_common_r14_alt(&mut probe)
            } else {
                parse_line_body_no_common_r14_3bd(&mut probe)
            };
            let Ok((start, end, extrusion)) = parsed else {
                continue;
            };
            let Some(mut score) = score_line_candidate(delta, start, end, extrusion) else {
                continue;
            };
            if parser_kind == 1 {
                // Prefer canonical interpretation when both are similarly plausible.
                score = score.saturating_add(8);
            } else if parser_kind == 2 {
                // R13/R14 LINE data is explicitly defined as start/end 3BD points.
                // Prefer this interpretation in R14 fallback scanning.
                score = score.saturating_sub(16);
            }
            if debug_enabled {
                debug_candidates.push((score, delta, start, end, extrusion));
            }

            let candidate = LineEntity {
                handle: object_handle,
                color_index: None,
                true_color: None,
                layer_handle: 0,
                start,
                end,
            };

            if is_high_confidence_line_candidate(delta, start, end, extrusion, score) {
                R14_LINE_PREFERRED_DELTA.store(delta as u32, Ordering::Relaxed);
                return Some(candidate);
            }

            match &best {
                Some((best_score, _, _)) if *best_score <= score => {}
                _ => best = Some((score, delta, candidate)),
            }
        }
        None
    };

    for delta in preferred_start..=preferred_end.min(256) {
        if let Some(entity) = consider_delta(delta) {
            return Ok(entity);
        }
    }

    for delta in 0..=256u64 {
        if delta >= preferred_start && delta <= preferred_end {
            continue;
        }
        if let Some(entity) = consider_delta(delta) {
            return Ok(entity);
        }
    }

    if let Some((_, best_delta, entity)) = best {
        R14_LINE_PREFERRED_DELTA.store(best_delta as u32, Ordering::Relaxed);
        if debug_enabled {
            debug_candidates.sort_by_key(|item| item.0);
            for (idx, (score, delta, start, end, extrusion)) in
                debug_candidates.iter().take(128).enumerate()
            {
                eprintln!(
                    "[r14-line] rank={} score={} delta={} start=({:.9},{:.9},{:.9}) end=({:.9},{:.9},{:.9}) extrusion=({:.9},{:.9},{:.9})",
                    idx,
                    score,
                    delta,
                    start.0,
                    start.1,
                    start.2,
                    end.0,
                    end.1,
                    end.2,
                    extrusion.0,
                    extrusion.1,
                    extrusion.2,
                );
            }
        }
        return Ok(entity);
    }

    Err(DwgError::new(
        ErrorKind::Decode,
        "failed to decode R14 LINE entity",
    ))
}

fn parse_line_body_no_common(
    reader: &mut BitReader<'_>,
) -> Result<((f64, f64, f64), (f64, f64, f64), (f64, f64, f64))> {
    let z_is_zero = reader.read_b()?;
    let x_start = reader.read_rd(Endian::Little)?;
    let x_end = reader.read_dd(x_start)?;
    let y_start = reader.read_rd(Endian::Little)?;
    let y_end = reader.read_dd(y_start)?;

    let (z_start, z_end) = if z_is_zero == 0 {
        let z_start = reader.read_rd(Endian::Little)?;
        let z_end = reader.read_dd(z_start)?;
        (z_start, z_end)
    } else {
        (0.0, 0.0)
    };

    let _thickness = reader.read_bt()?;
    let extrusion = reader.read_be()?;

    Ok((
        (x_start, y_start, z_start),
        (x_end, y_end, z_end),
        extrusion,
    ))
}

fn parse_line_body_no_common_r14_alt(
    reader: &mut BitReader<'_>,
) -> Result<((f64, f64, f64), (f64, f64, f64), (f64, f64, f64))> {
    let x_start = reader.read_rd(Endian::Little)?;
    let x_end = reader.read_dd(x_start)?;
    let y_start = reader.read_rd(Endian::Little)?;
    let y_end = reader.read_dd(y_start)?;
    let z_start = reader.read_rd(Endian::Little)?;
    let z_end = reader.read_dd(z_start)?;
    let _thickness = reader.read_bt()?;
    let extrusion = reader.read_be()?;

    Ok((
        (x_start, y_start, z_start),
        (x_end, y_end, z_end),
        extrusion,
    ))
}

fn parse_line_body_no_common_r14_3bd(
    reader: &mut BitReader<'_>,
) -> Result<((f64, f64, f64), (f64, f64, f64), (f64, f64, f64))> {
    let start = reader.read_3bd()?;
    let end = reader.read_3bd()?;
    let _thickness = reader.read_bt()?;
    let extrusion = reader.read_be()?;
    Ok((start, end, extrusion))
}

fn score_line_candidate(
    delta: u64,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
    extrusion: (f64, f64, f64),
) -> Option<u64> {
    let values = [
        start.0,
        start.1,
        start.2,
        end.0,
        end.1,
        end.2,
        extrusion.0,
        extrusion.1,
        extrusion.2,
    ];
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }

    let max_abs = values.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
    if max_abs > 1.0e9 {
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

    let dx = start.0 - end.0;
    let dy = start.1 - end.1;
    let dz = start.2 - end.2;
    let length2 = dx * dx + dy * dy + dz * dz;
    let geom_max_abs = [start.0, start.1, start.2, end.0, end.1, end.2]
        .iter()
        .fold(0.0_f64, |acc, v| acc.max(v.abs()));
    let near_zero_or_one = [start.0, start.1, start.2, end.0, end.1, end.2]
        .iter()
        .filter(|value| {
            let v = **value;
            v.abs() < 1.0e-9 || (v - 1.0).abs() < 1.0e-9 || (v + 1.0).abs() < 1.0e-9
        })
        .count();
    if length2 < 1.0e-18 {
        score = score.saturating_add(1_500);
    }
    if geom_max_abs < 1.0e-6 {
        score = score.saturating_add(2_000);
    }
    // In R14 fallback scans, many misaligned parses collapse into {0,1} defaults.
    // Downrank these hard so meaningful geometry (e.g. 50/100 coordinates) wins.
    if geom_max_abs <= 1.0 + 1.0e-9 {
        score = score.saturating_add(256);
    }
    if near_zero_or_one >= 5 {
        score = score.saturating_add(192);
    }
    if length2 <= 1.0 + 1.0e-9 {
        score = score.saturating_add(128);
    }

    if start.2.abs() > 1.0e-6 || end.2.abs() > 1.0e-6 {
        // Most 2D drawings encode LINE with zero Z; large Z often indicates
        // a misaligned candidate in R14 fallback scanning.
        score = score.saturating_add(512);
    }

    if (extrusion.0.abs() + extrusion.1.abs()) < 1.0e-6 && (extrusion.2 - 1.0).abs() < 1.0e-6 {
        score = score.saturating_sub(8);
    }

    Some(score)
}

fn is_high_confidence_line_candidate(
    delta: u64,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
    extrusion: (f64, f64, f64),
    score: u64,
) -> bool {
    if delta < 24 || score > 96 {
        return false;
    }

    let geom_max_abs = [start.0, start.1, start.2, end.0, end.1, end.2]
        .iter()
        .fold(0.0_f64, |acc, v| acc.max(v.abs()));
    if geom_max_abs < 2.0 {
        return false;
    }

    let dx = start.0 - end.0;
    let dy = start.1 - end.1;
    let dz = start.2 - end.2;
    let length2 = dx * dx + dy * dy + dz * dz;
    if length2 < 1.0 {
        return false;
    }

    if start.2.abs() > 1.0e-6 || end.2.abs() > 1.0e-6 {
        return false;
    }

    let extrusion_err = extrusion.0.abs() + extrusion.1.abs() + (extrusion.2 - 1.0).abs();
    extrusion_err < 1.0e-6
}

pub fn decode_line_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LineEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_line_with_header(reader, header, true, true)
}

pub fn decode_line_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LineEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_line_with_header(reader, header, true, true)
}

fn decode_line_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<LineEntity> {
    let z_is_zero = reader.read_b()?;
    let x_start = reader.read_rd(Endian::Little)?;
    let x_end = reader.read_dd(x_start)?;
    let y_start = reader.read_rd(Endian::Little)?;
    let y_end = reader.read_dd(y_start)?;

    let (z_start, z_end) = if z_is_zero == 0 {
        let z_start = reader.read_rd(Endian::Little)?;
        let z_end = reader.read_dd(z_start)?;
        (z_start, z_end)
    } else {
        (0.0, 0.0)
    };

    let _thickness = reader.read_bt()?;
    let _extrusion = reader.read_be()?;
    let layer_handle = decode_layer_handle_with_common_header(
        reader,
        &header,
        allow_handle_decode_failure,
        r2007_layer_only,
    )?;

    Ok(LineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        start: (x_start, y_start, z_start),
        end: (x_end, y_end, z_end),
    })
}

fn decode_line_with_header_r14(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
) -> Result<LineEntity> {
    // R13/R14 stores explicit 3BD start/end points.
    let start = reader.read_3bd()?;
    let end = reader.read_3bd()?;
    let _thickness = reader.read_bt()?;
    let _extrusion = reader.read_be()?;
    let layer_handle = decode_layer_handle_with_common_header(
        reader,
        &header,
        allow_handle_decode_failure,
        false,
    )?;

    Ok(LineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        start,
        end,
    })
}

fn decode_layer_handle_with_common_header(
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
