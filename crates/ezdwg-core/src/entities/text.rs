use crate::bit::{BitReader, Endian};
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, parse_common_entity_layer_handle, read_handle_reference,
    CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct TextEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub owner_handle: Option<u64>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextTailField {
    Generation,
    HorizontalAlignment,
    VerticalAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextStringEncoding {
    Tv,
    Tu,
}

pub fn decode_text(reader: &mut BitReader<'_>) -> Result<TextEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_text_with_header(reader, header, false, false)
}

pub fn decode_text_r14(reader: &mut BitReader<'_>, object_handle: u64) -> Result<TextEntity> {
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    decode_text_with_header_r14(reader, header, true)
}

pub fn decode_text_r2007(reader: &mut BitReader<'_>) -> Result<TextEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_text_with_header(reader, header, true, true)
}

pub fn decode_text_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<TextEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_text_with_header(reader, header, true, true)
}

pub fn decode_text_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<TextEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_text_with_header(reader, header, true, true)
}

fn decode_text_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    use_unicode_text: bool,
) -> Result<TextEntity> {
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

    let (owner_handle, layer_handle, style_handle) =
        decode_text_handles(reader, &header, allow_handle_decode_failure)?;

    Ok(TextEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        owner_handle,
        layer_handle,
        text,
        insertion: (insertion_x, insertion_y, elevation),
        alignment,
        extrusion,
        thickness,
        oblique_angle,
        height,
        rotation,
        width_factor,
        generation,
        horizontal_alignment,
        vertical_alignment,
        style_handle,
    })
}

pub(crate) fn decode_r21_text_tail(
    reader: &mut BitReader<'_>,
    data_flags: u8,
) -> Result<(String, u16, u16, u16)> {
    let mut present_fields = Vec::with_capacity(3);
    if (data_flags & 0x20) == 0 {
        present_fields.push(TextTailField::Generation);
    }
    if (data_flags & 0x40) == 0 {
        present_fields.push(TextTailField::HorizontalAlignment);
    }
    if (data_flags & 0x80) == 0 {
        present_fields.push(TextTailField::VerticalAlignment);
    }

    let orders = tail_field_orders(&present_fields);
    let mut best: Option<(i32, BitReader<'_>, String, u16, u16, u16)> = None;

    for encoding in [TextStringEncoding::Tu, TextStringEncoding::Tv] {
        for order in &orders {
            for split_index in 0..=order.len() {
                let mut candidate_reader = reader.clone();
                let mut generation = 0u16;
                let mut horizontal_alignment = 0u16;
                let mut vertical_alignment = 0u16;

                let mut parse_ok = true;
                for field in &order[..split_index] {
                    let value = match candidate_reader.read_bs() {
                        Ok(value) => value,
                        Err(_) => {
                            parse_ok = false;
                            break;
                        }
                    };
                    assign_tail_field(
                        *field,
                        value,
                        &mut generation,
                        &mut horizontal_alignment,
                        &mut vertical_alignment,
                    );
                }
                if !parse_ok {
                    continue;
                }

                let text = match read_r21_text_tail_string(&mut candidate_reader, encoding) {
                    Ok(text) => text,
                    Err(_) => continue,
                };

                for field in &order[split_index..] {
                    let value = match candidate_reader.read_bs() {
                        Ok(value) => value,
                        Err(_) => {
                            parse_ok = false;
                            break;
                        }
                    };
                    assign_tail_field(
                        *field,
                        value,
                        &mut generation,
                        &mut horizontal_alignment,
                        &mut vertical_alignment,
                    );
                }
                if !parse_ok {
                    continue;
                }

                let score = score_r21_text_candidate(
                    &text,
                    generation,
                    horizontal_alignment,
                    vertical_alignment,
                    encoding,
                );
                match &best {
                    Some((best_score, ..)) if score <= *best_score => {}
                    _ => {
                        best = Some((
                            score,
                            candidate_reader,
                            text,
                            generation,
                            horizontal_alignment,
                            vertical_alignment,
                        ));
                    }
                }
            }
        }
    }

    if let Some((
        _score,
        chosen_reader,
        text,
        generation,
        horizontal_alignment,
        vertical_alignment,
    )) = best
    {
        *reader = chosen_reader;
        return Ok((text, generation, horizontal_alignment, vertical_alignment));
    }

    // Fall back to both known string encodings in the legacy field order.
    for encoding in [TextStringEncoding::Tu, TextStringEncoding::Tv] {
        let mut candidate_reader = reader.clone();
        let text = match read_r21_text_tail_string(&mut candidate_reader, encoding) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let generation = if (data_flags & 0x20) == 0 {
            candidate_reader.read_bs()?
        } else {
            0
        };
        let horizontal_alignment = if (data_flags & 0x40) == 0 {
            candidate_reader.read_bs()?
        } else {
            0
        };
        let vertical_alignment = if (data_flags & 0x80) == 0 {
            candidate_reader.read_bs()?
        } else {
            0
        };
        *reader = candidate_reader;
        return Ok((text, generation, horizontal_alignment, vertical_alignment));
    }

    // Last resort: keep the previous TU fallback path for error reporting.
    let text = reader.read_tu()?;
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
    Ok((text, generation, horizontal_alignment, vertical_alignment))
}

fn read_r21_text_tail_string(
    reader: &mut BitReader<'_>,
    encoding: TextStringEncoding,
) -> Result<String> {
    match encoding {
        TextStringEncoding::Tv => reader.read_tv(),
        TextStringEncoding::Tu => reader.read_tu(),
    }
}

fn tail_field_orders(present_fields: &[TextTailField]) -> Vec<Vec<TextTailField>> {
    if present_fields.is_empty() {
        return vec![Vec::new()];
    }

    fn permute(
        current: &mut Vec<TextTailField>,
        remaining: &mut Vec<TextTailField>,
        out: &mut Vec<Vec<TextTailField>>,
    ) {
        if remaining.is_empty() {
            out.push(current.clone());
            return;
        }
        for index in 0..remaining.len() {
            let field = remaining.remove(index);
            current.push(field);
            permute(current, remaining, out);
            current.pop();
            remaining.insert(index, field);
        }
    }

    let mut current = Vec::with_capacity(present_fields.len());
    let mut remaining = present_fields.to_vec();
    let mut out = Vec::new();
    permute(&mut current, &mut remaining, &mut out);
    out
}

fn assign_tail_field(
    field: TextTailField,
    value: u16,
    generation: &mut u16,
    horizontal_alignment: &mut u16,
    vertical_alignment: &mut u16,
) {
    match field {
        TextTailField::Generation => *generation = value,
        TextTailField::HorizontalAlignment => *horizontal_alignment = value,
        TextTailField::VerticalAlignment => *vertical_alignment = value,
    }
}

fn score_r21_text_candidate(
    text: &str,
    generation: u16,
    horizontal_alignment: u16,
    vertical_alignment: u16,
    encoding: TextStringEncoding,
) -> i32 {
    if text.is_empty() {
        return i32::MIN / 4;
    }

    let mut score = 0i32;
    for ch in text.chars() {
        score += score_r21_text_char(ch);
    }

    if generation <= 6 {
        score += 2;
    } else {
        score -= 10;
    }
    if horizontal_alignment <= 5 {
        score += 6;
    } else {
        score -= 12;
    }
    if vertical_alignment <= 5 {
        score += 6;
    } else {
        score -= 12;
    }
    if matches!(encoding, TextStringEncoding::Tu) {
        score += 1;
    }
    score
}

fn score_r21_text_char(ch: char) -> i32 {
    if ch == '\u{FFFD}' {
        return -24;
    }
    if ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t' {
        return -20;
    }
    if ch.is_ascii_graphic() || matches!(ch, ' ' | '\n' | '\r' | '\t') {
        return 4;
    }

    let code = ch as u32;
    if (0x3000..=0x303F).contains(&code)
        || (0x3040..=0x309F).contains(&code)
        || (0x30A0..=0x30FF).contains(&code)
        || (0x4E00..=0x9FFF).contains(&code)
        || (0xFF01..=0xFF60).contains(&code)
        || (0xFF61..=0xFF9F).contains(&code)
    {
        return 6;
    }
    if (0x2190..=0x21FF).contains(&code) || (0x25A0..=0x25FF).contains(&code) {
        return 4;
    }
    if ch.is_alphanumeric() {
        return 2;
    }
    -4
}

