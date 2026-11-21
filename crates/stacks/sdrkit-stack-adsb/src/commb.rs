//! Comm-B (DF20, DF21): the register (BDS) an MB field holds, inferred from its content with the plausibility checks of the
//! spec, and the register layouts.
//!
//! Standard: ICAO Doc 9871 Appendix A

use crate::codes::{ac13_ft, callsign, callsign_defined};
use sdrkit_proto_aero::air::isa_ft;

/// A decoded Comm-B register.
#[derive(Clone, Debug, PartialEq)]
pub enum Bds {
    /// 1,0 data link capability.
    B10 {
        /// Mode S subnetwork version.
        subnetwork_version: u8,
        /// ACAS generates resolution advisories.
        acas_ra: bool,
    },
    /// 1,7 common usage GICB capability.
    B17 {
        /// The 24 capability bits (bit 1 the most significant).
        mask: u32,
    },
    /// 2,0 aircraft identification.
    B20 {
        /// Callsign.
        callsign: crate::codes::Callsign,
    },
    /// 3,0 ACAS active resolution advisory.
    B30 {
        /// Active resolution advisories (14 bits).
        ara: u16,
        /// RA complements (4 bits).
        rac: u8,
        /// RA terminated.
        terminated: bool,
        /// Multiple threat encounter.
        mte: bool,
        /// Threat type indicator.
        tti: u8,
        /// Threat address (TTI 1).
        threat: Option<u32>,
        /// Threat altitude, range (NM) and bearing (deg) (TTI 2).
        threat_position: Option<(i32, f64, f64)>,
    },
    /// 4,0 selected vertical intention.
    B40 {
        /// MCP/FCU selected altitude.
        mcp_ft: Option<i32>,
        /// FMS selected altitude.
        fms_ft: Option<i32>,
        /// Barometric setting.
        baro_hpa: Option<f64>,
    },
    /// 4,4 meteorological routine report.
    B44 {
        /// Figure of merit.
        fom: u8,
        /// Wind speed and direction it blows from.
        wind: Option<(f64, f64)>,
        /// Static air temperature.
        sat_c: f64,
        /// Static pressure.
        pressure_hpa: Option<f64>,
        /// Turbulence 0 to 3.
        turbulence: Option<u8>,
        /// Humidity, %.
        humidity_pct: Option<f64>,
    },
    /// 4,5 meteorological hazard report.
    B45 {
        /// Turbulence, wind shear, microburst, icing, wake vortex (0 nil to 3 severe), each when reported.
        hazards: [Option<u8>; 5],
        /// Static air temperature.
        sat_c: f64,
        /// Static pressure.
        pressure_hpa: Option<f64>,
        /// Radio height.
        radio_height_ft: Option<i32>,
    },
    /// 5,0 track and turn report.
    B50 {
        /// Roll, deg (right wing down positive).
        roll_deg: Option<f64>,
        /// True track 0 to 360.
        track_deg: Option<f64>,
        /// Ground speed.
        gs_kt: Option<f64>,
        /// Track angle rate, deg/s.
        track_rate_dps: Option<f64>,
        /// True airspeed.
        tas_kt: Option<f64>,
    },
    /// 6,0 heading and speed report.
    B60 {
        /// Magnetic heading 0 to 360.
        heading_deg: Option<f64>,
        /// Indicated airspeed.
        ias_kt: Option<f64>,
        /// Mach.
        mach: Option<f64>,
        /// Barometric altitude rate, ft/min.
        baro_rate_fpm: Option<i32>,
        /// Inertial vertical rate, ft/min.
        inertial_rate_fpm: Option<i32>,
    },
}

impl Bds {
    /// "2,0", "4,4" ...
    pub fn id(&self) -> &'static str {
        match self {
            Bds::B10 { .. } => "1,0",
            Bds::B17 { .. } => "1,7",
            Bds::B20 { .. } => "2,0",
            Bds::B30 { .. } => "3,0",
            Bds::B40 { .. } => "4,0",
            Bds::B44 { .. } => "4,4",
            Bds::B45 { .. } => "4,5",
            Bds::B50 { .. } => "5,0",
            Bds::B60 { .. } => "6,0",
        }
    }
}

