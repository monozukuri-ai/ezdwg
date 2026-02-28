use crate::bit::{BitReader, Endian};
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct MInsertEntity {
    pub handle: u64,
    pub position: (f64, f64, f64),
    pub scale: (f64, f64, f64),
    pub rotation: f64,
    pub num_columns: u16,
    pub num_rows: u16,
    pub column_spacing: f64,
    pub row_spacing: f64,
    pub block_header_handle: Option<u64>,
}

pub fn decode_minsert(reader: &mut BitReader<'_>) -> Result<MInsertEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_minsert_with_header(reader, header, false, false)
}

pub fn decode_minsert_r2007(reader: &mut BitReader<'_>) -> Result<MInsertEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_minsert_with_header(reader, header, true, false)
}

pub fn decode_minsert_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MInsertEntity> {
    let header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    let mut entity = decode_minsert_with_header(reader, header, true, false)?;
    entity.handle = object_handle;
    Ok(entity)
}

pub fn decode_minsert_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<MInsertEntity> {
    let header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    let mut entity = decode_minsert_with_header(reader, header, true, false)?;
    entity.handle = object_handle;
    Ok(entity)
}

fn decode_minsert_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<MInsertEntity> {
    let position = reader.read_3bd()?;
    let data_flags = reader.read_bb()?;

    let (x_scale, y_scale, z_scale) = match data_flags {
        0x03 => (1.0, 1.0, 1.0),
        0x01 => {
            let y = reader.read_dd(1.0)?;
            let z = reader.read_dd(1.0)?;
            (1.0, y, z)
        }
        0x02 => {
            let x = reader.read_rd(Endian::Little)?;
            (x, x, x)
        }
        _ => {
            let x = reader.read_rd(Endian::Little)?;
            let y = reader.read_dd(x)?;
            let z = reader.read_dd(x)?;
            (x, y, z)
        }
    };

    let rotation = reader.read_bd()?;
    let _extrusion = reader.read_3bd()?;
    let has_attribs = reader.read_b()?;
    let owned_obj_count = if has_attribs == 1 {
        reader.read_bl()?
    } else {
        0
    };

    let num_columns = reader.read_bs()?;
    let num_rows = reader.read_bs()?;
    let column_spacing = reader.read_bd()?;
    let row_spacing = reader.read_bd()?;

    let mut block_header_handle = None;
    reader.set_bit_pos(header.obj_size);

    let common_ok = if r2007_layer_only {
        parse_common_entity_layer_handle(reader, &header).map(|_| ())
    } else {
        parse_common_entity_handles(reader, &header).map(|_| ())
    };
    if let Err(err) = common_ok {
        if !(allow_handle_decode_failure
            && matches!(
                err.kind,
                ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
            ))
        {
            return Err(err);
        }
    } else {
        if let Ok(block_handle) = read_handle_reference(reader, header.handle) {
            block_header_handle = Some(block_handle);
        }
        if has_attribs == 1 {
            for _ in 0..owned_obj_count {
                if read_handle_reference(reader, header.handle).is_err() {
                    break;
                }
            }
            let _ = read_handle_reference(reader, header.handle);
        }
    }

    Ok(MInsertEntity {
        handle: header.handle,
        position,
        scale: (x_scale, y_scale, z_scale),
        rotation,
        num_columns,
        num_rows,
        column_spacing,
        row_spacing,
        block_header_handle,
    })
}
