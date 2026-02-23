use crate::bit::{BitWriter, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

#[derive(Debug, Clone, Copy)]
pub struct CommonEntityEncodeInput {
    pub handle: u64,
    pub owner_handle: u64,
    pub layer_handle: u64,
    pub color_index: u8,
}

pub fn encode_entity_payload<F>(
    type_code: u16,
    common: CommonEntityEncodeInput,
    write_body: F,
) -> Result<Vec<u8>>
where
    F: FnOnce(&mut BitWriter) -> Result<()>,
{
    validate_common_input(common)?;

    let mut type_prefix = BitWriter::new();
    type_prefix.write_bs(type_code)?;

    let mut pre_handle = BitWriter::new();
    write_common_header_no_obj_size(
        &mut pre_handle,
        common.handle,
        u16::from(common.color_index),
    )?;
    write_body(&mut pre_handle)?;

    let mut handle_stream = BitWriter::new();
    handle_stream.write_h(0x02, common.owner_handle)?;
    handle_stream.write_h(0x02, common.layer_handle)?;

    let obj_size_bits = type_prefix
        .len_bits()
        .saturating_add(32)
        .saturating_add(pre_handle.len_bits());
    if obj_size_bits > u32::MAX as u64 {
        return Err(DwgError::new(
            ErrorKind::Unsupported,
            format!("entity object data bits exceed u32: {obj_size_bits}"),
        ));
    }

    let mut out = BitWriter::new();
    out.write_bits_from_bytes(&type_prefix.to_bytes(), type_prefix.len_bits())?;
    out.write_rl(Endian::Little, obj_size_bits as u32)?;
    out.write_bits_from_bytes(&pre_handle.to_bytes(), pre_handle.len_bits())?;
    out.write_bits_from_bytes(&handle_stream.to_bytes(), handle_stream.len_bits())?;
    Ok(out.into_bytes())
}

fn validate_common_input(input: CommonEntityEncodeInput) -> Result<()> {
    if input.handle == 0 {
        return Err(DwgError::new(
            ErrorKind::Format,
            "entity handle must be non-zero",
        ));
    }
    if input.owner_handle == 0 || input.layer_handle == 0 {
        return Err(DwgError::new(
            ErrorKind::Format,
            "entity owner/layer handles must be non-zero",
        ));
    }
    Ok(())
}

fn write_common_header_no_obj_size(
    writer: &mut BitWriter,
    handle: u64,
    color_index: u16,
) -> Result<()> {
    writer.write_h(0x02, handle)?;
    writer.write_bs(0)?; // ext_size
    writer.write_b(0)?; // graphic_present_flag
    writer.write_bb(0)?; // entity_mode
    writer.write_bl(0)?; // num_of_reactors
    writer.write_b(1)?; // xdic_missing_flag
    writer.write_b(0)?; // no_links == 0 => CMC follows
    writer.write_b(1)?; // CMC mode 1 => ACI byte
    writer.write_rc((color_index & 0xFF) as u8)?;
    writer.write_bd(1.0)?; // ltype scale
    writer.write_bb(0)?; // ltype_flags
    writer.write_bb(0)?; // plotstyle_flags
    writer.write_bs(0)?; // invisibility
    writer.write_rc(0)?; // line weight
    Ok(())
}
