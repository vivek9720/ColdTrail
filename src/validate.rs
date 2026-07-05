use crate::analytics;
use crate::evidence::EvidenceBundle;
use crate::model::{DecodedEvent, Manifest, SensorSample};

#[derive(Debug, Clone)]
pub enum FindingSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct ValidationFinding {
    pub severity: FindingSeverity,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub shipment_id: String,
    pub samples_checked: usize,
    pub edi_events_checked: usize,
    pub binary_events_checked: usize,
    pub findings: Vec<ValidationFinding>,
}

impl ValidationReport {
    pub fn is_acceptable(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|f| matches!(f.severity, FindingSeverity::Critical))
    }
}

pub fn validate_evidence(bundle: &EvidenceBundle) -> ValidationReport {
    let mut report = ValidationReport::default();
    report.samples_checked = bundle.samples.len();
    report.edi_events_checked = bundle.edi_events.len();
    report.shipment_id = bundle
        .edi_events
        .iter()
        .find_map(|e| (!e.shipment_id.is_empty()).then(|| e.shipment_id.clone()))
        .unwrap_or_default();

    if bundle.manifest.is_none() {
        report.findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            code: "manifest.missing",
            message: String::from("No route manifest was included with the evidence bundle."),
        });
    }

    if bundle.samples.is_empty() {
        report.findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            code: "logger.empty",
            message: String::from("No temperature logger samples were available."),
        });
    }

    if let Some(manifest) = &bundle.manifest {
        validate_manifest(manifest, &mut report);
        validate_samples(manifest, &bundle.samples, &mut report);
    }
    validate_edi(&mut report, bundle);
    validate_binary_capture(&mut report, bundle);
    report
}

fn validate_manifest(manifest: &Manifest, report: &mut ValidationReport) {
    if manifest.max_temp_centi < manifest.min_temp_centi {
        report.findings.push(ValidationFinding {
            severity: FindingSeverity::Critical,
            code: "manifest.temperature_range",
            message: String::from("Manifest maximum temperature is lower than minimum temperature."),
        });
    }
    if manifest.stops.is_empty() {
        report.findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            code: "manifest.stops",
            message: String::from("Manifest does not list any route stops."),
        });
    }
    let score = analytics::manifest_risk_score(manifest);
    if score > 180 {
        report.findings.push(ValidationFinding {
            severity: FindingSeverity::Info,
            code: "manifest.high_risk_lane",
            message: format!("Route risk score is {score}; review chain-of-custody evidence carefully."),
        });
    }
}

fn validate_samples(manifest: &Manifest, samples: &[SensorSample], report: &mut ValidationReport) {
    let mut excursions = 0usize;
    let mut bad_quality = 0usize;
    for sample in samples {
        if sample.sensor_code & 0x000f == 0 || sample.unit == "C" {
            if sample.scaled_value < i32::from(manifest.min_temp_centi)
                || sample.scaled_value > i32::from(manifest.max_temp_centi)
            {
                excursions += 1;
            }
        }
        if sample.quality & 0xc0 != 0 {
            bad_quality += 1;
        }
    }
    if excursions > 0 {
        report.findings.push(ValidationFinding {
            severity: FindingSeverity::Critical,
            code: "temperature.excursion",
            message: format!("{excursions} logger samples fall outside the manifest temperature range."),
        });
    }
    if bad_quality > 0 {
        report.findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            code: "logger.quality",
            message: format!("{bad_quality} samples carry alarm or warning quality flags."),
        });
    }
}

fn validate_edi(report: &mut ValidationReport, bundle: &EvidenceBundle) {
    let delivered = bundle
        .edi_events
        .iter()
        .any(|event| matches!(event.status_code.as_str(), "D1" | "X1" | "AF"));
    let delayed = bundle
        .edi_events
        .iter()
        .any(|event| matches!(event.status_code.as_str(), "SD" | "A3" | "AG"));
    if !bundle.edi_events.is_empty() && !delivered {
        report.findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            code: "edi.delivery_status",
            message: String::from("EDI 214 status history does not include an observed delivery event."),
        });
    }
    if delayed {
        report.findings.push(ValidationFinding {
            severity: FindingSeverity::Warning,
            code: "edi.delay_status",
            message: String::from("EDI 214 status history includes a delay or appointment exception."),
        });
    }
}

fn validate_binary_capture(report: &mut ValidationReport, bundle: &EvidenceBundle) {
    if let Some(batch) = &bundle.binary_capture {
        report.binary_events_checked = batch.events.len();
        for event in &batch.events {
            if let DecodedEvent::Telemetry(samples) = event {
                report.samples_checked += samples.len();
            }
        }
        for warning in &batch.warnings {
            report.findings.push(ValidationFinding {
                severity: FindingSeverity::Info,
                code: "binary.warning",
                message: warning.clone(),
            });
        }
    }
}
