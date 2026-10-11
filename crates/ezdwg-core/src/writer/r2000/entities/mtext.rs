use crate::bit::BitWriter;
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone)]
pub struct MTextEncodeInput {
    pub common: CommonEntityEncodeInput,
    pub text: String,
    pub insertion: (f64, f64, f64),
    pub text_direction: (f64, f64, f64),
    pub rect_width: f64,
    pub text_height: f64,
    pub attachment: u16,
    pub drawing_dir: u16,
}

pub fn encode_mtext_entity_payload(input: &MTextEncodeInput) -> Result<Vec<u8>> {
    let common = input.common;
    encode_entity_payload(0x2C, common, |writer| write_mtext_body(writer, input))
}

fn write_mtext_body(writer: &mut BitWriter, input: &MTextEncodeInput) -> Result<()> {
    writer.write_3bd(input.insertion.0, input.insertion.1, input.insertion.2)?;
    writer.write_3bd(
        input.common.formatting.extrusion.0,
        input.common.formatting.extrusion.1,
        input.common.formatting.extrusion.2,
    )?; // extrusion
    writer.write_3bd(
        input.text_direction.0,
        input.text_direction.1,
        input.text_direction.2,
    )?;
    writer.write_bd(input.rect_width)?;
    writer.write_bd(input.text_height)?;
    writer.write_bs(input.attachment)?;
    writer.write_bs(input.drawing_dir)?;
    writer.write_bd(0.0)?; // extents height
    writer.write_bd(0.0)?; // extents width
    writer.write_tv(&input.text)?;
    writer.write_bs(input.common.formatting.line_spacing_style)?;
    writer.write_bd(input.common.formatting.line_spacing_factor)?;
    writer.write_b(0)?; // unknown bit
    Ok(())
}
