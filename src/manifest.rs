use crate::catalog;
use crate::cursor::Cursor;
use crate::error::{ColdTrailError, Result};
use crate::model::{Manifest, ProductClass, SessionConfig, Stop};
use crate::tlv::TlvReader;

pub const TLV_ROUTE_ID: u8 = 0x01;
pub const TLV_TRAILER_CLASS: u8 = 0x02;
pub const TLV_PRODUCT_CLASS: u8 = 0x03;
pub const TLV_TEMP_RANGE: u8 = 0x04;
pub const TLV_STOP: u8 = 0x05;
pub const TLV_STATION_NAME: u8 = 0x06;
pub const TLV_FLAGS: u8 = 0x07;
pub const TLV_LANE_PROFILE: u8 = 0x08;

pub fn parse_hello(input: &[u8]) -> Result<SessionConfig> {
    let mut cursor = Cursor::new(input);
    let route_id = cursor.read_u16_le().unwrap_or(0);
    let trailer_class = cursor.read_u8().unwrap_or(0);
    let max_fragment_bytes = usize::from(cursor.read_u16_le().unwrap_or(2048)).clamp(128, 16384);
    let features = cursor.read_u32_le().unwrap_or(0);
    let depot_len = usize::from(cursor.read_u8().unwrap_or(0)).min(cursor.remaining());
    let depot = cursor.read_string(depot_len).unwrap_or_default();
    Ok(SessionConfig {
        route_id,
        trailer_class,
        max_fragment_bytes,
        features,
        depot,
    })
}

pub fn parse_manifest(input: &[u8]) -> Result<Manifest> {
    let mut manifest = Manifest::default();
    let mut reader = TlvReader::new(input);
    while let Some(tlv) = reader.next()? {
        match tlv.tag {
            TLV_ROUTE_ID if tlv.value.len() >= 2 => {
                let mut c = Cursor::new(tlv.value);
                manifest.route_id = c.read_u16_le()?;
            }
            TLV_TRAILER_CLASS if !tlv.value.is_empty() => {
                manifest.trailer_class = tlv.value[0];
            }
            TLV_PRODUCT_CLASS if !tlv.value.is_empty() => {
                manifest.product_class = ProductClass::from_byte(tlv.value[0]);
            }
            TLV_TEMP_RANGE if tlv.value.len() >= 4 => {
                let mut c = Cursor::new(tlv.value);
                manifest.min_temp_centi = c.read_i16_le()?;
                manifest.max_temp_centi = c.read_i16_le()?;
            }
            TLV_STOP => {
                if let Some(stop) = parse_stop(tlv.value)? {
                    manifest.stops.push(stop);
                }
            }
            TLV_STATION_NAME => {
                manifest.station_name = core::str::from_utf8(tlv.value)
                    .map(|s| s.chars().take(64).collect())
                    .unwrap_or_default();
            }
            TLV_FLAGS if tlv.value.len() >= 4 => {
                let mut c = Cursor::new(tlv.value);
                manifest.flags = c.read_u32_le()?;
            }
            TLV_LANE_PROFILE if tlv.value.len() >= 2 => {
                let mut c = Cursor::new(tlv.value);
                manifest.lane_profile = c.read_u16_le()?;
            }
            0xf0..=0xff => {}
            other => {
                if tlv.value.len() > 64 {
                    return Err(ColdTrailError::UnknownTlv(other));
                }
            }
        }
    }
    if manifest.lane_profile == 0 {
        manifest.lane_profile = catalog::default_lane_for_route(manifest.route_id);
    }
    Ok(manifest)
}

fn parse_stop(input: &[u8]) -> Result<Option<Stop>> {
    if input.len() < 12 {
        return Ok(None);
    }
    let mut c = Cursor::new(input);
    let stop_id = c.read_u16_le()?;
    let station_id = c.read_u16_le()?;
    let dwell_minutes = c.read_u16_le()?;
    let latitude_micro = c.read_i32_le()?;
    let longitude_micro = if c.remaining() >= 4 { c.read_i32_le()? } else { 0 };
    Ok(Some(Stop {
        stop_id,
        station_id,
        dwell_minutes,
        latitude_micro,
        longitude_micro,
    }))
}
