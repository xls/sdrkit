//! The components of the `aprs` stack: `aprs.demod` (the front end and both modems: samples to AX.25 frames, and the listen
//! audio) and `aprs.tables` (frames, stations, messages and the results). The amateur client `ham.client.ax25` sends the
//! own frames.

use super::channels::known_tuning;
use super::mesh::OwnNote;
use super::{count, show_status};
use crate::aprs::rx::{AprsRx, RawFrame, RxConfig, AUDIO_RATE};
use crate::aprs::station::{AprsTables, Ax25Frame, Heard};
use crate::mesh::ham_client::HamHeard;
use crate::settings::{
    AprsDemodSetting, AprsTablesSetting, Emphasis, Speeds, AUDIO_INPUT, EMPHASIS, LISTEN,
    MATCH_SSID, NARROW,
};
use crate::typed::{
    AprsFields, HamFields, APRS_FRAMES, APRS_REMOVE, APRS_STATIONS, APRS_STATION_ROW, MESH_MESSAGES,
};
use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::Fields;
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::Published;
use sdrkit_core::FieldId;
use sdrkit_core::{
    Choice, ConfigError, Event, LockState, Record, Results, ResultsDecl, SettingError, SettingId,
    SettingSet, StateBlob, StateError, TableId, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    Timed,
};
use sdrkit_util_objects::commands::{self as objects, CLEAR};
use sdrkit_util_objects::shaped;
use sdrkit_util_objects::Tables;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

impl Item for RawFrame {
    const KIND: &'static str = "aprs.raw";
}

/// The signal measurements of the APRS front end, at every tick (`aprs.signal`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AprsSignal {
    /// Signal time.
    pub time: f64,
    /// A carrier is present.
    pub carrier: bool,
    /// The channel is busy: a carrier is heard on radio samples. Audio input is never busy.
    pub busy: bool,
    /// Last time a carrier was present.
    pub carrier_last: f64,
    /// SNR, dB.
    pub snr_db: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Input level, dBFS.
    pub level_db: f64,
    /// Good frames at 1200 and 9600 bit/s.
    pub good: [u64; 2],
    /// Failed transmissions: frames whose FCS was wrong.
    pub bad: u64,
    /// Where the user tuned, Hz; `None` when not known.
    pub tuned_hz: Option<f64>,
}

impl Item for AprsSignal {
    const KIND: &'static str = "aprs.signal";
}

impl Item for HamHeard {
    const KIND: &'static str = "aprs.heard";
}

/// `aprs.demod`.
pub struct AprsDemod {
    cfg: RxConfig,
    rate: f64,
    rx: Option<AprsRx>,
    error: Option<ConfigError>,
    rebuild: bool,
    out: Vec<RawFrame>,
    listen: bool,
    lp: Fir<f32>,
    lp_buf: Vec<f32>,
    phase: usize,
    speeds: Speeds,
    emphasis: Emphasis,
    tuned_hz: Option<f64>,
}

impl Default for AprsDemod {
    fn default() -> Self {
        AprsDemod {
            cfg: RxConfig::default(),
            rate: 0.0,
            rx: None,
            error: None,
            rebuild: true,
            out: Vec::new(),
            listen: false,
            lp: Fir::new(
                &lowpass(AUDIO_RATE, 3.4e3, 4e3, 40.0).expect("the filter settings are valid"),
            ),
            lp_buf: Vec::new(),
            phase: 0,
            speeds: Speeds::Both,
            emphasis: Emphasis::None,
            tuned_hz: None,
        }
    }
}

/// The channel's offset in the input: the tuned frequency less the centre (0 when either is not known).
fn offset_of(centre_hz: f64, tuned_hz: Option<f64>) -> f64 {
    match tuned_hz {
        Some(t) if centre_hz > 0.0 => t - centre_hz,
        _ => 0.0,
    }
}

impl AprsDemod {
    /// The packet speeds that the modems search.
    pub fn set_speeds(&mut self, speeds: Speeds) {
        self.speeds = speeds;
        self.rebuild = true;
    }

    /// Why the last build of the receiver failed. `None` after a good build.
    pub fn error(&self) -> Option<&ConfigError> {
        self.error.as_ref()
    }
}

