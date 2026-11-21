//! The radiosonde test signal: a scene of up to 7 sondes around 403 MHz (or one LMS6 at 1680 MHz). Each sends its own frames
//! with valid checks, FEC and calibration, as GFSK. The RS41 sends bursts, the others send without a break.

use super::landing::moved;
use super::{dfm, lms6, m10, rs41, rs92};
use crate::calendar;
use sdrkit_algo_sim::modulate::{Cpm, Ramp};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::Scene;
use sdrkit_core::{Complex32, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_util_geo::ellipsoid::{Enu, Observer};
use sdrkit_util_geo::time::{gnss, LeapSeconds};
use sdrkit_util_geo::Position;

/// Unix time of the scene's first frame (2026-06-01 00:00:00 UTC).
pub const START_UTC: f64 = 1_780_272_000.0;

/// A sonde of the scene.
#[derive(Clone, Debug)]
pub struct SondeSpec {
    /// Family: "RS41", "RS41-SGP", "DFM-17", "DFM-09", "DFM-06", "M10", "M20", "RS92", "LMS6".
    pub model: String,
    /// Offset from the centre, Hz.
    pub offset_hz: f64,
    /// Start latitude, longitude, altitude.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Altitude, m.
    pub alt: f64,
    /// Vertical speed, m/s.
    pub vv: f64,
    /// Serial.
    pub serial: String,
    /// First frame number.
    pub frame0: u32,
    /// Time of the first burst after the start, s.
    pub phase_s: f64,
}

/// The scene table of 2.17 (centred on 403.0 MHz).
pub fn scene_table(second_rs41_offset: f64) -> Vec<SondeSpec> {
    let s = |model: &str, mhz: f64, lat, lon, km: f64, vv, serial: &str, ph| SondeSpec {
        model: model.into(),
        offset_hz: (mhz - 403.0) * 1e6,
        lat,
        lon,
        alt: km * 1000.0,
        vv,
        serial: serial.into(),
        frame0: 1000,
        phase_s: ph,
    };
    let mut v = vec![
        s("RS41", 403.0, 25.20, 55.36, 12.0, 5.0, "N4750123", 0.05),
        s("DFM-17", 402.2, 25.30, 55.50, 22.0, -15.0, "17012345", 0.0),
        s("M10", 404.1, 25.10, 55.20, 8.0, 5.0, "310-2-11329", 0.3),
        s("RS41", 403.45, 24.90, 55.10, 5.0, 5.0, "N4750456", 0.35),
        s("M20", 401.6, 25.00, 55.60, 15.0, 5.0, "211-4-01234", 0.5),
        s("DFM-09", 405.0, 25.40, 55.30, 3.0, 4.0, "19076543", 0.0),
        s("RS92", 405.3, 25.15, 55.45, 6.0, 5.0, "P4953934", 0.0),
    ];
    v[3].offset_hz = second_rs41_offset;
    v
}

/// The L-band sonde.
pub fn lband_sonde() -> SondeSpec {
    SondeSpec {
        model: "LMS6".into(),
        offset_hz: 0.0,
        lat: 46.77,
        lon: -100.77,
        alt: 12_000.0,
        vv: 5.0,
        serial: "77668800".into(),
        frame0: 1000,
        phase_s: 0.1,
    }
}

/// Plain standard atmosphere: temperature (°C) and humidity (%) at a height.
pub fn atmosphere(h: f64) -> (f64, f64) {
    let km = h / 1000.0;
    let t = if km <= 11.0 {
        15.0 - 6.5 * km
    } else if km <= 20.0 {
        -56.5
    } else {
        -56.5 + (km - 20.0)
    };
    let rh = if km < 11.0 {
        80.0 * (-km / 3.5).exp()
    } else {
        3.0
    };
    (t, rh)
}

/// The RS41 calibration block the generator sends.
pub fn rs41_calibration(freq_hz: f64, sgp: bool) -> Vec<u8> {
    let mut c = vec![0u8; rs41::CAL_PARTS * 16];
    let steps = ((freq_hz - 400e6) / 10e3).round().max(0.0) as u32;
    c[3] = (steps / 4) as u8;
    c[2] = ((steps % 4) << 6) as u8;
    c[rs41::cal::KILL..rs41::cal::KILL + 2].copy_from_slice(&30_600u16.to_le_bytes());
    let put = |c: &mut Vec<u8>, o: usize, v: f32| c[o..o + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut c, rs41::cal::RF1, RS41_RF.0 as f32);
    put(&mut c, rs41::cal::RF2, RS41_RF.1 as f32);
    for k in 0..3 {
        put(&mut c, rs41::cal::CO + 4 * k, RS41_CO[k] as f32);
        put(&mut c, rs41::cal::CA + 4 * k, RS41_CA[k] as f32);
    }
    put(&mut c, rs41::cal::H0, RS41_H0 as f32);
    if sgp {
        c[rs41::cal::SGP] = b'P';
    }
    c
}

const RS41_RF: (f64, f64) = (750.0, 1100.0);
const RS41_CO: [f64; 3] = [-243.0, 0.24, 1.0e-5];
const RS41_CA: [f64; 3] = [1.0, 0.1, 0.001];
const RS41_H0: f64 = 47.0;
const RS41_F: (f64, f64) = (300_000.0, 440_000.0);

/// The 12 PTU counts that give a temperature and humidity with the generator's calibration.
pub fn rs41_ptu(t_c: f64, rh: f64) -> [u32; 12] {
    let (co, ca) = (
        RS41_CO.map(|v| v as f32 as f64),
        RS41_CA.map(|v| v as f32 as f64),
    );
    let (rf1, rf2) = (RS41_RF.0 as f32 as f64, RS41_RF.1 as f32 as f64);
    let (f1, f2) = RS41_F;
    // T = (co0 + co1 R + co2 R² + ca1)(1 + ca2) → R.
    let k = t_c / (1.0 + ca[2]) - ca[1] - co[0];
    let r = if co[2].abs() > 0.0 {
        (-co[1] + (co[1] * co[1] + 4.0 * co[2] * k).sqrt()) / (2.0 * co[2])
    } else {
        k / co[1]
    };
    let g = (f2 - f1) / (rf2 - rf1);
    let rb = (f1 * rf2 - f2 * rf1) / (f2 - f1);
    let f = g * (r / ca[0] + rb);
    // Humidity.
    let mut v = rh;
    if t_c < -40.0 {
        v /= 1.0 + (-40.0 - t_c) / 120.0;
    }
    if t_c < -20.0 {
        v /= 1.0 + (-20.0 - t_c) / 100.0;
    }
    let a1 = 350.0 / (RS41_H0 as f32 as f64);
    let fh = ((v + t_c / 5.5) / 100.0 + 7.5) / a1;
    let fhum = f1 + fh * (f2 - f1);
    let mut m = [0u32; 12];
    m[0] = f.round() as u32;
    m[1] = f1 as u32;
    m[2] = f2 as u32;
    m[3] = fhum.round() as u32;
    m[4] = f1 as u32;
    m[5] = f2 as u32;
    m
}

/// One sonde of the scene: a GFSK transmitter at 0 Hz that makes its frames as they become due.
struct SondeTx {
    spec: SondeSpec,
    family: &'static str,
    rate: f64,
    symbol_rate: f64,
    continuous: bool,
    period: f64,
    drift_e: f64,
    cpm: Cpm,
    k: u32,
    cal: Vec<u8>,
    cal92: Vec<u8>,
    dfm_k: u64,
    n: u64,
    next_frame: u64,
    buf: Vec<Complex32>,
    pos: usize,
    syms: Vec<bool>,
    levels: Vec<f32>,
}

/// Generator options.
#[derive(Clone, Debug)]
pub struct SondeGenOptions {
    /// Sondes (taken in table order among the types in `types`).
    pub count: usize,
    /// Types: any of "rs41", "dfm", "m10", "m20", "rs92".
    pub types: Vec<String>,
    /// East drift, m/s.
    pub drift_e: f64,
    /// Offset of the second RS41.
    pub second_rs41_hz: f64,
    /// The L-band LMS6 scene instead.
    pub lband: bool,
    /// Explicit sondes (replace the scene table).
    pub sondes: Option<Vec<SondeSpec>>,
}

impl Default for SondeGenOptions {
    fn default() -> Self {
        SondeGenOptions {
            count: 3,
            types: vec!["rs41".into(), "dfm".into(), "m10".into()],
            drift_e: 10.0,
            second_rs41_hz: 450e3,
            lband: false,
            sondes: None,
        }
    }
}

fn family(model: &str) -> &'static str {
    match model {
        m if m.starts_with("RS41") => "rs41",
        m if m.starts_with("DFM") => "dfm",
        "M10" => "m10",
        "M20" => "m20",
        "RS92" => "rs92",
        _ => "lms6",
    }
}

