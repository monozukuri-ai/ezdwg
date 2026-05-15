use crate::bit::{BitReader, Endian};
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct Face3dEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub p1: (f64, f64, f64),
    pub p2: (f64, f64, f64),
    pub p3: (f64, f64, f64),
    pub p4: (f64, f64, f64),
    pub invisible_edge_flags: u16,
}

pub fn decode_3dface(reader: &mut BitReader<'_>) -> Result<Face3dEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_3dface_with_header(reader, header, false, false)
}

pub fn decode_3dface_r2007(reader: &mut BitReader<'_>) -> Result<Face3dEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_3dface_with_header(reader, header, true, true)
}

pub fn decode_3dface_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<Face3dEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_3dface_with_header(reader, header, true, true)
}

pub fn decode_3dface_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<Face3dEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_3dface_with_header(reader, header, true, true)
}

fn decode_3dface_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<Face3dEntity> {
    let has_no_flag_ind = reader.read_b()?;
    let z_is_zero = reader.read_b()?;

    let x1 = reader.read_rd(Endian::Little)?;
    let y1 = reader.read_rd(Endian::Little)?;
    let z1 = if z_is_zero == 0 {
        reader.read_rd(Endian::Little)?
    } else {
        0.0
    };
    let p1 = (x1, y1, z1);

    let p2 = read_3dd(reader, p1)?;
    let p3 = read_3dd(reader, p2)?;
    let p4 = read_3dd(reader, p3)?;

    let invisible_edge_flags = if has_no_flag_ind == 0 {
        reader.read_bs()?
    } else {
        0
    };

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

    Ok(Face3dEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        p1,
        p2,
        p3,
        p4,
        invisible_edge_flags,
    })
}

fn read_3dd(reader: &mut BitReader<'_>, default: (f64, f64, f64)) -> Result<(f64, f64, f64)> {
    Ok((
        reader.read_dd(default.0)?,
        reader.read_dd(default.1)?,
        reader.read_dd(default.2)?,
    ))
}
