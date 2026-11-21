//! The end-of-train stack (`eot`): the telemetry of the end-of-train device and the commands of the head of train, 1200 bit/s
//! AFSK on narrow FM, each frame checked by a (63,45) cyclic code. The link direction is a setting; the host sets it from its
//! data. The stack holds no frequency.
//!
//! No public document confirms the frame, the code, which tone is a one, the field positions or the battery scaling; the `note`
//! field says so.

pub mod frame;
pub mod gen;
pub mod modem;

use crate::decoder::WallClock;
use frame::{Direction, EotData};
use modem::{EotHeard, EotModem};
use sdrkit_core::publish::keys::Family;
use crate::typed::{EotFields, EotFrameEvent, EotFrameRow, EOT_FRAME, EOT_FRAMES};
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, ConfigError, EventDef, GenConfig, Generator, LockState, Receiver, Record, ResultsDecl,
    Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, StateBlob, StateError,
    Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kept, Kind,
    Outbox, Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "eot";
/// The least input rate, Hz.
pub const MIN_RATE: f64 = 48e3;
/// The most frames kept.
pub const FRAME_CAP: usize = 400;
/// A frame within this many seconds of signal shows the lock.
pub const RECENT_S: f64 = 30.0;
/// What no public document confirms.
pub const NOT_CONFIRMED: &str = "Not confirmed by a public document: the frame, the check polynomial and its key, which tone is a one, every field position, the battery scaling, the bit sync length on the air; head-of-train fields beyond the message type and the unit are not read.";

sdrkit_core::choice_enum! {
    /// The link a stack listens to.
    pub enum LinkChoice {
        /// End of train to head of train.
        Eot = ("eot", "End of train to head of train"),
        /// Head of train to end of train.
        Hot = ("hot", "Head of train to end of train"),
    }
}

impl LinkChoice {
    /// The direction of the frames.
    pub fn direction(self) -> Direction {
        match self {
            LinkChoice::Eot => Direction::Eot,
            LinkChoice::Hot => Direction::Hot,
        }
    }
}

/// The default link.
pub const DIRECTION_CHOICE_DEFAULT: LinkChoice = LinkChoice::Eot;

/// The link listened to.
pub const DIRECTION: Setting<field::Choice<LinkChoice>> =
    Setting::choice("direction", "Link", DIRECTION_CHOICE_DEFAULT).help(
        "End of train to head of train (telemetry) or head of train to end of train (commands).",
    );

sdrkit_core::settings! {
    /// The settings of `eot.frames`.
    pub enum EotSetting {
        /// The link.
        Direction = DIRECTION,
    }
}

/// One frame shown.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EotFrame {
    /// Signal time, s.
    pub t: f64,
    /// Wall time, seconds since 1970, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<f64>,
    /// The link.
    pub direction: Direction,
    /// The fields.
    pub data: EotData,
    /// Bits the code repaired.
    pub repaired: u8,
}

impl Shift for EotFrame {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// The counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EotCounters {
    /// Frames shown.
    pub frames: u64,
    /// Frames the code refused.
    pub bad: u64,
    /// Frames with bits repaired.
    pub repaired: u64,
    /// Frame syncs.
    pub syncs: u64,
}

/// What the stack keeps and saves.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EotHeld {
    /// The frames, newest last.
    pub frames: LogRing<EotFrame>,
    /// The counters.
    pub counters: EotCounters,
}

impl Default for EotHeld {
    fn default() -> Self {
        EotHeld {
            frames: LogRing::new(FRAME_CAP),
            counters: EotCounters::default(),
        }
    }
}

impl Shift for EotHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
    }
}

impl Tables for EotHeld {
    const FORMAT: StateFormat = StateFormat::new("cell.eot_frames", 1);
    fn clear(&mut self) {
        self.frames.clear();
        self.counters = EotCounters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.counters = saved.counters;
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

impl Shaped for EotFrame {
    type Row = EotFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&EotFrameRow {
            time_s: self.t,
            link: self.direction.label(),
            unit: &self.data.unit_text(),
            text: &self.data.text(self.direction),
        });
    }
}

