//! The VDL Mode 3 receiver: the channel at 84 ksps (8 samples a symbol), the noise floor, the search for the four training
//! sequences by differential correlation, and the burst decoder (carrier from the training, a decision-directed phase loop, the
//! length of an S2* burst from its power, the Golay and Reed-Solomon words).

use super::phy::{
    bits_of_steps, BurstKind, OctetOrder, Payload, PayloadDecoder, Training, S2STAR_LENGTHS,
    SYMBOL_RATE, TRAINING_LEN, VD_SYMBOLS,
};
use crate::d8psk::{correlate, turns};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::design_by_taps;
use sdrkit_algo_dsp::floor::DEFAULT_MIN_FLOOR;
use sdrkit_algo_dsp::interp::{CatmullRom, Reader};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::{Complex32, ConfigError};
use std::f64::consts::{FRAC_PI_4, TAU};

/// The rate after the first stage, 16 samples a symbol.
pub const STAGE_RATE: f64 = 16.0 * SYMBOL_RATE;
/// The working rate, 8 samples a symbol.
pub const WORK_RATE: f64 = 8.0 * SYMBOL_RATE;
/// Samples a symbol at the working rate.
pub const SPS: usize = 8;
/// The cut-off of the channel filter, Hz: above the raised-cosine spectrum (8.4 kHz) with room for an offset, and with less
/// noise than a filter as wide as the channel.
pub const CUTOFF_HZ: f64 = 9500.0;
/// The taps of the channel filter.
pub const TAPS: usize = 63;
/// The noise bandwidth of the channel filter for the SNR, Hz.
pub const NOISE_BW_HZ: f64 = 2.0 * CUTOFF_HZ;
/// Samples of a noise block.
const BLOCK: usize = 128;
/// Noise blocks kept (about 0.3 s).
const BLOCKS: usize = 200;
/// Samples searched after a start for the peak.
const PEAK_SPAN: usize = 12;
/// A training at least this good marks the end of the burst before it.
const CLEAR: f64 = 0.9;
/// The longest burst after the training, symbols, with the margin that decoding waits for.
const LONGEST: usize = VD_SYMBOLS + 10;
/// The gains of the decision-directed loop.
const LOOP_PHASE: f64 = 0.2;
const LOOP_FREQ: f64 = 0.02;
/// The phase error of a good burst, units of pi/4.
pub const EVM_GOOD: f64 = 0.25;

/// A training sequence found by the search.
#[derive(Clone, Copy, Debug)]
struct Find {
    /// Fractional position of the first training symbol, absolute working-rate sample.
    t: f64,
    /// S2 (true) or S1.
    second: bool,
    metric: f64,
}

/// A burst decoded (EVM within the largest allowed).
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// Absolute working-rate sample of the end of the training.
    pub end_of_training: f64,
    /// The kind.
    pub kind: BurstKind,
    /// The training sequence.
    pub training: Training,
    /// Symbols after the training.
    pub symbols: usize,
    /// The payload.
    pub payload: Payload,
    /// The RMS phase error of the decisions, units of pi/4.
    pub evm: f64,
    /// Mean power of the training symbols, dBFS.
    pub level_dbfs: f64,
    /// Es/N0, dB.
    pub snr_db: f64,
    /// Carrier offset, Hz.
    pub cfo_hz: f64,
    /// Every Golay word decoded, the EVM under [`EVM_GOOD`] and, for data, the Reed-Solomon word good.
    pub good: bool,
}

/// What the receiver did with the samples of one call.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts {
    /// Training sequences found.
    pub trainings: u64,
    /// Finds dropped: their phase error was over the largest allowed.
    pub dropped: u64,
}

/// The receiver (module documentation).
pub struct Vdl3Rx {
    ch: Channelizer,
    decim: FirDecimator<Complex32>,
    stage: Vec<Complex32>,
    work: Vec<Complex32>,
    /// Working-rate samples, `y[0]` at absolute index `base`.
    y: Vec<Complex32>,
    base: u64,
    /// The next start the search looks at.
    cursor: u64,
    block_sum: f64,
    block_n: usize,
    blocks: Vec<f64>,
    block_at: usize,
    floor: Option<f64>,
    turns: [[Complex32; 16]; 2],
    finds: Vec<Find>,
    /// Finds waiting for their samples.
    pending: usize,
    busy_until: f64,
    payload: PayloadDecoder,
    bits: Vec<u8>,
    decisions: Vec<u8>,
    /// The training threshold.
    pub threshold: f64,
    /// The largest phase error kept, units of pi/4.
    pub evm_max: f64,
    /// The bit order of the Reed-Solomon octets.
    pub order: OctetOrder,
}

