use crate::bit::HandleRef;
use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

const MAX_COMMON_ENTITY_REACTORS: u32 = 1 << 20;

#[derive(Debug, Clone, Copy, Default)]
pub struct CommonEntityColor {
    pub index: Option<u16>,
    pub true_color: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct CommonEntityHeader {
    pub obj_size: u32,
    pub handle: u64,
    pub color: CommonEntityColor,
    pub entity_mode: u8,
    pub num_of_reactors: u32,
    pub xdic_missing_flag: u8,
    pub has_ds_binary_data: bool,
    pub ltype_flags: u8,
    pub plotstyle_flags: u8,
    pub material_flags: u8,
    pub has_full_visual_style: bool,
    pub has_face_visual_style: bool,
    pub has_edge_visual_style: bool,
    pub has_legacy_entity_links: bool,
}

#[derive(Debug, Clone)]
pub struct CommonEntityHandles {
    pub owner_ref: Option<u64>,
    pub reactors: Vec<u64>,
    pub xdic_obj: Option<u64>,
    pub layer: u64,
    pub ltype: Option<u64>,
    pub plotstyle: Option<u64>,
    pub material: Option<u64>,
}

pub fn parse_common_entity_header(reader: &mut BitReader<'_>) -> Result<CommonEntityHeader> {
    parse_common_entity_header_impl(reader, false, false, false, None)
}

pub fn parse_common_entity_header_r14(reader: &mut BitReader<'_>) -> Result<CommonEntityHeader> {
    let start = reader.get_pos();
    match parse_common_entity_header_r14_impl(reader, false) {
        Ok(header) => Ok(header),
        Err(err)
            if matches!(
                err.kind,
                crate::core::error::ErrorKind::Format
                    | crate::core::error::ErrorKind::Decode
                    | crate::core::error::ErrorKind::Io
            ) =>
        {
            reader.set_pos(start.0, start.1);
            parse_common_entity_header_r14_impl(reader, true)
        }
        Err(err) => Err(err),
    }
}

pub fn parse_common_entity_header_r2007(reader: &mut BitReader<'_>) -> Result<CommonEntityHeader> {
    parse_common_entity_header_impl(reader, true, false, false, None)
}

pub fn parse_common_entity_header_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
) -> Result<CommonEntityHeader> {
    parse_common_entity_header_with_byte_align_fallback(
        reader,
        true,
        true,
        false,
        Some(object_data_end_bit),
    )
}

pub fn parse_common_entity_header_with_proxy_graphics_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
) -> Result<(CommonEntityHeader, Option<Vec<u8>>)> {
    parse_common_entity_header_with_proxy_graphics_with_byte_align_fallback(
        reader,
        true,
        true,
        false,
        Some(object_data_end_bit),
    )
}

pub fn parse_common_entity_header_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
) -> Result<CommonEntityHeader> {
    parse_common_entity_header_with_byte_align_fallback(
        reader,
        true,
        true,
        true,
        Some(object_data_end_bit),
    )
}

pub fn parse_common_entity_header_with_proxy_graphics_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
) -> Result<(CommonEntityHeader, Option<Vec<u8>>)> {
    parse_common_entity_header_with_proxy_graphics_with_byte_align_fallback(
        reader,
        true,
        true,
        true,
        Some(object_data_end_bit),
    )
}

pub fn parse_embedded_common_entity_header_r2010(
    reader: &mut BitReader<'_>,
    base_handle: u64,
    r2013_plus: bool,
) -> Result<CommonEntityHeader> {
    parse_common_entity_header_fields_from_entmode(
        reader,
        0,
        base_handle,
        true,
        true,
        r2013_plus,
        false,
    )
}

fn parse_common_entity_header_with_byte_align_fallback(
    reader: &mut BitReader<'_>,
    with_material_and_shadow: bool,
    r2010_plus: bool,
    r2013_plus: bool,
    object_data_end_bit: Option<u32>,
) -> Result<CommonEntityHeader> {
    let start = reader.get_pos();
    match parse_common_entity_header_impl(
        reader,
        with_material_and_shadow,
        r2010_plus,
        r2013_plus,
        object_data_end_bit,
    ) {
        Ok(header) => Ok(header),
        Err(err)
            if r2010_plus
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            let start_abs_bits = (start.0 as u64)
                .saturating_mul(8)
                .saturating_add(start.1 as u64);
            let mut candidates: Vec<(usize, u8)> = Vec::new();
            if start.1 != 0 {
                candidates.push((start.0.saturating_add(1), 0));
            }
            for delta_bits in 1..=64u64 {
                let candidate_abs_bits = start_abs_bits.saturating_add(delta_bits);
                let Ok(byte_pos) = usize::try_from(candidate_abs_bits / 8) else {
                    continue;
                };
                let bit_pos = (candidate_abs_bits % 8) as u8;
                let candidate = (byte_pos, bit_pos);
                if candidate != start && !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
            for (byte_pos, bit_pos) in candidates {
                reader.set_pos(byte_pos, bit_pos);
                if let Ok(header) = parse_common_entity_header_impl(
                    reader,
                    with_material_and_shadow,
                    r2010_plus,
                    r2013_plus,
                    object_data_end_bit,
                ) {
                    return Ok(header);
                }
            }
            reader.set_pos(start.0, start.1);
            Err(err)
        }
        Err(err) => Err(err),
    }
}

