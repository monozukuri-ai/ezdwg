use crate::bit::{BitWriter, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone)]
pub struct LwPolylineEncodeInput {
    pub handle: u64,
    pub owner_handle: u64,
    pub layer_handle: u64,
    pub color_index: u8,
    pub flags: u16,
    pub vertices: Vec<(f64, f64)>,
    pub const_width: Option<f64>,
    pub bulges: Vec<f64>,
    pub widths: Vec<(f64, f64)>,
}

pub fn encode_lwpolyline_entity_payload(input: LwPolylineEncodeInput) -> Result<Vec<u8>> {
    if input.vertices.is_empty() {
        return Err(DwgError::new(
            ErrorKind::Format,
            "LWPOLYLINE requires at least one vertex",
        ));
    }
    let common = CommonEntityEncodeInput {
        handle: input.handle,
        owner_handle: input.owner_handle,
        layer_handle: input.layer_handle,
        color_index: input.color_index,
    };
    encode_entity_payload(0x4D, common, |writer| write_lwpolyline_body(writer, &input))
}

fn write_lwpolyline_body(writer: &mut BitWriter, input: &LwPolylineEncodeInput) -> Result<()> {
    let vertex_count = input.vertices.len();
    let bulge_count = input.bulges.len().min(vertex_count);
    let width_count = input.widths.len().min(vertex_count);

    let mut flags = input.flags & 0x0001; // keep closed bit only for v1
    if input.const_width.is_some() {
        flags |= 0x0004;
    }
    if bulge_count > 0 {
        flags |= 0x0010;
    }
    if width_count > 0 {
        flags |= 0x0020;
    }

    writer.write_bs(flags)?;

    if let Some(const_width) = input.const_width {
        writer.write_bd(const_width)?;
    }
    if (flags & 0x0001) != 0 {
        writer.write_3bd(0.0, 0.0, 1.0)?; // normal
    }

    writer.write_bl(vertex_count as u32)?;
    if bulge_count > 0 {
        writer.write_bl(bulge_count as u32)?;
    }
    if width_count > 0 {
        writer.write_bl(width_count as u32)?;
    }

    let (x0, y0) = input.vertices[0];
    writer.write_rd(Endian::Little, x0)?;
    writer.write_rd(Endian::Little, y0)?;
    let mut prev_x = x0;
    let mut prev_y = y0;
    for (x, y) in input.vertices.iter().skip(1).copied() {
        writer.write_dd(prev_x, x)?;
        writer.write_dd(prev_y, y)?;
        prev_x = x;
        prev_y = y;
    }

    for bulge in input.bulges.iter().take(bulge_count) {
        writer.write_bd(*bulge)?;
    }
    for (start_width, end_width) in input.widths.iter().take(width_count) {
        writer.write_bd(*start_width)?;
        writer.write_bd(*end_width)?;
    }
    Ok(())
}
