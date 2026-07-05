use crate::catalog;
use crate::error::{ColdTrailError, Result};
use crate::model::SensorSample;

#[derive(Debug, Clone, Default)]
struct CsvHeader {
    timestamp: Option<usize>,
    sensor: Option<usize>,
    value: Option<usize>,
    unit: Option<usize>,
    quality: Option<usize>,
}

/// Parse CSV files exported by common temperature data loggers.
///
/// The parser accepts a small, dependency-free CSV dialect: comma-separated
/// fields, CRLF or LF records, double-quoted fields, and doubled quotes inside
/// quoted fields. Header names are normalized so exports from different
/// devices can use names like `sensor`, `sensor_code`, `probe`, `value_c`,
/// `temperature`, `quality`, or `status`.
pub fn parse_logger_csv(input: &[u8]) -> Result<Vec<SensorSample>> {
    let text = core::str::from_utf8(input).map_err(|_| ColdTrailError::InvalidUtf8)?;
    let rows = parse_rows(text)?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let header = CsvHeader::from_row(&rows[0]);
    let mut samples = Vec::new();
    for (row_index, row) in rows.iter().enumerate().skip(1) {
        if row.iter().all(|f| f.trim().is_empty()) {
            continue;
        }
        let sensor_code = header
            .sensor
            .and_then(|idx| row.get(idx))
            .and_then(|s| parse_sensor_code(s))
            .unwrap_or_else(|| 0x4100u16.wrapping_add(row_index as u16));
        let value = header
            .value
            .and_then(|idx| row.get(idx))
            .and_then(|s| parse_decimal_centi(s))
            .unwrap_or(0);
        let unit_text = header
            .unit
            .and_then(|idx| row.get(idx))
            .map(String::as_str)
            .unwrap_or("C");
        let scaled_value = normalize_to_centi(value, unit_text);
        let quality = header
            .quality
            .and_then(|idx| row.get(idx))
            .map(|s| parse_quality(s))
            .unwrap_or(0);
        let sequence = header
            .timestamp
            .and_then(|idx| row.get(idx))
            .map(|s| timestamp_sequence(s))
            .unwrap_or(row_index as u16);
        let unit = catalog::lookup_sensor(sensor_code).map(|s| s.unit).unwrap_or("C");
        samples.push(SensorSample {
            sensor_code,
            sequence,
            unit,
            scaled_value,
            quality,
        });
        if samples.len() > 20000 {
            return Err(ColdTrailError::OversizedMessage(samples.len()));
        }
    }
    Ok(samples)
}

impl CsvHeader {
    fn from_row(row: &[String]) -> Self {
        let mut header = CsvHeader::default();
        for (idx, field) in row.iter().enumerate() {
            let normalized = normalize_header(field);
            match normalized.as_str() {
                "time" | "timestamp" | "date" | "datetime" | "loggedat" => {
                    header.timestamp = Some(idx)
                }
                "sensor" | "sensorcode" | "sensorid" | "probe" | "channel" => {
                    header.sensor = Some(idx)
                }
                "value" | "valuec" | "temperature" | "tempc" | "reading" => {
                    header.value = Some(idx)
                }
                "unit" | "units" | "uom" => header.unit = Some(idx),
                "quality" | "status" | "alarm" | "flags" => header.quality = Some(idx),
                _ => {}
            }
        }
        header
    }
}

fn parse_rows(text: &str) -> Result<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                row.push(core::mem::take(&mut field));
            }
            '\n' if !quoted => {
                if field.ends_with('\r') {
                    field.pop();
                }
                row.push(core::mem::take(&mut field));
                rows.push(core::mem::take(&mut row));
            }
            _ => field.push(ch),
        }
    }
    if quoted {
        return Err(ColdTrailError::InvalidLength {
            declared: text.len(),
            available: text.len().saturating_sub(1),
        });
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

fn normalize_header(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn parse_sensor_code(s: &str) -> Option<u16> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).ok()
    } else {
        t.parse().ok()
    }
}

fn parse_decimal_centi(s: &str) -> Option<i32> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let negative = trimmed.starts_with('-');
    let number = trimmed.trim_start_matches(|c| c == '+' || c == '-');
    let mut parts = number.splitn(2, '.');
    let whole: i32 = parts.next()?.parse().ok()?;
    let frac = parts.next().unwrap_or("0");
    let mut centi = whole.saturating_mul(100);
    let mut frac_digits = frac.chars().filter(|c| c.is_ascii_digit());
    let tenths = frac_digits.next().and_then(|c| c.to_digit(10)).unwrap_or(0) as i32;
    let hundredths = frac_digits.next().and_then(|c| c.to_digit(10)).unwrap_or(0) as i32;
    centi = centi.saturating_add(tenths * 10 + hundredths);
    if negative {
        Some(-centi)
    } else {
        Some(centi)
    }
}

fn normalize_to_centi(value: i32, unit: &str) -> i32 {
    let unit = unit.trim().to_ascii_lowercase();
    if unit == "f" || unit == "fahrenheit" {
        ((value - 3200) * 5) / 9
    } else if unit == "k" || unit == "kelvin" {
        value - 27315
    } else {
        value
    }
}

fn parse_quality(s: &str) -> u8 {
    let lower = s.trim().to_ascii_lowercase();
    if lower.contains("alarm") || lower.contains("excursion") || lower.contains("bad") {
        0x80
    } else if lower.contains("warn") || lower.contains("missing") {
        0x40
    } else {
        lower.parse::<u8>().unwrap_or(0)
    }
}

fn timestamp_sequence(s: &str) -> u16 {
    let mut hash = 0x9e37u16;
    for b in s.bytes() {
        hash = hash.rotate_left(5) ^ u16::from(b);
    }
    hash
}