impl Item for EotFrame {
    const KIND: &'static str = "eot.frame";
    fn event_def() -> Option<&'static EventDef> {
        Some(EOT_FRAME.def())
    }
    fn write_fields(&self, f: &mut Record) {
        let d = &self.data;
        let eot = (self.direction == Direction::Eot).then_some(d);
        EotFrameEvent {
            link: self.direction.label(),
            source: &d.unit_text(),
            message_type: i64::from(d.message_type),
            chaining: eot.map(|d| i64::from(d.chaining)),
            battery: eot.map(|d| i64::from(d.battery)),
            pressure_psi: eot.map(|d| i64::from(d.pressure_psi)),
            charge_pct: eot.map(|d| i64::from(d.charge_percent())),
            spare: eot.map(|d| d.spare),
            valve_open: eot.map(|d| d.valve_open),
            confirmation: eot.map(|d| d.confirmation),
            turbine: eot.map(|d| d.turbine),
            moving: eot.map(|d| d.moving),
            marker_battery: eot.map(|d| d.marker_battery),
            marker_light: eot.map(|d| d.marker_light),
            bits_corrected: i64::from(self.repaired),
            text: &d.text(self.direction),
            family: Family::Sensor,
        }
        .write(f);
    }
}

/// The `eot.frames` decoder: the frame table, the counters, the events and the saved state.
pub struct EotFrames {
    direction: LinkChoice,
    held: EotHeld,
    clock: WallClock,
    now: f64,
    started: bool,
    level_dbfs: Option<f32>,
    shown: Vec<EotFrame>,
    pending_log: Vec<String>,
    cleared: bool,
    dirty: bool,
    /// Signal time of the last change of frequency: an older frame tells nothing of the present signal.
    forgot_s: f64,
}

impl Default for EotFrames {
    fn default() -> Self {
        EotFrames {
            direction: DIRECTION_CHOICE_DEFAULT,
            held: EotHeld::default(),
            clock: WallClock::default(),
            now: 0.0,
            started: false,
            level_dbfs: None,
            shown: Vec::new(),
            pending_log: Vec::new(),
            cleared: false,
            dirty: true,
            forgot_s: f64::NEG_INFINITY,
        }
    }
}

impl EotFrames {
    /// The frames, the counters.
    pub fn held(&self) -> &EotHeld {
        &self.held
    }

    fn frame(&mut self, t: f64, data: &[u8; frame::DATA_BITS], repaired: u8) {
        let f = EotFrame {
            t,
            wall: self.clock.wall(t),
            direction: self.direction.direction(),
            data: EotData::from_bits(data),
            repaired,
        };
        let c = &mut self.held.counters;
        c.frames += 1;
        if repaired > 0 {
            c.repaired += 1;
        }
        self.pending_log.push(format!(
            "eot: {} {}: {}",
            f.direction.label(),
            f.data.unit_text(),
            f.data.text(f.direction)
        ));
        self.shown.push(f.clone());
        self.held.frames.push(f);
        self.dirty = true;
    }

    fn summary(&self, recent: bool) -> String {
        if !self.started {
            return "End of train: waiting for samples".into();
        }
        match self.held.frames.newest_first().next() {
            Some(f) if recent => format!(
                "{} {}: {}",
                f.direction.label(),
                f.data.unit_text(),
                f.data.text(f.direction)
            ),
            _ if self.held.counters.frames > 0 => {
                format!("End of train: {} frames", self.held.counters.frames)
            }
            _ => "End of train: listening".into(),
        }
    }
}

