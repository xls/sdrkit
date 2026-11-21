//! The clean UAT test signal: bursts of the sync word and the code word, modulated by `sdrkit-algo-sim`. Two schedules: the example
//! frame of RTCA DO-282B repeated in bursts, and a simulated airspace (aircraft once a second, a ground station with FIS-B uplinks).

use super::demod::{UatCodec, BIT_RATE, SYNC_DOWN, SYNC_UP};
use super::fisb;
use super::payload::{encode_downlink, encode_uplink, DownlinkState, InfoFrame, UplinkHeader};
use crate::generator::Rng;
use crate::geo::destination;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::scene::Scene;
use sdrkit_core::{Complex32, GenConfig, Generator, Hz};
use std::collections::VecDeque;

/// The example payload of RTCA DO-282B.
pub const EXAMPLE_PAYLOAD: [u8; 18] = [
    0x00, 0x7E, 0x69, 0x87, 0xD2, 0xD7, 0x42, 0x38, 0xC1, 0x45, 0x38, 0x55, 0xF8, 0x99, 0x80, 0xC7,
    0x52, 0x4F,
];

/// The encoders, made once.
fn codec() -> &'static UatCodec {
    static C: std::sync::OnceLock<UatCodec> = std::sync::OnceLock::new();
    C.get_or_init(UatCodec::default)
}

/// One burst: bits, amplitude, carrier offset and phase.
#[derive(Clone, Debug)]
pub struct UatBurst {
    /// Time of the first sync bit, s.
    pub start: f64,
    /// The bits (sync then code word).
    pub bits: Vec<bool>,
    /// Amplitude.
    pub amp: f32,
    /// Carrier offset, Hz.
    pub freq: f64,
    /// Phase at the start.
    pub phase: f64,
}

impl UatBurst {
    /// A downlink burst of a payload (18 or 34 bytes).
    pub fn downlink(start: f64, payload: &[u8], amp: f32, freq: f64, phase: f64) -> UatBurst {
        UatBurst::new(
            start,
            SYNC_DOWN,
            &codec().encode_downlink(payload).unwrap_or_default(),
            amp,
            freq,
            phase,
        )
    }
    /// An uplink burst of a 432-byte frame.
    pub fn uplink(start: f64, data: &[u8], amp: f32, freq: f64, phase: f64) -> UatBurst {
        UatBurst::new(
            start,
            SYNC_UP,
            &codec().encode_uplink(data).unwrap_or_default(),
            amp,
            freq,
            phase,
        )
    }
    fn new(start: f64, sync: u64, cw: &[u8], amp: f32, freq: f64, phase: f64) -> UatBurst {
        let mut bits: Vec<bool> = (0..36).map(|k| sync >> (35 - k) & 1 == 1).collect();
        for &b in cw {
            for k in 0..8 {
                bits.push(b >> (7 - k) & 1 == 1);
            }
        }
        UatBurst {
            start,
            bits,
            amp,
            freq,
            phase,
        }
    }
    /// The time just after the last bit.
    pub fn end(&self) -> f64 {
        self.start + self.bits.len() as f64 / BIT_RATE
    }
    /// The burst's samples at `rate`, at unit amplitude and its own phase: continuous-phase FSK with modulation index 0.6 from
    /// `sdrkit-algo-sim` (a 1 raises the frequency). DO-282B 2.2.2.1.
    pub fn samples(&self, rate: f64) -> Vec<Complex32> {
        let Ok(mut m) = Cpm::cpfsk(0.6, rate / BIT_RATE) else {
            return Vec::new();
        };
        let levels: Vec<f32> = self
            .bits
            .iter()
            .map(|&b| if b { 1.0 } else { -1.0 })
            .collect();
        let mut out = Vec::with_capacity((levels.len() as f64 * rate / BIT_RATE) as usize + 8);
        m.push(&levels, &mut out);
        m.flush(&mut out);
        let rot = Complex32::from_polar(1.0, self.phase as f32);
        for v in &mut out {
            *v *= rot;
        }
        out
    }
}

/// The schedule.
#[derive(Clone, Debug, PartialEq)]
pub enum UatSchedule {
    /// The example frame in `bursts` bursts (burst i at 0.4 + 1.7 i s, gain -6 i dB, offset (i - 1) x 1200 Hz), repeated after 1 s.
    Example {
        /// Bursts per round.
        bursts: usize,
    },
    /// A simulated airspace: aircraft once a second, and an uplink with FIS-B products once a second when `uplink`.
    Airspace {
        /// Aircraft.
        aircraft: usize,
        /// Uplink frames.
        uplink: bool,
    },
}

#[derive(Clone, Debug)]
struct Craft {
    address: u32,
    aq: u8,
    callsign: String,
    lat: f64,
    lon: f64,
    track: f64,
    gs: f64,
    alt: i32,
    vrate: i32,
    slot: f64,
    t: f64,
    turn: u8,
    amp: f32,
    freq: f64,
}

