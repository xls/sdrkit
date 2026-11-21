//! Z-Wave MPDUs: parsing, the check octets, the command names and the one-line description of a frame.
//!
//! Standard: ITU-T G.9959 (MPDU format and checks)

use sdrkit_core::publish::field;
use sdrkit_proto_ookdev::reading::{measure, Measure};
use sdrkit_proto_ookdev::{Check, Reading};
use sdrkit_util_bits::checksum::xor8;
use sdrkit_util_bits::crc::catalog::CRC16_SPI_FUJITSU;
use sdrkit_util_bits::crc::Trailer;
use serde::{Deserialize, Serialize};

/// A parsed MPDU.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mpdu {
    /// Home id.
    pub home: u32,
    /// Source node.
    pub src: u8,
    /// Frame control 1.
    pub fc1: u8,
    /// Frame control 2.
    pub fc2: u8,
    /// Length octet.
    pub len: u8,
    /// Destination node (singlecast, ack, routed), 0 for multicast.
    pub dst: u8,
    /// Multicast members.
    pub members: Vec<u16>,
    /// The application payload (command class, command, parameters).
    pub payload: Vec<u8>,
}

impl Mpdu {
    /// Header type (1 singlecast, 2 multicast, 3 ack, 8 routed).
    pub fn header_type(&self) -> u8 {
        self.fc1 & 0x0F
    }
    /// Sequence number.
    pub fn seq(&self) -> u8 {
        self.fc2 & 0x0F
    }
    /// Ack requested.
    pub fn ack_req(&self) -> bool {
        self.fc1 & 0x40 != 0
    }
    /// Routed.
    pub fn routed(&self) -> bool {
        self.fc1 & 0x80 != 0 || self.header_type() == 8
    }
    /// The encryption ("none", "S0", "S2").
    pub fn encryption(&self) -> &'static str {
        match (self.payload.first(), self.payload.get(1)) {
            (Some(0x98), Some(0x81 | 0xC1)) => "S0",
            (Some(0x9F), Some(0x03)) => "S2",
            _ => "none",
        }
    }
    /// Header type name.
    pub fn type_name(&self) -> String {
        match self.header_type() {
            1 => "Singlecast".into(),
            2 => "Multicast".into(),
            3 => "Ack".into(),
            8 => "Routed".into(),
            n => format!("Type {n}"),
        }
    }
}

/// Whether the check octets of an MPDU at `rate` are right. R1 and R2 end with 0xFF XOR every octet before it. R3 ends with
/// CRC-16/SPI-FUJITSU, most significant octet first (the frame check sequence of ITU-T G.9959).
pub fn check(mpdu: &[u8], rate: u8) -> bool {
    let l = mpdu.len();
    if rate == 3 {
        l >= 3 && CRC16_SPI_FUJITSU.verify(mpdu, Trailer::Big)
    } else {
        l >= 2 && mpdu[l - 1] == 0xFF ^ xor8(&mpdu[..l - 1])
    }
}

/// Parses an MPDU (check octets included; `rate` determines their number). `None` when it is malformed.
pub fn parse(m: &[u8], rate: u8) -> Option<Mpdu> {
    let ck = if rate == 3 { 2 } else { 1 };
    if m.len() < 9 + ck || m[7] as usize != m.len() {
        return None;
    }
    let home = u32::from_be_bytes([m[0], m[1], m[2], m[3]]);
    let (fc1, fc2) = (m[5], m[6]);
    let end = m.len() - ck;
    let ht = fc1 & 0x0F;
    let (dst, members, pstart) = if ht == 2 {
        let ctl = m[8];
        let off = (ctl >> 5) as u16;
        let ml = (ctl & 0x1F) as usize;
        if !(1..=29).contains(&ml) || 9 + ml > end {
            return None;
        }
        let mut mem = Vec::new();
        for i in 0..ml {
            for b in 0..8 {
                if m[9 + i] & (1 << b) != 0 {
                    mem.push(off * 32 + 8 * i as u16 + b + 1);
                }
            }
        }
        (0, mem, 9 + ml)
    } else {
        (m[8], vec![], 9)
    };
    Some(Mpdu {
        home,
        src: m[4],
        fc1,
        fc2,
        len: m[7],
        dst,
        members,
        payload: m[pstart..end].to_vec(),
    })
}

