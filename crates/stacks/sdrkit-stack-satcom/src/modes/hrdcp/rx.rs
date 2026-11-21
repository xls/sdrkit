//! The HRDCP receiver: the band scanner of `goes_dcs` finds the 2 s carriers; each carrier gets a follower: mixer and
//! decimator to 9.6 kHz, the carrier frequency from the lag products of the unmodulated part, the root raised cosine matched
//! filter, the search for the 192 known symbols of the preamble and marker in six pieces, the offset QPSK demodulator with its
//! carrier and timing loops, the soft Viterbi decoder and the message layer of [`super::message`].

use super::message::{self, Decoded, FRAME_CODED};
use crate::modes::goes::body::AddressCode;
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::pulse::rrc;
use sdrkit_algo_fec::conv::{codes, Ends, Viterbi};
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_proto_ccsds::channel::ChannelDecoder;
use std::f64::consts::{FRAC_PI_4, PI, TAU};

/// The rate of a follower, Hz.
pub const FOLLOW_RATE: f64 = 9600.0;
/// Samples a symbol in a follower.
pub const SPS: usize = 8;
/// Symbols a second on each rail.
pub const SYMBOL_RATE: f64 = 1200.0;
/// The width a follower keeps around its carrier, Hz.
const FOLLOW_WIDTH_HZ: f64 = 3000.0;
/// The preamble: A05050A0 four times.
const PREAMBLE: [u8; 16] = [
    0xA0, 0x50, 0x50, 0xA0, 0xA0, 0x50, 0x50, 0xA0, 0xA0, 0x50, 0x50, 0xA0, 0xA0, 0x50, 0x50, 0xA0,
];
/// The marker.
const MARKER: [u8; 8] = [0x03, 0x47, 0x76, 0xC7, 0x27, 0x28, 0x95, 0xB0];
/// Known symbols: preamble and marker.
pub const KNOWN: usize = 192;
/// Follower seconds at the first and the second marker search.
const SEARCH_AT_S: [f64; 2] = [2.6, 4.5];
/// The least normalised correlation of the known symbols.
const MIN_RHO: f64 = 0.5;
/// Symbols after which a follower ends (70 s).
const MAX_SYMBOLS: usize = 84_000;
/// Decoded bits of a frame.
const FRAME_BITS: usize = 8 * FRAME_CODED;
/// Viterbi steps decoded past a frame before it is read.
const SETTLE_STEPS: usize = 64;

/// The known symbols, +1 for a 0 bit and −1 for a 1 bit.
pub fn known_symbols() -> [f32; KNOWN] {
    let mut s = [0f32; KNOWN];
    for (i, o) in s.iter_mut().enumerate() {
        let b = if i < 128 {
            PREAMBLE[i / 8]
        } else {
            MARKER[(i - 128) / 8]
        };
        *o = if (b >> (7 - i % 8)) & 1 == 0 {
            1.0
        } else {
            -1.0
        };
    }
    s
}

/// The matched filter: root raised cosine, roll-off 0.5, ±5 symbols at 8 samples a symbol, unit energy.
fn matched_taps() -> Vec<f32> {
    let half = 5 * SPS as i64;
    let t: Vec<f64> = (-half..=half)
        .map(|i| rrc(i as f64 / SPS as f64, 0.5))
        .collect();
    let e = t.iter().map(|v| v * v).sum::<f64>().sqrt();
    t.iter().map(|v| (v / e) as f32).collect()
}

/// What a follower is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Looking for the marker.
    Marker,
    /// Decoding.
    Data,
    /// Finished.
    Done,
}

/// The end of a follower.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// A message whose header decoded (good or not).
    Message(Box<Decoded>),
    /// A header that did not decode: a bad message.
    BadHeader,
    /// No marker found: not counted.
    NoMarker,
}