/// The registers of the 1,7 capability bits, bit 1 first (Doc 9871 table A-2-23).
pub const GICB_REGISTERS: [&str; 24] = [
    "0,5", "0,6", "0,7", "0,8", "0,9", "0,A", "2,0", "2,1", "4,0", "4,1", "4,2", "4,3", "4,4",
    "4,5", "4,8", "5,0", "5,1", "5,2", "5,3", "5,4", "5,5", "5,6", "5,F", "6,0",
];

/// The registers a 1,7 mask announces.
pub fn gicb_registers(mask: u32) -> Vec<&'static str> {
    (0..24)
        .filter(|i| mask >> (23 - i) & 1 == 1)
        .map(|i| GICB_REGISTERS[i])
        .collect()
}

struct Mb(u64);

impl Mb {
    fn f(&self, a: u32, b: u32) -> u64 {
        (self.0 >> (56 - b)) & ((1u64 << (b - a + 1)) - 1)
    }
    fn bit(&self, a: u32) -> bool {
        self.f(a, a) == 1
    }
    /// Two's complement value of bits a..=b.
    fn signed(&self, a: u32, b: u32) -> i64 {
        let n = b - a + 1;
        let v = self.f(a, b) as i64;
        if v >= 1 << (n - 1) {
            v - (1 << n)
        } else {
            v
        }
    }
    /// A status bit at `s` with its value bits `a..=b`: `Some(present)` when consistent (a value whose status is 0 is all zeros).
    fn status(&self, s: u32, a: u32, b: u32) -> Option<bool> {
        if self.bit(s) {
            Some(true)
        } else if self.f(a, b) == 0 {
            Some(false)
        } else {
            None
        }
    }
}

fn b10(m: &Mb) -> Option<Bds> {
    if m.f(1, 8) != 0x10 || m.f(10, 14) != 0 {
        return None;
    }
    let v = m.f(17, 23) as u8;
    (v <= 5).then_some(Bds::B10 {
        subnetwork_version: v,
        acas_ra: m.bit(38),
    })
}

fn b17(m: &Mb) -> Option<Bds> {
    let mask = m.f(1, 24) as u32;
    (m.f(25, 56) == 0 && m.bit(7) && mask != 0).then_some(Bds::B17 { mask })
}

fn b20(m: &Mb) -> Option<Bds> {
    if m.f(1, 8) != 0x20 || !callsign_defined(m.f(9, 56)) {
        return None;
    }
    let cs = callsign(m.f(9, 56));
    (!cs.is_empty()).then_some(Bds::B20 { callsign: cs })
}

fn b30(m: &Mb) -> Option<Bds> {
    if m.f(1, 8) != 0x30 {
        return None;
    }
    let ara = m.f(9, 22) as u16;
    let rac = m.f(23, 26) as u8;
    let rat = m.bit(27);
    let mte = m.bit(28);
    let tti = m.f(29, 30) as u8;
    let mut threat = None;
    let mut threat_position = None;
    match tti {
        3 => return None,
        1 => {
            let a = m.f(31, 54) as u32;
            if m.f(55, 56) != 0 || a == 0 {
                return None;
            }
            threat = Some(a);
        }
        0 => {
            if m.f(31, 56) != 0 {
                return None;
            }
        }
        _ => {
            let alt = ac13_ft(m.f(31, 43) as u32)?;
            let range = m.f(44, 50);
            let brg = m.f(51, 56);
            if !(1..=60).contains(&brg) || range == 0 {
                return None;
            }
            threat_position = Some((alt, (range as f64 - 1.0) / 10.0, (brg as f64 - 1.0) * 6.0));
        }
    }
    if ara == 0 && !rat {
        return None;
    }
    let one = ara & (1 << 13) != 0; // ARA bit 9
    if one {
        if m.f(16, 22) != 0 || mte {
            return None;
        }
    } else if ara != 0 && !mte {
        return None;
    }
    if rac & 0b1100 == 0b1100 || rac & 0b0011 != 0 {
        return None;
    }
    Some(Bds::B30 {
        ara,
        rac,
        terminated: rat,
        mte,
        tti,
        threat,
        threat_position,
    })
}

