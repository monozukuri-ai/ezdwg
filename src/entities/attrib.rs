use crate::bit::{BitReader, Endian};
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};

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
}

pub fn decode_attrib(reader: &mut BitReader<'_>) -> Result<AttribEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_attrib_like_with_header(reader, header, false, false)
}

pub fn decode_attrib_r2007(reader: &mut BitReader<'_>) -> Result<AttribEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_attrib_like_with_header(reader, header, true, false)
}

pub fn decode_attrib_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<AttribEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_attrib_like_with_header(reader, header, true, false)
}

pub fn decode_attrib_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<AttribEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_attrib_like_with_header(reader, header, true, false)
}

pub fn decode_attdef(reader: &mut BitReader<'_>) -> Result<AttribEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_attrib_like_with_header(reader, header, false, true)
}

pub fn decode_attdef_r2007(reader: &mut BitReader<'_>) -> Result<AttribEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_attrib_like_with_header(reader, header, true, true)
}

pub fn decode_attdef_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<AttribEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_attrib_like_with_header(reader, header, true, true)
}

pub fn decode_attdef_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<AttribEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_attrib_like_with_header(reader, header, true, true)
}

fn decode_attrib_like_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    is_attdef: bool,
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

    let tail_start = reader.get_pos();
    let mut tail = AttribTailData::default();
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

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let (owner_handle, layer_handle, style_handle) = match parse_common_entity_handles(reader, &header) {
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

    Ok(AttribEntity {
        handle: header.handle,
        owner_handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
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
        tag: tail.tag,
        flags: tail.flags,
        lock_position: tail.lock_position,
        prompt: tail.prompt,
    })
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
    })
}