impl Vdl3Rx {
    /// A receiver for input at `rate` with the channel `offset_hz` from the centre.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] when the rate cannot hold the channel.
    pub fn new(rate: f64, offset_hz: f64) -> Result<Self, ConfigError> {
        let mut cfg = ChannelConfig::new(rate, offset_hz, 40e3, STAGE_RATE);
        cfg.exact_rate = true;
        let ch = Channelizer::new(cfg).map_err(|_| ConfigError::RateTooLow {
            need: 2.0 * (offset_hz.abs() + 20e3),
            got: rate,
        })?;
        let taps = design_by_taps(TAPS, CUTOFF_HZ / STAGE_RATE, Window::Hamming)
            .unwrap_or_else(|_| vec![1.0]);
        let s1 = super::phy::steps(false);
        let s2 = super::phy::steps(true);
        Ok(Vdl3Rx {
            ch,
            decim: FirDecimator::new(&taps, 2),
            stage: Vec::with_capacity(1 << 14),
            work: Vec::with_capacity(1 << 13),
            y: Vec::with_capacity(1 << 16),
            base: 0,
            cursor: 0,
            block_sum: 0.0,
            block_n: 0,
            blocks: Vec::with_capacity(BLOCKS),
            block_at: 0,
            floor: None,
            turns: [turns(|k| s1[k]), turns(|k| s2[k])],
            finds: Vec::with_capacity(64),
            pending: 0,
            busy_until: 0.0,
            payload: PayloadDecoder::default(),
            bits: Vec::with_capacity(1024),
            decisions: Vec::with_capacity(256),
            threshold: 0.8,
            evm_max: 0.3,
            order: OctetOrder::LsbFirst,
        })
    }

    /// The noise floor: the lowest mean power of the recent blocks, at least [`DEFAULT_MIN_FLOOR`] (digital silence gives
    /// this floor, so the search runs and the SNR of a burst stays finite).
    pub fn floor(&self) -> Option<f64> {
        self.floor
    }

    /// Working-rate samples so far.
    pub fn samples(&self) -> u64 {
        self.base + self.y.len() as u64
    }

    fn at(&self, t: f64) -> Complex32 {
        Reader::new(CatmullRom).at(&self.y, t - self.base as f64)
    }

    /// Runs input samples; calls `on_burst` for each burst decoded.
    pub fn process(
        &mut self,
        x: &[Complex32],
        counts: &mut Counts,
        mut on_burst: impl FnMut(&Decoded),
    ) {
        self.stage.clear();
        self.ch.process(x, &mut self.stage);
        self.work.clear();
        self.decim.process(&self.stage, &mut self.work);
        for &v in &self.work {
            let p = f64::from(v.norm_sqr());
            self.block_sum += p;
            self.block_n += 1;
            if self.block_n == BLOCK {
                let m = self.block_sum / BLOCK as f64;
                if self.blocks.len() < BLOCKS {
                    self.blocks.push(m);
                } else {
                    self.blocks[self.block_at] = m;
                    self.block_at = (self.block_at + 1) % BLOCKS;
                }
                self.floor = self
                    .blocks
                    .iter()
                    .copied()
                    .reduce(f64::min)
                    .map(|f| f.max(f64::from(DEFAULT_MIN_FLOOR)));
                self.block_sum = 0.0;
                self.block_n = 0;
            }
        }
        self.y.extend_from_slice(&self.work);
        self.search(counts);
        self.decode_ready(counts, &mut on_burst);
        // Keep what a waiting find and the search still read.
        let keep_from = self
            .finds
            .get(self.finds.len() - self.pending)
            .map_or(self.cursor, |f| f.t.floor().max(0.0) as u64)
            .min(self.cursor)
            .saturating_sub(4 * SPS as u64);
        if keep_from > self.base + 4096 {
            let n = (keep_from - self.base) as usize;
            self.y.drain(..n);
            self.base += n as u64;
        }
        // Old finds are needed only for the length test of the bursts still waiting.
        let horizon = self.samples() as f64 - ((TRAINING_LEN + LONGEST) * SPS * 3) as f64;
        let old = self.finds.len() - self.pending;
        let gone = self.finds[..old]
            .iter()
            .take_while(|f| f.t < horizon)
            .count();
        self.finds.drain(..gone);
    }

