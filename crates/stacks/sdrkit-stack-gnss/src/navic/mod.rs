//! NavIC (`navic`): the L5 SPS signal of the Indian regional system. The stack searches PRN 1 to 14, tracks up to 8
//! satellites and decodes their subframes: subframe ID, time of week count, week number, alert and autonav flags, and the
//! message and PRN IDs of subframes 3 and 4. It makes no position.
//!
//! Standard: IRNSS Signal-in-Space ICD for Standard Positioning Service, version 1.1, ISRO.

pub mod code;
pub mod gen;
pub mod rx;

use rx::{NavicCore, LOCK_HOLD_S};
use crate::typed::{
    NavicFields, NavicSatelliteRow, NavicSubframeEvent, NAVIC_SATELLITES, NAVIC_SUBFRAME,
};
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::Published;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, BenchResult, Complex32, ConfigError, GenConfig, Generator, InputInfo,
    LockProfile, LockState, Output, Receiver, ResultsDecl, SettingSet, StackConfig,
    StackDescriptor, Tuning, Values,
};
use sdrkit_graph::{
    Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kind, Outbox, Port,
    RateFrom, Registry, StackGraph,
};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "navic";
/// The least input rate.
pub const MIN_RATE_HZ: f64 = 2.2e6;
/// The rate the stack asks for.
pub const RATE_HZ: f64 = 4.096e6;
/// What the stack says about its sources.
pub const NOTE_TEXT: &str = "experimental: not confirmed on a real signal. The scale of the time of week count (12 s a \
     count) and the direction of the interleaver (ICD Table 9 read as written column by column) are not confirmed by a \
     public document.";

sdrkit_core::settings! {
    /// The settings of the stack: the test signal's.
    pub enum NavicSetting {
        /// Satellites of the test signal.
        GenSats = gen::GEN_SATS,
        /// Doppler of the test signal.
        GenDopplerHz = gen::GEN_DOPPLER_HZ,
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The results of the component: the result fields of the stack and the satellite table.
fn decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(NavicFields::SHAPE.fields)
                .with_table(NAVIC_SATELLITES.def()),
        )
    })
    .clone()
}

/// The receiver and display in one component (`gnss.navic_receiver`).
#[derive(Default)]
pub struct NavicComponent {
    core: Option<NavicCore>,
    cleared: bool,
    rate: f64,
    tuned_hz: f64,
    summary: String,
}

impl Decoder for NavicComponent {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "gnss.navic_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Published {
        Published::from_decl(decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: f.rate,
            });
        }
        self.tuned_hz = f.tuned_hz;
        self.rate = f.rate;
        self.core = Some(NavicCore::new(f.rate, f.signal_offset_hz()));
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        // A retune keeps the rate of the configuration.
        self.tuned_hz = f.tuned_hz;
        if self.core.is_some() {
            self.core = Some(NavicCore::new(self.rate, f.signal_offset_hz()));
        }
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let Some(c) = self.core.as_mut() else {
            return;
        };
        c.feed(x);
        for h in c.take_heard() {
            let s = &h.subframe;
            out.messages(0).push(
                NAVIC_SUBFRAME.event(
                    h.time_s,
                    self.tuned_hz,
                    &NavicSubframeEvent {
                        prn: i64::from(h.prn),
                        subframe_id: i64::from(s.id),
                        towc: i64::from(s.towc),
                        wn: s.wn.map(i64::from),
                        message_id: s.message_id.map(i64::from),
                        prn_id: s.prn_id.map(i64::from),
                        alert: s.alert,
                        autonav: s.autonav,
                    },
                ),
            );
        }
    }
    fn report(&mut self, p: &mut Published) {
        let Some(c) = self.core.as_mut() else {
            return;
        };
        let log = c.take_log();
        let now = c.now_s;
        let good: u64 = c.sats.values().map(|s| s.good).sum();
        let bad: u64 = c.sats.values().map(|s| s.bad).sum();
        let best = c
            .sats
            .values()
            .filter(|s| s.tracked)
            .max_by(|a, b| a.cn0.total_cmp(&b.cn0));
        let state = if good > 0 {
            2
        } else if c.tracked() > 0 {
            1
        } else {
            0
        };
        p.put(&NavicFields {
            state,
            snr_db: best.map(|b| b.cn0),
            offset_hz: best.map(|b| Hertz(b.doppler_hz)),
            frames_ok: good as i64,
            frames_bad: bad as i64,
            searching_prn: i64::from(c.searching),
            note: NOTE_TEXT,
        });
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&NAVIC_SATELLITES);
        }
        for s in c.sats.values() {
            NAVIC_SATELLITES.put(
                p,
                &format!("{:02}", s.prn),
                &NavicSatelliteRow {
                    prn: i64::from(s.prn),
                    state: i64::from(s.state),
                    cn0_dbhz: s.cn0,
                    doppler_hz: Hertz(s.doppler_hz),
                    subframes_ok: s.good as i64,
                    subframes_bad: s.bad as i64,
                    subframe_id: s.last.as_ref().map(|l| i64::from(l.id)),
                    towc: s.last.as_ref().map(|l| i64::from(l.towc)),
                    wn: s.wn.map(i64::from),
                    alert: s.last.as_ref().map(|l| l.alert),
                },
            );
        }
        let tracked = c.tracked();
        let recent = c
            .sats
            .values()
            .any(|s| s.good > 0 && now - s.last_good_s < LOCK_HOLD_S);
        let lock = if recent {
            LockState::Locked
        } else if tracked > 0 {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let r = p.results_mut();
        for l in log {
            r.log(l);
        }
        self.summary.clear();
        let _ = write!(self.summary, "NavIC: {tracked} tracked, {good} subframes");
        let snr = best.map(|b| b.cn0 as f32);
        let offset = best.map(|b| b.doppler_hz);
        let summary = self.summary.clone();
        r.update_status(|st| {
            let same = st.lock_state == lock
                && st.summary == summary
                && st.snr_db == snr
                && st.offset_hz == offset;
            if !same {
                st.lock_state = lock;
                st.summary.clone_from(&summary);
                st.snr_db = snr;
                st.offset_hz = offset;
            }
            !same
        });
    }
    fn clear(&mut self) {
        self.cleared = true;
        if let Some(c) = self.core.as_mut() {
            c.sats.retain(|_, s| s.tracked);
            for s in c.sats.values_mut() {
                s.good = 0;
                s.bad = 0;
            }
        }
    }
}

