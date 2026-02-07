use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{parse_common_entity_handles, parse_common_entity_header};

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
    let common_handles = parse_common_entity_handles(reader, &header)?;

    Ok(LwPolylineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle: common_handles.layer,
        flags,
        vertices,
    })
}
