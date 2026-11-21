//! The cdma2000 test signal: base stations of Spreading Rate 1, each with its pilot (Walsh 0), its Sync Channel (Walsh 32, the
//! whole coding chain, one Sync Channel Message every three superframes), a Paging Channel (Walsh 1) and eight other channels
//! (Walsh 8 to 15) with random data, spread by the pilot PN delayed by the station's PN offset. The pulse is the 48-tap
//! baseband filter of C.S0002-C 3.1.3.1.17.1 at 4 samples a chip (or a root raised cosine of roll-off 0.2 over 12 chips). Unit
//! mean power.

use super::sync::{frame_symbols, SyncMessage, FRAME_BITS};
use sdrkit_algo_fec::conv::codes::CDMA2000_K9;
use super::{baseband_fir, CHIP_RATE};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{self, Norm, Shape};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_algo_modem::spreading::{cdma2000::chip as pn_chip, cdma2000::PN_PERIOD, hadamard};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};

/// Samples a chip of the generator's native rate.
const NATIVE_SPC: usize = 4;
/// Chips made at a time.
const BLOCK_CHIPS: usize = 4096;
/// The channel powers relative to the station: pilot, Sync, Paging, the other channels together (dB).
const POWERS_DB: [f64; 4] = [-7.0, -16.0, -12.0, -2.0];

/// The pulse of the test signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pulse {
    /// The 48-tap baseband filter of C.S0002-C.
    Fir,
    /// A root raised cosine of roll-off 0.2 over 12 chips.
    Rrc,
}

/// One base station of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct BaseStation {
    /// Its Sync Channel Message; `pilot_pn` is the station's PN offset, and `sys_time` is the value of the message whose first
    /// frame starts at the even second (each later message adds 3).
    pub msg: SyncMessage,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The amplitude.
    pub amp: f32,
}

/// The stations of the stack's own test signal: PN 168 and PN 12 of one system.
pub fn scene() -> Vec<BaseStation> {
    let msg = |pn: u16| SyncMessage {
        p_rev: 6,
        min_p_rev: 6,
        sid: 4139,
        nid: 1,
        pilot_pn: pn,
        lc_state: 0x1_2345_6789,
        sys_time: 18_484_000_000,
        lp_sec: 18,
        ltm_off: 4,
        daylt: false,
        prat: 0,
        cdma_freq: 283,
        ext_cdma_freq: 283,
    };
    vec![
        BaseStation {
            msg: msg(168),
            offset_hz: 0.0,
            amp: 1.0,
        },
        BaseStation {
            msg: msg(12),
            offset_hz: 150.0,
            amp: 0.6,
        },
    ]
}

/// The system chip at which the stack's own test signal starts.
pub const SCENE_START_CHIP: i64 = 5000;

/// The state of one station.
struct Station {
    cfg: BaseStation,
    gains: [f32; 4],
    enc: Encoder,
    /// The frame whose symbols are held.
    frame: Option<i64>,
    symbols: [u8; 128],
    rng: Rng,
    /// The random data of the Paging and the other channels in the current Walsh period, and that period.
    data: [f32; 9],
    walsh_period: i64,
}

impl Station {
    fn new(cfg: BaseStation, seed: u64) -> Self {
        let g = POWERS_DB.map(|db| 10f64.powf(db / 20.0) as f32);
        Station {
            gains: [g[0], g[1], g[2], g[3] / 8f32.sqrt()],
            enc: Encoder::new(CDMA2000_K9),
            frame: None,
            symbols: [0; 128],
            rng: Rng::new(seed),
            data: [1.0; 9],
            walsh_period: i64::MIN,
            cfg,
        }
    }

    /// The 32 bits of Sync Channel frame `f` (frames counted from the even second of the station's time).
    fn frame_bits(&self, f: i64) -> [u8; FRAME_BITS] {
        let mut msg = self.cfg.msg.clone();
        let frames = msg.frames();
        let n = frames.len() as i64;
        let m = f.div_euclid(n);
        msg.sys_time = (msg.sys_time as i64 + 3 * m).max(0) as u64;
        msg.frames()[f.rem_euclid(n) as usize]
    }

    /// Makes the symbols of frame `f`, the encoder run on from the frame before (or from two frames before `f` when it is
    /// not the next one).
    fn make(&mut self, f: i64) {
        if self.frame != Some(f - 1) {
            self.enc.set_state(0);
            for g in f - 2..f {
                let mut out = Vec::new();
                self.enc.encode(&self.frame_bits(g), &mut out);
            }
        }
        let mut code = Vec::with_capacity(64);
        self.enc.encode(&self.frame_bits(f), &mut code);
        self.symbols = frame_symbols(&code);
        self.frame = Some(f);
    }