/// Builds an MPDU from its fields: header, destination (or multicast control and mask), payload and the check for `rate`.
pub fn build(
    home: u32,
    src: u8,
    fc1: u8,
    fc2: u8,
    dst_or_mask: &[u8],
    payload: &[u8],
    rate: u8,
) -> Vec<u8> {
    let ck = if rate == 3 { 2 } else { 1 };
    let mut m = home.to_be_bytes().to_vec();
    m.push(src);
    m.push(fc1);
    m.push(fc2);
    m.push((8 + dst_or_mask.len() + payload.len() + ck) as u8);
    m.extend_from_slice(dst_or_mask);
    m.extend_from_slice(payload);
    if rate == 3 {
        let c = CRC16_SPI_FUJITSU.checksum(&m);
        m.extend_from_slice(&c.to_be_bytes());
    } else {
        m.push(0xFF ^ xor8(&m));
    }
    m
}

/// The command class name of the full list.
pub fn class_name(c: u8) -> Option<&'static str> {
    Some(match c {
        0x00 => "NOP",
        0x01 => "PROTOCOL",
        0x20 => "BASIC",
        0x21 => "CONTROLLER_REPLICATION",
        0x22 => "APPLICATION_STATUS",
        0x23 => "ZIP",
        0x24 => "SECURITY_PANEL_MODE",
        0x25 => "SWITCH_BINARY",
        0x26 => "SWITCH_MULTILEVEL",
        0x27 => "SWITCH_ALL",
        0x28 => "SWITCH_TOGGLE_BINARY",
        0x29 => "SWITCH_TOGGLE_MULTILEVEL",
        0x2B => "SCENE_ACTIVATION",
        0x2C => "SCENE_ACTUATOR_CONF",
        0x2D => "SCENE_CONTROLLER_CONF",
        0x2E => "SECURITY_PANEL_ZONE",
        0x2F => "SECURITY_PANEL_ZONE_SENSOR",
        0x30 => "SENSOR_BINARY",
        0x31 => "SENSOR_MULTILEVEL",
        0x32 => "METER",
        0x33 => "SWITCH_COLOR",
        0x34 => "NETWORK_MANAGEMENT_INCLUSION",
        0x35 => "METER_PULSE",
        0x36 => "BASIC_TARIFF_INFO",
        0x37 => "HRV_STATUS",
        0x38 => "THERMOSTAT_HEATING",
        0x39 => "HRV_CONTROL",
        0x3A => "DCP_CONFIG",
        0x3B => "DCP_MONITOR",
        0x3C => "METER_TBL_CONFIG",
        0x3D => "METER_TBL_MONITOR",
        0x3E => "METER_TBL_PUSH",
        0x3F => "PREPAYMENT",
        0x40 => "THERMOSTAT_MODE",
        0x41 => "PREPAYMENT_ENCAPSULATION",
        0x42 => "THERMOSTAT_OPERATING_STATE",
        0x43 => "THERMOSTAT_SETPOINT",
        0x44 => "THERMOSTAT_FAN_MODE",
        0x45 => "THERMOSTAT_FAN_STATE",
        0x46 => "CLIMATE_CONTROL_SCHEDULE",
        0x47 => "THERMOSTAT_SETBACK",
        0x4C => "DOOR_LOCK_LOGGING",
        0x4D => "NETWORK_MANAGEMENT_BASIC",
        0x4E => "SCHEDULE_ENTRY_LOCK",
        0x4F => "ZIP_6LOWPAN",
        0x50 => "BASIC_WINDOW_COVERING",
        0x51 => "MTP_WINDOW_COVERING",
        0x52 => "NETWORK_MANAGEMENT_PROXY",
        0x53 => "SCHEDULE",
        0x54 => "NETWORK_MANAGEMENT_PRIMARY",
        0x55 => "TRANSPORT_SERVICE",
        0x56 => "CRC_16_ENCAP",
        0x57 => "APPLICATION_CAPABILITY",
        0x58 => "ZIP_ND",
        0x59 => "ASSOCIATION_GRP_INFO",
        0x5A => "DEVICE_RESET_LOCALLY",
        0x5B => "CENTRAL_SCENE",
        0x5C => "IP_ASSOCIATION",
        0x5D => "ANTITHEFT",
        0x5E => "ZWAVEPLUS_INFO",
        0x5F => "ZIP_GATEWAY",
        0x60 => "MULTI_CHANNEL",
        0x61 => "ZIP_PORTAL",
        0x62 => "DOOR_LOCK",
        0x63 => "USER_CODE",
        0x64 => "HUMIDITY_CONTROL_SETPOINT",
        0x65 => "DMX",
        0x66 => "BARRIER_OPERATOR",
        0x67 => "NETWORK_MANAGEMENT_INSTALLATION_MAINTENANCE",
        0x68 => "ZIP_NAMING",
        0x69 => "MAILBOX",
        0x6A => "WINDOW_COVERING",
        0x6B => "IRRIGATION",
        0x6C => "SUPERVISION",
        0x6D => "HUMIDITY_CONTROL_MODE",
        0x6E => "HUMIDITY_CONTROL_OPERATING_STATE",
        0x6F => "ENTRY_CONTROL",
        0x70 => "CONFIGURATION",
        0x71 => "NOTIFICATION",
        0x72 => "MANUFACTURER_SPECIFIC",
        0x73 => "POWERLEVEL",
        0x74 => "INCLUSION_CONTROLLER",
        0x75 => "PROTECTION",
        0x76 => "LOCK",
        0x77 => "NODE_NAMING",
        0x78 => "NODE_PROVISIONING",
        0x79 => "SOUND_SWITCH",
        0x7A => "FIRMWARE_UPDATE_MD",
        0x7B => "GROUPING_NAME",
        0x7C => "REMOTE_ASSOCIATION_ACTIVATE",
        0x7D => "REMOTE_ASSOCIATION",
        0x80 => "BATTERY",
        0x81 => "CLOCK",
        0x82 => "HAIL",
        0x84 => "WAKE_UP",
        0x85 => "ASSOCIATION",
        0x86 => "VERSION",
        0x87 => "INDICATOR",
        0x88 => "PROPRIETARY",
        0x89 => "LANGUAGE",
        0x8A => "TIME",
        0x8B => "TIME_PARAMETERS",
        0x8C => "GEOGRAPHIC_LOCATION",
        0x8E => "MULTI_CHANNEL_ASSOCIATION",
        0x8F => "MULTI_CMD",
        0x90 => "ENERGY_PRODUCTION",
        0x91 => "MANUFACTURER_PROPRIETARY",
        0x92 => "SCREEN_MD",
        0x93 => "SCREEN_ATTRIBUTES",
        0x94 => "SIMPLE_AV_CONTROL",
        0x98 => "SECURITY",
        0x9A => "IP_CONFIGURATION",
        0x9B => "ASSOCIATION_COMMAND_CONFIGURATION",
        0x9C => "SENSOR_ALARM",
        0x9D => "SILENCE_ALARM",
        0x9E => "SENSOR_CONFIGURATION",
        0x9F => "SECURITY_2",
        0xEF => "MARK",
        0xF0 => "NON_INTEROPERABLE",
        _ => return None,
    })
}

