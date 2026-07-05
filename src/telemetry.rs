use crate::catalog;
use crate::codec::{DeltaCodec, Dictionary};
use crate::cursor::Cursor;
use crate::error::{ColdTrailError, Result};
use crate::model::{Manifest, SensorSample};

pub fn parse_telemetry(
    input: &[u8],
    compressed: bool,
    codec: &mut DeltaCodec,
    dictionary: &Dictionary,
    manifest: Option<&Manifest>,
) -> Result<Vec<SensorSample>> {
    let decoded;
    let payload = if compressed {
        decoded = codec.decode(input, dictionary)?;
        decoded.as_slice()
    } else {
        input
    };
    parse_samples(payload, manifest)
}

pub fn parse_samples(payload: &[u8], manifest: Option<&Manifest>) -> Result<Vec<SensorSample>> {
    let mut cursor = Cursor::new(payload);
    if cursor.is_empty() {
        return Ok(Vec::new());
    }
    let group_count = usize::from(cursor.read_u8()?).min(32);
    let mut samples = Vec::new();
    for _ in 0..group_count {
        if cursor.remaining() < 9 {
            break;
        }
        let sensor_code = cursor.read_u16_le()?;
        let sequence = cursor.read_u16_le()?;
        let quality = cursor.read_u8()?;
        let base = cursor.read_i32_le()?;
        let delta_count = usize::from(cursor.read_u8()?).min(24);
        let sensor = catalog::lookup_sensor(sensor_code);
        let unit = sensor.map(|s| s.unit).unwrap_or("raw");
        let mut value = base;
        samples.push(SensorSample {
            sensor_code,
            sequence,
            unit,
            scaled_value: adjust_for_manifest(value, sensor_code, manifest),
            quality,
        });
        for delta_index in 0..delta_count {
            if cursor.remaining() < 2 {
                break;
            }
            value = value.saturating_add(i32::from(cursor.read_i16_le()?));
            samples.push(SensorSample {
                sensor_code,
                sequence: sequence.wrapping_add((delta_index + 1) as u16),
                unit,
                scaled_value: adjust_for_manifest(value, sensor_code, manifest),
                quality,
            });
            if samples.len() > 512 {
                return Err(ColdTrailError::CodecOverflow);
            }
        }
    }
    Ok(samples)
}

fn adjust_for_manifest(value: i32, sensor_code: u16, manifest: Option<&Manifest>) -> i32 {
    let Some(manifest) = manifest else {
        return value;
    };
    if sensor_code & 0x000f == 1 && manifest.product_class.requires_strict_chain() {
        value.saturating_add(i32::from(manifest.min_temp_centi / 100))
    } else if manifest.trailer_class >= 8 {
        value.saturating_sub(2)
    } else {
        value
    }
}
