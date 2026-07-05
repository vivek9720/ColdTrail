use crate::catalog;
use crate::model::{Manifest, ProductClass, SensorSample};

pub fn manifest_risk_score(manifest: &Manifest) -> u16 {
    let lane = catalog::lookup_lane(manifest.lane_profile);
    let mut score = lane.map(|l| u16::from(l.strictness) * 7).unwrap_or(40);
    score = score.saturating_add(match manifest.product_class {
        ProductClass::FrozenFood => 12,
        ProductClass::Vaccine => 55,
        ProductClass::Biologic => 70,
        ProductClass::BloodProduct => 68,
        ProductClass::OrganTransport => 90,
        ProductClass::IndustrialReagent => 35,
        ProductClass::Unknown(_) => 25,
    });
    let band = manifest.max_temp_centi.saturating_sub(manifest.min_temp_centi);
    if band < 400 {
        score = score.saturating_add(30);
    } else if band < 900 {
        score = score.saturating_add(18);
    }
    score.saturating_add((manifest.stops.len() as u16).saturating_mul(3))
}

pub fn sample_excursion_score(samples: &[SensorSample], manifest: &Manifest) -> u16 {
    let mut score = 0u16;
    for sample in samples.iter().take(256) {
        if sample.sensor_code & 0x000f == 1 {
            let temp = sample.scaled_value;
            if temp < i32::from(manifest.min_temp_centi) || temp > i32::from(manifest.max_temp_centi) {
                score = score.saturating_add(10);
            }
        }
        if sample.quality & 0x80 != 0 {
            score = score.saturating_add(2);
        }
    }
    score
}

pub fn lane_summary(lane_profile: u16) -> String {
    if let Some(lane) = catalog::lookup_lane(lane_profile) {
        format!(
            "{}:{}->{}:{}",
            lane.name, lane.origin_station, lane.destination_station, lane.region
        )
    } else {
        format!("lane:{lane_profile:#06x}")
    }
}
