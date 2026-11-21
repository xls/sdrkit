//! Intermodulation finder (`intermod`): the strong signals of a wide capture, the peaks at `2 f1 - f2`, `2 f2 - f1` and
//! `f2 - f1` of every pair (the product frequencies of ITU-R SM.1134), and whether the level of each such peak follows the
//! law of a nonlinearity: a third-order product moves 2 dB for each dB of `f1` and 1 dB for each dB of `f2`. With steady
//! signals it cannot tell. It measures and does not say where a product arises. The thresholds and the rules are confirmed
//! by no public document.

pub mod gen;

use crate::instrument::{publish_list, Log};
use crate::tracked::{weighted, HeldVerdicts, Span, SAME_HZ};
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{
    IntermodEventRow, IntermodFields, IntermodInputRow, IntermodKind, IntermodProductEvent,
    IntermodProductRow, IntermodVerdict, INTERMOD_EVENTS, INTERMOD_INPUTS, INTERMOD_PRODUCT,
    INTERMOD_PRODUCTS,
};
use sdrkit_algo_dsp::levelfit::{fit, LevelVerdict};
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "intermod";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 500e3;
/// A strong signal is present in at least this share of its history.
pub const INPUT_SHARE: f64 = 0.6;
/// A product frequency within this many bins of 0 Hz is skipped.
pub const NEAR_DC_BINS: f64 = 5.0;
/// A product frequency over this share of the rate is skipped.
pub const EDGE_SHARE: f64 = 0.49;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the peaks at the product frequencies of pairs of strong signals and whether their \
level follows the intermodulation law as the signals change; with steady signals it cannot tell. Does not say where a \
product arises. The thresholds and the rules are confirmed by no public document.";

/// The snapshot length.
pub const SNAPSHOT_S: Setting<field::F64> =
    Setting::new("snapshot_s", "Snapshot length", "s", 0.25).range(0.1, 2.0, 0.0);
/// The peak threshold.
pub const PEAK_DB: Setting<field::F64> =
    Setting::new("peak_db", "Peak over the noise", "dB", 10.0).range(6.0, 30.0, 0.0);
/// The strong signal threshold.
pub const INPUT_DB: Setting<field::F64> = Setting::new(
    "input_db",
    "Strong signal over the noise (mean)",
    "dB",
    20.0,
)
.range(10.0, 60.0, 0.0);
/// The strong signals used.
pub const MAX_INPUTS: Setting<field::I64> =
    Setting::new("max_inputs", "Strong signals used", "", 8).range(2.0, 16.0, 1.0);
/// Also the second-order product.
pub const SECOND_ORDER: Setting<field::Bool> =
    Setting::new("second_order", "Also f2 - f1", "", true);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The snapshot.
        SnapshotS = SNAPSHOT_S,
        /// The peak threshold.
        PeakDb = PEAK_DB,
        /// The strong signal threshold.
        InputDb = INPUT_DB,
        /// The strong signals used.
        MaxInputs = MAX_INPUTS,
        /// The second order.
        SecondOrder = SECOND_ORDER,
    }
}

sdrkit_core::settings! {
    /// The settings of the `intermod` stack.
    pub enum IntermodSetting {
        /// The snapshot.
        SnapshotS = SNAPSHOT_S,
        /// The peak threshold.
        PeakDb = PEAK_DB,
        /// The strong signal threshold.
        InputDb = INPUT_DB,
        /// The strong signals used.
        MaxInputs = MAX_INPUTS,
        /// The second order.
        SecondOrder = SECOND_ORDER,
        /// The first tone of the test signal.
        GenF1Hz = gen::GEN_F1_HZ,
        /// The second tone.
        GenF2Hz = gen::GEN_F2_HZ,
        /// Their amplitude.
        GenAmp = gen::GEN_AMP,
        /// Their swing.
        GenSwingDb = gen::GEN_SWING_DB,
        /// The rates of the swing.
        GenSwingHz = gen::GEN_SWING_HZ,
        /// The cubic term of the receiver.
        GenK = gen::GEN_K,
        /// An independent tone.
        GenExtraHz = gen::GEN_EXTRA_HZ,
        /// Its amplitude.
        GenExtraAmp = gen::GEN_EXTRA_AMP,
    }
}

