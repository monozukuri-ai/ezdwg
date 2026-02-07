use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{parse_common_entity_handles, parse_common_entity_header};

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
}

pub fn decode_mtext(reader: &mut BitReader<'_>) -> Result<MTextEntity> {
    let header = parse_common_entity_header(reader)?;

    let insertion = reader.read_3bd()?;
    let extrusion = reader.read_3bd()?;
    let x_axis_dir = reader.read_3bd()?;
    let rect_width = reader.read_bd()?;
    let text_height = reader.read_bd()?;
    let attachment = reader.read_bs()?;
    let drawing_dir = reader.read_bs()?;
    let _extents_height = reader.read_bd()?;
    let _extents_width = reader.read_bd()?;
    let text = reader.read_tv()?;
    let _linespacing_style = reader.read_bs()?;
    let _linespacing_factor = reader.read_bd()?;
    let _unknown_bit = reader.read_b()?;

    let background_flags = reader.read_bl()?;
    if background_flags == 1 {
        return Err(DwgError::new(
            ErrorKind::NotImplemented,
            "mtext background fill is not supported",
        ));
    }
    let common_handles = parse_common_entity_handles(reader, &header)?;

    Ok(MTextEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
        text,
        insertion,
        extrusion,
        x_axis_dir,
        rect_width,
        text_height,
        attachment,
        drawing_dir,
    })
}
