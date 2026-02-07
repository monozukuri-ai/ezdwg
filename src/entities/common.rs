use crate::bit::HandleRef;
use crate::bit::{BitReader, Endian};
use crate::core::result::Result;

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
    pub ltype_flags: u8,
    pub plotstyle_flags: u8,
}

#[derive(Debug, Clone)]
pub struct CommonEntityHandles {
    pub owner_ref: Option<u64>,
    pub reactors: Vec<u64>,
    pub xdic_obj: Option<u64>,
    pub layer: u64,
    pub ltype: Option<u64>,
    pub plotstyle: Option<u64>,
}

pub fn parse_common_entity_header(reader: &mut BitReader<'_>) -> Result<CommonEntityHeader> {
    let obj_size = reader.read_rl(Endian::Little)?;
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
    if graphic_present_flag == 1 {
        let graphic_size = reader.read_rl(Endian::Little)? as usize;
        let _ = reader.read_rcs(graphic_size)?;
    }

    let entity_mode = reader.read_bb()?;
    let num_of_reactors = reader.read_bl()?;
    let xdic_missing_flag = reader.read_b()?;

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

    let _invisibility = reader.read_bs()?;
    let _line_weight = reader.read_rc()?;

    Ok(CommonEntityHeader {
        obj_size,
        handle,
        color,
        entity_mode,
        num_of_reactors,
        xdic_missing_flag,
        ltype_flags,
        plotstyle_flags,
    })
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

    Ok(CommonEntityHandles {
        owner_ref,
        reactors,
        xdic_obj,
        layer,
        ltype,
        plotstyle,
    })
}

pub fn read_handle_reference(reader: &mut BitReader<'_>, base_handle: u64) -> Result<u64> {
    let HandleRef { code, value, .. } = reader.read_h()?;
    let absolute = match code {
        0x06 => base_handle + 1,
        0x08 => base_handle.saturating_sub(1),
        0x0A => base_handle.saturating_add(value),
        0x0C => base_handle.saturating_sub(value),
        0x02 | 0x03 | 0x04 | 0x05 => value,
        _ => value,
    };
    Ok(absolute)
}
