use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

#[derive(Debug, Clone, Copy)]
pub enum Endian {
    Little,
    Big,
}

#[derive(Debug, Clone, Copy)]
pub struct HandleRef {
    pub code: u8,
    pub counter: u8,
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    pub fn tell_bits(&self) -> u64 {
        (self.byte_pos as u64) * 8 + self.bit_pos as u64
    }

    pub fn get_pos(&self) -> (usize, u8) {
        (self.byte_pos, self.bit_pos)
    }

    pub fn set_pos(&mut self, byte_pos: usize, bit_pos: u8) {
        self.byte_pos = byte_pos;
        self.bit_pos = bit_pos.min(7);
    }

    pub fn set_bit_pos(&mut self, bit_pos: u32) {
        let (byte_pos, bit_pos) = (bit_pos / 8, bit_pos % 8);
        self.set_pos(byte_pos as usize, bit_pos as u8);
    }

    pub fn align_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }

    pub fn read_b(&mut self) -> Result<u8> {
        if self.byte_pos >= self.data.len() {
            return Err(
                DwgError::new(ErrorKind::Io, "unexpected EOF").with_offset(self.byte_pos as u64)
            );
        }
        let byte = self.data[self.byte_pos];
        let bit = (byte & (0x80 >> self.bit_pos)) >> (7 - self.bit_pos);
        self.advance(1);
        Ok(bit)
    }

    pub fn read_bb(&mut self) -> Result<u8> {
        Ok(self.read_bits_msb(2)? as u8)
    }

    pub fn read_3b(&mut self) -> Result<u8> {
        Ok(self.read_bits_msb(3)? as u8)
    }

    pub fn read_bits_msb(&mut self, n: u8) -> Result<u64> {
        if n > 64 {
            return Err(DwgError::new(
                ErrorKind::Decode,
                format!("read_bits supports up to 64 bits, got {n}"),
            ));
        }
        let mut value = 0u64;
        for _ in 0..n {
            value = (value << 1) | (self.read_b()? as u64);
        }
        Ok(value)
    }

    pub fn read_rc(&mut self) -> Result<u8> {
        if self.byte_pos >= self.data.len() {
            return Err(
                DwgError::new(ErrorKind::Io, "unexpected EOF").with_offset(self.byte_pos as u64)
            );
        }

        let mut value = self.data[self.byte_pos] as u16;
        if self.bit_pos != 0 {
            value <<= self.bit_pos;
            if self.byte_pos + 1 < self.data.len() {
                value |= (self.data[self.byte_pos + 1] as u16) >> (8 - self.bit_pos);
            }
        }
        self.advance(8);
        Ok((value & 0xFF) as u8)
    }

    pub fn read_rcs(&mut self, count: usize) -> Result<Vec<u8>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        if self.bit_pos == 0 {
            if self.byte_pos + count > self.data.len() {
                return Err(
                    DwgError::new(ErrorKind::Io, "unexpected EOF while reading raw bytes")
                        .with_offset(self.byte_pos as u64),
                );
            }
            let start = self.byte_pos;
            let end = start + count;
            self.byte_pos = end;
            return Ok(self.data[start..end].to_vec());
        }

        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.read_rc()?);
        }
        Ok(out)
    }

    pub fn read_rs(&mut self, endian: Endian) -> Result<u16> {
        let byte1 = self.read_rc()? as u16;
        let byte2 = self.read_rc()? as u16;
        let value = match endian {
            Endian::Little => (byte2 << 8) | byte1,
            Endian::Big => (byte1 << 8) | byte2,
        };
        Ok(value)
    }

    pub fn read_rl(&mut self, endian: Endian) -> Result<u32> {
        let short1 = self.read_rs(endian)? as u32;
        let short2 = self.read_rs(endian)? as u32;
        let value = match endian {
            Endian::Little => (short2 << 16) | short1,
            Endian::Big => (short1 << 16) | short2,
        };
        Ok(value)
    }

    pub fn read_rd(&mut self, endian: Endian) -> Result<f64> {
        let mut data = [0u8; 8];
        for byte in &mut data {
            *byte = self.read_rc()?;
        }
        let value = match endian {
            Endian::Little => f64::from_le_bytes(data),
            Endian::Big => f64::from_be_bytes(data),
        };
        Ok(value)
    }

    pub fn read_bd(&mut self) -> Result<f64> {
        let what_it_is = self.read_bb()?;
        let value = match what_it_is {
            0x00 => self.read_rd(Endian::Little)?,
            0x01 => 1.0,
            0x02 => 0.0,
            0x03 => 0.0,
            _ => 0.0,
        };
        Ok(value)
    }

    pub fn read_3bd(&mut self) -> Result<(f64, f64, f64)> {
        Ok((self.read_bd()?, self.read_bd()?, self.read_bd()?))
    }

    pub fn read_dd(&mut self, default_value: f64) -> Result<f64> {
        let what_it_is = self.read_bb()?;
        let value = match what_it_is {
            0 => default_value,
            1 => {
                let mut data = default_value.to_le_bytes();
                data[0] = self.read_rc()?;
                data[1] = self.read_rc()?;
                data[2] = self.read_rc()?;
                data[3] = self.read_rc()?;
                f64::from_le_bytes(data)
            }
            2 => {
                let mut data = default_value.to_le_bytes();
                data[4] = self.read_rc()?;
                data[5] = self.read_rc()?;
                data[0] = self.read_rc()?;
                data[1] = self.read_rc()?;
                data[2] = self.read_rc()?;
                data[3] = self.read_rc()?;
                f64::from_le_bytes(data)
            }
            3 => self.read_rd(Endian::Little)?,
            _ => default_value,
        };
        Ok(value)
    }

    pub fn read_bt(&mut self) -> Result<f64> {
        let what_it_is = self.read_b()?;
        if what_it_is == 1 {
            Ok(0.0)
        } else {
            self.read_bd()
        }
    }

    pub fn read_be(&mut self) -> Result<(f64, f64, f64)> {
        let what_it_is = self.read_b()?;
        if what_it_is == 1 {
            Ok((0.0, 0.0, 1.0))
        } else {
            Ok((self.read_bd()?, self.read_bd()?, self.read_bd()?))
        }
    }

    pub fn read_bs(&mut self) -> Result<u16> {
        let what_it_is = self.read_bb()?;
        let value = match what_it_is {
            0x00 => self.read_rs(Endian::Little)?,
            0x01 => self.read_rc()? as u16,
            0x02 => 0,
            0x03 => 256,
            _ => 0,
        };
        Ok(value)
    }

    pub fn read_bl(&mut self) -> Result<u32> {
        let what_it_is = self.read_bb()?;
        let value = match what_it_is {
            0x00 => self.read_rl(Endian::Little)?,
            0x01 => self.read_rc()? as u32,
            0x02 => 0,
            0x03 => 0,
            _ => 0,
        };
        Ok(value)
    }

    pub fn read_bll(&mut self) -> Result<u64> {
        let length = self.read_3b()? as usize;
        let mut value = 0u64;
        for _ in 0..length {
            value = (value << 8) | self.read_rc()? as u64;
        }
        Ok(value)
    }

    pub fn read_ms(&mut self) -> Result<u32> {
        let mut value: u32 = 0;
        let mut shift = 0;

        for _ in 0..2 {
            let mut word = self.read_rs(Endian::Little)?;
            if (word & 0x8000) == 0 {
                value |= (word as u32) << shift;
                return Ok(value);
            }
            word &= 0x7FFF;
            value |= (word as u32) << shift;
            shift += 15;
        }

        Ok(value)
    }

    pub fn read_mc(&mut self) -> Result<i64> {
        let mut value: i64 = 0;
        let mut shift = 0;

        for _ in 0..4 {
            let mut byte = self.read_rc()?;
            if (byte & 0x80) == 0 {
                let negative = (byte & 0x40) != 0;
                if negative {
                    byte &= 0xBF;
                }
                value |= (byte as i64) << shift;
                if negative {
                    value = -value;
                }
                return Ok(value);
            }
            byte &= 0x7F;
            value |= (byte as i64) << shift;
            shift += 7;
        }

        Ok(value)
    }

    pub fn read_umc(&mut self) -> Result<u32> {
        let mut value: u32 = 0;
        let mut shift = 0u32;

        for _ in 0..5 {
            let byte = self.read_rc()?;
            let chunk = (byte & 0x7F) as u32;
            value |= chunk << shift;
            if (byte & 0x80) == 0 {
                return Ok(value);
            }
            shift += 7;
        }

        Ok(value)
    }

    pub fn read_ot_r2010(&mut self) -> Result<u16> {
        let opcode = self.read_bb()?;
        let type_code = match opcode {
            0 => self.read_rc()? as u16,
            1 => self.read_rc()? as u16 + 0x01F0,
            2 | 3 => self.read_rs(Endian::Little)?,
            _ => 0,
        };
        Ok(type_code)
    }

    pub fn read_h(&mut self) -> Result<HandleRef> {
        let mut code = self.read_rc()?;
        let counter = code & 0x0F;
        code = (code & 0xF0) >> 4;
        if counter > 4 {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!("invalid handle counter {counter}"),
            )
            .with_offset(self.byte_pos as u64));
        }
        let mut value: u64 = 0;
        if counter > 0 {
            for idx in (0..counter).rev() {
                let byte = self.read_rc()? as u64;
                value |= byte << (idx * 8);
            }
        }
        Ok(HandleRef {
            code,
            counter,
            value,
        })
    }

    pub fn read_tv(&mut self) -> Result<String> {
        let length = self.read_bs()? as usize;
        let mut text = Vec::with_capacity(length);
        for _ in 0..length {
            let mut ch = self.read_rc()?;
            if ch == 0x00 {
                continue;
            }
            if ch >= 0x7F {
                ch = 0x2A;
            }
            text.push(ch);
        }
        Ok(String::from_utf8_lossy(&text).to_string())
    }

    pub fn read_crc(&mut self) -> Result<u16> {
        if self.bit_pos > 0 {
            self.set_pos(self.byte_pos + 1, 0);
        }
        self.read_rs(Endian::Little)
    }

    fn advance(&mut self, bits: u8) {
        let pos_end = self.bit_pos as u16 + bits as u16;
        self.byte_pos += (pos_end / 8) as usize;
        self.bit_pos = (pos_end % 8) as u8;
    }
}
