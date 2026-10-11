use crate::bit::BitWriter;
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct CircleEncodeInput {
    pub common: CommonEntityEncodeInput,
    pub center: (f64, f64, f64),
    pub radius: f64,
}

pub fn encode_circle_entity_payload(input: CircleEncodeInput) -> Result<Vec<u8>> {
    let common = input.common;
    encode_entity_payload(0x12, common, |writer| write_circle_geometry(writer, input))
}

fn write_circle_geometry(writer: &mut BitWriter, input: CircleEncodeInput) -> Result<()> {
    writer.write_3bd(input.center.0, input.center.1, input.center.2)?;
    writer.write_bd(input.radius)?;
    writer.write_bt(input.common.formatting.thickness)?; // thickness
    writer.write_be(
        input.common.formatting.extrusion.0,
        input.common.formatting.extrusion.1,
        input.common.formatting.extrusion.2,
    )?; // extrusion
    Ok(())
}
