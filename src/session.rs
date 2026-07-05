use crate::analytics;
use crate::codec::{DeltaCodec, Dictionary};
use crate::error::Result;
use crate::frame::{self, Frame, FrameKind, FLAG_COMPRESSED};
use crate::fragment;
use crate::manifest;
use crate::model::{DecodedBatch, DecodedEvent, Manifest, SessionConfig};
use crate::reassembly::FragmentAssembler;
use crate::script;
use crate::telemetry;

const MAX_SESSIONS: usize = 16;

#[derive(Debug)]
struct SessionState {
    id: u16,
    config: SessionConfig,
    manifest: Option<Manifest>,
    dictionary: Dictionary,
    codec: DeltaCodec,
    assembler: FragmentAssembler,
    frames_seen: usize,
}

impl SessionState {
    fn new(id: u16) -> Self {
        Self {
            id,
            config: SessionConfig::default(),
            manifest: None,
            dictionary: Dictionary::default(),
            codec: DeltaCodec::new(),
            assembler: FragmentAssembler::new(),
            frames_seen: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct Decoder {
    sessions: Vec<SessionState>,
}

impl Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ingest_stream(&mut self, data: &[u8]) -> Result<DecodedBatch> {
        let frames = frame::parse_frames(data)?;
        let mut batch = DecodedBatch {
            frames_seen: frames.len(),
            sessions_seen: 0,
            events: Vec::new(),
            warnings: Vec::new(),
        };
        for frame in frames {
            self.handle_frame(frame, &mut batch)?;
        }
        batch.sessions_seen = self.sessions.len();
        Ok(batch)
    }

    fn handle_frame(&mut self, frame: Frame, batch: &mut DecodedBatch) -> Result<()> {
        match frame.kind {
            FrameKind::ReplaySegment => {
                for nested in frame::parse_frames(&frame.payload)? {
                    self.handle_frame(nested, batch)?;
                }
                Ok(())
            }
            _ => {
                let state = self.state_for(frame.session_id);
                state.frames_seen += 1;
                match frame.kind {
                    FrameKind::Hello => {
                        let config = manifest::parse_hello(&frame.payload)?;
                        state.config = config.clone();
                        state.assembler.configure(
                            state.config.max_fragment_bytes,
                            state
                                .manifest
                                .as_ref()
                                .map(Manifest::zero_copy_fragments_allowed)
                                .unwrap_or(false),
                        );
                        batch.events.push(DecodedEvent::Hello(config));
                    }
                    FrameKind::Manifest => {
                        let parsed = manifest::parse_manifest(&frame.payload)?;
                        let allow_aliases = parsed.zero_copy_fragments_allowed();
                        let risk = analytics::manifest_risk_score(&parsed);
                        state
                            .assembler
                            .configure(state.config.max_fragment_bytes, allow_aliases);
                        if parsed.refreshes_dictionary() {
                            state.dictionary.replace_from_payload(&[])?;
                            state.codec.reset();
                            state.assembler.release_cached_pages();
                        }
                        if risk > 180 {
                            batch
                                .warnings
                                .push(format!("session {} route risk {}", state.id, risk));
                        }
                        state.manifest = Some(parsed.clone());
                        batch.events.push(DecodedEvent::Manifest(parsed));
                    }
                    FrameKind::Telemetry => {
                        let samples = telemetry::parse_telemetry(
                            &frame.payload,
                            frame.has_flag(FLAG_COMPRESSED),
                            &mut state.codec,
                            &state.dictionary,
                            state.manifest.as_ref(),
                        )?;
                        batch.events.push(DecodedEvent::Telemetry(samples));
                    }
                    FrameKind::Fragment => {
                        let block = fragment::parse_fragment(&frame.payload)?;
                        if let Some(message) = state.assembler.accept(block)? {
                            if message.logical_kind == FrameKind::Telemetry {
                                let samples = telemetry::parse_telemetry(
                                    &message.payload,
                                    frame.has_flag(FLAG_COMPRESSED),
                                    &mut state.codec,
                                    &state.dictionary,
                                    state.manifest.as_ref(),
                                )?;
                                batch.events.push(DecodedEvent::Telemetry(samples));
                            }
                            batch.events.push(DecodedEvent::Fragment(message));
                        }
                    }
                    FrameKind::Script => {
                        let program = script::parse_script(&frame.payload)?;
                        batch.events.push(DecodedEvent::Script(program));
                    }
                    FrameKind::Dictionary => {
                        state.dictionary.replace_from_payload(&frame.payload)?;
                        state.codec.reset();
                        state.assembler.release_cached_pages();
                        batch.events.push(DecodedEvent::Dictionary {
                            generation: state.dictionary.generation,
                            bytes: state.dictionary.total_bytes(),
                        });
                    }
                    FrameKind::Ack => {
                        let status = frame.payload.first().copied().unwrap_or(0);
                        batch.events.push(DecodedEvent::Ack {
                            sequence: frame.sequence,
                            status,
                        });
                    }
                    FrameKind::Unknown(_) | FrameKind::ReplaySegment => {}
                }
                Ok(())
            }
        }
    }

    fn state_for(&mut self, id: u16) -> &mut SessionState {
        if let Some(index) = self.sessions.iter().position(|s| s.id == id) {
            return &mut self.sessions[index];
        }
        if self.sessions.len() >= MAX_SESSIONS {
            self.sessions.remove(0);
        }
        self.sessions.push(SessionState::new(id));
        let index = self.sessions.len() - 1;
        &mut self.sessions[index]
    }
}