fn b40(m: &Mb) -> Option<Bds> {
    let s1 = m.status(1, 2, 13)?;
    let s2 = m.status(14, 15, 26)?;
    let s3 = m.status(27, 28, 39)?;
    m.status(48, 49, 51)?;
    m.status(54, 55, 56)?;
    if m.f(40, 47) != 0 || m.f(52, 53) != 0 {
        return None;
    }
    if (s1 as u8 + s2 as u8 + s3 as u8) < 2 {
        return None;
    }
    let baro = s3.then(|| 800.0 + 0.1 * m.f(28, 39) as f64);
    if let Some(b) = baro {
        if !(900.0..=1100.0).contains(&b) {
            return None;
        }
    }
    Some(Bds::B40 {
        mcp_ft: s1.then(|| m.f(2, 13) as i32 * 16),
        fms_ft: s2.then(|| m.f(15, 26) as i32 * 16),
        baro_hpa: baro,
    })
}

fn b50(m: &Mb) -> Option<Bds> {
    let s = [
        m.status(1, 2, 11)?,
        m.status(12, 13, 23)?,
        m.status(24, 25, 34)?,
        m.status(35, 36, 45)?,
        m.status(46, 47, 56)?,
    ];
    if s.iter().filter(|&&x| x).count() < 4 {
        return None;
    }
    let roll = s[0].then(|| m.signed(2, 11) as f64 * 45.0 / 256.0);
    let track = s[1].then(|| {
        let t = m.signed(13, 23) as f64 * 90.0 / 512.0;
        if t < 0.0 {
            t + 360.0
        } else {
            t
        }
    });
    let gs = s[2].then(|| m.f(25, 34) as f64 * 2.0);
    let rate = s[3].then(|| m.signed(36, 45) as f64 * 8.0 / 256.0);
    let tas = s[4].then(|| m.f(47, 56) as f64 * 2.0);
    if roll.is_some_and(|r| r.abs() > 50.0)
        || gs.is_some_and(|g| g > 600.0)
        || tas.is_some_and(|t| t > 500.0)
    {
        return None;
    }
    if let (Some(g), Some(t)) = (gs, tas) {
        if (g - t).abs() > 200.0 {
            return None;
        }
    }
    if let (Some(r), Some(w), Some(t)) = (roll, rate, tas) {
        if t > 40.0 {
            let ct = (9.806_65 * r.to_radians().tan() / (t * 0.514_444)).to_degrees();
            if (w - ct).abs() > 1.0 {
                return None;
            }
        }
    }
    Some(Bds::B50 {
        roll_deg: roll,
        track_deg: track,
        gs_kt: gs,
        track_rate_dps: rate,
        tas_kt: tas,
    })
}

fn b60(m: &Mb) -> Option<Bds> {
    let s = [
        m.status(1, 2, 12)?,
        m.status(13, 14, 23)?,
        m.status(24, 25, 34)?,
        m.status(35, 36, 45)?,
        m.status(46, 47, 56)?,
    ];
    if s.iter().filter(|&&x| x).count() < 4 {
        return None;
    }
    let hdg = s[0].then(|| {
        let h = m.signed(2, 12) as f64 * 90.0 / 512.0;
        if h < 0.0 {
            h + 360.0
        } else {
            h
        }
    });
    let ias = s[1].then(|| m.f(14, 23) as f64);
    let mach = s[2].then(|| m.f(25, 34) as f64 * 0.004);
    let br = s[3].then(|| m.signed(36, 45) as i32 * 32);
    let ir = s[4].then(|| m.signed(47, 56) as i32 * 32);
    if ias.is_some_and(|v| v > 500.0) || mach.is_some_and(|v| v > 1.0) {
        return None;
    }
    if br.is_some_and(|v| v.abs() > 6000) || ir.is_some_and(|v| v.abs() > 6000) {
        return None;
    }
    if let (Some(i), Some(mc)) = (ias, mach) {
        if i > mc * 661.5 + 25.0 {
            return None;
        }
    }
    Some(Bds::B60 {
        heading_deg: hdg,
        ias_kt: ias,
        mach,
        baro_rate_fpm: br,
        inertial_rate_fpm: ir,
    })
}

