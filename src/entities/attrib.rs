use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};
use crate::entities::mtext::{decode_embedded_mtext_r2010, EmbeddedMTextData};
use crate::entities::text::decode_r21_text_tail;

#[derive(Debug, Clone)]
pub struct AttribEntity {
    pub handle: u64,
    pub owner_handle: Option<u64>,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub text: String,
    pub insertion: (f64, f64, f64),
    pub alignment: Option<(f64, f64, f64)>,
    pub extrusion: (f64, f64, f64),
    pub thickness: f64,
    pub oblique_angle: f64,
    pub height: f64,
    pub rotation: f64,
    pub width_factor: f64,
    pub generation: u16,
    pub horizontal_alignment: u16,
    pub vertical_alignment: u16,
    pub style_handle: Option<u64>,
    pub tag: Option<String>,
    pub flags: u8,
    pub lock_position: bool,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct AttribTailData {
    tag: Option<String>,
    flags: u8,
    lock_position: bool,
    prompt: Option<String>,
    embedded_mtext: Option<EmbeddedMTextData>,
}

pub fn decode_attrib(reader: &mut BitReader<'_>) -> Result<AttribEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_attrib_like_with_header(reader, header, false, false, false, false, false)
}

pub fn decode_attrib_r2007(reader: &mut BitReader<'_>) -> Result<AttribEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_attrib_like_with_header(reader, header, true, false, true, false, false)
}

pub fn decode_attrib_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<AttribEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_attrib_like_with_header(reader, header, true, false, true, true, false)
}

pub fn decode_attrib_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<AttribEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_attrib_like_with_header(reader, header, true, false, true, true, true)
}

pub fn decode_attdef(reader: &mut BitReader<'_>) -> Result<AttribEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_attrib_like_with_header(reader, header, false, true, false, false, false)
}

pub fn decode_attdef_r2007(reader: &mut BitReader<'_>) -> Result<AttribEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_attrib_like_with_header(reader, header, true, true, true, false, false)
}

pub fn decode_attdef_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<AttribEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_attrib_like_with_header(reader, header, true, true, true, true, false)
}

pub fn decode_attdef_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<AttribEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_attrib_like_with_header(reader, header, true, true, true, true, true)
}

fn decode_attrib_like_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    is_attdef: bool,
    use_unicode_text: bool,
    use_r2010_plus_tail: bool,
    r2013_plus_tail: bool,
) -> Result<AttribEntity> {
    decode_attrib_like_body(
        reader,
        header,
        allow_handle_decode_failure,
        is_attdef,
        use_unicode_text,
        use_r2010_plus_tail,
        r2013_plus_tail,
        true,
    )
}

