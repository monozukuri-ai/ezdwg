use crate::bit::bit_reader::{BitReader, Endian, HandleRef};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

#[derive(Debug, Clone, Default)]
pub struct BitWriter {
    data: Vec<u8>,
    byte_pos: usize,
    bit_pos: u8,
    max_bit_pos: u64,
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            byte_pos: 0,
            bit_pos: 0,
            max_bit_pos: 0,
        }
    }

    pub fn tell_bits(&self) -> u64 {
        self.byte_pos as u64 * 8 + self.bit_pos as u64
    }

    pub fn get_pos(&self) -> (usize, u8) {
        (self.byte_pos, self.bit_pos)
    }

    pub fn set_pos(&mut self, byte_pos: usize, bit_pos: u8) {
        self.byte_pos = byte_pos;
        self.bit_pos = bit_pos.min(7);
    }

    pub fn set_bit_pos(&mut self, bit_pos: u32) {
        self.set_pos((bit_pos / 8) as usize, (bit_pos % 8) as u8);
    }

    pub fn align_byte(&mut self) {
        if self.bit_pos != 0 {
            self.byte_pos += 1;
            self.bit_pos = 0;
        }
    }

    pub fn write_b(&mut self, bit: u8) -> Result<()> {
        if bit > 1 {
            return Err(DwgError::new(
                ErrorKind::Decode,
                format!("invalid bit value: {bit}"),
            ));
        }
        self.ensure_byte(self.byte_pos);
        let mask = 0x80u8 >> self.bit_pos;
        if bit == 1 {
            self.data[self.byte_pos] |= mask;
        } else {
            self.data[self.byte_pos] &= !mask;
        }
        self.advance(1);
        self.max_bit_pos = self.max_bit_pos.max(self.tell_bits());
        Ok(())
    }

    pub fn write_bb(&mut self, value: u8) -> Result<()> {
        self.write_bits_msb((value & 0x03) as u64, 2)
    }

    pub fn write_3b(&mut self, value: u8) -> Result<()> {
        self.write_bits_msb((value & 0x07) as u64, 3)
    }

    pub fn write_bits_msb(&mut self, value: u64, n: u8) -> Result<()> {
        if n > 64 {
            return Err(DwgError::new(
                ErrorKind::Decode,
                format!("write_bits supports up to 64 bits, got {n}"),
            ));
        }
        for shift in (0..n).rev() {
            self.write_b(((value >> shift) & 1) as u8)?;
        }
        Ok(())
    }

    pub fn write_rc(&mut self, value: u8) -> Result<()> {
        self.write_bits_msb(value as u64, 8)
    }

    pub fn write_rcs(&mut self, values: &[u8]) -> Result<()> {
        for value in values {
            self.write_rc(*value)?;
        }
        Ok(())
    }

    pub fn write_rs(&mut self, endian: Endian, value: u16) -> Result<()> {
        match endian {
            Endian::Little => {
                self.write_rc((value & 0x00FF) as u8)?;
                self.write_rc((value >> 8) as u8)?;
            }
            Endian::Big => {
                self.write_rc((value >> 8) as u8)?;
                self.write_rc((value & 0x00FF) as u8)?;
            }
        }
        Ok(())
    }

    pub fn write_rl(&mut self, endian: Endian, value: u32) -> Result<()> {
        match endian {
            Endian::Little => {
                self.write_rs(Endian::Little, (value & 0xFFFF) as u16)?;
                self.write_rs(Endian::Little, (value >> 16) as u16)?;
            }
            Endian::Big => {
                self.write_rs(Endian::Big, (value >> 16) as u16)?;
                self.write_rs(Endian::Big, (value & 0xFFFF) as u16)?;
            }
        }
        Ok(())
    }

    pub fn write_rd(&mut self, endian: Endian, value: f64) -> Result<()> {
        let bytes = match endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        };
        self.write_rcs(&bytes)
    }

    pub fn write_bd(&mut self, value: f64) -> Result<()> {
        if value == 1.0 {
            self.write_bb(0x01)
        } else if value == 0.0 {
            self.write_bb(0x02)
        } else {
            self.write_bb(0x00)?;
            self.write_rd(Endian::Little, value)
        }
    }

    pub fn write_3bd(&mut self, x: f64, y: f64, z: f64) -> Result<()> {
        self.write_bd(x)?;
        self.write_bd(y)?;
        self.write_bd(z)
    }

    pub fn write_dd(&mut self, default_value: f64, value: f64) -> Result<()> {
        if value == default_value {
            self.write_bb(0x00)
        } else {
            self.write_bb(0x03)?;
            self.write_rd(Endian::Little, value)
        }
    }

    pub fn write_bt(&mut self, value: f64) -> Result<()> {
        if value == 0.0 {
            self.write_b(1)
        } else {
            self.write_b(0)?;
            self.write_bd(value)
        }
    }

    pub fn write_be(&mut self, x: f64, y: f64, z: f64) -> Result<()> {
        if x == 0.0 && y == 0.0 && z == 1.0 {
            self.write_b(1)
        } else {
            self.write_b(0)?;
            self.write_bd(x)?;
            self.write_bd(y)?;
            self.write_bd(z)
        }
    }

    pub fn write_bs(&mut self, value: u16) -> Result<()> {
        if value == 0 {
            self.write_bb(0x02)
        } else if value == 256 {
            self.write_bb(0x03)
        } else if value <= 0x00FF {
            self.write_bb(0x01)?;
            self.write_rc(value as u8)
        } else {
            self.write_bb(0x00)?;
            self.write_rs(Endian::Little, value)
        }
    }

    pub fn write_bl(&mut self, value: u32) -> Result<()> {
        if value == 0 {
            self.write_bb(0x02)
        } else if value <= 0x00FF {
            self.write_bb(0x01)?;
            self.write_rc(value as u8)
        } else {
            self.write_bb(0x00)?;
            self.write_rl(Endian::Little, value)
        }
    }

    pub fn write_bll(&mut self, value: u64) -> Result<()> {
        if value == 0 {
            self.write_3b(0)?;
            return Ok(());
        }
        let bytes = value.to_be_bytes();
        let start = bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len());
        let significant = &bytes[start..];
        if significant.len() > 7 {
            return Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("BLL value exceeds 7-byte encoding range: {value}"),
            ));
        }
        self.write_3b(significant.len() as u8)?;
        self.write_rcs(significant)
    }

    pub fn write_ms(&mut self, value: u32) -> Result<()> {
        if value > 0x3FFF_FFFF {
            return Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("MS value exceeds 30-bit range: {value}"),
            ));
        }
        let low = (value & 0x7FFF) as u16;
        let high = ((value >> 15) & 0x7FFF) as u16;
        if high == 0 {
            self.write_rs(Endian::Little, low)
        } else {
            self.write_rs(Endian::Little, low | 0x8000)?;
            self.write_rs(Endian::Little, high)
        }
    }

    pub fn write_mc(&mut self, value: i64) -> Result<()> {
        let bytes = encode_modular_char(value)?;
        self.write_rcs(&bytes)
    }

    pub fn write_umc(&mut self, value: u32) -> Result<()> {
        let mut remaining = value;
        for _ in 0..5 {
            let mut byte = (remaining & 0x7F) as u8;
            remaining >>= 7;
            if remaining != 0 {
                byte |= 0x80;
            }
            self.write_rc(byte)?;
            if remaining == 0 {
                return Ok(());
            }
        }
        Err(DwgError::new(
            ErrorKind::Unsupported,
            format!("UMC value exceeds supported length: {value}"),
        ))
    }

    pub fn write_ot_r2010(&mut self, type_code: u16) -> Result<()> {
        if type_code <= 0x00FF {
            self.write_bb(0)?;
            self.write_rc(type_code as u8)
        } else if (0x01F0..=0x02EF).contains(&type_code) {
            self.write_bb(1)?;
            self.write_rc((type_code - 0x01F0) as u8)
        } else {
            self.write_bb(2)?;
            self.write_rs(Endian::Little, type_code)
        }
    }

    pub fn write_h(&mut self, code: u8, value: u64) -> Result<()> {
        let counter = if value == 0 {
            0
        } else {
            ((64 - value.leading_zeros() as usize) + 7) / 8
        };
        if counter > 4 {
            return Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("handle value exceeds 4-byte payload: {value}"),
            ));
        }

        let header = ((code & 0x0F) << 4) | (counter as u8 & 0x0F);
        self.write_rc(header)?;
        for idx in (0..counter).rev() {
            self.write_rc(((value >> (idx * 8)) & 0xFF) as u8)?;
        }
        Ok(())
    }

    pub fn write_handle_ref(&mut self, handle: HandleRef) -> Result<()> {
        self.write_h(handle.code, handle.value)
    }

    pub fn write_tv(&mut self, text: &str) -> Result<()> {
        let mut bytes = Vec::with_capacity(text.len());
        for ch in text.bytes() {
            let sanitized = if ch == 0x00 {
                b' '
            } else if ch >= 0x7F {
                0x2A
            } else {
                ch
            };
            bytes.push(sanitized);
        }
        if bytes.len() > u16::MAX as usize {
            return Err(DwgError::new(
                ErrorKind::Unsupported,
                format!("TV string too long: {} bytes", bytes.len()),
            ));
        }
        self.write_bs(bytes.len() as u16)?;
        self.write_rcs(&bytes)
    }

    pub fn write_crc(&mut self, crc: u16) -> Result<()> {
        self.align_byte();
        self.write_rs(Endian::Little, crc)
    }

    pub fn write_crc_zero(&mut self) -> Result<()> {
        self.write_crc(0)
    }

    pub fn write_bits_from_bytes(&mut self, bytes: &[u8], bit_len: u64) -> Result<()> {
        if bit_len == 0 {
            return Ok(());
        }
        let available_bits = (bytes.len() as u64).saturating_mul(8);
        if bit_len > available_bits {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!("bit_len {bit_len} exceeds source bit length {available_bits}"),
            ));
        }
        let mut reader = BitReader::new(bytes);
        for _ in 0..bit_len {
            self.write_b(reader.read_b()?)?;
        }
        Ok(())
    }

    pub fn len_bits(&self) -> u64 {
        self.max_bit_pos
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let used = ((self.max_bit_pos + 7) / 8) as usize;
        self.data.into_iter().take(used).collect()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let used = ((self.max_bit_pos + 7) / 8) as usize;
        self.data.iter().take(used).copied().collect()
    }

    fn ensure_byte(&mut self, byte_index: usize) {
        if self.data.len() <= byte_index {
            self.data.resize(byte_index + 1, 0);
        }
    }

    fn advance(&mut self, bits: u8) {
        let pos_end = self.bit_pos as u16 + bits as u16;
        self.byte_pos += (pos_end / 8) as usize;
        self.bit_pos = (pos_end % 8) as u8;
    }
}

