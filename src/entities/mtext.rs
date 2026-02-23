use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct MTextEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub text: String,
    pub insertion: (f64, f64, f64),
    pub extrusion: (f64, f64, f64),
    pub x_axis_dir: (f64, f64, f64),
    pub rect_width: f64,
    pub text_height: f64,
    pub attachment: u16,
    pub drawing_dir: u16,
    pub background_flags: u32,
    pub background_scale_factor: Option<f64>,
    pub background_color_index: Option<u16>,
    pub background_true_color: Option<u32>,
    pub background_transparency: Option<u32>,
}

pub fn decode_mtext(reader: &mut BitReader<'_>) -> Result<MTextEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_mtext_with_header(reader, header, false, false, false, false)
}

pub fn decode_mtext_r2004(reader: &mut BitReader<'_>) -> Result<MTextEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_mtext_with_header(reader, header, false, true, false, false)
}

pub fn decode_mtext_r2007(reader: &mut BitReader<'_>) -> Result<MTextEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_mtext_with_header(reader, header, true, true, true, true)
}

pub fn decode_mtext_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MTextEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_mtext_with_header(reader, header, true, true, true, true)
}

pub fn decode_mtext_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MTextEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_mtext_with_header(reader, header, true, true, true, true)
}

fn decode_mtext_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    has_background_data: bool,
    use_unicode_text: bool,
    has_rect_height: bool,
) -> Result<MTextEntity> {
    let insertion = reader.read_3bd()?;
    let extrusion = reader.read_3bd()?;
    let x_axis_dir = reader.read_3bd()?;
    let rect_width = reader.read_bd()?;
    if has_rect_height {
        let _rect_height = reader.read_bd()?;
    }
    let text_height = reader.read_bd()?;
    let attachment = reader.read_bs()?;
    let drawing_dir = reader.read_bs()?;
    let _extents_height = reader.read_bd()?;
    let _extents_width = reader.read_bd()?;
    let text = if use_unicode_text {
        reader.read_tu()?
    } else {
        reader.read_tv()?
    };
    let _linespacing_style = reader.read_bs()?;
    let _linespacing_factor = reader.read_bd()?;
    let _unknown_bit = reader.read_b()?;

    let mut background_flags = 0u32;
    let mut background_scale_factor = None;
    let mut background_color_index = None;
    let mut background_true_color = None;
    let mut background_transparency = None;
    if has_background_data {
        background_flags = reader.read_bl()?;
        if (background_flags & 0x01) != 0 || (background_flags & 0x10) != 0 {
            let parse_start = reader.get_pos();
            let parsed_background = (|| -> Result<(f64, u16, Option<u32>, u32)> {
                let scale_factor = reader.read_bd()?;
                let color_index = reader.read_bs()?;
                let color_rgb = reader.read_bl()?;
                let color_byte = reader.read_rc()?;
                if (color_byte & 0x01) != 0 {
                    let _color_name = if use_unicode_text {
                        reader.read_tu()?
                    } else {
                        reader.read_tv()?
                    };
                }
                if (color_byte & 0x02) != 0 {
                    let _book_name = if use_unicode_text {
                        reader.read_tu()?
                    } else {
                        reader.read_tv()?
                    };
                }
                let transparency = reader.read_bl()?;
                Ok((
                    scale_factor,
                    color_index,
                    decode_mtext_background_true_color(color_rgb),
                    transparency,
                ))
            })();

            match parsed_background {
                Ok((scale_factor, color_index, true_color, transparency)) => {
                    background_scale_factor = Some(scale_factor);
                    background_color_index = Some(color_index);
                    background_true_color = true_color;
                    background_transparency = Some(transparency);
                }
                Err(err)
                    if matches!(
                        err.kind,
                        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                    ) =>
                {
                    reader.set_pos(parse_start.0, parse_start.1);
                }
                Err(err) => return Err(err),
            }
        }
    }

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let layer_handle = match parse_common_entity_handles(reader, &header) {
        Ok(common_handles) => common_handles.layer,
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            reader.set_pos(handles_pos.0, handles_pos.1);
            parse_common_entity_layer_handle(reader, &header).unwrap_or(0)
        }
        Err(err) => return Err(err),
    };

    Ok(MTextEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        text,
        insertion,
        extrusion,
        x_axis_dir,
        rect_width,
        text_height,
        attachment,
        drawing_dir,
        background_flags,
        background_scale_factor,
        background_color_index,
        background_true_color,
        background_transparency,
    })
}

fn decode_mtext_background_true_color(raw: u32) -> Option<u32> {
    if raw == 0 || (raw >> 24) == 0 {
        return None;
    }
    let rgb = raw & 0x00FF_FFFF;
    if rgb == 0 {
        None
    } else {
        Some(rgb)
    }
}
