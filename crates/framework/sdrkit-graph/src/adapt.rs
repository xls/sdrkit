//! The adapter catalog: the declared conversions that the graph inserts where a produced format does not fit an input
//! ([`Catalog`], [`Adapter`]), and the search for the cheapest chain of them.

use crate::component::{
    checked_values_of, no_values, values_of, CentreRule, Component, FormatConstraint,
};
use crate::components::{
    AnalyticSetting, ChannelCountSetting, LlrRescaleSetting, ResampleSetting, ShiftResampleSetting,
    ShiftSetting, SpectrumSetting,
};
use crate::graph::GraphError;
use crate::stream::{Format, Kind, SampleType};
use sdrkit_core::{
    AdapterCatalog, Choice, SettingError, SettingId, SettingSet, StackConfig, Value, Values,
};
use std::sync::{Arc, LazyLock};

/// What a conversion does to the meaning of a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AdapterClass {
    /// Nothing that the consumer needs is lost (a rate change in band, a shift, a duplicate).
    Lossless,
    /// Information is lost by declaration (a downmix, a hard decision). The graph inserts it only for an input that takes it
    /// ([`FormatConstraint::lossy`]).
    DeclaredLossy,
    /// The stream becomes another signal (IQ to real is a demodulator). The graph never inserts it.
    MeaningChanging,
}

impl AdapterClass {
    /// Whether the graph can insert an adapter of this class for an input that requires `want`.
    fn insertable_for(self, want: &FormatConstraint) -> bool {
        match self {
            AdapterClass::Lossless => true,
            AdapterClass::DeclaredLossy => want.lossy,
            AdapterClass::MeaningChanging => false,
        }
    }
}

/// One entry of the catalog: a registered component kind that converts streams of kind `from` to kind `to`.
#[derive(Clone, Debug)]
pub struct Adapter {
    /// The component kind (`"resampler"`).
    pub kind: &'static str,
    /// The kind it takes.
    pub from: Kind,
    /// The kind it produces.
    pub to: Kind,
    /// What it does to the meaning of the stream.
    pub class: AdapterClass,
    /// The cost per input item, against the other entries. The graph weights it by the items that the step sees, so the
    /// step that cuts the stream most runs first.
    pub cost: u32,
    /// The settings of the component that moves a stream of format `input` toward the rule `want`. `None` when this entry
    /// does not help.
    pub settings: fn(input: &Format, want: &FormatConstraint) -> Option<Values>,
    /// Makes the component at its defaults.
    pub make: fn() -> Box<dyn Component>,
}

/// The declared conversions, in the order that breaks a tie of cost.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    adapters: Vec<Adapter>,
}

