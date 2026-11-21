//! The LBJ stack (`lbj`): the train broadcasts of the Chinese railway, POCSAG pages at 1200 bit/s on its UHF channel.
//!
//! Standard: ITU-R M.584 (POCSAG), TB/T 3504 (LBJ messages).

pub mod gen;
pub mod message;
pub mod modem;

use crate::decoder::WallClock;
use message::{Action, LbjMessage, Locomotives, Outcome, Pairing, LOCOMOTIVES};
use modem::{LbjBurst, LbjModem, ModemItem};
use sdrkit_core::publish::keys::Family;
use crate::typed::{LbjFields, LbjMessageEvent, LbjMessageRow, LBJ_MESSAGE, LBJ_MESSAGES};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    ConfigError, EventDef, GenConfig, Generator, LockState, Receiver, Record, ResultsDecl, Schema,
    SettingError, SettingId, SettingSet, StackConfig, StageError, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Kept, Outbox, Port, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogLine, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "lbj";
/// The least input rate, Hz.
pub const MIN_RATE: f64 = 256e3;
/// The most messages kept.
pub const MESSAGE_CAP: usize = 400;
/// The most lines the event log keeps.
pub const LOG_LINES: usize = 300;
/// A message within this many seconds of signal makes the lock state show Locked.
pub const RECENT_S: f64 = 30.0;
/// The prefix of the lines in the results log.
const PREFIX: &str = "lbj: ";
/// Knots in one km/h.
const KT_PER_KMH: f64 = 1.0 / 1.852;

/// The counters of the stack.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LbjCounters {
    /// Bursts with a sync word.
    pub bursts: u64,
    /// Messages shown.
    pub messages: u64,
    /// Messages refused by the checks.
    pub failed_checks: u64,
    /// Empty frames.
    pub blank_frames: u64,
}

/// What the stack keeps and saves: the messages, the event log and the counters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LbjHeld {
    /// The messages, newest last.
    pub messages: LogRing<LbjMessage>,
    /// The event log, newest last.
    pub log: LogRing<LogLine>,
    /// The counters.
    pub counters: LbjCounters,
}

impl Default for LbjHeld {
    fn default() -> Self {
        LbjHeld {
            messages: LogRing::new(MESSAGE_CAP),
            log: LogRing::new(LOG_LINES),
            counters: LbjCounters::default(),
        }
    }
}

impl Shift for LbjHeld {
    fn shift_times(&mut self, by_s: f64) {
        self.messages.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl Tables for LbjHeld {
    const FORMAT: StateFormat = StateFormat::new("cell.lbj_messages", 1);
    fn clear(&mut self) {
        self.messages.clear();
        self.log.clear();
        self.counters = LbjCounters::default();
    }
    fn adopt(&mut self, saved: Self) {
        self.messages.adopt(saved.messages);
        self.log.adopt(saved.log);
        self.counters = saved.counters;
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

fn text(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|t| !t.is_empty())
}

impl LbjMessage {
    /// The train, or the network time of a time message.
    fn train_text(&self) -> Option<&str> {
        text(&self.train).or(text(&self.network_time))
    }
    fn speed(&self) -> Option<i64> {
        self.speed_kmh.as_deref().and_then(|s| s.parse().ok())
    }
    fn km(&self) -> Option<f64> {
        self.km_post.as_deref().and_then(|s| s.parse().ok())
    }
    fn place(&self) -> Option<(f64, f64)> {
        self.position
            .as_ref()
            .filter(|p| p.valid)
            .map(|p| (p.latitude, p.longitude))
    }
    /// The log line of the message.
    fn line(&self) -> String {
        let dash = |s: &Option<String>| text(s).unwrap_or("-").to_owned();
        format!(
            "LBJ {} km {} {} km/h",
            self.train_text().unwrap_or_default(),
            dash(&self.km_post),
            dash(&self.speed_kmh),
        )
    }
}

impl Shaped for LbjMessage {
    type Row = LbjMessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&LbjMessageRow {
            received: self.wall.map(Time),
            train: self.train_text(),
            speed_kmh: self.speed(),
            km_post_km: self.km(),
            direction: self.direction,
            locomotive: text(&self.locomotive),
            route: text(&self.route),
            position: self.place(),
            level_dbfs: f64::from(self.level_dbfs),
        });
    }
}

impl Item for LbjMessage {
    const KIND: &'static str = "lbj.message";
    fn event_def() -> Option<&'static EventDef> {
        Some(LBJ_MESSAGE.def())
    }
    fn write_fields(&self, fields: &mut Record) {
        LbjMessageEvent {
            source: self.train_text(),
            text: &self.line(),
            family: Family::Pager,
            position: self.place(),
            speed_kt: self.speed().map(|s| s as f64 * KT_PER_KMH),
            speed_kmh: self.speed(),
            km_post_km: self.km(),
            direction: self.direction,
            locomotive: text(&self.locomotive),
            route: text(&self.route),
        }
        .write(fields);
    }
}