/// One followed carrier.
pub struct Follower {
    /// The carrier, Hz from the input's 0 Hz (the line found).
    pub offset: f64,
    /// The line over the noise floor when found, dB.
    pub snr_db: f64,
    /// Signal time of the first sample, s.
    pub start_s: f64,
    mix: Channelizer,
    tmp: Vec<Complex32>,
    raw: Vec<Complex32>,
    turned: Vec<Complex32>,
    mf: Vec<Complex32>,
    taps: Vec<f32>,
    /// The carrier error found on the unmodulated part, Hz (`None` until it is measured).
    pub df: Option<f64>,
    /// What the follower is doing.
    pub phase: Phase,
    tries: usize,
    // Demodulator state.
    t: f64,
    period: f64,
    freq: f64,
    theta: f64,
    amp: f64,
    prev_i: f64,
    prev_q: f64,
    prev_qs: Complex32,
    symbols: usize,
    llr: Vec<i8>,
    frames: Option<usize>,
    /// The end of the follower, once known.
    pub outcome: Option<Outcome>,
    code: AddressCode,
    rs: ChannelDecoder,
}

impl Follower {
    /// A follower of the carrier at `offset` Hz in an input at `rate_in`, its first sample at signal time `start_s`.
    ///
    /// # Errors
    ///
    /// A [`ConfigError`] when the rate cannot hold the follower's channel.
    pub fn new(rate_in: f64, offset: f64, start_s: f64, snr_db: f64) -> Result<Self, ConfigError> {
        let mut c = ChannelConfig::new(rate_in, offset, FOLLOW_WIDTH_HZ, FOLLOW_RATE);
        c.exact_rate = true;
        Ok(Follower {
            offset,
            snr_db,
            start_s,
            mix: Channelizer::new(c)?,
            tmp: Vec::new(),
            raw: Vec::with_capacity((6.0 * FOLLOW_RATE) as usize),
            turned: Vec::with_capacity((6.0 * FOLLOW_RATE) as usize),
            mf: Vec::with_capacity((6.0 * FOLLOW_RATE) as usize),
            taps: matched_taps(),
            df: None,
            phase: Phase::Marker,
            tries: 0,
            t: 0.0,
            period: SPS as f64,
            freq: 0.0,
            theta: 0.0,
            amp: 1.0,
            prev_i: 0.0,
            prev_q: 0.0,
            prev_qs: Complex32::default(),
            symbols: 0,
            llr: Vec::new(),
            frames: None,
            outcome: None,
            code: AddressCode::new(),
            rs: ChannelDecoder::new(message::channel()).expect("the CCSDS code"),
        })
    }

    /// Bytes decoded so far (an estimate from the symbols).
    pub fn bytes_so_far(&self) -> usize {
        self.symbols / 8
    }

    /// Takes input samples.
    pub fn process(&mut self, x: &[Complex32]) {
        if self.phase == Phase::Done {
            return;
        }
        self.tmp.clear();
        self.mix.process(x, &mut self.tmp);
        self.raw.extend_from_slice(&self.tmp);
        if self.df.is_none() && self.raw.len() as f64 >= 0.46 * FOLLOW_RATE {
            self.df = Some(self.carrier_error());
        }
        if self.df.is_some() {
            self.filter();
        }
        match self.phase {
            Phase::Marker => {
                if let Some(&at) = SEARCH_AT_S.get(self.tries) {
                    if self.mf.len() as f64 >= at * FOLLOW_RATE {
                        self.tries += 1;
                        if self.search() {
                            self.phase = Phase::Data;
                        } else if self.tries == SEARCH_AT_S.len() {
                            self.finish(Outcome::NoMarker);
                        }
                    }
                }
            }
            Phase::Data => self.demodulate(),
            Phase::Done => {}
        }
    }

    fn finish(&mut self, o: Outcome) {
        self.phase = Phase::Done;
        self.outcome = Some(o);
        self.raw = Vec::new();
        self.turned = Vec::new();
        self.mf = Vec::new();
        self.llr = Vec::new();
    }

    /// The carrier error on the unmodulated part (0.02 s to 0.45 s): a 16-sample running sum, the phase of its lag-16 product
    /// (±300 Hz), refined by the lag-128 product.
    fn carrier_error(&self) -> f64 {
        let a = (0.02 * FOLLOW_RATE) as usize;
        let b = (0.45 * FOLLOW_RATE) as usize;
        let x = &self.raw[a..b];
        let mut sums = Vec::with_capacity(x.len());
        let mut s = Complex32::default();
        for (i, &v) in x.iter().enumerate() {
            s += v;
            if i >= 16 {
                s -= x[i - 16];
            }
            if i >= 15 {
                sums.push(s);
            }
        }
        let lag = |l: usize| -> f64 {
            let mut acc = Complex32::default();
            for k in l..sums.len() {
                acc += sums[k] * sums[k - l].conj();
            }
            f64::from(acc.im.atan2(acc.re))
        };
        let coarse = lag(16) / TAU * FOLLOW_RATE / 16.0;
        let step = FOLLOW_RATE / 128.0;
        let fine = lag(128) / TAU * step;
        let k = ((coarse - fine) / step).round();
        fine + k * step
    }

