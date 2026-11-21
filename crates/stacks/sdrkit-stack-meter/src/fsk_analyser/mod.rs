//! FSK parameter analyser (`fsk_analyser`): of an unknown FSK or GFSK signal around the tuned frequency, the symbol rate,
//! two or four levels, the shift, the level frequencies, the modulation index, the centre, the spacing of four levels and the
//! eye SNR, from the discriminator audio of the channel cut down to the band that the signal occupies in each block. It tells
//! which decoder to try and decodes nothing. The rules are confirmed by no public document; the modulation index and the GFSK
//! BT are general theory.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{band_gain, index_percentile, tx_stack, Ctx, TxTest};
use crate::typed::{
    FskAnalyserEventRow, FskAnalyserFields, FskAnalyserLevelRow, FskLevels, FSK_ANALYSER_EVENTS,
    FSK_ANALYSER_LEVEL_FREQS,
};
use sdrkit_algo_dsp::bandwidth::{bridged_region, power_share_edges};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::kmeans::{kmeans_1d, Start};
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "fsk_analyser";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 48e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 100e3;
/// The largest pass edge of the channel filter, Hz.
pub const PASS_HZ: f64 = 45e3;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.1;
/// Candidates of the symbol rate.
pub const CANDIDATES: usize = 3;
/// Candidates are at least this many bins apart.
pub const CANDIDATE_GAP: usize = 20;
/// A candidate is this many times the median of its band.
pub const CANDIDATE_RATIO: f64 = 8.0;
/// The fewest samples per symbol.
pub const MIN_SPS: f64 = 6.0;
/// The symbols of the timing search, at most.
pub const TIMING_SYMBOLS: usize = 600;
/// The fewest symbols of a measurement.
pub const MIN_SYMBOLS: usize = 200;
/// A row at least this often while a signal is measured, s.
pub const ROW_EVERY_S: f64 = 30.0;
/// The frame of the Welch spectrum that finds the occupied band.
pub const WELCH_N: usize = 1024;
/// A bin of the occupied band is this many times the floor.
pub const HOT_RATIO: f64 = 4.0;
/// Gaps of this many cold bins do not end the occupied band.
pub const GAP_BINS: usize = 2;
/// An occupied band wider than this share of the pass band leaves the whole channel.
pub const FULL_SHARE: f64 = 0.8;
/// The stretch at each end of a block left out of the audio, where the channel filter wraps around, s.
pub const EDGE_S: f64 = 0.01;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures an unknown FSK or GFSK signal: symbol rate, 2 or 4 levels, shift, level frequencies, \
modulation index, centre, level spacing and eye SNR; decodes nothing. The discriminator works on the band that the signal \
occupies, measured on each block. The shift of GFSK with BT 0.5 reads about 7 % under the peak deviation. The \
rules are confirmed by no public document.";

/// The block length.
pub const BLOCK_S: Setting<field::F64> =
    Setting::new("block_s", "Block length", "s", 2.0).range(0.5, 10.0, 0.0);
/// The lowest symbol rate.
pub const MIN_BAUD: Setting<field::F64> =
    Setting::new("min_baud", "Lowest symbol rate", "Bd", 100.0).range(20.0, 5000.0, 0.0);
/// The lowest eye SNR shown.
pub const MIN_EYE_DB: Setting<field::F64> =
    Setting::new("min_eye_db", "Lowest eye SNR shown", "dB", 6.0).range(0.0, 20.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The block.
        BlockS = BLOCK_S,
        /// The lowest rate.
        MinBaud = MIN_BAUD,
        /// The lowest eye.
        MinEyeDb = MIN_EYE_DB,
    }
}

sdrkit_core::settings! {
    /// The settings of the `fsk_analyser` stack.
    pub enum FskAnalyserSetting {
        /// The block.
        BlockS = BLOCK_S,
        /// The lowest rate.
        MinBaud = MIN_BAUD,
        /// The lowest eye.
        MinEyeDb = MIN_EYE_DB,
        /// The levels of the test signal.
        GenLevels = gen::GEN_LEVELS,
        /// Its symbol rate.
        GenBaud = gen::GEN_BAUD,
        /// Its deviation.
        GenDeviationHz = gen::GEN_DEVIATION_HZ,
        /// Its BT.
        GenBt = gen::GEN_BT,
    }
}

/// One measurement.
#[derive(Clone, Debug, Default, PartialEq)]
struct Reading {
    four: bool,
    rate: f64,
    shift: f64,
    outer: f64,
    h: f64,
    centre: f64,
    ratio: Option<f64>,
    eye: f64,
    freqs: Vec<f64>,
}

