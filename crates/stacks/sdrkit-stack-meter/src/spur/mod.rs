//! Spur and image finder (`spur`): from one wide capture, what is not a real signal: the radio's DC spike, the IQ image of
//! each strong signal at `-f` with the gain or phase mismatch that would give it (the image ratio of a mismatch is
//! `sqrt(e^2 + p^2) / 2`, general theory of quadrature mixers), its 2nd and 3rd harmonics folded into the span, and fixed
//! spurs at multiples of the rate over a grid. A peak counts as an image or a harmonic only if its level follows the
//! signal's; with a steady signal it cannot tell. It measures; the list holds for this radio at this gain, rate and tuning.
//! The rules are confirmed by no public document.

pub mod gen;

use crate::instrument::{publish_list, Log};
use crate::tracked::{fold, presence, HeldVerdicts, Span, MATCH_BINS, RECENT, SAME_HZ};
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{
    SpurEventKind, SpurEventRow, SpurFields, SpurFindingRow, SpurKind, SpurVerdict, SPUR_EVENTS,
    SPUR_FINDINGS,
};
use sdrkit_algo_dsp::levelfit::{fit, LevelVerdict};
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "spur";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 500e3;
/// A strong signal is present in at least this share of its history.
pub const SIGNAL_SHARE: f64 = 0.6;
/// A strong signal stands this far over the noise (mean), dB.
pub const SIGNAL_DB: f64 = 20.0;
/// A place within this many bins of its signal or of 0 Hz is skipped.
pub const NEAR_BINS: f64 = 5.0;
/// A fixed spur is present in at least this share of its history.
pub const FIXED_SHARE: f64 = 0.8;
/// A fixed spur has at least this much history.
pub const FIXED_HISTORY: usize = 12;
/// The snapshot length, s.
pub const SNAPSHOT_S: f64 = 0.25;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the DC spike, the images and harmonics of strong signals that follow their signal's \
level, and fixed spurs on a grid of the rate; with steady signals it cannot tell. The findings hold for this radio at this \
gain, rate and tuning. The rules are confirmed by no public document.";

/// The peak threshold.
pub const PEAK_DB: Setting<field::F64> =
    Setting::new("peak_db", "Peak over the noise", "dB", 8.0).range(4.0, 30.0, 0.0);
/// The DC threshold.
pub const DC_DB: Setting<field::F64> =
    Setting::new("dc_db", "DC spike over the noise", "dB", 6.0).range(3.0, 30.0, 0.0);
/// The fixed spur grid.
pub const SPUR_GRID: Setting<field::I64> = Setting::new(
    "spur_grid",
    "Fixed spur grid as a fraction of the rate",
    "1/n",
    16,
)
.range(4.0, 64.0, 1.0);
/// The strong signals used.
pub const MAX_SIGNALS: Setting<field::I64> =
    Setting::new("max_signals", "Strong signals used", "", 6).range(1.0, 16.0, 1.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The peak threshold.
        PeakDb = PEAK_DB,
        /// The DC threshold.
        DcDb = DC_DB,
        /// The grid.
        SpurGrid = SPUR_GRID,
        /// The signals used.
        MaxSignals = MAX_SIGNALS,
    }
}

sdrkit_core::settings! {
    /// The settings of the `spur` stack.
    pub enum SpurSetting {
        /// The peak threshold.
        PeakDb = PEAK_DB,
        /// The DC threshold.
        DcDb = DC_DB,
        /// The grid.
        SpurGrid = SPUR_GRID,
        /// The signals used.
        MaxSignals = MAX_SIGNALS,
        /// The signals of the test signal.
        GenSignals = gen::GEN_SIGNALS,
        /// Their swing.
        GenSwingDb = gen::GEN_SWING_DB,
        /// The image rejection of the radio.
        GenImageDb = gen::GEN_IMAGE_DB,
        /// The phase of the image.
        GenImagePhase = gen::GEN_IMAGE_PHASE,
        /// The DC of the radio.
        GenDc = gen::GEN_DC,
        /// The second-order term.
        GenH2 = gen::GEN_H2,
        /// The third-order term.
        GenH3 = gen::GEN_H3,
        /// The fixed spur.
        GenSpurHz = gen::GEN_SPUR_HZ,
        /// Its amplitude.
        GenSpurAmp = gen::GEN_SPUR_AMP,
        /// A real signal.
        GenRealHz = gen::GEN_REAL_HZ,
        /// Its amplitude.
        GenRealAmp = gen::GEN_REAL_AMP,
        /// The shift of the signals.
        GenShiftHz = gen::GEN_SHIFT_HZ,
    }
}

/// The place of a finding: kind, frequency and signal, Hz from the centre.
type FindingKey = (SpurKind, f64, Option<f64>);

/// A finding of one evaluation (frequencies in Hz from the centre).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Finding {
    kind: SpurKind,
    hz: f64,
    from: Option<f64>,
    below: f64,
    verdict: SpurVerdict,
    slope: Option<f64>,
    correlation: Option<f64>,
}

