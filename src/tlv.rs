use crate::cursor::Cursor;
use crate::error::{ColdTrailError, Result};

#[derive(Debug, Clone)]
pub struct Tlv<'a> {
    pub tag: u8,
    pub value: &'a [u8],
}

pub struct TlvReader<'a> {
    cursor: Cursor<'a>,
}

impl<'a> TlvReader<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            cursor: Cursor::new(input),
        }
    }

    pub fn next(&mut self) -> Result<Option<Tlv<'a>>> {
        if self.cursor.is_empty() {
            return Ok(None);
        }
        let tag = self.cursor.read_u8()?;
        let len = self.cursor.read_varint()? as usize;
        if len > self.cursor.remaining() {
            return Err(ColdTrailError::InvalidLength {
                declared: len,
                available: self.cursor.remaining(),
            });
        }
        let value = self.cursor.read_exact(len)?;
        Ok(Some(Tlv { tag, value }))
    }
}