impl SondeTx {
    fn new(spec: SondeSpec, rate: f64, drift_e: f64) -> SondeTx {
        let family = family(&spec.model);
        let (sr, dev, continuous, period) = match family {
            "rs41" => (4800.0, 2400.0, false, 1.0),
            "rs92" => (4800.0, 2400.0, true, 1.0),
            "dfm" => (2500.0, 2400.0, true, 0.224),
            "m10" => (9615.0, 4800.0, true, 1.0),
            "m20" => (9600.0, 4800.0, true, 1.0),
            _ => (9616.0, 75_000.0, true, 1.0),
        };
        let sps = rate / sr;
        let mut cpm = Cpm::gfsk(0.5, 2.0 * dev / sr, sps).expect("a valid GFSK");
        if !continuous {
            cpm = cpm
                .with_ramp(Ramp::RaisedCosine(0.0005 * rate))
                .expect("a valid ramp");
        }
        let freq = 403e6 + spec.offset_hz;
        let cal = rs41_calibration(freq, spec.model == "RS41-SGP");
        let mut cal92 = vec![0u8; rs92::CAL_PARTS * 16];
        let st = ((freq - 400e6) / 10e3).round() as u16;
        cal92[rs92::CAL_FREQ..rs92::CAL_FREQ + 2].copy_from_slice(&st.to_le_bytes());
        SondeTx {
            next_frame: (spec.phase_s * rate).round() as u64,
            spec,
            family,
            rate,
            symbol_rate: sr,
            continuous,
            period,
            drift_e,
            cpm,
            k: 0,
            cal,
            cal92,
            dfm_k: 0,
            n: 0,
            buf: Vec::new(),
            pos: 0,
            syms: Vec::with_capacity(20_000),
            levels: Vec::with_capacity(20_000),
        }
    }

