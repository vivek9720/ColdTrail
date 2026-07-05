use crate::csvlog;
use crate::edi::{self, Edi214Event};
use crate::error::{ColdTrailError, Result};
use crate::frame;
use crate::manifest;
use crate::model::{DecodedBatch, Manifest, SensorSample};
use crate::session::Decoder;

#[derive(Debug, Clone, Default)]
pub struct EvidenceBundle {
    pub manifest: Option<Manifest>,
    pub samples: Vec<SensorSample>,
    pub edi_events: Vec<Edi214Event>,
    pub binary_capture: Option<DecodedBatch>,
    pub source_notes: Vec<String>,
}

/// Parse an audit evidence bundle.
///
/// The bundle format is intentionally plain text so quality teams can assemble
/// it from exports: `[manifest]` contains key/value route facts, `[logger_csv]`
/// contains the raw data-logger CSV table, `[edi214]` contains X12 EDI 214
/// segments, and `[binary_capture_hex]` may contain hexadecimal bytes from a
/// refrigerated-trailer gateway.
pub fn parse_evidence_bundle(input: &[u8]) -> Result<EvidenceBundle> {
    if input.first() == Some(&frame::SYNC) {
        let mut decoder = Decoder::new();
        let binary_capture = Some(decoder.ingest_stream(input)?);
        return Ok(EvidenceBundle {
            binary_capture,
            ..EvidenceBundle::default()
        });
    }
    let text = core::str::from_utf8(input).map_err(|_| ColdTrailError::InvalidUtf8)?;
    if looks_like_edi(text) {
        return Ok(EvidenceBundle {
            edi_events: edi::parse_edi214(input)?,
            ..EvidenceBundle::default()
        });
    }
    if looks_like_csv(text) {
        return Ok(EvidenceBundle {
            samples: csvlog::parse_logger_csv(input)?,
            ..EvidenceBundle::default()
        });
    }

    let sections = split_sections(text);
    let mut bundle = EvidenceBundle::default();
    for (name, body) in sections {
        match name.as_str() {
            "manifest" => bundle.manifest = parse_manifest_text(&body)?,
            "logger_csv" | "csv" | "temperature_log" => {
                bundle.samples.extend(csvlog::parse_logger_csv(body.as_bytes())?);
            }
            "edi214" | "edi" => {
                bundle.edi_events.extend(edi::parse_edi214(body.as_bytes())?);
            }
            "binary_capture_hex" | "gateway_hex" => {
                let bytes = parse_hex_bytes(&body)?;
                let mut decoder = Decoder::new();
                bundle.binary_capture = Some(decoder.ingest_stream(&bytes)?);
            }
            "note" | "notes" => bundle.source_notes.push(body.trim().to_string()),
            _ => {}
        }
    }
    Ok(bundle)
}

fn looks_like_edi(text: &str) -> bool {
    text.starts_with("ISA*") || text.starts_with("ST*214") || text.contains("~AT7*")
}

fn looks_like_csv(text: &str) -> bool {
    let first = text.lines().next().unwrap_or("").to_ascii_lowercase();
    first.contains("sensor") && (first.contains("value") || first.contains("temperature"))
}

fn split_sections(text: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut current = String::from("notes");
    let mut body = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if !body.trim().is_empty() {
                sections.push((current, core::mem::take(&mut body)));
            }
            current = trimmed
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_ascii_lowercase();
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    if !body.trim().is_empty() {
        sections.push((current, body));
    }
    sections
}

fn parse_manifest_text(body: &str) -> Result<Option<Manifest>> {
    let mut tlv = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        append_manifest_tlv(&mut tlv, key.trim(), value.trim());
    }
    if tlv.is_empty() {
        Ok(None)
    } else {
        manifest::parse_manifest(&tlv).map(Some)
    }
}

