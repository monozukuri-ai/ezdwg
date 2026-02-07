use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct DimensionCommonData {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
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
    pub dimstyle_handle: Option<u64>,
    pub anonymous_block_handle: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DimLinearEntity {
    pub common: DimensionCommonData,
    pub point13: (f64, f64, f64),
    pub point14: (f64, f64, f64),
    pub point10: (f64, f64, f64),
    pub ext_line_rotation: f64,
    pub dim_rotation: f64,
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
    decode_dim_linear_with_header(reader, header, false)
}

pub fn decode_dim_linear_r2007(reader: &mut BitReader<'_>) -> Result<DimLinearEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_dim_linear_with_header(reader, header, true)
}

pub fn decode_dim_linear_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<DimLinearEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_dim_linear_r2010_plus_with_header(reader, header, true)
}

pub fn decode_dim_linear_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<DimLinearEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_dim_linear_r2010_plus_with_header(reader, header, true)
}

fn decode_dim_linear_r2010_plus_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
) -> Result<DimLinearEntity> {
    let data_pos = reader.get_pos();
    let variants = [
        R2010PlusVariant {
            has_dimension_version: true,
            has_user_text: true,
            extrusion_is_be: false,
        },
        R2010PlusVariant {
            has_dimension_version: true,
            has_user_text: false,
            extrusion_is_be: false,
        },
        R2010PlusVariant {
            has_dimension_version: false,
            has_user_text: true,
            extrusion_is_be: false,
        },
        R2010PlusVariant {
            has_dimension_version: false,
            has_user_text: false,
            extrusion_is_be: false,
        },
        R2010PlusVariant {
            has_dimension_version: true,
            has_user_text: true,
            extrusion_is_be: true,
        },
        R2010PlusVariant {
            has_dimension_version: true,
            has_user_text: false,
            extrusion_is_be: true,
        },
        R2010PlusVariant {
            has_dimension_version: false,
            has_user_text: true,
            extrusion_is_be: true,
        },
        R2010PlusVariant {
            has_dimension_version: false,
            has_user_text: false,
            extrusion_is_be: true,
        },
    ];

    let mut best: Option<(u64, DimLinearEntity)> = None;
    let mut last_error: Option<DwgError> = None;
    for parse_variant in variants {
        reader.set_pos(data_pos.0, data_pos.1);
        match decode_r2010_plus_variant(
            reader,
            &header,
            parse_variant,
            allow_handle_decode_failure,
        ) {
            Ok(entity) => {
                let score = plausibility_score(&entity);
                match &best {
                    Some((best_score, _)) if score >= *best_score => {}
                    _ => best = Some((score, entity)),
                }
            }
            Err(err) => last_error = Some(err),
        }
    }

    if let Some((_, entity)) = best {
        return Ok(entity);
    }

    Err(last_error.unwrap_or_else(|| {
        DwgError::new(
            ErrorKind::Decode,
            "failed to decode R2010+ DIM_LINEAR with all variants",
        )
    }))
}

#[derive(Clone, Copy)]
struct R2010PlusVariant {
    has_dimension_version: bool,
    has_user_text: bool,
    extrusion_is_be: bool,
}

