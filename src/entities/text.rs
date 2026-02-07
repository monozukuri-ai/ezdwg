use crate::bit::{BitReader, Endian};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, read_handle_reference,
};

#[derive(Debug, Clone)]
pub struct TextEntity {
    pub handle: u64,
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
}

pub fn decode_text(reader: &mut BitReader<'_>) -> Result<TextEntity> {
    let header = parse_common_entity_header(reader)?;

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

    let common_handles = parse_common_entity_handles(reader, &header)?;
    let style_handle = read_handle_reference(reader, header.handle).ok();

    Ok(TextEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
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