fn parse_common_entity_header_with_proxy_graphics_with_byte_align_fallback(
    reader: &mut BitReader<'_>,
    with_material_and_shadow: bool,
    r2010_plus: bool,
    r2013_plus: bool,
    object_data_end_bit: Option<u32>,
) -> Result<(CommonEntityHeader, Option<Vec<u8>>)> {
    let start = reader.get_pos();
    match parse_common_entity_header_impl_with_proxy_graphics(
        reader,
        with_material_and_shadow,
        r2010_plus,
        r2013_plus,
        object_data_end_bit,
    ) {
        Ok(decoded) => Ok(decoded),
        Err(err)
            if matches!(
                err.kind,
                crate::core::error::ErrorKind::Format
                    | crate::core::error::ErrorKind::Decode
                    | crate::core::error::ErrorKind::Io
            ) && start.1 != 0 =>
        {
            let mut candidates = vec![(start.0, 0u8)];
            if start.0 > 0 {
                candidates.push((start.0 - 1, 0));
            }
            for (byte_pos, bit_pos) in candidates {
                reader.set_pos(byte_pos, bit_pos);
                if let Ok(decoded) = parse_common_entity_header_impl_with_proxy_graphics(
                    reader,
                    with_material_and_shadow,
                    r2010_plus,
                    r2013_plus,
                    object_data_end_bit,
                ) {
                    return Ok(decoded);
                }
            }
            reader.set_pos(start.0, start.1);
            Err(err)
        }
        Err(err) => Err(err),
    }
}

fn parse_common_entity_header_impl(
    reader: &mut BitReader<'_>,
    with_material_and_shadow: bool,
    r2010_plus: bool,
    r2013_plus: bool,
    object_data_end_bit: Option<u32>,
) -> Result<CommonEntityHeader> {
    let (header, _proxy_graphics) = parse_common_entity_header_impl_with_proxy_graphics(
        reader,
        with_material_and_shadow,
        r2010_plus,
        r2013_plus,
        object_data_end_bit,
    )?;
    Ok(header)
}

fn parse_common_entity_header_impl_with_proxy_graphics(
    reader: &mut BitReader<'_>,
    with_material_and_shadow: bool,
    r2010_plus: bool,
    r2013_plus: bool,
    object_data_end_bit: Option<u32>,
) -> Result<(CommonEntityHeader, Option<Vec<u8>>)> {
    let (obj_size, handle, proxy_graphics) =
        read_common_entity_header_preamble(reader, r2010_plus, object_data_end_bit)?;

    let header = parse_common_entity_header_fields_from_entmode(
        reader,
        obj_size,
        handle,
        with_material_and_shadow,
        r2010_plus,
        r2013_plus,
        false,
    )?;

    Ok((header, proxy_graphics))
}

fn read_common_entity_header_preamble(
    reader: &mut BitReader<'_>,
    r2010_plus: bool,
    object_data_end_bit: Option<u32>,
) -> Result<(u32, u64, Option<Vec<u8>>)> {
    let obj_size = match object_data_end_bit {
        Some(bits) => bits,
        None => reader.read_rl(Endian::Little)?,
    };
    let handle = reader.read_h()?.value;

    let mut ext_size = reader.read_bs()?;
    if ext_size > 0 {
        let mut size = ext_size;
        while size > 0 {
            let _app_handle = reader.read_h()?;
            for _ in 0..size {
                let _ = reader.read_rc()?;
            }
            ext_size = reader.read_bs()?;
            size = ext_size;
        }
    }

    let graphic_present_flag = reader.read_b()?;
    let proxy_graphics = if graphic_present_flag == 1 {
        let graphic_size = if r2010_plus {
            reader.read_bll()? as usize
        } else {
            reader.read_rl(Endian::Little)? as usize
        };
        Some(reader.read_rcs(graphic_size)?.to_vec())
    } else {
        None
    };

    Ok((obj_size, handle, proxy_graphics))
}

