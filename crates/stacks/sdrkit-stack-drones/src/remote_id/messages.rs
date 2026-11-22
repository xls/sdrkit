//! Broadcast Remote ID messages: decoding and encoding of the six message types and the Message Pack, the two carriers (Bluetooth
//! LE service data, Wi-Fi vendor element) and the one-line summaries.
//!
//! Standard: ASTM F3411-22a, ASD-STAN prEN 4709-002.

use crate::family::{ascii_text, mac_text};
use serde::{Deserialize, Serialize};

/// Bytes in every message.
pub const MSG_LEN: usize = 25;
/// The ASTM 16-bit service UUID on Bluetooth LE.
pub const BLE_UUID: u16 = 0xFFFA;
/// The application code after the UUID (and the Wi-Fi OUI type).
pub const APP_CODE: u8 = 0x0D;
/// The Wi-Fi vendor element OUI.
pub const WIFI_OUI: [u8; 3] = [0xFA, 0x0B, 0xBC];

sdrkit_core::choice_enum! {
    /// The message types, in type order.
    #[derive(Serialize, Deserialize, PartialOrd, Ord)]
    pub enum MsgType {
        /// 0.
        BasicId = ("basic_id", "Basic ID"),
        /// 1.
        Location = ("location", "Location"),
        /// 2.
        Authentication = ("authentication", "Authentication"),
        /// 3.
        SelfId = ("self_id", "Self-ID"),
        /// 4.
        System = ("system", "System"),
        /// 5.
        OperatorId = ("operator_id", "Operator ID"),
    }
}

impl MsgType {
    /// The type number.
    pub fn code(self) -> u8 {
        self as u8
    }
    /// The name shown.
    pub fn name(self) -> &'static str {
        match self {
            MsgType::BasicId => "Basic ID",
            MsgType::Location => "Location",
            MsgType::Authentication => "Authentication",
            MsgType::SelfId => "Self-ID",
            MsgType::System => "System",
            MsgType::OperatorId => "Operator ID",
        }
    }
    /// All six, in type order.
    pub const ALL: [MsgType; 6] = [
        MsgType::BasicId,
        MsgType::Location,
        MsgType::Authentication,
        MsgType::SelfId,
        MsgType::System,
        MsgType::OperatorId,
    ];
}

/// UA type names (0..15).
pub const UA_TYPES: [&str; 16] = [
    "not declared",
    "aeroplane",
    "multirotor / helicopter",
    "gyroplane",
    "hybrid lift (VTOL)",
    "ornithopter",
    "glider",
    "kite",
    "free balloon",
    "captive balloon",
    "airship",
    "parachute",
    "rocket",
    "tethered powered",
    "ground obstacle",
    "other",
];

/// The ID type names (0..4; others reserved).
pub fn id_type_name(t: u8) -> &'static str {
    match t {
        0 => "none",
        1 => "serial number",
        2 => "CAA registration",
        3 => "UTM assigned UUID",
        4 => "specific session ID",
        _ => "reserved",
    }
}

/// Status names.
pub fn status_name(s: u8) -> &'static str {
    match s {
        0 => "undeclared",
        1 => "on the ground",
        2 => "airborne",
        3 => "emergency",
        4 => "Remote ID system failure",
        _ => "reserved",
    }
}

/// Height type names.
pub fn height_type_name(h: u8) -> &'static str {
    if h == 0 {
        "above take-off"
    } else {
        "above ground"
    }
}

/// Horizontal accuracy names.
pub fn horiz_acc_name(a: u8) -> &'static str {
    const N: [&str; 13] = [
        "unknown",
        "< 18.5 km",
        "< 7.4 km",
        "< 3.7 km",
        "< 1.9 km",
        "< 926 m",
        "< 556 m",
        "< 185 m",
        "< 93 m",
        "< 30 m",
        "< 10 m",
        "< 3 m",
        "< 1 m",
    ];
    N.get(a as usize).copied().unwrap_or("reserved")
}

/// Vertical (and barometric) accuracy names.
pub fn vert_acc_name(a: u8) -> &'static str {
    const N: [&str; 7] = [
        "unknown", "< 150 m", "< 45 m", "< 25 m", "< 10 m", "< 3 m", "< 1 m",
    ];
    N.get(a as usize).copied().unwrap_or("reserved")
}

/// Speed accuracy names.
pub fn speed_acc_name(a: u8) -> &'static str {
    const N: [&str; 5] = ["unknown", "< 10 m/s", "< 3 m/s", "< 1 m/s", "< 0.3 m/s"];
    N.get(a as usize).copied().unwrap_or("reserved")
}