impl AdapterCatalog for Catalog {
    fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

static STANDARD: LazyLock<Arc<Catalog>> = LazyLock::new(|| Arc::new(Catalog::standard()));

impl Catalog {
    /// An empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// The conversions of this crate: rate change, shift, shift and rate change in one pass, real to mono audio, the
    /// analytic signal, downmix, channel select, duplicate, the hard decision, the LLR rescale, raw samples in bytes to `Iq`
    /// or `Real`, and the spectrum of `Iq` or `Real`.
    pub fn standard() -> Self {
        use crate::components as c;
        let mut cat = Catalog::new();
        for k in [Kind::Iq, Kind::Real, Kind::Audio] {
            cat.add(Adapter {
                kind: "dsp.resampler",
                from: k,
                to: k,
                class: AdapterClass::Lossless,
                cost: 4,
                settings: rate_change,
                make: || Box::new(c::Resample::default()),
            });
        }
        cat.add(Adapter {
            kind: "dsp.shift",
            from: Kind::Iq,
            to: Kind::Iq,
            class: AdapterClass::Lossless,
            cost: 2,
            settings: shift,
            make: || Box::new(c::Shift::default()),
        });
        cat.add(Adapter {
            kind: "dsp.shift_resample",
            from: Kind::Iq,
            to: Kind::Iq,
            class: AdapterClass::Lossless,
            cost: 5,
            settings: shift_rate_change,
            make: || Box::new(c::ShiftResample::default()),
        });
        cat.add(Adapter {
            kind: "dsp.real_to_audio",
            from: Kind::Real,
            to: Kind::Audio,
            class: AdapterClass::Lossless,
            cost: 1,
            settings: |_, _| Some(Values::defaults(&c::RealToAudioSetting::schema())),
            make: || Box::new(c::RealToAudio::default()),
        });
        cat.add(Adapter {
            kind: "dsp.analytic",
            from: Kind::Real,
            to: Kind::Iq,
            class: AdapterClass::Lossless,
            cost: 4,
            settings: analytic,
            make: || Box::new(c::Analytic::default()),
        });
        cat.add(Adapter {
            kind: "dsp.downmix",
            from: Kind::Audio,
            to: Kind::Audio,
            class: AdapterClass::DeclaredLossy,
            cost: 1,
            settings: |f, want| {
                (f.channels > 1 && want.channels == crate::ChannelRule::Exact(1)).then(no_values)
            },
            make: || Box::new(c::Downmix),
        });
        cat.add(Adapter {
            kind: "dsp.select_channels",
            from: Kind::Audio,
            to: Kind::Audio,
            class: AdapterClass::DeclaredLossy,
            cost: 1,
            settings: |f, want| match want.channels {
                crate::ChannelRule::Exact(n) if n > 1 && n < f.channels => Some(values_of([(
                    ChannelCountSetting::Channels,
                    Value::I64(i64::from(n)),
                )])),
                _ => None,
            },
            make: || Box::new(c::SelectChannels::default()),
        });
        cat.add(Adapter {
            kind: "dsp.duplicate",
            from: Kind::Audio,
            to: Kind::Audio,
            class: AdapterClass::Lossless,
            cost: 1,
            settings: |f, want| match want.channels {
                crate::ChannelRule::Exact(n) if n > 1 && f.channels == 1 => Some(values_of([(
                    ChannelCountSetting::Channels,
                    Value::I64(i64::from(n)),
                )])),
                _ => None,
            },
            make: || Box::new(c::Duplicate::default()),
        });
        cat.add(Adapter {
            kind: "dsp.hard_decision",
            from: Kind::Llr,
            to: Kind::HardBits,
            class: AdapterClass::DeclaredLossy,
            cost: 1,
            settings: |_, _| Some(no_values()),
            make: || Box::new(c::HardDecision),
        });
        cat.add(Adapter {
            kind: "dsp.llr_rescale",
            from: Kind::Llr,
            to: Kind::Llr,
            class: AdapterClass::Lossless,
            cost: 1,
            settings: llr_rescale,
            make: || Box::new(c::LlrRescale::default()),
        });
        cat.add(Adapter {
            kind: "dsp.to_iq",
            from: Kind::Bytes,
            to: Kind::Iq,
            class: AdapterClass::Lossless,
            cost: 1,
            settings: |f, _| edge(f, &COMPLEX_TYPES),
            make: || Box::new(c::ToIq::default()),
        });
        cat.add(Adapter {
            kind: "dsp.to_real",
            from: Kind::Bytes,
            to: Kind::Real,
            class: AdapterClass::Lossless,
            cost: 1,
            settings: |f, _| edge(f, &REAL_TYPES),
            make: || Box::new(c::ToReal::default()),
        });
        for k in [Kind::Iq, Kind::Real] {
            cat.add(Adapter {
                kind: "dsp.spectrum",
                from: k,
                to: Kind::Spectrum,
                class: AdapterClass::Lossless,
                cost: 8,
                settings: spectrum,
                make: || Box::new(c::Spectrum::default()),
            });
        }
        cat
    }

    /// The standard catalog, shared.
    pub(crate) fn standard_arc() -> Arc<Catalog> {
        Arc::clone(&STANDARD)
    }

    /// The catalog that a stack configuration carries, or the standard one if it carries none.
    ///
    /// # Errors
    ///
    /// [`GraphError::ForeignAdapters`] if the configuration carries adapters of another type.
    pub fn of(cfg: &StackConfig) -> Result<Arc<Catalog>, GraphError> {
        match &cfg.adapters {
            None => Ok(Self::standard_arc()),
            Some(a) => Arc::clone(a)
                .into_any()
                .downcast::<Catalog>()
                .map_err(|_| GraphError::ForeignAdapters),
        }
    }

    /// Adds an entry. An entry added later loses a tie of cost.
    pub fn add(&mut self, adapter: Adapter) -> &mut Self {
        self.adapters.push(adapter);
        self
    }