fn decode_attrib_like_body(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    is_attdef: bool,
    use_unicode_text: bool,
    use_r2010_plus_tail: bool,
    r2013_plus_tail: bool,
    log_prefix_debug: bool,
) -> Result<AttribEntity> {
    let data_flags = reader.read_rc()?;

    let elevation = if (data_flags & 0x01) == 0 {
        reader.read_rd(Endian::Little)?
    } else {
        0.0
    };

    let insertion_x = reader.read_rd(Endian::Little)?;
    let insertion_y = reader.read_rd(Endian::Little)?;

    let alignment = if (data_flags & 0x02) == 0 {
        let align_x = reader.read_dd(insertion_x)?;
        let align_y = reader.read_dd(insertion_y)?;
        Some((align_x, align_y, elevation))
    } else {
        None
    };

    let extrusion = reader.read_be()?;
    let thickness = reader.read_bt()?;

    let oblique_angle = if (data_flags & 0x04) == 0 {
        reader.read_rd(Endian::Little)?
    } else {
        0.0
    };

    let rotation = if (data_flags & 0x08) == 0 {
        reader.read_rd(Endian::Little)?
    } else {
        0.0
    };

    let height = reader.read_rd(Endian::Little)?;

    let width_factor = if (data_flags & 0x10) == 0 {
        reader.read_rd(Endian::Little)?
    } else {
        1.0
    };

    let (text, generation, horizontal_alignment, vertical_alignment) = if use_unicode_text {
        decode_r21_text_tail(reader, data_flags)?
    } else {
        let text = reader.read_tv()?;
        let generation = if (data_flags & 0x20) == 0 {
            reader.read_bs()?
        } else {
            0
        };
        let horizontal_alignment = if (data_flags & 0x40) == 0 {
            reader.read_bs()?
        } else {
            0
        };
        let vertical_alignment = if (data_flags & 0x80) == 0 {
            reader.read_bs()?
        } else {
            0
        };
        (text, generation, horizontal_alignment, vertical_alignment)
    };

    let tail_start = reader.get_pos();
    let mut tail = AttribTailData::default();
    if use_r2010_plus_tail {
        reader.set_pos(tail_start.0, tail_start.1);
        tail = match parse_attrib_tail_data_r2010_plus_with_candidates(
            reader,
            tail_start,
            is_attdef,
            header.handle,
            r2013_plus_tail,
        ) {
            Ok(tail) => tail,
            Err(err) => {
                if log_prefix_debug && attrib_prefix_debug_enabled() {
                    eprintln!(
                        "[attrib-prefix] handle={} type={} tail_err={:?} text={:?} ins=({:.6e},{:.6e},{:.6e}) align={:?} ext=({:.6e},{:.6e},{:.6e}) h={:.6e} rot={:.6e} width={:.6e} gen={} halign={} valign={}",
                        header.handle,
                        if is_attdef { "ATTDEF" } else { "ATTRIB" },
                        err,
                        text,
                        insertion_x,
                        insertion_y,
                        elevation,
                        alignment,
                        extrusion.0,
                        extrusion.1,
                        extrusion.2,
                        height,
                        rotation,
                        width_factor,
                        generation,
                        horizontal_alignment,
                        vertical_alignment,
                    );
                }
                return Err(err);
            }
        };
    } else {
        for with_version_prefix in [false, true] {
            reader.set_pos(tail_start.0, tail_start.1);
            match parse_attrib_tail_data(reader, is_attdef, with_version_prefix) {
                Ok(parsed) => {
                    tail = parsed;
                    break;
                }
                Err(err)
                    if matches!(
                        err.kind,
                        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                    ) => {}
                Err(err) => return Err(err),
            }
        }
    }

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let (owner_handle, layer_handle, style_handle) =
        match parse_common_entity_handles(reader, &header) {
            Ok(common_handles) => (
                common_handles.owner_ref,
                common_handles.layer,
                read_handle_reference(reader, header.handle).ok(),
            ),
            Err(err)
                if allow_handle_decode_failure
                    && matches!(
                        err.kind,
                        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                    ) =>
            {
                reader.set_pos(handles_pos.0, handles_pos.1);
                let layer = parse_common_entity_layer_handle(reader, &header).unwrap_or(0);
                (None, layer, None)
            }
            Err(err) => return Err(err),
        };

    let mut final_text = text;
    let mut final_insertion = (insertion_x, insertion_y, elevation);
    let mut final_alignment = alignment;
    let mut final_extrusion = extrusion;
    let mut final_height = height;
    let mut final_rotation = rotation;
    let mut final_style_handle = style_handle;
    let mut final_owner_handle = owner_handle;
    let mut final_layer_handle = layer_handle;
    if let Some(embedded) = tail.embedded_mtext.as_ref() {
        if !embedded.text.is_empty() {
            final_text = embedded.text.clone();
        }
        final_insertion = embedded.insertion;
        final_alignment = None;
        final_extrusion = embedded.extrusion;
        if embedded.text_height.is_finite() && embedded.text_height > 0.0 {
            final_height = embedded.text_height;
        }
        if let Some(embedded_rotation) = rotation_from_x_axis_dir(embedded.x_axis_dir) {
            final_rotation = embedded_rotation;
        }
        final_style_handle = final_style_handle.or(embedded.style_handle);
        if final_owner_handle.is_none() {
            final_owner_handle = embedded.owner_handle;
        }
        if final_layer_handle == 0 && embedded.layer_handle != 0 {
            final_layer_handle = embedded.layer_handle;
        }
    }

    Ok(AttribEntity {
        handle: header.handle,
        owner_handle: final_owner_handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: final_layer_handle,
        text: final_text,
        insertion: final_insertion,
        alignment: final_alignment,
        extrusion: final_extrusion,
        thickness,
        oblique_angle,
        height: final_height,
        rotation: final_rotation,
        width_factor,
        generation,
        horizontal_alignment,
        vertical_alignment,
        style_handle: final_style_handle,
        tag: tail.tag,
        flags: tail.flags,
        lock_position: tail.lock_position,
        prompt: tail.prompt,
    })
}

fn attrib_prefix_debug_enabled() -> bool {
    std::env::var("EZDWG_DEBUG_ATTRIB_PREFIX")
        .ok()
        .is_some_and(|value| value != "0")
}