    /// Turns the new raw samples back by the carrier error and filters them into `mf`.
    fn filter(&mut self) {
        let df = self.df.unwrap_or(0.0);
        while self.turned.len() < self.raw.len() {
            let i = self.turned.len();
            let ph = -TAU * (df * i as f64 / FOLLOW_RATE).fract();
            self.turned
                .push(self.raw[i] * Complex32::new(ph.cos() as f32, ph.sin() as f32));
        }
        let half = self.taps.len() / 2;
        // Output k is centred on sample k; it needs the samples up to k + half.
        while self.mf.len() + half < self.turned.len() {
            let k = self.mf.len();
            let mut acc = Complex32::default();
            for (j, &tap) in self.taps.iter().enumerate() {
                if let Some(i) = (k + j).checked_sub(half) {
                    acc += self.turned[i] * tap;
                }
            }
            self.mf.push(acc);
        }
    }

    /// The matched filter output at fractional sample `t` (linear interpolation).
    fn at(&self, t: f64) -> Complex32 {
        let i = t.floor();
        let f = (t - i) as f32;
        let i = i as usize;
        let a = self.mf.get(i).copied().unwrap_or_default();
        let b = self.mf.get(i + 1).copied().unwrap_or_default();
        a * (1.0 - f) + b * f
    }

    /// The correlation of the known symbols starting at sample `t`: the six piece sums and the normalised magnitude.
    fn correlate(&self, t: f64, known: &[f32; KNOWN]) -> ([Complex32; 6], f64) {
        let mut pieces = [Complex32::default(); 6];
        let mut mag = 0f64;
        for (i, &s) in known.iter().enumerate() {
            let y = self.at(t + (i * SPS) as f64);
            pieces[i / 32] += y * s;
            mag += f64::from(y.norm());
        }
        let sum: f64 = pieces.iter().map(|c| f64::from(c.norm())).sum();
        (pieces, if mag > 0.0 { sum / mag } else { 0.0 })
    }

    /// Searches the buffer for the known symbols; on success sets the demodulator up at the first data symbol.
    fn search(&mut self) -> bool {
        let known = known_symbols();
        let span = KNOWN * SPS;
        if self.mf.len() < span + 2 {
            return false;
        }
        let mut best = (0.0, 0usize);
        for t in 0..self.mf.len() - span - 1 {
            let (_, rho) = self.correlate(t as f64, &known);
            if rho > best.0 {
                best = (rho, t);
            }
        }
        if best.0 < MIN_RHO {
            return false;
        }
        // A parabola through the neighbours places the start between samples.
        let t0 = best.1 as f64;
        let r = |t: f64| self.correlate(t, &known).1;
        let (a, b, c) = (r(t0 - 1.0), best.0, r(t0 + 1.0));
        let den = a - 2.0 * b + c;
        let frac = if t0 >= 1.0 && den < 0.0 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let t = t0 + frac;
        let (pieces, _) = self.correlate(t, &known);
        // The phases of the pieces, unwrapped, by a least-squares line against the symbol index of their centres.
        let mut ph: Vec<f64> = pieces.iter().map(|c| f64::from(c.im.atan2(c.re))).collect();
        for k in 1..ph.len() {
            while ph[k] - ph[k - 1] > PI {
                ph[k] -= TAU;
            }
            while ph[k] - ph[k - 1] < -PI {
                ph[k] += TAU;
            }
        }
        let xs: Vec<f64> = (0..6).map(|k| 32.0 * k as f64 + 15.5).collect();
        let mx = xs.iter().sum::<f64>() / 6.0;
        let my = ph.iter().sum::<f64>() / 6.0;
        let sxy: f64 = xs.iter().zip(&ph).map(|(x, y)| (x - mx) * (y - my)).sum();
        let sxx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
        let slope = sxy / sxx;
        self.freq = slope;
        self.theta = my + slope * (KNOWN as f64 - mx) - FRAC_PI_4;
        let sum: f64 = pieces.iter().map(|c| f64::from(c.norm())).sum();
        self.amp = (sum / (KNOWN as f64 * std::f64::consts::SQRT_2)).max(1e-12);
        self.t = t + (KNOWN * SPS) as f64;
        self.period = SPS as f64;
        true
    }

