//! The `qam_video` stack: its settings, results, receiver graph (coarse carrier search, resampler, frame receiver, FEC
//! groups), transmit branch, test signal and descriptor.

use super::container::{Packet, Unpacker, TEXT};
use super::phy::{Waveform, Profile};
use super::rx::{Frame, FrameReceiver};
use super::{ident_callsign, session_samples, Session, Source};
use crate::family::{hex_bytes, DEMO_CALLSIGN};
use crate::typed::{QamVideoFields, QamVideoStreamRow, TxEvent, QAM_VIDEO_STREAMS, QAM_VIDEO_TX};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Record, Schema, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor, StageError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Decoder, DecoderComponent, Format, Io, Kept, Kind, Outbox, Port,
    RateFrom, RunContext, TagKind,
};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "qam_video";
/// The lowest input rate.
pub const MIN_RATE: f64 = 2.0e6;
/// What the stack says about its source.
pub const NOTE: &str = crate::slotted_voice::stack::NOTE;

sdrkit_core::choice_enum! {
    /// The profile of the transmitter.
    #[derive(Default)]
    pub enum TxProfile {
        /// 700 ksym/s, 16-QAM.
        #[default]
        A = ("a", "A: 700 ksym/s, 16-QAM"),
        /// 1050 ksym/s, 16-QAM.
        B = ("b", "B: 1050 ksym/s, 16-QAM"),
        /// 1400 ksym/s, 16-QAM.
        C = ("c", "C: 1400 ksym/s, 16-QAM"),
        /// 1400 ksym/s, 64-QAM.
        D = ("d", "D: 1400 ksym/s, 64-QAM"),
    }
}

impl TxProfile {
    /// The profile.
    pub fn profile(self) -> Profile {
        Profile::ALL[self.index() as usize]
    }
}

sdrkit_core::choice_enum! {
    /// The symbol rate the receiver listens to.
    #[derive(Default)]
    pub enum ListenProfile {
        /// 700 ksym/s.
        #[default]
        A = ("a", "A: 700 ksym/s"),
        /// 1050 ksym/s.
        B = ("b", "B: 1050 ksym/s"),
        /// 1400 ksym/s.
        Cd = ("cd", "C and D: 1400 ksym/s"),
    }
}

impl ListenProfile {
    /// Symbols a second.
    pub fn symbol_rate(self) -> f64 {
        match self {
            ListenProfile::A => 700_000.0,
            ListenProfile::B => 1_050_000.0,
            ListenProfile::Cd => 1_400_000.0,
        }
    }
}

const TX: &str = "Transmit";

/// The station callsign.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Station callsign", "", "")
    .section(TX)
    .help("1 to 7 characters of A-Z, 0-9 and /. Empty sends the demonstration callsign TEST1.");
/// The bytes before " de " in the identification text.
pub const STATION_TAG: Setting<field::Text> = Setting::new("station_tag", "Station tag", "", "")
    .section(TX)
    .help("Hex bytes that start the station identification text, before \" de \" and the callsign. The bytes that other \
           stations of this signal send are a row of data/custom/custom_tags.csv.");
/// The transmit profile.
pub const PROFILE: Setting<field::Choice<TxProfile>> = Setting::choice("profile", "Profile", TxProfile::A).section(TX);
/// The FEC profile.
pub const FEC: Setting<field::I64> = Setting::new("fec", "FEC profile", "", 1)
    .range(0.0, 7.0, 1.0)
    .section(TX)
    .help("0: 4 data cells; 1: 14 + 2; 2: 12 + 4; 3: 28 + 4; 4: 24 + 8; 5: 6 + 2; 6: 56 + 8; 7: 48 + 16.");
/// The file that `send` transmits.
pub const FILE: Setting<field::Text> = Setting::new("file", "File", "", "")
    .section(TX)
    .help("A file sent as Data packets of stream 1. Empty sends the synthetic test stream.");
