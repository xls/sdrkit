//! The VDL4 test signal: aircraft sending synchronisation bursts (CPR positions) and DLS bursts in turn on one or more channels,
//! GFSK at 19 200 bit/s, with an answer key. The signal is clean: the engine adds the noise.
//! Standard: ICAO Doc 9816 Part 2, ETSI EN 302 842-2 (GFSK, BT 0.28, NRZI, HDLC).

use super::reader::{sync_info, write};
use super::BAUD;
use sdrkit_algo_dsp::pulse::gaussian_freq;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_proto_link::hdlc::{Line, Transmitter};
use sdrkit_util_bits::line::Nrzi;
use sdrkit_util_geo::cpr::{self, CprFormat};
use sdrkit_util_geo::Position;
use std::f64::consts::TAU;

/// The amplitude of a burst.
pub const AMPLITUDE: f64 = 0.08;

/// The scene.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// Output rate.
    pub rate_hz: f64,
    /// The frequency of the centre of the samples, Hz: the station of the session, where the engine puts it. 0 makes the
    /// channels offsets from that centre.
    pub centre_hz: f64,
    /// The channels, Hz, on the same scale as `centre_hz`; the bursts take them in turn.
    pub channels: Vec<f64>,
    /// Aircraft.
    pub aircraft: usize,
    /// Seed.
    pub seed: u64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            rate_hz: 1e6,
            centre_hz: 0.0,
            // One channel at the station.
            channels: vec![0.0],
            aircraft: 4,
            seed: 1,
        }
    }
}

/// A burst sent.
#[derive(Clone, Debug)]
pub struct Sent {
    /// Start of the training sequence, s.
    pub time_s: f64,
    /// Channel.
    pub freq_hz: f64,
    /// The frame.
    pub frame: Vec<u8>,
}

/// The line bits (frequency signs) of a burst carrying `frame`: ramp ones, the training, the HDLC bits, NRZI coded.
pub fn line_bits(frame: &[u8]) -> Vec<f64> {
    let mut data: Vec<u8> = vec![1; 16];
    data.extend((0..24).map(|i| (i % 2) as u8));
    Transmitter::new(Line::Nrz).encode(&[frame], 1, &mut data);
    data.extend([1; 8]);
    Nrzi::new(1).encode(&mut data);
    data.iter()
        .map(|&b| if b != 0 { 1.0 } else { -1.0 })
        .collect()
}

struct Tx {
    start: f64,
    levels: Vec<f64>,
    freq: f64,
    phase: f64,
}

/// The VDL4 generator.
pub struct Vdl4Generator {
    p: GenParams,
    rng: Rng,
    n: u64,
    txs: Vec<Tx>,
    pulse: Vec<f64>,
    /// Every burst sent.
    pub sent: Vec<Sent>,
}

impl Vdl4Generator {
    /// A generator for the scene. The schedule repeats every cycle with 3 s of silence.
    pub fn new(p: GenParams) -> Self {
        // The Gaussian-filtered rectangular pulse, BT 0.28, over +-2 bits at 32 points per bit.
        let pulse: Vec<f64> = (0..=160)
            .map(|i| gaussian_freq((i as f64 - 80.0) / 32.0, 0.28))
            .collect();
        Vdl4Generator {
            rng: Rng::new(p.seed),
            n: 0,
            txs: Vec::new(),
            pulse,
            sent: Vec::new(),
            p,
        }
    }

    /// A generator from the stack's `gen_*` settings and the rate.
    pub fn from_config(cfg: &GenConfig) -> Self {
        Vdl4Generator::new(GenParams {
            rate_hz: cfg.rate_hz,
            seed: cfg.seed,
            ..Default::default()
        })
    }

    fn plan(&mut self, until: f64) {
        let n_ac = self.p.aircraft.max(1);
        let cycle = n_ac as f64 * 2.0 * 0.4 + 3.0;
        loop {
            let k = self.sent.len();
            let c = k / (2 * n_ac);
            let j = k % (2 * n_ac);
            let t = 0.2 + c as f64 * cycle + j as f64 * 0.4;
            if t > until {
                return;
            }
            let i = j / 2;
            let addr = 0x4CA2B1 + i as u32;
            let frame = if j.is_multiple_of(2) {
                let p = Position::new(51.4 + 0.02 * i as f64, -0.5 + 0.02 * i as f64);
                let c = cpr::encode(&CprFormat::VDL4, &p, false).unwrap_or(cpr::CprFrame {
                    odd: false,
                    lat: 0,
                    lon: 0,
                });
                write(
                    true,
                    1,
                    addr,
                    (k % 256) as u8,
                    1,
                    &sync_info(c.lat, c.lon, false, 35_000.0),
                )
            } else {
                let mut info = vec![0x10, 0x00];
                info.extend(b"VDL4 TEST");
                write(true, 1, addr, (k % 256) as u8, 3, &info)
            };
            let f = match self.p.channels.len() {
                0 => self.p.centre_hz,
                n => self.p.channels[k % n],
            };
            let freq = f - self.p.centre_hz + self.rng.range(-150.0, 150.0);
            let levels = line_bits(&frame);
            // The training sequence starts after the 16 ramp bits.
            self.txs.push(Tx {
                start: t - 16.0 / BAUD,
                levels,
                freq,
                phase: self.rng.range(0.0, TAU),
            });
            self.sent.push(Sent {
                time_s: t,
                freq_hz: f,
                frame,
            });
        }
    }
}

impl Generator for Vdl4Generator {
    fn rate_hz(&self) -> f64 {
        self.p.rate_hz
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        let mut s = SignalInfo::new(25e3)
            .with_symbol_rate(BAUD)
            .with_bit_rate(BAUD);
        s.power = AMPLITUDE * AMPLITUDE;
        Some(s)
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let fs = self.p.rate_hz;
        let t1 = (self.n + out.len() as u64) as f64 / fs;
        self.plan(t1 + 1.0);
        out.fill(Complex32::default());
        for tx in self.txs.iter_mut() {
            let end = tx.start + tx.levels.len() as f64 / BAUD;
            let t0 = self.n as f64 / fs;
            if end < t0 || tx.start > t1 {
                continue;
            }
            let first = ((tx.start * fs).ceil() as u64).max(self.n);
            let last = ((end * fs).floor() as u64).min(self.n + out.len() as u64 - 1);
            for k in first..=last {
                let u = (k as f64 / fs - tx.start) * BAUD;
                let c = u.floor() as i64;
                let mut fr = 0.0;
                for b in (c - 2).max(0)..=(c + 2).min(tx.levels.len() as i64 - 1) {
                    let d = u - b as f64 - 0.5;
                    if d.abs() >= 2.5 {
                        continue;
                    }
                    let pi = ((d * 32.0) + 80.0).round().clamp(0.0, 160.0) as usize;
                    fr += tx.levels[b as usize] * self.pulse[pi];
                }
                // Deviation 4.8 kHz.
                tx.phase += TAU * (4800.0 * fr) / fs;
                let ph = tx.phase + TAU * tx.freq * k as f64 / fs;
                out[(k - self.n) as usize] +=
                    Complex32::new((AMPLITUDE * ph.cos()) as f32, (AMPLITUDE * ph.sin()) as f32);
            }
        }
        self.n += out.len() as u64;
        let tn = self.n as f64 / fs;
        self.txs
            .retain(|t| t.start + t.levels.len() as f64 / BAUD >= tn);
        out.len()
    }
}
