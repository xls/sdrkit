//! UAT downlink payloads (header, state vector, mode status, auxiliary state vector) decoded and encoded, and the uplink frame
//! header with its information frames.
//!
//! Standard: RTCA DO-282B 2.2.4 and 2.2.3.2

use serde::Serialize;

/// Base-40 characters of the mode status callsign.
const BASE40: &[u8; 40] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ    ";

/// A decoded downlink payload.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Downlink {
    /// Payload type (MDB type), 0 to 31.
    pub payload_type: u8,
    /// Address qualifier (2 and 3 are TIS-B).
    pub address_qualifier: u8,
    /// 24-bit address.
    pub address: u32,
    /// TIS-B.
    pub tisb: bool,
    /// The state vector.
    pub sv: StateVector,
    /// Mode status (payload types 1 and 3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ms: Option<ModeStatus>,
    /// Auxiliary state vector: the secondary altitude and whether it is geometric (payload types 1, 2, 5, 6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aux_alt: Option<(i32, bool)>,
}

/// The state vector.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct StateVector {
    /// Latitude and longitude, when valid.
    pub pos: Option<(f64, f64)>,
    /// NIC.
    pub nic: u8,
    /// Altitude (ft) and whether it is geometric.
    pub alt: Option<(i32, bool)>,
    /// Air/ground state: 0 subsonic, 1 supersonic, 2 on the ground, 3 reserved.
    pub air_ground: u8,
    /// North and east velocity, kt (airborne).
    pub velocity: Option<(f64, f64)>,
    /// Ground speed, kt.
    pub speed_kt: Option<f64>,
    /// Track (airborne) or the ground track/heading.
    pub track_deg: Option<f64>,
    /// On the ground: 1 true track, 2 magnetic heading, 3 true heading.
    pub track_type: u8,
    /// Vertical rate, ft/min, and whether it is barometric.
    pub vrate: Option<(i32, bool)>,
    /// On the ground: length/width code.
    pub length_width: Option<u8>,
    /// TIS-B site id (address qualifier 2 or 3).
    pub tisb_site: Option<u8>,
    /// UTC coupled.
    pub utc_coupled: bool,
}

/// The mode status.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ModeStatus {
    /// Emitter category.
    pub category: u8,
    /// Eight characters (callsign or squawk), trailing spaces removed.
    pub ident: String,
    /// The characters are a callsign (not a squawk).
    pub is_callsign: bool,
    /// Emergency status.
    pub emergency: u8,
    /// UAT version.
    pub version: u8,
    /// SIL.
    pub sil: u8,
    /// Transmit MSO.
    pub mso: u8,
    /// NACp.
    pub nacp: u8,
    /// NACv.
    pub nacv: u8,
    /// NIC baro.
    pub nic_baro: bool,
    /// CDTI and ACAS capability.
    pub cdti: bool,
    /// ACAS installed.
    pub acas: bool,
    /// ACAS RA active.
    pub ra_active: bool,
    /// IDENT.
    pub ident_active: bool,
    /// ATC services.
    pub atc_services: bool,
    /// Heading is magnetic (else true).
    pub magnetic: bool,
}

fn alt12(raw: u32) -> Option<i32> {
    (raw != 0).then(|| (raw as i32 - 1) * 25 - 1000)
}