fn decode_r2010_plus_variant(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
    parse_variant: R2010PlusVariant,
    allow_handle_decode_failure: bool,
) -> Result<DimLinearEntity> {
    if parse_variant.has_dimension_version {
        let _dimension_version = reader.read_rc()?;
    }
    let extrusion = if parse_variant.extrusion_is_be {
        reader.read_be()?
    } else {
        reader.read_3bd()?
    };
    let text_mid_x = reader.read_rd(Endian::Little)?;
    let text_mid_y = reader.read_rd(Endian::Little)?;
    let elevation = reader.read_bd()?;
    let dim_flags = reader.read_rc()?;
    let user_text = if parse_variant.has_user_text {
        reader.read_tv()?
    } else {
        String::new()
    };
    let text_rotation = reader.read_bd()?;
    let horizontal_direction = reader.read_bd()?;
    let scale_x = reader.read_bd()?;
    let scale_y = reader.read_bd()?;
    let scale_z = reader.read_bd()?;
    let insert_rotation = reader.read_bd()?;
    let attachment_point = Some(reader.read_bs()?);
    let line_spacing_style = Some(reader.read_bs()?);
    let line_spacing_factor = Some(reader.read_bd()?);
    let actual_measurement = Some(reader.read_bd()?);
    let _unknown = reader.read_b()?;
    let _flip_arrow1 = reader.read_b()?;
    let _flip_arrow2 = reader.read_b()?;
    let point12_x = reader.read_rd(Endian::Little)?;
    let point12_y = reader.read_rd(Endian::Little)?;
    let insert_point = Some((point12_x, point12_y, elevation));

    let point13 = reader.read_3bd()?;
    let point14 = reader.read_3bd()?;
    let point10 = reader.read_3bd()?;
    let ext_line_rotation = reader.read_bd()?;
    let dim_rotation = reader.read_bd()?;

    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let (dimstyle_handle, anonymous_block_handle, layer_handle) = match (
        read_handle_reference(reader, header.handle),
        read_handle_reference(reader, header.handle),
        parse_common_entity_handles(reader, header),
    ) {
        (Ok(dimstyle), Ok(block), Ok(common_handles)) => {
            (Some(dimstyle), Some(block), common_handles.layer)
        }
        _ if allow_handle_decode_failure => {
            reader.set_pos(handles_pos.0, handles_pos.1);
            let layer = parse_common_entity_layer_handle(reader, header).unwrap_or(0);
            (None, None, layer)
        }
        _ => {
            reader.set_pos(handles_pos.0, handles_pos.1);
            return Err(DwgError::new(
                ErrorKind::Decode,
                "failed to decode DIM_LINEAR handles",
            ));
        }
    };

    let common = DimensionCommonData {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
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
        dimstyle_handle,
        anonymous_block_handle,
    };

    Ok(DimLinearEntity {
        common,
        point13,
        point14,
        point10,
        ext_line_rotation,
        dim_rotation,
    })
}

fn decode_dim_linear_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
) -> Result<DimLinearEntity> {
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

    let mut best: Option<(u64, DimLinearEntity)> = None;
    let mut last_error: Option<DwgError> = None;
    for parse_variant in variants {
        reader.set_pos(data_pos.0, data_pos.1);
        match decode_variant(reader, &header, parse_variant, allow_handle_decode_failure) {
            Ok(entity) => {
                let score = plausibility_score(&entity);
                match &best {
                    Some((best_score, _)) if score >= *best_score => {}
                    _ => best = Some((score, entity)),
                }
            }
            Err(err) => last_error = Some(err),
        }
    }

    if let Some((_, entity)) = best {
        return Ok(entity);
    }

    Err(last_error
        .unwrap_or_else(|| DwgError::new(ErrorKind::Decode, "failed to decode DIM_LINEAR")))
}