    /// Where the sonde is `t` seconds after the start: latitude, longitude, altitude and the east, north and up speeds.
    fn flight(&self, t: f64) -> (f64, f64, f64, f64, f64, f64) {
        let s = &self.spec;
        let alt = s.alt + s.vv * t;
        let (lat, lon) = moved(s.lat, s.lon, self.drift_e * t, 0.0);
        (lat, lon, alt, self.drift_e, 0.0, s.vv)
    }

    /// Makes the frame that starts `t` seconds after the start into `self.buf`.
    fn make_frame(&mut self, t: f64) {
        let (lat, lon, alt, ve, vn, vu) = self.flight(t);
        let utc = START_UTC + t;
        let (temp, rh) = atmosphere(alt);
        let gps = gnss::gps_from_utc(utc, &LeapSeconds::BUILTIN).unwrap_or(gnss::WeekTime {
            week: 0,
            tow_s: 0.0,
        });
        let mut syms = std::mem::take(&mut self.syms);
        syms.clear();
        match self.family {
            "rs41" => {
                let p = Position::with_alt(lat, lon, alt);
                let pos = p.to_ecef().to_array();
                let vel = Observer::new(p).rotate_from_enu(&Enu {
                    east_m: ve,
                    north_m: vn,
                    up_m: vu,
                });
                let f = rs41::build_frame(&rs41::Rs41Tx {
                    serial: self.spec.serial.clone(),
                    frame: (self.spec.frame0 + self.k) as u16,
                    battery_v: 2.7,
                    cal: self.cal.clone(),
                    week: gps.week as u16,
                    tow_ms: (gps.tow_s * 1000.0).round() as u32,
                    pos,
                    vel,
                    sats: 9,
                    ptu: rs41_ptu(temp, rh),
                    extended: false,
                });
                rs41::frame_bits(&f, &mut syms);
            }
            "rs92" => {
                let f = rs92::build_frame(&rs92::Rs92Tx {
                    serial: self.spec.serial.clone(),
                    frame: (self.spec.frame0 + self.k) as u16,
                    cal: self.cal92.clone(),
                    tow_ms: (gps.tow_s * 1000.0).round() as u32,
                    tracked: 9,
                });
                rs92::frame_symbols(&f, &mut syms);
                if self.k > 0 {
                    syms.drain(..160);
                }
            }
            "dfm" => {
                let model = match self.spec.model.as_str() {
                    "DFM-06" => dfm::DfmModel::Dfm06,
                    "DFM-09" => dfm::DfmModel::Dfm09,
                    _ => dfm::DfmModel::Dfm17,
                };
                let bits = dfm_frame(
                    model,
                    &self.spec.serial,
                    self.dfm_k,
                    utc,
                    lat,
                    lon,
                    alt,
                    ve,
                    vn,
                    vu,
                    temp,
                );
                self.dfm_k += 1;
                dfm::manchester(&bits, model == dfm::DfmModel::Dfm06, &mut syms);
            }
            "m10" | "m20" => {
                let m = m10::M10Tx {
                    serial: self.spec.serial.clone(),
                    lat,
                    lon,
                    alt,
                    ve,
                    vn,
                    vu,
                    utc,
                    sats: 9,
                    temp_c: temp,
                    humidity: rh,
                    battery_v: 3.1,
                };
                let f = if self.family == "m10" {
                    m10::m10_frame(&m)
                } else {
                    m10::m20_frame(&m, self.k as u8, false)
                };
                for i in 0..96 {
                    syms.push(i % 2 == 0);
                }
                m10::frame_symbols(&f, &mut syms);
                let fill = (self.symbol_rate * self.period) as usize;
                let mut lv = syms.last().copied().unwrap_or(false);
                while syms.len() < fill {
                    lv = !lv;
                    syms.push(lv);
                }
            }
            _ => {
                let b = lms6::second_bytes(&lms6::Lms6Tx {
                    serial: self.spec.serial.parse().unwrap_or(0),
                    frame: self.spec.frame0 + self.k,
                    tow_ms: (gps.tow_s * 1000.0).round() as u32,
                    lat,
                    lon,
                    alt,
                    ve,
                    vn,
                    vu,
                });
                lms6::uart_bits(&b, &mut syms);
                // The rest of the second: filler bytes that look like data (the carrier is never a bare tone).
                let fill = (self.symbol_rate * self.period) as usize;
                let mut r = Rng::new(self.k as u64 + 77);
                let mut filler = Vec::new();
                while syms.len() + 10 * filler.len() < fill {
                    filler.push((r.next_u64() & 0x7F) as u8);
                }
                lms6::uart_bits(&filler, &mut syms);
                syms.truncate(fill);
            }
        }
        self.k += 1;
        self.levels.clear();
        self.levels
            .extend(syms.iter().map(|&b| if b { 1.0 } else { -1.0 }));
        self.syms = syms;
        self.buf.clear();
        self.pos = 0;
        self.cpm.push(&self.levels, &mut self.buf);
        if !self.continuous {
            self.cpm.flush(&mut self.buf);
        }
    }
}