/// The measurement of `fsk_analyser`.
#[derive(Default)]
pub struct FskAnalyser {
    front: Option<Front>,
    disc: Option<Discriminator>,
    rate: f64,
    block: usize,
    min_baud: f64,
    min_eye: f64,
    audio: Vec<f32>,
    pass: f64,
    x: Vec<Complex32>,
    t: f64,
    reading: Option<Reading>,
    last_row: Option<(f64, bool, f64)>,
    log: Log<FskAnalyserEventRow>,
}

impl FskAnalyser {
    /// The symbol-rate candidates of `y` (mean removed), Bd.
    fn candidates(&self, y: &[f64]) -> Vec<f64> {
        let n = y.len();
        let nf = n.next_power_of_two();
        let b = self.rate / nf as f64;
        let mean_sq = y.iter().map(|v| v * v).sum::<f64>() / n as f64;
        let w = Window::Hann.coefficients(n);
        let mut buf = vec![Complex32::default(); nf];
        for ((o, &v), &wi) in buf.iter_mut().zip(y).zip(&w) {
            *o = Complex32::new(((v * v - mean_sq) * f64::from(wi)) as f32, 0.0);
        }
        Fft::forward(nf).process(&mut buf);
        let e: Vec<f64> = buf[..nf / 2]
            .iter()
            .map(|z| f64::from(z.norm_sqr()))
            .collect();
        let lo = 3usize.max((self.min_baud / b).floor() as usize);
        let hi = ((self.rate / 5.0 / b).floor() as usize).min(e.len() - 2);
        if hi <= lo + 2 {
            return Vec::new();
        }
        let mut band = e[lo..=hi].to_vec();
        let m = band.len() / 2;
        let (_, med, _) = band.select_nth_unstable_by(m, f64::total_cmp);
        let med = *med;
        let mut out: Vec<usize> = Vec::new();
        for _ in 0..CANDIDATES {
            let Some(k) = (lo..=hi)
                .filter(|&k| out.iter().all(|&c| c.abs_diff(k) >= CANDIDATE_GAP))
                .max_by(|&i, &j| e[i].total_cmp(&e[j]))
            else {
                break;
            };
            if e[k] < CANDIDATE_RATIO * med {
                break;
            }
            out.push(k);
        }
        out.iter()
            .map(|&k| (k as f64 + log_parabolic(e[k - 1], e[k], e[k + 1]).offset) * b)
            .collect()
    }

    /// The measurement of `y` (mean removed) at the symbol rate `r_bd`.
    fn measure(&self, y: &[f64], mean: f64, r_bd: f64, bin_hz: f64) -> Option<Reading> {
        let t = self.rate / r_bd;
        if t < MIN_SPS {
            return None;
        }
        let w = ((t / 4.0).floor() as usize).max(1);
        let mut s = Vec::with_capacity(y.len());
        let mut acc = 0.0;
        for i in 0..y.len() {
            acc += y[i];
            if i >= w {
                acc -= y[i - w];
            }
            s.push(acc / (i + 1).min(w) as f64);
        }
        let at = |p: f64| -> Option<f64> {
            let i = p.floor() as usize;
            (i + 1 < s.len()).then(|| s[i] + (p - i as f64) * (s[i + 1] - s[i]))
        };
        let best = (0..16)
            .map(|ph| {
                let start = t + t * ph as f64 / 16.0;
                let v: Vec<f64> = (0..TIMING_SYMBOLS)
                    .map_while(|k| at(start + k as f64 * t))
                    .collect();
                (
                    start,
                    v.iter().map(|x| x * x).sum::<f64>() / v.len().max(1) as f64,
                )
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))?
            .0;
        let sym: Vec<f64> = (0..).map_while(|k| at(best + k as f64 * t)).collect();
        if sym.len() < MIN_SYMBOLS {
            return None;
        }
        let n = sym.len() as f64;
        let k2 = kmeans_1d(&sym, 2, Start::Spread, 30)?;
        let k4 = kmeans_1d(&sym, 4, Start::Spread, 30)?;
        let four = k4.residual < 0.25 * k2.residual && k4.centres.windows(2).all(|c| c[1] > c[0]);
        let r = if four {
            let c = &k4.centres;
            let o = [c[1] - c[0], c[2] - c[1], c[3] - c[2]];
            let shift = (o[0] + o[1] + o[2]) / 3.0;
            let outer = c[3] - c[0];
            Reading {
                four,
                rate: r_bd,
                shift,
                outer,
                h: outer / r_bd,
                centre: c.iter().sum::<f64>() / 4.0 + mean,
                ratio: Some(0.5 * (o[0] + o[2]) / o[1]),
                eye: 10.0 * (shift * shift / 12.0 / (k4.residual / n)).log10(),
                freqs: c.iter().map(|v| v + mean).collect(),
            }
        } else {
            let c = &k2.centres;
            let shift = c[1] - c[0];
            Reading {
                four,
                rate: r_bd,
                shift,
                outer: shift,
                h: shift / r_bd,
                centre: (c[0] + c[1]) / 2.0 + mean,
                ratio: None,
                eye: 10.0 * (shift * shift / 4.0 / (k2.residual / n)).log10(),
                freqs: c.iter().map(|v| v + mean).collect(),
            }
        };
        (r.eye > self.min_eye && r.shift > 4.0 * bin_hz).then_some(r)
    }