impl Component for AprsDemod {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "aprs.demod",
            vec![Port::iq("in")],
            vec![
                Port::typed::<RawFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<AprsSignal>("signal").rate_from(RateFrom::Fixed(0.0)),
                Port::new("listen", Kind::Audio).rate_from(RateFrom::Fixed(8000.0)),
            ],
        )
        .with_settings(AprsDemodSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(f) = inputs.first() {
            self.rate = f.rate;
            self.tuned_hz = known_tuning(f.tuned_hz);
            self.cfg.offset_hz = offset_of(f.centre_hz, self.tuned_hz);
        }
        self.rebuild = true;
        Ok(vec![
            Format::new(Kind::typed::<RawFrame>(), 0.0),
            Format::new(Kind::typed::<AprsSignal>(), 0.0),
            Format::new(Kind::Audio, 8000.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        for t in io.input(0).tags() {
            if let sdrkit_graph::TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                self.tuned_hz = known_tuning(tuned_hz);
                self.cfg.offset_hz = offset_of(centre_hz, self.tuned_hz);
                self.rebuild = true;
            }
        }
        if self.rebuild && self.rate > 0.0 {
            self.rebuild = false;
            match AprsRx::new(self.rate, self.cfg.clone()) {
                Ok(mut rx) => {
                    rx.speeds = self.speeds;
                    self.rx = Some(rx);
                    self.error = None;
                }
                Err(e) => {
                    self.rx = None;
                    self.error = Some(e);
                }
            }
        }
        let Some(rx) = &mut self.rx else {
            return;
        };
        let t0 = io.input(0).first() as f64 / self.rate;
        let local0 = rx.front.time();
        self.out.clear();
        rx.process(io.iq(0), &mut self.out);
        for mut f in self.out.drain(..) {
            // Frame times on the graph's clock.
            f.time = t0 + (f.time - local0);
            io.items_out::<RawFrame>(0).push(Timed {
                time: f.time,
                value: f,
            });
        }
        if self.listen {
            self.lp_buf.clear();
            self.lp_buf.extend_from_slice(rx.audio());
            self.lp.process_in_place(&mut self.lp_buf);
            let g = 1.0 / 5000.0;
            let out = &mut io.audio_out(2).samples;
            for &x in &self.lp_buf {
                if self.phase == 0 {
                    out.push(x * g);
                }
                self.phase = (self.phase + 1) % 6;
            }
        }
        if io.tick() {
            let level = rx.front.take_level();
            let t = t0 + io.input_len(0) as f64 / self.rate;
            let s = AprsSignal {
                time: t,
                carrier: rx.front.carrier,
                busy: rx.front.carrier && !self.cfg.audio_input,
                carrier_last: t0 + (rx.front.carrier_last - local0),
                snr_db: rx.front.snr_db,
                offset_hz: rx.front.offset_hz,
                level_db: level,
                good: rx.good,
                bad: rx.bad,
                tuned_hz: self.tuned_hz,
            };
            io.items_out::<AprsSignal>(1)
                .push(Timed { time: t, value: s });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AprsDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AprsDemodSetting::Narrow => self.cfg.narrow = NARROW.read(id, v)?,
            AprsDemodSetting::Emphasis => {
                self.emphasis = EMPHASIS.read(id, v)?;
                self.cfg.emphasis = match self.emphasis {
                    Emphasis::None => 0,
                    Emphasis::De => 1,
                    Emphasis::Pre => 2,
                };
            }
            AprsDemodSetting::AudioInput => self.cfg.audio_input = AUDIO_INPUT.read(id, v)?,
            AprsDemodSetting::Listen => {
                self.listen = LISTEN.read(id, v)?;
                return Ok(Applied::Live);
            }
        }
        self.rebuild = true;
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (AprsDemodSetting::Narrow, Value::Bool(self.cfg.narrow)),
            (
                AprsDemodSetting::Emphasis,
                Value::Choice(self.emphasis.index()),
            ),
            (
                AprsDemodSetting::AudioInput,
                Value::Bool(self.cfg.audio_input),
            ),
            (AprsDemodSetting::Listen, Value::Bool(self.listen)),
        ])
    }
}

sdrkit_core::commands! {
    /// The commands of `aprs.tables`.
    pub enum AprsTablesCommand {
        /// Clear.
        Clear = CLEAR,
        /// Remove.
        Remove = APRS_REMOVE.command,
    }
}

