use super::checksum::crc16;
use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;

const SENTINEL_CLASSES_BEFORE: [u8; 16] = [
    0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6, 0x8A,
];
const SENTINEL_CLASSES_AFTER: [u8; 16] = [
    0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49, 0x75,
];

/// Class numbers of the raster image classes (present only when images are written).
pub const IMAGE_CLASS: u16 = 503;
pub const IMAGEDEF_CLASS: u16 = 504;
pub const IMAGEDEF_REACTOR_CLASS: u16 = 505;
pub const RASTERVARIABLES_CLASS: u16 = 506;

pub fn encode_minimal_classes_section() -> Result<Vec<u8>> {
    encode_classes_section(false)
}

/// The three classes of every writer output, plus the raster image classes
/// (`IMAGE`, `IMAGEDEF`, `IMAGEDEF_REACTOR`, `RASTERVARIABLES`) when requested.
pub fn encode_classes_section(with_images: bool) -> Result<Vec<u8>> {
    let mut data = BitWriter::new();
    let mut classes: Vec<(u16, u16, &str, &str, u16)> = vec![
        (
            500,
            0,
            "AcDbDictionaryWithDefault",
            "ACDBDICTIONARYWDFLT",
            0x1F3,
        ),
        (501, 0, "AcDbPlaceHolder", "ACDBPLACEHOLDER", 0x1F3),
        (502, 0, "AcDbLayout", "LAYOUT", 0x1F3),
    ];
    if with_images {
        classes.extend([
            (IMAGE_CLASS, 127, "AcDbRasterImage", "IMAGE", 0x1F2),
            (IMAGEDEF_CLASS, 0, "AcDbRasterImageDef", "IMAGEDEF", 0x1F3),
            (
                IMAGEDEF_REACTOR_CLASS,
                1,
                "AcDbRasterImageDefReactor",
                "IMAGEDEF_REACTOR",
                0x1F3,
            ),
            (
                RASTERVARIABLES_CLASS,
                0,
                "AcDbRasterVariables",
                "RASTERVARIABLES",
                0x1F3,
            ),
        ]);
    }
    for (number, proxy_flags, cpp, dxf, item_class) in classes {
        data.write_bs(number)?;
        data.write_bs(proxy_flags)?;
        data.write_tv("ObjectDBX Classes")?;
        data.write_tv(cpp)?;
        data.write_tv(dxf)?;
        data.write_b(0)?;
        data.write_bs(item_class)?;
    }
    let data = data.into_bytes();
    let mut writer = BitWriter::new();
    writer.write_rcs(&SENTINEL_CLASSES_BEFORE)?;
    writer.write_rl(Endian::Little, data.len() as u32)?;
    writer.write_rcs(&data)?;
    let crc = crc16(0xC0C1, &writer.to_bytes()[16..]);
    writer.write_crc(crc)?;
    writer.write_rcs(&SENTINEL_CLASSES_AFTER)?;
    Ok(writer.into_bytes())
}