    /// The baseband chip of system chip `t`, before the pulse.
    fn chip(&mut self, t: i64) -> Complex32 {
        let u = t - 64 * i64::from(self.cfg.msg.pilot_pn);
        let f = u.div_euclid(PN_PERIOD as i64);
        let pos = u.rem_euclid(PN_PERIOD as i64) as usize;
        if self.frame != Some(f) {
            self.make(f);
        }
        let wp = u.div_euclid(64);
        if wp != self.walsh_period {
            self.walsh_period = wp;
            for d in self.data.iter_mut() {
                *d = if self.rng.next_u64() & 1 == 0 { 1.0 } else { -1.0 };
            }
        }
        let w = pos % 64;
        let sym = 1.0 - 2.0 * f32::from(self.symbols[pos / 256]);
        let mut s = self.gains[0] + self.gains[1] * sym * f32::from(hadamard(32, w));
        s += self.gains[2] * self.data[0] * f32::from(hadamard(1, w));
        for (k, &d) in self.data[1..].iter().enumerate() {
            s += self.gains[3] * d * f32::from(hadamard(8 + k, w));
        }
        let (i, q) = pn_chip(pos);
        Complex32::new(f32::from(i), f32::from(q)) * (s * self.cfg.amp * std::f32::consts::FRAC_1_SQRT_2)
    }
}

/// The generator.
pub struct CdmaGenerator {
    rate: f64,
    stations: Vec<(Station, Nco)>,
    taps: Vec<f32>,
    t: i64,
    hist: Vec<Complex32>,
    native: Vec<Complex32>,
    scale: f32,
    resampler: Option<FractionalResampler<Complex32>>,
    out: Vec<Complex32>,
    pos: usize,
}

impl CdmaGenerator {
    /// The `stations` at output rate `rate`, the first sample at system chip `start_chip`, with pulse `pulse`, from `seed`.
    pub fn new(rate: f64, stations: Vec<BaseStation>, start_chip: i64, pulse: Pulse, seed: u64) -> Self {
        let mut g = Self::build(rate, stations.clone(), start_chip, pulse, seed);
        let native = CHIP_RATE * NATIVE_SPC as f64;
        let mut probe = Self::build(native, stations, start_chip, pulse, seed);
        let mut x = vec![Complex32::default(); (0.05 * native) as usize];
        probe.generate(&mut x);
        let p = x.iter().map(|v| v.norm_sqr()).sum::<f32>() / x.len() as f32;
        if p > 0.0 {
            g.scale = p.sqrt().recip();
        }
        g
    }

    fn build(rate: f64, stations: Vec<BaseStation>, start_chip: i64, pulse: Pulse, seed: u64) -> Self {
        let native = CHIP_RATE * NATIVE_SPC as f64;
        let taps = match pulse {
            Pulse::Fir => baseband_fir().map(|v| v as f32).to_vec(),
            Pulse::Rrc => pulse::taps(Shape::Rrc { beta: 0.2 }, NATIVE_SPC as f64, 12.0, Norm::Peak)
                .unwrap_or_else(|_| vec![1.0]),
        };
        let stations = stations
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                let nco = Nco::new(native, s.offset_hz);
                (Station::new(s, seed ^ (0x4344_0000 + i as u64)), nco)
            })
            .collect();
        CdmaGenerator {
            rate,
            stations,
            hist: vec![Complex32::default(); taps.len()],
            taps,
            t: start_chip,
            native: Vec::new(),
            scale: 1.0,
            resampler: ((rate - native).abs() > 1e-6)
                .then(|| FractionalResampler::with_rates(native, rate).ok())
                .flatten(),
            out: Vec::new(),
            pos: 0,
        }
    }

    fn refill(&mut self) {
        let n = BLOCK_CHIPS * NATIVE_SPC;
        let mut sum = vec![Complex32::default(); n];
        let mut part = vec![Complex32::default(); n];
        for (st, nco) in self.stations.iter_mut() {
            part.fill(Complex32::default());
            for c in 0..BLOCK_CHIPS {
                part[NATIVE_SPC * c] = st.chip(self.t + c as i64);
            }
            nco.mix_in_place(&mut part);
            for (s, p) in sum.iter_mut().zip(&part) {
                *s += p;
            }
        }
        self.t += BLOCK_CHIPS as i64;
        let l = self.taps.len();
        let mut ext = std::mem::take(&mut self.hist);
        ext.truncate(l - 1);
        ext.extend_from_slice(&sum);
        self.native.clear();
        for i in 0..n {
            let mut acc = Complex32::default();
            for (k, &h) in self.taps.iter().enumerate() {
                acc += ext[i + l - 1 - k] * h;
            }
            self.native.push(acc * self.scale);
        }
        self.hist = ext[ext.len() - (l - 1)..].to_vec();
        self.out.clear();
        self.pos = 0;
        match self.resampler.as_mut() {
            Some(r) => r.process(&self.native, &mut self.out),
            None => self.out.extend_from_slice(&self.native),
        }
    }
}

impl Generator for CdmaGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos >= self.out.len() {
                self.refill();
                continue;
            }
            let n = (self.out.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.out[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        out.len()
    }
    /// The SNR of the test signal is measured over 2.4576 MHz (two samples a chip), as the stack's tests state it.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(2.0 * CHIP_RATE).with_symbol_rate(CHIP_RATE))
    }
}