/// Seconds of the synthetic stream.
pub const MEDIA_S: Setting<field::F64> =
    Setting::new("media_s", "Synthetic stream", "s", 5.0).range(1.0, 600.0, 0.0).section(TX);
/// The listen profile.
pub const LISTEN: Setting<field::Choice<ListenProfile>> =
    Setting::choice("listen_profile", "Listen profile", ListenProfile::A);

sdrkit_core::settings! {
    /// The transmitter settings.
    pub enum TxSetting {
        /// Callsign.
        Callsign = CALLSIGN,
        /// Tag.
        Tag = STATION_TAG,
        /// Profile.
        Profile = PROFILE,
        /// FEC.
        Fec = FEC,
        /// File.
        File = FILE,
        /// Media seconds.
        Media = MEDIA_S,
    }
}

sdrkit_core::settings! {
    /// The receiver settings.
    pub enum RxSetting {
        /// Listen profile.
        Listen = LISTEN,
    }
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Schema::of(&[RxSetting::SETTINGS, TxSetting::SETTINGS].concat())))
        .clone()
}

fn kind_name(k: u8) -> &'static str {
    ["video", "audio", "text", "picture", "data"].get(k as usize).copied().unwrap_or("other")
}

/// The search offsets of the coarse mixer, in steps of 0.04 Rs.
const STEPS: [f64; 7] = [0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0];

/// The `qam_video.rx` decoder.
pub struct QvDecoder {
    listen: ListenProfile,
    rate: f64,
    centre: f64,
    rs: Option<Resampler<Complex32>>,
    mixed: Vec<Complex32>,
    buf: Vec<Complex32>,
    rx: FrameReceiver,
    unpack: Vec<(usize, Unpacker)>,
    frames: Vec<Frame>,
    packets: Vec<Packet>,
    // Coarse search.
    base_hz: f64,
    step: usize,
    since_step: f64,
    since_header: f64,
    searching: bool,
    n: u64,
    nco: Option<Nco>,
    now: f64,
    // Counters.
    frames_ok: u64,
    frames_bad: u64,
    blocks_bad: u64,
    packets_n: u64,
    last: Option<Frame>,
    callsign: Option<String>,
    streams: std::collections::BTreeMap<(u8, u8), u64>,
    cleared: bool,
    started: bool,
}

impl Default for QvDecoder {
    fn default() -> Self {
        QvDecoder {
            listen: ListenProfile::A,
            rate: 0.0,
            centre: 0.0,
            rs: None,
            mixed: Vec::new(),
            buf: Vec::new(),
            rx: FrameReceiver::new(700_000.0),
            unpack: Vec::new(),
            frames: Vec::new(),
            packets: Vec::new(),
            base_hz: 0.0,
            step: 0,
            since_step: 0.0,
            since_header: f64::INFINITY,
            searching: true,
            n: 0,
            nco: None,
            now: 0.0,
            frames_ok: 0,
            frames_bad: 0,
            blocks_bad: 0,
            packets_n: 0,
            last: None,
            callsign: None,
            streams: std::collections::BTreeMap::new(),
            cleared: false,
            started: false,
        }
    }
}

impl QvDecoder {
    fn build(&mut self) -> Result<(), ConfigError> {
        let rs = self.listen.symbol_rate();
        self.rs = Some(Resampler::new(self.rate, 4.0 * rs).map_err(|e| ConfigError::Stage(StageError::new(e)))?);
        self.rx = FrameReceiver::new(rs);
        self.searching = true;
        self.step = 0;
        self.base_hz = 0.0;
        Ok(())
    }

    fn offset_hz(&self) -> f64 {
        let s = if self.searching { STEPS[self.step] } else { 0.0 };
        self.base_hz + s * 0.04 * self.listen.symbol_rate()
    }