/// Decodes a payload (18 or 34 bytes).
pub fn decode_downlink(f: &[u8]) -> Option<Downlink> {
    if f.len() < 17 {
        return None;
    }
    let mut d = Downlink {
        payload_type: f[0] >> 3,
        address_qualifier: f[0] & 7,
        address: (f[1] as u32) << 16 | (f[2] as u32) << 8 | f[3] as u32,
        ..Default::default()
    };
    d.tisb = matches!(d.address_qualifier, 2 | 3);
    let sv = &mut d.sv;
    let raw_lat = (f[4] as u32) << 15 | (f[5] as u32) << 7 | (f[6] as u32) >> 1;
    let raw_lon =
        ((f[6] & 1) as u32) << 23 | (f[7] as u32) << 15 | (f[8] as u32) << 7 | (f[9] as u32) >> 1;
    sv.nic = f[11] & 15;
    if raw_lat != 0 || raw_lon != 0 || sv.nic != 0 {
        let mut lat = raw_lat as f64 * 360.0 / 16_777_216.0;
        if lat > 90.0 {
            lat -= 180.0;
        }
        let mut lon = raw_lon as f64 * 360.0 / 16_777_216.0;
        if lon > 180.0 {
            lon -= 360.0;
        }
        sv.pos = Some((lat, lon));
    }
    let raw_alt = (f[10] as u32) << 4 | (f[11] as u32) >> 4;
    sv.alt = alt12(raw_alt).map(|a| (a, f[9] & 1 == 1));
    sv.air_ground = f[12] >> 6;
    match sv.air_ground {
        0 | 1 => {
            let mult = if sv.air_ground == 1 { 4.0 } else { 1.0 };
            let raw_ns = ((f[12] & 0x1F) as u32) << 6 | (f[13] as u32) >> 2;
            let raw_ew = ((f[13] & 3) as u32) << 9 | (f[14] as u32) << 1 | (f[15] as u32) >> 7;
            let comp = |r: u32| {
                (r & 0x3FF != 0).then(|| {
                    ((r & 0x3FF) as f64 - 1.0) * mult * if r & 0x400 != 0 { -1.0 } else { 1.0 }
                })
            };
            if let (Some(n), Some(e)) = (comp(raw_ns), comp(raw_ew)) {
                sv.velocity = Some((n, e));
                sv.speed_kt = Some(n.hypot(e));
                let mut t = e.atan2(n).to_degrees();
                if t < 0.0 {
                    t += 360.0;
                }
                sv.track_deg = Some(t);
                sv.track_type = 1;
            }
            let raw_v = ((f[15] & 0x7F) as u32) << 4 | (f[16] as u32) >> 4;
            if raw_v & 0x1FF != 0 {
                let v = ((raw_v & 0x1FF) as i32 - 1) * 64 * if raw_v & 0x200 != 0 { -1 } else { 1 };
                sv.vrate = Some((v, raw_v & 0x400 != 0));
            }
        }
        2 => {
            let raw_gs = ((f[12] & 0x1F) as u32) << 6 | (f[13] as u32) >> 2;
            if raw_gs & 0x3FF != 0 {
                sv.speed_kt = Some((raw_gs & 0x3FF) as f64 - 1.0);
            }
            let raw_t = ((f[13] & 3) as u32) << 9 | (f[14] as u32) << 1 | (f[15] as u32) >> 7;
            sv.track_type = ((raw_t & 0x600) >> 9) as u8;
            if sv.track_type != 0 {
                sv.track_deg = Some((raw_t & 0x1FF) as f64 * 360.0 / 512.0);
            }
            sv.length_width = Some((f[15] & 0x78) >> 3);
        }
        _ => {}
    }
    if d.tisb {
        sv.tisb_site = Some(f[16] & 0x0F);
    } else {
        sv.utc_coupled = f[16] & 0x08 != 0;
    }
    if matches!(d.payload_type, 1 | 3) && f.len() >= 29 {
        let w = |i: usize| (f[i] as u32) << 8 | f[i + 1] as u32;
        let (a, b, c) = (w(17), w(19), w(21));
        let mut ident = String::with_capacity(8);
        for v in [
            a / 40 % 40,
            a % 40,
            b / 1600 % 40,
            b / 40 % 40,
            b % 40,
            c / 1600 % 40,
            c / 40 % 40,
            c % 40,
        ] {
            ident.push(BASE40[v as usize] as char);
        }
        ident.truncate(ident.trim_end().len());
        d.ms = Some(ModeStatus {
            category: (a / 1600 % 40) as u8,
            ident,
            is_callsign: f[26] & 0x02 != 0,
            emergency: (f[23] >> 5) & 7,
            version: (f[23] >> 2) & 7,
            sil: f[23] & 3,
            mso: (f[24] >> 2) & 0x3F,
            nacp: (f[25] >> 4) & 15,
            nacv: (f[25] >> 1) & 7,
            nic_baro: f[25] & 1 != 0,
            cdti: f[26] & 0x80 != 0,
            acas: f[26] & 0x40 != 0,
            ra_active: f[26] & 0x20 != 0,
            ident_active: f[26] & 0x10 != 0,
            atc_services: f[26] & 0x08 != 0,
            magnetic: f[26] & 0x04 != 0,
        });
    }
    if matches!(d.payload_type, 1 | 2 | 5 | 6) && f.len() >= 31 {
        let raw = (f[29] as u32) << 4 | (f[30] as u32) >> 4;
        let primary_geo = d.sv.alt.map(|a| a.1).unwrap_or(f[9] & 1 == 1);
        d.aux_alt = alt12(raw).map(|a| (a, !primary_geo));
    }
    Some(d)
}