    /// Demodulates every symbol whose samples are in, and decodes when enough are in.
    fn demodulate(&mut self) {
        let p = SPS as f64;
        while self.t + p / 2.0 + 2.0 < self.mf.len() as f64 {
            let rot_i = Complex32::from_polar(1.0, -self.theta as f32);
            let rot_q = Complex32::from_polar(1.0, -(self.theta + self.freq / 2.0) as f32);
            let is = self.at(self.t) * rot_i / self.amp as f32;
            let qs = self.at(self.t + p / 2.0) * rot_q / self.amp as f32;
            let (i, q) = (f64::from(is.re), f64::from(qs.im));
            // Carrier loop.
            let ec = i.signum() * f64::from(is.im) - q.signum() * f64::from(qs.re);
            self.freq += 3.1e-5 * ec;
            self.theta += self.freq + 0.0055 * ec;
            // Timing loop: the in-phase part of the last Q sample and the quadrature part of the I sample are the midpoints.
            let et = (f64::from(self.prev_qs.re) * (self.prev_i - i)
                + f64::from(is.im) * (self.prev_q - q))
                .clamp(-2.0, 2.0);
            if self.symbols > 0 {
                self.period = (self.period + 0.0002 * et).clamp(p * 0.998, p * 1.002);
                self.t += self.period + 0.02 * et;
            } else {
                self.t += self.period;
            }
            self.prev_i = i;
            self.prev_q = q;
            self.prev_qs = qs;
            self.symbols += 1;
            let l = |v: f64| (v * 32.0).clamp(-127.0, 127.0) as i8;
            self.llr.push(l(i));
            self.llr.push(l(q));
            if self.decode_ready() || self.phase == Phase::Done {
                return;
            }
        }
    }

    /// Decodes the header once its frame is in, and the message once every frame is in. Returns true when the follower ended.
    fn decode_ready(&mut self) -> bool {
        let steps = self.llr.len() / 2;
        match self.frames {
            None if steps >= FRAME_BITS + SETTLE_STEPS => {
                let mut bytes = self.viterbi(FRAME_BITS + SETTLE_STEPS, FRAME_BITS);
                bytes.truncate(FRAME_CODED);
                let r = self.rs.decode(&mut bytes);
                let header = match r {
                    Ok(r) if r.ok() => message::read_header(&bytes, &mut self.code),
                    _ => None,
                };
                match header {
                    Some(h) => {
                        self.frames = Some(message::frames_of(h.length));
                        false
                    }
                    None => {
                        self.finish(Outcome::BadHeader);
                        true
                    }
                }
            }
            Some(f) if steps >= f * FRAME_BITS + 8 + SETTLE_STEPS => {
                let mut bytes = self.viterbi(f * FRAME_BITS + 8 + SETTLE_STEPS, f * FRAME_BITS);
                bytes.truncate(f * FRAME_CODED);
                let out = message::decode(&mut bytes, &mut self.rs, &mut self.code);
                self.finish(match out {
                    Some(d) => Outcome::Message(Box::new(d)),
                    None => Outcome::BadHeader,
                });
                true
            }
            _ if self.symbols >= MAX_SYMBOLS => {
                self.finish(Outcome::BadHeader);
                true
            }
            _ => false,
        }
    }

    /// Decodes the first `steps` steps from state 0 and returns the first `keep` bits as bytes.
    fn viterbi(&self, steps: usize, keep: usize) -> Vec<u8> {
        let Ok(mut v) = Viterbi::new(codes::CCSDS_K7, steps) else {
            return Vec::new();
        };
        let mut bits = vec![0u8; steps];
        if v.decode_only(&self.llr[..2 * steps], Ends::Start(0), &mut bits)
            .is_err()
        {
            return Vec::new();
        }
        bits[..keep]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_known_symbols_start_with_the_preamble() {
        let k = known_symbols();
        // A0: 1010 0000.
        assert_eq!(&k[..8], &[-1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        // 03 starts the marker.
        assert_eq!(&k[128..136], &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0]);
        let e: f32 = matched_taps().iter().map(|t| t * t).sum();
        assert!((e - 1.0).abs() < 1e-5);
    }
}
