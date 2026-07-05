use crate::checksum;
use crate::cursor::Cursor;
use crate::error::{ColdTrailError, Result};

pub const SYNC: u8 = 0xc7;
pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 10;
pub const MAX_FRAME_PAYLOAD: usize = 8192;

pub const FLAG_CHECKSUM: u8 = 0x01;
pub const FLAG_COMPRESSED: u8 = 0x02;
pub const FLAG_FRAGMENT_LAST: u8 = 0x04;
pub const FLAG_REPLAY: u8 = 0x08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    Hello,
    Manifest,
    Telemetry,
    Fragment,
    Script,
    Dictionary,
    ReplaySegment,
    Ack,
    Unknown(u8),
}

impl FrameKind {
    pub fn from_byte(value: u8) -> Self {
        match value {
            0x01 => FrameKind::Hello,
            0x02 => FrameKind::Manifest,
            0x03 => FrameKind::Telemetry,
            0x04 => FrameKind::Fragment,
            0x05 => FrameKind::Script,
            0x06 => FrameKind::Dictionary,
            0x07 => FrameKind::ReplaySegment,
            0x08 => FrameKind::Ack,
            other => FrameKind::Unknown(other),
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            FrameKind::Hello => 0x01,
            FrameKind::Manifest => 0x02,
            FrameKind::Telemetry => 0x03,
            FrameKind::Fragment => 0x04,
            FrameKind::Script => 0x05,
            FrameKind::Dictionary => 0x06,
            FrameKind::ReplaySegment => 0x07,
            FrameKind::Ack => 0x08,
            FrameKind::Unknown(v) => v,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub kind: FrameKind,
    pub flags: u8,
    pub channel: u8,
    pub session_id: u16,
    pub sequence: u16,
    pub payload: Vec<u8>,
    pub offset: usize,
}

impl Frame {
    pub fn has_flag(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

pub fn parse_frames(input: &[u8]) -> Result<Vec<Frame>> {
    let mut cursor = Cursor::new(input);
    let mut frames = Vec::new();
    while cursor.remaining() >= HEADER_LEN {
        if cursor.peek_u8()? != SYNC {
            cursor.advance(1)?;
            continue;
        }
        let offset = cursor.position();
        cursor.advance(1)?;
        let version_flags = cursor.read_u8()?;
        let version = version_flags & 0x0f;
        let flags = version_flags >> 4;
        if version != VERSION {
            continue;
        }
        let kind_byte = cursor.read_u8()?;
        let kind = FrameKind::from_byte(kind_byte);
        let channel = cursor.read_u8()?;
        let session_id = cursor.read_u16_le()?;
        let sequence = cursor.read_u16_le()?;
        let payload_len = usize::from(cursor.read_u16_le()?);
        if payload_len > MAX_FRAME_PAYLOAD {
            return Err(ColdTrailError::OversizedFrame(payload_len));
        }
        let tail = if flags & FLAG_CHECKSUM != 0 { 2 } else { 0 };
        if cursor.remaining() < payload_len + tail {
            break;
        }
        let payload = cursor.read_vec(payload_len)?;
        if flags & FLAG_CHECKSUM != 0 {
            let expected = cursor.read_u16_le()?;
            let actual = checksum::frame_checksum(kind_byte, session_id, sequence, &payload);
            if expected != actual {
                return Err(ColdTrailError::InvalidChecksum { expected, actual });
            }
        }
        frames.push(Frame {
            kind,
            flags,
            channel,
            session_id,
            sequence,
            payload,
            offset,
        });
    }
    Ok(frames)
}

pub fn encode_lab_frame(kind: FrameKind, flags: u8, session: u16, seq: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len() + 2);
    out.push(SYNC);
    out.push(((flags & 0x0f) << 4) | VERSION);
    out.push(kind.to_byte());
    out.push(0);
    out.extend_from_slice(&session.to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(payload);
    if flags & FLAG_CHECKSUM != 0 {
        let crc = checksum::frame_checksum(kind.to_byte(), session, seq, payload);
        out.extend_from_slice(&crc.to_le_bytes());
    }
    out
}
