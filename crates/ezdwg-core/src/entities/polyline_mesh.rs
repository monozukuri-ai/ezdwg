use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct PolylineMeshEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub flags: u16,
    pub curve_type: u16,
    pub m_vertex_count: u16,
    pub n_vertex_count: u16,
    pub m_density: u16,
    pub n_density: u16,
    pub owned_handles: Vec<u64>,
}

pub fn decode_polyline_mesh(reader: &mut BitReader<'_>) -> Result<PolylineMeshEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_polyline_mesh_with_header(reader, header, false)
}

pub fn decode_polyline_mesh_r2007(reader: &mut BitReader<'_>) -> Result<PolylineMeshEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_polyline_mesh_with_header(reader, header, true)
}

pub fn decode_polyline_mesh_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<PolylineMeshEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_polyline_mesh_with_header(reader, header, true)
}

pub fn decode_polyline_mesh_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<PolylineMeshEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_polyline_mesh_with_header(reader, header, true)
}

fn decode_polyline_mesh_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
) -> Result<PolylineMeshEntity> {
    let flags = reader.read_bs()?;
    let curve_type = reader.read_bs()?;
    let m_vertex_count = reader.read_bs()?;
    let n_vertex_count = reader.read_bs()?;
    let m_density = reader.read_bs()?;
    let n_density = reader.read_bs()?;
    let owned_obj_count = reader.read_bl()? as usize;

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let (layer_handle, owned_handles) = match parse_common_entity_handles(reader, &header) {
        Ok(common_handles) => {
            let mut owned_handles = Vec::with_capacity(owned_obj_count);
            for _ in 0..owned_obj_count {
                owned_handles.push(read_handle_reference(reader, header.handle)?);
            }
            (common_handles.layer, owned_handles)
        }
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            reader.set_pos(handles_pos.0, handles_pos.1);
            (
                parse_common_entity_layer_handle(reader, &header).unwrap_or(0),
                Vec::new(),
            )
        }
        Err(err) => return Err(err),
    };

    Ok(PolylineMeshEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        flags,
        curve_type,
        m_vertex_count,
        n_vertex_count,
        m_density,
        n_density,
        owned_handles,
    })
}
