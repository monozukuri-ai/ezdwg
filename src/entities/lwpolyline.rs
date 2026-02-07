use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct LwPolylineEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub flags: u16,
    pub vertices: Vec<(f64, f64)>,
}

pub fn decode_lwpolyline(reader: &mut BitReader<'_>) -> Result<LwPolylineEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_lwpolyline_with_header(reader, header, false, false)
}

pub fn decode_lwpolyline_r2007(reader: &mut BitReader<'_>) -> Result<LwPolylineEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_lwpolyline_with_header(reader, header, true, true)
}

pub fn decode_lwpolyline_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LwPolylineEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_lwpolyline_with_header(reader, header, true, true)
}

pub fn decode_lwpolyline_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LwPolylineEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_lwpolyline_with_header(reader, header, true, true)
}

fn decode_lwpolyline_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<LwPolylineEntity> {
    let flags = reader.read_bs()?;
    let num_verts = reader.read_bs()? as usize;

    let has_widths = (flags & 0x04) != 0;
    let has_bulges = (flags & 0x08) != 0;
    if has_widths || has_bulges {
        return Err(DwgError::new(
            ErrorKind::NotImplemented,
            "lwpolyline widths/bulges not supported",
        ));
    }

    let mut vertices = Vec::with_capacity(num_verts);
    if num_verts > 0 {
        let x0 = reader.read_rd(Endian::Little)?;
        let y0 = reader.read_rd(Endian::Little)?;
        vertices.push((x0, y0));

        for _ in 1..num_verts {
            let x = reader.read_dd(vertices.last().unwrap().0)?;
            let y = reader.read_dd(vertices.last().unwrap().1)?;
            vertices.push((x, y));
        }
    }
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

    Ok(LwPolylineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        flags,
        vertices,
    })
}