/// What a simulated aircraft puts into a payload.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DownlinkState {
    /// Payload type.
    pub payload_type: u8,
    /// Address qualifier.
    pub address_qualifier: u8,
    /// Address.
    pub address: u32,
    /// Position.
    pub pos: Option<(f64, f64)>,
    /// NIC.
    pub nic: u8,
    /// Altitude and geometric flag.
    pub alt: Option<(i32, bool)>,
    /// North and east velocity (kt).
    pub velocity: Option<(f64, f64)>,
    /// On the ground: speed and true track.
    pub ground: Option<(f64, f64)>,
    /// Vertical rate, barometric.
    pub vrate: Option<i32>,
    /// Callsign (payload types 1, 3).
    pub callsign: Option<String>,
    /// Emitter category.
    pub category: u8,
    /// NACp.
    pub nacp: u8,
    /// Secondary altitude (payload types 1, 2, 5, 6).
    pub aux_alt: Option<i32>,
}

/// Encodes a payload (18 bytes for type 0, otherwise 34).
pub fn encode_downlink(s: &DownlinkState) -> Vec<u8> {
    let mut f = vec![0u8; if s.payload_type == 0 { 18 } else { 34 }];
    f[0] = s.payload_type << 3 | (s.address_qualifier & 7);
    f[1] = (s.address >> 16) as u8;
    f[2] = (s.address >> 8) as u8;
    f[3] = s.address as u8;
    if let Some((lat, lon)) = s.pos {
        let rl = ((if lat < 0.0 { lat + 180.0 } else { lat }) * 16_777_216.0 / 360.0).round()
            as u32
            & 0x7F_FFFF;
        let ro = ((if lon < 0.0 { lon + 360.0 } else { lon }) * 16_777_216.0 / 360.0).round()
            as u32
            & 0xFF_FFFF;
        f[4] = (rl >> 15) as u8;
        f[5] = (rl >> 7) as u8;
        f[6] = ((rl & 0x7F) << 1) as u8 | (ro >> 23) as u8;
        f[7] = (ro >> 15) as u8;
        f[8] = (ro >> 7) as u8;
        f[9] = ((ro & 0x7F) << 1) as u8;
    }
    let (raw_alt, geo) = match s.alt {
        Some((a, g)) => ((((a + 1000) as f64 / 25.0).round() as u32 + 1).min(4095), g),
        None => (0, false),
    };
    f[9] |= geo as u8;
    f[10] = (raw_alt >> 4) as u8;
    f[11] = ((raw_alt & 15) << 4) as u8 | (s.nic & 15);
    if let Some((gs, trk)) = s.ground {
        f[12] = 2 << 6;
        let raw_gs = (gs.round() as u32 + 1).min(1023);
        f[12] |= ((raw_gs >> 6) & 0x1F) as u8;
        f[13] = ((raw_gs & 0x3F) << 2) as u8;
        let raw_t = (1u32 << 9) | (((trk.rem_euclid(360.0)) * 512.0 / 360.0).round() as u32 % 512);
        f[13] |= ((raw_t >> 9) & 3) as u8;
        f[14] = (raw_t >> 1) as u8;
        f[15] = ((raw_t & 1) << 7) as u8;
    } else if let Some((n, e)) = s.velocity {
        let comp = |v: f64| -> u32 {
            (if v < 0.0 { 0x400 } else { 0 }) | (v.abs().round() as u32 + 1).min(1023)
        };
        let (rn, re) = (comp(n), comp(e));
        f[12] = ((rn >> 6) & 0x1F) as u8;
        f[13] = ((rn & 0x3F) << 2) as u8 | ((re >> 9) & 3) as u8;
        f[14] = (re >> 1) as u8;
        f[15] = ((re & 1) << 7) as u8;
        if let Some(v) = s.vrate {
            let raw = 0x400
                | (if v < 0 { 0x200 } else { 0 })
                | ((v.unsigned_abs() as f64 / 64.0).round() as u32 + 1).min(511);
            f[15] |= ((raw >> 4) & 0x7F) as u8;
            f[16] = ((raw & 15) << 4) as u8;
        }
    } else {
        f[12] = 3 << 6;
    }
    if matches!(s.payload_type, 1 | 3) {
        let cs = s.callsign.clone().unwrap_or_default();
        let mut c = [36u32; 8];
        for (i, ch) in cs.chars().take(8).enumerate() {
            c[i] = BASE40
                .iter()
                .position(|&b| b as char == ch.to_ascii_uppercase())
                .unwrap_or(36) as u32;
        }
        let a = s.category as u32 * 1600 + c[0] * 40 + c[1];
        let b = c[2] * 1600 + c[3] * 40 + c[4];
        let cc = c[5] * 1600 + c[6] * 40 + c[7];
        for (i, v) in [a, b, cc].into_iter().enumerate() {
            f[17 + 2 * i] = (v >> 8) as u8;
            f[18 + 2 * i] = v as u8;
        }
        f[23] = 2 << 2 | 3; // UAT version 2, SIL 3
        f[25] = (s.nacp & 15) << 4;
        f[26] = 0x02; // the characters are a callsign
    }
    if matches!(s.payload_type, 1 | 2 | 5 | 6) {
        if let Some(a) = s.aux_alt {
            let raw = ((((a + 1000) as f64) / 25.0).round() as u32 + 1).min(4095);
            f[29] = (raw >> 4) as u8;
            f[30] = ((raw & 15) << 4) as u8;
        }
    }
    f
}