fn decode_text_with_header_r14(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
) -> Result<TextEntity> {
    let elevation = reader.read_bd()?;
    let insertion_x = reader.read_rd(Endian::Little)?;
    let insertion_y = reader.read_rd(Endian::Little)?;
    let align_x = reader.read_rd(Endian::Little)?;
    let align_y = reader.read_rd(Endian::Little)?;
    let extrusion = reader.read_3bd()?;
    let thickness = reader.read_bd()?;
    let oblique_angle = reader.read_bd()?;
    let rotation = reader.read_bd()?;
    let height = reader.read_bd()?;
    let width_factor = reader.read_bd()?;
    let text = reader.read_tv()?;
    let generation = reader.read_bs()?;
    let horizontal_alignment = reader.read_bs()?;
    let vertical_alignment = reader.read_bs()?;

    let (owner_handle, layer_handle, style_handle) =
        decode_text_handles(reader, &header, allow_handle_decode_failure)?;

    Ok(TextEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        owner_handle,
        layer_handle,
        text,
        insertion: (insertion_x, insertion_y, elevation),
        alignment: Some((align_x, align_y, elevation)),
        extrusion,
        thickness,
        oblique_angle,
        height,
        rotation,
        width_factor,
        generation,
        horizontal_alignment,
        vertical_alignment,
        style_handle,
    })
}

fn decode_text_handles(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
    allow_handle_decode_failure: bool,
) -> Result<(Option<u64>, u64, Option<u64>)> {
    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    match parse_common_entity_handles(reader, header) {
        Ok(common_handles) => Ok((
            common_handles.owner_ref,
            common_handles.layer,
            read_handle_reference(reader, header.handle).ok(),
        )),
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            reader.set_pos(handles_pos.0, handles_pos.1);
            let layer = parse_common_entity_layer_handle(reader, header).unwrap_or(0);
            Ok((None, layer, None))
        }
        Err(err) => Err(err),
    }
}
