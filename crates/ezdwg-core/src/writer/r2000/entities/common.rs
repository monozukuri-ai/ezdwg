use crate::bit::{BitWriter, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

#[derive(Debug, Clone, Copy)]
pub struct CommonEntityEncodeInput {
    pub handle: u64,
    pub owner_handle: u64,
    /// Entity mode (ODA "entmode"): 0 stores `owner_handle` in the handle
    /// stream (block definitions and this writer's model space); 1 marks an
    /// entity of the current paper-space block (`*Paper_Space`) and 2 one of
    /// `*Model_Space`, both without a stored owner handle.
    pub entity_mode: u8,
    pub layer_handle: u64,
    pub color_index: u16,
    pub linetype_flags: u8,
    pub linetype_handle: u64,
    pub codepage: u16,
    pub modern: bool,
    pub true_color: Option<u32>,
    pub formatting: crate::writer::ir::EntityFormatting,
    pub previous_handle: u64,
    pub next_handle: u64,
    pub style_handle: u64,
}

pub fn encode_entity_payload<F>(
    type_code: u16,
    common: CommonEntityEncodeInput,
    write_body: F,
) -> Result<Vec<u8>>
where
    F: FnOnce(&mut BitWriter) -> Result<()>,
{
    encode_entity_payload_with_handles(type_code, common, write_body, &[])
}

pub fn encode_entity_payload_with_handles<F>(
    type_code: u16,
    common: CommonEntityEncodeInput,
    write_body: F,
    extra_handles: &[(u8, u64)],
) -> Result<Vec<u8>>
where
    F: FnOnce(&mut BitWriter) -> Result<()>,
{
    validate_common_input(common)?;

    if common.modern {
        let mut data = BitWriter::with_unicode();
        data.write_ot_r2010(type_code)?;
        data.write_h(0, common.handle)?;
        data.write_bs(0)?;
        data.write_b(0)?; // no proxy graphics
        data.write_bb(common.entity_mode)?;
        data.write_bl(0)?;
        data.write_b(1)?; // xdictionary missing
        data.write_b(0)?; // no DS binary data
        data.write_bs(if common.true_color.is_some() {
            0x8000
        } else {
            common.color_index
        })?;
        if let Some(rgb) = common.true_color {
            data.write_bl(0xc200_0000 | rgb)?;
        }
        data.write_bd(common.formatting.linetype_scale)?;
        data.write_bb(common.linetype_flags)?;
        data.write_bb(0)?; // material BYLAYER
        data.write_rc(0)?; // shadow flags
        data.write_bb(0)?; // plot style BYLAYER
        for _ in 0..3 {
            data.write_b(0)?;
        } // visual styles absent
        data.write_bs(u16::from(common.formatting.invisible))?;
        data.write_rc(crate::writer::validation::lineweight_index(
            common.formatting.lineweight,
        )?)?;
        write_body(&mut data)?;
        let mut handles = BitWriter::new();
        if common.entity_mode == 0 {
            handles.write_h(4, common.owner_handle)?;
        }
        handles.write_h(5, common.layer_handle)?;
        if common.linetype_flags == 3 {
            handles.write_h(5, common.linetype_handle)?;
        }
        if matches!(type_code, 1 | 2 | 3 | 0x2c) {
            handles.write_h(5, common.style_handle)?;
        }
        for &(code, h) in extra_handles {
            handles.write_h(code, h)?;
        }
        return crate::writer::r2018::streams::finish(data, &handles);
    }

    let mut type_prefix = BitWriter::new();
    type_prefix.write_bs(type_code)?;

    let mut pre_handle = BitWriter::with_codepage(common.codepage);
    write_common_header_no_obj_size(&mut pre_handle, common)?;
    write_body(&mut pre_handle)?;

    let mut handle_stream = BitWriter::new();
    if common.entity_mode == 0 {
        handle_stream.write_h(0x04, common.owner_handle)?;
    }
    // R2000 always stores the xdictionary handle (null here); the "XDic Missing
    // Flag" that lets later versions omit it does not exist yet.
    handle_stream.write_h(0x03, 0)?;
    handle_stream.write_h(0x04, common.previous_handle)?;
    handle_stream.write_h(0x04, common.next_handle)?;
    handle_stream.write_h(0x05, common.layer_handle)?;
    if common.linetype_flags == 3 {
        handle_stream.write_h(5, common.linetype_handle)?;
    }
    if matches!(type_code, 0x01 | 0x02 | 0x03 | 0x2C) {
        handle_stream.write_h(0x05, common.style_handle)?;
    }

    for &(code, h) in extra_handles {
        handle_stream.write_h(code, h)?;
    }

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
    if input.entity_mode > 2 {
        return Err(DwgError::new(
            ErrorKind::Format,
            "entity mode must be 0 (owner), 1 (paper space) or 2 (model space)",
        ));
    }
    Ok(())
}

fn write_common_header_no_obj_size(
    writer: &mut BitWriter,
    common: CommonEntityEncodeInput,
) -> Result<()> {
    writer.write_h(0x00, common.handle)?;
    writer.write_bs(0)?; // ext_size
    writer.write_b(0)?; // graphic_present_flag
    writer.write_bb(common.entity_mode)?;
    writer.write_bl(0)?; // num_of_reactors
    writer.write_b(0)?; // explicit previous/next handles, including null endpoints
    writer.write_bs(common.color_index)?;
    writer.write_bd(common.formatting.linetype_scale)?;
    writer.write_bb(common.linetype_flags)?;
    writer.write_bb(0)?; // plotstyle BYLAYER
    writer.write_bs(u16::from(common.formatting.invisible))?;
    writer.write_rc(crate::writer::validation::lineweight_index(
        common.formatting.lineweight,
    )?)?;
    Ok(())
}
