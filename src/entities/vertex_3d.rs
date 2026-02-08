use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct Vertex3dEntity {
    pub handle: u64,
    pub flags: u8,
    pub position: (f64, f64, f64),
}

pub fn decode_vertex_3d(reader: &mut BitReader<'_>) -> Result<Vertex3dEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_vertex_3d_with_header(reader, header, false, false)
}

pub fn decode_vertex_3d_r2007(reader: &mut BitReader<'_>) -> Result<Vertex3dEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_vertex_3d_with_header(reader, header, true, true)
}

pub fn decode_vertex_3d_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<Vertex3dEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_vertex_3d_with_header(reader, header, true, true)
}

pub fn decode_vertex_3d_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<Vertex3dEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_vertex_3d_with_header(reader, header, true, true)
}

fn decode_vertex_3d_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<Vertex3dEntity> {
    let flags = reader.read_rc()?;
    let position = reader.read_3bd()?;
    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    if let Err(err) = if r2007_layer_only {
        parse_common_entity_layer_handle(reader, &header).map(|_| ())
    } else {
        parse_common_entity_handles(reader, &header).map(|_| ())
    } {
        if !(allow_handle_decode_failure
            && matches!(
                err.kind,
                ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
            ))
        {
            return Err(err);
        }
    }
    Ok(Vertex3dEntity {
        handle: header.handle,
        flags,
        position,
    })
}
