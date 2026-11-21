//! Spectral kurtosis RFI finder (`kurtosis`), with the estimator of Nita et al. (2007) and Nita and Gary (2010): which bins
//! of a wide capture hold something that is not Gaussian noise, steady (SK under 1) or pulsed (SK over 1), grouped in
//! regions because neighbouring bins of a Hann spectrum are correlated; the clean share of the band. It measures and does
//! not name a source. The flag threshold, the region rule and the floor (the median of the mean powers over 0.693, which
//! reads about 1.6 dB over the true mean noise) are confirmed by no public document.

pub mod gen;

use crate::instrument::{publish_list, Log};
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{
    KurtosisChange, KurtosisEventRow, KurtosisFields, KurtosisKind, KurtosisRegionRow,
    KURTOSIS_EVENTS, KURTOSIS_REGIONS,
};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::speckurt::{sk_flag, SkEstimator, SkKind};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "kurtosis";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The used bins start at this signed bin.
pub const FIRST_BIN: i64 = 3;
/// The used bins end at this share of the frame.
pub const EDGE_SHARE: f64 = 0.45;
/// The median of the mean powers over this ratio is the noise per bin.
pub const MEDIAN_RATIO: f64 = 0.693;
/// A region bridges at most this many other bins.
pub const GAP_BINS: i64 = 2;
/// A region matches one of the evaluation before within this many bins of its centre.
pub const SAME_BINS: f64 = 4.0;
/// The regions listed.
pub const MAX_REGIONS: usize = 8;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures which bins hold power that is not Gaussian noise by spectral kurtosis (Nita and Gary \
2010), steady or pulsed, in regions; does not name a source. The threshold, the region rule and the floor (about 1.6 dB over \
the mean noise) are confirmed by no public document.";

/// The frame length.
pub const FFT: Setting<field::I64> =
    Setting::new("fft", "Frame length", "points", 1024).range(256.0, 8192.0, 1.0);
/// The frames of an estimate.
pub const FRAMES: Setting<field::I64> =
    Setting::new("frames", "Frames per estimate", "", 128).range(16.0, 1024.0, 1.0);
/// The estimates of an evaluation.
pub const BLOCKS: Setting<field::I64> =
    Setting::new("blocks", "Estimates averaged", "", 8).range(1.0, 64.0, 1.0);
/// The flag threshold.
pub const SIGMAS: Setting<field::F64> =
    Setting::new("sigmas", "Flag threshold", "sigma", 4.5).range(3.0, 10.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The frame length.
        Fft = FFT,
        /// The frames of an estimate.
        Frames = FRAMES,
        /// The estimates averaged.
        Blocks = BLOCKS,
        /// The threshold.
        Sigmas = SIGMAS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `kurtosis` stack.
    pub enum KurtosisSetting {
        /// The frame length.
        Fft = FFT,
        /// The frames of an estimate.
        Frames = FRAMES,
        /// The estimates averaged.
        Blocks = BLOCKS,
        /// The threshold.
        Sigmas = SIGMAS,
        /// The tones of the test signal.
        GenTones = gen::GEN_TONES,
    }
}

/// A region of one evaluation, in signed bins.
#[derive(Clone, Copy, Debug)]
struct Region {
    lo: f64,
    hi: f64,
    kind: KurtosisKind,
    sk: f64,
    over_db: f64,
    count: u32,
    reported: bool,
}

impl Region {
    fn centre(&self) -> f64 {
        0.5 * (self.lo + self.hi)
    }
}