/// The readable name the `ism` stack shows.
pub fn readable_class(c: u8) -> String {
    match c {
        0x00 => "No Operation",
        0x01 => "Z-Wave Protocol",
        0x20 => "Basic",
        0x25 => "Switch Binary",
        0x26 => "Switch Multilevel",
        0x30 => "Sensor Binary",
        0x31 => "Sensor Multilevel",
        0x32 => "Meter",
        0x56 => "CRC-16 Encap",
        0x59 => "Association Group Info",
        0x5E => "Z-Wave Plus Info",
        0x60 => "Multi Channel",
        0x70 => "Configuration",
        0x71 => "Notification",
        0x72 => "Manufacturer Specific",
        0x73 => "Powerlevel",
        0x7A => "Firmware Update",
        0x80 => "Battery",
        0x84 => "Wake Up",
        0x85 => "Association",
        0x86 => "Version",
        0x8E => "Multi Channel Association",
        0x98 => "Security S0",
        0x9F => "Security S2",
        _ => return format!("0x{c:02X}"),
    }
    .into()
}

/// The command name (`ism` adds a few commands of its own).
pub fn command_name(class: u8, cmd: u8, ism: bool) -> Option<&'static str> {
    Some(match (class, cmd) {
        (0x01, 0x01) => "Node Info",
        (0x01, 0x02) => "Request Node Info",
        (0x01, 0x03) => "Assign IDs",
        (0x01, 0x04) => "Find Nodes In Range",
        (0x01, 0x05) => "Get Nodes In Range",
        (0x01, 0x06) => "Range Info",
        (0x01, 0x07) => "Command Complete",
        (0x01, 0x0C) => "Assign Return Route",
        (0x01, 0x0D) => "New Node Registered",
        (0x01, 0x11) => "SUC Node ID",
        (0x01, 0x12) => "Set SUC",
        (0x01, 0x15) => "Static Route Request",
        (0x01, 0x16) => "Lost",
        (0x01, 0x18) => "NOP Power",
        (0x20 | 0x25 | 0x26, 1) => "Set",
        (0x20 | 0x25 | 0x26, 2) => "Get",
        (0x20 | 0x25 | 0x26, 3) => "Report",
        (0x26, 4) => "Start Level Change",
        (0x26, 5) => "Stop Level Change",
        (0x30, 2) => "Get",
        (0x30, 3) => "Report",
        (0x31, 4) => "Get",
        (0x31, 5) => "Report",
        (0x31, 1) if ism => "Supported Get",
        (0x31, 2) if ism => "Supported Report",
        (0x32, 1) => "Get",
        (0x32, 2) => "Report",
        (0x32, 3) if ism => "Supported Get",
        (0x32, 4) if ism => "Supported Report",
        (0x32, 5) if ism => "Reset",
        (0x62, 1) => "Operation Set",
        (0x62, 2) => "Operation Get",
        (0x62, 3) => "Operation Report",
        (0x70, 4) => "Set",
        (0x70, 5) => "Get",
        (0x70, 6) => "Report",
        (0x71, 4) => "Get",
        (0x71, 5) => "Report",
        (0x71, 7) if ism => "Supported Get",
        (0x71, 8) if ism => "Supported Report",
        (0x72 | 0x5E | 0x6C, 4) if class == 0x72 => "Get",
        (0x72, 5) => "Report",
        (0x5E | 0x6C, 1) => "Get",
        (0x5E | 0x6C, 2) => "Report",
        (0x80, 2) => "Get",
        (0x80, 3) => "Report",
        (0x84, 4) => "Interval Set",
        (0x84, 5) => "Interval Get",
        (0x84, 6) => "Interval Report",
        (0x84, 7) => "Notification",
        (0x84, 8) => "No More Information",
        (0x85, 1) => "Set",
        (0x85, 2) => "Get",
        (0x85, 3) => "Report",
        (0x85, 4) => "Remove",
        (0x86, 0x11) => "Get",
        (0x86, 0x12) => "Report",
        (0x98, 0x40) => "Nonce Get",
        (0x98, 0x80) => "Nonce Report",
        (0x98, 0x02) => "Commands Supported Get",
        (0x98, 0x03) => "Commands Supported Report",
        (0x98, 0x04) => "Scheme Get",
        (0x98, 0x05) => "Scheme Report",
        (0x98, 0x06) => "Network Key Set",
        (0x98, 0x07) => "Network Key Verify",
        (0x98, 0x08) => "Scheme Inherit",
        (0x9F, 1) => "Nonce Get",
        (0x9F, 2) => "Nonce Report",
        (0x9F, 3) => "Message Encapsulation",
        (0x9F, 4) => "KEX Get",
        (0x9F, 5) => "KEX Report",
        (0x9F, 6) => "KEX Set",
        (0x9F, 7) => "KEX Fail",
        (0x9F, 8) => "Public Key Report",
        (0x9F, 9) => "Network Key Get",
        (0x9F, 10) => "Network Key Report",
        (0x9F, 11) => "Network Key Verify",
        (0x9F, 12) => "Transfer End",
        (0x9F, 13) => "Commands Supported Get",
        (0x9F, 14) => "Commands Supported Report",
        (0x9F, 15) => "Capabilities Get",
        (0x9F, 16) => "Capabilities Report",
        _ => return None,
    })
}