    /// The entries, in order.
    pub fn adapters(&self) -> &[Adapter] {
        &self.adapters
    }

    /// Whether a chain of at most [`Catalog::MAX_DEPTH`] entries that the graph can insert leads from kind `from` to a kind
    /// that `to` accepts. A declared-lossy entry counts only when `to` takes one. With `declared`, the format known of `from`
    /// before the stream starts, a first entry counts only when it serves that format.
    pub fn joins(&self, from: Kind, declared: Option<&Format>, to: &FormatConstraint) -> bool {
        let mut reach = vec![from];
        let mut next = Vec::new();
        for depth in 0..=Self::MAX_DEPTH {
            if reach.iter().any(|k| to.kind.accepts(k)) {
                return true;
            }
            next.clear();
            for a in self.insertable(to) {
                if reach.contains(&a.from)
                    && !reach.contains(&a.to)
                    && !next.contains(&a.to)
                    && (depth > 0 || declared.is_none_or(|f| (a.settings)(f, to).is_some()))
                {
                    next.push(a.to);
                }
            }
            if next.is_empty() {
                return false;
            }
            reach.extend_from_slice(&next);
        }
        false
    }

    /// Whether an entry that the graph can insert for `to` takes and returns kind `kind`: a codec or pixel conversion.
    pub fn converts_within(&self, kind: Kind, to: &FormatConstraint) -> bool {
        self.insertable(to).any(|a| a.from == kind && a.to == kind)
    }

    /// The most entries in one chain.
    pub const MAX_DEPTH: usize = 3;

    fn insertable<'a>(&'a self, want: &'a FormatConstraint) -> impl Iterator<Item = &'a Adapter> {
        self.adapters
            .iter()
            .filter(move |a| a.class.insertable_for(want))
    }

    /// The cheapest chain that brings a stream of format `input` to one that `want` accepts, each step with its settings and its
    /// output format. `None` when no chain of at most [`Catalog::MAX_DEPTH`] entries does it. The search is
    /// deterministic: the lowest cost wins, then the earlier entries.
    pub(crate) fn chain(&self, input: &Format, want: &FormatConstraint) -> Option<Vec<Step>> {
        let mut best: Option<(f64, Vec<Step>)> = None;
        let mut path = Vec::new();
        let volume = volume(input).max(f64::MIN_POSITIVE);
        self.search(input, want, 0.0, volume, &mut path, &mut best);
        best.map(|(_, steps)| steps)
    }