fn decode_variant(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
    parse_variant: DimLinearVariant,
    allow_handle_decode_failure: bool,
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

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let (dimstyle_handle, anonymous_block_handle, layer_handle) = if allow_handle_decode_failure {
        let layer = parse_common_entity_layer_handle(reader, header).unwrap_or(0);
        (None, None, layer)
    } else if parse_variant.style_before_common {
        let dimstyle = Some(read_handle_reference(reader, header.handle)?);
        let block = Some(read_handle_reference(reader, header.handle)?);
        let common_handles = parse_common_entity_handles(reader, header)?;
        (dimstyle, block, common_handles.layer)
    } else {
        match parse_common_entity_handles(reader, header) {
            Ok(common_handles) => (
                read_handle_reference(reader, header.handle).ok(),
                read_handle_reference(reader, header.handle).ok(),
                common_handles.layer,
            ),
            Err(err) => {
                reader.set_pos(handles_pos.0, handles_pos.1);
                return Err(err);
            }
        }
    };

    let common = DimensionCommonData {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
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
        dimstyle_handle,
        anonymous_block_handle,
    };

    Ok(DimLinearEntity {
        common,
        point13,
        point14,
        point10,
        ext_line_rotation,
        dim_rotation,
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

fn plausibility_score(entity: &DimLinearEntity) -> u64 {
    let mut score = 0u64;
    let common = &entity.common;

    for pt in [
        entity.point10,
        entity.point13,
        entity.point14,
        common.text_midpoint,
    ] {
        score = score.saturating_add(point_score(pt));
    }
    if let Some(insert_point) = common.insert_point {
        score = score.saturating_add(point_score(insert_point));
    }
    score = score.saturating_add(point_score(common.extrusion));
    score = score.saturating_add(point_score(common.insert_scale));
    score = score.saturating_add(extrusion_score(common.extrusion));
    score = score.saturating_add(scale_score(common.insert_scale));

    for angle in [
        common.text_rotation,
        common.horizontal_direction,
        entity.ext_line_rotation,
        entity.dim_rotation,
        common.insert_rotation,
    ] {
        score = score.saturating_add(angle_score(angle));
    }

    if let Some(measurement) = common.actual_measurement {
        score = score.saturating_add(value_score(measurement));
    }
    if let Some(line_spacing) = common.line_spacing_factor {
        score = score.saturating_add(value_score(line_spacing));
    }
    if let Some(attachment_point) = common.attachment_point {
        if attachment_point > 9 {
            score = score.saturating_add(10_000);
        }
    }
    if let Some(line_spacing_style) = common.line_spacing_style {
        if line_spacing_style > 2 {
            score = score.saturating_add(10_000);
        }
    }
    if common.dim_flags > 0x3F {
        score = score.saturating_add(1_000);
    }

    score
}

fn extrusion_score(extrusion: (f64, f64, f64)) -> u64 {
    if !extrusion.0.is_finite() || !extrusion.1.is_finite() || !extrusion.2.is_finite() {
        return 1_000_000;
    }
    let norm_sq = extrusion.0 * extrusion.0 + extrusion.1 * extrusion.1 + extrusion.2 * extrusion.2;
    if norm_sq <= 1e-12 {
        return 50_000;
    }
    let norm = norm_sq.sqrt();
    let mut score = 0u64;
    let norm_err = (norm - 1.0).abs();
    if norm_err > 0.25 {
        score = score.saturating_add(25_000);
    } else if norm_err > 0.05 {
        score = score.saturating_add(2_500);
    }
    if extrusion.2.abs() < 0.5 {
        score = score.saturating_add(250);
    }
    score
}

fn scale_score(scale: (f64, f64, f64)) -> u64 {
    let mut score = 0u64;
    for value in [scale.0, scale.1, scale.2] {
        if !value.is_finite() {
            return 1_000_000;
        }
        if value.abs() < 1e-12 {
            score = score.saturating_add(2_500);
        } else if value.abs() > 1_000.0 {
            score = score.saturating_add(250);
        }
    }
    score
}

fn point_score(point: (f64, f64, f64)) -> u64 {
    value_score(point.0)
        .saturating_add(value_score(point.1))
        .saturating_add(value_score(point.2))
}

fn angle_score(value: f64) -> u64 {
    if !value.is_finite() {
        return 1_000_000;
    }
    let abs = value.abs();
    if abs <= 1_000.0 {
        0
    } else if abs <= 1_000_000.0 {
        25
    } else if abs <= 1_000_000_000_000.0 {
        250
    } else {
        1_000_000
    }
}

fn value_score(value: f64) -> u64 {
    if !value.is_finite() {
        return 1_000_000;
    }
    let abs = value.abs();
    if abs <= 1_000_000.0 {
        0
    } else if abs <= 1_000_000_000.0 {
        10
    } else if abs <= 1_000_000_000_000.0 {
        100
    } else if abs <= 1.0e18 {
        1_000
    } else if abs <= 1.0e24 {
        10_000
    } else {
        1_000_000
    }
}