    /// The occupied band of the block `x`, Hz from the tuned frequency: the band with 99 % of the power over the floor of the
    /// strongest run of bins at least `HOT_RATIO` over the floor (gaps of `GAP_BINS` bridged), on a Welch spectrum of
    /// `WELCH_N`-point Hann frames. The floor is the 20th percentile of the bins inside the pass band. `None` when no run stands out
    /// or the run fills most of the pass band.
    fn occupied(&self, x: &[Complex32]) -> Option<(f64, f64)> {
        let mut welch = Welch::new(WELCH_N, Window::Hann, 0.5, Scaling::Power).ok()?;
        let mut pf = vec![0f32; WELCH_N];
        welch.compute(x, &mut pf).ok()?;
        let p: Vec<f64> = pf.iter().map(|&v| f64::from(v)).collect();
        let bin = self.rate / WELCH_N as f64;
        let hz = |i: usize| (i as f64 - (WELCH_N / 2) as f64) * bin;
        let inside: Vec<bool> = (0..WELCH_N).map(|i| hz(i).abs() <= self.pass).collect();
        let mut band: Vec<f64> = (0..WELCH_N).filter(|&i| inside[i]).map(|i| p[i]).collect();
        let span = band.len();
        let floor = index_percentile(&mut band, 0.2)?;
        let hot: Vec<bool> = (0..WELCH_N)
            .map(|i| inside[i] && p[i] > HOT_RATIO * floor)
            .collect();
        let r = bridged_region(&hot, &p, GAP_BINS)?;
        if (r.hi - r.lo + 1) as f64 > FULL_SHARE * span as f64 {
            return None;
        }
        let over: Vec<f64> = p.iter().map(|&v| (v - floor).max(0.0)).collect();
        let (a, z) = power_share_edges(&over, r.lo, r.hi, 0.99)?;
        Some((hz(a) - 0.5 * bin, hz(z) + 0.5 * bin))
    }

    /// The discriminator audio of the block `x` in Hz, without `EDGE_S` at each end: the channel cut down to the occupied band
    /// and a margin of half its width on each side (raised-cosine skirts as wide), or the whole channel.
    fn audio_of(&mut self, x: &[Complex32]) -> Vec<f64> {
        let mut z = x.to_vec();
        if let Some((lo, hi)) = self.occupied(x) {
            let margin = (0.5 * (hi - lo)).max(2.0 * self.rate / WELCH_N as f64);
            let (centre, half) = (0.5 * (lo + hi), 0.5 * (hi - lo) + margin);
            let n = z.len();
            Fft::forward(n).process(&mut z);
            for (k, b) in z.iter_mut().enumerate() {
                let j = if k <= n / 2 {
                    k as f64
                } else {
                    k as f64 - n as f64
                };
                *b *= (band_gain(j * self.rate / n as f64 - centre, 0.0, half, margin) / n as f64)
                    as f32;
            }
            Fft::inverse(n).process(&mut z);
        }
        let Some(disc) = self.disc.as_mut() else {
            return Vec::new();
        };
        disc.reset();
        self.audio.clear();
        disc.process(&z, &mut self.audio);
        let edge = ((EDGE_S * self.rate) as usize).min(self.audio.len() / 4);
        self.audio[edge..self.audio.len() - edge]
            .iter()
            .map(|&a| f64::from(a))
            .collect()
    }

