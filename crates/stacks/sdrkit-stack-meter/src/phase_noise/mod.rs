//! Phase noise meter (`phase_noise`): the averaged spectrum of a carrier near the carrier, read as single-sideband phase noise
//! L(f) in dBc/Hz at 10 Hz to 100 kHz offsets, the spurs and the integrated phase jitter (L(f) and the jitter integral as
//! IEEE Std 1139 defines them). It measures and decodes nothing. The reading includes amplitude noise, the radio's additive
//! noise and the radio's own oscillator. The blocks, thresholds and rules are confirmed by no public document.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{
    PhaseNoiseEventRow, PhaseNoiseFields, PhaseNoiseKind, PhaseNoiseSpurRow, PHASE_NOISE_EVENTS,
    PHASE_NOISE_SPURS,
};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::floor::quantile_to_mean;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::f64::consts::TAU;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "phase_noise";
/// The least input rate, Hz: the channel filter needs twice its pass edge.
pub const MIN_RATE_HZ: f64 = 200e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 250e3;
/// The pass edge of the channel filter, Hz.
pub const PASS_HZ: f64 = 100e3;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.3;
/// The samples of a block.
pub const BLOCK: usize = 1 << 18;
/// The carrier is followed within this far of its last place, Hz.
pub const FOLLOW_HZ: f64 = 300.0;
/// A carrier is this many times the median bin.
pub const CARRIER_RATIO: f64 = 1e5;
/// The offsets of L(f), Hz.
pub const OFFSETS_HZ: [f64; 5] = [10.0, 100.0, 1e3, 1e4, 1e5];
/// The least blocks for a result.
pub const MIN_BLOCKS: usize = 4;
/// The most spurs listed.
pub const MAX_SPURS: usize = 6;
/// The band of the CNR, Hz.
pub const CNR_BAND_HZ: f64 = 200e3;
/// A reading row at most this often, s.
pub const ROW_EVERY_S: f64 = 8.0;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures L(f) at 10 Hz to 100 kHz from the carrier, the spurs and the integrated phase jitter of \
the averaged spectrum; decodes nothing. The reading is the sum of the transmitter's and the radio's own oscillator, and \
includes amplitude noise and the radio's additive noise (a carrier 60 dB over the noise in 100 kHz shows no better than \
about -110 dBc/Hz far out). The jitter as time needs the tuned frequency. The rules are confirmed by no public document.";

/// The blocks of the plain average, then the weight of the exponential one.
pub const AVERAGE_BLOCKS: Setting<field::I64> =
    Setting::new("average_blocks", "Blocks averaged", "", 16).range(4.0, 64.0, 1.0);
/// The spur over its neighbourhood.
pub const SPUR_RATIO_DB: Setting<field::F64> =
    Setting::new("spur_ratio_db", "Spur over its neighbourhood", "dB", 9.0).range(3.0, 20.0, 0.0);
/// The low edge of the jitter integration.
pub const JITTER_LO_HZ: Setting<field::F64> =
    Setting::new("jitter_lo_hz", "Jitter band, low edge", "Hz", 100.0).range(10.0, 1e6, 0.0);
/// The high edge of the jitter integration.
pub const JITTER_HI_HZ: Setting<field::F64> =
    Setting::new("jitter_hi_hz", "Jitter band, high edge", "Hz", 100e3).range(10.0, 1e6, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The average.
        AverageBlocks = AVERAGE_BLOCKS,
        /// The spur test.
        SpurRatioDb = SPUR_RATIO_DB,
        /// The jitter band.
        JitterLoHz = JITTER_LO_HZ,
        /// The jitter band.
        JitterHiHz = JITTER_HI_HZ,
    }
}

sdrkit_core::settings! {
    /// The settings of the `phase_noise` stack.
    pub enum PhaseNoiseSetting {
        /// The average.
        AverageBlocks = AVERAGE_BLOCKS,
        /// The spur test.
        SpurRatioDb = SPUR_RATIO_DB,
        /// The jitter band.
        JitterLoHz = JITTER_LO_HZ,
        /// The jitter band.
        JitterHiHz = JITTER_HI_HZ,
        /// The linewidth of the test signal.
        GenLinewidthHz = gen::GEN_LINEWIDTH_HZ,
        /// Its white floor.
        GenFloorDbc = gen::GEN_FLOOR_DBC,
        /// Its spur.
        GenSpurHz = gen::GEN_SPUR_HZ,
        /// The spur level.
        GenSpurDbc = gen::GEN_SPUR_DBC,
    }
}

