use crate::bit::BitWriter;
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct RayEncodeInput {
    pub common: CommonEntityEncodeInput,
    pub start: (f64, f64, f64),
    pub unit_vector: (f64, f64, f64),
}

pub fn encode_ray_entity_payload(input: RayEncodeInput) -> Result<Vec<u8>> {
    let common = input.common;
    encode_entity_payload(0x28, common, |writer| write_ray_body(writer, input))
}

fn write_ray_body(writer: &mut BitWriter, input: RayEncodeInput) -> Result<()> {
    writer.write_3bd(input.start.0, input.start.1, input.start.2)?;
    writer.write_3bd(
        input.unit_vector.0,
        input.unit_vector.1,
        input.unit_vector.2,
    )?;
    Ok(())
}
