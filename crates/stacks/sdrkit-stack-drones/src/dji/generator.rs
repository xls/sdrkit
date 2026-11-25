//! The DJI test signal: DroneID bursts (the readable packet, the encrypted format or the empty frame), and stand-ins of the OcuSync-like and Lightbridge-like shapes and of
//! Wi-Fi-like bursts. The stand-ins exercise the shape detector. They are not the real protocols. The generator makes the clean
//! signal only: noise, offsets and the receiver's impairments come from the standard `gen_*` settings.

use super::droneid::{self, DroneIdPacket, FRAME, NC, NFFT};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::resample_span;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};

/// What the generator makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scenario {
    /// DroneID 5 MHz below, the OcuSync-like train 5 MHz above, sparse Wi-Fi-like bursts.
    Everything,
    /// DroneID bursts only.
    DroneIdOnly,
    /// The OcuSync-like train only.
    OcuSyncOnly,
    /// The Lightbridge-like train only.
    LightbridgeOnly,
    /// Wi-Fi-like bursts only.
    WifiOnly,
    /// DroneID bursts each overlapped by a Wi-Fi-like burst.
    Collisions,
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct DjiGenOptions {
    /// Output rate (at least 16 Msps).
    pub rate: f64,
    /// Scenario.
    pub scenario: Scenario,
    /// DroneID period, s.
    pub period_s: f64,
    /// DroneID without its first symbol.
    pub eight: bool,
    /// The OcuSync-like train 20 MHz wide (2048 FFT at 30.72 Msps, centred).
    pub wide: bool,
    /// The frame of the DroneID bursts.
    pub frame: DroneIdFrame,
    /// The radio centre, Hz: the DroneID bursts are on the DroneID frequency nearest to it, one each 5 periods, without the
    /// carriers outside the capture. `None` puts them 5 MHz below the centre, one each period.
    pub droneid_centre_hz: Option<f64>,
    /// Every second DroneID burst comes from a second aircraft.
    pub two_aircraft: bool,
    /// Seed.
    pub seed: u64,
}

/// The frame of the DroneID bursts of the test signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DroneIdFrame {
    /// The readable packet of [`packet`].
    Readable,
    /// The encrypted format: the magic, the session key of the aircraft, 8 changing bytes, a random body and the inner CRC-16.
    Encrypted,
    /// The empty frame.
    Empty,
}

/// The session keys of the two aircraft of the test signal.
pub const SESSION_KEYS: [[u8; 4]; 2] = [[0x5d, 0x4b, 0x01, 0xa7], [0x5d, 0x4b, 0x02, 0x3c]];
/// The serial number of the second aircraft of the test signal.
pub const SECOND_SERIAL: &str = "SDRKTESTDRONE002";
/// The periods between two DroneID bursts on a DroneID frequency.
pub const SPARSE: u64 = 5;
/// The 4-byte tail of the frames of the test signal.
const TAIL: [u8; 4] = [0x21, 0x43, 0x65, 0x87];

impl Default for DjiGenOptions {
    fn default() -> Self {
        DjiGenOptions {
            rate: 20e6,
            scenario: Scenario::Everything,
            period_s: 0.64,
            eight: false,
            wide: false,
            frame: DroneIdFrame::Readable,
            droneid_centre_hz: None,
            two_aircraft: false,
            seed: 1,
        }
    }
}

/// The DroneID packet the generator sends as its `n`th, at signal time `t`.
pub fn packet(n: u64, t: f64) -> DroneIdPacket {
    DroneIdPacket {
        length: 88,
        version: 2,
        seq: (1000 + n) as u16,
        state: 0x1FF3,
        serial: "SDRKTESTDRONE001".into(),
        lat: 51.4468 + 0.00002 * n as f64,
        lon: 7.2679 + 0.00003 * n as f64,
        alt_raw: (4054 + 3 * n) as i16,
        height_raw: (305 + n) as i16,
        v_north: 29,
        v_east: 84,
        v_up: 0,
        yaw: 127,
        gps_ms: 1_650_894_651_295 + (t * 1000.0) as u64,
        pilot_lat: 43.2682071,
        pilot_lon: 6.6401597,
        home_lat: 51.4468438,
        home_lon: 7.2679321,
        device_type: 60,
        uuid_len: 0,
        uuid: String::new(),
        inner_crc_ok: true,
    }
}