fn parse_common_entity_header_fields_from_entmode(
    reader: &mut BitReader<'_>,
    obj_size: u32,
    handle: u64,
    with_material_and_shadow: bool,
    r2010_plus: bool,
    r2013_plus: bool,
    has_legacy_entity_links: bool,
) -> Result<CommonEntityHeader> {
    let entity_mode = reader.read_bb()?;
    let num_of_reactors = reader.read_bl()?;
    if num_of_reactors > MAX_COMMON_ENTITY_REACTORS {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!(
                "common entity reactor count too large: {num_of_reactors} (max {MAX_COMMON_ENTITY_REACTORS})"
            ),
        ));
    }
    let xdic_missing_flag = reader.read_b()?;
    let has_ds_binary_data = if r2013_plus {
        reader.read_b()? != 0
    } else {
        false
    };

    let mut color = CommonEntityColor::default();
    let no_links = reader.read_b()?;
    if no_links == 0 {
        let color_mode = reader.read_b()?;
        if color_mode == 1 {
            color.index = Some(reader.read_rc()? as u16);
        } else {
            let flags = reader.read_rs(Endian::Little)?;
            color.index = Some(flags & 0x01FF);
            if flags & 0x8000 != 0 {
                color.true_color = Some(reader.read_bl()?);
                let _name = reader.read_tv()?;
            }
            if flags & 0x2000 != 0 {
                let _transparency = reader.read_bl()?;
            }
        }
    } else {
        let _color_unknown = reader.read_b()?;
    }

    let _ltype_scale = reader.read_bd()?;
    let ltype_flags = reader.read_bb()?;
    let plotstyle_flags = reader.read_bb()?;
    let material_flags = if with_material_and_shadow {
        let flags = reader.read_bb()?;
        let _shadow_flags = reader.read_rc()?;
        flags
    } else {
        0
    };
    let (has_full_visual_style, has_face_visual_style, has_edge_visual_style) = if r2010_plus {
        (
            reader.read_b()? != 0,
            reader.read_b()? != 0,
            reader.read_b()? != 0,
        )
    } else {
        (false, false, false)
    };

    let _invisibility = reader.read_bs()?;
    let _line_weight = reader.read_rc()?;

    Ok(CommonEntityHeader {
        obj_size,
        handle,
        color,
        entity_mode,
        num_of_reactors,
        xdic_missing_flag,
        has_ds_binary_data,
        ltype_flags,
        plotstyle_flags,
        material_flags,
        has_full_visual_style,
        has_face_visual_style,
        has_edge_visual_style,
        has_legacy_entity_links,
    })
}

fn parse_common_entity_header_r14_impl(
    reader: &mut BitReader<'_>,
    with_ds_binary_flag: bool,
) -> Result<CommonEntityHeader> {
    let handle = reader.read_h()?.value;
    skip_eed(reader)?;

    let graphic_present_flag = reader.read_b()?;
    if graphic_present_flag == 1 {
        let graphic_size = reader.read_rl(Endian::Little)? as usize;
        let _ = reader.read_rcs(graphic_size)?;
    }

    let obj_size = reader.read_rl(Endian::Little)?;
    let entity_mode = reader.read_bb()?;
    let num_of_reactors = reader.read_bl()?;
    if num_of_reactors > MAX_COMMON_ENTITY_REACTORS {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!(
                "common entity reactor count too large: {num_of_reactors} (max {MAX_COMMON_ENTITY_REACTORS})"
            ),
        ));
    }
    let xdic_missing_flag = reader.read_b()?;
    let has_ds_binary_data = if with_ds_binary_flag {
        reader.read_b()? != 0
    } else {
        false
    };

    let is_bylayer_ltype = reader.read_b()? != 0;
    let no_links = reader.read_b()?;
    let color = read_common_entity_color_cmc(reader)?;
    let _ltype_scale = reader.read_bd()?;
    let _invisibility = reader.read_bs()?;
    let _line_weight = reader.read_rc()?;

    let ltype_flags = if is_bylayer_ltype { 0 } else { 3 };

    Ok(CommonEntityHeader {
        obj_size,
        handle,
        color,
        entity_mode,
        num_of_reactors,
        xdic_missing_flag,
        has_ds_binary_data,
        ltype_flags,
        plotstyle_flags: 0,
        material_flags: 0,
        has_full_visual_style: false,
        has_face_visual_style: false,
        has_edge_visual_style: false,
        has_legacy_entity_links: no_links == 0,
    })
}