/// The declaration of the fields and tables of `aprs.tables`.
pub fn tables_decl() -> &'static ResultsDecl {
    static DECL: std::sync::OnceLock<ResultsDecl> = std::sync::OnceLock::new();
    DECL.get_or_init(|| {
        ResultsDecl::new()
            .with_fields(AprsFields::SHAPE.fields)
            .with_fields(HamFields::SHAPE.fields)
            .with_table(APRS_FRAMES.def())
            .with_table(APRS_STATIONS.def())
            .with_table(MESH_MESSAGES.def())
    })
}

/// The field of the own call in the results of the `aprs` stack: the state that refuses a command without a valid call.
pub fn call_field() -> FieldId {
    tables_decl()
        .field_id(HamFields::APRS_CALL.key)
        .expect("the call field is declared")
}

/// `aprs.tables`.
pub struct AprsTablesComp {
    /// The tables and messaging.
    pub tables: AprsTables,
    p: Published,
    sig: AprsSignal,
    /// The signal report is of the present frequency: false after a change of frequency until the next report.
    sig_fresh: bool,
    last_good: f64,
    state: u8,
    now: f64,
    events: Vec<Event>,
    to_client: Vec<HamHeard>,
    /// The frequency own frames go on, from the client.
    tx_freq_hz: Option<f64>,
    stations_table: TableId,
    text: String,
}

impl Default for AprsTablesComp {
    fn default() -> Self {
        let p = Published::from_decl(std::sync::Arc::new(tables_decl().clone()));
        let stations_table = p
            .results()
            .decl()
            .table_id(APRS_STATIONS.key)
            .expect("the station table is declared");
        AprsTablesComp {
            tables: AprsTables::default(),
            p,
            sig: AprsSignal {
                level_db: -200.0,
                ..Default::default()
            },
            sig_fresh: false,
            last_good: -1e9,
            state: 0,
            now: 0.0,
            events: Vec::new(),
            to_client: Vec::new(),
            tx_freq_hz: None,
            stations_table,
            text: String::new(),
        }
    }
}

impl AprsTablesComp {
    fn publish(&mut self) {
        let s = self.sig;
        let state = if s.time - self.last_good < 15.0 {
            2
        } else if s.carrier || s.time - s.carrier_last < 5.0 {
            1
        } else {
            0
        };
        if state != self.state {
            self.p.results_mut().log(match state {
                2 => "APRS: decoding frames",
                1 => "APRS: signal, no good frame lately",
                _ => "APRS: no signal",
            });
            self.state = state;
        }
        let h = &self.tables.heard;
        let total = s.good[0] + s.good[1];
        let t = &mut self.text;
        t.clear();
        let _ = write!(
            t,
            "APRS / Packet: {total} frames (1200: {}, 9600: {}), {} stations, {:.1} dBFS",
            s.good[0],
            s.good[1],
            h.stations.len(),
            s.level_db
        );
        if self.tables.own_call.is_empty() {
            t.push_str("; no callsign in the station, so nothing is sent");
        }
        let lock_state = match state {
            2 => LockState::Locked,
            1 => LockState::Tracking,
            _ => LockState::Searching,
        };
        let p = &mut self.p;
        let fresh = self.sig_fresh;
        show_status(
            p,
            lock_state,
            fresh.then_some(s.snr_db),
            fresh.then_some(s.offset_hz),
            fresh.then_some(s.level_db),
            t,
        );
        p.put(&AprsFields {
            frames_ok: count(total),
            frames_bad: count(s.bad),
            frames_1200: count(s.good[0]),
            frames_9600: count(s.good[1]),
            aprs_frames: count(h.aprs_frames),
            copies: count(h.copies_count),
            not_ax25: count(h.bad_parse),
            stations: count(h.stations.len()),
            carrier: fresh && s.carrier,
            snr_db: fresh.then_some(s.snr_db),
            level_dbfs: fresh.then_some(s.level_db),
        });
        let call = &self.tables.own_call;
        p.put(&HamFields {
            aprs_call: (!call.is_empty()).then_some(call.as_str()),
            aprs_freq: self.tx_freq_hz.map(Hertz),
        });
        shaped::log_rows(&h.frames, &APRS_FRAMES, s.time, p);
        shaped::rows(&h.stations, &APRS_STATIONS, s.time, p);
        shaped::row_points(&h.stations, &APRS_STATIONS, "stations/", p.results_mut());
        self.tables.heard.messages.publish(p, s.time);
    }
}