/// The regions of `kind` in the used bins `-edge..=edge` of `sk` (signed order, `n` bins).
fn regions_of(
    kind: SkKind,
    flags: &[Option<SkKind>],
    sk: &[f64],
    mean: &[f64],
    noise: f64,
    edge: i64,
) -> Vec<Region> {
    let n = flags.len() as i64;
    let used = |j: i64| j.abs() >= FIRST_BIN && j.abs() <= edge;
    let mut out = Vec::new();
    let mut open: Option<(i64, i64, Vec<usize>)> = None;
    let close = |o: (i64, i64, Vec<usize>), out: &mut Vec<Region>| {
        if o.2.len() < 2 {
            return;
        }
        let m = o.2.len() as f64;
        out.push(Region {
            lo: o.0 as f64 - 0.5,
            hi: o.1 as f64 + 0.5,
            kind: if kind == SkKind::Pulsed {
                KurtosisKind::Pulsed
            } else {
                KurtosisKind::Steady
            },
            sk: o.2.iter().map(|&i| sk[i]).sum::<f64>() / m,
            over_db: 10.0
                * (o.2.iter().map(|&i| mean[i]).sum::<f64>() / m / noise.max(1e-30))
                    .max(1e-30)
                    .log10(),
            count: 0,
            reported: false,
        });
    };
    for j in -edge..=edge {
        let i = (j + n / 2) as usize;
        let hit = used(j) && flags[i] == Some(kind);
        match (&mut open, hit) {
            (Some(o), true) => {
                o.1 = j;
                o.2.push(i);
            }
            (Some(o), false) if j - o.1 > GAP_BINS => {
                if let Some(o) = open.take() {
                    close(o, &mut out);
                }
            }
            (None, true) => open = Some((j, j, vec![i])),
            _ => {}
        }
    }
    if let Some(o) = open.take() {
        close(o, &mut out);
    }
    out
}

/// The measurement of `kurtosis`.
#[derive(Default)]
pub struct Kurtosis {
    rate: f64,
    n: usize,
    sigmas: f64,
    fft: Option<Fft>,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    fill: usize,
    power: Vec<f32>,
    est: Option<SkEstimator>,
    t: f64,
    clean: Option<f64>,
    regions: Option<Vec<Region>>,
    log: Log<KurtosisEventRow>,
}

impl Kurtosis {
    fn hz(&self, bin: f64, c: &Ctx) -> Hertz {
        Hertz(c.dial() + bin * self.rate / self.n as f64)
    }

    fn evaluate(&mut self, c: &mut Ctx) {
        let Some(est) = self.est.as_ref() else {
            return;
        };
        let n = self.n as i64;
        let edge = (EDGE_SHARE * n as f64).floor() as i64;
        let sigma = est.sigma();
        let (sk, mean) = (est.sk(), est.mean_power());
        let mut used = Vec::new();
        let flags: Vec<Option<SkKind>> = (0..n)
            .map(|i| {
                let j = i - n / 2;
                if j.abs() < FIRST_BIN || j.abs() > edge {
                    return None;
                }
                used.push(mean[i as usize]);
                sk_flag(sk[i as usize], sigma, self.sigmas)
            })
            .collect();
        let flagged = flags.iter().filter(|f| f.is_some()).count();
        self.clean = Some(100.0 * (1.0 - flagged as f64 / used.len().max(1) as f64));
        let mid = used.len() / 2;
        used.select_nth_unstable_by(mid, f64::total_cmp);
        let noise = used.get(mid).copied().unwrap_or(0.0) / MEDIAN_RATIO;
        let mut now = regions_of(SkKind::Pulsed, &flags, sk, mean, noise, edge);
        now.extend(regions_of(SkKind::Steady, &flags, sk, mean, noise, edge));
        now.sort_by(|a, b| (b.sk - 1.0).abs().total_cmp(&(a.sk - 1.0).abs()));
        let before = self.regions.take().unwrap_or_default();
        let mut matched = vec![false; before.len()];
        let mut rows = Vec::new();
        for r in &mut now {
            if let Some(k) = (0..before.len()).find(|&k| {
                !matched[k]
                    && before[k].kind == r.kind
                    && (before[k].centre() - r.centre()).abs() <= SAME_BINS
            }) {
                matched[k] = true;
                r.count = before[k].count + 1;
                r.reported = before[k].reported;
            }
            if r.count >= 2 && !r.reported {
                r.reported = true;
                let kind = if r.kind == KurtosisKind::Pulsed {
                    KurtosisChange::Pulsed
                } else {
                    KurtosisChange::Steady
                };
                rows.push((kind, *r));
            }
        }
        for (b, m) in before.iter().zip(&matched) {
            if b.reported && !m {
                rows.push((KurtosisChange::Gone, *b));
            }
        }
        for (kind, r) in rows {
            self.log.push(KurtosisEventRow {
                time_s: Seconds(self.t),
                kind,
                centre: self.hz(r.centre(), c),
                low: self.hz(r.lo, c),
                high: self.hz(r.hi, c),
                sk: r.sk,
                over_noise_db: r.over_db,
            });
        }
        if now.is_empty() {
            c.tally.good(self.t);
        } else {
            c.tally.bad(self.t);
        }
        self.regions = Some(now);
    }
}

