use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;

const SENTINEL_CLASSES_BEFORE: [u8; 16] = [
    0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6, 0x8A,
];
const SENTINEL_CLASSES_AFTER: [u8; 16] = [
    0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49, 0x75,
];

pub fn encode_minimal_classes_section() -> Result<Vec<u8>> {
    let mut writer = BitWriter::new();
    writer.write_rcs(&SENTINEL_CLASSES_BEFORE)?;
    writer.write_rl(Endian::Little, 0)?; // class data size bytes
    writer.write_crc_zero()?;
    writer.write_rcs(&SENTINEL_CLASSES_AFTER)?;
    Ok(writer.into_bytes())
}