fn encode_modular_char(value: i64) -> Result<Vec<u8>> {
    let negative = value < 0;
    let mut remaining = value.unsigned_abs();
    let mut out = Vec::with_capacity(4);

    for _ in 0..4 {
        let chunk = (remaining & 0x7F) as u8;
        remaining >>= 7;
        if remaining == 0 && chunk <= 0x3F {
            let mut final_byte = chunk;
            if negative {
                final_byte |= 0x40;
            }
            out.push(final_byte);
            return Ok(out);
        }
        out.push(chunk | 0x80);
    }

    Err(DwgError::new(
        ErrorKind::Unsupported,
        format!("modular char value out of range: {value}"),
    ))
}

#[cfg(test)]
mod tests {
    use super::BitWriter;
    use crate::bit::{BitReader, Endian};

    #[test]
    fn roundtrip_bit_and_byte_mixed_sequence() {
        let mut writer = BitWriter::new();
        writer.write_b(1).unwrap();
        writer.write_bb(0b10).unwrap();
        writer.write_3b(0b101).unwrap();
        writer.write_rc(0x5A).unwrap();
        writer.write_b(0).unwrap();

        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        assert_eq!(reader.read_b().unwrap(), 1);
        assert_eq!(reader.read_bb().unwrap(), 0b10);
        assert_eq!(reader.read_3b().unwrap(), 0b101);
        assert_eq!(reader.read_rc().unwrap(), 0x5A);
        assert_eq!(reader.read_b().unwrap(), 0);
    }