/// Authentication type names.
pub fn auth_type_name(t: u8) -> &'static str {
    match t {
        0 => "none",
        1 => "UAS ID signature",
        2 => "operator ID signature",
        3 => "message set signature",
        4 => "network Remote ID",
        5 => "specific",
        6..=9 => "reserved",
        _ => "private use",
    }
}

/// Operator location type names.
pub fn location_type_name(t: u8) -> &'static str {
    match t {
        0 => "take-off",
        1 => "live GNSS",
        2 => "fixed",
        _ => "reserved",
    }
}

/// Classification type names.
pub fn classification_name(t: u8) -> &'static str {
    match t {
        0 => "undeclared",
        1 => "EU",
        _ => "reserved",
    }
}

/// EU category names.
pub fn category_name(c: u8) -> &'static str {
    match c {
        0 => "undefined",
        1 => "open",
        2 => "specific",
        3 => "certified",
        _ => "reserved",
    }
}

/// EU class text: "C0" … "C6", "-" otherwise.
pub fn class_name(c: u8) -> String {
    if (1..=7).contains(&c) {
        format!("C{}", c - 1)
    } else {
        "-".into()
    }
}

/// Basic ID (type 0).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BasicId {
    /// ID type (0..15).
    pub id_type: u8,
    /// UA type (0..15).
    pub ua_type: u8,
    /// The ID as shown (text, a UUID, "reserved" or empty).
    pub id: String,
}

/// Location / Vector (type 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Location {
    /// Status (0..15).
    pub status: u8,
    /// Height type: 0 above take-off, 1 above ground.
    pub height_type: u8,
    /// Track direction, degrees (None: unknown).
    pub direction: Option<f64>,
    /// Horizontal speed, m/s (None: unknown).
    pub speed: Option<f64>,
    /// Vertical speed, m/s up (None: unknown).
    pub vspeed: Option<f64>,
    /// Latitude, degrees (None: no position).
    pub lat: Option<f64>,
    /// Longitude.
    pub lon: Option<f64>,
    /// Pressure altitude, m.
    pub alt_baro: Option<f64>,
    /// Geodetic altitude (WGS 84), m.
    pub alt_geo: Option<f64>,
    /// Height, m.
    pub height: Option<f64>,
    /// Horizontal accuracy code.
    pub h_acc: u8,
    /// Vertical accuracy code.
    pub v_acc: u8,
    /// Barometric altitude accuracy code.
    pub baro_acc: u8,
    /// Speed accuracy code.
    pub speed_acc: u8,
    /// Tenths of a second since the hour (None: unknown).
    pub timestamp: Option<u16>,
    /// Timestamp accuracy, tenths of a second (0 unknown).
    pub ts_acc: u8,
}

/// Authentication (type 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Authentication {
    /// Authentication type.
    pub auth_type: u8,
    /// Page number.
    pub page: u8,
    /// Page 0: the last page index.
    pub last_page: Option<u8>,
    /// Page 0: the total length in bytes.
    pub length: Option<u8>,
    /// Page 0: seconds since 2019-01-01.
    pub timestamp: Option<u32>,
    /// The data bytes of this page.
    pub data: Vec<u8>,
}

/// Self-ID (type 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelfId {
    /// Description type.
    pub desc_type: u8,
    /// The text.
    pub text: String,
}

/// System (type 4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct System {
    /// Classification type.
    pub class_type: u8,
    /// Operator location type.
    pub loc_type: u8,
    /// Operator latitude (None: no operator position).
    pub op_lat: Option<f64>,
    /// Operator longitude.
    pub op_lon: Option<f64>,
    /// Area count.
    pub area_count: u16,
    /// Area radius, m.
    pub area_radius: f64,
    /// Area ceiling, m.
    pub area_ceiling: Option<f64>,
    /// Area floor, m.
    pub area_floor: Option<f64>,
    /// EU category.
    pub category: u8,
    /// EU class code (0 undefined, 1..7 = C0..C6).
    pub class: u8,
    /// Operator geodetic altitude, m.
    pub op_alt: Option<f64>,
    /// Seconds since 2019-01-01.
    pub timestamp: u32,
}

/// Operator ID (type 5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OperatorId {
    /// Operator ID type.
    pub id_type: u8,
    /// The text.
    pub text: String,
}

/// One decoded message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
    /// Type 0.
    BasicId(BasicId),
    /// Type 1.
    Location(Location),
    /// Type 2.
    Authentication(Authentication),
    /// Type 3.
    SelfId(SelfId),
    /// Type 4.
    System(System),
    /// Type 5.
    OperatorId(OperatorId),
}

impl Message {
    /// Its type.
    pub fn kind(&self) -> MsgType {
        match self {
            Message::BasicId(_) => MsgType::BasicId,
            Message::Location(_) => MsgType::Location,
            Message::Authentication(_) => MsgType::Authentication,
            Message::SelfId(_) => MsgType::SelfId,
            Message::System(_) => MsgType::System,
            Message::OperatorId(_) => MsgType::OperatorId,
        }
    }
}