/// A sensor or meter value from its precision/scale/size octet and the value octets that follow (big-endian signed).
pub fn level_value(p: &[u8]) -> Option<(f64, u8, u8)> {
    let lvl = *p.first()?;
    let prec = lvl >> 5;
    let scale = (lvl >> 3) & 3;
    let size = (lvl & 7) as usize;
    if !matches!(size, 1 | 2 | 4) || p.len() < 1 + size {
        return None;
    }
    let mut v: i64 = if p[1] & 0x80 != 0 { -1 } else { 0 };
    for &b in &p[1..1 + size] {
        v = (v << 8) | b as i64;
    }
    Some((v as f64 / 10f64.powi(prec as i32), prec, scale))
}

fn sensor_type_name(t: u8) -> &'static str {
    match t {
        1 => "temperature",
        2 => "general",
        3 => "luminance",
        4 => "power",
        5 => "humidity",
        6 => "velocity",
        7 => "direction",
        8 => "atmospheric pressure",
        9 => "barometric pressure",
        10 => "solar radiation",
        _ => "sensor",
    }
}

fn onoff(v: u8) -> String {
    match v {
        0 => " off".into(),
        0xFF => " on".into(),
        1..=99 => format!(" {v}%"),
        _ => format!(" {v}"),
    }
}