    fn take(&mut self, f: Frame) {
        let good = f.cells.iter().filter(|c| c.is_some()).count();
        if good == 4 {
            self.frames_ok += 1;
        } else {
            self.frames_bad += 1;
        }
        self.blocks_bad += (4 - good) as u64;
        let cell = f.header.waveform.cell();
        let i = match self.unpack.iter().position(|u| u.0 == cell) {
            Some(i) => i,
            None => {
                self.unpack.push((cell, Unpacker::new(cell)));
                self.unpack.len() - 1
            }
        };
        self.unpack[i].1.frame(f.header.seq, usize::from(f.header.fec), &f.cells, &mut self.packets);
        self.since_header = 0.0;
        self.last = Some(f);
    }

    /// Counts the packets restored so far, reads the station identification, and sends them out.
    fn emit(&mut self, out: &mut Outbox) {
        for p in self.packets.drain(..) {
            self.packets_n += 1;
            *self.streams.entry((p.kind, p.stream)).or_insert(0) += 1;
            if p.kind == TEXT && p.stream == 0 {
                if let Some(c) = ident_callsign(&p.payload) {
                    self.callsign = Some(c);
                }
            }
            out.bytes(0).push(sdrkit_core::Packet::new(Arc::from(p.to_bytes()), self.now));
        }
    }
}