/// A random-QPSK OFDM burst: `symbols` symbols of FFT `n`, `used` carriers, prefix `cp`; with `repeat_first`, the second symbol
/// repeats the first.
pub fn ofdm_burst(
    n: usize,
    used: usize,
    cp: usize,
    symbols: usize,
    repeat_first: bool,
    rng: &mut Rng,
) -> Vec<Complex32> {
    let mut fft = Fft::inverse(n);
    let mut out = Vec::with_capacity(symbols * (n + cp));
    let mut first: Vec<Complex32> = Vec::new();
    for s in 0..symbols {
        let t = if repeat_first && s == 1 {
            first.clone()
        } else {
            let mut x = vec![Complex32::new(0.0, 0.0); n];
            for k in 0..used {
                let bin = if k < used / 2 {
                    n - used / 2 + k
                } else {
                    k - used / 2 + 1
                };
                let b = rng.next_u64();
                let s = std::f32::consts::FRAC_1_SQRT_2;
                x[bin] = Complex32::new(
                    if b & 1 == 0 { s } else { -s },
                    if b & 2 == 0 { s } else { -s },
                );
            }
            fft.process(&mut x);
            x
        };
        if s == 0 {
            first = t.clone();
        }
        out.extend_from_slice(&t[n - cp..]);
        out.extend_from_slice(&t);
    }
    out
}

struct Active {
    start: u64,
    x: Vec<Complex32>,
    nco: Nco,
}

/// The DJI generator. Each burst has unit mean power while it sends.
pub struct DjiGenerator {
    o: DjiGenOptions,
    rng: Rng,
    n: u64,
    next_droneid: u64,
    next_ocu: u64,
    next_lb: u64,
    next_wifi: f64,
    active: Vec<Active>,
    wifi_taps: Vec<f32>,
}

impl DjiGenerator {
    /// A generator with these options. If the rate is too low for the Wi-Fi filter, the Wi-Fi bursts are not filtered.
    pub fn new(o: DjiGenOptions) -> Self {
        let wifi_taps = lowpass(
            o.rate,
            (8.0e6f64).min(0.44 * o.rate),
            (9.5e6f64).min(0.49 * o.rate),
            50.0,
        )
        .unwrap_or_else(|_| vec![1.0]);
        let first_wifi = match o.scenario {
            Scenario::WifiOnly => 0.002,
            Scenario::Everything => 0.02,
            Scenario::Collisions => 0.04,
            _ => f64::INFINITY,
        };
        DjiGenerator {
            rng: Rng::new(o.seed),
            n: 0,
            next_droneid: 0,
            next_ocu: 0,
            next_lb: 0,
            next_wifi: first_wifi,
            active: Vec::new(),
            wifi_taps,
            o,
        }
    }

    /// From the stack's settings. The schema of `cfg.settings` must hold the settings of [`super::DjiSetting`].
    pub fn from_config(cfg: &GenConfig) -> Self {
        use super::GenScenario as G;
        let s = &cfg.settings;
        DjiGenerator::new(DjiGenOptions {
            rate: cfg.rate_hz,
            scenario: match s.get(&super::GEN_SCENARIO) {
                G::DroneIdOnly => Scenario::DroneIdOnly,
                G::OcuSyncOnly => Scenario::OcuSyncOnly,
                G::LightbridgeOnly => Scenario::LightbridgeOnly,
                G::WifiOnly => Scenario::WifiOnly,
                G::Collisions => Scenario::Collisions,
                G::Everything => Scenario::Everything,
            },
            period_s: s.get(&super::GEN_PERIOD_S),
            eight: s.get(&super::GEN_EIGHT_SYMBOLS),
            wide: s.get(&super::GEN_WIDE),
            frame: match s.get(&super::GEN_FRAME) {
                super::GenFrame::Readable => DroneIdFrame::Readable,
                super::GenFrame::Encrypted => DroneIdFrame::Encrypted,
                super::GenFrame::Empty => DroneIdFrame::Empty,
            },
            droneid_centre_hz: s.get(&super::GEN_DRONEID_FREQUENCY).then(|| {
                let own = s.get(&super::GEN_CENTRE_HZ);
                if own > 0.0 {
                    own
                } else {
                    cfg.tuned_hz().unwrap_or(2440e6)
                }
            }),
            two_aircraft: s.get(&super::GEN_TWO_AIRCRAFT),
            seed: cfg.seed,
        })
    }

    fn push(&mut self, t: f64, mut y: Vec<Complex32>, freq: f64) {
        let p = y.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / y.len().max(1) as f64;
        let g = (1.0 / p.max(1e-24)).sqrt() as f32;
        y.iter_mut().for_each(|v| *v *= g);
        self.active.push(Active {
            start: (t * self.o.rate).round() as u64,
            x: y,
            nco: Nco::new(self.o.rate, freq),
        });
    }

    fn place(&mut self, t: f64, x: &[Complex32], fs: f64, freq: f64) {
        let mut y = Vec::new();
        resample_span(x, fs, self.o.rate, &mut y);
        self.push(t, y, freq);
    }

    fn wifi_burst(&mut self, t: f64, dur: f64) {
        let n = (dur * self.o.rate) as usize;
        let mut fir = Fir::<Complex32>::new(&self.wifi_taps);
        let skip = self.wifi_taps.len();
        let mut y = Vec::with_capacity(n + skip);
        for _ in 0..n + skip {
            y.push(fir.process_sample(self.rng.complex_normal(1.0)));
        }
        y.drain(..skip);
        self.push(t, y, 0.0);
    }