sdrkit_core::settings! {
    /// The settings of the `lbj.messages` decoder.
    pub enum MessagesSetting {
        /// The class names of the locomotive type codes.
        Locomotives = LOCOMOTIVES,
    }
}

/// The `lbj.messages` decoder: the pages of each burst, the LBJ messages and their pairing, and the table, log and counters.
pub struct LbjMessages {
    pairing: Pairing,
    locomotives: String,
    held: LbjHeld,
    pending_log: Vec<String>,
    clock: WallClock,
    now: f64,
    started: bool,
    level_dbfs: Option<f32>,
    snr_db: Option<f32>,
    /// Messages shown since the last output.
    shown: Vec<LbjMessage>,
    /// The ids of the messages on the map.
    on_map: HashSet<u64>,
    /// The results log must be emptied at the next report.
    cleared: bool,
    /// The messages changed since the last report.
    dirty: bool,
    /// The level in whole dB that the summary names.
    summary_level_db: Option<i32>,
    /// Signal time of the last change of frequency: an older message tells nothing of the present signal.
    forgot_s: f64,
}

impl Default for LbjMessages {
    fn default() -> Self {
        LbjMessages {
            pairing: Pairing::default(),
            locomotives: LOCOMOTIVES.default.to_owned(),
            held: LbjHeld::default(),
            pending_log: Vec::new(),
            clock: WallClock::default(),
            now: 0.0,
            started: false,
            level_dbfs: None,
            snr_db: None,
            shown: Vec::new(),
            on_map: HashSet::new(),
            cleared: false,
            dirty: true,
            summary_level_db: None,
            forgot_s: f64::NEG_INFINITY,
        }
    }
}

impl LbjMessages {
    /// The messages, the event log and the counters.
    pub fn held(&self) -> &LbjHeld {
        &self.held
    }

    fn burst(&mut self, b: &LbjBurst) {
        let rel = (b.rel.len() == b.codewords.len() * 32).then_some(&b.rel[..]);
        let mut o = Outcome::default();
        for p in message::pages(&b.codewords, rel) {
            self.pairing.page(
                p.address,
                &p.text,
                p.lost_words,
                p.bits_repaired,
                b.time,
                &mut o,
            );
        }
        let c = &mut self.held.counters;
        c.bursts += 1;
        c.failed_checks += o.failed as u64;
        c.blank_frames += o.blank as u64;
        for a in o.actions {
            let (mut m, replaces) = match a {
                Action::Show(m) => (m, None),
                Action::Replace(id, m) => (m, Some(id)),
            };
            m.level_dbfs = b.level_dbfs;
            m.carrier_offset_hz = b.carrier_offset_hz;
            m.wall = self.clock.wall(m.t);
            self.show(m, replaces);
        }
    }

    fn show(&mut self, mut m: LbjMessage, replaces: Option<u64>) {
        let line = m.line();
        self.pending_log.push(format!("{PREFIX}{line}"));
        self.held.log.push(LogLine {
            at: self.clock.stamp(m.t),
            text: line,
        });
        self.shown.push(m.clone());
        self.dirty = true;
        match replaces {
            Some(id) => {
                m.id = id;
                match self
                    .held
                    .messages
                    .find_recent_mut(MESSAGE_CAP, |x| x.id == id)
                {
                    Some(row) => *row = m,
                    None => {
                        self.held.messages.push(m);
                    }
                }
            }
            None => {
                self.held.counters.messages += 1;
                self.held.messages.push(m);
            }
        }
    }

    fn summary(&self, recent: bool) -> String {
        let c = &self.held.counters;
        if !self.started {
            return "LBJ: waiting for samples".into();
        }
        if recent {
            let m = self
                .held
                .messages
                .newest_first()
                .find(|m| m.train.is_some());
            return match m {
                Some(m) => {
                    let train = m.train.clone().unwrap_or_default();
                    match m.km_post.as_deref() {
                        Some(km) if !km.is_empty() => format!("LBJ {train} at km {km}"),
                        _ => format!("LBJ {train}"),
                    }
                }
                None => format!("LBJ: {} messages", c.messages),
            };
        }
        if c.messages > 0 {
            format!("LBJ: {} messages", c.messages)
        } else {
            format!(
                "LBJ: listening ({:.0} dBFS)",
                self.level_dbfs.unwrap_or(-200.0)
            )
        }
    }

    /// Puts the messages with a valid position on the map, labelled with the train, and takes off those that left.
    fn points(&mut self, p: &mut Published) {
        let r = p.results_mut();
        let mut now = HashSet::new();
        for m in self.held.messages.iter() {
            let Some((lat, lon)) = m.place() else {
                continue;
            };
            r.update_point(
                &format!("lbj/{}", m.id),
                lat,
                lon,
                m.train.as_deref().unwrap_or_default(),
                None,
            );
            now.insert(m.id);
        }
        for gone in self.on_map.difference(&now) {
            r.remove_point(&format!("lbj/{gone}"));
        }
        self.on_map = now;
    }
}

