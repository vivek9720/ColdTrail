use crate::error::{ColdTrailError, Result};

#[derive(Debug, Clone, Default)]
pub struct Edi214Event {
    pub shipment_id: String,
    pub carrier: String,
    pub equipment: String,
    pub status_code: String,
    pub reason_code: String,
    pub city: String,
    pub state: String,
    pub timestamp: String,
    pub reference: String,
}

/// Parse the shipment-status subset of ANSI X12 EDI 214.
///
/// This handles the common segments needed by cold-chain reconciliation:
/// `B10` for shipment identity, `MS2` for equipment, `AT7` for status and
/// status time, `MS1` for location, and `L11` for bill/reference numbers.
pub fn parse_edi214(input: &[u8]) -> Result<Vec<Edi214Event>> {
    let text = core::str::from_utf8(input).map_err(|_| ColdTrailError::InvalidUtf8)?;
    let segment_sep = detect_segment_separator(text);
    let mut current = Edi214Event::default();
    let mut events = Vec::new();
    for raw_segment in text.split(segment_sep) {
        let segment = raw_segment.trim_matches(|c: char| c.is_ascii_whitespace());
        if segment.is_empty() {
            continue;
        }
        let mut fields = segment.split('*');
        let tag = fields.next().unwrap_or("");
        let values: Vec<&str> = fields.collect();
        match tag {
            "ISA" | "GS" | "ST" | "SE" | "GE" | "IEA" => {}
            "B10" => {
                if !current.status_code.is_empty() {
                    events.push(core::mem::take(&mut current));
                }
                current.shipment_id = values
                    .get(1)
                    .or_else(|| values.get(0))
                    .copied()
                    .unwrap_or("")
                    .to_string();
                current.carrier = values.get(2).copied().unwrap_or("").to_string();
            }
            "MS2" => {
                if let Some(carrier) = values.first().copied().filter(|s| !s.is_empty()) {
                    current.carrier = carrier.to_string();
                }
                current.equipment = values.get(1).copied().unwrap_or("").to_string();
            }
            "AT7" => {
                if !current.status_code.is_empty() {
                    events.push(current.clone());
                }
                current.status_code = values.first().copied().unwrap_or("").to_string();
                current.reason_code = values.get(1).copied().unwrap_or("").to_string();
                let date = values.get(4).copied().unwrap_or("");
                let time = values.get(5).copied().unwrap_or("");
                current.timestamp = format!("{date}{time}");
            }
            "MS1" => {
                current.city = values.first().copied().unwrap_or("").to_string();
                current.state = values.get(1).copied().unwrap_or("").to_string();
            }
            "L11" => {
                let value = values.first().copied().unwrap_or("");
                let qualifier = values.get(1).copied().unwrap_or("");
                current.reference = if qualifier.is_empty() {
                    value.to_string()
                } else {
                    format!("{qualifier}:{value}")
                };
            }
            "LX" => {
                if !current.status_code.is_empty() {
                    events.push(core::mem::take(&mut current));
                }
            }
            _ => {}
        }
        if events.len() > 2048 {
            return Err(ColdTrailError::OversizedMessage(events.len()));
        }
    }
    if !current.status_code.is_empty() || !current.shipment_id.is_empty() {
        events.push(current);
    }
    Ok(events)
}

fn detect_segment_separator(text: &str) -> char {
    if text.contains('~') {
        '~'
    } else if text.contains('\n') {
        '\n'
    } else {
        '~'
    }
}
