use crate::validate::{FindingSeverity, ValidationReport};

pub fn render_text_report(report: &ValidationReport) -> String {
    let mut out = String::new();
    out.push_str("ColdTrail validation report\n");
    if !report.shipment_id.is_empty() {
        out.push_str("Shipment: ");
        out.push_str(&report.shipment_id);
        out.push('\n');
    }
    out.push_str(&format!(
        "Samples: {} | EDI events: {} | Binary events: {}\n",
        report.samples_checked, report.edi_events_checked, report.binary_events_checked
    ));
    out.push_str(if report.is_acceptable() {
        "Disposition: reviewable\n"
    } else {
        "Disposition: blocked\n"
    });
    for finding in &report.findings {
        let severity = match finding.severity {
            FindingSeverity::Info => "info",
            FindingSeverity::Warning => "warning",
            FindingSeverity::Critical => "critical",
        };
        out.push_str(&format!("[{severity}] {}: {}\n", finding.code, finding.message));
    }
    out
}