impl TxTest for Kurtosis {
    const ID: &'static str = ID;
    const NAME: &'static str = "kurtosis.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let n = (v.get(&FFT).clamp(256, 8192) as usize).next_power_of_two();
        self.rate = rate_hz;
        self.n = n;
        self.sigmas = v.get(&SIGMAS);
        self.fft = Some(Fft::forward(n));
        self.win = Window::Hann.coefficients(n);
        self.buf = vec![Complex32::default(); n];
        self.power = vec![0.0; n];
        self.fill = 0;
        self.est = Some(SkEstimator::new(
            n,
            v.get(&FRAMES).max(2) as usize,
            v.get(&BLOCKS).max(1) as usize,
        ));
        self.clean = None;
        self.regions = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let n = self.n;
        for z in x {
            self.buf[self.fill] = z * self.win[self.fill];
            self.fill += 1;
            if self.fill < n {
                continue;
            }
            self.fill = 0;
            self.t += n as f64 / self.rate;
            let (Some(fft), Some(est)) = (self.fft.as_mut(), self.est.as_mut()) else {
                return;
            };
            fft.process(&mut self.buf);
            for (i, p) in self.power.iter_mut().enumerate() {
                *p = self.buf[(i + n / 2) % n].norm_sqr();
            }
            if est.push(&self.power) {
                self.evaluate(c);
            }
        }
    }
    fn clear(&mut self) {
        self.clean = None;
        self.regions = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, c: &Ctx) -> String {
        self.log.publish(&KURTOSIS_EVENTS, p);
        let list = self.regions.clone().unwrap_or_default();
        let rows: Vec<KurtosisRegionRow> = list
            .iter()
            .take(MAX_REGIONS)
            .map(|r| KurtosisRegionRow {
                low: self.hz(r.lo, c),
                high: self.hz(r.hi, c),
                kind: r.kind,
                sk: r.sk,
                full_power_pct: (r.kind == KurtosisKind::Pulsed && r.sk > 1.5)
                    .then(|| 100.0 / (r.sk + 1.0)),
                over_noise_db: r.over_db,
            })
            .collect();
        publish_list(&KURTOSIS_REGIONS, &rows, p);
        let (sigma, frames) = self
            .est
            .as_ref()
            .map_or((0.0, 0), |e| (e.sigma(), e.frames_per_evaluation()));
        p.put(&KurtosisFields {
            clean_pct: self.clean,
            sigma,
            regions: self.regions.as_ref().map(|r| r.len() as i64),
            block_ms: 1e3 * (frames * self.n) as f64 / self.rate.max(1.0),
        });
        match self.clean {
            Some(cl) => format!(
                "{:.1} % of the band clean; {} flagged regions",
                cl,
                list.len()
            ),
            None => "collecting frames".to_string(),
        }
    }
}

const KEYS: [&str; 4] = ["fft", "frames", "blocks", "sigmas"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("a steady and a pulsed tone in noise", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.snr_db = Some(0.0);
    c.expect_lock = false;
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::instrument::number(r, KurtosisFields::REGIONS.key);
        vec![CheckLine::new(
            KurtosisFields::REGIONS.key,
            n >= 2.0,
            format!("{n}, at least 2 expected"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: Kurtosis,
    settings: KurtosisSetting,
    keys: KEYS,
    name: "Spectral kurtosis RFI finder",
    blurb: "The bins of a wide capture that hold non-Gaussian power, steady or pulsed, by spectral kurtosis; the clean share of the band.",
    tuning: (0.1e6, 6e9, 2e6, 2e6, false),
    rate_text: "The spectral kurtosis finder needs at least 250 ksps",
    generator: |cfg| gen::KurtosisGenerator::new(cfg.rate_hz, &cfg.settings),
    verify: verify_cases
}