/// The readings of the average.
#[derive(Clone, Debug, Default, PartialEq)]
struct Reading {
    l: [Option<f64>; 5],
    jitter: f64,
    cnr: f64,
    spurs: Vec<(f64, f64)>,
}

/// The measurement of `phase_noise`.
#[derive(Default)]
pub struct PhaseNoiseMeter {
    front: Option<Front>,
    fft: Option<Fft>,
    window: Vec<f32>,
    w2: f64,
    enbw: f64,
    rate: f64,
    buf: Vec<Complex32>,
    work: Vec<Complex32>,
    p: Vec<f64>,
    avg: Vec<f64>,
    blocks: usize,
    average: usize,
    spur_ratio: f64,
    jlo: f64,
    jhi: f64,
    fc: Option<f64>,
    phase: f64,
    t: f64,
    last_row: Option<f64>,
    reading: Option<Reading>,
    log: Log<PhaseNoiseEventRow>,
}

impl PhaseNoiseMeter {
    fn bin_hz(&self) -> f64 {
        self.rate / BLOCK as f64
    }

    fn lose(&mut self) {
        self.avg.iter_mut().for_each(|a| *a = 0.0);
        self.blocks = 0;
        self.fc = None;
        self.reading = None;
    }

    fn evaluate(&self) -> Reading {
        let n = BLOCK;
        let b = self.bin_hz();
        let norm = n as f64 * self.w2;
        let pk = |k: usize| self.avg[k % n] / norm;
        let half = (0.45 * self.rate / b).floor() as usize;
        let c: f64 = pk(0) + (1..=half).map(|j| pk(j) + pk(n - j)).sum::<f64>();
        let mut l = [None; 5];
        for (i, &f) in OFFSETS_HZ.iter().enumerate() {
            if f > 0.8 * self.rate / 2.0 || f < 3.0 * b {
                continue;
            }
            let k0 = (0.85 * f / b).round() as usize;
            let k1 = (k0 + 1).max((1.15 * f / b).round() as usize);
            let s: f64 = (k0..=k1).map(|k| pk(k) + pk(n - k)).sum();
            let mean = s / (2 * (k1 - k0 + 1)) as f64;
            l[i] = Some(10.0 * (mean / b / c).max(1e-30).log10());
        }
        let j1 = (self.jlo / b).round() as usize;
        let j2 = (n / 2 - 1).min((self.jhi.min(0.4 * self.rate) / b).round() as usize);
        let integral: f64 = (j1..=j2).map(|k| 0.5 * (pk(k) + pk(n - k)) / c).sum();
        let mut sorted = self.avg.clone();
        let m = n / 2;
        let (_, med, _) = sorted.select_nth_unstable_by(m, f64::total_cmp);
        let to_mean = quantile_to_mean(self.blocks.clamp(1, 1000) as u32, 0.5)
            .unwrap_or(std::f64::consts::LN_2);
        let per_hz = *med / to_mean / norm / b;
        let mut spurs = Vec::new();
        let mut k = (j1 / 2).max((30.0 / b).floor() as usize).max(1);
        let mut nb = Vec::with_capacity(32);
        while k + 2 <= n / 2 {
            let s = self.avg[k];
            if s >= self.avg[k - 1] && s >= self.avg[k + 1] {
                nb.clear();
                nb.extend(
                    (-60i64..=60)
                        .step_by(4)
                        .filter(|d| d.abs() > 8)
                        .map(|d| k as i64 + d)
                        .filter(|&i| i >= 0 && i < n as i64 / 2)
                        .map(|i| self.avg[i as usize]),
                );
                let mid = nb.len() / 2;
                let (_, med, _) = nb.select_nth_unstable_by(mid, f64::total_cmp);
                if s > self.spur_ratio * *med {
                    let p: f64 = (k - 4..=k + 4).map(pk).sum();
                    spurs.push((k as f64 * b, 10.0 * (p / c).log10()));
                    k += 7;
                    continue;
                }
            }
            k += 1;
        }
        spurs.sort_by(|a, b| b.1.total_cmp(&a.1));
        spurs.truncate(MAX_SPURS);
        Reading {
            l,
            jitter: (2.0 * integral).sqrt(),
            cnr: 10.0 * (c / (per_hz * CNR_BAND_HZ)).log10(),
            spurs,
        }
    }