fn parse_attrib_tail_data_r2010_plus_with_candidates(
    reader: &mut BitReader<'_>,
    tail_start: (usize, u8),
    is_attdef: bool,
    object_handle: u64,
    r2013_plus: bool,
) -> Result<AttribTailData> {
    let start_bits = (tail_start.0 as u64)
        .saturating_mul(8)
        .saturating_add(tail_start.1 as u64);
    let deltas = r2010_plus_attrib_tail_candidate_deltas();
    let mut best: Option<(i32, BitReader<'_>, AttribTailData)> = None;
    let mut first_err: Option<DwgError> = None;

    for delta in deltas {
        let candidate_bits_i64 = i64::try_from(start_bits).unwrap_or(i64::MAX) + i64::from(delta);
        if candidate_bits_i64 < 0 {
            continue;
        }
        let Ok(candidate_bits) = u32::try_from(candidate_bits_i64) else {
            continue;
        };
        match try_parse_r2010_plus_attrib_tail_candidate(
            reader,
            candidate_bits,
            start_bits,
            is_attdef,
            object_handle,
            r2013_plus,
        ) {
            Ok(Some((score, candidate_reader, tail))) => match &best {
                Some((best_score, ..)) if score <= *best_score => {}
                _ => best = Some((score, candidate_reader, tail)),
            },
            Ok(None) => {}
            Err(err) => {
                if first_err.is_none() {
                    first_err = Some(err);
                }
            }
        }
    }

    if let Some((_score, chosen_reader, tail)) = best {
        *reader = chosen_reader;
        return Ok(tail);
    }

    Err(first_err.unwrap_or_else(|| {
        DwgError::not_implemented("R2010+ multiline ATTRIB/ATTDEF is not yet supported")
    }))
}

fn r2010_plus_attrib_tail_candidate_deltas() -> Vec<i32> {
    let mut deltas = vec![0];
    for magnitude in (4..=128).step_by(4) {
        deltas.push(magnitude);
        deltas.push(-magnitude);
    }
    for magnitude in (160..=512).step_by(16) {
        deltas.push(magnitude);
        deltas.push(-magnitude);
    }
    for magnitude in (640..=4096).step_by(64) {
        deltas.push(magnitude);
        deltas.push(-magnitude);
    }
    deltas
}

fn parse_attrib_tail_data_r2010_plus(
    reader: &mut BitReader<'_>,
    is_attdef: bool,
    object_handle: u64,
    r2013_plus: bool,
) -> Result<AttribTailData> {
    let _version = reader.read_rc()?;
    let attribute_type = reader.read_rc()?;

    if attribute_type == 1 {
        return parse_single_line_attrib_tail_data_r2010_plus(reader, is_attdef);
    }

    if !matches!(attribute_type, 2 | 4) {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("unsupported R2010+ attribute type: {attribute_type}"),
        ));
    }

    parse_multiline_attrib_tail_data_r2010_plus(reader, is_attdef, object_handle, r2013_plus)
}

fn try_parse_r2010_plus_attrib_tail_candidate<'a>(
    reader: &BitReader<'a>,
    candidate_bits: u32,
    tail_start_bits: u64,
    is_attdef: bool,
    object_handle: u64,
    r2013_plus: bool,
) -> Result<Option<(i32, BitReader<'a>, AttribTailData)>> {
    if !prefilter_r2010_plus_attrib_tail_start(reader, candidate_bits)? {
        return Ok(None);
    }

    let mut candidate_reader = reader.clone();
    candidate_reader.set_bit_pos(candidate_bits);
    let tail =
        parse_attrib_tail_data_r2010_plus(&mut candidate_reader, is_attdef, object_handle, r2013_plus)?;

    let mut score = score_r2010_plus_attrib_tail_candidate(&tail);
    let distance = u64::from(candidate_bits).abs_diff(tail_start_bits);
    score = score.saturating_sub(i32::try_from(distance / 32).unwrap_or(i32::MAX));
    Ok(Some((score, candidate_reader, tail)))
}

fn prefilter_r2010_plus_attrib_tail_start(
    reader: &BitReader<'_>,
    candidate_bits: u32,
) -> Result<bool> {
    let mut probe = reader.clone();
    probe.set_bit_pos(candidate_bits);
    let version = probe.read_rc()?;
    if version > 32 {
        return Ok(false);
    }
    let attribute_type = probe.read_rc()?;
    Ok(matches!(attribute_type, 1 | 2 | 4))
}

