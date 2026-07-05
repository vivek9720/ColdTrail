use crate::error::{ColdTrailError, Result};

#[derive(Debug, Clone)]
pub struct Cursor<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.pos)
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn advance(&mut self, n: usize) -> Result<()> {
        if self.remaining() < n {
            return Err(ColdTrailError::UnexpectedEof);
        }
        self.pos += n;
        Ok(())
    }

    pub fn peek_u8(&self) -> Result<u8> {
        self.input
            .get(self.pos)
            .copied()
            .ok_or(ColdTrailError::UnexpectedEof)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        let b = self.peek_u8()?;
        self.pos += 1;
        Ok(b)
    }

    pub fn read_i8(&mut self) -> Result<i8> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_u16_le(&mut self) -> Result<u16> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_i16_le(&mut self) -> Result<i16> {
        Ok(self.read_u16_le()? as i16)
    }

    pub fn read_u32_le(&mut self) -> Result<u32> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_i32_le(&mut self) -> Result<i32> {
        Ok(self.read_u32_le()? as i32)
    }

    pub fn read_u64_le(&mut self) -> Result<u64> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub fn read_varint(&mut self) -> Result<u64> {
        let mut shift = 0u32;
        let mut value = 0u64;
        for _ in 0..10 {
            let b = self.read_u8()?;
            value |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
        Err(ColdTrailError::InvalidVarint)
    }

    pub fn read_exact(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(ColdTrailError::UnexpectedEof);
        }
        let start = self.pos;
        self.pos += n;
        Ok(&self.input[start..start + n])
    }

    pub fn read_vec(&mut self, n: usize) -> Result<Vec<u8>> {
        Ok(self.read_exact(n)?.to_vec())
    }

    pub fn read_string(&mut self, n: usize) -> Result<String> {
        let bytes = self.read_exact(n)?;
        core::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|_| ColdTrailError::InvalidUtf8)
    }

    pub fn split(&mut self, n: usize) -> Result<Cursor<'a>> {
        let bytes = self.read_exact(n)?;
        Ok(Cursor::new(bytes))
    }
}
