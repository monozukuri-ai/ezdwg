use crate::bit::BitWriter;
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct ArcEncodeInput {
    pub common: CommonEntityEncodeInput,
    pub center: (f64, f64, f64),
    pub radius: f64,
    pub angle_start: f64,
    pub angle_end: f64,
}

pub fn encode_arc_entity_payload(input: ArcEncodeInput) -> Result<Vec<u8>> {
    let common = input.common;
    encode_entity_payload(0x11, common, |writer| write_arc_geometry(writer, input))
}

fn write_arc_geometry(writer: &mut BitWriter, input: ArcEncodeInput) -> Result<()> {
    writer.write_3bd(input.center.0, input.center.1, input.center.2)?;
    writer.write_bd(input.radius)?;
    writer.write_bt(input.common.formatting.thickness)?; // thickness
    writer.write_be(
        input.common.formatting.extrusion.0,
        input.common.formatting.extrusion.1,
        input.common.formatting.extrusion.2,
    )?; // extrusion
    writer.write_bd(input.angle_start)?;
    writer.write_bd(input.angle_end)?;
    Ok(())
}