    fn analyse(&mut self, c: &mut Ctx) {
        let block: Vec<Complex32> = self.x[..self.block].to_vec();
        self.t += self.block as f64 / self.rate;
        let raw = self.audio_of(&block);
        let n = raw.len();
        if n == 0 {
            return;
        }
        let mean = raw.iter().sum::<f64>() / n as f64;
        let y: Vec<f64> = raw.iter().map(|v| v - mean).collect();
        let var = y.iter().map(|v| v * v).sum::<f64>() / n as f64;
        if var < 1.0 {
            self.reading = None;
            c.tally.bad(self.t);
            return;
        }
        let bin = self.rate / n.next_power_of_two() as f64;
        self.reading = self
            .candidates(&y)
            .into_iter()
            .filter_map(|r| self.measure(&y, mean, r, bin))
            .max_by(|a, b| a.eye.total_cmp(&b.eye));
        let Some(r) = self.reading.as_ref() else {
            c.tally.energy(self.t);
            return;
        };
        c.tally.good(self.t);
        if self.last_row.is_none_or(|(t, four, rate)| {
            four != r.four || (r.rate - rate).abs() > 0.02 * rate || self.t - t >= ROW_EVERY_S
        }) {
            self.last_row = Some((self.t, r.four, r.rate));
            self.log.push(FskAnalyserEventRow {
                time_s: Seconds(self.t),
                levels: if r.four {
                    FskLevels::Four
                } else {
                    FskLevels::Two
                },
                symbol_rate_bd: r.rate,
                shift_hz: Hertz(r.shift),
                mod_index: r.h,
                centre_hz: Hertz(r.centre),
                eye_snr_db: r.eye,
            });
        }
    }
}

impl TxTest for FskAnalyser {
    const ID: &'static str = ID;
    const NAME: &'static str = "fsk_analyser.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let d = (rate_hz / WORK_HZ).floor().max(1.0);
        self.pass = PASS_HZ.min(0.45 * rate_hz / d);
        let front = Front::new(rate_hz, WORK_HZ, self.pass, SKIP_S);
        self.rate = front.rate();
        self.disc = Discriminator::new(self.rate, 1.0).ok();
        self.front = Some(front);
        self.block = (v.get(&BLOCK_S) * self.rate).round() as usize;
        self.min_baud = v.get(&MIN_BAUD);
        self.min_eye = v.get(&MIN_EYE_DB);
        self.x.clear();
        self.t = self.t.max(SKIP_S);
        self.reading = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let Some(front) = self.front.as_mut() else {
            return;
        };
        self.x.extend_from_slice(front.process(x));
        while self.block > 0 && self.x.len() >= self.block {
            self.analyse(c);
            self.x.drain(..self.block);
        }
    }
    fn clear(&mut self) {
        self.reading = None;
        self.last_row = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&FSK_ANALYSER_EVENTS, p);
        let r = self.reading.as_ref();
        let rows: Vec<FskAnalyserLevelRow> = r
            .map(|r| {
                r.freqs
                    .iter()
                    .enumerate()
                    .map(|(i, &f)| FskAnalyserLevelRow {
                        index: i as i64,
                        frequency_hz: Hertz(f),
                    })
                    .collect()
            })
            .unwrap_or_default();
        crate::instrument::publish_list(&FSK_ANALYSER_LEVEL_FREQS, &rows, p);
        p.put(&FskAnalyserFields {
            levels: r.map(|r| {
                if r.four {
                    FskLevels::Four
                } else {
                    FskLevels::Two
                }
            }),
            msk_like: r.map(|r| !r.four && (r.h - 0.5).abs() < 0.06),
            symbol_rate_bd: r.map(|r| r.rate),
            shift_hz: r.map(|r| Hertz(r.shift)),
            outer_hz: r.map(|r| Hertz(r.outer)),
            mod_index: r.map(|r| r.h),
            centre_hz: r.map(|r| Hertz(r.centre)),
            spacing_ratio: r.and_then(|r| r.ratio),
            spacing_equal: r.and_then(|r| r.ratio.map(|q| (q - 1.0).abs() <= 0.35)),
            eye_snr_db: r.map(|r| r.eye),
        });
        match r {
            Some(r) => format!(
                "{} levels at {:.0} Bd, shift {:.0} Hz, h {:.2}, eye {:.1} dB",
                if r.four { 4 } else { 2 },
                r.rate,
                r.shift,
                r.h,
                r.eye
            ),
            None => "no FSK signal".to_string(),
        }
    }
}

const KEYS: [&str; 3] = ["block_s", "min_baud", "min_eye_db"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("GFSK of 9600 Bd and 2400 Hz deviation", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let b = crate::instrument::number(r, FskAnalyserFields::SYMBOL_RATE_BD.key);
        vec![CheckLine::new(
            FskAnalyserFields::SYMBOL_RATE_BD.key,
            (b - 9600.0).abs() < 40.0,
            format!("{b:.1} Bd, 9600 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: FskAnalyser,
    settings: FskAnalyserSetting,
    keys: KEYS,
    name: "FSK parameter analyser",
    blurb: "Of an unknown FSK or GFSK signal: symbol rate, 2 or 4 levels, shift, levels, modulation index, centre and eye SNR.",
    tuning: (0.1e6, 6e9, 250e3, 100e3, false),
    rate_text: "The FSK analyser needs at least 48 ksps",
    generator: |cfg| gen::FskGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