    #[test]
    fn roundtrip_numeric_codecs() {
        let mut writer = BitWriter::new();
        writer.write_bs(0).unwrap();
        writer.write_bs(256).unwrap();
        writer.write_bs(200).unwrap();
        writer.write_bs(900).unwrap();
        writer.write_bl(0).unwrap();
        writer.write_bl(44).unwrap();
        writer.write_bl(4096).unwrap();
        writer.write_ms(123).unwrap();
        writer.write_ms(40000).unwrap();
        writer.write_umc(0).unwrap();
        writer.write_umc(127).unwrap();
        writer.write_umc(128).unwrap();
        writer.write_umc(0x1FFF_FFFF).unwrap();
        writer.write_mc(0).unwrap();
        writer.write_mc(63).unwrap();
        writer.write_mc(64).unwrap();
        writer.write_mc(-63).unwrap();
        writer.write_mc(-200).unwrap();
        writer.write_bll(0).unwrap();
        writer.write_bll(0xA1B2C3D4).unwrap();

        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        assert_eq!(reader.read_bs().unwrap(), 0);
        assert_eq!(reader.read_bs().unwrap(), 256);
        assert_eq!(reader.read_bs().unwrap(), 200);
        assert_eq!(reader.read_bs().unwrap(), 900);
        assert_eq!(reader.read_bl().unwrap(), 0);
        assert_eq!(reader.read_bl().unwrap(), 44);
        assert_eq!(reader.read_bl().unwrap(), 4096);
        assert_eq!(reader.read_ms().unwrap(), 123);
        assert_eq!(reader.read_ms().unwrap(), 40000);
        assert_eq!(reader.read_umc().unwrap(), 0);
        assert_eq!(reader.read_umc().unwrap(), 127);
        assert_eq!(reader.read_umc().unwrap(), 128);
        assert_eq!(reader.read_umc().unwrap(), 0x1FFF_FFFF);
        assert_eq!(reader.read_mc().unwrap(), 0);
        assert_eq!(reader.read_mc().unwrap(), 63);
        assert_eq!(reader.read_mc().unwrap(), 64);
        assert_eq!(reader.read_mc().unwrap(), -63);
        assert_eq!(reader.read_mc().unwrap(), -200);
        assert_eq!(reader.read_bll().unwrap(), 0);
        assert_eq!(reader.read_bll().unwrap(), 0xA1B2C3D4);
    }

