//! Fleet Broadcast (`fleet_broadcast`): the 1200 Bd differential BPSK multiplex of 16 channels of 75 bit/s on UHF military
//! satellites. The stack finds the carrier and the symbol clock, reads the bits, cuts them into 16 columns and frames them by
//! the one column that repeats. It classes each channel and decodes nothing.

pub mod gen;
pub mod rx;

use crate::display::set_status;
use sdrkit_util_objects::shaped;
use crate::typed::{
    FleetBroadcastChannelRow, FleetBroadcastFields, FleetBroadcastFrameRow,
    FleetBroadcastStateEvent, FLEET_BROADCAST_CHANNELS, FLEET_BROADCAST_FRAMES,
    FLEET_BROADCAST_STATE,
};
use rx::{Class, FleetRx, State, COLUMNS};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kind, Outbox,
    Port, RateFrom, Registry, StackGraph,
};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "fleet_broadcast";
/// The stack name.
pub const NAME: &str = "Fleet Broadcast";
/// The least input rate: twice the tune offset, the search range and the signal.
pub const MIN_RATE: f64 = 72e3;
/// Seconds between two reports.
pub const REPORT_S: f64 = 0.25;
/// The tune offset, Hz.
pub const TUNE_OFFSET_HZ: f64 = 30e3;
/// The dial of the test signal. It is not a carrier of a real broadcast: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 250.0e6;
/// What the stack says about its sources.
pub const NOTE_TEXT: &str = "experimental: simulated only. The sync pattern of channel 0 and its length, the pulse shape, the \
     bit polarity and any framing inside a channel are not public; the frame alignment by repetition and every threshold of \
     the stack are not confirmed by a public document.";

/// Flips every bit.
pub const INVERT: Setting<field::Bool> = Setting::new("invert", "Invert bits", "", false);

sdrkit_core::settings! {
    /// The settings of the stack.
    pub enum FleetSetting {
        /// Invert bits.
        Invert = INVERT,
        /// Scenario of the test signal.
        GenScenario = gen::GEN_SCENARIO,
        /// Carrier of the test signal.
        GenCarrierHz = gen::GEN_CARRIER_HZ,
        /// Fading of the test signal.
        GenFadeDb = gen::GEN_FADE_DB,
        /// Inverted bits in the test signal.
        GenInvert = gen::GEN_INVERT,
    }
}

sdrkit_core::settings! {
    /// The settings of [`FleetComponent`].
    pub enum FleetRxSetting {
        /// Invert bits.
        Invert = INVERT,
    }
}

fn decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(FleetBroadcastFields::SHAPE.fields)
                .with_table(&FLEET_BROADCAST_CHANNELS)
                .with_table(&FLEET_BROADCAST_FRAMES),
        )
    })
    .clone()
}

/// The receiver and display in one component (`satcom.fleet_receiver`).
pub struct FleetComponent {
    rx: Option<FleetRx>,
    invert: bool,
    centre_hz: f64,
    tuned_hz: f64,
    time_s: f64,
    rate: f64,
    summary: String,
}

impl Default for FleetComponent {
    fn default() -> Self {
        FleetComponent {
            rx: None,
            invert: false,
            centre_hz: 0.0,
            tuned_hz: 0.0,
            time_s: 0.0,
            rate: 0.0,
            summary: String::new(),
        }
    }
}

impl FleetComponent {
    fn class_name(c: Class) -> &'static str {
        match c {
            Class::Sync => "sync",
            Class::Reversals => "reversals",
            Class::Idle => "idle",
            Class::Random => "random-looking",
            Class::Unknown => "unknown",
        }
    }
}

