use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, parse_common_entity_layer_handle, read_handle_reference,
    CommonEntityHeader,
};
use crate::entities::dim_common::{plausibility_score, R2010PlusVariant, R2010_PLUS_VARIANTS};
use crate::entities::dim_linear::{
    decode_dim_linear, decode_dim_linear_r2007, DimLinearEntity, DimensionCommonData,
};

pub type DimRadiusEntity = DimLinearEntity;

pub fn decode_dim_radius(reader: &mut BitReader<'_>) -> Result<DimRadiusEntity> {
    // R2000/R2004 radius dimensions share a largely compatible body layout
    // with linear dimensions for the fields we currently surface.
    decode_dim_linear(reader)
}

pub fn decode_dim_radius_r2007(reader: &mut BitReader<'_>) -> Result<DimRadiusEntity> {
    decode_dim_linear_r2007(reader)
}

pub fn decode_dim_radius_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<DimRadiusEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_dim_radius_r2010_plus_with_header(reader, header, true)
}

pub fn decode_dim_radius_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<DimRadiusEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_dim_radius_r2010_plus_with_header(reader, header, true)
}

fn decode_dim_radius_r2010_plus_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
) -> Result<DimRadiusEntity> {
    let data_pos = reader.get_pos();

    let mut best: Option<(u64, DimRadiusEntity)> = None;
    let mut last_error: Option<DwgError> = None;
    for parse_variant in R2010_PLUS_VARIANTS {
        reader.set_pos(data_pos.0, data_pos.1);
        match decode_r2010_plus_variant(reader, &header, parse_variant, allow_handle_decode_failure)
        {
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
            "failed to decode R2010+ DIM_RADIUS with all variants",
        )
    }))
}

fn decode_r2010_plus_variant(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
    parse_variant: R2010PlusVariant,
    allow_handle_decode_failure: bool,
) -> Result<DimRadiusEntity> {
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

    // DIM_RADIUS: 10-pt, 15-pt, leader length
    let point10 = reader.read_3bd()?;
    let point15 = reader.read_3bd()?;
    let _leader_length = reader.read_bd()?;

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
                "failed to decode DIM_RADIUS handles",
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
        point13: point10,
        point14: point15,
        point10,
        ext_line_rotation: 0.0,
        dim_rotation: 0.0,
    })
}
