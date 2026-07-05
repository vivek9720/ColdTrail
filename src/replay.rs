use crate::codec::{DeltaCodec, Dictionary};
use crate::cursor::Cursor;
use crate::error::Result;
use crate::frame;
use crate::model::ReplayReport;
use crate::session::Decoder;

pub struct ReplayRunner {
    decoder: Decoder,
    codec: DeltaCodec,
    dictionary: Dictionary,
}

impl Default for ReplayRunner {
    fn default() -> Self {
        Self {
            decoder: Decoder::new(),
            codec: DeltaCodec::new(),
            dictionary: Dictionary::default(),
        }
    }
}

impl ReplayRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run(&mut self, data: &[u8]) -> Result<ReplayReport> {
        if data.starts_with(b"RPLY") {
            self.run_bundle(&data[4..])
        } else {
            let batch = self.decoder.ingest_stream(data)?;
            Ok(ReplayReport {
                segments: 1,
                nested_frames: batch.frames_seen,
                events: batch.events.len(),
                discarded_segments: 0,
                warnings: batch.warnings,
            })
        }
    }

    fn run_bundle(&mut self, data: &[u8]) -> Result<ReplayReport> {
        let mut cursor = Cursor::new(data);
        let mut report = ReplayReport::default();
        while cursor.remaining() >= 4 {
            let flags = cursor.read_u8()?;
            let len = usize::from(cursor.read_u16_le()?);
            let generation_hint = cursor.read_u8()?;
            if len > cursor.remaining() {
                report.discarded_segments += 1;
                break;
            }
            let segment = cursor.read_exact(len)?;
            report.segments += 1;
            let decoded;
            let payload = if flags & 0x01 != 0 {
                if generation_hint & 0x80 != 0 {
                    self.dictionary.replace_from_payload(segment)?;
                }
                decoded = self.codec.decode(segment, &self.dictionary)?;
                decoded.as_slice()
            } else {
                segment
            };
            let frames = frame::parse_frames(payload)?;
            report.nested_frames += frames.len();
            let batch = self.decoder.ingest_stream(payload)?;
            report.events += batch.events.len();
            report.warnings.extend(batch.warnings);
        }
        Ok(report)
    }
}