/// The verdict of the interface for a fit verdict.
pub fn verdict_of(v: LevelVerdict) -> IntermodVerdict {
    match v {
        LevelVerdict::Follows => IntermodVerdict::Follows,
        LevelVerdict::Independent => IntermodVerdict::Independent,
        LevelVerdict::Unclear => IntermodVerdict::Unclear,
        LevelVerdict::CannotTell => IntermodVerdict::CannotTell,
    }
}

/// A product found at one evaluation (frequencies in Hz from the centre).
#[derive(Clone, Copy, Debug)]
struct Product {
    hz: f64,
    kind: IntermodKind,
    a_hz: f64,
    b_hz: f64,
    below: f64,
    verdict: IntermodVerdict,
    slope: Option<f64>,
    correlation: Option<f64>,
    points: i64,
}

/// The measurement of `intermod`.
#[derive(Default)]
pub struct Intermod {
    rate: f64,
    input_db: f64,
    max_inputs: usize,
    second: bool,
    span: Span,
    t: f64,
    inputs: Option<Vec<(f64, f64)>>,
    products: Option<Vec<Product>>,
    held: HeldVerdicts<f64, IntermodVerdict>,
    log: Log<IntermodEventRow>,
}

impl Intermod {
    fn evaluate(&mut self, c: &mut Ctx) {
        let s = &self.span;
        let bin = s.bin_hz();
        let strong = s.strong(INPUT_SHARE, self.input_db, self.max_inputs);
        let mut found: Vec<(usize, Product)> = Vec::new();
        for (i, a) in strong.iter().enumerate() {
            for b in &strong[i + 1..] {
                let (a, b) = if a.bin < b.bin { (a, b) } else { (b, a) };
                let (sa, sb) = (s.series(a.index), s.series(b.index));
                let weaker = a.mean.min(b.mean);
                let mut kinds = vec![
                    (IntermodKind::TwoAMinusB, 2.0 * a.bin - b.bin, 2.0, 1.0),
                    (IntermodKind::TwoBMinusA, 2.0 * b.bin - a.bin, 1.0, 2.0),
                ];
                if self.second {
                    kinds.push((IntermodKind::BMinusA, b.bin - a.bin, 1.0, 1.0));
                }
                for (kind, place, wa, wb) in kinds {
                    if (place * bin).abs() > EDGE_SHARE * self.rate || place.abs() < NEAR_DC_BINS {
                        continue;
                    }
                    let x = weighted(&sa, wa, &sb, wb);
                    for (m, below) in s.matches(place, weaker, &[a.index, b.index]) {
                        let Some(f) = fit(&x, &s.series(m)) else {
                            continue;
                        };
                        found.push((
                            m,
                            Product {
                                hz: s.tracks()[m].bin * bin,
                                kind,
                                a_hz: a.bin * bin,
                                b_hz: b.bin * bin,
                                below,
                                verdict: verdict_of(f.verdict()),
                                slope: f.slope,
                                correlation: f.correlation,
                                points: f.n as i64,
                            },
                        ));
                    }
                }
            }
        }
        // One peak matched by several pairs keeps the match of the stronger parents.
        let mut list: Vec<Product> = Vec::new();
        for (_, p) in found {
            match list.iter_mut().find(|q| (q.hz - p.hz).abs() <= SAME_HZ) {
                Some(q) if p.below > q.below => *q = p,
                Some(_) => {}
                None => list.push(p),
            }
        }
        list.sort_by(|a, b| a.below.total_cmp(&b.below));
        let now: Vec<(f64, IntermodVerdict)> = list.iter().map(|p| (p.hz, p.verdict)).collect();
        for (hz, _) in self.held.update(&now, |a, b| (a - b).abs() <= SAME_HZ) {
            if let Some(p) = list.iter().find(|p| p.hz == hz) {
                self.row(c, p);
            }
        }
        if list.iter().any(|p| p.verdict == IntermodVerdict::Follows) {
            c.tally.good(self.t);
        } else if !list.is_empty() || !strong.is_empty() {
            c.tally.energy(self.t);
        }
        self.inputs = Some(strong.iter().map(|g| (g.bin * bin, g.mean)).collect());
        self.products = Some(list);
    }

