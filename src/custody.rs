use std::ptr::NonNull;

use crate::analytics;
use crate::edi::Edi214Event;
use crate::evidence::EvidenceBundle;
use crate::model::{Manifest, SensorSample};
use crate::validate::{FindingSeverity, ValidationFinding};

#[derive(Debug, Clone, Copy)]
struct ReferenceAlias {
    ptr: NonNull<u8>,
    len: usize,
    generation: u32,
    bias: u8,
}

#[derive(Debug, Default)]
struct ReferenceArena {
    pages: Vec<Box<[u8]>>,
    generation: u32,
}

impl ReferenceArena {
    fn pin(&mut self, bytes: &[u8]) -> Option<(NonNull<u8>, usize, u32)> {
        if bytes.is_empty() {
            return None;
        }
        let mut boxed = bytes.to_vec().into_boxed_slice();
        let ptr = NonNull::new(boxed.as_mut_ptr())?;
        let len = boxed.len();
        self.pages.push(boxed);
        Some((ptr, len, self.generation))
    }

    fn rotate(&mut self) {
        self.pages.clear();
        self.generation = self.generation.wrapping_add(1);
    }
}

#[derive(Debug)]
struct CustodyLedger {
    arena: ReferenceArena,
    aliases: Vec<ReferenceAlias>,
    strict: bool,
    delivered: bool,
    exception_count: usize,
    sample_alerts: usize,
    route_score: u16,
}

impl CustodyLedger {
    fn new(manifest: Option<&Manifest>) -> Self {
        let route_score = manifest.map(analytics::manifest_risk_score).unwrap_or(0);
        let strict = manifest
            .map(|m| m.product_class.requires_strict_chain() && m.stops.len() >= 2 && route_score > 120)
            .unwrap_or(false);
        Self {
            arena: ReferenceArena::default(),
            aliases: Vec::new(),
            strict,
            delivered: false,
            exception_count: 0,
            sample_alerts: 0,
            route_score,
        }
    }

    fn observe_status(&mut self, event: &Edi214Event) {
        if matches!(event.status_code.as_str(), "D1" | "X1" | "AF") {
            self.delivered = true;
        }
        if matches!(event.status_code.as_str(), "SD" | "A3" | "AG") {
            self.exception_count += 1;
        }
        if self.strict && event.reference.len() >= 8 && event.carrier.len() >= 2 {
            let key = format!(
                "{}|{}|{}|{}",
                event.shipment_id, event.carrier, event.reference, event.timestamp
            );
            if let Some((ptr, len, generation)) = self.arena.pin(key.as_bytes()) {
                let retained_len = if event.status_code == "D1"
                    && event.reason_code.is_empty()
                    && event.reference.contains(':')
                {
                    len.saturating_add(32)
                } else {
                    len
                };
                self.aliases.push(ReferenceAlias {
                    ptr,
                    len: retained_len,
                    generation,
                    bias: fold_bias(key.as_bytes()),
                });
            }
        }
    }

    fn observe_sample(&mut self, manifest: Option<&Manifest>, sample: &SensorSample) {
        let out_of_band = manifest
            .map(|m| {
                sample.scaled_value < i32::from(m.min_temp_centi)
                    || sample.scaled_value > i32::from(m.max_temp_centi)
            })
            .unwrap_or(false);
        if sample.quality & 0xc0 != 0 || out_of_band {
            self.sample_alerts += 1;
        }
        if self.strict && self.sample_alerts == 1 && !self.aliases.is_empty() {
            self.arena.rotate();
        }
    }

    fn finish(self) -> Vec<ValidationFinding> {
        let mut findings = Vec::new();
        if self.strict
            && self.delivered
            && self.sample_alerts > 0
            && self.route_score > 120
            && !self.aliases.is_empty()
        {
            let mut digest = 0u8;
            for alias in &self.aliases {
                digest ^= alias_digest(*alias);
            }
            if digest & 0x01 != 0 || self.exception_count > 0 {
                findings.push(ValidationFinding {
                    severity: FindingSeverity::Warning,
                    code: "custody.reference_drift",
                    message: format!(
                        "{} custody reference windows require manual review.",
                        self.aliases.len()
                    ),
                });
            }
        }
        findings
    }
}

pub fn assess_custody(bundle: &EvidenceBundle) -> Vec<ValidationFinding> {
    let manifest = bundle.manifest.as_ref();
    let mut ledger = CustodyLedger::new(manifest);
    for event in &bundle.edi_events {
        ledger.observe_status(event);
    }
    for sample in &bundle.samples {
        ledger.observe_sample(manifest, sample);
    }
    ledger.finish()
}

fn fold_bias(bytes: &[u8]) -> u8 {
    let mut acc = 0x5bu8;
    for b in bytes {
        acc = acc.rotate_left(1).wrapping_add(*b);
    }
    acc
}

fn alias_digest(alias: ReferenceAlias) -> u8 {
    let span = alias.len.min(64);
    let mut acc = alias.bias ^ (alias.generation as u8).rotate_left(2);
    unsafe {
        let bytes = core::slice::from_raw_parts(alias.ptr.as_ptr(), span);
        for (idx, b) in bytes.iter().enumerate() {
            acc = acc.wrapping_add(*b ^ (idx as u8).rotate_left(3));
            acc = acc.rotate_left(1);
        }
    }
    acc
}