fn parse_single_line_attrib_tail_data_r2010_plus(
    reader: &mut BitReader<'_>,
    is_attdef: bool,
) -> Result<AttribTailData> {
    let tag = read_r2010_plus_attrib_text(reader)?;
    let _field_length = reader.read_bs()?;
    let flags = reader.read_rc()?;
    let lock_position = reader.read_b()? != 0;
    let prompt = if is_attdef {
        Some(read_r2010_plus_attdef_prompt(reader)?)
    } else {
        None
    };

    Ok(AttribTailData {
        tag: Some(tag),
        flags,
        lock_position,
        prompt,
        embedded_mtext: None,
    })
}

fn parse_multiline_attrib_tail_data_r2010_plus(
    reader: &mut BitReader<'_>,
    is_attdef: bool,
    object_handle: u64,
    r2013_plus: bool,
) -> Result<AttribTailData> {
    let original_reader = reader.clone();
    let original_abs_bits = u64::try_from(original_reader.get_pos().0)
        .unwrap_or(u64::MAX)
        .saturating_mul(8)
        .saturating_add(u64::from(original_reader.get_pos().1));
    let mut best: Option<(i32, BitReader<'_>, AttribTailData)> = None;
    let mut first_err: Option<crate::core::error::DwgError> = None;

    for delta in [-64i32, -48, -32, -24, -16, -12, -8, -4, 0, 4, 8, 12, 16, 24, 32, 48, 64] {
        let candidate_bits_i64 = i64::try_from(original_abs_bits).unwrap_or(i64::MAX) + i64::from(delta);
        if candidate_bits_i64 < 0 {
            continue;
        }
        let Ok(candidate_bits) = u32::try_from(candidate_bits_i64) else {
            continue;
        };

        for parse_handles_inline in [true, false] {
            let mut candidate_reader = original_reader.clone();
            candidate_reader.set_bit_pos(candidate_bits);
            let embedded = match decode_embedded_mtext_r2010(
                &mut candidate_reader,
                object_handle,
                r2013_plus,
                parse_handles_inline,
            ) {
                Ok(embedded) => embedded,
                Err(err) => {
                    if first_err.is_none() {
                        first_err = Some(err);
                    }
                    continue;
                }
            };

            if !is_plausible_embedded_mtext(&embedded) {
                continue;
            }

            let mut score = score_attrib_text_candidate(&embedded.text);
            if parse_handles_inline {
                score += 6;
            }
            score = score.saturating_sub(delta.abs() / 4);
            if embedded.rect_width >= 0.0 {
                score += 1;
            }
            if (1..=9).contains(&embedded.attachment) {
                score += 2;
            }
            if embedded.drawing_dir <= 5 {
                score += 1;
            }

            let fallback_tail = AttribTailData {
                tag: None,
                flags: 0,
                lock_position: false,
                prompt: None,
                embedded_mtext: Some(embedded.clone()),
            };
            match &best {
                Some((best_score, ..)) if score - 12 <= *best_score => {}
                _ => best = Some((score - 12, candidate_reader.clone(), fallback_tail)),
            }

            let annotative_size = match candidate_reader.read_bs() {
                Ok(value) => usize::from(value),
                Err(_) => continue,
            };
            if annotative_size > 4096 {
                continue;
            }
            let mut parse_failed = false;
            for _ in 0..annotative_size {
                if candidate_reader.read_rc().is_err() {
                    parse_failed = true;
                    break;
                }
            }
            if parse_failed {
                continue;
            }

            if candidate_reader.read_h().is_err() {
                continue;
            }
            let tag_unknown = match candidate_reader.read_bs() {
                Ok(value) => value,
                Err(_) => continue,
            };
            let tag = match read_r2010_plus_attrib_text(&mut candidate_reader) {
                Ok(text) => text,
                Err(_) => continue,
            };
            let flag_unknown = match candidate_reader.read_bs() {
                Ok(value) => value,
                Err(_) => continue,
            };
            let flags = match candidate_reader.read_rc() {
                Ok(value) => value,
                Err(_) => continue,
            };
            if flags > 15 {
                continue;
            }
            let lock_position = match candidate_reader.read_b() {
                Ok(value) => value != 0,
                Err(_) => continue,
            };
            let prompt = if is_attdef {
                match read_r2010_plus_attdef_prompt(&mut candidate_reader) {
                    Ok(text) => Some(text),
                    Err(_) => continue,
                }
            } else {
                None
            };

            score += score_attrib_text_candidate(&tag);
            if let Some(prompt_text) = prompt.as_deref() {
                score += score_attrib_text_candidate(prompt_text);
            }
            if tag_unknown == 0 {
                score += 4;
            } else {
                score -= 2;
            }
            if flag_unknown == 0 {
                score += 4;
            } else {
                score -= 2;
            }
            if annotative_size == 0 {
                score += 2;
            }

            let tail = AttribTailData {
                tag: Some(tag),
                flags,
                lock_position,
                prompt,
                embedded_mtext: Some(embedded),
            };
            match &best {
                Some((best_score, ..)) if score <= *best_score => {}
                _ => best = Some((score, candidate_reader, tail)),
            }
        }
    }

    if let Some((_score, chosen_reader, tail)) = best {
        *reader = chosen_reader;
        return Ok(tail);
    }

    Err(first_err.unwrap_or_else(|| {
        DwgError::not_implemented("R2010+ multiline ATTRIB/ATTDEF is not yet supported")
    }))
}

fn is_plausible_embedded_mtext(embedded: &EmbeddedMTextData) -> bool {
    if score_attrib_text_candidate(&embedded.text) < 0 {
        return false;
    }
    if !embedded.insertion.0.is_finite()
        || !embedded.insertion.1.is_finite()
        || !embedded.insertion.2.is_finite()
    {
        return false;
    }
    if embedded.insertion.0.abs() > 1.0e6
        || embedded.insertion.1.abs() > 1.0e6
        || embedded.insertion.2.abs() > 1.0e6
    {
        return false;
    }
    embedded.text_height.is_finite() && embedded.text_height > 0.0 && embedded.text_height <= 10_000.0
}

fn parse_attrib_tail_data(
    reader: &mut BitReader<'_>,
    is_attdef: bool,
    with_version_prefix: bool,
) -> Result<AttribTailData> {
    if with_version_prefix {
        let _version = reader.read_rc()?;
    }

    let tag = reader.read_tv()?;
    let _field_length = reader.read_bs()?;
    let flags = reader.read_rc()?;
    let lock_position = reader.read_b()? != 0;
    let prompt = if is_attdef {
        Some(reader.read_tv()?)
    } else {
        None
    };
    Ok(AttribTailData {
        tag: Some(tag),
        flags,
        lock_position,
        prompt,
        embedded_mtext: None,
    })
}

fn read_r2010_plus_attrib_text(reader: &mut BitReader<'_>) -> Result<String> {
    let mut tv_candidate: Option<(i32, BitReader<'_>, String)> = None;
    let mut tu_candidate: Option<(i32, BitReader<'_>, String)> = None;

    {
        let mut candidate_reader = reader.clone();
        if let Ok(text) = candidate_reader.read_tv() {
            let score = score_attrib_text_candidate(&text);
            tv_candidate = Some((score, candidate_reader, text));
        }
    }

    {
        let mut candidate_reader = reader.clone();
        if let Ok(text) = candidate_reader.read_tu() {
            let score = score_attrib_text_candidate(&text).saturating_add(1);
            tu_candidate = Some((score, candidate_reader, text));
        }
    }

    if let Some((tv_score, chosen_reader, text)) = tv_candidate.as_ref() {
        let tu_better = tu_candidate
            .as_ref()
            .is_some_and(|(tu_score, ..)| *tu_score > *tv_score && *tv_score < 0);
        if !tu_better {
            *reader = chosen_reader.clone();
            return Ok(text.clone());
        }
    }

    if let Some((_score, chosen_reader, text)) = tu_candidate {
        *reader = chosen_reader;
        return Ok(text);
    }

    reader.read_tv()
}

fn read_r2010_plus_attdef_prompt(reader: &mut BitReader<'_>) -> Result<String> {
    let mut best: Option<(i32, BitReader<'_>, String)> = None;

    for with_version_prefix in [false, true] {
        let mut candidate_reader = reader.clone();
        if with_version_prefix && candidate_reader.read_rc().is_err() {
            continue;
        }
        let text = match read_r2010_plus_attrib_text(&mut candidate_reader) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let mut score = score_attrib_text_candidate(&text);
        if with_version_prefix {
            score += 1;
        }
        match &best {
            Some((best_score, ..)) if score <= *best_score => {}
            _ => best = Some((score, candidate_reader, text)),
        }
    }

    if let Some((_score, chosen_reader, text)) = best {
        *reader = chosen_reader;
        return Ok(text);
    }

    read_r2010_plus_attrib_text(reader)
}

fn score_attrib_text_candidate(text: &str) -> i32 {
    if text.is_empty() {
        return -4;
    }
    let mut score = 0i32;
    for ch in text.chars() {
        score += score_attrib_text_char(ch);
    }
    score
}

fn score_attrib_text_char(ch: char) -> i32 {
    if ch == '\u{FFFD}' || ('\u{E000}'..='\u{F8FF}').contains(&ch) {
        return -8;
    }
    if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
        return -6;
    }
    if ch.is_ascii_alphanumeric() {
        return 3;
    }
    if ch.is_ascii_punctuation() || ch.is_ascii_whitespace() {
        return 1;
    }
    if matches!(
        ch,
        '\u{3000}'..='\u{303F}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{FF01}'..='\u{FF60}'
            | '\u{FFE0}'..='\u{FFE6}'
    ) {
        return 3;
    }
    if ch.is_alphabetic() || ch.is_numeric() || ch.is_whitespace() {
        return 1;
    }
    -2
}

pub(crate) fn is_plausible_attrib_entity(entity: &AttribEntity) -> bool {
    score_attrib_entity_candidate(entity).is_some()
}

fn score_attrib_entity_candidate(entity: &AttribEntity) -> Option<i32> {
    let scalar_values = [
        entity.insertion.0,
        entity.insertion.1,
        entity.insertion.2,
        entity.extrusion.0,
        entity.extrusion.1,
        entity.extrusion.2,
        entity.thickness,
        entity.oblique_angle,
        entity.height,
        entity.rotation,
        entity.width_factor,
    ];
    if scalar_values
        .iter()
        .any(|value| !value.is_finite() || value.abs() > 1.0e12)
    {
        return None;
    }
    if let Some(alignment) = entity.alignment {
        let values = [alignment.0, alignment.1, alignment.2];
        if values
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 1.0e12)
        {
            return None;
        }
    }
    if entity.height < 1.0e-8 || entity.height > 1.0e6 {
        return None;
    }
    if entity.width_factor <= 1.0e-8 || entity.width_factor > 1.0e4 {
        return None;
    }
    if entity.generation > 6 || entity.generation % 2 != 0 {
        return None;
    }
    if entity.horizontal_alignment > 6 || entity.vertical_alignment > 6 {
        return None;
    }
    if entity.flags > 15 {
        return None;
    }
    if !is_plausible_attrib_text(&entity.text) {
        return None;
    }
    if entity
        .tag
        .as_deref()
        .is_some_and(|tag| !is_plausible_attrib_text(tag))
    {
        return None;
    }
    if entity
        .prompt
        .as_deref()
        .is_some_and(|prompt| !is_plausible_attrib_text(prompt))
    {
        return None;
    }

    let mut score = 0i32;
    if !entity.text.is_empty() {
        score += score_attrib_text_candidate(&entity.text);
    }
    if let Some(tag) = entity.tag.as_deref() {
        score += score_attrib_text_candidate(tag);
    }
    if let Some(prompt) = entity.prompt.as_deref() {
        score += score_attrib_text_candidate(prompt);
    }
    if entity.layer_handle != 0 {
        score += 2;
    }
    if entity.style_handle.is_some() {
        score += 1;
    }
    if entity.owner_handle.is_some() {
        score += 1;
    }
    if entity.alignment.is_some() {
        score += 1;
    }
    Some(score)
}

fn is_plausible_attrib_text(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    if text.chars().count() > 512 {
        return false;
    }
    let mut score = 0i32;
    let mut count = 0i32;
    for ch in text.chars() {
        count += 1;
        score += score_attrib_text_char(ch);
    }
    score >= -(count / 2)
}

fn score_r2010_plus_attrib_tail_candidate(tail: &AttribTailData) -> i32 {
    let mut score = 0i32;
    if let Some(embedded) = tail.embedded_mtext.as_ref() {
        score += score_attrib_text_candidate(&embedded.text);
        if embedded.text_height.is_finite() && embedded.text_height > 1.0e-6 {
            score += 4;
        }
        if embedded.layer_handle != 0 {
            score += 2;
        }
    }
    if let Some(tag) = tail.tag.as_deref() {
        score += score_attrib_text_candidate(tag);
    }
    if let Some(prompt) = tail.prompt.as_deref() {
        score += score_attrib_text_candidate(prompt);
    }
    if tail.flags <= 15 {
        score += 2;
    }
    score
}

fn rotation_from_x_axis_dir(x_axis_dir: (f64, f64, f64)) -> Option<f64> {
    if !x_axis_dir.0.is_finite() || !x_axis_dir.1.is_finite() {
        return None;
    }
    if x_axis_dir.0.abs() < 1.0e-12 && x_axis_dir.1.abs() < 1.0e-12 {
        return None;
    }
    Some(x_axis_dir.1.atan2(x_axis_dir.0))
}

#[cfg(test)]
mod tests {
    use super::{
        parse_attrib_tail_data_r2010_plus, parse_attrib_tail_data_r2010_plus_with_candidates,
    };
    use crate::bit::{BitReader, BitWriter, Endian};

    fn write_tu(writer: &mut BitWriter, text: &str) {
        writer
            .write_bs(text.encode_utf16().count() as u16)
            .expect("write tu length");
        for unit in text.encode_utf16() {
            writer
                .write_rs(Endian::Little, unit)
                .expect("write tu unit");
        }
    }

    fn write_minimal_embedded_mtext(writer: &mut BitWriter, text: &str, with_handles_inline: bool) {
        writer.write_bb(0).expect("write entity mode");
        writer.write_bl(0).expect("write reactor count");
        writer.write_b(1).expect("write xdic missing");
        writer.write_b(1).expect("write no links");
        writer.write_b(0).expect("write color unknown");
        writer.write_bd(1.0).expect("write ltype scale");
        writer.write_bb(0).expect("write ltype flags");
        writer.write_bb(0).expect("write plotstyle flags");
        writer.write_bb(0).expect("write material flags");
        writer.write_rc(0).expect("write shadow flags");
        writer.write_b(0).expect("write full visual style");
        writer.write_b(0).expect("write face visual style");
        writer.write_b(0).expect("write edge visual style");
        writer.write_bs(0).expect("write invisibility");
        writer.write_rc(0).expect("write line weight");
        writer
            .write_3bd(10.0, 20.0, 0.0)
            .expect("write insertion");
        writer
            .write_3bd(0.0, 0.0, 1.0)
            .expect("write extrusion");
        writer.write_3bd(1.0, 0.0, 0.0).expect("write x axis");
        writer.write_bd(0.0).expect("write rect width");
        writer.write_bd(0.0).expect("write rect height");
        writer.write_bd(2.5).expect("write text height");
        writer.write_bs(1).expect("write attachment");
        writer.write_bs(1).expect("write drawing dir");
        writer.write_bd(0.0).expect("write extents height");
        writer.write_bd(0.0).expect("write extents width");
        write_tu(writer, text);
        writer.write_bs(1).expect("write linespacing style");
        writer.write_bd(1.0).expect("write linespacing factor");
        writer.write_b(0).expect("write unknown bit");
        writer.write_bl(0).expect("write background flags");
        if with_handles_inline {
            writer.write_h(0x02, 0).expect("write owner");
            writer.write_h(0x02, 0x10).expect("write layer");
            writer.write_h(0x02, 0x20).expect("write style");
        }
    }

    #[test]
    fn parse_r2010_plus_single_line_attrib_tail() {
        let mut writer = BitWriter::new();
        writer.write_rc(0).expect("write version");
        writer.write_rc(1).expect("write attribute type");
        writer.write_tv("TAG1").expect("write tag");
        writer.write_bs(12).expect("write field length");
        writer.write_rc(8).expect("write flags");
        writer.write_b(1).expect("write lock position");
        let bytes = writer.into_bytes();

        let mut reader = BitReader::new(&bytes);
        let tail =
            parse_attrib_tail_data_r2010_plus(&mut reader, false, 0x100, false).expect("parse tail");

        assert_eq!(tail.tag.as_deref(), Some("TAG1"));
        assert_eq!(tail.flags, 8);
        assert!(tail.lock_position);
        assert_eq!(tail.prompt, None);
        assert!(tail.embedded_mtext.is_none());
    }

    #[test]
    fn parse_r2010_plus_single_line_attdef_tail() {
        let mut writer = BitWriter::new();
        writer.write_rc(0).expect("write version");
        writer.write_rc(1).expect("write attribute type");
        writer.write_tv("TAG2").expect("write tag");
        writer.write_bs(0).expect("write field length");
        writer.write_rc(2).expect("write flags");
        writer.write_b(0).expect("write lock position");
        writer.write_tv("PROMPT").expect("write prompt");
        let bytes = writer.into_bytes();

        let mut reader = BitReader::new(&bytes);
        let tail =
            parse_attrib_tail_data_r2010_plus(&mut reader, true, 0x100, false).expect("parse tail");

        assert_eq!(tail.tag.as_deref(), Some("TAG2"));
        assert_eq!(tail.flags, 2);
        assert!(!tail.lock_position);
        assert_eq!(tail.prompt.as_deref(), Some("PROMPT"));
        assert!(tail.embedded_mtext.is_none());
    }

    #[test]
    fn parse_r2010_plus_multiline_attrib_tail_reads_embedded_mtext() {
        let mut writer = BitWriter::new();
        writer.write_rc(0).expect("write version");
        writer.write_rc(2).expect("write attribute type");
        write_minimal_embedded_mtext(&mut writer, "VALUE", true);
        writer.write_bs(0).expect("write annotative size");
        writer.write_h(0x02, 0x30).expect("write regapp");
        writer.write_bs(0).expect("write tag unknown");
        writer.write_tv("TAG").expect("write tag");
        writer.write_bs(0).expect("write flag unknown");
        writer.write_rc(4).expect("write flags");
        writer.write_b(1).expect("write lock position");
        let bytes = writer.into_bytes();

        let mut reader = BitReader::new(&bytes);
        let tail =
            parse_attrib_tail_data_r2010_plus(&mut reader, false, 0x100, false).expect("multiline tail");

        assert_eq!(tail.tag.as_deref(), Some("TAG"));
        assert_eq!(tail.flags, 4);
        assert!(tail.lock_position);
        let embedded = tail.embedded_mtext.expect("embedded mtext");
        assert_eq!(embedded.text, "VALUE");
        assert_eq!(embedded.insertion, (10.0, 20.0, 0.0));
        assert_eq!(embedded.layer_handle, 0x10);
        assert_eq!(embedded.style_handle, Some(0x20));
    }

    #[test]
    fn parse_r2010_plus_multiline_attdef_tail_reads_prompt() {
        let mut writer = BitWriter::new();
        writer.write_rc(0).expect("write version");
        writer.write_rc(4).expect("write attribute type");
        write_minimal_embedded_mtext(&mut writer, "DEFAULT", true);
        writer.write_bs(0).expect("write annotative size");
        writer.write_h(0x02, 0x30).expect("write regapp");
        writer.write_bs(0).expect("write tag unknown");
        writer.write_tv("NAME").expect("write tag");
        writer.write_bs(0).expect("write flag unknown");
        writer.write_rc(2).expect("write flags");
        writer.write_b(0).expect("write lock position");
        writer.write_tv("PROMPT").expect("write prompt");
        let bytes = writer.into_bytes();

        let mut reader = BitReader::new(&bytes);
        let tail =
            parse_attrib_tail_data_r2010_plus(&mut reader, true, 0x100, false).expect("multiline attdef tail");

        assert_eq!(tail.tag.as_deref(), Some("NAME"));
        assert_eq!(tail.prompt.as_deref(), Some("PROMPT"));
        assert_eq!(tail.flags, 2);
        assert!(!tail.lock_position);
    }

    #[test]
    fn parse_r2010_plus_multiline_attrib_tail_recovers_shifted_embedded_mtext() {
        let mut writer = BitWriter::new();
        writer.write_rc(0).expect("write version");
        writer.write_rc(2).expect("write attribute type");
        writer.write_bits_msb(0b1010, 4).expect("write pad bits");
        write_minimal_embedded_mtext(&mut writer, "VALUE", true);
        let bytes = writer.into_bytes();

        let mut reader = BitReader::new(&bytes);
        let tail =
            parse_attrib_tail_data_r2010_plus(&mut reader, false, 0x100, false).expect("shifted multiline tail");

        let embedded = tail.embedded_mtext.expect("embedded mtext");
        assert_eq!(embedded.text, "VALUE");
        assert_eq!(embedded.insertion, (10.0, 20.0, 0.0));
    }

    #[test]
    fn parse_r2010_plus_tail_candidates_recover_far_shifted_multiline_attrib_tail() {
        let mut writer = BitWriter::new();
        writer.write_bits_msb(0, 64).expect("write far pad bits");
        writer.write_bits_msb(0, 32).expect("write far pad bits");
        writer.write_rc(0).expect("write version");
        writer.write_rc(2).expect("write attribute type");
        write_minimal_embedded_mtext(&mut writer, "VALUE", true);
        let bytes = writer.into_bytes();

        let mut reader = BitReader::new(&bytes);
        let tail = parse_attrib_tail_data_r2010_plus_with_candidates(
            &mut reader,
            (0, 0),
            false,
            0x100,
            false,
        )
        .expect("recover far shifted multiline tail");

        let embedded = tail.embedded_mtext.expect("embedded mtext");
        assert_eq!(embedded.text, "VALUE");
        assert_eq!(embedded.insertion, (10.0, 20.0, 0.0));
    }

}
