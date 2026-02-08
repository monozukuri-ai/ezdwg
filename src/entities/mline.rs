use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct MLineVertex {
    pub position: (f64, f64, f64),
    pub vertex_direction: (f64, f64, f64),
    pub miter_direction: (f64, f64, f64),
}

#[derive(Debug, Clone)]
pub struct MLineEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub scale: f64,
    pub justification: u8,
    pub base_point: (f64, f64, f64),
    pub extrusion: (f64, f64, f64),
    pub open_closed: u16,
    pub lines_in_style: u8,
    pub vertices: Vec<MLineVertex>,
    pub mlinestyle_handle: Option<u64>,
}

pub fn decode_mline(reader: &mut BitReader<'_>) -> Result<MLineEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_mline_with_header(reader, header, false, false)
}

pub fn decode_mline_r2007(reader: &mut BitReader<'_>) -> Result<MLineEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_mline_with_header(reader, header, true, true)
}

pub fn decode_mline_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MLineEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_mline_with_header(reader, header, true, true)
}

pub fn decode_mline_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MLineEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_mline_with_header(reader, header, true, true)
}

fn decode_mline_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<MLineEntity> {
    let scale = reader.read_bd()?;
    let justification = reader.read_rc()?;
    let base_point = reader.read_3bd()?;
    let extrusion = reader.read_3bd()?;
    let open_closed = reader.read_bs()?;
    let lines_in_style = reader.read_rc()?;
    let lines_count = bounded_count(lines_in_style as u32, "mline lines in style")?;
    if lines_count > 64 {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("mline lines in style is too large: {}", lines_count),
        ));
    }
    let vertex_count = bounded_count(reader.read_bs()? as u32, "mline vertices")?;

    let mut vertices = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        let position = reader.read_3bd()?;
        let vertex_direction = reader.read_3bd()?;
        let miter_direction = reader.read_3bd()?;
        for _ in 0..lines_count {
            let num_seg_parms = bounded_count(reader.read_bs()? as u32, "mline segment params")?;
            for _ in 0..num_seg_parms {
                let _segparm = reader.read_bd()?;
            }
            let num_area_fill_parms =
                bounded_count(reader.read_bs()? as u32, "mline area fill params")?;
            for _ in 0..num_area_fill_parms {
                let _areafillparm = reader.read_bd()?;
            }
        }
        vertices.push(MLineVertex {
            position,
            vertex_direction,
            miter_direction,
        });
    }

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let (layer_handle, mlinestyle_handle) = match if r2007_layer_only {
        parse_common_entity_layer_handle(reader, &header)
            .map(|layer| (layer, read_handle_reference(reader, header.handle).ok()))
    } else {
        parse_common_entity_handles(reader, &header).map(|common_handles| {
            (
                common_handles.layer,
                read_handle_reference(reader, header.handle).ok(),
            )
        })
    } {
        Ok(parsed) => parsed,
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            reader.set_pos(handles_pos.0, handles_pos.1);
            (
                parse_common_entity_layer_handle(reader, &header).unwrap_or(0),
                None,
            )
        }
        Err(err) => return Err(err),
    };

    Ok(MLineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        scale,
        justification,
        base_point,
        extrusion,
        open_closed,
        lines_in_style,
        vertices,
        mlinestyle_handle,
    })
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