fn isa_ok(alt_ft: Option<i32>, sat: f64, p: Option<f64>) -> bool {
    let Some(isa) = alt_ft.and_then(|a| isa_ft(a as f64).ok()) else {
        return false;
    };
    if (sat - isa.temp_c).abs() > 35.0 {
        return false;
    }
    if let Some(p) = p {
        let isa = isa.pressure_hpa;
        if (p - isa).abs() > 10.0 + 0.04 * isa {
            return false;
        }
    }
    true
}

fn b44(m: &Mb, alt: Option<i32>) -> Option<Bds> {
    let fom = m.f(1, 4) as u8;
    if !(1..=4).contains(&fom) || !m.bit(5) {
        return None;
    }
    let speed = m.f(6, 14) as f64;
    let dir = m.f(15, 23) as f64 * 180.0 / 256.0;
    if speed > 250.0 {
        return None;
    }
    if m.f(24, 34) == 0 {
        return None;
    }
    let sat = m.signed(24, 34) as f64 * 0.25;
    if !(-80.0..=60.0).contains(&sat) {
        return None;
    }
    let ps = m.status(35, 36, 46)?;
    let ts = m.status(47, 48, 49)?;
    let hs = m.status(50, 51, 56)?;
    let p = ps.then(|| m.f(36, 46) as f64);
    if p.is_some_and(|p| !(100.0..=1100.0).contains(&p)) {
        return None;
    }
    let h = hs.then(|| m.f(51, 56) as f64 * 100.0 / 64.0);
    if h.is_some_and(|h| h > 100.0) {
        return None;
    }
    if !isa_ok(alt, sat, p) {
        return None;
    }
    Some(Bds::B44 {
        fom,
        wind: Some((speed, dir)),
        sat_c: sat,
        pressure_hpa: p,
        turbulence: ts.then(|| m.f(48, 49) as u8),
        humidity_pct: h,
    })
}

fn b45(m: &Mb, alt: Option<i32>) -> Option<Bds> {
    if m.f(52, 56) != 0 {
        return None;
    }
    let mut hazards = [None; 5];
    for (k, h) in hazards.iter_mut().enumerate() {
        let s = 1 + 3 * k as u32;
        if m.status(s, s + 1, s + 2)? {
            *h = Some(m.f(s + 1, s + 2) as u8);
        }
    }
    if hazards.iter().all(|h| h.is_none()) {
        return None;
    }
    if !m.bit(16) {
        return None;
    }
    let sat = m.signed(17, 26) as f64 * 0.25;
    if !(-80.0..=60.0).contains(&sat) {
        return None;
    }
    let ps = m.status(27, 28, 38)?;
    let p = ps.then(|| m.f(28, 38) as f64);
    if p.is_some_and(|p| !(100.0..=1100.0).contains(&p)) {
        return None;
    }
    let rs = m.status(39, 40, 51)?;
    let rh = rs.then(|| m.f(40, 51) as i32 * 16);
    if let Some(r) = rh {
        if r > 5000 || alt.is_some_and(|a| r > a + 2000) {
            return None;
        }
    }
    if !isa_ok(alt, sat, p) {
        return None;
    }
    Some(Bds::B45 {
        hazards,
        sat_c: sat,
        pressure_hpa: p,
        radio_height_ft: rh,
    })
}

/// Infers the register of an MB field. `alt_ft` is the reply's altitude (for the weather registers): the AC altitude of a DF20, or
/// the aircraft's recent barometric altitude for a DF21.
pub fn infer(mb: u64, alt_ft: Option<i32>) -> Option<Bds> {
    if mb == 0 {
        return None;
    }
    let m = Mb(mb);
    let mut common = [b20(&m), b40(&m), b50(&m), b60(&m)].into_iter().flatten();
    let first = common.next();
    if common.next().is_some() {
        return None;
    }
    let fixed = b10(&m).is_some() || b30(&m).is_some();
    if let Some(c) = first {
        match &c {
            Bds::B40 { baro_hpa: None, .. } if b17(&m).is_some() => return None,
            Bds::B40 { .. } | Bds::B50 { .. } | Bds::B60 { .. } if fixed => return None,
            _ => {}
        }
        return Some(c);
    }
    let mut others = [b10(&m), b17(&m), b30(&m), b44(&m, alt_ft), b45(&m, alt_ft)]
        .into_iter()
        .flatten();
    let first = others.next();
    if others.next().is_none() {
        first
    } else {
        None
    }
}