/// The uplink frame header.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct UplinkHeader {
    /// Ground station position, when valid.
    pub pos: Option<(f64, f64)>,
    /// UTC coupled.
    pub utc_coupled: bool,
    /// Application data valid.
    pub app_data_valid: bool,
    /// Slot id.
    pub slot_id: u8,
    /// TIS-B site id.
    pub tisb_site: u8,
}

/// One information frame of an uplink.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InfoFrame {
    /// Type (4 bits).
    pub frame_type: u8,
    /// Its bytes.
    pub data: Vec<u8>,
}

/// Decodes an uplink frame (432 bytes): the header and its information frames.
pub fn decode_uplink(f: &[u8]) -> Option<(UplinkHeader, Vec<InfoFrame>)> {
    if f.len() < 8 {
        return None;
    }
    let raw_lat = (f[0] as u32) << 15 | (f[1] as u32) << 7 | (f[2] as u32) >> 1;
    let raw_lon =
        ((f[2] & 1) as u32) << 23 | (f[3] as u32) << 15 | (f[4] as u32) << 7 | (f[5] as u32) >> 1;
    let valid = f[5] & 1 != 0;
    let pos = valid.then(|| {
        let mut lat = raw_lat as f64 * 360.0 / 16_777_216.0;
        if lat > 90.0 {
            lat -= 180.0;
        }
        let mut lon = raw_lon as f64 * 360.0 / 16_777_216.0;
        if lon > 180.0 {
            lon -= 360.0;
        }
        (lat, lon)
    });
    let h = UplinkHeader {
        pos,
        utc_coupled: f[6] & 0x80 != 0,
        app_data_valid: f[6] & 0x20 != 0,
        slot_id: f[6] & 0x1F,
        tisb_site: f[7] >> 4,
    };
    let mut frames = Vec::new();
    let data = &f[8..f.len().min(432)];
    let mut i = 0;
    while i + 2 <= data.len() {
        let len = ((data[i] as usize) << 1) | (data[i + 1] as usize >> 7);
        let typ = data[i + 1] & 0x0F;
        if len == 0 || i + 2 + len > data.len() {
            break;
        }
        frames.push(InfoFrame {
            frame_type: typ,
            data: data[i + 2..i + 2 + len].to_vec(),
        });
        i += 2 + len;
    }
    Some((h, frames))
}