    fn search(
        &self,
        cur: &Format,
        want: &FormatConstraint,
        cost: f64,
        volume0: f64,
        path: &mut Vec<Step>,
        best: &mut Option<(f64, Vec<Step>)>,
    ) {
        if path.len() == Self::MAX_DEPTH {
            return;
        }
        for (index, a) in self.adapters.iter().enumerate() {
            if a.from != cur.kind || !a.class.insertable_for(want) {
                continue;
            }
            if a.from != a.to && !reaches(self, a.to, want, Self::MAX_DEPTH - path.len() - 1) {
                continue;
            }
            let Some(values) = (a.settings)(cur, want) else {
                continue;
            };
            let step_cost = cost + f64::from(a.cost) * volume(cur) / volume0;
            if best.as_ref().is_some_and(|(c, _)| step_cost >= *c) {
                continue;
            }
            let Some((_, output)) = configured(a, &values, cur) else {
                continue;
            };
            if output == *cur {
                continue;
            }
            path.push(Step {
                adapter: index,
                values,
                output,
            });
            let out = &path.last().expect("pushed").output;
            if want.accepts_format(out) {
                *best = Some((step_cost, path.clone()));
                path.pop();
                continue;
            }
            let out = out.clone();
            self.search(&out, want, step_cost, volume0, path, best);
            path.pop();
        }
    }
}

/// Whether kind `to` reaches a kind that `want` accepts in at most `depth` more steps.
fn reaches(cat: &Catalog, to: Kind, want: &FormatConstraint, depth: usize) -> bool {
    if want.kind.accepts(&to) {
        return true;
    }
    depth > 0
        && cat
            .insertable(want)
            .any(|a| a.from == to && a.to != to && reaches(cat, a.to, want, depth - 1))
}

/// One step of a chain: the catalog entry, its settings and its output format.
#[derive(Clone)]
pub(crate) struct Step {
    pub adapter: usize,
    pub values: Values,
    pub output: Format,
}

/// Makes the component of `a` with `values`, configured for `input`, and its output format.
pub(crate) fn configured(
    a: &Adapter,
    values: &Values,
    input: &Format,
) -> Option<(Box<dyn Component>, Format)> {
    let mut c = (a.make)();
    for i in 0..values.schema().len() {
        let id = SettingId(i as u16);
        if let Some(v) = values.value(id) {
            c.apply(id, v).ok()?;
        }
    }
    let mut outs = c.configure(std::slice::from_ref(input)).ok()?;
    let out = outs.pop().filter(|o| o.kind == a.to)?;
    Some((c, out))
}

/// Items per second times the values per item: what a step must touch.
fn volume(f: &Format) -> f64 {
    let lanes = if matches!(f.kind, Kind::Iq | Kind::Symbols) {
        2.0
    } else {
        1.0
    };
    f.rate.max(1.0) * f64::from(f.channels.max(1)) * lanes
}

/// The rate that `want` requires, when `f` does not have it. A width rule with any rate produces the input rate divided by the
/// largest whole number that keeps it at the width or above.
fn wanted_rate(f: &Format, want: &FormatConstraint) -> Option<f64> {
    match (want.rate, want.width) {
        (crate::RateRule::Exact(r), _) if r != f.rate => Some(r),
        (crate::RateRule::AtLeast(r), _) if f.rate < r => Some(r),
        (crate::RateRule::Any, Some(w)) if !want.width_accepts(f.rate) => Some(if f.rate < w {
            w
        } else {
            f.rate / (f.rate / w).floor()
        }),
        _ => None,
    }
}

/// The shift that brings the centre that `want` needs to 0 Hz, when `f` is not there.
#[derive(Clone, Copy)]
enum ShiftNeed {
    /// By this much, inside the capture.
    Fixed(f64),
    /// By this much, silent while it leaves the capture.
    MayLeave(f64),
    /// The tuned frequency to 0 Hz, following each retune.
    Follow,
}

/// The shift that `want` needs from `f`. A fixed centre outside the capture needs one that the graph cannot make.
fn wanted_shift(f: &Format, want: &FormatConstraint) -> Option<ShiftNeed> {
    match want.centre.filter(|c| !c.fits(f))? {
        CentreRule::Fixed(c) => {
            let s = f.centre_hz - c;
            (s.abs() <= f.rate / 2.0).then_some(ShiftNeed::Fixed(s))
        }
        CentreRule::MayLeave(c) => Some(ShiftNeed::MayLeave(f.centre_hz - c)),
        CentreRule::Tuned => Some(ShiftNeed::Follow),
    }
}

/// The settings of a shift for `need`, with the ids of its `freq`, `follow` and `out_of_view` settings.
fn shift_values<S: SettingSet>(need: ShiftNeed, ids: [S; 3], rate: Option<(S, f64)>) -> Values {
    let [freq, follow, out_of_view] = ids;
    let mut v: Vec<(S, Value)> = match need {
        ShiftNeed::Fixed(s) => vec![(freq, Value::Hz(s))],
        ShiftNeed::MayLeave(s) => vec![(freq, Value::Hz(s)), (out_of_view, Value::Bool(true))],
        ShiftNeed::Follow => vec![(follow, Value::Bool(true))],
    };
    v.extend(rate.map(|(id, r)| (id, Value::Hz(r))));
    values_of(v)
}

fn rate_change(f: &Format, want: &FormatConstraint) -> Option<Values> {
    // For `Iq` away from the centre that the input needs, the shift comes first.
    if f.kind == Kind::Iq && want.centre.is_some_and(|c| !c.fits(f)) {
        return None;
    }
    let r = wanted_rate(f, want)?;
    Some(values_of([(ResampleSetting::Rate, Value::Hz(r))]))
}

fn shift(f: &Format, want: &FormatConstraint) -> Option<Values> {
    use ShiftSetting as S;
    let need = wanted_shift(f, want)?;
    Some(shift_values(need, [S::Freq, S::Follow, S::OutOfView], None))
}

fn shift_rate_change(f: &Format, want: &FormatConstraint) -> Option<Values> {
    use ShiftResampleSetting as S;
    let need = wanted_shift(f, want)?;
    let r = wanted_rate(f, want)?;
    Some(shift_values(
        need,
        [S::Freq, S::Follow, S::OutOfView],
        Some((S::Rate, r)),
    ))
}

fn analytic(f: &Format, want: &FormatConstraint) -> Option<Values> {
    let low = want.lowest_hz.unwrap_or(0.025 * f.rate);
    Some(values_of([(AnalyticSetting::Low, Value::Hz(low))]))
}

fn llr_rescale(f: &Format, want: &FormatConstraint) -> Option<Values> {
    let (from, to) = (f.llr_scale?, want.llr_scale?);
    ((from - to).abs() > 1e-6 * to.abs())
        .then(|| values_of([(LlrRescaleSetting::Scale, Value::F64(f64::from(to)))]))
}

/// The sample types that [`crate::components::ToIq`] reads.
const COMPLEX_TYPES: [SampleType; 5] = [
    SampleType::Cs8,
    SampleType::Cu8,
    SampleType::Cs16,
    SampleType::Cs32,
    SampleType::Cf32,
];
/// The sample types that [`crate::components::ToReal`] reads.
const REAL_TYPES: [SampleType; 5] = [
    SampleType::I8,
    SampleType::U8,
    SampleType::I16,
    SampleType::I32,
    SampleType::F32,
];

/// No settings, when `f` holds samples of one of `types`.
fn edge(f: &Format, types: &[SampleType]) -> Option<Values> {
    f.sample_type
        .filter(|t| types.contains(t))
        .map(|_| no_values())
}

fn spectrum(f: &Format, want: &FormatConstraint) -> Option<Values> {
    want.spectrum?.values(f.rate).ok()
}

impl crate::SpectrumSpec {
    /// The settings of the spectrum component that makes this spectrum from a source of `rate_hz` samples per second.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if a value is outside the range of the component's setting (a frame rate or an average too high).
    pub fn values(&self, rate_hz: f64) -> Result<Values, SettingError> {
        use crate::components::FrameTiming;
        // A back-to-back spectrum does not read `fps`: it keeps its default.
        let (timing, fps) = match self.frames {
            crate::FrameRule::PerSecond(r) => (FrameTiming::PerSecond, r),
            crate::FrameRule::BackToBack => {
                (FrameTiming::BackToBack, crate::components::FPS_DEFAULT)
            }
        };
        checked_values_of([
            (
                SpectrumSetting::Size,
                Value::Choice(self.size.size(rate_hz).index()),
            ),
            (SpectrumSetting::Window, Value::Choice(self.window.index())),
            (
                SpectrumSetting::Average,
                Value::I64(i64::from(self.average)),
            ),
            (SpectrumSetting::Fps, Value::F64(fps)),
            (SpectrumSetting::Timing, Value::Choice(timing.index())),
            (SpectrumSetting::Scale, Value::Choice(self.scale.index())),
        ])
    }
}

/// What [`Graph::validate`](crate::Graph::validate) resolved and inserted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ValidateReport {
    /// Every port that carries a stream (the graph input, each output, each inserted adapter's output) with its resolved
    /// format, in stream order.
    pub ports: Vec<(String, Format)>,
    /// Every adapter and derivation that the graph inserted, in the order it inserted them.
    pub adapters: Vec<InsertedAdapter>,
}

/// One adapter or derivation that validate inserted.
#[derive(Clone, Debug, PartialEq)]
pub struct InsertedAdapter {
    /// Its id in the graph: `~` and the kind and a number, so it never meets an id of the plan.
    pub id: String,
    /// The component kind of the catalog entry.
    pub kind: &'static str,
    /// What it does to the meaning of the stream.
    pub class: AdapterClass,
    /// The port it reads.
    pub source: String,
    /// The port it produces.
    pub output: String,
    /// The inputs that read its output at the end of their chain. Empty for a step inside a chain.
    pub consumers: Vec<String>,
    /// Its group delay, in seconds. The format of each port holds the delay of its whole path ([`Format::delay_s`]).
    pub group_delay_s: f64,
    /// The index of its first output item whose filters are full.
    pub first_valid: u64,
}
