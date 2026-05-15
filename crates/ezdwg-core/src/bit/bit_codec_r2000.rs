use crate::bit::bit_reader::BitReader;
use crate::core::error::DwgError;
use crate::core::result::Result;

pub fn read_modular_int(_reader: &mut BitReader<'_>) -> Result<i64> {
    Err(DwgError::not_implemented(
        "bit_codec_r2000::read_modular_int",
    ))
}

pub fn read_handle_ref(_reader: &mut BitReader<'_>) -> Result<(u8, u64)> {
    Err(DwgError::not_implemented(
        "bit_codec_r2000::read_handle_ref",
    ))
}
