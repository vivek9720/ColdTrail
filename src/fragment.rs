use crate::cursor::Cursor;
use crate::error::{ColdTrailError, Result};
use crate::frame::FrameKind;

pub const FRAG_ALIAS_HINT: u8 = 0x40;
pub const FRAG_CONTROL_PAYLOAD: u8 = 0x20;

#[derive(Debug, Clone)]
pub struct FragmentBlock {
    pub message_id: u32,
    pub total_len: usize,
    pub index: usize,
    pub count: usize,
    pub flags: u8,
    pub logical_kind: FrameKind,
    pub data: Vec<u8>,
}

pub fn parse_fragment(input: &[u8]) -> Result<FragmentBlock> {
    if input.len() < 10 {
        return Err(ColdTrailError::UnexpectedEof);
    }
    let mut c = Cursor::new(input);
    let message_id = c.read_u32_le()?;
    let total_len = usize::from(c.read_u16_le()?);
    let index = usize::from(c.read_u8()?);
    let count = usize::from(c.read_u8()?);
    let logical_kind = FrameKind::from_byte(c.read_u8()?);
    let flags = c.read_u8()?;
    if count == 0 || index >= count {
        return Err(ColdTrailError::FragmentIndex);
    }
    if total_len > 65535 {
        return Err(ColdTrailError::FragmentTooLarge);
    }
    let data = c.read_vec(c.remaining())?;
    Ok(FragmentBlock {
        message_id,
        total_len,
        index,
        count,
        flags,
        logical_kind,
        data,
    })
}
