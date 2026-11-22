//! The Airband results: the channel table, the transmission log (kept through a restart and in the saved state), the band
//! watch list, the status line and the events.

use super::channel::ChannelStatus;
use super::{Activity, BankState, ScanState, Seen as Carrier, SeenList, ONLY_ACTIVE};
use crate::report::{self, Heard};
use crate::typed::{
    AirbandActivityEvent, AirbandActivityRow, AirbandChannelRow, AirbandFields, AirbandSeenRow,
    AIRBAND_ACTIVITY, AIRBAND_CHANNELS, AIRBAND_SEEN, AIRBAND_TRANSMISSION,
};
use sdrkit_core::interface::{Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, CommandError, CommandId, CommandSet, Commands, ConfigError, Event, Record, Results,
    SettingError, SettingId, SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
};
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Tables as _};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Log entries kept.
pub const LOG: usize = 300;

impl Shaped for Heard<Activity> {
    type Row = AirbandActivityRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let a = &self.item;
        out.put(&AirbandActivityRow {
            time: self.wall.map(Time),
            start_s: self.at,
            name: &a.name,
            label: report::text(&a.label),
            frequency: Hertz(a.freq_hz),
            length: Seconds(a.dur_s),
            cn_db: f64::from(a.cn_db),
            level_db: f64::from(a.level_db),
            heterodyne: a.heterodyne,
        });
    }
}

impl Shaped for ChannelStatus {
    type Row = AirbandChannelRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AirbandChannelRow {
            name: &self.name,
            label: report::text(&self.label),
            frequency: Hertz(self.freq_hz),
            in_band: self.in_band,
            cn_db: f64::from(self.cn_db),
            level_db: f64::from(self.level_db),
            squelch_db: self.squelch_db,
            open: self.open,
            playing: self.playing,
            heterodyne: self.heterodyne,
            offset_hz: Hertz(f64::from(self.offset_hz)),
            count: i64::try_from(self.transmissions).unwrap_or(i64::MAX),
        });
    }
}

impl Shaped for Carrier {
    type Row = AirbandSeenRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AirbandSeenRow {
            name: &self.name,
            frequency: Hertz(self.freq_hz),
            is833: self.is833,
            excess_db: f64::from(self.excess_db),
            reports: i64::try_from(self.reports).unwrap_or(i64::MAX),
            active: self.active,
            approximate: self.approximate,
            last_s: self.last_s,
        });
    }
}

/// What is kept across sessions: the transmission log.
#[derive(Serialize, Deserialize)]
pub struct Store {
    log: LogRing<Heard<Activity>>,
    heard: u64,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            log: LogRing::new(LOG),
            heard: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.log.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("avdata.airband", 2);
    fn clear(&mut self) {
        self.log.clear();
        self.heard = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.log.adopt(saved.log);
        self.heard = saved.heard;
    }
}