impl Finding {
    fn image(&self) -> Option<(f64, f64, f64)> {
        (self.kind == SpurKind::Image).then(|| {
            let rho = 10f64.powf(-self.below / 20.0);
            (self.below, 8.686 * 2.0 * rho, 57.3 * 2.0 * rho)
        })
    }
}

fn verdict_of(v: LevelVerdict) -> SpurVerdict {
    match v {
        LevelVerdict::Follows => SpurVerdict::Follows,
        LevelVerdict::Independent => SpurVerdict::Independent,
        LevelVerdict::Unclear => SpurVerdict::Unclear,
        LevelVerdict::CannotTell => SpurVerdict::CannotTell,
    }
}

fn event_kind(k: SpurKind) -> SpurEventKind {
    match k {
        SpurKind::Image => SpurEventKind::Image,
        SpurKind::SecondHarmonic => SpurEventKind::SecondHarmonic,
        SpurKind::ThirdHarmonic => SpurEventKind::ThirdHarmonic,
        SpurKind::FixedSpur => SpurEventKind::FixedSpur,
    }
}

/// The measurement of `spur`.
#[derive(Default)]
pub struct Spur {
    rate: f64,
    dc_db: f64,
    grid: f64,
    max_signals: usize,
    span: Span,
    t: f64,
    dc: Option<f64>,
    dc_armed: bool,
    signals: Option<usize>,
    findings: Option<Vec<Finding>>,
    held: HeldVerdicts<FindingKey, SpurVerdict>,
    log: Log<SpurEventRow>,
}

impl Spur {
    fn evaluate(&mut self, c: &mut Ctx) {
        let s = &self.span;
        let bin = s.bin_hz();
        let dc = s.level_near(0.0);
        let strong = s.strong(SIGNAL_SHARE, SIGNAL_DB, self.max_signals);
        let mut used: Vec<usize> = Vec::new();
        let mut list = Vec::new();
        for a in &strong {
            let sa = s.series(a.index);
            for (kind, n, place) in [
                (SpurKind::Image, 1.0, -a.bin),
                (
                    SpurKind::SecondHarmonic,
                    2.0,
                    fold(2.0 * a.bin * bin, self.rate) / bin,
                ),
                (
                    SpurKind::ThirdHarmonic,
                    3.0,
                    fold(3.0 * a.bin * bin, self.rate) / bin,
                ),
            ] {
                if (place - a.bin).abs() < NEAR_BINS || place.abs() < NEAR_BINS {
                    continue;
                }
                let x: Vec<Option<f64>> = sa.iter().map(|v| v.map(|v| n * v)).collect();
                for (m, below) in s.matches(place, a.mean, &[a.index]) {
                    let Some(f) = fit(&x, &s.series(m)) else {
                        continue;
                    };
                    used.push(m);
                    list.push(Finding {
                        kind,
                        hz: s.tracks()[m].bin * bin,
                        from: Some(a.bin * bin),
                        below,
                        verdict: verdict_of(f.verdict()),
                        slope: f.slope,
                        correlation: f.correlation,
                    });
                }
            }
        }
        let step = self.rate / self.grid;
        let k = s.snapshots() as usize;
        for (i, t) in s.tracks().iter().enumerate() {
            if used.contains(&i) || !t.seen_within(RECENT) || t.history.len() < FIXED_HISTORY {
                continue;
            }
            let (n, mean) = presence(t);
            let hz = t.bin * bin;
            let multiple = (hz / step).round();
            if (n as f64) < FIXED_SHARE * t.history.len().min(k) as f64
                || multiple == 0.0
                || (hz - multiple * step).abs() > MATCH_BINS * bin
            {
                continue;
            }
            list.push(Finding {
                kind: SpurKind::FixedSpur,
                hz,
                from: None,
                below: mean,
                verdict: SpurVerdict::Fixed,
                slope: None,
                correlation: None,
            });
        }
        if let Some(d) = dc {
            if d > self.dc_db && self.dc_armed {
                self.dc_armed = false;
                self.log.push(SpurEventRow {
                    time_s: Seconds(self.t),
                    kind: SpurEventKind::Dc,
                    frequency: Hertz(c.dial()),
                    from_signal: None,
                    below_db: None,
                    verdict: None,
                    slope: None,
                    correlation: None,
                    image_rejection_db: None,
                    gain_mismatch_db: None,
                    phase_mismatch_deg: None,
                    dc_db: Some(d),
                });
            } else if d <= self.dc_db {
                self.dc_armed = true;
            }
        }
        let now: Vec<(FindingKey, SpurVerdict)> = list
            .iter()
            .map(|f| ((f.kind, f.hz, f.from), f.verdict))
            .collect();
        let same = |a: &FindingKey, b: &FindingKey| {
            a.0 == b.0
                && (a.1 - b.1).abs() <= SAME_HZ
                && match (a.2, b.2) {
                    (Some(x), Some(y)) => (x - y).abs() <= SAME_HZ,
                    (None, None) => true,
                    _ => false,
                }
        };
        for (key, _) in self.held.update(&now, same) {
            if let Some(f) = list.iter().find(|f| (f.kind, f.hz, f.from) == key) {
                let d = c.dial();
                let img = f.image();
                self.log.push(SpurEventRow {
                    time_s: Seconds(self.t),
                    kind: event_kind(f.kind),
                    frequency: Hertz(d + f.hz),
                    from_signal: f.from.map(|h| Hertz(d + h)),
                    below_db: Some(f.below),
                    verdict: Some(f.verdict),
                    slope: f.slope,
                    correlation: f.correlation,
                    image_rejection_db: img.map(|i| i.0),
                    gain_mismatch_db: img.map(|i| i.1),
                    phase_mismatch_deg: img.map(|i| i.2),
                    dc_db: None,
                });
            }
        }
        if dc.is_some_and(|d| d > self.dc_db)
            || list.iter().any(|f| f.verdict != SpurVerdict::Independent)
        {
            c.tally.good(self.t);
        } else {
            c.tally.energy(self.t);
        }
        self.dc = dc;
        self.signals = Some(strong.len());
        self.findings = Some(list);
    }
}

