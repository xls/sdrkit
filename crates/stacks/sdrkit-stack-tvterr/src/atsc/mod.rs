//! ATSC 1.0 (8-VSB): the receiver graph, its report and the stack. The submodules hold the channel coding, the signal
//! processing, the components, the results and the test signal. PSIP is read by the shared transport-stream layer.
//!
//! Standard: ATSC A/53 Part 2, A/65.

pub mod fec;
pub mod gen;
pub mod graph;
pub mod results;
pub mod rx;

use crate::status::{write_status, QualityScore, StatusLine, StepColour};
use crate::typed::AtscFields;
use graph::*;
use rx::*;
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    AutoGain, ConfigError, CostTable, GenConfig, Generator, LockState, Receiver, Results,
    SettingSet, SignalType, StackConfig, StackDescriptor, StageError, Tuning,
};
use sdrkit_graph::{
    Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io, Kept, Port, StackGraph,
    Stage,
};
use sdrkit_proto_mpegts::layer::{self, TsLayer, DECODE_VIDEO};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "atsc";

/// `atsc.report`: gathers the stages' states into the results (lock states, typed fields, log lines).
pub struct Report {
    p: Published,
    lock_losses: u64,
    found: Option<PilotFound>,
    pl: PilotLoopState,
    clk: ClockState,
    fs: FieldSyncState,
    fec: FecReport,
    shift_hz: f64,
    quality: QualityScore,
    last_level: i64,
}

impl Default for Report {
    fn default() -> Self {
        Report {
            p: Published::from_decl(Arc::new(results::report_declaration())),
            lock_losses: 0,
            found: None,
            pl: PilotLoopState::default(),
            clk: ClockState::default(),
            fs: FieldSyncState::default(),
            fec: FecReport::default(),
            shift_hz: 0.0,
            quality: QualityScore::default(),
            last_level: -1,
        }
    }
}

impl Component for Report {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "atsc.report",
            vec![
                Port::typed::<PilotFound>("pilot"),
                Port::typed::<PilotLoopState>("loop"),
                Port::typed::<ClockState>("clock"),
                Port::typed::<FieldSyncState>("field"),
                Port::typed::<FecReport>("fec"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset();
        Ok(vec![])
    }
    /// Every result is of the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.reset();
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        for f in io.items::<PilotFound>(0) {
            self.found = Some(f.value);
            self.shift_hz = f.value.shift_hz;
            changed = true;
        }
        if let Some(s) = io.items::<PilotLoopState>(1).last() {
            self.pl = s.value;
            changed = true;
        }
        if let Some(s) = io.items::<ClockState>(2).last() {
            self.clk = s.value;
            changed = true;
        }
        if let Some(s) = io.items::<FieldSyncState>(3).last() {
            if self.fs.locked && !s.value.locked {
                self.lock_losses += 1;
            }
            self.fs = s.value;
            changed = true;
        }
        for s in io.items::<FecReport>(4) {
            self.fec = s.value;
            let c = s.value.counters;
            self.quality
                .update(c.rs_clean + c.rs_corrected, c.rs_failed, c.packets > 0);
            changed = true;
        }
        if changed {
            self.publish();
        }
    }
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

impl Report {
    /// Starts again and publishes the empty state.
    fn reset(&mut self) {
        self.lock_losses = 0;
        self.found = None;
        self.pl = PilotLoopState::default();
        self.clk = ClockState::default();
        self.fs = FieldSyncState::default();
        self.fec = FecReport::default();
        self.shift_hz = 0.0;
        self.quality.reset();
        self.last_level = -1;
        self.publish();
    }