/// The advisory text of a 3,0 register (for screens).
pub fn advisory_text(ara: u16, rac: u8, terminated: bool) -> String {
    let bit = |n: u32| ara & (1 << (22 - n)) != 0; // ARA bits are MB bits 9 to 22
    let mut s = if terminated {
        "terminated".to_string()
    } else if bit(9) {
        let mut s = String::from(if bit(11) { "descend" } else { "climb" });
        s.push_str(if bit(10) {
            ", corrective"
        } else {
            ", preventive"
        });
        for (b, t) in [
            (12, ", increase rate"),
            (13, ", sense reversal"),
            (14, ", crossing"),
            (15, ", positive"),
        ] {
            if bit(b) {
                s.push_str(t);
            }
        }
        s
    } else if ara != 0 {
        "multiple threats".to_string()
    } else {
        "advisory".to_string()
    };
    for (i, t) in [
        (3, " (no descend)"),
        (2, " (no climb)"),
        (1, " (no left)"),
        (0, " (no right)"),
    ] {
        if rac >> i & 1 == 1 {
            s.push_str(t);
        }
    }
    s
}

/// A short description of a register's content for the message monitor.
pub fn describe(b: &Bds) -> String {
    match b {
        Bds::B10 {
            subnetwork_version,
            acas_ra,
        } => {
            format!(
                "capability, subnetwork version {subnetwork_version}{}",
                if *acas_ra { ", ACAS RA" } else { "" }
            )
        }
        Bds::B17 { mask } => format!("GICB {}", gicb_registers(*mask).join(" ")),
        Bds::B20 { callsign } => format!("callsign {callsign}"),
        Bds::B30 {
            ara,
            rac,
            terminated,
            threat,
            ..
        } => {
            let mut s = format!("RA {}", advisory_text(*ara, *rac, *terminated));
            if let Some(t) = threat {
                s.push_str(&format!(", threat {t:06X}"));
            }
            s
        }
        Bds::B40 {
            mcp_ft,
            fms_ft,
            baro_hpa,
        } => {
            let mut v = Vec::new();
            if let Some(a) = mcp_ft {
                v.push(format!("MCP {a} ft"));
            }
            if let Some(a) = fms_ft {
                v.push(format!("FMS {a} ft"));
            }
            if let Some(b) = baro_hpa {
                v.push(format!("{b:.1} hPa"));
            }
            v.join(", ")
        }
        Bds::B44 {
            wind,
            sat_c,
            pressure_hpa,
            ..
        } => {
            let mut s = String::new();
            if let Some((sp, d)) = wind {
                s.push_str(&format!("wind {d:.0}/{sp:.0} kt, "));
            }
            s.push_str(&format!("SAT {sat_c:+.2} C"));
            if let Some(p) = pressure_hpa {
                s.push_str(&format!(", {p:.0} hPa"));
            }
            s
        }
        Bds::B45 { hazards, sat_c, .. } => {
            let names = [
                "turbulence",
                "wind shear",
                "microburst",
                "icing",
                "wake vortex",
            ];
            let mut v: Vec<String> = hazards
                .iter()
                .zip(names)
                .filter_map(|(h, n)| h.map(|l| format!("{n} {l}")))
                .collect();
            v.push(format!("SAT {sat_c:+.2} C"));
            v.join(", ")
        }
        Bds::B50 {
            roll_deg,
            track_deg,
            gs_kt,
            tas_kt,
            ..
        } => {
            let mut v = Vec::new();
            if let Some(r) = roll_deg {
                v.push(format!("roll {r:.1}"));
            }
            if let Some(t) = track_deg {
                v.push(format!("track {t:.1}"));
            }
            if let Some(g) = gs_kt {
                v.push(format!("GS {g:.0} kt"));
            }
            if let Some(t) = tas_kt {
                v.push(format!("TAS {t:.0} kt"));
            }
            v.join(", ")
        }
        Bds::B60 {
            heading_deg,
            ias_kt,
            mach,
            ..
        } => {
            let mut v = Vec::new();
            if let Some(h) = heading_deg {
                v.push(format!("heading {h:.1}"));
            }
            if let Some(i) = ias_kt {
                v.push(format!("IAS {i:.0} kt"));
            }
            if let Some(m) = mach {
                v.push(format!("Mach {m:.3}"));
            }
            v.join(", ")
        }
    }
}

