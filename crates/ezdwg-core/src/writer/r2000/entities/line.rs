use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct LineEncodeInput {
    pub handle: u64,
    pub owner_handle: u64,
    pub layer_handle: u64,
    pub color_index: u8,
    pub start: (f64, f64, f64),
    pub end: (f64, f64, f64),
}

pub fn encode_line_entity_payload(input: LineEncodeInput) -> Result<Vec<u8>> {
    let common = CommonEntityEncodeInput {
        handle: input.handle,
        owner_handle: input.owner_handle,
        layer_handle: input.layer_handle,
        color_index: input.color_index,
    };
    encode_entity_payload(0x13, common, |writer| {
        write_line_geometry(writer, input.start, input.end)
    })
}

fn write_line_geometry(
    writer: &mut BitWriter,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
) -> Result<()> {
    let z_is_zero = start.2 == 0.0 && end.2 == 0.0;
    writer.write_b(if z_is_zero { 1 } else { 0 })?;
    writer.write_rd(Endian::Little, start.0)?;
    writer.write_dd(start.0, end.0)?;
    writer.write_rd(Endian::Little, start.1)?;
    writer.write_dd(start.1, end.1)?;
    if !z_is_zero {
        writer.write_rd(Endian::Little, start.2)?;
        writer.write_dd(start.2, end.2)?;
    }
    writer.write_bt(0.0)?; // thickness
    writer.write_be(0.0, 0.0, 1.0)?; // extrusion
    Ok(())
}
