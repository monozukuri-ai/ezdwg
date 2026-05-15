use crate::bit::{BitReader, Endian};
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct TraceEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub p1: (f64, f64, f64),
    pub p2: (f64, f64, f64),
    pub p3: (f64, f64, f64),
    pub p4: (f64, f64, f64),
    pub thickness: f64,
    pub extrusion: (f64, f64, f64),
}

pub fn decode_trace(reader: &mut BitReader<'_>) -> Result<TraceEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_trace_with_header(reader, header, false, false)
}

pub fn decode_trace_r2007(reader: &mut BitReader<'_>) -> Result<TraceEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_trace_with_header(reader, header, true, true)
}

pub fn decode_trace_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<TraceEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_trace_with_header(reader, header, true, true)
}

pub fn decode_trace_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<TraceEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_trace_with_header(reader, header, true, true)
}

fn decode_trace_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<TraceEntity> {
    let thickness = reader.read_bt()?;
    let elevation = reader.read_bd()?;

    let c1 = read_2rd(reader)?;
    let c2 = read_2rd(reader)?;
    let c3 = read_2rd(reader)?;
    let c4 = read_2rd(reader)?;

    let extrusion = reader.read_be()?;

    let p1 = (c1.0, c1.1, elevation);
    let p2 = (c2.0, c2.1, elevation);
    let p3 = (c3.0, c3.1, elevation);
    let p4 = (c4.0, c4.1, elevation);

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

    Ok(TraceEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        p1,
        p2,
        p3,
        p4,
        thickness,
        extrusion,
    })
}

fn read_2rd(reader: &mut BitReader<'_>) -> Result<(f64, f64)> {
    Ok((
        reader.read_rd(Endian::Little)?,
        reader.read_rd(Endian::Little)?,
    ))
}