impl Decoder for EotFrames {
    type Input = Timed<EotHeard>;
    fn name(&self) -> &'static str {
        "eot.frames"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<EotFrame>("frames")]
    }
    fn settings(&self) -> Arc<Schema> {
        EotSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(EotSetting::Direction, Value::Choice(self.direction.index()))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match EotSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            EotSetting::Direction => self.direction = DIRECTION.read(id, v)?,
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![item_format::<EotFrame>(input)])
    }
    fn process(&mut self, x: &[Timed<EotHeard>], _first: u64, out: &mut Outbox) {
        for it in x {
            self.started = true;
            self.now = self.now.max(it.time);
            match &it.value {
                EotHeard::Level(l) => self.level_dbfs = Some(*l),
                EotHeard::Sync => self.held.counters.syncs += 1,
                EotHeard::Bad => {
                    self.held.counters.bad += 1;
                    self.dirty = true;
                }
                EotHeard::Good { data, repaired } => self.frame(it.time, data, *repaired),
            }
        }
        out.items::<EotFrame>(0)
            .extend(self.shown.drain(..).map(|f| Timed {
                time: f.t,
                value: f,
            }));
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now;
        let newest = self.held.frames.newest_first().next();
        let recent = newest.is_some_and(|f| f.t > self.forgot_s && now - f.t <= RECENT_S);
        let last_unit = newest.map(|f| f.data.unit_text());
        let c = &self.held.counters;
        p.put(&EotFields {
            frames_ok: c.frames as i64,
            frames_bad: c.bad as i64,
            frames_repaired: c.repaired as i64,
            syncs: c.syncs as i64,
            last_unit: last_unit.as_deref(),
            level_dbfs: self.level_dbfs.map(f64::from),
            note: NOT_CONFIRMED,
        });
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        let dirty = std::mem::take(&mut self.dirty);
        if dirty {
            shaped::log_rows(&self.held.frames, &EOT_FRAMES, now, p);
            for line in self.pending_log.drain(..) {
                p.results_mut().log(line);
            }
        }
        let lock_state = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let level = self.level_dbfs;
        let new_summary = dirty
            || p.results().status().lock_state != lock_state
            || p.results().status().summary.is_empty();
        let summary = new_summary.then(|| self.summary(recent));
        p.results_mut().update_status(|s| {
            let same = s.lock_state == lock_state
                && summary.as_ref().is_none_or(|t| *t == s.summary)
                && s.level_dbfs == level;
            if !same {
                s.lock_state = lock_state;
                if let Some(t) = summary {
                    s.summary = t;
                }
                s.level_dbfs = level;
            }
            !same
        });
    }
    fn clear(&mut self) {
        self.held.clear();
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
    }
    /// Keeps the frames and their counts while the stack keeps the frame table; forgets the level.
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(EOT_FRAMES.key) {
            self.clear();
        }
        self.level_dbfs = None;
        self.forgot_s = self.now;
        self.dirty = true;
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock.attach(ctx.live, &ctx.clock, self.now);
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.held.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.held.restore_state(state, self.now)?;
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
        Ok(())
    }
}

/// The format of a stream of items that keeps the frequencies of `from`, so the events carry the channel frequency.
fn item_format<T: Item>(from: &Format) -> Format {
    Format {
        rate: 0.0,
        frame_len: 0,
        media: None,
        ..from.with_kind(Kind::typed::<T>())
    }
}

/// The settings of the `eot` stack: the link and the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| {
Arc::new(crate::typed::shape(ID).schema())
        })
        .clone()
}

/// The receiver: the modem and the frame decoder, with the `clear` command.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("modem", DecoderComponent::new(EotModem::default()))
        .add("frames", DecoderComponent::new(EotFrames::default()))
        .input("modem")
        .connect("modem", "frames")
        .output("frames")
        .status_from("frames")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE,
            "End of train needs a sample rate of at least 48 ksps",
        )
        .route(DIRECTION.key(), &["frames.direction"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
///
/// # Errors
///
/// As [`build`].
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the looping test signal of the settings of `cfg`, the channel where the receiver looks.
///
/// # Errors
///
/// None; the signature is the descriptor's.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::EotGenerator::from_config(
        cfg,
        crate::EOT.tuning.tune_offset_hz,
    )))
}