impl Component for AprsTablesComp {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "aprs.tables",
            vec![
                Port::typed::<RawFrame>("frames"),
                Port::typed::<AprsSignal>("signal"),
                Port::typed::<OwnNote>("own").optional("no own messages, call or frequency"),
            ],
            vec![
                Port::typed::<Ax25Frame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HamHeard>("heard").rate_from(RateFrom::Fixed(0.0)),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(AprsTablesSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<Ax25Frame>(), 0.0),
            Format::new(Kind::typed::<HamHeard>(), 0.0),
            Format::new(Kind::Messages, 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        for n in io.items::<OwnNote>(2) {
            match &n.value {
                OwnNote::HamStatus(h) => {
                    h.aprs_call.clone_into(&mut self.tables.own_call);
                    self.tx_freq_hz = h.aprs_freq_hz;
                }
                OwnNote::Text(m) => self.tables.own_message(m.clone()),
                OwnNote::HamStation(st) => self.tables.own_station(st),
                OwnNote::Log(l) => self.p.results_mut().log(l.clone()),
                _ => continue,
            }
            changed = true;
        }
        let mut to_client = std::mem::take(&mut self.to_client);
        let mut events = std::mem::take(&mut self.events);
        for f in io.items::<RawFrame>(0) {
            self.last_good = f.time;
            self.now = self.now.max(f.time);
            if let Some(row) = self.tables.frame(&f.value, &mut to_client, &mut events) {
                io.items_out::<Ax25Frame>(0).push(Timed {
                    time: f.time,
                    value: row,
                });
            }
        }
        for l in self.tables.logs.drain(..) {
            self.p.results_mut().log(l);
        }
        for s in io.items::<AprsSignal>(1) {
            self.sig = s.value;
            self.sig_fresh = true;
            self.tables.signal = Some((s.value.snr_db, s.value.level_db));
            self.now = s.value.time;
            if let Some(t) = s.value.tuned_hz {
                self.tables.freq_hz = t;
            }
        }
        if io.tick() || changed {
            self.publish();
        }
        for h in to_client.drain(..) {
            io.items_out::<HamHeard>(1).push(Timed {
                time: self.now,
                value: h,
            });
        }
        self.to_client = to_client;
        io.messages_out(2).append(&mut events);
        self.events = events;
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AprsTablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AprsTablesSetting::MatchSsid => self.tables.match_ssid = MATCH_SSID.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(
            AprsTablesSetting::MatchSsid,
            Value::Bool(self.tables.match_ssid),
        )])
    }

    fn commands(&self) -> &Commands {
        AprsTablesCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match AprsTablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            AprsTablesCommand::Clear => {
                self.tables.heard.clear();
                self.p.clear_rows(&APRS_FRAMES);
                self.p.clear_rows(&APRS_STATIONS);
                self.p.clear_rows(&MESH_MESSAGES);
                self.p.results_mut().clear_points();
            }
            AprsTablesCommand::Remove => {
                objects::remove(
                    &mut self.tables.heard.stations,
                    self.stations_table,
                    &mut self.p,
                    args,
                    &APRS_STATION_ROW,
                )?;
            }
        }
        // What is shown changes at once, not at the next report.
        self.publish();
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }

    fn forget(&mut self, kept: &Kept) {
        if !kept.table(APRS_FRAMES.key) {
            self.tables.heard.frames.clear();
            self.p.clear_rows(&APRS_FRAMES);
        }
        if !kept.table(APRS_STATIONS.key) {
            self.tables.heard.stations.clear();
            self.p.clear_rows(&APRS_STATIONS);
            self.p.results_mut().clear_points();
        }
        if !kept.table(MESH_MESSAGES.key) {
            self.tables.heard.messages.clear();
            self.p.clear_rows(&MESH_MESSAGES);
        }
        self.sig_fresh = false;
        self.tables.signal = None;
        self.last_good = -1e9;
        self.publish();
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.tables.heard.save_state(self.now).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let now = self.now;
        let mut h = Heard::default();
        h.restore_state(state, now)?;
        self.tables.heard = h;
        Ok(())
    }
}
