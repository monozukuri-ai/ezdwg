use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::parse_common_entity_header_r14_with_handle;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, string_stream_reader,
    CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct ToleranceEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub text: String,
    pub insertion: (f64, f64, f64),
    pub x_direction: (f64, f64, f64),
    pub extrusion: (f64, f64, f64),
    /// Text height and gap as R13/R14 store them. Later versions take both
    /// from the dimension style and store nothing: the values are 0 there.
    pub height: f64,
    pub dimgap: f64,
    pub dimstyle_handle: Option<u64>,
}

/// What a TOLERANCE record holds besides its three vectors (ODA specification
/// 20.4.56).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToleranceLayout {
    /// R13/R14: an unknown short, the text height and the gap come first.
    R14,
    /// R2000/R2004: the vectors, then the text.
    Inline,
    /// R2007+: the vectors; the text is in the string stream.
    StringStream,
}

pub fn decode_tolerance(reader: &mut BitReader<'_>) -> Result<ToleranceEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_tolerance_with_header(reader, header, false, false, ToleranceLayout::Inline)
}

pub fn decode_tolerance_r14(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<ToleranceEntity> {
    let header = parse_common_entity_header_r14_with_handle(reader, object_handle)?;
    decode_tolerance_with_header(reader, header, true, false, ToleranceLayout::R14)
}

pub fn decode_tolerance_r2007(reader: &mut BitReader<'_>) -> Result<ToleranceEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_tolerance_with_header(reader, header, true, true, ToleranceLayout::StringStream)
}

pub fn decode_tolerance_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ToleranceEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_tolerance_with_header(reader, header, true, true, ToleranceLayout::StringStream)
}

pub fn decode_tolerance_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ToleranceEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_tolerance_with_header(reader, header, true, true, ToleranceLayout::StringStream)
}

fn decode_tolerance_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
    layout: ToleranceLayout,
) -> Result<ToleranceEntity> {
    let (height, dimgap) = if layout == ToleranceLayout::R14 {
        let _unknown = reader.read_bs()?;
        (reader.read_bd()?, reader.read_bd()?)
    } else {
        (0.0, 0.0)
    };
    let insertion = reader.read_3bd()?;
    let x_direction = reader.read_3bd()?;
    let extrusion = reader.read_3bd()?;
    let text = if layout == ToleranceLayout::StringStream {
        let (mut strings, strings_end) = string_stream_reader(reader, header.obj_size)
            .ok_or_else(|| DwgError::new(ErrorKind::Format, "TOLERANCE has no string stream"))?;
        let text = strings.read_tu()?;
        if strings.tell_bits() > u64::from(strings_end) {
            return Err(DwgError::new(
                ErrorKind::Format,
                "TOLERANCE text runs past its string stream",
            ));
        }
        text
    } else {
        reader.read_tv()?
    };

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let (layer_handle, dimstyle_handle) = match if r2007_layer_only {
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

    Ok(ToleranceEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        text,
        insertion,
        x_direction,
        extrusion,
        height,
        dimgap,
        dimstyle_handle,
    })
}