// ----- encoders, for the generator and tests -----

fn put(v: &mut u64, a: u32, b: u32, x: u64) {
    let n = b - a + 1;
    let mask = ((1u64 << n) - 1) << (56 - b);
    *v = (*v & !mask) | ((x << (56 - b)) & mask);
}

fn tc(x: i64, bits: u32) -> u64 {
    (x as u64) & ((1u64 << bits) - 1)
}

/// Encodes BDS 2,0.
pub fn encode_20(cs: &str) -> u64 {
    let mut v = 0;
    put(&mut v, 1, 8, 0x20);
    put(&mut v, 9, 56, crate::codes::encode_callsign(cs));
    v
}

/// Encodes BDS 4,0 (MCP, FMS altitudes in ft, baro in hPa).
pub fn encode_40(mcp_ft: Option<i32>, fms_ft: Option<i32>, baro_hpa: Option<f64>) -> u64 {
    let mut v = 0;
    if let Some(a) = mcp_ft {
        put(&mut v, 1, 1, 1);
        put(&mut v, 2, 13, (a / 16).max(0) as u64);
    }
    if let Some(a) = fms_ft {
        put(&mut v, 14, 14, 1);
        put(&mut v, 15, 26, (a / 16).max(0) as u64);
    }
    if let Some(b) = baro_hpa {
        put(&mut v, 27, 27, 1);
        put(&mut v, 28, 39, ((b - 800.0) * 10.0).round().max(0.0) as u64);
    }
    v
}

/// Encodes BDS 5,0.
pub fn encode_50(roll: f64, track: f64, gs: f64, rate: f64, tas: f64) -> u64 {
    let mut v = 0;
    put(&mut v, 1, 1, 1);
    put(&mut v, 2, 11, tc((roll * 256.0 / 45.0).round() as i64, 10));
    let t = if track > 180.0 { track - 360.0 } else { track };
    put(&mut v, 12, 12, 1);
    put(&mut v, 13, 23, tc((t * 512.0 / 90.0).round() as i64, 11));
    put(&mut v, 24, 24, 1);
    put(&mut v, 25, 34, ((gs / 2.0).round() as u64).min(1023));
    put(&mut v, 35, 35, 1);
    put(&mut v, 36, 45, tc((rate * 256.0 / 8.0).round() as i64, 10));
    put(&mut v, 46, 46, 1);
    put(&mut v, 47, 56, ((tas / 2.0).round() as u64).min(1023));
    v
}

/// Encodes BDS 6,0.
pub fn encode_60(heading: f64, ias: f64, mach: f64, baro_rate: i32, inertial_rate: i32) -> u64 {
    let mut v = 0;
    let h = if heading > 180.0 {
        heading - 360.0
    } else {
        heading
    };
    put(&mut v, 1, 1, 1);
    put(&mut v, 2, 12, tc((h * 512.0 / 90.0).round() as i64, 11));
    put(&mut v, 13, 13, 1);
    put(&mut v, 14, 23, (ias.round() as u64).min(1023));
    put(&mut v, 24, 24, 1);
    put(&mut v, 25, 34, ((mach / 0.004).round() as u64).min(1023));
    put(&mut v, 35, 35, 1);
    put(&mut v, 36, 45, tc((baro_rate / 32) as i64, 10));
    put(&mut v, 46, 46, 1);
    put(&mut v, 47, 56, tc((inertial_rate / 32) as i64, 10));
    v
}

