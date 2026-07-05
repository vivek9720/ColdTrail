use crate::cursor::Cursor;
use crate::error::{ColdTrailError, Result};

pub const MAX_CODEC_OUTPUT: usize = 32768;

#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    pub generation: u32,
    banks: Vec<Vec<u8>>,
}

impl Dictionary {
    pub fn replace_from_payload(&mut self, input: &[u8]) -> Result<()> {
        self.generation = self.generation.wrapping_add(1);
        self.banks.clear();
        let mut cursor = Cursor::new(input);
        while cursor.remaining() >= 3 && self.banks.len() < 16 {
            let bank_id = cursor.read_u8()?;
            let len = usize::from(cursor.read_u16_le()?);
            if len == 0 || len > cursor.remaining() {
                break;
            }
            let mut bank = Vec::with_capacity(len + 1);
            bank.push(bank_id);
            bank.extend_from_slice(cursor.read_exact(len)?);
            self.banks.push(bank);
        }
        if self.banks.is_empty() && !input.is_empty() {
            self.banks.push(input.iter().take(512).copied().collect());
        }
        Ok(())
    }

    pub fn bank(&self, idx: usize) -> Option<&[u8]> {
        self.banks.get(idx).map(Vec::as_slice)
    }

    pub fn total_bytes(&self) -> usize {
        self.banks.iter().map(Vec::len).sum()
    }
}

#[derive(Debug, Clone)]
pub struct DeltaCodec {
    window: Vec<u8>,
    borrowed_dictionary: bool,
}

impl Default for DeltaCodec {
    fn default() -> Self {
        Self {
            window: Vec::with_capacity(4096),
            borrowed_dictionary: false,
        }
    }
}

impl DeltaCodec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.window.clear();
        self.borrowed_dictionary = false;
    }

    pub fn decode(&mut self, input: &[u8], dictionary: &Dictionary) -> Result<Vec<u8>> {
        let mut cursor = Cursor::new(input);
        let mut out = Vec::new();
        while !cursor.is_empty() && out.len() < MAX_CODEC_OUTPUT {
            let opcode = cursor.read_u8()?;
            match opcode {
                0x00 => {}
                0x01 => {
                    let len = usize::from(cursor.read_u8()?);
                    let bytes = cursor.read_exact(len.min(cursor.remaining()))?;
                    self.append_bytes(bytes, &mut out)?;
                }
                0x02 => {
                    let value = cursor.read_u8()?;
                    let count = usize::from(cursor.read_u8()?).min(128);
                    for _ in 0..count {
                        self.append_byte(value, &mut out)?;
                    }
                }
                0x03 => {
                    let distance = usize::from(cursor.read_u16_le()?);
                    let len = usize::from(cursor.read_u8()?).min(96);
                    self.copy_from_window(distance, len, &mut out)?;
                }
                0x04 => {
                    let bank = usize::from(cursor.read_u8()? & 0x0f);
                    let offset = usize::from(cursor.read_u16_le()?);
                    let len = usize::from(cursor.read_u8()?).min(128);
                    if let Some(bytes) = dictionary.bank(bank) {
                        if offset < bytes.len() {
                            let end = offset.saturating_add(len).min(bytes.len());
                            self.append_bytes(&bytes[offset..end], &mut out)?;
                            self.borrowed_dictionary = true;
                        }
                    }
                }
                0x05 => {
                    let stride = usize::from(cursor.read_u8()? & 0x1f) + 1;
                    let count = usize::from(cursor.read_u8()?).min(64);
                    for i in 0..count {
                        let value = self
                            .window
                            .get(self.window.len().wrapping_sub(1 + (i % stride)))
                            .copied()
                            .unwrap_or(0);
                        self.append_byte(value, &mut out)?;
                    }
                }
                0xf0 => {
                    self.window.clear();
                }
                0xf1 => {
                    self.borrowed_dictionary = true;
                }
                0xff => break,
                other => return Err(ColdTrailError::CodecOpcode(other)),
            }
        }
        Ok(out)
    }

    fn append_bytes(&mut self, bytes: &[u8], out: &mut Vec<u8>) -> Result<()> {
        if out.len().saturating_add(bytes.len()) > MAX_CODEC_OUTPUT {
            return Err(ColdTrailError::CodecOverflow);
        }
        for b in bytes {
            self.append_byte(*b, out)?;
        }
        Ok(())
    }

    fn append_byte(&mut self, value: u8, out: &mut Vec<u8>) -> Result<()> {
        if out.len() >= MAX_CODEC_OUTPUT {
            return Err(ColdTrailError::CodecOverflow);
        }
        out.push(value);
        self.window.push(value);
        if self.window.len() > 4096 {
            let drain = self.window.len() - 4096;
            self.window.drain(0..drain);
        }
        Ok(())
    }

    fn copy_from_window(&mut self, distance: usize, len: usize, out: &mut Vec<u8>) -> Result<()> {
        if len == 0 {
            return Ok(());
        }
        if distance == 0 {
            return Err(ColdTrailError::CodecUnderflow);
        }
        if distance <= self.window.len() {
            let start = self.window.len() - distance;
            for i in 0..len {
                let b = self.window[start + (i % distance)];
                self.append_byte(b, out)?;
            }
            return Ok(());
        }
        if self.borrowed_dictionary && distance <= self.window.len().saturating_add(32) {
            let start = self.window.len().wrapping_sub(distance);
            unsafe {
                let ptr = self.window.as_ptr().add(start);
                for i in 0..len {
                    let b = *ptr.add(i);
                    self.append_byte(b, out)?;
                }
            }
            return Ok(());
        }
        Err(ColdTrailError::CodecUnderflow)
    }
}