    /// The frame of the `n`th DroneID period at signal time `t`, of the first or the second aircraft.
    fn frame(&mut self, n: u64, t: f64, second: bool) -> [u8; FRAME] {
        match self.o.frame {
            DroneIdFrame::Readable => {
                let mut p = packet(n, t);
                if second {
                    p.serial = SECOND_SERIAL.into();
                    p.lat += 0.001;
                }
                p.frame()
            }
            DroneIdFrame::Encrypted => {
                let mut body = [0u8; 109];
                body.iter_mut().for_each(|b| *b = self.rng.next_u64() as u8);
                let changing = (0x0102_0304_0506_0708u64 ^ self.rng.next_u64()).to_le_bytes();
                droneid::encrypted_frame(
                    if n.is_multiple_of(2) { 0x10 } else { 0x11 },
                    SESSION_KEYS[usize::from(second)],
                    changing,
                    &body,
                    TAIL,
                )
            }
            DroneIdFrame::Empty => droneid::empty_frame(TAIL),
        }
    }

    fn schedule(&mut self, until_s: f64) {
        use Scenario::*;
        let sc = self.o.scenario;
        if matches!(sc, Everything | DroneIdOnly | Collisions) {
            loop {
                let t = 0.05 + self.next_droneid as f64 * self.o.period_s;
                if t >= until_s {
                    break;
                }
                let n = self.next_droneid;
                let sparse = self.o.droneid_centre_hz.is_some();
                if sparse && !n.is_multiple_of(SPARSE) {
                    self.next_droneid += 1;
                    continue;
                }
                let sent = if sparse { n / SPARSE } else { n };
                let second = self.o.two_aircraft && sent % 2 == 1;
                let frame = self.frame(n, t, second);
                let offset = self.o.droneid_centre_hz.map_or(-5e6, |c| {
                    super::DRONEID_FREQS
                        .iter()
                        .map(|mhz| mhz * 1e6 - c)
                        .min_by(|a, b| a.abs().total_cmp(&b.abs()))
                        .unwrap_or(0.0)
                });
                let carriers = if sparse {
                    droneid::visible_carriers(offset, 0.5 * self.o.rate)
                } else {
                    0..NC
                };
                let b = droneid::modulate_carriers(&frame, 0, self.o.eight, carriers);
                self.place(t, &b, droneid::FS, offset);
                if sc == Collisions {
                    let start = t + self.rng.range(-0.3e-3, 0.7e-3);
                    let dur = self.rng.range(100e-6, 1500e-6);
                    self.wifi_burst(start.max(0.0), dur);
                }
                self.next_droneid += 1;
            }
        }
        if matches!(sc, Everything | OcuSyncOnly) {
            loop {
                let t = 0.01 + self.next_ocu as f64 * 2e-3;
                if t >= until_s {
                    break;
                }
                if self.o.wide && sc == OcuSyncOnly {
                    let b = ofdm_burst(2048, 1200, 144, 14, false, &mut self.rng);
                    self.place(t, &b, 30.72e6, 0.0);
                } else {
                    let b = ofdm_burst(NFFT, 600, 72, 14, false, &mut self.rng);
                    self.place(t, &b, 15.36e6, 5e6);
                }
                self.next_ocu += 1;
            }
        }
        if sc == LightbridgeOnly {
            loop {
                let t = 0.01 + self.next_lb as f64 * 1e-3;
                if t >= until_s {
                    break;
                }
                let b = ofdm_burst(NFFT, 600, 72, 10, true, &mut self.rng);
                self.place(t, &b, 15.36e6, 5e6);
                self.next_lb += 1;
            }
        }
        while self.next_wifi < until_s {
            let t = self.next_wifi;
            let dur = self.rng.range(100e-6, 1500e-6);
            self.wifi_burst(t, dur);
            self.next_wifi = match sc {
                WifiOnly => t + dur + self.rng.range(0.5e-3, 6.5e-3),
                Everything => t + self.rng.range(20e-3, 60e-3),
                Collisions => t + self.rng.range(40e-3, 140e-3),
                _ => f64::INFINITY,
            };
        }
    }
}

impl Generator for DjiGenerator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let r = self.o.rate;
        let end = self.n + out.len() as u64;
        self.schedule(end as f64 / r + 0.002);
        out.iter_mut().for_each(|o| *o = Complex32::new(0.0, 0.0));
        for a in &mut self.active {
            let s = a.start.max(self.n);
            let e = (a.start + a.x.len() as u64).min(end);
            for k in s..e {
                out[(k - self.n) as usize] += a.x[(k - a.start) as usize] * a.nco.next_sample();
            }
        }
        self.active.retain(|a| a.start + a.x.len() as u64 > end);
        self.n = end;
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(9e6).with_symbol_rate(15e3 * 600.0))
    }
}