    fn block(&mut self, c: &mut Ctx) {
        let n = BLOCK;
        let Some(fft) = self.fft.as_mut() else { return };
        for ((b, &s), &w) in self.buf.iter_mut().zip(&self.work).zip(&self.window) {
            *b = s * w;
        }
        fft.process(&mut self.buf);
        for (p, b) in self.p.iter_mut().zip(&self.buf) {
            *p = f64::from(b.norm_sqr());
        }
        self.t += n as f64 / self.rate;
        let b = self.bin_hz();
        let peak = match self.fc {
            None => (0..n)
                .max_by(|&i, &j| self.p[i].total_cmp(&self.p[j]))
                .unwrap_or(0),
            Some(_) => {
                let h = (FOLLOW_HZ / b).floor() as i64;
                (-h..=h)
                    .map(|j| j.rem_euclid(n as i64) as usize)
                    .max_by(|&i, &j| self.p[i].total_cmp(&self.p[j]))
                    .unwrap_or(0)
            }
        };
        let mut sorted = self.p.clone();
        let (_, med, _) = sorted.select_nth_unstable_by(n / 2, f64::total_cmp);
        if self.p[peak] < CARRIER_RATIO * *med {
            if self.fc.is_some() {
                self.log.push(PhaseNoiseEventRow {
                    time_s: Seconds(self.t),
                    kind: PhaseNoiseKind::Lost,
                    l_10hz: None,
                    l_100hz: None,
                    l_1khz: None,
                    l_10khz: None,
                    l_100khz: None,
                    jitter_mrad: None,
                    spurs: 0,
                });
            }
            self.lose();
            c.tally.energy(self.t);
            return;
        }
        let at = |k: i64| self.p[k.rem_euclid(n as i64) as usize];
        let j = if peak > n / 2 {
            peak as i64 - n as i64
        } else {
            peak as i64
        };
        let d = log_parabolic(at(j - 1), at(j), at(j + 1)).offset;
        let o = (j as f64 + d) * b;
        let Some(fc) = self.fc else {
            self.fc = Some(o);
            self.work.clear();
            return;
        };
        self.fc = Some(fc + 0.5 * o);
        self.blocks += 1;
        let w = if self.blocks <= self.average {
            1.0 / self.blocks as f64
        } else {
            1.0 / self.average as f64
        };
        for (k, a) in self.avg.iter_mut().enumerate() {
            *a += w * (self.p[(k + peak) % n] - *a);
        }
        if self.blocks >= MIN_BLOCKS {
            let r = self.evaluate();
            c.tally.good(self.t);
            if self.last_row.is_none_or(|t| self.t - t >= ROW_EVERY_S) {
                self.last_row = Some(self.t);
                self.log.push(PhaseNoiseEventRow {
                    time_s: Seconds(self.t),
                    kind: PhaseNoiseKind::Reading,
                    l_10hz: r.l[0],
                    l_100hz: r.l[1],
                    l_1khz: r.l[2],
                    l_10khz: r.l[3],
                    l_100khz: r.l[4],
                    jitter_mrad: Some(1000.0 * r.jitter),
                    spurs: r.spurs.len() as i64,
                });
            }
            self.reading = Some(r);
        }
    }
}

