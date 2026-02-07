use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, read_handle_reference,
    CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct DimLinearEntity {
    pub handle: u64,
    pub extrusion: (f64, f64, f64),
    pub text_midpoint: (f64, f64, f64),
    pub elevation: f64,
    pub dim_flags: u8,
    pub user_text: String,
    pub text_rotation: f64,
    pub horizontal_direction: f64,
    pub insert_scale: (f64, f64, f64),
    pub insert_rotation: f64,
    pub attachment_point: Option<u16>,
    pub line_spacing_style: Option<u16>,
    pub line_spacing_factor: Option<f64>,
    pub actual_measurement: Option<f64>,
    pub insert_point: Option<(f64, f64, f64)>,
    pub point13: (f64, f64, f64),
    pub point14: (f64, f64, f64),
    pub point10: (f64, f64, f64),
    pub ext_line_rotation: f64,
    pub dim_rotation: f64,
    pub dimstyle_handle: Option<u64>,
    pub anonymous_block_handle: Option<u64>,
}

#[derive(Clone, Copy)]
struct DimLinearVariant {
    has_attachment: bool,
    has_unknown_flag: bool,
    has_flip_arrow1: bool,
    has_flip_arrow2: bool,
    has_point12: bool,
    style_before_common: bool,
}

pub fn decode_dim_linear(reader: &mut BitReader<'_>) -> Result<DimLinearEntity> {
    let header = parse_common_entity_header(reader)?;
    let data_pos = reader.get_pos();

    let variants = [
        variant(true, true, true, true, true, true),
        variant(true, true, true, false, true, true),
        variant(true, true, false, false, true, true),
        variant(true, false, false, false, true, true),
        variant(true, false, false, false, false, true),
        variant(false, false, false, false, false, true),
        variant(true, true, true, true, true, false),
        variant(true, true, true, false, true, false),
        variant(true, true, false, false, true, false),
        variant(true, false, false, false, true, false),
        variant(true, false, false, false, false, false),
        variant(false, false, false, false, false, false),
    ];

    let mut last_error: Option<DwgError> = None;
    for parse_variant in variants {
        reader.set_pos(data_pos.0, data_pos.1);
        match decode_variant(reader, &header, parse_variant) {
            Ok(entity) => return Ok(entity),
            Err(err) => last_error = Some(err),
        }
    }

    Err(last_error
        .unwrap_or_else(|| DwgError::new(ErrorKind::Decode, "failed to decode DIM_LINEAR")))
}

fn decode_variant(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
    parse_variant: DimLinearVariant,
) -> Result<DimLinearEntity> {
    let extrusion = reader.read_3bd()?;
    let text_mid_x = reader.read_rd(Endian::Little)?;
    let text_mid_y = reader.read_rd(Endian::Little)?;
    let elevation = reader.read_bd()?;
    let dim_flags = reader.read_rc()?;
    let user_text = reader.read_tv()?;
    let text_rotation = reader.read_bd()?;
    let horizontal_direction = reader.read_bd()?;
    let scale_x = reader.read_bd()?;
    let scale_y = reader.read_bd()?;
    let scale_z = reader.read_bd()?;
    let insert_rotation = reader.read_bd()?;

    let (attachment_point, line_spacing_style, line_spacing_factor, actual_measurement) =
        if parse_variant.has_attachment {
            (
                Some(reader.read_bs()?),
                Some(reader.read_bs()?),
                Some(reader.read_bd()?),
                Some(reader.read_bd()?),
            )
        } else {
            (None, None, None, None)
        };

    if parse_variant.has_unknown_flag {
        let _unknown = reader.read_b()?;
    }
    if parse_variant.has_flip_arrow1 {
        let _flip_arrow1 = reader.read_b()?;
    }
    if parse_variant.has_flip_arrow2 {
        let _flip_arrow2 = reader.read_b()?;
    }

    let insert_point = if parse_variant.has_point12 {
        let x = reader.read_rd(Endian::Little)?;
        let y = reader.read_rd(Endian::Little)?;
        Some((x, y, elevation))
    } else {
        None
    };

    let point13 = reader.read_3bd()?;
    let point14 = reader.read_3bd()?;
    let point10 = reader.read_3bd()?;
    let ext_line_rotation = reader.read_bd()?;
    let dim_rotation = reader.read_bd()?;

    let (dimstyle_handle, anonymous_block_handle) = if parse_variant.style_before_common {
        let dimstyle = Some(read_handle_reference(reader, header.handle)?);
        let block = Some(read_handle_reference(reader, header.handle)?);
        let _common_handles = parse_common_entity_handles(reader, header)?;
        (dimstyle, block)
    } else {
        let _common_handles = parse_common_entity_handles(reader, header)?;
        (
            read_handle_reference(reader, header.handle).ok(),
            read_handle_reference(reader, header.handle).ok(),
        )
    };

    Ok(DimLinearEntity {
        handle: header.handle,
        extrusion,
        text_midpoint: (text_mid_x, text_mid_y, elevation),
        elevation,
        dim_flags,
        user_text,
        text_rotation,
        horizontal_direction,
        insert_scale: (scale_x, scale_y, scale_z),
        insert_rotation,
        attachment_point,
        line_spacing_style,
        line_spacing_factor,
        actual_measurement,
        insert_point,
        point13,
        point14,
        point10,
        ext_line_rotation,
        dim_rotation,
        dimstyle_handle,
        anonymous_block_handle,
    })
}

const fn variant(
    has_attachment: bool,
    has_unknown_flag: bool,
    has_flip_arrow1: bool,
    has_flip_arrow2: bool,
    has_point12: bool,
    style_before_common: bool,
) -> DimLinearVariant {
    DimLinearVariant {
        has_attachment,
        has_unknown_flag,
        has_flip_arrow1,
        has_flip_arrow2,
        has_point12,
        style_before_common,
    }
}
