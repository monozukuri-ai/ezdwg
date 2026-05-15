use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone)]
pub struct TextEncodeInput {
    pub handle: u64,
    pub owner_handle: u64,
    pub layer_handle: u64,
    pub color_index: u8,
    pub text: String,
    pub insertion: (f64, f64, f64),
    pub height: f64,
    pub rotation: f64,
}

pub fn encode_text_entity_payload(input: &TextEncodeInput) -> Result<Vec<u8>> {
    let common = CommonEntityEncodeInput {
        handle: input.handle,
        owner_handle: input.owner_handle,
        layer_handle: input.layer_handle,
        color_index: input.color_index,
    };
    encode_entity_payload(0x01, common, |writer| write_text_body(writer, input))
}

fn write_text_body(writer: &mut BitWriter, input: &TextEncodeInput) -> Result<()> {
    let has_elevation = input.insertion.2 != 0.0;
    let has_rotation = input.rotation != 0.0;
    let mut data_flags: u8 = 0;

    if !has_elevation {
        data_flags |= 0x01;
    }
    data_flags |= 0x02; // no alignment point
    data_flags |= 0x04; // oblique default
    if !has_rotation {
        data_flags |= 0x08;
    }
    data_flags |= 0x10; // width factor default
    data_flags |= 0x20; // generation default
    data_flags |= 0x40; // horizontal alignment default
    data_flags |= 0x80; // vertical alignment default

    writer.write_rc(data_flags)?;
    if has_elevation {
        writer.write_rd(Endian::Little, input.insertion.2)?;
    }
    writer.write_rd(Endian::Little, input.insertion.0)?;
    writer.write_rd(Endian::Little, input.insertion.1)?;
    writer.write_be(0.0, 0.0, 1.0)?; // extrusion
    writer.write_bt(0.0)?; // thickness
    if has_rotation {
        writer.write_rd(Endian::Little, input.rotation)?;
    }
    writer.write_rd(Endian::Little, input.height)?;
    writer.write_tv(&input.text)?;
    Ok(())
}