    /// The 16 samples one symbol apart from `j` (relative index in `y`).
    #[inline]
    fn gather(&self, j: usize) -> [Complex32; 16] {
        std::array::from_fn(|k| self.y[j + k * SPS])
    }

    /// The training metric of the 16 samples at `j` for sequence `second`, and the correlation.
    fn metric_at(&self, j: usize, second: bool) -> (f64, Complex32, f64) {
        let z = self.gather(j);
        let e: f64 = z.iter().map(|v| f64::from(v.norm_sqr())).sum();
        let d = correlate(&z, &self.turns[usize::from(second)]);
        let m = if e > 0.0 {
            f64::from(d.norm()) / (e * 15.0 / 16.0)
        } else {
            0.0
        };
        (m, d, e)
    }

    fn search(&mut self, counts: &mut Counts) {
        let Some(floor) = self.floor else {
            self.cursor = self.cursor.max(self.base);
            return;
        };
        let span = 15 * SPS + PEAK_SPAN + 1;
        let end = self.samples();
        while self.cursor + span as u64 <= end {
            let j = (self.cursor - self.base) as usize;
            let z = self.gather(j);
            let e: f64 = z.iter().map(|v| f64::from(v.norm_sqr())).sum();
            if e / 16.0 <= 4.0 * floor {
                self.cursor += 1;
                continue;
            }
            let (m1, ..) = self.metric_at(j, false);
            let (m2, ..) = self.metric_at(j, true);
            let (m, second) = if m2 > m1 { (m2, true) } else { (m1, false) };
            if m <= self.threshold {
                self.cursor += 1;
                continue;
            }
            // The peak over the next samples, with a parabola through its neighbours.
            let ms: Vec<f64> = (0..=PEAK_SPAN)
                .map(|i| self.metric_at(j + i, second).0)
                .collect();
            let (best, &mb) = ms
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap_or((0, &m));
            let frac = if best > 0 && best < PEAK_SPAN {
                let (a, b, c) = (ms[best - 1], mb, ms[best + 1]);
                let den = a - 2.0 * b + c;
                if den.abs() > 1e-12 {
                    (0.5 * (a - c) / den).clamp(-0.5, 0.5)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            let t = (self.cursor + best as u64) as f64 + frac;
            self.finds.push(Find {
                t,
                second,
                metric: mb,
            });
            self.pending += 1;
            counts.trainings += 1;
            // Finds within 8 symbols are one training.
            self.cursor += (best + 8 * SPS) as u64;
        }
    }

    fn decode_ready(&mut self, counts: &mut Counts, on_burst: &mut impl FnMut(&Decoded)) {
        let have = self.samples() as f64;
        while self.pending > 0 {
            let i = self.finds.len() - self.pending;
            let f = self.finds[i];
            if f.t + ((TRAINING_LEN + LONGEST) * SPS + 4) as f64 > have {
                break;
            }
            self.pending -= 1;
            // A training inside the last good burst is that burst's data.
            if f.t < self.busy_until {
                continue;
            }
            match self.decode(f, i) {
                Some(d) => {
                    if d.good {
                        self.busy_until = f.t + ((TRAINING_LEN + d.symbols) * SPS) as f64;
                    }
                    on_burst(&d);
                }
                None => counts.dropped += 1,
            }
        }
    }

    /// The power of the symbol `k` symbols after the first training symbol at `t`.
    fn symbol_power(&self, t: f64, k: usize) -> f64 {
        f64::from(self.at(t + (k * SPS) as f64).norm_sqr())
    }

    /// The length of an S2* burst at `t`: the first of 128, 40, 32 and 16 symbols that ends before a later clear training,
    /// whose last payload symbols are on and the symbols after it off. 16 when none is.
    fn s2star_length(&self, f: Find, index: usize, train_power: f64) -> usize {
        let mean = |a: usize, b: usize| {
            (a..=b)
                .map(|k| self.symbol_power(f.t, TRAINING_LEN + k))
                .sum::<f64>()
                / (b + 1 - a) as f64
        };
        for c in S2STAR_LENGTHS {
            let end = f.t + ((TRAINING_LEN + c) * SPS) as f64;
            let reaches = self.finds[index + 1..]
                .iter()
                .any(|g| g.metric >= CLEAR && g.t > f.t + (8 * SPS) as f64 && g.t < end);
            if reaches {
                continue;
            }
            let last = mean(c - 4, c - 1);
            if last > 0.4 * train_power && mean(c + 2, c + 4) < 0.5 * last {
                return c;
            }
        }
        16
    }

    fn decode(&mut self, f: Find, index: usize) -> Option<Decoded> {
        let z: [Complex32; 16] = std::array::from_fn(|k| self.at(f.t + (k * SPS) as f64));
        let e: f64 = z.iter().map(|v| f64::from(v.norm_sqr())).sum();
        let d = correlate(&z, &self.turns[usize::from(f.second)]);
        let star = d.re < 0.0;
        let training = match (f.second, star) {
            (false, false) => Training::S1,
            (false, true) => Training::S1Star,
            (true, false) => Training::S2,
            (true, true) => Training::S2Star,
        };
        let d = if star { -d } else { d };
        let mut w = f64::from(d.arg());
        let cfo_hz = w / TAU * SYMBOL_RATE;
        let train_power = e / 16.0;
        // The phase of each training symbol relative to the first, in units of pi/4.
        let steps = training.steps();
        let mut cum = [0i32; TRAINING_LEN];
        for k in 1..TRAINING_LEN {
            cum[k] = cum[k - 1] + steps[k];
        }
        // The phase and its slope over the training by least squares, around the differential estimate.
        let mut acc = Complex32::default();
        for k in 0..TRAINING_LEN {
            let ph = w * k as f64 + FRAC_PI_4 * f64::from(cum[k]);
            acc += z[k] * Complex32::from_polar(1.0, -ph as f32);
        }
        let mean = f64::from(acc.arg());
        let (mut sxy, mut sxx) = (0.0, 0.0);
        for k in 0..TRAINING_LEN {
            let ph = w * k as f64 + FRAC_PI_4 * f64::from(cum[k]) + mean;
            let r = f64::from((z[k] * Complex32::from_polar(1.0, -ph as f32)).arg());
            let x = k as f64 - 7.5;
            sxy += x * r;
            sxx += x * x;
        }
        let dw = sxy / sxx;
        w += dw;
        let phi0 = mean - 7.5 * dw;
        let symbols = match training {
            Training::S1 | Training::S1Star => 16,
            Training::S2 => VD_SYMBOLS,
            Training::S2Star => self.s2star_length(f, index, train_power),
        };
        // The decision-directed loop from the last training symbol.
        let mut theta = phi0 + w * 15.0;
        let mut prev = cum[15].rem_euclid(8);
        let mut err2 = 0.0;
        self.decisions.clear();
        for k in 0..symbols {
            let r = self.at(f.t + ((TRAINING_LEN + k) * SPS) as f64);
            theta += w;
            let ph = f64::from(r.arg()) - theta;
            let a = (ph / FRAC_PI_4).round();
            let e =
                (ph - a * FRAC_PI_4 + std::f64::consts::PI).rem_euclid(TAU) - std::f64::consts::PI;
            theta += LOOP_PHASE * e;
            w += LOOP_FREQ * e;
            err2 += (e / FRAC_PI_4).powi(2);
            let a = (a as i32).rem_euclid(8);
            self.decisions.push((a - prev).rem_euclid(8) as u8);
            prev = a;
        }
        let evm = (err2 / symbols as f64).sqrt();
        if evm > self.evm_max {
            return None;
        }
        self.bits.clear();
        bits_of_steps(&self.decisions, &mut self.bits);
        let vd = training == Training::S2;
        let kind = match training {
            Training::S1 => BurstKind::MDownlink,
            Training::S1Star => BurstKind::NetEntry,
            Training::S2 => BurstKind::VdData,
            Training::S2Star => BurstKind::of_s2star(symbols).unwrap_or(BurstKind::PollResponse),
        };
        let mut payload = Payload::default();
        self.payload
            .decode(&mut self.bits, kind.words(), vd, self.order, &mut payload);
        let kind = if vd && !payload.rs_ok {
            BurstKind::VdVoice
        } else {
            kind
        };
        let good =
            payload.golay_ok && evm < EVM_GOOD && (kind != BurstKind::VdData || payload.rs_ok);
        let floor = self.floor.unwrap_or(f64::from(DEFAULT_MIN_FLOOR));
        let snr =
            ((train_power / floor - 1.0).max(1e-3) * NOISE_BW_HZ / SYMBOL_RATE).log10() * 10.0;
        Some(Decoded {
            end_of_training: f.t + (TRAINING_LEN * SPS) as f64,
            kind,
            training,
            symbols,
            payload,
            evm,
            level_dbfs: 10.0 * train_power.max(1e-20).log10(),
            snr_db: snr,
            cfo_hz,
            good,
        })
    }
}