impl Decoder for LbjMessages {
    type Input = Timed<ModemItem>;
    fn name(&self) -> &'static str {
        "lbj.messages"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<LbjMessage>("messages")]
    }
    fn settings(&self) -> Arc<Schema> {
        MessagesSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match MessagesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MessagesSetting::Locomotives => {
                let text = LOCOMOTIVES.read(id, value)?;
                self.pairing.set_locomotives(Arc::new(Locomotives::parse(text)));
                self.locomotives = text.to_owned();
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([(MessagesSetting::Locomotives, Value::Text(self.locomotives.clone()))])
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn configure(&mut self, _input: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<LbjMessage>(), 0.0)])
    }
    fn retune(&mut self, _input: &Format) {
        self.pairing.reset();
    }
    fn process(&mut self, x: &[Timed<ModemItem>], _first: u64, out: &mut Outbox) {
        for it in x {
            self.started = true;
            self.now = self.now.max(it.time);
            match &it.value {
                ModemItem::Level { level_dbfs, snr_db } => {
                    self.level_dbfs = Some(*level_dbfs);
                    self.snr_db = Some(*snr_db);
                }
                ModemItem::Burst(b) => self.burst(b),
            }
        }
        out.items::<LbjMessage>(0)
            .extend(self.shown.drain(..).map(|m| Timed {
                time: m.t,
                value: m,
            }));
    }
    fn report(&mut self, p: &mut Published) {
        let now = self.now;
        let newest = self.held.messages.newest_first().next().filter(|m| m.t > self.forgot_s);
        let recent = newest.is_some_and(|m| now - m.t <= RECENT_S);
        let offset = newest.map(|m| f64::from(m.carrier_offset_hz));
        let c = &self.held.counters;
        p.put(&LbjFields {
            bursts: c.bursts as i64,
            messages: c.messages as i64,
            failed_checks: c.failed_checks as i64,
            blank_frames: c.blank_frames as i64,
            level_dbfs: self.level_dbfs.map(f64::from),
            snr_db: self.snr_db.map(f64::from),
            offset_hz: offset.map(Hertz),
        });
        if self.cleared {
            self.cleared = false;
            p.results_mut().clear_log();
        }
        let dirty = std::mem::take(&mut self.dirty);
        if dirty {
            shaped::log_rows(&self.held.messages, &LBJ_MESSAGES, now, p);
            for line in self.pending_log.drain(..) {
                p.results_mut().log(line);
            }
            self.points(p);
        }
        let lock_state = if !self.started {
            LockState::Off
        } else if recent {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let (level, snr) = (self.level_dbfs, self.snr_db);
        // The summary names the level in whole dB while no message is held.
        let level_db = level.map(|l| l.round() as i32);
        let new_summary = dirty
            || p.results().status().lock_state != lock_state
            || p.results().status().summary.is_empty()
            || (self.held.counters.messages == 0 && level_db != self.summary_level_db);
        let summary = new_summary.then(|| self.summary(recent));
        self.summary_level_db = level_db;
        p.results_mut().update_status(|s| {
            let same = s.lock_state == lock_state
                && summary.as_ref().is_none_or(|t| *t == s.summary)
                && s.level_dbfs == level
                && s.snr_db == snr
                && s.offset_hz == offset;
            if !same {
                s.lock_state = lock_state;
                if let Some(t) = summary {
                    s.summary = t;
                }
                s.level_dbfs = level;
                s.snr_db = snr;
                s.offset_hz = offset;
            }
            !same
        });
    }
    fn clear(&mut self) {
        self.held.clear();
        self.pairing.reset();
        self.pending_log.clear();
        self.cleared = true;
        self.dirty = true;
    }
    /// Keeps the messages and their counts while the stack keeps the message table; forgets the pages waiting for
    /// their pair, the level and the SNR.
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(LBJ_MESSAGES.key) {
            self.clear();
        }
        self.pairing.reset();
        self.level_dbfs = None;
        self.snr_db = None;
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
        self.pairing.reset();
        let last = self.held.messages.iter().map(|m| m.id).max().unwrap_or(0);
        self.pairing.continue_after(last);
        self.pending_log.clear();
        self.cleared = true;
        for l in self.held.log.iter() {
            self.pending_log.push(format!("{PREFIX}{}", l.text));
        }
        self.dirty = true;
        Ok(())
    }
}

/// The settings of the `lbj` stack: the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(crate::typed::shape(ID).schema()))
        .clone()
}

/// The LBJ receiver: the modem and the message decoder, with the `clear` command.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("modem", DecoderComponent::new(LbjModem::default()))
        .add("messages", DecoderComponent::new(LbjMessages::default()))
        .input("modem")
        .connect("modem", "messages")
        .status_from("messages")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE, "LBJ needs a sample rate of at least 256 ksps")
        .route(LOCOMOTIVES.key(), &["messages.locomotives"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the demo-loop generator of the test signal settings.
///
/// # Errors
///
/// [`ConfigError::Stage`] if the modulator cannot serve the rate.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        gen::LbjGenerator::from_config(cfg).map_err(|e| ConfigError::Stage(StageError::new(e)))?,
    ))
}