/// The one-line description of a frame.
pub fn describe(m: &Mpdu) -> String {
    if m.header_type() == 3 {
        return "ack".into();
    }
    if m.routed() {
        return "routed (route header not decoded)".into();
    }
    let n = m.payload.len();
    match m.encryption() {
        "S0" => return format!("Security S0 encrypted ({n} octets, not decoded)"),
        "S2" => return format!("Security S2 encrypted ({n} octets, not decoded)"),
        _ => {}
    }
    let p = &m.payload;
    let Some(&class) = p.first() else {
        return "empty".into();
    };
    if class == 0 {
        return "NOP".into();
    }
    let cname = class_name(class)
        .map(String::from)
        .unwrap_or_else(|| format!("command class 0x{class:02X}"));
    let Some(&cmd) = p.get(1) else { return cname };
    let cmdname = command_name(class, cmd, false)
        .map(String::from)
        .unwrap_or_else(|| format!("cmd 0x{cmd:02X}"));
    let mut s = format!("{cname} {cmdname}");
    match (class, cmd) {
        (0x20 | 0x25 | 0x26, 1 | 3) => {
            if let Some(&v) = p.get(2) {
                s.push_str(&onoff(v));
            }
        }
        (0x80, 3) => {
            if let Some(&v) = p.get(2) {
                s.push_str(&if v == 0xFF {
                    " low battery".into()
                } else {
                    format!(" {v}%")
                });
            }
        }
        (0x30, 3) => {
            if let Some(&v) = p.get(2) {
                s.push_str(if v != 0 { " triggered" } else { " idle" });
            }
        }
        (0x31, 5) => {
            if let (Some(&t), Some((v, prec, scale))) =
                (p.get(2), level_value(p.get(3..).unwrap_or(&[])))
            {
                s.push_str(&format!(" {} {:.*}", sensor_type_name(t), prec as usize, v));
                if t == 1 {
                    s.push_str(if scale == 1 { " F" } else { " C" });
                }
            }
        }
        _ => {}
    }
    s
}

/// The reading fields of a frame's command (Value, Level, Temperature ...), as (name, unit, value, decimals) or text.
pub fn command_fields(p: &[u8], mut r: Reading) -> Reading {
    let (Some(&class), Some(&cmd)) = (p.first(), p.get(1)) else {
        return r;
    };
    match (class, cmd) {
        (0x25, 1 | 3) => {
            if let Some(&v) = p.get(2) {
                r = match v {
                    0 => r.with(&measure::SWITCH_ON, false),
                    0xFF => r.with(&measure::SWITCH_ON, true),
                    v => r.num(&measure::VALUE, f64::from(v), 0),
                };
            }
        }
        (0x20 | 0x26, 1 | 3) => {
            if let Some(&v) = p.get(2) {
                r = if v <= 99 {
                    r.num(&measure::LEVEL_PCT, f64::from(v), 0)
                } else {
                    r.with(&measure::LEVEL, i64::from(v))
                };
            }
        }
        (0x30, 3) => {
            if let Some(&v) = p.get(2) {
                r = r.with(&measure::STATE, if v != 0 { "triggered" } else { "idle" });
            }
        }
        (0x80, 3) => {
            if let Some(&v) = p.get(2) {
                r = if v == 0xFF {
                    r.with_battery(false)
                } else {
                    r.num(&measure::BATTERY_LEVEL_PCT, f64::from(v), 0)
                };
            }
        }
        (0x84, 6) => {
            if p.len() >= 6 {
                let s = (p[2] as u32) << 16 | (p[3] as u32) << 8 | p[4] as u32;
                r = r.with(&measure::WAKE_UP_INTERVAL, f64::from(s));
            }
        }
        (0x71, 5) => {
            if p.len() >= 4 {
                r = r
                    .with(&measure::ALARM_TYPE, i64::from(p[2]))
                    .with(&measure::ALARM_LEVEL, i64::from(p[3]));
            }
        }
        (0x31, 5) => {
            if let (Some(&t), Some((v, prec, scale))) =
                (p.get(2), level_value(p.get(3..).unwrap_or(&[])))
            {
                let (m, v) = sensor_measure(t, scale, v);
                r = r.num(m, v, prec);
            }
        }
        (0x32, 2) => {
            if let (Some(&mt), Some((v, prec, scale))) =
                (p.get(2), level_value(p.get(3..).unwrap_or(&[])))
            {
                let m = match (mt & 0x1F, scale) {
                    (1, 2) => &measure::POWER_0_W,
                    _ => &measure::VALUE,
                };
                r = r.num(m, v, prec);
            }
        }
        _ => {}
    }
    r
}

