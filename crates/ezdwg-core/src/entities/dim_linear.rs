use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};
use crate::entities::dim_common::{plausibility_score, R2010PlusVariant, R2010_PLUS_VARIANTS};

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
    /// DXF code 15 (angular vertex / second-line start); `None` for other types.
    pub point15: Option<(f64, f64, f64)>,
    /// DXF code 16 (2-line angular dimension arc point); `None` for other types.
    pub point16: Option<(f64, f64)>,
}

/// Type-specific tail of a dimension object (ODA spec 20.4.22-20.4.27).
///
/// The common dimension data is shared; only the fields after the 12-pt differ:
/// - LINEAR: 3BD 13, 3BD 14, 3BD 10, BD ext-line rotation, BD dim rotation
/// - ALIGNED: 3BD 13, 3BD 14, 3BD 10, BD ext-line rotation
/// - ANG3PT: 3BD 10, 3BD 13, 3BD 14, 3BD 15
/// - ANG2LN: 2RD 16, 3BD 13, 3BD 14, 3BD 15, 3BD 10
/// - ORDINATE: 3BD 10, 3BD 13, 3BD 14, RC flags2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimSpecificLayout {
    Linear,
    Aligned,
    Ang3Pt,
    Ang2Ln,
    Ordinate,
}

struct DimSpecificData {
    point13: (f64, f64, f64),
    point14: (f64, f64, f64),
    point10: (f64, f64, f64),
    ext_line_rotation: f64,
    dim_rotation: f64,
    point15: Option<(f64, f64, f64)>,
    point16: Option<(f64, f64)>,
}

fn read_dim_specific(
    reader: &mut BitReader<'_>,
    layout: DimSpecificLayout,
) -> Result<DimSpecificData> {
    let mut data = DimSpecificData {
        point13: (0.0, 0.0, 0.0),
        point14: (0.0, 0.0, 0.0),
        point10: (0.0, 0.0, 0.0),
        ext_line_rotation: 0.0,
        dim_rotation: 0.0,
        point15: None,
        point16: None,
    };
    match layout {
        DimSpecificLayout::Linear => {
            data.point13 = reader.read_3bd()?;
            data.point14 = reader.read_3bd()?;
            data.point10 = reader.read_3bd()?;
            data.ext_line_rotation = reader.read_bd()?;
            data.dim_rotation = reader.read_bd()?;
        }
        DimSpecificLayout::Aligned => {
            data.point13 = reader.read_3bd()?;
            data.point14 = reader.read_3bd()?;
            data.point10 = reader.read_3bd()?;
            data.ext_line_rotation = reader.read_bd()?;
        }
        DimSpecificLayout::Ang3Pt => {
            data.point10 = reader.read_3bd()?;
            data.point13 = reader.read_3bd()?;
            data.point14 = reader.read_3bd()?;
            data.point15 = Some(reader.read_3bd()?);
        }
        DimSpecificLayout::Ang2Ln => {
            let x16 = reader.read_rd(Endian::Little)?;
            let y16 = reader.read_rd(Endian::Little)?;
            data.point16 = Some((x16, y16));
            data.point13 = reader.read_3bd()?;
            data.point14 = reader.read_3bd()?;
            data.point15 = Some(reader.read_3bd()?);
            data.point10 = reader.read_3bd()?;
        }
        DimSpecificLayout::Ordinate => {
            data.point10 = reader.read_3bd()?;
            data.point13 = reader.read_3bd()?;
            data.point14 = reader.read_3bd()?;
            let _flags2 = reader.read_rc()?;
        }
    }
    Ok(data)
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
    decode_dim_layout(reader, DimSpecificLayout::Linear)
}

pub fn decode_dim_linear_r2007(reader: &mut BitReader<'_>) -> Result<DimLinearEntity> {
    decode_dim_layout_r2007(reader, DimSpecificLayout::Linear)
}

pub fn decode_dim_linear_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<DimLinearEntity> {
    decode_dim_layout_r2010(
        reader,
        object_data_end_bit,
        object_handle,
        DimSpecificLayout::Linear,
    )
}

pub fn decode_dim_linear_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<DimLinearEntity> {
    decode_dim_layout_r2013(
        reader,
        object_data_end_bit,
        object_handle,
        DimSpecificLayout::Linear,
    )
}

/// R2000/R2004 dimension of the given type-specific layout.
pub fn decode_dim_layout(
    reader: &mut BitReader<'_>,
    layout: DimSpecificLayout,
) -> Result<DimLinearEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_dim_linear_with_header(reader, header, false, layout)
}

/// R2007 dimension of the given type-specific layout.
pub fn decode_dim_layout_r2007(
    reader: &mut BitReader<'_>,
    layout: DimSpecificLayout,
) -> Result<DimLinearEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    let data_pos = reader.get_pos();
    // R2007 has no dimension version byte and keeps the user text in the string
    // stream (no bits in the data stream). The R2010+ variant table contains that
    // exact layout, so try it first; the R2000-style variants (inline TV text)
    // stay as a fallback for writers that deviate.
    if let Ok(entity) =
        decode_dim_linear_r2010_plus_with_header(reader, header.clone(), true, layout)
    {
        return Ok(entity);
    }
    reader.set_pos(data_pos.0, data_pos.1);
    decode_dim_linear_with_header(reader, header, true, layout)
}

/// R2010 dimension of the given type-specific layout.
pub fn decode_dim_layout_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
    layout: DimSpecificLayout,
) -> Result<DimLinearEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_dim_linear_r2010_plus_with_header(reader, header, true, layout)
}