/// A message with its protocol version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Versioned {
    /// Protocol version (low nibble of byte 0).
    pub version: u8,
    /// The message.
    pub msg: Message,
}

/// Why bytes are not a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// Shorter than 25 bytes.
    Short,
    /// A reserved type as a single message.
    Reserved,
    /// A Message Pack with a wrong size, count or length.
    BadPack,
}

fn u16le(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn i32le(b: &[u8], i: usize) -> i32 {
    i32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u32le(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// The altitude encoding: raw x 0.5 - 1000 m; 0 unknown.
pub fn alt_decode(raw: u16) -> Option<f64> {
    (raw != 0).then_some(raw as f64 * 0.5 - 1000.0)
}

/// The altitude encoding's inverse (None: 0).
pub fn alt_encode(a: Option<f64>) -> u16 {
    match a {
        Some(a) => ((a + 1000.0) / 0.5).round().clamp(0.0, 65535.0) as u16,
        None => 0,
    }
}

fn latlon(lat: i32, lon: i32) -> (Option<f64>, Option<f64>) {
    let (a, o) = (lat as f64 * 1e-7, lon as f64 * 1e-7);
    if (lat == 0 && lon == 0) || a.abs() > 90.0 || o.abs() > 180.0 {
        (None, None)
    } else {
        (Some(a), Some(o))
    }
}

fn uuid_text(b: &[u8]) -> String {
    let h: String = b[..16].iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

/// Decodes one 25-byte message (a pack is refused here).
pub fn decode_message(b: &[u8]) -> Result<Versioned, Malformed> {
    if b.len() < MSG_LEN {
        return Err(Malformed::Short);
    }
    let t = b[0] >> 4;
    let version = b[0] & 15;
    let msg = match t {
        0 => {
            let id_type = b[1] >> 4;
            let ua_type = b[1] & 15;
            let id = match id_type {
                0 => String::new(),
                1 | 2 | 4 => ascii_text(&b[2..22]),
                3 => uuid_text(&b[2..22]),
                _ => "reserved".into(),
            };
            Message::BasicId(BasicId {
                id_type,
                ua_type,
                id,
            })
        }
        1 => {
            let f = b[1];
            let ew = (f >> 1) & 1 == 1;
            let mult = f & 1 == 1;
            let raw_dir = b[2] as u32 + if ew { 180 } else { 0 };
            let direction = if raw_dir >= 361 {
                None
            } else if raw_dir == 360 {
                Some(0.0)
            } else {
                Some(raw_dir as f64)
            };
            let speed = match (mult, b[3]) {
                (true, 255) => None,
                (true, r) => Some(r as f64 * 0.75 + 63.75),
                (false, r) => Some(r as f64 * 0.25),
            };
            let vs = b[4] as i8;
            let vspeed = (vs != 126).then_some(vs as f64 * 0.5);
            let (lat, lon) = latlon(i32le(b, 5), i32le(b, 9));
            let ts = u16le(b, 21);
            Message::Location(Location {
                status: f >> 4,
                height_type: (f >> 2) & 1,
                direction,
                speed,
                vspeed,
                lat,
                lon,
                alt_baro: alt_decode(u16le(b, 13)),
                alt_geo: alt_decode(u16le(b, 15)),
                height: alt_decode(u16le(b, 17)),
                h_acc: b[19] & 15,
                v_acc: b[19] >> 4,
                baro_acc: b[20] >> 4,
                speed_acc: b[20] & 15,
                timestamp: (ts != 0xFFFF && ts <= 36000).then_some(ts),
                ts_acc: b[23] & 15,
            })
        }
        2 => {
            let page = b[1] & 15;
            if page == 0 {
                Message::Authentication(Authentication {
                    auth_type: b[1] >> 4,
                    page,
                    last_page: Some(b[2] & 15),
                    length: Some(b[3]),
                    timestamp: Some(u32le(b, 4)),
                    data: b[8..25].to_vec(),
                })
            } else {
                Message::Authentication(Authentication {
                    auth_type: b[1] >> 4,
                    page,
                    last_page: None,
                    length: None,
                    timestamp: None,
                    data: b[2..25].to_vec(),
                })
            }
        }
        3 => Message::SelfId(SelfId {
            desc_type: b[1],
            text: ascii_text(&b[2..25]),
        }),
        4 => {
            let (op_lat, op_lon) = latlon(i32le(b, 2), i32le(b, 6));
            Message::System(System {
                class_type: (b[1] >> 2) & 7,
                loc_type: b[1] & 3,
                op_lat,
                op_lon,
                area_count: u16le(b, 10),
                area_radius: b[12] as f64 * 10.0,
                area_ceiling: alt_decode(u16le(b, 13)),
                area_floor: alt_decode(u16le(b, 15)),
                category: b[17] >> 4,
                class: b[17] & 15,
                op_alt: alt_decode(u16le(b, 18)),
                timestamp: u32le(b, 20),
            })
        }
        5 => Message::OperatorId(OperatorId {
            id_type: b[1],
            text: ascii_text(&b[2..22]),
        }),
        _ => return Err(Malformed::Reserved),
    };
    Ok(Versioned { version, msg })
}

/// Decodes a message or a Message Pack: every message that decodes (reserved types inside a pack are skipped, a pack inside a pack
/// is skipped). A malformed single message or pack returns an error and no messages.
pub fn decode_payload(b: &[u8]) -> Result<Vec<Versioned>, Malformed> {
    if b.is_empty() {
        return Err(Malformed::Short);
    }
    if b[0] >> 4 == 0xF {
        if b.len() < 3 {
            return Err(Malformed::BadPack);
        }
        let size = b[1] as usize;
        let n = b[2] as usize;
        if size != MSG_LEN || !(1..=10).contains(&n) || b.len() < 3 + n * MSG_LEN {
            return Err(Malformed::BadPack);
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let m = &b[3 + i * MSG_LEN..3 + (i + 1) * MSG_LEN];
            if m[0] >> 4 == 0xF {
                continue;
            }
            if let Ok(v) = decode_message(m) {
                out.push(v);
            }
        }
        Ok(out)
    } else {
        decode_message(b).map(|m| vec![m])
    }
}

fn put_text(b: &mut [u8], s: &str) {
    for (o, c) in b.iter_mut().zip(s.bytes()) {
        *o = c;
    }
}

/// Encodes one message (version `version`).
pub fn encode_message(m: &Message, version: u8) -> [u8; MSG_LEN] {
    let mut b = [0u8; MSG_LEN];
    b[0] = (m.kind().code() << 4) | (version & 15);
    match m {
        Message::BasicId(x) => {
            b[1] = (x.id_type << 4) | (x.ua_type & 15);
            if x.id_type == 3 {
                let hex: String = x.id.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                for i in 0..16.min(hex.len() / 2) {
                    b[2 + i] = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap_or(0);
                }
            } else {
                put_text(&mut b[2..22], &x.id);
            }
        }
        Message::Location(x) => {
            let (dir_raw, ew) = match x.direction {
                None => (181u8, true),
                Some(d) => {
                    let d = (d.round() as i64).rem_euclid(360) as u32;
                    if d >= 180 {
                        ((d - 180) as u8, true)
                    } else {
                        (d as u8, false)
                    }
                }
            };
            let (sp_raw, mult) = match x.speed {
                None => (255u8, true),
                Some(s) if s <= 63.75 => ((s / 0.25).round().clamp(0.0, 255.0) as u8, false),
                Some(s) => (((s - 63.75) / 0.75).round().clamp(0.0, 254.0) as u8, true),
            };
            b[1] = (x.status << 4) | ((x.height_type & 1) << 2) | ((ew as u8) << 1) | mult as u8;
            b[2] = dir_raw;
            b[3] = sp_raw;
            b[4] = match x.vspeed {
                None => 126,
                Some(v) => ((v.clamp(-62.0, 62.0) / 0.5).round() as i8) as u8,
            };
            let (la, lo) = match (x.lat, x.lon) {
                (Some(a), Some(o)) => ((a * 1e7).round() as i32, (o * 1e7).round() as i32),
                _ => (0, 0),
            };
            b[5..9].copy_from_slice(&la.to_le_bytes());
            b[9..13].copy_from_slice(&lo.to_le_bytes());
            b[13..15].copy_from_slice(&alt_encode(x.alt_baro).to_le_bytes());
            b[15..17].copy_from_slice(&alt_encode(x.alt_geo).to_le_bytes());
            b[17..19].copy_from_slice(&alt_encode(x.height).to_le_bytes());
            b[19] = (x.v_acc << 4) | (x.h_acc & 15);
            b[20] = (x.baro_acc << 4) | (x.speed_acc & 15);
            b[21..23].copy_from_slice(&x.timestamp.unwrap_or(0xFFFF).to_le_bytes());
            b[23] = x.ts_acc & 15;
        }
        Message::Authentication(x) => {
            b[1] = (x.auth_type << 4) | (x.page & 15);
            if x.page == 0 {
                b[2] = x.last_page.unwrap_or(0) & 15;
                b[3] = x.length.unwrap_or(0);
                b[4..8].copy_from_slice(&x.timestamp.unwrap_or(0).to_le_bytes());
                for (o, d) in b[8..25].iter_mut().zip(&x.data) {
                    *o = *d;
                }
            } else {
                for (o, d) in b[2..25].iter_mut().zip(&x.data) {
                    *o = *d;
                }
            }
        }
        Message::SelfId(x) => {
            b[1] = x.desc_type;
            put_text(&mut b[2..25], &x.text);
        }
        Message::System(x) => {
            b[1] = ((x.class_type & 7) << 2) | (x.loc_type & 3);
            let (la, lo) = match (x.op_lat, x.op_lon) {
                (Some(a), Some(o)) => ((a * 1e7).round() as i32, (o * 1e7).round() as i32),
                _ => (0, 0),
            };
            b[2..6].copy_from_slice(&la.to_le_bytes());
            b[6..10].copy_from_slice(&lo.to_le_bytes());
            b[10..12].copy_from_slice(&x.area_count.to_le_bytes());
            b[12] = (x.area_radius / 10.0).round().clamp(0.0, 255.0) as u8;
            b[13..15].copy_from_slice(&alt_encode(x.area_ceiling).to_le_bytes());
            b[15..17].copy_from_slice(&alt_encode(x.area_floor).to_le_bytes());
            b[17] = (x.category << 4) | (x.class & 15);
            b[18..20].copy_from_slice(&alt_encode(x.op_alt).to_le_bytes());
            b[20..24].copy_from_slice(&x.timestamp.to_le_bytes());
        }
        Message::OperatorId(x) => {
            b[1] = x.id_type;
            put_text(&mut b[2..22], &x.text);
        }
    }
    b
}

/// Encodes a Message Pack of `msgs` (version `version`).
pub fn encode_pack(msgs: &[Message], version: u8) -> Vec<u8> {
    let mut v = vec![0xF0 | (version & 15), MSG_LEN as u8, msgs.len() as u8];
    for m in msgs {
        v.extend_from_slice(&encode_message(m, version));
    }
    v
}

/// The one-line summary of a message.
pub fn summary(m: &Message) -> String {
    match m {
        Message::BasicId(x) => format!(
            "Basic ID: {} {}, {}",
            id_type_name(x.id_type),
            x.id,
            UA_TYPES[x.ua_type as usize & 15]
        ),
        Message::Location(x) => match (x.lat, x.lon) {
            (Some(a), Some(o)) => format!(
                "Location: {a:.6} {o:.6}, {} m geo, height {} m, {:.1} m/s, {}, {}",
                x.alt_geo.map_or("-".into(), |v| format!("{v:.0}")),
                x.height.map_or("-".into(), |v| format!("{v:.0}")),
                x.speed.unwrap_or(0.0),
                x.direction
                    .map_or("track unknown".into(), |d| format!("{d:.0} deg")),
                status_name(x.status)
            ),
            _ => format!("Location: no position, {}", status_name(x.status)),
        },
        Message::Authentication(x) => format!(
            "Authentication: {}, page {}",
            auth_type_name(x.auth_type),
            x.page
        ),
        Message::SelfId(x) => format!("Self-ID: {}", x.text),
        Message::System(x) => match (x.op_lat, x.op_lon) {
            (Some(a), Some(o)) => format!(
                "System: operator {a:.6} {o:.6} ({}), {} {}",
                location_type_name(x.loc_type),
                category_name(x.category),
                class_name(x.class)
            ),
            _ => "System: no operator position".into(),
        },
        Message::OperatorId(x) => format!("Operator ID: {}", x.text),
    }
}

sdrkit_core::choice_enum! {
    /// Where a payload came from.
    #[derive(Serialize, Deserialize)]
    pub enum Transport {
        /// Bluetooth LE advertising.
        Ble = ("ble", "BLE"),
        /// Wi-Fi beacon or probe response.
        WiFi = ("wifi", "Wi-Fi"),
    }
}

impl Transport {
    /// "BLE" or "Wi-Fi".
    pub fn name(self) -> &'static str {
        match self {
            Transport::Ble => "BLE",
            Transport::WiFi => "Wi-Fi",
        }
    }
}

/// The Remote ID in a carrier: the counter and the messages.
#[derive(Clone, Debug, PartialEq)]
pub struct Carried {
    /// The message counter.
    pub counter: u8,
    /// The messages, in order.
    pub messages: Vec<Versioned>,
    /// The first message or pack as received, header byte first, counter removed.
    pub payload: Vec<u8>,
}

fn carried(found: &mut Option<Carried>, s: &[u8]) {
    if s.len() < 2 + MSG_LEN || s[0] != APP_CODE {
        return;
    }
    if let Ok(msgs) = decode_payload(&s[2..]) {
        if msgs.is_empty() {
            return;
        }
        match found {
            Some(f) => f.messages.extend(msgs),
            None => {
                *found = Some(Carried {
                    counter: s[1],
                    messages: msgs,
                    payload: s[2..].to_vec(),
                })
            }
        }
    }
}

/// The Remote ID in Bluetooth LE advertising data (AD structures): every service data structure of UUID 0xFFFA with the
/// application code. `None` when there is none.
pub fn from_ble_adv_data(ad: &[u8]) -> Option<Carried> {
    let mut found = None;
    for s in sdrkit_proto_wlan_phy::ble::ad::service_data16(ad, BLE_UUID) {
        carried(&mut found, s);
    }
    found
}

/// The Remote ID in an 802.11 beacon or probe response (FCS removed): the transmitter address and what it carries. `None` for
/// another frame or a frame without Remote ID.
pub fn from_wifi_frame(f: &[u8]) -> Option<(String, Carried)> {
    let frame = sdrkit_proto_wlan_phy::mac::Frame::parse(f).ok()?;
    let beacon = frame.beacon()?;
    let ta = mac_text(&frame.addr2?);
    let mut found = None;
    // The vendor element data after the OUI and type start with the counter, so add the type back in front.
    for d in sdrkit_proto_wlan_phy::mac::vendor_elements(beacon.elements, WIFI_OUI, APP_CODE) {
        let mut s = [0u8; 256];
        s[0] = APP_CODE;
        s[1..1 + d.len()].copy_from_slice(d);
        carried(&mut found, &s[..1 + d.len()]);
    }
    found.map(|c| (ta, c))
}

/// The BLE service data AD structure that carries a message or a pack: `len 16 FA FF 0D counter payload`.
pub fn ble_service_data(counter: u8, payload: &[u8]) -> Vec<u8> {
    let mut data = vec![APP_CODE, counter];
    data.extend_from_slice(payload);
    let mut v = Vec::with_capacity(data.len() + 4);
    sdrkit_proto_wlan_phy::ble::ad::push_service_data16(&mut v, BLE_UUID, &data)
        .expect("a Remote ID payload fits a service data structure");
    v
}

/// The Wi-Fi vendor element that carries a pack: `DD len FA 0B BC 0D counter pack`.
pub fn wifi_element(counter: u8, pack: &[u8]) -> Vec<u8> {
    let mut data = vec![counter];
    data.extend_from_slice(pack);
    let mut v = Vec::with_capacity(data.len() + 6);
    sdrkit_proto_wlan_phy::mac::push_vendor(&mut v, WIFI_OUI, APP_CODE, &data)
        .expect("a Message Pack fits a vendor element");
    v
}

/// An 802.11 beacon (without FCS) from `bssid` with an SSID, the rates 1, 2, 5.5 and 11 Mbit/s, channel 6 and the elements
/// `extra`.
pub fn beacon(bssid: [u8; 6], ssid: &str, seq: u16, extra: &[u8]) -> Vec<u8> {
    use sdrkit_proto_wlan_phy::mac;
    let mut f = Vec::with_capacity(64 + extra.len());
    mac::push_mgmt_header(
        &mut f,
        mac::MgmtSubtype::Beacon,
        mac::BROADCAST,
        bssid,
        bssid,
        seq,
    );
    mac::push_beacon_fields(&mut f, 0, 100, mac::CAP_ESS | 0x0020);
    let _ = mac::push_element(&mut f, 0, &ssid.as_bytes()[..ssid.len().min(32)]);
    let _ = mac::push_element(&mut f, 1, &[0x82, 0x84, 0x8B, 0x96]);
    let _ = mac::push_element(&mut f, 3, &[6]);
    f.extend_from_slice(extra);
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap())
            .collect()
    }

    fn pad(mut v: Vec<u8>) -> Vec<u8> {
        v.resize(25, 0);
        v
    }

    #[test]
    fn r1_basic_id() {
        let mut b = vec![0x02, 0x12];
        b.extend_from_slice(b"1581F5FKD229400B");
        let v = decode_message(&pad(b)).unwrap();
        assert_eq!(v.version, 2);
        let Message::BasicId(x) = v.msg else { panic!() };
        assert_eq!((x.id_type, x.ua_type), (1, 2));
        assert_eq!(id_type_name(x.id_type), "serial number");
        assert_eq!(UA_TYPES[x.ua_type as usize], "multirotor / helicopter");
        assert_eq!(x.id, "1581F5FKD229400B");
    }

    #[test]
    fn r2_location() {
        let b = hex("12 20 5A 22 03 C0 9A 98 20 48 60 11 0F 9D 09 BA 09 C0 08 4A 33 39 30 02 00");
        let Message::Location(x) = decode_message(&b).unwrap().msg else {
            panic!()
        };
        assert_eq!(x.status, 2);
        assert_eq!(x.height_type, 0);
        assert_eq!(x.direction, Some(90.0));
        assert_eq!(x.speed, Some(8.5));
        assert_eq!(x.vspeed, Some(1.5));
        assert!((x.lat.unwrap() - 54.6872).abs() < 1e-6 && (x.lon.unwrap() - 25.2797).abs() < 1e-6);
        assert_eq!(
            (x.alt_baro, x.alt_geo, x.height),
            (Some(230.5), Some(245.0), Some(120.0))
        );
        assert_eq!((x.h_acc, x.v_acc, x.baro_acc, x.speed_acc), (10, 4, 3, 3));
        assert_eq!(horiz_acc_name(x.h_acc), "< 10 m");
        assert_eq!(vert_acc_name(x.v_acc), "< 10 m");
        assert_eq!(speed_acc_name(x.speed_acc), "< 1 m/s");
        assert_eq!((x.timestamp, x.ts_acc), (Some(12345), 2));
        // Encoding returns the same bytes.
        assert_eq!(encode_message(&Message::Location(x), 2).to_vec(), b);
    }

    #[test]
    fn r3_location_with_ew_bit_and_multiplier() {
        let b = hex("12 27 5F 08 7E A8 92 9A 20 10 81 14 0F 04 0B 18 0B 28 0A 5B 04 58 02 00 00");
        let Message::Location(x) = decode_message(&b).unwrap().msg else {
            panic!()
        };
        assert_eq!(x.direction, Some(275.0));
        assert_eq!(x.speed, Some(69.75));
        assert_eq!(x.vspeed, None);
        assert_eq!(x.height_type, 1);
        assert!((x.lat.unwrap() - 54.7001).abs() < 1e-6 && (x.lon.unwrap() - 25.3002).abs() < 1e-6);
        assert_eq!(x.height, Some(300.0));
        assert_eq!(x.timestamp, Some(600));
    }

    #[test]
    fn r4_location_unknowns() {
        let mut b = pad(hex("12 03 B5 FF 7E"));
        b[21] = 0xFF;
        b[22] = 0xFF;
        let Message::Location(x) = decode_message(&b).unwrap().msg else {
            panic!()
        };
        assert_eq!(
            (x.lat, x.direction, x.speed, x.vspeed),
            (None, None, None, None)
        );
        assert_eq!((x.alt_geo, x.timestamp), (None, None));
        assert_eq!(
            summary(&Message::Location(x)),
            "Location: no position, undeclared"
        );
    }

    #[test]
    fn r5_system() {
        let b = hex("42 05 E0 6B 98 20 D0 F6 10 0F 01 00 00 00 00 00 00 12 CC 08 40 67 9A 0E 00");
        let Message::System(x) = decode_message(&b).unwrap().msg else {
            panic!()
        };
        assert_eq!(location_type_name(x.loc_type), "live GNSS");
        assert_eq!(classification_name(x.class_type), "EU");
        assert!(
            (x.op_lat.unwrap() - 54.686).abs() < 1e-6 && (x.op_lon.unwrap() - 25.277).abs() < 1e-6
        );
        assert_eq!(x.op_alt, Some(126.0));
        assert_eq!(x.area_count, 1);
        assert_eq!(
            (category_name(x.category), class_name(x.class).as_str()),
            ("open", "C1")
        );
        assert_eq!(x.timestamp, 245_000_000);
    }

    #[test]
    fn r6_texts_and_authentication() {
        let mut s = vec![0x32, 0x00];
        s.extend_from_slice(b"Survey flight");
        let Message::SelfId(x) = decode_message(&pad(s)).unwrap().msg else {
            panic!()
        };
        assert_eq!(x.text, "Survey flight");
        let mut o = vec![0x52, 0x00];
        o.extend_from_slice(b"LTUabcd1234efgh");
        let Message::OperatorId(x) = decode_message(&pad(o)).unwrap().msg else {
            panic!()
        };
        assert_eq!(x.text, "LTUabcd1234efgh");
        let mut a = hex("22 10 02 33 40 67 9A 0E");
        a.extend(8u8..=0x18);
        let Message::Authentication(x) = decode_message(&a).unwrap().msg else {
            panic!()
        };
        assert_eq!(
            (x.auth_type, x.page, x.last_page, x.length),
            (1, 0, Some(2), Some(0x33))
        );
        assert_eq!(x.timestamp, Some(245_000_000));
        assert_eq!(x.data.len(), 17);
        assert_eq!(x.data[0], 8);
    }

    #[test]
    fn r7_reserved_and_short() {
        let b = pad(vec![0x72, 1, 2, 3]);
        assert_eq!(decode_message(&b), Err(Malformed::Reserved));
        assert_eq!(decode_payload(&b), Err(Malformed::Reserved));
        assert_eq!(decode_payload(&b[..10]), Err(Malformed::Short));
        let mut pack = vec![0xF2, 25, 2];
        pack.extend_from_slice(&b);
        pack.extend_from_slice(&encode_message(
            &Message::SelfId(SelfId {
                desc_type: 0,
                text: "x".into(),
            }),
            2,
        ));
        assert_eq!(decode_payload(&pack).unwrap().len(), 1);
        assert_eq!(decode_payload(&pack[..40]), Err(Malformed::BadPack));
    }

    fn r8_messages() -> Vec<Message> {
        vec![
            Message::BasicId(BasicId {
                id_type: 4,
                ua_type: 6,
                id: "abc-123".into(),
            }),
            Message::Location(Location {
                status: 3,
                height_type: 0,
                direction: Some(359.0),
                speed: Some(200.0),
                vspeed: Some(-4.0),
                lat: Some(-33.8688),
                lon: Some(151.2093),
                alt_geo: Some(50.0),
                height: Some(12.5),
                timestamp: Some(35999),
                ..Default::default()
            }),
            Message::System(System {
                loc_type: 2,
                op_lat: Some(-33.87),
                op_lon: Some(151.2),
                area_count: 3,
                area_radius: 250.0,
                area_ceiling: Some(120.0),
                area_floor: Some(0.0),
                ..Default::default()
            }),
            Message::OperatorId(OperatorId {
                id_type: 0,
                text: "OP-1".into(),
            }),
        ]
    }

    #[test]
    fn r8_pack_round_trip() {
        let msgs = r8_messages();
        let p = encode_pack(&msgs, 2);
        assert_eq!(&p[..3], &[0xF2, 0x19, 0x04]);
        assert_eq!(p.len(), 103);
        let back = decode_payload(&p).unwrap();
        assert_eq!(back.len(), 4);
        for (a, b) in msgs.iter().zip(&back) {
            match (a, &b.msg) {
                (Message::Location(x), Message::Location(y)) => {
                    assert!((x.speed.unwrap() - y.speed.unwrap()).abs() <= 0.4);
                    assert_eq!(x.direction, y.direction);
                    assert_eq!(x.vspeed, y.vspeed);
                    assert!((x.lat.unwrap() - y.lat.unwrap()).abs() < 1e-7);
                    assert_eq!(
                        (x.alt_geo, x.height, x.timestamp, x.status),
                        (y.alt_geo, y.height, y.timestamp, y.status)
                    );
                }
                (Message::System(x), Message::System(y)) => {
                    assert_eq!(
                        (x.area_count, x.area_radius, x.area_ceiling, x.area_floor),
                        (y.area_count, y.area_radius, y.area_ceiling, y.area_floor)
                    );
                    assert!((x.op_lat.unwrap() - y.op_lat.unwrap()).abs() < 1e-7);
                    assert_eq!(x.loc_type, y.loc_type);
                }
                (a, b) => assert_eq!(a, b),
            }
        }
    }

    #[test]
    fn r9_pack_in_a_beacon() {
        let p = encode_pack(&r8_messages(), 2);
        let el = wifi_element(7, &p);
        assert_eq!(&el[..7], &[0xDD, 108, 0xFA, 0x0B, 0xBC, 0x0D, 0x07]);
        let b = beacon([0x60, 0x60, 0x1F, 0xAA, 0xBB, 0xCC], "RID", 1, &el);
        let (ta, c) = from_wifi_frame(&b).unwrap();
        assert_eq!(ta, "60:60:1F:AA:BB:CC");
        assert_eq!((c.counter, c.messages.len()), (7, 4));
        let plain = beacon([0x60, 0x60, 0x1F, 0xAA, 0xBB, 0xCC], "RID", 1, &[]);
        assert!(from_wifi_frame(&plain).is_none());
    }

    #[test]
    fn r10_ble_service_data() {
        let loc = encode_message(&r8_messages()[1], 2);
        let sd = ble_service_data(42, &loc);
        assert_eq!(sd.len(), 31);
        assert_eq!(&sd[..6], &[0x1E, 0x16, 0xFA, 0xFF, 0x0D, 0x2A]);
        let mut ad = vec![2, 1, 6];
        ad.extend_from_slice(&sd);
        let c = from_ble_adv_data(&ad).unwrap();
        assert_eq!((c.counter, c.messages.len()), (42, 1));
        assert_eq!(c.messages[0].msg.kind(), MsgType::Location);
        assert!(from_ble_adv_data(&hex("05 16 0F 18 0D 01")).is_none());
    }
}