/// Encodes BDS 4,4 (FOM 1, wind speed kt and direction, SAT, pressure, turbulence, humidity).
pub fn encode_44(
    wind_kt: f64,
    wind_from: f64,
    sat: f64,
    pressure: Option<f64>,
    turb: Option<u8>,
    hum: Option<f64>,
) -> u64 {
    let mut v = 0;
    put(&mut v, 1, 4, 1);
    put(&mut v, 5, 5, 1);
    put(&mut v, 6, 14, wind_kt.round() as u64);
    put(
        &mut v,
        15,
        23,
        ((wind_from * 256.0 / 180.0).round() as u64) % 512,
    );
    put(&mut v, 24, 34, tc((sat / 0.25).round() as i64, 11));
    if let Some(p) = pressure {
        put(&mut v, 35, 35, 1);
        put(&mut v, 36, 46, p.round() as u64);
    }
    if let Some(t) = turb {
        put(&mut v, 47, 47, 1);
        put(&mut v, 48, 49, t as u64);
    }
    if let Some(h) = hum {
        put(&mut v, 50, 50, 1);
        put(&mut v, 51, 56, ((h * 64.0 / 100.0).round() as u64).min(63));
    }
    v
}

/// Encodes BDS 4,5.
pub fn encode_45(
    hazards: [Option<u8>; 5],
    sat: f64,
    pressure: Option<f64>,
    rh_ft: Option<i32>,
) -> u64 {
    let mut v = 0;
    for (k, h) in hazards.iter().enumerate() {
        if let Some(l) = h {
            let s = 1 + 3 * k as u32;
            put(&mut v, s, s, 1);
            put(&mut v, s + 1, s + 2, *l as u64);
        }
    }
    put(&mut v, 16, 16, 1);
    put(&mut v, 17, 26, tc((sat / 0.25).round() as i64, 10));
    if let Some(p) = pressure {
        put(&mut v, 27, 27, 1);
        put(&mut v, 28, 38, p.round() as u64);
    }
    if let Some(r) = rh_ft {
        put(&mut v, 39, 39, 1);
        put(&mut v, 40, 51, (r / 16) as u64);
    }
    v
}