/// Adds the components of the stack to `r`.
pub fn register(r: &mut Registry) {
    r.add("gnss.navic_receiver", || {
        Box::new(DecoderComponent::new(NavicComponent::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("rx", DecoderComponent::new(NavicComponent::default()))
        .input("rx")
        .output("rx.events")
        .status_from("rx")
        .clock(0.25)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            MIN_RATE_HZ,
            "NavIC needs a sample rate of at least 2.2 Msps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::NavicGenerator::from_config(cfg)))
}

/// NavIC.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "NavIC",
    group: "Satellite navigation",
    blurb: "The L5 SPS signal of NavIC (IRNSS): satellites tracked, C/N0, Doppler, subframes with the time of week count and the week number. No position. Experimental: the time count scale and the interleaver direction are not confirmed by a public document. Needs an active antenna (bias-tee on).",
    tuning: Tuning {
        min_hz: 1100e6,
        max_hz: 2600e6,
        default_hz: code::L5_HZ,
        rate_hz: RATE_HZ,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 3.5e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: NavicSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};

/// The verification of the stack: satellites tracked on the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, four satellites", o, 0.0);
    c.expect_audio = Some(false);
    c.check = Some(check_tracked);
    vec![c]
}

fn check_tracked(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let t = r
        .decl()
        .table_id(NAVIC_SATELLITES.key)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len());
    vec![CheckLine::new(
        "satellites",
        t >= 1,
        format!("{t} satellites seen"),
    )]
}

/// The receiver on its test signal at 4.096 Msps.
pub fn bench_receiver(seconds: f64) -> BenchResult {
    let rate = RATE_HZ;
    let v = Values::defaults(&NavicSetting::schema());
    let mut g = gen::NavicGenerator::from_config(&GenConfig::new(rate, v.clone()).with_seed(1));
    let signal_s = 1.0;
    let mut x = vec![Complex32::default(); (rate * signal_s) as usize];
    g.generate(&mut x);
    let Ok(mut rx) = make_receiver(&StackConfig::new(v)) else {
        return BenchResult::default();
    };
    if rx
        .configure(&InputInfo::new(rate, code::L5_HZ, code::L5_HZ).recording())
        .is_err()
    {
        return BenchResult::default();
    }
    // Warm up: the satellites found and tracked before the timed runs.
    let mut out = Output::default();
    for _ in 0..3 {
        for c in x.chunks(65536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    }
    sdrkit_core::bench::measure(seconds, x.len(), signal_s, || {
        for c in x.chunks(65536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}