/// Encodes an uplink frame (432 bytes) from its header and information frames.
pub fn encode_uplink(h: &UplinkHeader, frames: &[InfoFrame]) -> Vec<u8> {
    let mut f = vec![0u8; 432];
    if let Some((lat, lon)) = h.pos {
        let rl = ((if lat < 0.0 { lat + 180.0 } else { lat }) * 16_777_216.0 / 360.0).round()
            as u32
            & 0x7F_FFFF;
        let ro = ((if lon < 0.0 { lon + 360.0 } else { lon }) * 16_777_216.0 / 360.0).round()
            as u32
            & 0xFF_FFFF;
        f[0] = (rl >> 15) as u8;
        f[1] = (rl >> 7) as u8;
        f[2] = ((rl & 0x7F) << 1) as u8 | (ro >> 23) as u8;
        f[3] = (ro >> 15) as u8;
        f[4] = (ro >> 7) as u8;
        f[5] = ((ro & 0x7F) << 1) as u8 | 1;
    }
    f[6] = (h.utc_coupled as u8) << 7 | (h.app_data_valid as u8) << 5 | (h.slot_id & 0x1F);
    f[7] = h.tisb_site << 4;
    let mut i = 8;
    for fr in frames {
        let len = fr.data.len();
        if i + 2 + len > 432 {
            break;
        }
        f[i] = (len >> 1) as u8;
        f[i + 1] = ((len & 1) << 7) as u8 | (fr.frame_type & 0x0F);
        f[i + 2..i + 2 + len].copy_from_slice(&fr.data);
        i += 2 + len;
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_payload() {
        let hex = "007E6987D2D74238C1453855F89980C7524F";
        let b: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let d = decode_downlink(&b).unwrap();
        assert_eq!(
            (d.payload_type, d.address_qualifier, d.address, d.tisb),
            (0, 0, 0x7E6987, false)
        );
        let (lat, lon) = d.sv.pos.unwrap();
        assert!((lat + 31.75252676010132).abs() < 1e-9 && (lon - 39.9058198928833).abs() < 1e-9);
        assert_eq!(d.sv.alt, Some((21500, true)));
        assert_eq!(
            (d.sv.nic, d.sv.air_ground, d.sv.velocity, d.sv.utc_coupled),
            (5, 3, None, false)
        );
    }

    #[test]
    fn payloads_round_trip() {
        let s = DownlinkState {
            payload_type: 1,
            address: 0xA12345,
            pos: Some((37.5, -122.25)),
            nic: 8,
            alt: Some((5500, false)),
            velocity: Some((-120.0, 80.0)),
            vrate: Some(-640),
            callsign: Some("N123AB".into()),
            category: 1,
            nacp: 9,
            aux_alt: Some(5650),
            ..Default::default()
        };
        let d = decode_downlink(&encode_downlink(&s)).unwrap();
        let (lat, lon) = d.sv.pos.unwrap();
        assert!((lat - 37.5).abs() < 1e-4 && (lon + 122.25).abs() < 1e-4);
        assert_eq!(d.sv.alt, Some((5500, false)));
        assert_eq!(d.sv.velocity, Some((-120.0, 80.0)));
        assert_eq!(d.sv.vrate, Some((-640, true)));
        let ms = d.ms.unwrap();
        assert_eq!(
            (ms.ident.as_str(), ms.category, ms.nacp, ms.is_callsign),
            ("N123AB", 1, 9, true)
        );
        assert_eq!(d.aux_alt, Some((5650, true)));
    }

    #[test]
    fn uplink_frames() {
        let h = UplinkHeader {
            pos: Some((40.0, -75.0)),
            utc_coupled: true,
            app_data_valid: true,
            slot_id: 5,
            tisb_site: 3,
        };
        let fr = vec![InfoFrame {
            frame_type: 0,
            data: vec![1; 10],
        }];
        let (h2, f2) = decode_uplink(&encode_uplink(&h, &fr)).unwrap();
        assert_eq!(h2.slot_id, 5);
        assert_eq!(f2, fr);
        let mut bad = encode_uplink(&h, &[]);
        bad[8] = 0xFF; // 510 bytes claimed
        bad[9] = 0x00;
        assert!(decode_uplink(&bad).unwrap().1.is_empty());
        assert!(decode_uplink(&[]).is_none());
    }
}
