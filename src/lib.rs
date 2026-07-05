//! ColdTrail validates cold-chain shipment evidence.
//!
//! The crate is intended for auditors, quality teams, and logistics systems
//! that need to reconcile route manifests, data-logger CSV exports, X12 EDI 214
//! shipment status messages, and refrigerated-trailer telemetry captures. The
//! binary gateway decoder is one ingest path; the higher-level APIs normalize
//! those records into evidence bundles and validation reports.

pub mod analytics;
pub mod catalog;
pub mod checksum;
pub mod codec;
pub mod csvlog;
pub mod cursor;
pub mod edi;
pub mod error;
pub mod evidence;
pub mod frame;
pub mod fragment;
pub mod manifest;
pub mod model;
pub mod report;
pub mod replay;
pub mod reassembly;
pub mod script;
pub mod session;
pub mod telemetry;
pub mod tlv;
pub mod validate;

pub use crate::error::{ColdTrailError, Result};
pub use crate::evidence::{parse_evidence_bundle, EvidenceBundle};
pub use crate::model::{DecodedBatch, Manifest, ReplayReport, ScriptProgram};
pub use crate::session::Decoder;
pub use crate::validate::{validate_evidence, ValidationReport};

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

/// Parse a data-logger CSV export into normalized sensor samples.
pub fn parse_logger_csv(data: &[u8]) -> Result<Vec<model::SensorSample>> {
    csvlog::parse_logger_csv(data)
}

/// Parse the shipment-status subset of ANSI X12 EDI 214.
pub fn parse_edi214(data: &[u8]) -> Result<Vec<edi::Edi214Event>> {
    edi::parse_edi214(data)
}
