use core::fmt;

pub type Result<T> = core::result::Result<T, ColdTrailError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColdTrailError {
    UnexpectedEof,
    InvalidMagic,
    InvalidVersion(u8),
    InvalidFrameKind(u8),
    InvalidLength { declared: usize, available: usize },
    InvalidChecksum { expected: u16, actual: u16 },
    InvalidVarint,
    InvalidUtf8,
    OversizedFrame(usize),
    OversizedMessage(usize),
    UnknownTlv(u8),
    MissingManifest,
    FragmentIndex,
    FragmentConflict,
    FragmentTooLarge,
    CodecUnderflow,
    CodecOverflow,
    CodecOpcode(u8),
    ScriptOpcode(u8),
    ScriptStack,
    ScriptBudget,
    PolicyViolation(&'static str),
}

impl fmt::Display for ColdTrailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ColdTrailError::UnexpectedEof => write!(f, "unexpected end of input"),
            ColdTrailError::InvalidMagic => write!(f, "invalid frame magic"),
            ColdTrailError::InvalidVersion(v) => write!(f, "invalid protocol version {v}"),
            ColdTrailError::InvalidFrameKind(v) => write!(f, "invalid frame kind {v:#04x}"),
            ColdTrailError::InvalidLength { declared, available } => {
                write!(f, "declared length {declared} exceeds available {available}")
            }
            ColdTrailError::InvalidChecksum { expected, actual } => {
                write!(f, "checksum mismatch expected {expected:#06x} actual {actual:#06x}")
            }
            ColdTrailError::InvalidVarint => write!(f, "invalid varint"),
            ColdTrailError::InvalidUtf8 => write!(f, "invalid utf-8"),
            ColdTrailError::OversizedFrame(n) => write!(f, "oversized frame {n} bytes"),
            ColdTrailError::OversizedMessage(n) => write!(f, "oversized message {n} bytes"),
            ColdTrailError::UnknownTlv(tag) => write!(f, "unknown tlv tag {tag:#04x}"),
            ColdTrailError::MissingManifest => write!(f, "manifest is required before this record"),
            ColdTrailError::FragmentIndex => write!(f, "invalid fragment index"),
            ColdTrailError::FragmentConflict => write!(f, "conflicting fragment data"),
            ColdTrailError::FragmentTooLarge => write!(f, "fragment exceeds policy limit"),
            ColdTrailError::CodecUnderflow => write!(f, "codec back-reference underflow"),
            ColdTrailError::CodecOverflow => write!(f, "codec output overflow"),
            ColdTrailError::CodecOpcode(op) => write!(f, "unknown codec opcode {op:#04x}"),
            ColdTrailError::ScriptOpcode(op) => write!(f, "unknown script opcode {op:#04x}"),
            ColdTrailError::ScriptStack => write!(f, "script stack error"),
            ColdTrailError::ScriptBudget => write!(f, "script execution budget exhausted"),
            ColdTrailError::PolicyViolation(msg) => write!(f, "policy violation: {msg}"),
        }
    }
}

impl std::error::Error for ColdTrailError {}
