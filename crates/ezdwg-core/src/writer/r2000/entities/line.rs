use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct LineEncodeInput {
    pub common: CommonEntityEncodeInput,
    pub start: (f64, f64, f64),
    pub end: (f64, f64, f64),
}

pub fn encode_line_entity_payload(input: LineEncodeInput) -> Result<Vec<u8>> {
    let common = input.common;
    encode_entity_payload(0x13, common, |writer| {
        write_line_geometry(writer, input.start, input.end, input)
    })
}

fn write_line_geometry(
    writer: &mut BitWriter,
    start: (f64, f64, f64),
    end: (f64, f64, f64),
    input: LineEncodeInput,
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
    writer.write_bt(input.common.formatting.thickness)?; // thickness
    writer.write_be(
        input.common.formatting.extrusion.0,
        input.common.formatting.extrusion.1,
        input.common.formatting.extrusion.2,
    )?; // extrusion
    Ok(())
}
