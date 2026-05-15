use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct LeaderEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub annotation_type: u16,
    pub path_type: u16,
    pub points: Vec<(f64, f64, f64)>,
}

pub fn decode_leader(reader: &mut BitReader<'_>) -> Result<LeaderEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_leader_with_header(reader, header, false, false)
}

pub fn decode_leader_r2007(reader: &mut BitReader<'_>) -> Result<LeaderEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_leader_with_header(reader, header, true, true)
}

pub fn decode_leader_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LeaderEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_leader_with_header(reader, header, true, true)
}

pub fn decode_leader_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LeaderEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_leader_with_header(reader, header, true, true)
}

fn decode_leader_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<LeaderEntity> {
    let _unknown = reader.read_b()?;
    let annotation_type = reader.read_bs()?;
    let path_type = reader.read_bs()?;
    let num_points = bounded_count(reader.read_bl()?, "leader points")?;
    let mut points = Vec::with_capacity(num_points);
    for _ in 0..num_points {
        points.push(reader.read_3bd()?);
    }

    // Keep reading LEADER payload in a best-effort way so malformed optional
    // fields do not block core geometry extraction.
    if let Err(err) = skip_optional_leader_payload(reader) {
        if !matches!(
            err.kind,
            ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
        ) {
            return Err(err);
        }
    }

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let layer_handle = match if r2007_layer_only {
        parse_common_entity_layer_handle(reader, &header)
    } else {
        parse_common_entity_handles(reader, &header).map(|common_handles| common_handles.layer)
    } {
        Ok(layer_handle) => layer_handle,
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            0
        }
        Err(err) => return Err(err),
    };

    Ok(LeaderEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        annotation_type,
        path_type,
        points,
    })
}

fn skip_optional_leader_payload(reader: &mut BitReader<'_>) -> Result<()> {
    let _origin = reader.read_3bd()?;
    let _extrusion = reader.read_3bd()?;
    let _x_direction = reader.read_3bd()?;
    let _offset_to_block_insert = reader.read_3bd()?;
    let _endpoint_projection = reader.read_3bd()?;
    let _dimgap = reader.read_bd()?;
    let _box_height = reader.read_bd()?;
    let _box_width = reader.read_bd()?;
    let _hookline_on_x_dir = reader.read_b()?;
    let _arrowhead_on = reader.read_b()?;
    let _arrowhead_type = reader.read_bs()?;
    let _dimasz = reader.read_bd()?;
    let _unknown_a = reader.read_b()?;
    let _unknown_b = reader.read_b()?;
    let _unknown_c = reader.read_bs()?;
    let _by_block_color = reader.read_bs()?;
    let _unknown_d = reader.read_b()?;
    let _unknown_e = reader.read_b()?;
    let _unknown_f = reader.read_bs()?;
    let _unknown_g = reader.read_b()?;
    let _unknown_h = reader.read_b()?;
    Ok(())
}

fn bounded_count(raw: u32, label: &str) -> Result<usize> {
    let count = raw as usize;
    if count > 1_000_000 {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("{} count is too large: {}", label, count),
        ));
    }
    Ok(count)
}
