use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct VertexPFaceFaceEntity {
    pub handle: u64,
    pub index1: u16,
    pub index2: u16,
    pub index3: u16,
    pub index4: u16,
}

pub fn decode_vertex_pface_face(reader: &mut BitReader<'_>) -> Result<VertexPFaceFaceEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_vertex_pface_face_with_header(reader, header, false, false)
}

pub fn decode_vertex_pface_face_r2007(reader: &mut BitReader<'_>) -> Result<VertexPFaceFaceEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_vertex_pface_face_with_header(reader, header, true, true)
}

pub fn decode_vertex_pface_face_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<VertexPFaceFaceEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_vertex_pface_face_with_header(reader, header, true, true)
}

pub fn decode_vertex_pface_face_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<VertexPFaceFaceEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_vertex_pface_face_with_header(reader, header, true, true)
}

fn decode_vertex_pface_face_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<VertexPFaceFaceEntity> {
    let index1 = reader.read_bs()?;
    let index2 = reader.read_bs()?;
    let index3 = reader.read_bs()?;
    let index4 = reader.read_bs()?;

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

    Ok(VertexPFaceFaceEntity {
        handle: header.handle,
        index1,
        index2,
        index3,
        index4,
    })
}