    fn publish(&mut self) {
        let c = self.fec.counters;
        let pilot_locked = self.pl.locked;
        let segment_sync = self.clk.locked;
        let field_sync = self.fs.locked;
        let trained = field_sync && self.fec.snr_db >= 9.0;
        let good = c.rs_clean + c.rs_corrected;
        let flowing = field_sync && good > 0 && self.fec.decoded;
        // The carrier offset: the mix that put the pilot in place, plus what the loop follows (with the sign of the signal).
        let mirror = self.found.is_some_and(|x| x.mirrored);
        let off = -self.shift_hz + self.pl.freq_hz;
        let cfo = if mirror { -off } else { off };
        let detection = if flowing {
            4
        } else if field_sync {
            3
        } else if segment_sync {
            2
        } else if pilot_locked {
            1
        } else {
            0
        };
        let rs_share = if good + c.rs_failed == 0 {
            0.0
        } else {
            good as f64 / (good + c.rs_failed) as f64
        };
        let lock_state = StepColour::of;
        let rs = if c.fields == 0 {
            StepColour::Off
        } else if rs_share > 0.995 {
            StepColour::Green
        } else if rs_share > 0.5 {
            StepColour::Amber
        } else {
            StepColour::Red
        };
        let p = &mut self.p;
        p.put(&AtscFields {
            detection,
            pilot_locked,
            segment_sync,
            field_sync,
            equaliser_trained: trained,
            ts_flowing: flowing,
            offset_hz: pilot_locked.then_some(Hertz(cfo)),
            clock_ppm: segment_sync.then_some(self.clk.ppm),
            snr_db: field_sync.then_some(self.fec.snr_db),
            frames_ok: good as i64,
            frames_bad: c.rs_failed as i64,
            segment_correlation: self.clk.quality,
            field_parity: field_sync.then_some(self.fs.parity as i64),
            data_snr_db: self.fec.decoded.then_some(self.fec.data_snr_db),
            pilot_db: pilot_locked.then_some(self.pl.pilot_db),
            fields: c.fields as i64,
            rs_clean: c.rs_clean as i64,
            rs_corrected: c.rs_corrected as i64,
            rs_failed: c.rs_failed as i64,
            packets: c.packets as i64,
            field_sync_misses: self.fs.misses as i64,
            lock_losses: self.lock_losses as i64,
            quality_pct: self.quality.score,
            quality: self.quality.class(field_sync),
            step_pilot: lock_state(pilot_locked, false),
            step_segment: lock_state(segment_sync, false),
            step_field: lock_state(field_sync, segment_sync),
            step_equaliser: lock_state(trained, field_sync),
            step_trellis: lock_state(flowing, trained),
            step_rs: rs,
            step_ts: lock_state(flowing, false),
        });
        let r = p.results_mut();
        if detection != self.last_level {
            let line = match detection {
                0 => "ATSC: searching for the pilot and segment sync",
                1 => "ATSC: pilot locked",
                2 => "ATSC: segment sync, symbol clock locked",
                3 => "ATSC: field sync found",
                _ => "ATSC: transport stream flowing",
            };
            r.log(line);
            if detection >= 3 {
                r.log(format!(
                    "ATSC: carrier offset {:+.0} Hz, symbol clock {:+.1} ppm, SNR {:.1} dB",
                    cfo, self.clk.ppm, self.fec.snr_db
                ));
            }
            self.last_level = detection;
        }
        let summary = if flowing {
            "Locked - ATSC 8-VSB, 6 MHz"
        } else if field_sync {
            "syncing - ATSC 8-VSB, 6 MHz"
        } else {
            "searching - ATSC 8-VSB, 6 MHz"
        };
        let lock_state = if flowing {
            LockState::Locked
        } else if segment_sync {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        write_status(
            r,
            &StatusLine {
                lock_state,
                snr_db: field_sync.then_some(self.fec.snr_db as f32),
                offset_hz: pilot_locked.then_some(cfo),
                level_dbfs: None,
                summary,
            },
        );
    }
}

/// The recorded cost of each stage on the test signal (`cost.txt` of the crate).
const COSTS: &str = include_str!("../../cost.txt");

/// The ATSC receiver graph, with the transport stream layer after it. Whole fields cross into the stages `equalise`
/// and `fec`, whole transport packets into the stage `ts`.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<sdrkit_graph::Graph, GraphError> {
    let b = GraphBuilder::new()
        .add("pilot_find", PilotFind::default())
        .add("front", FrontComp::default())
        .add("pilot_loop", PilotLoopComp::default())
        .add("symbol_clock", ClockComp::default())
        .add("field_sync", FieldComp::default())
        .add("equalise", EqComp::default())
        .add("fec", FecComp::default())
        .add("report", Report::default())
        .input("pilot_find.in")
        .connect("pilot_find.out", "front.in")
        .connect("front.out", "pilot_loop.in")
        .connect("pilot_loop.out", "symbol_clock.in")
        .connect("symbol_clock.out", "field_sync.in")
        .connect("field_sync.out", "equalise.in")
        .connect("equalise.out", "fec.in")
        .connect("pilot_find.state", "report.pilot")
        .connect("pilot_loop.state", "report.loop")
        .connect("symbol_clock.state", "report.clock")
        .connect("field_sync.state", "report.field")
        .connect("fec.state", "report.fec")
        .status_from("report");
    layer::attach(b, "fec.ts", &TsLayer::basic())
        .first_stage("front")
        .stage("field_sync", "equalise", Stage::new("equalise"))
        .stage("equalise", "fec", Stage::new("fec"))
        .stage("fec", layer::DEMUX, Stage::new("ts").ahead_bytes(1 << 20))
        .stage("fec", layer::TS_MEDIA, Stage::new("ts").ahead_bytes(1 << 20))
        .stage("fec", "report", Stage::new("ts").ahead_bytes(1 << 20))
        .costs(&CostTable::parse(COSTS), ID, "-")
        .catalog(catalog)
        .build()
}

/// The ATSC receiver: the graph in a [`StackGraph`] that refuses a rate below 6.5 Msps.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that the stack refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            6.5e6,
            "the sample rate is too low for ATSC (6.5 Msps or more is needed)",
        )
        .route(layer::DECODE_VIDEO_ROUTE.0, layer::DECODE_VIDEO_ROUTE.1)?
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let g =
        gen::AtscGenerator::from_config(cfg).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    Ok(Box::new(g))
}

sdrkit_core::settings! {
    /// The settings of the ATSC stack.
    pub enum AtscSetting {
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
    }
}

/// The ATSC 1.0 stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "ATSC",
    group: "TV",
    blurb: "ATSC 1.0, 8-VSB, 6 MHz channel (US, Canada, Mexico, South Korea)",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 54e6,
        max_hz: 806e6,
        ..Tuning::anywhere(539e6, 8e6, 6.5e6, 6e6)
    },
    submodes: &[],
    settings: AtscSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[SignalType::Atsc1],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::ATSC),
    benchmarks: None,
};