/// The `ism` reading of a checked MPDU at `rate` (1, 2, 3): the home id and source as the id, the frame type, and the command
/// with its value when the payload is not encrypted.
pub fn ism_reading(mpdu: &[u8], rate: u8) -> Option<Reading> {
    let m = parse(mpdu, rate)?;
    let model = ["Z-Wave-R1", "Z-Wave-R2", "Z-Wave-R3"][(rate - 1) as usize];
    let check = if rate == 3 {
        Check::Crc
    } else {
        Check::Checksum
    };
    let mut r = Reading::new(model, "zwave", format!("{:08X}:{}", m.home, m.src), check)
        .with(&measure::HOME_ID, &format!("{:08X}", m.home))
        .with(&measure::SOURCE_NODE, i64::from(m.src));
    let destination = if m.header_type() == 2 {
        let list: Vec<String> = m.members.iter().map(|x| x.to_string()).collect();
        list.join(", ")
    } else if m.dst == 0xFF {
        "broadcast".to_string()
    } else {
        m.dst.to_string()
    };
    r = r
        .with(&measure::DESTINATION, &destination)
        .with(&measure::FRAME, &m.type_name())
        .with(&measure::SEQUENCE, i64::from(m.seq()))
        .with(&measure::ACK_REQUESTED, m.ack_req());
    if m.header_type() != 3 && !m.routed() {
        if let Some(&c) = m.payload.first() {
            r = r.with(&measure::COMMAND_CLASS, &readable_class(c));
            if let Some(&cmd) = m.payload.get(1) {
                if m.encryption() != "none" {
                    r = r.with(&measure::COMMAND, "encrypted");
                } else {
                    let name = command_name(c, cmd, true)
                        .map(String::from)
                        .unwrap_or_else(|| format!("cmd 0x{cmd:02X}"));
                    r = command_fields(&m.payload, r.with(&measure::COMMAND, &name));
                }
            }
        }
    }
    Some(r)
}

/// The result key of a value named `name` in `unit`: snake_case with the unit as a suffix ("temperature_c", "power_w").
/// The measurement of a multilevel sensor report of sensor type `t` and scale `scale`, with the value in the unit of the
/// measurement. A type that the catalogue has no measurement for is a plain value. ITU-T G.9959 does not define these:
/// Z-Wave Command Class Specification, Multilevel Sensor Command Class.
fn sensor_measure(t: u8, scale: u8, v: f64) -> (&'static Measure<field::F64>, f64) {
    match (t, scale) {
        (1, 0) => (&measure::TEMPERATURE_C, v),
        (1, _) => (&measure::TEMPERATURE_C, (v - 32.0) * 5.0 / 9.0),
        (3, 1) => (&measure::LIGHT_LUX, v),
        (4, 0) => (&measure::POWER_0_W, v),
        (5, 0) => (&measure::HUMIDITY_PCT, v),
        (8, 0) => (&measure::PRESSURE_KPA, v),
        (0x10, 0) => (&measure::CURRENT_A, v),
        (0x10, _) => (&measure::CURRENT_A, v / 1000.0),
        (0x11, _) => (&measure::CO2_PPM, v),
        (0x1B, _) => (&measure::UV_INDEX, v),
        _ => (&measure::VALUE, v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::{from_hex, hex};

    #[test]
    fn builds_and_parses() {
        let m = build(0xC0FFEE01, 1, 0x41, 1, &[2], &[0x25, 0x01, 0xFF], 2);
        assert_eq!(hex(&m), "C0FFEE010141010D022501FFBA");
        assert!(check(&m, 2));
        let p = parse(&m, 2).unwrap();
        assert_eq!(describe(&p), "SWITCH_BINARY Set on");
    }

    #[test]
    fn multicast_members() {
        let m = from_hex("C0FFEE0101020A0E01162501001B").unwrap();
        assert!(check(&m, 2));
        let p = parse(&m, 2).unwrap();
        assert_eq!(p.members, vec![2, 3, 5]);
        assert_eq!(describe(&p), "SWITCH_BINARY Set off");
    }
}
