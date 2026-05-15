use crate::bit::{BitReader, Endian};
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct Vertex2dEntity {
    pub handle: u64,
    pub flags: u16,
    pub position: (f64, f64, f64),
    pub start_width: f64,
    pub end_width: f64,
    pub bulge: f64,
    pub tangent_dir: f64,
    pub owner_handle: Option<u64>,
}

pub fn decode_vertex_2d(reader: &mut BitReader<'_>) -> Result<Vertex2dEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_vertex_2d_with_header(reader, header, false, false)
}

pub fn decode_vertex_2d_r2007(reader: &mut BitReader<'_>) -> Result<Vertex2dEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_vertex_2d_with_header(reader, header, true, true)
}

pub fn decode_vertex_2d_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<Vertex2dEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_vertex_2d_with_header(reader, header, true, true)
}

pub fn decode_vertex_2d_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<Vertex2dEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_vertex_2d_with_header(reader, header, true, true)
}

fn decode_vertex_2d_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<Vertex2dEntity> {
    // Flags are NOT bit-pair-coded in the DWG spec for VERTEX(2D).
    let flags = reader.read_rs(Endian::Little)?;
    let position = reader.read_3bd()?;

    let mut start_width = reader.read_bd()?;
    let end_width = if start_width < 0.0 {
        start_width = -start_width;
        start_width
    } else {
        reader.read_bd()?
    };

    let bulge = reader.read_bd()?;
    let tangent_dir = reader.read_bd()?;

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let owner_handle = if r2007_layer_only {
        let handles_pos = reader.get_pos();
        match parse_common_entity_handles(reader, &header) {
            Ok(common_handles) => common_handles.owner_ref,
            Err(err)
                if allow_handle_decode_failure
                    && matches!(
                        err.kind,
                        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                    ) =>
            {
                reader.set_pos(handles_pos.0, handles_pos.1);
                let _ = parse_common_entity_layer_handle(reader, &header);
                None
            }
            Err(err) => return Err(err),
        }
    } else {
        match parse_common_entity_handles(reader, &header) {
            Ok(common_handles) => common_handles.owner_ref,
            Err(err)
                if allow_handle_decode_failure
                    && matches!(
                        err.kind,
                        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                    ) =>
            {
                None
            }
            Err(err) => return Err(err),
        }
    };

    Ok(Vertex2dEntity {
        handle: header.handle,
        flags,
        position,
        start_width,
        end_width,
        bulge,
        tangent_dir,
        owner_handle,
    })
}
