use crate::bit::BitWriter;
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct XLineEncodeInput {
    pub handle: u64,
    pub owner_handle: u64,
    pub layer_handle: u64,
    pub color_index: u8,
    pub start: (f64, f64, f64),
    pub unit_vector: (f64, f64, f64),
}

pub fn encode_xline_entity_payload(input: XLineEncodeInput) -> Result<Vec<u8>> {
    let common = CommonEntityEncodeInput {
        handle: input.handle,
        owner_handle: input.owner_handle,
        layer_handle: input.layer_handle,
        color_index: input.color_index,
    };
    encode_entity_payload(0x29, common, |writer| write_xline_body(writer, input))
}

fn write_xline_body(writer: &mut BitWriter, input: XLineEncodeInput) -> Result<()> {
    writer.write_3bd(input.start.0, input.start.1, input.start.2)?;
    writer.write_3bd(
        input.unit_vector.0,
        input.unit_vector.1,
        input.unit_vector.2,
    )?;
    Ok(())
}