fn append_manifest_tlv(out: &mut Vec<u8>, key: &str, value: &str) {
    let key = key.to_ascii_lowercase();
    match key.as_str() {
        "route_id" | "route" => push_tlv(out, 0x01, &parse_u16(value).unwrap_or(0).to_le_bytes()),
        "trailer_class" | "trailer" => push_tlv(out, 0x02, &[value.parse::<u8>().unwrap_or(0)]),
        "product_class" | "product" => push_tlv(out, 0x03, &[parse_product_class(value)]),
        "temp_range" | "temperature_range" => {
            let mut parts = value.split(',');
            let min = parts.next().and_then(parse_centi).unwrap_or(-2500) as i16;
            let max = parts.next().and_then(parse_centi).unwrap_or(800) as i16;
            let mut payload = Vec::new();
            payload.extend_from_slice(&min.to_le_bytes());
            payload.extend_from_slice(&max.to_le_bytes());
            push_tlv(out, 0x04, &payload);
        }
        "station" | "station_name" | "depot" => push_tlv(out, 0x06, value.as_bytes()),
        "flags" => push_tlv(out, 0x07, &parse_u32(value).unwrap_or(0).to_le_bytes()),
        "lane_profile" | "lane" => push_tlv(out, 0x08, &parse_u16(value).unwrap_or(0).to_le_bytes()),
        _ if key.starts_with("stop") => {
            let fields: Vec<&str> = value.split(',').map(str::trim).collect();
            if fields.len() >= 5 {
                let stop_id = parse_u16(fields[0]).unwrap_or(0);
                let station_id = parse_u16(fields[1]).unwrap_or(0);
                let dwell = parse_u16(fields[2]).unwrap_or(0);
                let lat = parse_micro_degrees(fields[3]).unwrap_or(0);
                let lon = parse_micro_degrees(fields[4]).unwrap_or(0);
                let mut payload = Vec::new();
                payload.extend_from_slice(&stop_id.to_le_bytes());
                payload.extend_from_slice(&station_id.to_le_bytes());
                payload.extend_from_slice(&dwell.to_le_bytes());
                payload.extend_from_slice(&lat.to_le_bytes());
                payload.extend_from_slice(&lon.to_le_bytes());
                push_tlv(out, 0x05, &payload);
            }
        }
        _ => {}
    }
}

fn push_tlv(out: &mut Vec<u8>, tag: u8, value: &[u8]) {
    out.push(tag);
    let mut len = value.len() as u64;
    while len >= 0x80 {
        out.push((len as u8 & 0x7f) | 0x80);
        len >>= 7;
    }
    out.push(len as u8);
    out.extend_from_slice(value);
}

fn parse_product_class(value: &str) -> u8 {
    match value.trim().to_ascii_lowercase().as_str() {
        "frozen_food" | "frozen" | "food" => 0,
        "vaccine" | "vaccines" => 1,
        "biologic" | "biologics" => 2,
        "blood" | "blood_product" => 3,
        "organ" | "organ_transport" => 4,
        "reagent" | "industrial_reagent" => 5,
        other => other.parse().unwrap_or(0),
    }
}

fn parse_u16(value: &str) -> Option<u16> {
    let t = value.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).ok()
    } else {
        t.parse().ok()
    }
}

fn parse_u32(value: &str) -> Option<u32> {
    let t = value.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        t.parse().ok()
    }
}

fn parse_centi(value: &str) -> Option<i32> {
    let value = value.trim();
    let negative = value.starts_with('-');
    let digits = value.trim_start_matches(|c| c == '+' || c == '-');
    let mut parts = digits.splitn(2, '.');
    let whole: i32 = parts.next()?.parse().ok()?;
    let frac = parts.next().unwrap_or("0");
    let tenths = frac.chars().next().and_then(|c| c.to_digit(10)).unwrap_or(0) as i32;
    let hundredths = frac.chars().nth(1).and_then(|c| c.to_digit(10)).unwrap_or(0) as i32;
    let centi = whole.saturating_mul(100).saturating_add(tenths * 10 + hundredths);
    if negative {
        Some(-centi)
    } else {
        Some(centi)
    }
}

fn parse_micro_degrees(value: &str) -> Option<i32> {
    parse_centi(value).map(|v| v.saturating_mul(10000))
}

fn parse_hex_bytes(body: &str) -> Result<Vec<u8>> {
    let mut nibbles = Vec::new();
    for ch in body.chars() {
        if let Some(value) = ch.to_digit(16) {
            nibbles.push(value as u8);
        }
    }
    if nibbles.len() % 2 != 0 {
        return Err(ColdTrailError::InvalidLength {
            declared: nibbles.len(),
            available: nibbles.len().saturating_sub(1),
        });
    }
    let mut out = Vec::with_capacity(nibbles.len() / 2);
    for pair in nibbles.chunks(2) {
        out.push((pair[0] << 4) | pair[1]);
    }
    Ok(out)
}