/// R2013+ dimension of the given type-specific layout.
pub fn decode_dim_layout_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
    layout: DimSpecificLayout,
) -> Result<DimLinearEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_dim_linear_r2010_plus_with_header(reader, header, true, layout)
}

fn decode_dim_linear_r2010_plus_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    layout: DimSpecificLayout,
) -> Result<DimLinearEntity> {
    let data_pos = reader.get_pos();

    let mut best: Option<(u64, DimLinearEntity)> = None;
    let mut last_error: Option<DwgError> = None;
    let debug = std::env::var("EZDWG_DEBUG_DIM")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|handle| handle == header.handle);
    for parse_variant in R2010_PLUS_VARIANTS {
        reader.set_pos(data_pos.0, data_pos.1);
        match decode_r2010_plus_variant(
            reader,
            &header,
            parse_variant,
            allow_handle_decode_failure,
            layout,
        ) {
            Ok(entity) => {
                let score = plausibility_score(&entity);
                if debug {
                    eprintln!(
                        "[ezdwg dim r2010+ {:?}] handle={:#x} variant={:?} score={} p10={:?} p13={:?} p14={:?} p15={:?} p16={:?} mid={:?} ins={:?} meas={:?} ext={:?}",
                        layout,
                        header.handle,
                        parse_variant,
                        score,
                        entity.point10,
                        entity.point13,
                        entity.point14,
                        entity.point15,
                        entity.point16,
                        entity.common.text_midpoint,
                        entity.common.insert_point,
                        entity.common.actual_measurement,
                        entity.common.extrusion,
                    );
                }
                match &best {
                    Some((best_score, _)) if score >= *best_score => {}
                    _ => best = Some((score, entity)),
                }
            }
            Err(err) => {
                if debug {
                    eprintln!(
                        "[ezdwg dim r2010+ {:?}] handle={:#x} variant={:?} error={}",
                        layout, header.handle, parse_variant, err
                    );
                }
                last_error = Some(err)
            }
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

fn decode_r2010_plus_variant(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
    parse_variant: R2010PlusVariant,
    allow_handle_decode_failure: bool,
    layout: DimSpecificLayout,
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
    if parse_variant.has_r2007_flags {
        let _unknown = reader.read_b()?;
        let _flip_arrow1 = reader.read_b()?;
        let _flip_arrow2 = reader.read_b()?;
    }
    let point12_x = reader.read_rd(Endian::Little)?;
    let point12_y = reader.read_rd(Endian::Little)?;
    let insert_point = Some((point12_x, point12_y, elevation));

    let specific = read_dim_specific(reader, layout)?;
    let point13 = specific.point13;
    let point14 = specific.point14;
    let point10 = specific.point10;
    let ext_line_rotation = specific.ext_line_rotation;
    let dim_rotation = specific.dim_rotation;

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
        point15: specific.point15,
        point16: specific.point16,
    })
}

fn decode_dim_linear_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    layout: DimSpecificLayout,
) -> Result<DimLinearEntity> {
    let data_pos = reader.get_pos();

    // The R2000-R2004 layout per spec (attachment block present, no R2007+
    // unknown/flip-arrow flags, 12-pt present) goes first: candidates are only
    // replaced by a strictly better score, so on ties the spec layout wins
    // instead of a coincidentally plausible mis-alignment.
    let variants = [
        variant(true, false, false, false, true, true),
        variant(true, false, false, false, true, false),
        variant(true, true, true, true, true, true),
        variant(true, true, true, false, true, true),
        variant(true, true, false, false, true, true),
        variant(true, false, false, false, false, true),
        variant(false, false, false, false, false, true),
        variant(true, true, true, true, true, false),
        variant(true, true, true, false, true, false),
        variant(true, true, false, false, true, false),
        variant(true, false, false, false, false, false),
        variant(false, false, false, false, false, false),
    ];

    let mut best: Option<(u64, DimLinearEntity)> = None;
    let mut last_error: Option<DwgError> = None;
    let debug = std::env::var("EZDWG_DEBUG_DIM")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|handle| handle == header.handle);
    for parse_variant in variants {
        reader.set_pos(data_pos.0, data_pos.1);
        match decode_variant(
            reader,
            &header,
            parse_variant,
            allow_handle_decode_failure,
            layout,
        ) {
            Ok(entity) => {
                let score = plausibility_score(&entity);
                if debug {
                    eprintln!(
                        "[dim debug] handle={} variant=(att={} unk={} f1={} f2={} p12={} sbc={}) score={score} p13={:?} p14={:?} p10={:?} ext_rot={} dim_rot={} text={:?} meas={:?} insert={:?}",
                        header.handle,
                        parse_variant.has_attachment,
                        parse_variant.has_unknown_flag,
                        parse_variant.has_flip_arrow1,
                        parse_variant.has_flip_arrow2,
                        parse_variant.has_point12,
                        parse_variant.style_before_common,
                        entity.point13,
                        entity.point14,
                        entity.point10,
                        entity.ext_line_rotation,
                        entity.dim_rotation,
                        entity.common.user_text,
                        entity.common.actual_measurement,
                        entity.common.insert_point
                    );
                }
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
    layout: DimSpecificLayout,
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

    let specific = read_dim_specific(reader, layout)?;
    let point13 = specific.point13;
    let point14 = specific.point14;
    let point10 = specific.point10;
    let ext_line_rotation = specific.ext_line_rotation;
    let dim_rotation = specific.dim_rotation;

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
        point15: specific.point15,
        point16: specific.point16,
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
