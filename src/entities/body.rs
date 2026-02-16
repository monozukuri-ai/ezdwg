use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, parse_common_entity_layer_handle,
    read_additional_entity_handles, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct BodyEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub acis_handles: Vec<u64>,
}

pub fn decode_body(reader: &mut BitReader<'_>) -> Result<BodyEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_body_with_header(reader, header, false, false)
}

pub fn decode_body_r14(reader: &mut BitReader<'_>, object_handle: u64) -> Result<BodyEntity> {
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    decode_body_with_header(reader, header, true, false)
}

pub fn decode_body_r2007(reader: &mut BitReader<'_>) -> Result<BodyEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_body_with_header(reader, header, true, true)
}

pub fn decode_body_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<BodyEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_body_with_header(reader, header, true, true)
}

pub fn decode_body_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<BodyEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_body_with_header(reader, header, true, true)
}

fn decode_body_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<BodyEntity> {
    // BODY ACIS payload decode is TODO. Expose common metadata first.
    reader.set_bit_pos(header.obj_size);
    let handles_start = reader.get_pos();
    let (layer_handle, mut acis_handles) = if r2007_layer_only {
        match parse_common_entity_handles(reader, &header) {
            Ok(common_handles) => (
                common_handles.layer,
                read_additional_entity_handles(reader, header.handle, 8),
            ),
            Err(err)
                if allow_handle_decode_failure
                    && matches!(
                        err.kind,
                        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                    ) =>
            {
                reader.set_pos(handles_start.0, handles_start.1);
                let layer = parse_common_entity_layer_handle(reader, &header).unwrap_or(0);
                let extra = read_additional_entity_handles(reader, header.handle, 8);
                (layer, extra)
            }
            Err(err) => return Err(err),
        }
    } else {
        match parse_common_entity_handles(reader, &header) {
            Ok(common_handles) => (
                common_handles.layer,
                read_additional_entity_handles(reader, header.handle, 8),
            ),
            Err(err)
                if allow_handle_decode_failure
                    && matches!(
                        err.kind,
                        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                    ) =>
            {
                (0, Vec::new())
            }
            Err(err) => return Err(err),
        }
    };

    acis_handles.retain(|handle| *handle != layer_handle);

    Ok(BodyEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        acis_handles,
    })
}