impl Decoder for QvDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "qam_video.rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::bytes("packets")]
    }
    fn settings(&self) -> Arc<Schema> {
        RxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([(RxSetting::Listen, Value::Choice(self.listen.index()))])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RxSetting::Listen => {
                self.listen = LISTEN.read(id, v)?;
                if self.rate > 0.0 {
                    let _ = self.build();
                }
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::new(QamVideoFields::SHAPE.fields).with_tables(&[QAM_VIDEO_STREAMS.def()])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.centre = input.centre_hz;
        self.nco = None;
        self.build()?;
        Ok(vec![Format::new(Kind::Bytes, 0.0)])
    }
    fn retune(&mut self, input: &Format) {
        self.centre = input.centre_hz;
        let _ = self.build();
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        let dt = x.len() as f64 / self.rate.max(1.0);
        self.now = (first + x.len() as u64) as f64 / self.rate.max(1.0);
        // Mix by the coarse offset, in pieces of the search step.
        let f = self.offset_hz();
        let rate = self.rate.max(1.0);
        let nco = self.nco.get_or_insert_with(|| Nco::new(rate, -f));
        if nco.freq() != -f {
            nco.set_freq(-f);
        }
        self.mixed.resize(x.len(), Complex32::new(0.0, 0.0));
        nco.mix(x, &mut self.mixed);
        self.n += x.len() as u64;
        self.buf.clear();
        if let Some(rs) = self.rs.as_mut() {
            rs.process(&self.mixed, &mut self.buf);
        }
        self.frames.clear();
        let headers = self.rx.frames;
        self.rx.process(&self.buf, &mut self.frames);
        if self.rx.frames != headers {
            // A header passed: the search stops at once on the offset that found it.
            self.base_hz = self.offset_hz();
            self.searching = false;
            self.since_header = 0.0;
        }
        let frames = std::mem::take(&mut self.frames);
        for fr in frames {
            self.take(fr);
        }
        self.since_header += dt;
        if self.searching {
            self.since_step += dt;
            if self.since_step >= 0.1 {
                self.since_step = 0.0;
                self.step = (self.step + 1) % STEPS.len();
            }
        } else if self.since_header > 0.5 {
            self.searching = true;
            self.step = 0;
        }
        self.emit(out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        self.frames.clear();
        self.rx.finish(&mut self.frames);
        for fr in std::mem::take(&mut self.frames) {
            self.take(fr);
        }
        for (_, u) in &mut self.unpack {
            u.finish(&mut self.packets);
        }
        self.emit(out);
    }
    fn report(&mut self, p: &mut Published) {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(&QAM_VIDEO_STREAMS);
        }
        let locked = self.since_header < 2.0;
        let (restored, lost) = self
            .unpack
            .iter()
            .fold((0, 0), |a, u| (a.0 + u.1.counts.restored, a.1 + u.1.counts.lost));
        let profile = self.last.as_ref().map(|f| {
            let m = f.header.waveform;
            format!("{}{}", m.profile.letter(), if m.robust { " robust" } else { "" })
        });
        p.put(&QamVideoFields {
            locked,
            profile: profile.as_deref(),
            esn0_db: self.last.as_ref().map(|f| f.esn0_db),
            offset_hz: self.last.as_ref().map(|f| Hertz(self.base_hz + f.carrier * self.listen.symbol_rate())),
            frames_ok: self.frames_ok as i64,
            frames_bad: self.frames_bad as i64,
            blocks_bad: self.blocks_bad as i64,
            groups_restored: restored as i64,
            groups_lost: lost as i64,
            packets: self.packets_n as i64,
            callsign: self.callsign.as_deref(),
            symbol_rate_hz: Hertz(self.listen.symbol_rate()),
            note: NOTE,
        });
        for ((k, s), n) in &self.streams {
            QAM_VIDEO_STREAMS.put(
                p,
                &format!("{k}.{s}"),
                &QamVideoStreamRow { stream: &s.to_string(), r#type: kind_name(*k), packets: *n as i64 },
            );
        }
        let lock = if locked {
            LockState::Locked
        } else if self.started {
            LockState::Searching
        } else {
            LockState::Off
        };
        let summary = if locked {
            format!("{} frames, {} packets", self.frames_ok, self.packets_n)
        } else {
            "Searching for frames".to_string()
        };
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock || s.summary != summary;
            s.lock_state = lock;
            s.summary.clone_from(&summary);
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn clear(&mut self) {
        self.frames_ok = 0;
        self.frames_bad = 0;
        self.blocks_bad = 0;
        self.packets_n = 0;
        self.streams.clear();
        self.cleared = true;
    }
    fn forget(&mut self, _kept: &Kept) {
        self.clear();
        self.frames.clear();
        self.packets.clear();
        self.unpack.clear();
        self.last = None;
        self.callsign = None;
        self.since_header = f64::INFINITY;
    }
}

/// Why `send` was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit switch is off.
    Off,
    /// A transmission is under way.
    Busy,
    /// The output rate is below 1.6 times the symbol rate.
    Rate,
    /// The station tag is not hex bytes.
    Tag,
    /// The file cannot be read.
    File,
    /// The session cannot be made.
    Session(String),
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::Busy => f.write_str("a transmission is under way"),
            TxError::Rate => f.write_str("the transmit rate must be at least 1.6 times the symbol rate"),
            TxError::Tag => f.write_str("the station tag must be hex bytes"),
            TxError::File => f.write_str("the file cannot be read"),
            TxError::Session(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for TxError {}

/// Starts a transmission.
pub const SEND: Command = Command::new("send", "Send")
    .help("Sends the file (or the synthetic test stream) once with the station's settings. Transmit must be on.")
    .transmits(sdrkit_core::TxContent::File)
    .stopped_by("stop");
/// Ends the transmission.
pub const STOP: Command = Command::new("stop", "Stop").help("Ends the transmission at once.");

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Send.
        Send = SEND,
        /// Stop.
        Stop = STOP,
    }
}

/// The sender (`qam_video.tx`).
pub struct QvSender {
    callsign: String,
    tag: String,
    profile: TxProfile,
    fec: i64,
    file: String,
    media_s: f64,
    rate: f64,
    transmit: bool,
    pending: Vec<Complex32>,
    at: usize,
    events: Vec<sdrkit_core::Event>,
}

impl Default for QvSender {
    fn default() -> Self {
        QvSender {
            callsign: String::new(),
            tag: String::new(),
            profile: TxProfile::A,
            fec: 1,
            file: String::new(),
            media_s: 5.0,
            rate: 0.0,
            transmit: false,
            pending: Vec::new(),
            at: 0,
            events: Vec::new(),
        }
    }
}

/// The session of the transmit settings.
pub fn tx_session(call: &str, tag: &str, profile: Profile, fec: usize, source: Source) -> Result<Session, TxError> {
    Ok(Session {
        waveform: Waveform {
            profile,
            robust: false,
        },
        fec,
        callsign: if call.trim().is_empty() { DEMO_CALLSIGN.to_string() } else { call.to_string() },
        tag: hex_bytes(tag).ok_or(TxError::Tag)?,
        source,
    })
}

impl QvSender {
    fn event(&mut self, state: &str, reason: &str) {
        self.events.push(QAM_VIDEO_TX.event(0.0, 0.0, &TxEvent { state, reason }));
    }

    /// Starts a transmission.
    pub fn send(&mut self) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.at < self.pending.len() {
            return Err(TxError::Busy);
        }
        let p = self.profile.profile();
        if self.rate < 1.6 * p.symbol_rate() {
            return Err(TxError::Rate);
        }
        let source = if self.file.is_empty() {
            Source::Synthetic {
                seconds: self.media_s,
                video_bps: 1_200_000.0,
            }
        } else {
            Source::File(std::fs::read(&self.file).map_err(|_| TxError::File)?)
        };
        let s = tx_session(&self.callsign, &self.tag, p, self.fec as usize, source)?;
        let (x, _) = session_samples(&s).map_err(|e| TxError::Session(e.to_string()))?;
        let mut y = Vec::with_capacity((x.len() as f64 * self.rate / (4.0 * p.symbol_rate())) as usize + 64);
        Resampler::new(4.0 * p.symbol_rate(), self.rate).map_err(|_| TxError::Rate)?.process(&x, &mut y);
        for v in &mut y {
            *v *= 0.25;
        }
        self.pending = y;
        self.at = 0;
        Ok(())
    }
}

