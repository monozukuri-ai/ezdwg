//! R13/R2000 CRC-16 (reflected polynomial 0xA001).
pub fn crc16(mut seed: u16, bytes: &[u8]) -> u16 {
    for byte in bytes {
        seed ^= u16::from(*byte);
        for _ in 0..8 {
            seed = (seed >> 1) ^ if seed & 1 != 0 { 0xA001 } else { 0 };
        }
    }
    seed
}

#[cfg(test)]
mod tests {
    use super::crc16;

    #[test]
    fn standard_crc16_arc_check_value() {
        assert_eq!(crc16(0, b"123456789"), 0xBB3D);
    }
}