impl Decoder for FleetComponent {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.fleet_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        FleetRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(FleetRxSetting::Invert, Value::Bool(self.invert))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match FleetRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FleetRxSetting::Invert => {
                self.invert = INVERT.read(id, v)?;
                if let Some(r) = self.rx.as_mut() {
                    r.invert = self.invert;
                }
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.centre_hz = f.centre_hz;
        self.tuned_hz = f.tuned_hz;
        let mut r = FleetRx::new(f.rate, f.signal_offset_hz());
        r.invert = self.invert;
        self.rx = Some(r);
        self.time_s = 0.0;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        self.centre_hz = f.centre_hz;
        self.tuned_hz = f.tuned_hz;
        if let Some(r) = self.rx.as_mut() {
            r.retune(f.signal_offset_hz());
        }
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        let Some(r) = self.rx.as_mut() else {
            return;
        };
        r.process(x);
        self.time_s = (first + x.len() as u64) as f64 / self.rate;
        let state = r.state as i64;
        let freq = self.tuned_hz + r.carrier_hz;
        for n in r.take_notes() {
            out.messages(0).push(FLEET_BROADCAST_STATE.event(
                self.time_s,
                freq,
                &FleetBroadcastStateEvent {
                    text: n.as_str(),
                    state,
                },
            ));
        }
    }
    fn report(&mut self, p: &mut Published) {
        let Some(r) = self.rx.as_ref() else {
            return;
        };
        let framed = r.state == State::Framed;
        let sync = r
            .sync_column
            .or_else(|| r.columns.iter().position(|c| c.class == Class::Sync));
        // Channel k is column (sync + k) mod 16 once framed; before that the columns in order.
        let base = r.sync_column.unwrap_or(0);
        let order: Vec<usize> = (0..COLUMNS).map(|k| (base + k) % COLUMNS).collect();
        let classes: String = order.iter().map(|&c| r.columns[c].class.letter()).collect();
        p.put(&FleetBroadcastFields {
            state: r.state as i64,
            offset_hz: Hertz(r.carrier_hz),
            freq_hz: (self.tuned_hz > 0.0).then_some(Hertz(self.tuned_hz + r.carrier_hz)),
            line_db: r.line_db,
            clock_ppm: r.clock_ppm,
            eye: r.eye(),
            snr_db: r.snr_db(),
            symbols: r.symbols as i64,
            framed,
            ambiguous: r.ambiguous,
            sync_period: sync.map(|s| r.columns[s].period as i64),
            sync_agreement: sync.map(|s| 100.0 * r.columns[s].agreement),
            frames_ok: r.frames_ok as i64,
            frames_bad: r.frames_bad as i64,
            level_dbfs: 10.0 * r.level().max(1e-20).log10(),
            classes: format!("[{classes}]").as_str(),
            note: NOTE_TEXT,
        });
        let rows: Vec<(usize, usize)> = order.iter().copied().enumerate().collect();
        shaped::indexed_rows(p, &FLEET_BROADCAST_CHANNELS, &rows, |&(k, c), out| {
            let col = &r.columns[c];
            let bits = r.column_bits(c, 64);
            let pad = (4 - bits.len() % 4) % 4;
            let mut v: Vec<u8> = vec![0; pad];
            v.extend(bits);
            let hex: String = v
                .chunks(4)
                .map(|n| {
                    let d = n.iter().fold(0u32, |a, &b| (a << 1) | u32::from(b));
                    char::from_digit(d, 16).unwrap_or('0')
                })
                .collect();
            out.put(&FleetBroadcastChannelRow {
                    channel: if framed { k as i64 } else { c as i64 },
                    class: Self::class_name(col.class),
                    activity_pct: 100.0 * col.activity,
                    ones_pct: 100.0 * col.ones,
                    sync_period: (col.class == Class::Sync).then_some(col.period as i64),
                    last_bits: hex.as_str(),
                },
            );
        });
        let log = r.frame_log();
        shaped::indexed_rows(p, &FLEET_BROADCAST_FRAMES, &log, |w, out| {
            out.put(&FleetBroadcastFrameRow {
                    word: format!("{w:04x}").as_str(),
                    bits: format!("{w:016b}").as_str(),
                },
            );
        });
        let summary = &mut self.summary;
        summary.clear();
        let _ = match r.state {
            State::Nothing => write!(summary, "listening for a Fleet Broadcast carrier"),
            State::Carrier => write!(
                summary,
                "BPSK carrier at {:+.0} Hz, {:.0} dB over the floor, looking for the 1200 Bd clock",
                r.carrier_hz, r.line_db
            ),
            State::Bits if r.ambiguous => write!(
                summary,
                "1200 Bd bits, several repeating columns: frame start not known"
            ),
            State::Bits => write!(summary, "1200 Bd bits, looking for the sync channel"),
            State::Framed => write!(
                summary,
                "framed [{classes}], sync repeat {} frames, {} frames",
                sync.map_or(0, |s| r.columns[s].period),
                r.frames_ok
            ),
        };
        let lock = match r.state {
            State::Nothing => LockState::Searching,
            State::Carrier | State::Bits => LockState::Tracking,
            State::Framed => LockState::Locked,
        };
        let snr = (r.state >= State::Bits).then_some(r.snr_db() as f32);
        set_status(
            p.results_mut(),
            lock,
            Some(r.carrier_hz),
            snr,
            None,
            summary,
        );
    }
    fn clear(&mut self) {
        if let Some(r) = self.rx.as_mut() {
            r.frames_bad = 0;
            r.frames_ok = 0;
        }
    }
}

/// Adds the components of this stack to `r`.
pub fn register(r: &mut Registry) {
    r.add("satcom.fleet_receiver", || {
        Box::new(DecoderComponent::new(FleetComponent::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("rx", DecoderComponent::new(FleetComponent::default()))
        .input("rx")
        .output("rx.events")
        .status_from("rx")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            MIN_RATE,
            "Fleet Broadcast needs a sample rate of at least 72 ksps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz + TUNE_OFFSET_HZ)
        .route("invert", &["rx.invert"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::FleetGenerator::from_config(cfg)))
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "The 16-channel 1200 Bd DBPSK multiplex of UHF Fleet Broadcast: carrier, clock, frame alignment and the activity of each channel. Nothing is decoded. Experimental: the sync channel's pattern is not public. Receive only.",
    tuning: Tuning {
        min_hz: 240e6,
        max_hz: 270e6,
        default_hz: TEST_SIGNAL_HZ,
        rate_hz: 250e3,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 0.25e6,
        tune_offset_hz: TUNE_OFFSET_HZ,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: FleetSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

/// The verification of the stack: the test signal frames.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, scenario 0", o, 0.0);
    c.seconds = c.seconds.max(10.0);
    c.expect_audio = Some(false);
    c.check = Some(check_framed);
    vec![c]
}

fn check_framed(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let s = crate::field(r, FleetBroadcastFields::STATE.key);
    vec![CheckLine::new("state", s >= 3.0, format!("state {s}"))]
}

/// The benchmark of the receiver at 250 ksps.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "fleet_broadcast.receiver_250k",
        run: |s| crate::bench_receiver(s, &DESCRIPTOR, None, 4.0),
    }]
}