/// Encodes BDS 3,0 with one threat given by address.
pub fn encode_30(ara: u16, rac: u8, terminated: bool, threat: Option<u32>) -> u64 {
    let mut v = 0;
    put(&mut v, 1, 8, 0x30);
    put(&mut v, 9, 22, ara as u64);
    put(&mut v, 23, 26, rac as u64);
    put(&mut v, 27, 27, terminated as u64);
    if let Some(t) = threat {
        put(&mut v, 29, 30, 1);
        put(&mut v, 31, 54, t as u64);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Frame;

    fn mb(hex: &str) -> u64 {
        Frame::from_hex(hex).unwrap().me()
    }

    #[test]
    fn published_registers() {
        assert_eq!(
            infer(mb("A000083E202CC371C31DE0AA1CCF"), Some(12550)),
            Some(Bds::B20 {
                callsign: crate::codes::Callsign::try_from_str("KLM1017").unwrap()
            })
        );
        match infer(mb("A8001EBCAEE57730A80106DE1344"), None).unwrap() {
            Bds::B40 {
                mcp_ft,
                fms_ft,
                baro_hpa,
            } => {
                assert_eq!((mcp_ft, fms_ft), (Some(24000), Some(24000)));
                assert!((baro_hpa.unwrap() - 1013.2).abs() < 0.05);
            }
            b => panic!("{b:?}"),
        }
        match infer(mb("A80006ACF9363D3BBF9CE98F1E1D"), None).unwrap() {
            Bds::B50 {
                roll_deg,
                track_deg,
                gs_kt,
                track_rate_dps,
                tas_kt,
            } => {
                assert!((roll_deg.unwrap() + 9.668).abs() < 0.01);
                assert!((track_deg.unwrap() - 140.273).abs() < 0.001);
                assert_eq!(
                    (gs_kt, track_rate_dps, tas_kt),
                    (Some(476.0), Some(-0.40625), Some(466.0))
                );
            }
            b => panic!("{b:?}"),
        }
        match infer(mb("A80004AAA74A072BFDEFC1D5CB4F"), None).unwrap() {
            Bds::B60 {
                heading_deg,
                ias_kt,
                mach,
                baro_rate_fpm,
                inertial_rate_fpm,
            } => {
                assert!((heading_deg.unwrap() - 110.391).abs() < 0.001);
                assert_eq!(
                    (ias_kt, baro_rate_fpm, inertial_rate_fpm),
                    (Some(259.0), Some(-2144), Some(-2016))
                );
                assert!((mach.unwrap() - 0.7).abs() < 1e-9);
            }
            b => panic!("{b:?}"),
        }
    }

    #[test]
    fn constructed_registers() {
        match infer(mb("A000171818B71DCBE38ED30F8002"), Some(36000)).unwrap() {
            Bds::B44 {
                fom,
                wind,
                sat_c,
                pressure_hpa,
                turbulence,
                humidity_pct,
            } => {
                assert_eq!(
                    (fom, sat_c, pressure_hpa, turbulence),
                    (1, -52.25, Some(227.0), Some(1))
                );
                let (s, d) = wind.unwrap();
                assert!(s == 45.0 && (d - 280.0).abs() < 0.71);
                assert!((humidity_pct.unwrap() - 30.0).abs() < 0.8);
            }
            b => panic!("{b:?}"),
        }
        let m45 = mb("A0000318C05102ADAE0C8038A0DF");
        match infer(m45, Some(4000)).unwrap() {
            Bds::B45 {
                hazards,
                sat_c,
                pressure_hpa,
                radio_height_ft,
            } => {
                assert_eq!(hazards, [Some(2), None, None, Some(1), None]);
                assert_eq!(
                    (sat_c, pressure_hpa, radio_height_ft),
                    (2.5, Some(875.0), Some(1600))
                );
            }
            b => panic!("{b:?}"),
        }
        assert_eq!(infer(m45, None), None);
        assert_eq!(infer(m45, Some(30000)), None);
        match infer(mb("A000171830C0000625848CC8D40F"), Some(36000)).unwrap() {
            Bds::B30 {
                ara,
                rac,
                terminated,
                tti,
                threat,
                ..
            } => {
                assert_eq!((tti, threat, terminated), (1, Some(0x896123), false));
                assert_eq!(advisory_text(ara, rac, terminated), "climb, corrective");
            }
            b => panic!("{b:?}"),
        }
        assert_eq!(
            infer(mb("A000171810000680940000D418CF"), None),
            Some(Bds::B10 {
                subnetwork_version: 3,
                acas_ra: true
            })
        );
        assert_eq!(
            infer(mb("A000171802A10100000000633D0B"), None),
            Some(Bds::B17 { mask: 0x02A101 })
        );
    }

    #[test]
    fn look_alikes_are_refused() {
        let base = |w: f64| encode_44(w, 270.0, -52.25, Some(227.0), Some(1), Some(30.0));
        assert!(infer(base(45.0), Some(36000)).is_some());
        assert!(infer(base(300.0), Some(36000)).is_none());
        // pressure value without its status bit
        let mut v = base(45.0);
        put(&mut v, 35, 35, 0);
        assert!(infer(v, Some(36000)).is_none());
        let mut v = base(45.0);
        put(&mut v, 1, 4, 6);
        assert!(infer(v, Some(36000)).is_none());
        // +10 C at FL360
        assert!(infer(encode_44(45.0, 270.0, 10.0, None, None, None), Some(36000)).is_none());
        // a 4,5 at 30 000 ft with ground values
        assert!(infer(
            encode_45([Some(1), None, None, None, None], 15.0, Some(1010.0), None),
            Some(30000)
        )
        .is_none());
        // a reserved bit in 1,0
        let mut v = mb("A000171810000680940000D418CF");
        put(&mut v, 11, 11, 1);
        assert!(infer(v, None).is_none());
        // TTI 3
        let mut v = encode_30(1 << 13, 0, false, Some(0x123456));
        put(&mut v, 29, 30, 3);
        assert!(infer(v, None).is_none());
        // 4,5 without SAT
        let mut v = encode_45([Some(1), None, None, None, None], 2.5, None, None);
        put(&mut v, 16, 26, 0);
        assert!(infer(v, Some(4000)).is_none());
    }

    #[test]
    fn random_fields_are_rarely_taken() {
        let mut s = 0x9E37_79B9_7F4A_7C15u64;
        let mut n = 0;
        for i in 0..1_000_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let alt = if i % 2 == 0 {
                Some(((s >> 60) as i32) * 3000)
            } else {
                None
            };
            if infer(s & ((1 << 56) - 1), alt).is_some() {
                n += 1;
            }
        }
        assert!(n < 600, "{n}");
    }
}