impl Generator for SondeTx {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos < self.buf.len() {
                let take = (self.buf.len() - self.pos).min(out.len() - i);
                out[i..i + take].copy_from_slice(&self.buf[self.pos..self.pos + take]);
                self.pos += take;
                i += take;
                self.n += take as u64;
                continue;
            }
            if self.n < self.next_frame {
                let take = ((self.next_frame - self.n) as usize).min(out.len() - i);
                out[i..i + take].fill(Complex32::new(0.0, 0.0));
                i += take;
                self.n += take as u64;
                continue;
            }
            let start = self.n;
            self.make_frame(start as f64 / self.rate);
            self.next_frame = if self.continuous && self.family != "lms6" {
                start + self.buf.len() as u64
            } else {
                ((self.spec.phase_s + self.k as f64 * self.period) * self.rate).round() as u64
            };
        }
        out.len()
    }
}

/// The radiosonde scene generator. Each sonde sends at unit power at its offset from the centre.
pub struct SondeGenerator {
    rate: f64,
    scene: Scene,
}

impl SondeGenerator {
    /// A scene at `rate`. Sondes more than 45 % of the rate from the centre are left out.
    pub fn new(rate: f64, opt: SondeGenOptions) -> SondeGenerator {
        let specs = if let Some(s) = opt.sondes.clone() {
            s
        } else if opt.lband {
            vec![lband_sonde()]
        } else {
            scene_table(opt.second_rs41_hz)
                .into_iter()
                .filter(|s| opt.types.iter().any(|t| t == family(&s.model)))
                .take(opt.count.clamp(1, 7))
                .collect()
        };
        let mut scene = Scene::new(rate).expect("a positive rate");
        for spec in specs
            .into_iter()
            .filter(|s| s.offset_hz.abs() <= 0.45 * rate)
        {
            let off = spec.offset_hz;
            let tx = SondeTx::new(spec, rate, opt.drift_e);
            let _ = scene.add_stream(0.0, None, off, 0.0, Box::new(tx));
        }
        SondeGenerator { rate, scene }
    }

