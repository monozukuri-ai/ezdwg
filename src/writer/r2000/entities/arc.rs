use crate::bit::BitWriter;
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct ArcEncodeInput {
    pub handle: u64,
    pub owner_handle: u64,
    pub layer_handle: u64,
    pub color_index: u8,
    pub center: (f64, f64, f64),
    pub radius: f64,
    pub angle_start: f64,
    pub angle_end: f64,
}

pub fn encode_arc_entity_payload(input: ArcEncodeInput) -> Result<Vec<u8>> {
    let common = CommonEntityEncodeInput {
        handle: input.handle,
        owner_handle: input.owner_handle,
        layer_handle: input.layer_handle,
        color_index: input.color_index,
    };
    encode_entity_payload(0x11, common, |writer| write_arc_geometry(writer, input))
}

fn write_arc_geometry(writer: &mut BitWriter, input: ArcEncodeInput) -> Result<()> {
    writer.write_3bd(input.center.0, input.center.1, input.center.2)?;
    writer.write_bd(input.radius)?;
    writer.write_bt(0.0)?; // thickness
    writer.write_be(0.0, 0.0, 1.0)?; // extrusion
    writer.write_bd(input.angle_start)?;
    writer.write_bd(input.angle_end)?;
    Ok(())
}