/// The UAT generator.
pub struct UatGenerator {
    rate: f64,
    schedule: UatSchedule,
    rng: Rng,
    n: u64,
    queue: VecDeque<UatBurst>,
    scene: Scene,
    round: u64,
    crafts: Vec<Craft>,
    station: (f64, f64),
    /// Every burst sent, when `keep_log` is set: (time, payload or uplink data, is uplink).
    pub log: Vec<(f64, Vec<u8>, bool)>,
    /// Keep the log.
    pub keep_log: bool,
}

impl UatGenerator {
    /// A generator at `rate` (at least 1.9 Msps).
    pub fn new(rate: Hz, schedule: UatSchedule, seed: u64) -> UatGenerator {
        let mut rng = Rng::new(seed);
        let station = (47.45, -122.31);
        let mut crafts = Vec::new();
        if let UatSchedule::Airspace { aircraft, .. } = schedule {
            for i in 0..aircraft {
                let d = rng.range(5.0, 120.0);
                let b = rng.range(0.0, 360.0);
                let (lat, lon, _) = destination(station.0, station.1, b, d);
                crafts.push(Craft {
                    address: 0xA00000 + rng.below(0xFFFFF) as u32,
                    aq: if i % 5 == 4 { 2 } else { 0 },
                    callsign: format!("UAT{}", 100 + rng.below(900)),
                    lat,
                    lon,
                    track: rng.range(0.0, 360.0),
                    gs: rng.range(90.0, 220.0),
                    alt: (rng.range(2000.0, 17000.0) / 25.0).round() as i32 * 25,
                    vrate: (rng.range(-1000.0, 1000.0) / 64.0).round() as i32 * 64,
                    slot: 0.19 + rng.range(0.0, 0.79),
                    t: 0.0,
                    turn: 0,
                    amp: (0.08 * 10f64.powf(rng.range(-12.0, 0.0) / 20.0)) as f32,
                    freq: rng.normal() * 2000.0,
                });
            }
        }
        UatGenerator {
            rate,
            schedule,
            rng,
            n: 0,
            queue: VecDeque::new(),
            scene: Scene::new(rate)
                .unwrap_or_else(|_| Scene::new(2.083334e6).expect("a valid rate")),
            round: 0,
            crafts,
            station,
            log: Vec::new(),
            keep_log: false,
        }
    }
    /// From a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> UatGenerator {
        use crate::settings::{GEN_BURSTS, GEN_EXAMPLE, GEN_UPLINK, UAT_GEN_AIRCRAFT};
        let v = &cfg.settings;
        let schedule = if v.get(&GEN_EXAMPLE) {
            UatSchedule::Example {
                bursts: v.get(&GEN_BURSTS).clamp(1, 20) as usize,
            }
        } else {
            UatSchedule::Airspace {
                aircraft: v.get(&UAT_GEN_AIRCRAFT).clamp(0, 100) as usize,
                uplink: v.get(&GEN_UPLINK),
            }
        };
        UatGenerator::new(cfg.rate_hz, schedule, cfg.seed)
    }

    /// The uplink frame the airspace's ground station sends in second `n`.
    pub fn uplink_frame(n: u64, station: (f64, f64)) -> Vec<u8> {
        let h = UplinkHeader {
            pos: Some(station),
            utc_coupled: true,
            app_data_valid: true,
            slot_id: 5,
            tisb_site: 2,
        };
        let (hh, mm, ss) = (
            ((n / 3600) % 24) as u8,
            ((n / 60) % 60) as u8,
            (n % 60) as u8,
        );
        let mut metar = fisb::encode_apdu(413, false, false, 0, 0, 0, hh, mm, 0);
        metar.extend(fisb::dlac_encode(&format!(
            "METAR KSEA {:02}{:02}{:02}Z 18010KT 10SM FEW040 12/06 A3001",
            8, hh, mm
        )));
        let mut nexrad = fisb::encode_apdu(63, false, false, 1, 0, 0, hh, mm, ss);
        let bins: Vec<u8> = (0..128)
            .map(|i| ((i / 16 + n as usize) % 8) as u8)
            .collect();
        nexrad.extend(fisb::encode_grid_block(127_123, false, 0, &bins));
        let mut airmet = fisb::encode_apdu(11, false, false, 2, 11, 8, hh, mm, 0);
        airmet.extend(fisb::encode_twgo_text(
            "KSEA",
            100 + (n % 50) as u16,
            26,
            "AIRMET SIERRA FOR IFR AND MTN OBSCN",
        ));
        let mut gair = fisb::encode_apdu(14, false, false, 3, 11, 8, hh, mm, ss);
        gair.extend(fisb::encode_twgo_polygon(
            "KSEA",
            7,
            1,
            &[
                (47.0, -123.0, 0),
                (48.0, -123.0, 0),
                (48.0, -121.0, 12000),
                (47.0, -121.0, 12000),
            ],
        ));
        let frames: Vec<InfoFrame> = [metar, nexrad, airmet, gair]
            .into_iter()
            .map(|d| InfoFrame {
                frame_type: 0,
                data: d,
            })
            .collect();
        encode_uplink(&h, &frames)
    }

    fn fill(&mut self, until: f64) {
        while self.queue.back().is_none_or(|b| b.start < until + 0.5) {
            match self.schedule.clone() {
                UatSchedule::Example { bursts } => {
                    let period = 0.4 + 1.7 * bursts as f64 + 1.0;
                    let t0 = self.round as f64 * period;
                    for i in 0..bursts {
                        let t = t0 + 0.4 + 1.7 * i as f64;
                        let amp = (0.08 * 10f64.powf(-6.0 * i as f64 / 20.0)) as f32;
                        let ph = self.rng.range(0.0, std::f64::consts::TAU);
                        if self.keep_log {
                            self.log.push((t, EXAMPLE_PAYLOAD.to_vec(), false));
                        }
                        self.queue.push_back(UatBurst::downlink(
                            t,
                            &EXAMPLE_PAYLOAD,
                            amp,
                            (i as f64 - 1.0) * 1200.0,
                            ph,
                        ));
                    }
                }
                UatSchedule::Airspace { uplink, .. } => {
                    let sec = self.round;
                    let t0 = sec as f64;
                    let mut bursts = Vec::new();
                    if uplink {
                        let data = Self::uplink_frame(sec, self.station);
                        if self.keep_log {
                            self.log.push((t0 + 0.0335, data.clone(), true));
                        }
                        bursts.push(UatBurst::uplink(
                            t0 + 0.0335,
                            &data,
                            0.06,
                            500.0,
                            self.rng.range(0.0, std::f64::consts::TAU),
                        ));
                    }
                    for c in self.crafts.iter_mut() {
                        let t = t0 + c.slot;
                        let dt = t - c.t;
                        if c.t > 0.0 {
                            let (la, lo, trk) =
                                destination(c.lat, c.lon, c.track, c.gs * dt / 3600.0);
                            c.lat = la;
                            c.lon = lo;
                            c.track = trk;
                            c.alt += (c.vrate as f64 * dt / 60.0) as i32;
                        }
                        c.t = t;
                        c.turn = c.turn.wrapping_add(1);
                        let pt = if c.turn % 2 == 1 { 1 } else { 0 };
                        let (n, e) = (
                            c.gs * c.track.to_radians().cos(),
                            c.gs * c.track.to_radians().sin(),
                        );
                        let st = DownlinkState {
                            payload_type: pt,
                            address_qualifier: c.aq,
                            address: c.address,
                            pos: Some((c.lat, c.lon)),
                            nic: 8,
                            alt: Some(((c.alt / 25) * 25, false)),
                            velocity: Some((n.round(), e.round())),
                            ground: None,
                            vrate: Some(c.vrate),
                            callsign: Some(c.callsign.clone()),
                            category: 1,
                            nacp: 9,
                            aux_alt: Some((c.alt / 25) * 25 + 150),
                        };
                        let p = encode_downlink(&st);
                        if self.keep_log {
                            self.log.push((t, p.clone(), false));
                        }
                        bursts.push(UatBurst::downlink(
                            t,
                            &p,
                            c.amp,
                            c.freq,
                            self.rng.range(0.0, std::f64::consts::TAU),
                        ));
                    }
                    bursts.sort_by(|a, b| a.start.total_cmp(&b.start));
                    self.queue.extend(bursts);
                }
            }
            self.round += 1;
        }
    }
}

impl Generator for UatGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        // An aircraft at the strongest level (amplitude 0.08), in the 2.083 MHz the demodulator works at.
        Some(sdrkit_core::SignalInfo {
            bandwidth_hz: super::demod::WORK_RATE,
            symbol_rate_hz: Some(BIT_RATE),
            bit_rate_hz: Some(BIT_RATE),
            power: 0.08 * 0.08,
        })
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let t_end = (self.n + out.len() as u64) as f64 / self.rate;
        self.fill(t_end);
        while self.queue.front().is_some_and(|b| b.start < t_end) {
            let Some(b) = self.queue.pop_front() else {
                break;
            };
            let x = b.samples(self.rate);
            let level_db = 20.0 * (b.amp.max(1e-9) as f64).log10();
            let start = b.start.max(self.scene.position_s());
            let _ = self.scene.add_burst(start, b.freq, level_db, x);
        }
        let n = self.scene.generate(out);
        self.n += n as u64;
        n
    }
}