sdrkit_core::settings! {
    /// The settings of [`Tables`].
    pub enum TablesSetting {
        /// Show only the open channels.
        OnlyActive = ONLY_ACTIVE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Tables`].
    pub enum TablesCommand {
        /// Forget every transmission.
        Clear = objcmd::CLEAR,
    }
}

/// The results component (`avdata.airband_tables`).
pub struct Tables {
    published: Published,
    store: Store,
    bank: BankState,
    scan: ScanState,
    seen: SeenList,
    events: Vec<Event>,
    summary: String,
    only_active: bool,
    live: bool,
    clock: Clock,
    configured: bool,
}

impl Default for Tables {
    fn default() -> Self {
        Tables {
            published: crate::typed::results(super::ID),
            store: Store::default(),
            bank: BankState::default(),
            scan: ScanState::default(),
            seen: SeenList::default(),
            events: Vec::new(),
            summary: String::new(),
            only_active: ONLY_ACTIVE.default,
            live: false,
            clock: Clock::fixed(0.0),
            configured: false,
        }
    }
}

impl Tables {
    fn publish(&mut self) {
        for c in self.bank.channels.iter_mut() {
            c.playing = self.scan.playing.contains(&(c.freq_hz as i64));
        }
        let b = &self.bank;
        let now = b.time_s;
        let inb = b.channels.iter().filter(|c| c.in_band).count();
        let open = b.channels.iter().filter(|c| c.in_band && c.open).count();
        let state = if inb == 0 {
            0
        } else if open == 0 {
            1
        } else {
            2
        };
        let n = self.store.log.len();
        self.summary.clear();
        let _ = if b.channels.is_empty() {
            write!(self.summary, "Airband: no channel in the list")
        } else if inb == 0 {
            write!(
                self.summary,
                "Airband: no channel inside the band, tune nearer"
            )
        } else if open > 0 {
            let _ = write!(self.summary, "Airband: ");
            let open_channels = b.channels.iter().filter(|c| c.in_band && c.open);
            for (i, c) in open_channels.enumerate() {
                let label = if c.label.is_empty() {
                    &c.name
                } else {
                    &c.label
                };
                let sep = if i == 0 { "" } else { ", " };
                let _ = write!(self.summary, "{sep}{label}");
            }
            write!(self.summary, " open, {n} transmissions")
        } else {
            write!(
                self.summary,
                "Airband: {} {inb} channels, {n} transmissions",
                if self.scan.scan {
                    "scanning"
                } else {
                    "listening to"
                },
            )
        };
        let best = b
            .channels
            .iter()
            .filter(|c| c.in_band && (open == 0 || c.open))
            .map(|c| c.cn_db)
            .fold(f32::MIN, f32::max);
        let p = &mut self.published;
        report::status(
            p.results_mut(),
            state,
            (best > f32::MIN).then_some(best),
            Some(b.level_db),
            Some(b.tune_error_hz),
            &self.summary,
        );
        p.put(&AirbandFields {
            channel_rate: Hertz(b.channel_rate_hz),
            tuning_error: b.known.then_some(Hertz(b.tune_error_hz)),
            squelch_db: b.squelch_db,
            hang: Seconds(b.hang_s),
            scan: self.scan.scan,
            scan_channel: (self.scan.channel >= 0).then_some(Hertz(self.scan.channel as f64)),
            transmissions: i64::try_from(self.store.heard).unwrap_or(i64::MAX),
        });
        let only_active = self.only_active;
        shaped::rows_of(
            b.channels.iter().filter(|c| !only_active || c.open),
            |c, w| w.write_str(&c.name),
            &AIRBAND_CHANNELS,
            now,
            p,
        );
        shaped::rows_of(
            &self.seen.entries,
            |s, w| w.write_str(&s.name),
            &AIRBAND_SEEN,
            now,
            p,
        );
        shaped::log_rows(&self.store.log, &AIRBAND_ACTIVITY, now, p);
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.airband_tables",
            vec![
                Port::typed::<BankState>("state"),
                Port::typed::<Activity>("activity"),
                Port::typed::<SeenList>("seen"),
                Port::typed::<ScanState>("scan"),
            ],
            vec![Port::new("out", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TablesSetting::schema())
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A restart forgets the signal and keeps the log: its times move back by the old signal time.
        if self.configured {
            self.store.shift_times(-self.bank.time_s);
        }
        self.configured = true;
        self.bank = BankState::default();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        for a in io.items::<Activity>(1) {
            let a = &a.value;
            self.store.heard += 1;
            self.events.push(AIRBAND_TRANSMISSION.event(
                a.start_s,
                a.freq_hz,
                &AirbandActivityEvent {
                    source: None,
                    target: None,
                    channel: &a.name,
                    text: report::text(&a.label),
                    snr_db: f64::from(a.cn_db),
                    family: Family::Voice,
                    length: Seconds(a.dur_s),
                    cn_db: f64::from(a.cn_db),
                },
            ));
            let wall = report::wall(self.live, &self.clock);
            self.store.log.push(Heard {
                at: a.start_s,
                wall,
                item: a.clone(),
            });
        }
        for s in io.items::<SeenList>(2) {
            self.seen.clone_from(&s.value);
        }
        for s in io.items::<ScanState>(3) {
            self.scan.clone_from(&s.value);
        }
        let mut rep = false;
        for s in io.items::<BankState>(0) {
            for l in &s.value.log {
                self.published.results_mut().log(l.as_str());
            }
            self.bank.clone_from(&s.value);
            rep = true;
        }
        if rep {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TablesSetting::OnlyActive => self.only_active = ONLY_ACTIVE.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(TablesSetting::OnlyActive, Value::Bool(self.only_active))])
    }

    fn commands(&self) -> &Commands {
        TablesCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match TablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TablesCommand::Clear => self.store.clear(),
        }
        self.publish();
        Ok(None)
    }

    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        self.bank.channels.clear();
        self.bank.open_since.clear();
        self.bank.known = false;
        self.scan = ScanState::default();
        if !kept.table(AIRBAND_SEEN.key) {
            self.seen = SeenList::default();
        }
        if !kept.table(AIRBAND_ACTIVITY.key) {
            self.store.log.clear();
        }
        if !kept.field("transmissions") {
            self.store.heard = 0;
        }
        self.publish();
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.bank.time_s).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.bank.time_s)?;
        self.publish();
        Ok(())
    }
}
