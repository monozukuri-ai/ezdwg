use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone)]
pub struct TextEncodeInput {
    pub common: CommonEntityEncodeInput,
    pub text: String,
    pub insertion: (f64, f64, f64),
    pub height: f64,
    pub rotation: f64,
}

pub fn encode_text_entity_payload(input: &TextEncodeInput) -> Result<Vec<u8>> {
    let common = input.common;
    encode_entity_payload(0x01, common, |writer| write_text_body(writer, input))
}

pub(super) fn write_text_body(writer: &mut BitWriter, input: &TextEncodeInput) -> Result<()> {
    let f = input.common.formatting;
    let mut flags = 0u8;
    if input.insertion.2 == 0. {
        flags |= 1;
    }
    if f.alignment.is_none() {
        flags |= 2;
    }
    if f.oblique == 0. {
        flags |= 4;
    }
    if input.rotation == 0. {
        flags |= 8;
    }
    if f.width_factor == 1. {
        flags |= 16;
    }
    if f.generation == 0 {
        flags |= 32;
    }
    if f.halign == 0 {
        flags |= 64;
    }
    if f.valign == 0 {
        flags |= 128;
    }
    writer.write_rc(flags)?;
    if flags & 1 == 0 {
        writer.write_rd(Endian::Little, input.insertion.2)?;
    }
    writer.write_rd(Endian::Little, input.insertion.0)?;
    writer.write_rd(Endian::Little, input.insertion.1)?;
    if let Some(p) = f.alignment {
        writer.write_dd(input.insertion.0, p.0)?;
        writer.write_dd(input.insertion.1, p.1)?;
    }
    writer.write_be(f.extrusion.0, f.extrusion.1, f.extrusion.2)?;
    writer.write_bt(f.thickness)?;
    if flags & 4 == 0 {
        writer.write_rd(Endian::Little, f.oblique)?;
    }
    if flags & 8 == 0 {
        writer.write_rd(Endian::Little, input.rotation)?;
    }
    writer.write_rd(Endian::Little, input.height)?;
    if flags & 16 == 0 {
        writer.write_rd(Endian::Little, f.width_factor)?;
    }
    writer.write_tv(&input.text)?;
    if flags & 32 == 0 {
        writer.write_bs(f.generation)?;
    }
    if flags & 64 == 0 {
        writer.write_bs(f.halign)?;
    }
    if flags & 128 == 0 {
        writer.write_bs(f.valign)?;
    }
    Ok(())
}
