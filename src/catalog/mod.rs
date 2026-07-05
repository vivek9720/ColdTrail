pub mod lanes;
pub mod profiles;
pub mod sensors;
pub mod stations;

pub use lanes::LaneInfo;
pub use profiles::ProfileInfo;
pub use sensors::SensorInfo;
pub use stations::StationInfo;

pub fn lookup_sensor(code: u16) -> Option<&'static SensorInfo> {
    sensors::SENSORS.iter().find(|s| s.code == code)
}

pub fn lookup_lane(id: u16) -> Option<&'static LaneInfo> {
    lanes::LANES.iter().find(|l| l.id == id)
}

pub fn lookup_station(id: u16) -> Option<&'static StationInfo> {
    stations::STATIONS.iter().find(|s| s.id == id)
}

pub fn lookup_profile(id: u16) -> Option<&'static ProfileInfo> {
    profiles::PROFILES.iter().find(|p| p.id == id)
}

pub fn default_lane_for_route(route_id: u16) -> u16 {
    if lanes::LANES.is_empty() {
        return 0x3000;
    }
    let idx = usize::from(route_id) % lanes::LANES.len();
    lanes::LANES[idx].id
}
