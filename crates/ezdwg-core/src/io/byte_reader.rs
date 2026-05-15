use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;

#[derive(Debug, Clone)]
pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    pub fn tell(&self) -> u64 {
        self.pos as u64
    }

    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.data.len() {
            return Err(DwgError::new(
                ErrorKind::Io,
                format!("seek out of range: {pos} > {}", self.data.len()),
            ));
        }
        self.pos = pos;
        Ok(())
    }

    pub fn skip(&mut self, n: usize) -> Result<()> {
        let new_pos = self.pos.saturating_add(n);
        self.seek(new_pos)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        self.require(1)?;
        let value = self.data[self.pos];
        self.pos += 1;
        Ok(value)
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_u16_le(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u32_le(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_u64_le(&mut self) -> Result<u64> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub fn read_i32_le(&mut self) -> Result<i32> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_f64_le(&mut self) -> Result<f64> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.require(n)?;
        let start = self.pos;
        let end = self.pos + n;
        self.pos = end;
        Ok(&self.data[start..end])
    }

    pub fn peek_bytes(&self, n: usize) -> Result<&'a [u8]> {
        self.require_peek(n)?;
        let start = self.pos;
        let end = self.pos + n;
        Ok(&self.data[start..end])
    }

    fn require(&self, n: usize) -> Result<()> {
        if self.pos + n <= self.data.len() {
            Ok(())
        } else {
            Err(DwgError::new(
                ErrorKind::Io,
                format!("unexpected EOF: need {n} bytes, have {}", self.remaining()),
            )
            .with_offset(self.pos as u64))
        }
    }

    fn require_peek(&self, n: usize) -> Result<()> {
        if self.pos + n <= self.data.len() {
            Ok(())
        } else {
            Err(DwgError::new(
                ErrorKind::Io,
                format!("unexpected EOF: need {n} bytes, have {}", self.remaining()),
            )
            .with_offset(self.pos as u64))
        }
    }
}
