use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
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
}

pub fn decode_mtext(reader: &mut BitReader<'_>) -> Result<MTextEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_mtext_with_header(reader, header, false)
}

pub fn decode_mtext_r2007(reader: &mut BitReader<'_>) -> Result<MTextEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_mtext_with_header(reader, header, true)
}

pub fn decode_mtext_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MTextEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_mtext_with_header(reader, header, true)
}

pub fn decode_mtext_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MTextEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_mtext_with_header(reader, header, true)
}

fn decode_mtext_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
) -> Result<MTextEntity> {
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
    })
}
