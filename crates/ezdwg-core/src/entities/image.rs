//! IMAGE (`AcDbRasterImage`, ODA 20.4.80): a class-based raster reference.
use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct ImageEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub class_version: u32,
    pub insert: (f64, f64, f64),
    pub u_vector: (f64, f64, f64),
    pub v_vector: (f64, f64, f64),
    pub size: (f64, f64),
    pub display_props: u16,
    pub clipping: bool,
    pub brightness: u8,
    pub contrast: u8,
    pub fade: u8,
    /// R2010+ only: 0 outside, 1 inside.
    pub clip_mode: Option<u8>,
    pub clip_type: u16,
    pub clip_vertices: Vec<(f64, f64)>,
    pub image_def_handle: u64,
    pub reactor_handle: u64,
}

pub fn decode_image(reader: &mut BitReader<'_>) -> Result<ImageEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_image_with_header(reader, header, false, false, false)
}

pub fn decode_image_r2007(reader: &mut BitReader<'_>) -> Result<ImageEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_image_with_header(reader, header, true, true, false)
}

pub fn decode_image_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ImageEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_image_with_header(reader, header, true, true, true)
}

pub fn decode_image_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<ImageEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_image_with_header(reader, header, true, true, true)
}

fn decode_image_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
    has_clip_mode: bool,
) -> Result<ImageEntity> {
    let class_version = reader.read_bl()?;
    let insert = reader.read_3bd()?;
    let u_vector = reader.read_3bd()?;
    let v_vector = reader.read_3bd()?;
    let size = (
        reader.read_rd(Endian::Little)?,
        reader.read_rd(Endian::Little)?,
    );
    let display_props = reader.read_bs()?;
    let clipping = reader.read_b()? != 0;
    let brightness = reader.read_rc()?;
    let contrast = reader.read_rc()?;
    let fade = reader.read_rc()?;
    let clip_mode = if has_clip_mode {
        Some(reader.read_b()?)
    } else {
        None
    };
    let clip_type = reader.read_bs()?;
    let count = if clip_type == 1 {
        2
    } else {
        let n = reader.read_bl()?;
        if n > 1_000_000 {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!("IMAGE clip boundary vertex count is implausible: {n}"),
            ));
        }
        n as usize
    };
    let mut clip_vertices = Vec::with_capacity(count);
    for _ in 0..count {
        let x = reader.read_rd(Endian::Little)?;
        let y = reader.read_rd(Endian::Little)?;
        clip_vertices.push((x, y));
    }

    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let (layer_handle, image_def_handle, reactor_handle) = match (|| -> Result<_> {
        let layer = if r2007_layer_only {
            parse_common_entity_layer_handle(reader, &header)?
        } else {
            parse_common_entity_handles(reader, &header)?.layer
        };
        let def = read_handle_reference(reader, header.handle)?;
        let reactor = read_handle_reference(reader, header.handle)?;
        Ok((layer, def, reactor))
    })() {
        Ok(v) => v,
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            reader.set_pos(handles_pos.0, handles_pos.1);
            let layer = parse_common_entity_layer_handle(reader, &header).unwrap_or(0);
            (layer, 0, 0)
        }
        Err(err) => return Err(err),
    };

    Ok(ImageEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        class_version,
        insert,
        u_vector,
        v_vector,
        size,
        display_props,
        clipping,
        brightness,
        contrast,
        fade,
        clip_mode,
        clip_type,
        clip_vertices,
        image_def_handle,
        reactor_handle,
    })
}