impl TxTest for Spur {
    const ID: &'static str = ID;
    const NAME: &'static str = "spur.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        self.rate = rate_hz;
        self.dc_db = v.get(&DC_DB);
        self.grid = v.get(&SPUR_GRID).max(1) as f64;
        self.max_signals = v.get(&MAX_SIGNALS).max(1) as usize;
        self.span = Span::new(rate_hz, SNAPSHOT_S, v.get(&PEAK_DB));
        self.dc = None;
        self.dc_armed = true;
        self.signals = None;
        self.findings = None;
        self.held.clear();
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let mut span = std::mem::take(&mut self.span);
        let mut due = 0;
        span.push(x, |_| due += 1);
        self.t += x.len() as f64 / self.rate;
        self.span = span;
        if due > 0 {
            self.evaluate(c);
        }
    }
    fn clear(&mut self) {
        self.dc = None;
        self.dc_armed = true;
        self.signals = None;
        self.findings = None;
        self.held.clear();
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, c: &Ctx) -> String {
        self.log.publish(&SPUR_EVENTS, p);
        let d = c.dial();
        let list = self.findings.clone().unwrap_or_default();
        let rows: Vec<SpurFindingRow> = list
            .iter()
            .map(|f| {
                let img = f.image();
                SpurFindingRow {
                    kind: f.kind,
                    frequency: Hertz(d + f.hz),
                    from_signal: f.from.map(|h| Hertz(d + h)),
                    below_db: f.below,
                    verdict: f.verdict,
                    slope: f.slope,
                    correlation: f.correlation,
                    image_rejection_db: img.map(|i| i.0),
                    gain_mismatch_db: img.map(|i| i.1),
                    phase_mismatch_deg: img.map(|i| i.2),
                }
            })
            .collect();
        publish_list(&SPUR_FINDINGS, &rows, p);
        let count = |k: &[SpurKind]| {
            self.findings.as_ref().map(|l| {
                l.iter()
                    .filter(|f| k.contains(&f.kind) && f.verdict != SpurVerdict::Independent)
                    .count() as i64
            })
        };
        p.put(&SpurFields {
            dc_db: self.dc,
            dc_spike: self.dc.map(|v| v > self.dc_db),
            signals: self.signals.map(|n| n as i64),
            images: count(&[SpurKind::Image]),
            harmonics: count(&[SpurKind::SecondHarmonic, SpurKind::ThirdHarmonic]),
            fixed_spurs: count(&[SpurKind::FixedSpur]),
            snapshots: self.span.snapshots() as i64,
        });
        match self.findings {
            Some(_) => match self.dc {
                Some(d) => format!("DC {d:.1} dB over the noise; {} findings", list.len()),
                None => format!("{} findings", list.len()),
            },
            None => "collecting snapshots".to_string(),
        }
    }
}

const KEYS: [&str; 4] = ["peak_db", "dc_db", "spur_grid", "max_signals"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("two swinging signals through a faulty radio", o, 0.0);
    c.seconds = o.seconds.max(30.0);
    c.snr_db = Some(57.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::instrument::number(r, SpurFields::IMAGES.key);
        vec![CheckLine::new(
            SpurFields::IMAGES.key,
            n >= 2.0,
            format!("{n}, at least 2 expected"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: Spur,
    settings: SpurSetting,
    keys: KEYS,
    name: "Spur and image finder",
    blurb: "The radio's DC spike, the IQ images and harmonics of strong signals that follow their levels, and fixed spurs on a grid.",
    tuning: (0.1e6, 6e9, 2e6, 2e6, false),
    rate_text: "The spur and image finder needs at least 500 ksps",
    generator: |cfg| gen::SpurGenerator::new(cfg.rate_hz, &cfg.settings),
    verify: verify_cases
}