    fn row(&mut self, c: &mut Ctx, p: &Product) {
        let d = c.dial();
        let e = IntermodProductEvent {
            time_s: Seconds(self.t),
            frequency: Hertz(d + p.hz),
            kind: p.kind,
            parent_a: Hertz(d + p.a_hz),
            parent_b: Hertz(d + p.b_hz),
            below_db: p.below,
            verdict: p.verdict,
            slope: p.slope,
            correlation: p.correlation,
            points: p.points,
        };
        c.events
            .push(INTERMOD_PRODUCT.event(self.t, e.frequency.0, &e));
        self.log.push(IntermodEventRow {
            time_s: e.time_s,
            frequency: e.frequency,
            kind: e.kind,
            parent_a: e.parent_a,
            parent_b: e.parent_b,
            below_db: e.below_db,
            verdict: e.verdict,
            slope: e.slope,
            correlation: e.correlation,
            points: e.points,
        });
    }
}

impl TxTest for Intermod {
    const ID: &'static str = ID;
    const NAME: &'static str = "intermod.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        self.rate = rate_hz;
        self.input_db = v.get(&INPUT_DB);
        self.max_inputs = v.get(&MAX_INPUTS).max(2) as usize;
        self.second = v.get(&SECOND_ORDER);
        self.span = Span::new(rate_hz, v.get(&SNAPSHOT_S), v.get(&PEAK_DB));
        self.inputs = None;
        self.products = None;
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
        self.inputs = None;
        self.products = None;
        self.held.clear();
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, c: &Ctx) -> String {
        self.log.publish(&INTERMOD_EVENTS, p);
        let d = c.dial();
        let inputs: Vec<IntermodInputRow> = self
            .inputs
            .iter()
            .flatten()
            .map(|&(hz, l)| IntermodInputRow {
                frequency: Hertz(d + hz),
                level_db: l,
            })
            .collect();
        publish_list(&INTERMOD_INPUTS, &inputs, p);
        let list = self.products.clone().unwrap_or_default();
        let rows: Vec<IntermodProductRow> = list
            .iter()
            .map(|q| IntermodProductRow {
                frequency: Hertz(d + q.hz),
                kind: q.kind,
                parent_a: Hertz(d + q.a_hz),
                parent_b: Hertz(d + q.b_hz),
                below_db: q.below,
                verdict: q.verdict,
                slope: q.slope,
                correlation: q.correlation,
                points: q.points,
            })
            .collect();
        publish_list(&INTERMOD_PRODUCTS, &rows, p);
        let count = |f: &dyn Fn(IntermodVerdict) -> bool| {
            self.products
                .as_ref()
                .map(|l| l.iter().filter(|q| f(q.verdict)).count() as i64)
        };
        let follow = count(&|v| v == IntermodVerdict::Follows);
        p.put(&IntermodFields {
            inputs: self.inputs.as_ref().map(|i| i.len() as i64),
            products: count(&|_| true),
            products_follow: follow,
            products_independent: count(&|v| v == IntermodVerdict::Independent),
            products_unknown: count(&|v| {
                matches!(v, IntermodVerdict::Unclear | IntermodVerdict::CannotTell)
            }),
            snapshots: self.span.snapshots() as i64,
        });
        match &self.products {
            Some(l) => format!(
                "{} strong signals; {} products, {} follow the law",
                inputs.len(),
                l.len(),
                follow.unwrap_or(0)
            ),
            None => "collecting snapshots".to_string(),
        }
    }
}

const KEYS: [&str; 5] = [
    "snapshot_s",
    "peak_db",
    "input_db",
    "max_inputs",
    "second_order",
];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("two swinging tones through a cubic receiver", o, 0.0);
    c.seconds = o.seconds.max(30.0);
    c.snr_db = Some(57.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::instrument::number(r, IntermodFields::PRODUCTS_FOLLOW.key);
        vec![CheckLine::new(
            IntermodFields::PRODUCTS_FOLLOW.key,
            n >= 2.0,
            format!("{n}, at least 2 expected"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: Intermod,
    settings: IntermodSetting,
    keys: KEYS,
    name: "Intermodulation finder",
    blurb: "Strong signals of a wide capture, the peaks at their product frequencies and whether each follows the intermodulation law.",
    tuning: (0.1e6, 6e9, 2e6, 2e6, false),
    rate_text: "The intermodulation finder needs at least 500 ksps",
    generator: |cfg| gen::IntermodGenerator::new(cfg.rate_hz, &cfg.settings),
    verify: verify_cases
}