    /// From the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> SondeGenerator {
        let s = &cfg.settings;
        let mut o = SondeGenOptions {
            count: s.get(&super::GEN_COUNT) as usize,
            drift_e: s.get(&super::GEN_DRIFT),
            second_rs41_hz: s.get(&super::GEN_RS41B) * 1e3,
            lband: s.get(&super::GEN_LBAND),
            ..SondeGenOptions::default()
        };
        let t = s.get(&super::GEN_TYPES);
        if !t.is_empty() {
            o.types = t.split(',').map(|x| x.trim().to_lowercase()).collect();
        }
        SondeGenerator::new(cfg.rate_hz, o)
    }
}

#[allow(clippy::too_many_arguments)]
fn dfm_frame(
    model: dfm::DfmModel,
    serial: &str,
    k: u64,
    utc: f64,
    lat: f64,
    lon: f64,
    alt: f64,
    ve: f64,
    vn: f64,
    vu: f64,
    temp: f64,
) -> Vec<bool> {
    let dfm06 = model == dfm::DfmModel::Dfm06;
    // Configuration channel of this frame.
    // The cycle: channels 0–6, then the serial channel (DFM-06: its serial is channel 6).
    let last = if dfm06 { 6 } else { 7 };
    let chan = match (k % (last + 1)) as u8 {
        7 => model.serial_channel(),
        c => c,
    };
    let rf = model.rf();
    let g = 1.0e-5;
    let r = super::frame::steinhart_inverse(temp, dfm::THERMISTOR);
    let m3 = 0.01;
    let m4 = g * rf;
    let m0 = r * g + m3;
    let serial_n: u64 = serial.parse().unwrap_or(0);
    let val = match chan {
        0 => dfm::encode_float(m0, dfm06),
        1 | 2 => dfm::encode_float(0.5, dfm06),
        3 => dfm::encode_float(m3, dfm06),
        4 => dfm::encode_float(m4, dfm06),
        5 if dfm06 => 0xA0_0000,
        5 => 3100,
        6 if dfm06 => u32::from_str_radix(serial, 16).unwrap_or(0) & 0xFF_FFFF,
        6 => dfm::encode_float(1.0, false),
        c if c == model.serial_channel() => {
            let half = ((k / (last + 1)) % 2) as u32;
            let part = if half == 0 {
                serial_n & 0xFFFF
            } else {
                (serial_n >> 16) & 0xFFFF
            };
            (0xC << 20) | ((part as u32) << 4) | half
        }
        _ => 0,
    };
    // Data blocks: ids cycle 0..8, two per frame.
    let civ = calendar::civil(utc.floor() as i64);
    let ms = ((civ.second as f64 + utc.fract()) * 1000.0).round() as u64;
    let sep = 30.0;
    let vh = (ve * ve + vn * vn).sqrt();
    let dir = super::frame::heading(ve, vn);
    let i32b = |v: f64| (v.round() as i64 as u64) & 0xFFFF_FFFF;
    let i16b = |v: f64| (v.round() as i64 as u64) & 0xFFFF;
    let mut data = [(0u8, 0u64); 2];
    for (j, d) in data.iter_mut().enumerate() {
        let id = ((2 * k + j as u64) % 9) as u8;
        let p: u64 = match id {
            0 => (2u64 << 24) | (((utc.floor() as u64) % 256) << 16),
            1 => (0xFFFF_0000u64 << 16) | ms,
            2 => (i32b(lat * 1e7) << 16) | i16b(vh * 100.0),
            3 => (i32b(lon * 1e7) << 16) | i16b(dir * 100.0),
            4 => (i32b((alt - sep) * 100.0) << 16) | i16b(vu * 100.0),
            5 => i16b(sep * 100.0) << 32,
            8 => {
                ((civ.year as u64 & 0xFFF) << 36)
                    | ((civ.month as u64) << 32)
                    | ((civ.day as u64) << 27)
                    | ((civ.hour as u64) << 22)
                    | ((civ.minute as u64) << 16)
                    | (9u64 << 8)
            }
            _ => 0,
        };
        *d = (id, p);
    }
    let mut bits = Vec::with_capacity(280);
    dfm::frame_bits(chan, val, data, &mut bits);
    bits
}

impl Generator for SondeGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo {
            symbol_rate_hz: Some(4800.0),
            bit_rate_hz: Some(4800.0),
            ..SignalInfo::new(10e3)
        })
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