fn read_common_entity_color_cmc(reader: &mut BitReader<'_>) -> Result<CommonEntityColor> {
    let color_index = reader.read_bs()?;
    let color_rgb = reader.read_bl()?;
    let color_byte = reader.read_rc()?;
    if (color_byte & 0x01) != 0 {
        let _color_name = reader.read_tv()?;
    }
    if (color_byte & 0x02) != 0 {
        let _book_name = reader.read_tv()?;
    }

    let true_color = if color_rgb == 0 || (color_rgb >> 24) == 0 {
        None
    } else {
        let rgb = color_rgb & 0x00FF_FFFF;
        if rgb == 0 {
            None
        } else {
            Some(rgb)
        }
    };

    Ok(CommonEntityColor {
        index: Some(color_index),
        true_color,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_common_entity_header_r2010, parse_common_entity_header_r2013};
    use crate::bit::{BitReader, BitWriter};

    fn build_minimal_common_header_bytes(r2013_plus: bool) -> Vec<u8> {
        let mut writer = BitWriter::new();
        writer.write_h(4, 0).expect("write handle");
        writer.write_bs(0).expect("write ext size");
        writer.write_b(0).expect("write graphic flag");
        writer.write_bb(0).expect("write entity mode");
        writer.write_bl(0).expect("write reactors");
        writer.write_b(1).expect("write xdic missing flag");
        if r2013_plus {
            writer.write_b(0).expect("write ds binary flag");
        }
        writer.write_b(1).expect("write no links");
        writer.write_b(0).expect("write color unknown");
        writer.write_bd(1.0).expect("write ltype scale");
        writer.write_bb(0).expect("write ltype flags");
        writer.write_bb(0).expect("write plotstyle flags");
        writer.write_bb(0).expect("write material flags");
        writer.write_rc(0).expect("write shadow flags");
        writer.write_b(0).expect("write full visual style flag");
        writer.write_b(0).expect("write face visual style flag");
        writer.write_b(0).expect("write edge visual style flag");
        writer.write_bs(0).expect("write invisibility");
        writer.write_rc(0).expect("write line weight");
        writer.into_bytes()
    }

    fn build_prefixed_r2010_entity_bytes(type_code: u16, r2013_plus: bool) -> Vec<u8> {
        let mut writer = BitWriter::new();
        writer.write_umc(0).expect("write handle stream size");
        writer.write_ot_r2010(type_code).expect("write type code");
        writer.align_byte();
        writer
            .write_rcs(&build_minimal_common_header_bytes(r2013_plus))
            .expect("write header body");
        writer.into_bytes()
    }

    fn build_shifted_r2010_entity_bytes(
        type_code: u16,
        r2013_plus: bool,
        padding_bits: u8,
    ) -> Vec<u8> {
        let mut writer = BitWriter::new();
        writer.write_umc(0).expect("write handle stream size");
        writer.write_ot_r2010(type_code).expect("write type code");
        for _ in 0..padding_bits {
            writer.write_b(0).expect("write padding bit");
        }
        writer
            .write_rcs(&build_minimal_common_header_bytes(r2013_plus))
            .expect("write shifted header body");
        writer.into_bytes()
    }

    #[test]
    fn parse_common_entity_header_r2010_recovers_from_byte_aligned_body() {
        let bytes = build_prefixed_r2010_entity_bytes(0x02, false);
        let mut reader = BitReader::new(&bytes);
        let _ = reader.read_umc().expect("read handle stream size");
        let _ = reader.read_ot_r2010().expect("read type code");
        assert_eq!(reader.get_pos(), (2, 2));

        let header =
            parse_common_entity_header_r2010(&mut reader, 64).expect("parse common header");

        assert_eq!(header.handle, 0);
        assert_eq!(header.obj_size, 64);
        assert!(reader.tell_bits() >= 24);
    }

    #[test]
    fn parse_common_entity_header_r2013_recovers_from_byte_aligned_body() {
        let bytes = build_prefixed_r2010_entity_bytes(0x03, true);
        let mut reader = BitReader::new(&bytes);
        let _ = reader.read_umc().expect("read handle stream size");
        let _ = reader.read_ot_r2010().expect("read type code");
        assert_eq!(reader.get_pos(), (2, 2));

        let header =
            parse_common_entity_header_r2013(&mut reader, 72).expect("parse common header");

        assert_eq!(header.handle, 0);
        assert_eq!(header.obj_size, 72);
        assert!(!header.has_ds_binary_data);
        assert!(reader.tell_bits() >= 24);
    }

    #[test]
    fn parse_common_entity_header_r2010_recovers_from_shifted_body() {
        let bytes = build_shifted_r2010_entity_bytes(0x02, false, 3);
        let mut reader = BitReader::new(&bytes);
        let _ = reader.read_umc().expect("read handle stream size");
        let _ = reader.read_ot_r2010().expect("read type code");
        assert_eq!(reader.get_pos(), (2, 2));

        let header =
            parse_common_entity_header_r2010(&mut reader, 64).expect("parse common header");

        assert_eq!(header.handle, 0);
        assert_eq!(header.obj_size, 64);
        assert!(reader.tell_bits() >= 24);
    }
}

fn skip_eed(reader: &mut BitReader<'_>) -> Result<()> {
    let mut ext_size = reader.read_bs()?;
    while ext_size > 0 {
        let _app_handle = reader.read_h()?;
        for _ in 0..ext_size {
            let _ = reader.read_rc()?;
        }
        ext_size = reader.read_bs()?;
    }
    Ok(())
}

pub fn parse_common_entity_handles(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
) -> Result<CommonEntityHandles> {
    let owner_ref = if header.entity_mode == 0 {
        Some(read_handle_reference(reader, header.handle)?)
    } else {
        None
    };

    let mut reactors = Vec::with_capacity(header.num_of_reactors as usize);
    for _ in 0..header.num_of_reactors {
        reactors.push(read_handle_reference(reader, header.handle)?);
    }

    let xdic_obj = if header.xdic_missing_flag == 0 {
        Some(read_handle_reference(reader, header.handle)?)
    } else {
        None
    };

    if header.has_legacy_entity_links {
        let _previous = read_handle_reference(reader, header.handle)?;
        let _next = read_handle_reference(reader, header.handle)?;
    }

    let layer = read_handle_reference(reader, header.handle)?;

    let ltype = if header.ltype_flags == 3 {
        Some(read_handle_reference(reader, header.handle)?)
    } else {
        None
    };

    let plotstyle = if header.plotstyle_flags == 3 {
        Some(read_handle_reference(reader, header.handle)?)
    } else {
        None
    };

    let material = if header.material_flags == 3 {
        Some(read_handle_reference(reader, header.handle)?)
    } else {
        None
    };

    if header.has_full_visual_style {
        let _full_visual_style = read_handle_reference(reader, header.handle)?;
    }
    if header.has_face_visual_style {
        let _face_visual_style = read_handle_reference(reader, header.handle)?;
    }
    if header.has_edge_visual_style {
        let _edge_visual_style = read_handle_reference(reader, header.handle)?;
    }

    Ok(CommonEntityHandles {
        owner_ref,
        reactors,
        xdic_obj,
        layer,
        ltype,
        plotstyle,
        material,
    })
}

pub fn parse_common_entity_layer_handle(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
) -> Result<u64> {
    if header.entity_mode == 0 {
        let _owner_ref = read_handle_reference(reader, header.handle)?;
    }

    for _ in 0..header.num_of_reactors {
        let _reactor = read_handle_reference(reader, header.handle)?;
    }

    if header.xdic_missing_flag == 0 {
        let _xdic_obj = read_handle_reference(reader, header.handle)?;
    }

    if header.has_legacy_entity_links {
        let _previous = read_handle_reference(reader, header.handle)?;
        let _next = read_handle_reference(reader, header.handle)?;
    }

    read_handle_reference(reader, header.handle)
}

pub fn read_handle_reference(reader: &mut BitReader<'_>, base_handle: u64) -> Result<u64> {
    let HandleRef { code, value, .. } = reader.read_h()?;
    let absolute = match code {
        0x06 => base_handle + 1,
        0x08 => base_handle.saturating_sub(1),
        0x0A => base_handle.saturating_add(value),
        0x0C => base_handle.saturating_sub(value),
        0x02..=0x05 => value,
        _ => value,
    };
    Ok(absolute)
}

pub fn read_additional_entity_handles(
    reader: &mut BitReader<'_>,
    base_handle: u64,
    max_count: usize,
) -> Vec<u64> {
    let mut handles = Vec::new();
    for _ in 0..max_count {
        let handle = match read_handle_reference(reader, base_handle) {
            Ok(value) => value,
            Err(_) => break,
        };
        if handle == 0 || handles.contains(&handle) {
            continue;
        }
        handles.push(handle);
    }
    handles
}