impl Component for QvSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "qam_video.tx",
            vec![Port::iq("in")],
            vec![Port::iq("out"), Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        let dial = if f.tuned_hz > 0.0 { f.tuned_hz } else { f.centre_hz };
        Ok(vec![Format::iq(self.rate, dial, dial), Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        if !ctx.transmit && self.at < self.pending.len() {
            self.pending.clear();
            self.at = 0;
            self.event("finished", "the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        if self.at < self.pending.len() {
            let n = room.min(self.pending.len() - self.at);
            let chunk = self.pending[self.at..self.at + n].to_vec();
            io.iq_out(0).extend(chunk);
            self.at += n;
            if self.at >= self.pending.len() {
                if let Some(last) = io.iq_out(0).len().checked_sub(1) {
                    io.output(0).tag_here(last, TagKind::BurstEnd);
                }
                self.pending.clear();
                self.at = 0;
                self.event("finished", "");
            }
        }
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Callsign => self.callsign = CALLSIGN.read(id, v)?.to_owned(),
            TxSetting::Tag => self.tag = STATION_TAG.read(id, v)?.to_owned(),
            TxSetting::Profile => self.profile = PROFILE.read(id, v)?,
            TxSetting::Fec => self.fec = FEC.read(id, v)?.clamp(0, 7),
            TxSetting::File => self.file = FILE.read(id, v)?.to_owned(),
            TxSetting::Media => self.media_s = MEDIA_S.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (TxSetting::Callsign, Value::Text(self.callsign.clone())),
            (TxSetting::Tag, Value::Text(self.tag.clone())),
            (TxSetting::Profile, Value::Choice(self.profile.index())),
            (TxSetting::Fec, Value::I64(self.fec)),
            (TxSetting::File, Value::Text(self.file.clone())),
            (TxSetting::Media, Value::F64(self.media_s)),
        ])
    }
    fn commands(&self) -> &Commands {
        TxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match TxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TxCommand::Send => match self.send() {
                Ok(()) => self.event("started", ""),
                Err(e) => {
                    self.event("refused", &e.to_string());
                    return Err(CommandError::Failed {
                        source: StageError::new(e),
                    });
                }
            },
            TxCommand::Stop => {
                if self.at < self.pending.len() {
                    self.pending.clear();
                    self.at = 0;
                    self.event("finished", "stopped");
                }
            }
        }
        Ok(None)
    }
}

