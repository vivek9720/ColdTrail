//! ColdTrail decodes staged cold-chain telemetry captures.
//!
//! A stream contains wire frames, route-manifest TLVs, compressed sensor
//! blocks, fragmented shipment reports, and field-service scripts. The public
//! entry points intentionally stay small; most behavior lives behind the
//! staged decoder so fuzzers exercise real parser state rather than a thin
//! byte-slice wrapper.

pub mod analytics;
pub mod catalog;
pub mod checksum;
pub mod codec;
pub mod cursor;
pub mod error;
pub mod frame;
pub mod fragment;
pub mod manifest;
pub mod model;
pub mod replay;
pub mod reassembly;
pub mod script;
pub mod session;
pub mod telemetry;
pub mod tlv;

pub use crate::error::{ColdTrailError, Result};
pub use crate::model::{DecodedBatch, Manifest, ReplayReport, ScriptProgram};
pub use crate::session::Decoder;

/// Parse a complete ColdTrail capture using a fresh state machine.
pub fn parse(data: &[u8]) -> Result<DecodedBatch> {
    let mut decoder = Decoder::new();
    decoder.ingest_stream(data)
}

/// Parse a replay bundle, which may itself contain nested ColdTrail frames.
pub fn replay(data: &[u8]) -> Result<ReplayReport> {
    let mut runner = replay::ReplayRunner::new();
    runner.run(data)
}

/// Parse only the field-service command script sublanguage.
pub fn parse_script(data: &[u8]) -> Result<ScriptProgram> {
    script::parse_script(data)
}

/// Parse wire frames without semantic session handling.
pub fn parse_frames(data: &[u8]) -> Result<Vec<frame::Frame>> {
    frame::parse_frames(data)
}