    #[test]
    fn roundtrip_handle_and_text() {
        let mut writer = BitWriter::new();
        writer.write_h(0x0A, 0x1234).unwrap();
        writer.write_h(0x02, 0).unwrap();
        writer.write_tv("LAYER0").unwrap();

        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        let h1 = reader.read_h().unwrap();
        assert_eq!(h1.code, 0x0A);
        assert_eq!(h1.counter, 2);
        assert_eq!(h1.value, 0x1234);
        let h2 = reader.read_h().unwrap();
        assert_eq!(h2.code, 0x02);
        assert_eq!(h2.counter, 0);
        assert_eq!(h2.value, 0);
        assert_eq!(reader.read_tv().unwrap(), "LAYER0");
    }

    #[test]
    fn roundtrip_crc_writer() {
        let mut writer = BitWriter::new();
        writer.write_rc(0xAB).unwrap();
        writer.write_crc(0x1234).unwrap();
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        assert_eq!(reader.read_rc().unwrap(), 0xAB);
        assert_eq!(reader.read_crc().unwrap(), 0x1234);
    }

    #[test]
    fn roundtrip_float_family() {
        let mut writer = BitWriter::new();
        writer.write_bd(1.0).unwrap();
        writer.write_bd(0.0).unwrap();
        writer.write_bd(12.25).unwrap();
        writer.write_bt(0.0).unwrap();
        writer.write_bt(7.5).unwrap();
        writer.write_be(0.0, 0.0, 1.0).unwrap();
        writer.write_be(1.0, 2.0, 3.0).unwrap();
        writer.write_dd(4.0, 4.0).unwrap();
        writer.write_dd(4.0, 6.5).unwrap();
        writer.write_rd(Endian::Little, 9.25).unwrap();

        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        assert_eq!(reader.read_bd().unwrap(), 1.0);
        assert_eq!(reader.read_bd().unwrap(), 0.0);
        assert_eq!(reader.read_bd().unwrap(), 12.25);
        assert_eq!(reader.read_bt().unwrap(), 0.0);
        assert_eq!(reader.read_bt().unwrap(), 7.5);
        assert_eq!(reader.read_be().unwrap(), (0.0, 0.0, 1.0));
        assert_eq!(reader.read_be().unwrap(), (1.0, 2.0, 3.0));
        assert_eq!(reader.read_dd(4.0).unwrap(), 4.0);
        assert_eq!(reader.read_dd(4.0).unwrap(), 6.5);
        assert_eq!(reader.read_rd(Endian::Little).unwrap(), 9.25);
    }
}
