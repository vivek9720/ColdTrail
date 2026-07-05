use crate::frame::FrameKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductClass {
    FrozenFood,
    Vaccine,
    Biologic,
    BloodProduct,
    OrganTransport,
    IndustrialReagent,
    Unknown(u8),
}

impl ProductClass {
    pub fn from_byte(value: u8) -> Self {
        match value {
            0 => ProductClass::FrozenFood,
            1 => ProductClass::Vaccine,
            2 => ProductClass::Biologic,
            3 => ProductClass::BloodProduct,
            4 => ProductClass::OrganTransport,
            5 => ProductClass::IndustrialReagent,
            other => ProductClass::Unknown(other),
        }
    }

    pub fn requires_strict_chain(&self) -> bool {
        matches!(
            self,
            ProductClass::Vaccine
                | ProductClass::Biologic
                | ProductClass::BloodProduct
                | ProductClass::OrganTransport
        )
    }
}

#[derive(Debug, Clone)]
pub struct Stop {
    pub stop_id: u16,
    pub station_id: u16,
    pub dwell_minutes: u16,
    pub latitude_micro: i32,
    pub longitude_micro: i32,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub route_id: u16,
    pub lane_profile: u16,
    pub trailer_class: u8,
    pub product_class: ProductClass,
    pub min_temp_centi: i16,
    pub max_temp_centi: i16,
    pub flags: u32,
    pub station_name: String,
    pub stops: Vec<Stop>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            route_id: 0,
            lane_profile: 0,
            trailer_class: 0,
            product_class: ProductClass::FrozenFood,
            min_temp_centi: -2500,
            max_temp_centi: 800,
            flags: 0,
            station_name: String::new(),
            stops: Vec::new(),
        }
    }
}

impl Manifest {
    pub fn zero_copy_fragments_allowed(&self) -> bool {
        let narrow_band = self.max_temp_centi.saturating_sub(self.min_temp_centi) <= 900;
        self.flags & 0x20 != 0
            && self.product_class.requires_strict_chain()
            && self.stops.len() >= 2
            && narrow_band
    }

    pub fn refreshes_dictionary(&self) -> bool {
        self.flags & 0x80 != 0
    }
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub route_id: u16,
    pub trailer_class: u8,
    pub max_fragment_bytes: usize,
    pub features: u32,
    pub depot: String,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            route_id: 0,
            trailer_class: 0,
            max_fragment_bytes: 2048,
            features: 0,
            depot: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SensorSample {
    pub sensor_code: u16,
    pub sequence: u16,
    pub unit: &'static str,
    pub scaled_value: i32,
    pub quality: u8,
}

#[derive(Debug, Clone)]
pub struct FragmentMessage {
    pub message_id: u32,
    pub logical_kind: FrameKind,
    pub payload: Vec<u8>,
    pub trust_score: u8,
}

#[derive(Debug, Clone)]
pub enum DecodedEvent {
    Hello(SessionConfig),
    Manifest(Manifest),
    Telemetry(Vec<SensorSample>),
    Fragment(FragmentMessage),
    Script(ScriptProgram),
    Dictionary { generation: u32, bytes: usize },
    Ack { sequence: u16, status: u8 },
}

#[derive(Debug, Clone, Default)]
pub struct DecodedBatch {
    pub frames_seen: usize,
    pub sessions_seen: usize,
    pub events: Vec<DecodedEvent>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ScriptInstruction {
    LoadConst(i32),
    LoadSensor(u16),
    StoreSlot(u8),
    Add,
    Sub,
    Mul,
    Clamp { min: i32, max: i32 },
    Emit { channel: u8 },
    JumpIfBelow { slot: u8, threshold: i32, target: u8 },
    MarkStation(u16),
    Stop,
}

#[derive(Debug, Clone, Default)]
pub struct ScriptProgram {
    pub instructions: Vec<ScriptInstruction>,
    pub labels: Vec<u8>,
    pub emits: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ReplayReport {
    pub segments: usize,
    pub nested_frames: usize,
    pub events: usize,
    pub discarded_segments: usize,
    pub warnings: Vec<String>,
}