impl TxTest for PhaseNoiseMeter {
    const ID: &'static str = ID;
    const NAME: &'static str = "phase_noise.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let front = Front::new(rate_hz, WORK_HZ, PASS_HZ, SKIP_S);
        self.rate = front.rate();
        self.front = Some(front);
        self.fft = Some(Fft::forward(BLOCK));
        self.window = Window::BlackmanHarris4.coefficients(BLOCK);
        self.w2 = self.window.iter().map(|&w| f64::from(w).powi(2)).sum();
        let s1: f64 = self.window.iter().map(|&w| f64::from(w)).sum();
        self.enbw = BLOCK as f64 * self.w2 / (s1 * s1);
        self.buf = vec![Complex32::default(); BLOCK];
        self.p = vec![0.0; BLOCK];
        self.avg = vec![0.0; BLOCK];
        self.work.clear();
        self.average = v.get(&AVERAGE_BLOCKS).max(1) as usize;
        self.spur_ratio = 10f64.powf(v.get(&SPUR_RATIO_DB) / 10.0);
        self.jlo = v.get(&JITTER_LO_HZ);
        self.jhi = v.get(&JITTER_HI_HZ).max(self.jlo);
        self.t = self.t.max(SKIP_S);
        self.phase = 0.0;
        self.lose();
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let Some(mut front) = self.front.take() else {
            return;
        };
        for &z in front.process(x) {
            let s = match self.fc {
                Some(fc) => {
                    self.phase = (self.phase - TAU * fc / self.rate).rem_euclid(TAU);
                    z * Complex32::from_polar(1.0, self.phase as f32)
                }
                None => z,
            };
            self.work.push(s);
            if self.work.len() == BLOCK {
                self.block(c);
                self.work.clear();
            }
        }
        self.front = Some(front);
    }
    fn clear(&mut self) {
        self.lose();
        self.last_row = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, c: &Ctx) -> String {
        self.log.publish(&PHASE_NOISE_EVENTS, p);
        let r = self.reading.as_ref();
        let spurs: Vec<PhaseNoiseSpurRow> = r
            .map(|r| {
                r.spurs
                    .iter()
                    .map(|&(f, l)| PhaseNoiseSpurRow {
                        offset_hz: Hertz(f),
                        level_dbc: l,
                    })
                    .collect()
            })
            .unwrap_or_default();
        crate::instrument::publish_list(&PHASE_NOISE_SPURS, &spurs, p);
        let l = |i: usize| r.and_then(|r| r.l[i]);
        let ok = self.rate > 0.0;
        p.put(&PhaseNoiseFields {
            l_10hz: l(0),
            l_100hz: l(1),
            l_1khz: l(2),
            l_10khz: l(3),
            l_100khz: l(4),
            jitter_mrad: r.map(|r| 1000.0 * r.jitter),
            jitter_ps: r.and_then(|r| c.tuned_hz.map(|f| r.jitter / (TAU * f) * 1e12)),
            carrier_offset_hz: r.and(self.fc).map(Hertz),
            cnr_db: r.map(|r| r.cnr),
            blocks: self.blocks.min(self.average) as i64,
            block_s: ok.then(|| Seconds(BLOCK as f64 / self.rate)),
            bin_hz: ok.then(|| Hertz(self.bin_hz())),
            enbw_bins: ok.then_some(self.enbw),
        });
        match r {
            Some(r) => format!(
                "L(1 kHz) {}, jitter {:.1} mrad, {} spurs",
                r.l[2].map_or("none".to_string(), |v| format!("{v:.1} dBc/Hz")),
                1000.0 * r.jitter,
                r.spurs.len()
            ),
            None if self.fc.is_some() => format!("averaging, {} blocks", self.blocks),
            None => "searching for a carrier".to_string(),
        }
    }
}

const KEYS: [&str; 4] = [
    "average_blocks",
    "spur_ratio_db",
    "jitter_lo_hz",
    "jitter_hi_hz",
];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("a carrier of 20 Hz linewidth", o, 0.0);
    c.seconds = o.seconds.max(10.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let l = crate::instrument::number(r, PhaseNoiseFields::L_1KHZ.key);
        vec![CheckLine::new(
            PhaseNoiseFields::L_1KHZ.key,
            (l + 55.0).abs() < 1.5,
            format!("{l:.1} dBc/Hz, -55.0 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: PhaseNoiseMeter,
    settings: PhaseNoiseSetting,
    keys: KEYS,
    name: "Phase noise meter",
    blurb: "L(f) of a carrier at 10 Hz to 100 kHz offsets, its spurs and the integrated phase jitter.",
    tuning: (0.1e6, 6e9, 250e3, 100e3, true),
    rate_text: "The phase noise meter needs at least 200 ksps",
    generator: |cfg| gen::PhaseNoiseGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