/// The station tag of the test signal (an artificial value).
pub const TEST_TAG: &str = "54455354";

/// The test signal: profile A, FEC 1, the synthetic stream in 3 s loops with 0.2 s of silence between, the station TEST1, at
/// unit power.
pub struct QvGenerator {
    rate: f64,
    loop_samples: Vec<Complex32>,
    at: usize,
}

impl Generator for QvGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(875_000.0).with_symbol_rate(700_000.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            let v = self.loop_samples[self.at];
            self.at = (self.at + 1) % self.loop_samples.len();
            *o = v;
        }
        out.len()
    }
}

/// The generator of the stack: the synthetic stream of 3 s at the transmit profile setting, looped at the radio rate.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let profile = cfg.settings.get(&PROFILE).profile();
    let s = tx_session(
        DEMO_CALLSIGN,
        TEST_TAG,
        profile,
        1,
        Source::Synthetic {
            seconds: 3.0,
            video_bps: 1_200_000.0,
        },
    )
    .map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    let (mut x, _) = session_samples(&s).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    let fs = 4.0 * profile.symbol_rate();
    x.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), (0.2 * fs) as usize));
    let mut y = Vec::new();
    Resampler::new(fs, cfg.rate_hz)
        .map_err(|e| ConfigError::Stage(StageError::new(e)))?
        .process(&x, &mut y);
    if y.is_empty() {
        y.push(Complex32::new(0.0, 0.0));
    }
    Ok(Box::new(QvGenerator {
        rate: cfg.rate_hz,
        loop_samples: y,
        at: 0,
    }))
}

/// The receiver and transmitter graph.
///
/// # Errors
///
/// The errors of the graph and of a refused setting.
pub fn graph(cfg: &StackConfig) -> Result<sdrkit_graph::StackGraph, sdrkit_graph::GraphError> {
    use sdrkit_graph::{Catalog, CommandTarget, GraphBuilder, StackGraph};
    let b = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("rx", DecoderComponent::new(QvDecoder::default()))
        .add("sender", QvSender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("rx")
        .input("sender.in")
        .connect("sender.out", "tx.in")
        .output("rx.packets")
        .output("sender.events")
        .status_from("rx")
        .clock(0.25);
    let mut g = StackGraph::of(crate::typed::shape(ID), b.build()?)
        .min_rate(MIN_RATE, "qam_video needs a sample rate of at least 2 MHz")
        .route(LISTEN.key(), &["rx.listen_profile"])?;
    for s in TxSetting::SETTINGS {
        let k = s.key();
        g = g.route(k, &[&format!("sender.{k}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("send", CommandTarget::of("sender", "send"))
        .command("stop", CommandTarget::of("sender", "stop"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    Ok(Box::new(graph(cfg)?))
}

/// The stack.
pub static QAM_VIDEO: StackDescriptor = StackDescriptor {
    id: ID,
    name: "QAM video link",
    group: crate::GROUP,
    blurb: "A single-carrier 16-QAM or 64-QAM link at 700 to 1400 ksymbols a second with LDPC and erasure-coded groups: \
            video, audio, text and data packets, and the station's callsign. Receive and transmit.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 100e6,
        max_hz: 6000e6,
        ..Tuning::anywhere(1290e6, 2.4e6, MIN_RATE, 2.0e6)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::QAM_VIDEO_VERIFY),
    benchmarks: None,
};
