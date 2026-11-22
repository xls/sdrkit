//! The stack's components with typed ports, and the registry of their kinds (`wsjt.*`).

use crate::bands;
use crate::encode::{self, Transmission};
use crate::front::{Demod, FrontEnd, FrontLevels, AUDIO_RATE};
use crate::mode::{Mode, Sub};
use crate::proto::js8msg::{command_code, Outgoing};
use crate::proto::msg77::PackError;
use crate::qso::{Beacon, Js8Sender, QsoSettings, Sequencer, TxMessage};
use crate::runner::{DecodeState, PoolRunner, SlotDecoder, SlotQueue, SlotResult};
use crate::rx::js8::Js8Frame;
use crate::rx::slot::{Decode, Slot};
use crate::slots::{ClockMap, Cutter, SlotClock, TimeChoice, TimeSource, WallClock};
use crate::table::{ClockReport, Display, Js8Message};
use crate::typed::{HopFields, MultibandRow, SendArgs, TxEvent, WideFields, MONITOR, SEND, TX};
use num_complex::Complex32;
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, ConfigError, EventDef, Record, Results, Setting, SettingError, SettingId, SettingSet,
    StationItem, Value, Values,
};
use sdrkit_graph::components::{SpectrumSize, SpectrumWindow};
use sdrkit_graph::{
    values_of, Applied, ChannelsFrom, Component, ComponentInfo, DecoderComponent, Format, Io, Item,
    Kind, Port, RateFrom, Registry, RunContext, SpectrumSpec, TagKind, Timed,
};
use sdrkit_util_objects::Tables;

impl Item for Decode {
    const KIND: &'static str = "wsjt.decode";
}
impl Item for Js8Frame {
    const KIND: &'static str = "js8.frame";
}
impl Item for Js8Message {
    const KIND: &'static str = "js8.message";
}
impl Item for ClockReport {
    const KIND: &'static str = "wsjt.clock";
}
impl Item for Slot {
    const KIND: &'static str = "wsjt.slot";
}
impl Item for ClockMap {
    const KIND: &'static str = "wsjt.time";
}
impl Item for SlotResult {
    const KIND: &'static str = "wsjt.slot_result";
}
impl Item for FrontLevels {
    const KIND: &'static str = "wsjt.levels";
}

impl Item for TxMessage {
    const KIND: &'static str = "wsjt.tx";
    fn event_def() -> Option<&'static EventDef> {
        Some(TX.def())
    }
    fn write_fields(&self, r: &mut Record) {
        TxEvent {
            mode: self.mode,
            text: &self.text,
            audio_hz: Hertz(self.hz),
            slot: Time(self.slot),
            period: Seconds(self.period),
            family: Family::Message,
        }
        .write(r);
    }
}

/// The spectrum the JS8 waterfall reads from the audio: 2048 bins with a Hann window, four lines a second.
pub const WATERFALL: SpectrumSpec = SpectrumSpec {
    size: sdrkit_graph::SizeRule::Bins(SpectrumSize::S2048),
    window: SpectrumWindow::Hann,
    average: 1,
    frames: sdrkit_graph::FrameRule::PerSecond(4.0),
    scale: sdrkit_graph::components::SpectrumScale::Db,
};

/// The format of a typed output: no rate, the input's centre and tuned frequency.
fn typed_from<T: Item>(from: &Format) -> Format {
    Format {
        centre_hz: from.centre_hz,
        tuned_hz: from.tuned_hz,
        ..Format::new(Kind::typed::<T>(), 0.0)
    }
}

/// A typed output port.
fn typed_out<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

// The front end.

pub(crate) const DEMOD: Setting<field::Choice<Demod>> =
    Setting::choice("demod", "Demodulator", Demod::Usb);
pub(crate) const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", false)
    .help("Copies the 8 kHz audio to the speaker. Decoding is the same either way.");

sdrkit_core::settings! {
    /// The settings of [`FrontComponent`].
    pub enum FrontSetting {
        /// The demodulator.
        Demod = DEMOD,
        /// Listen.
        Listen = LISTEN,
    }
}

/// The front end (`wsjt.front`): the radio's samples in, the 8 kHz audio of the decoders, mono audio to listen to and the
/// levels out.
pub struct FrontComponent {
    fe: Option<FrontEnd>,
    plain: bool,
    demod: Demod,
    listen: bool,
    audio: Vec<f32>,
}

impl Default for FrontComponent {
    fn default() -> Self {
        FrontComponent {
            fe: None,
            plain: false,
            demod: DEMOD.default,
            listen: LISTEN.default,
            audio: Vec::new(),
        }
    }
}

impl FrontComponent {
    /// The front end without the slow AGC ([`FrontEnd::plain`]): the audio of the burst and ping receivers.
    pub fn plain() -> Self {
        FrontComponent {
            plain: true,
            ..Self::default()
        }
    }

    fn front(&self, rate: f64, offset_hz: f64, demod: Demod) -> Result<FrontEnd, ConfigError> {
        if self.plain {
            FrontEnd::plain(rate, offset_hz, demod)
        } else {
            FrontEnd::new(rate, offset_hz, demod)
        }
    }
}

impl Component for FrontComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wsjt.front",
            vec![Port::iq("in")],
            vec![
                Port::real("audio").rate_from(RateFrom::Fixed(AUDIO_RATE)),
                Port::audio("listen")
                    .rate_from(RateFrom::Fixed(AUDIO_RATE))
                    .channels_from(ChannelsFrom::Fixed(1)),
                typed_out::<FrontLevels>("levels"),
            ],
        )
        .with_settings(FrontSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.fe = Some(self.front(f.rate, f.signal_offset_hz(), self.demod)?);
        let at_audio = |kind| Format {
            centre_hz: f.centre_hz,
            tuned_hz: f.tuned_hz,
            ..Format::new(kind, AUDIO_RATE)
        };
        Ok(vec![
            Format {
                bandwidth_hz: 3800.0,
                ..at_audio(Kind::Real)
            },
            at_audio(Kind::Audio),
            typed_from::<FrontLevels>(f),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let iq = io.iq(0);
        self.audio.clear();
        if let Some(fe) = self.fe.as_mut() {
            fe.process(iq, &mut self.audio);
        }
        io.real_out(0).extend_from_slice(&self.audio);
        if self.listen {
            io.audio_out(1)
                .samples
                .extend(self.audio.iter().map(|&x| x * 0.25));
        }
        if io.tick() {
            if let Some(fe) = self.fe.as_mut() {
                let l = fe.take_levels();
                io.items_out::<FrontLevels>(2).push(Timed {
                    time: 0.0,
                    value: l,
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::Demod => {
                let d = DEMOD.read(id, v)?;
                if d != self.demod {
                    if let Some(fe) = self.fe.as_ref() {
                        let fe = self.front(fe.rate(), fe.offset_hz(), d).map_err(|e| {
                            SettingError::Config {
                                id,
                                source: Box::new(e),
                            }
                        })?;
                        self.fe = Some(fe);
                    }
                    self.demod = d;
                }
            }
            FrontSetting::Listen => self.listen = LISTEN.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (FrontSetting::Demod, Value::Choice(self.demod.index())),
            (FrontSetting::Listen, Value::Bool(self.listen)),
        ])
    }
}

// The slot clock.

pub(crate) const TIME_SOURCE: Setting<field::Choice<TimeChoice>> =
    Setting::choice("source", "Slot clock", TimeChoice::System).restart();
pub(crate) const TIME_START: Setting<field::Text> =
    Setting::new("start", "Recording start (UTC)", "", "")
        .restart()
        .help("An ISO 8601 time such as 2026-01-01T00:00:00Z. Empty: none given.");
/// The input that is not live (a test signal) is in step with the system clock: its first sample is now.
pub(crate) const TIME_ON_CLOCK: Setting<field::Bool> =
    Setting::new("on_clock", "Input in step with the clock", "", false);

sdrkit_core::settings! {
    /// The settings of [`ClockComponent`].
    pub enum ClockSetting {
        /// The time source.
        Source = TIME_SOURCE,
        /// The start of a recording.
        Start = TIME_START,
        /// An input that is not live follows the system clock.
        OnClock = TIME_ON_CLOCK,
    }
}

/// The slot clock (`wsjt.slot_clock`): the audio in, the mapping of audio samples to UTC out. The wall clock is the host's
/// ([`RunContext::clock`]). The first sample is anchored once; after it, time is the sample count, and only a live input
/// with the system clock follows the wall clock.
pub struct ClockComponent {
    clock: SlotClock,
    choice: TimeChoice,
    start: f64,
    start_text: String,
    wall: WallClock,
    live: bool,
    on_clock: bool,
}

impl Default for ClockComponent {
    fn default() -> Self {
        ClockComponent {
            clock: SlotClock::new(
                TimeChoice::System,
                0.0,
                AUDIO_RATE,
                crate::slots::system_clock(),
            ),
            choice: TimeChoice::System,
            start: 0.0,
            start_text: String::new(),
            wall: crate::slots::system_clock(),
            live: true,
            on_clock: false,
        }
    }
}

impl ClockComponent {
    /// The time source in use for the chosen one: on an input that is not live, the system clock means the given start,
    /// else the system clock when the input is in step with it, else slot search.
    fn effective(&self) -> TimeChoice {
        match self.choice {
            TimeChoice::System if !self.live && !self.start_text.is_empty() => TimeChoice::Start,
            TimeChoice::System if !self.live && !self.on_clock => TimeChoice::Search,
            c => c,
        }
    }

    /// The time source in use and its start: slot search without a given start uses a grid near the wall clock, so the
    /// times read as today until the slots are found.
    fn source(&self) -> (TimeChoice, f64) {
        let choice = self.effective();
        if choice == TimeChoice::Search && self.start_text.is_empty() {
            (choice, self.wall.now())
        } else {
            (choice, self.start)
        }
    }

    /// A new clock from the settings; it anchors at the next sample.
    fn remake(&mut self) {
        let (choice, start) = self.source();
        self.clock = SlotClock::new(choice, start, AUDIO_RATE, self.wall.clone());
        self.clock.set_follow(self.live);
    }
}

impl Component for ClockComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wsjt.slot_clock",
            vec![Port::real("in"), Port::typed::<ClockReport>("clock")],
            vec![typed_out::<ClockMap>("time")],
        )
        .with_settings(ClockSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.wall = ctx.clock.clone();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.remake();
        Ok(vec![typed_from::<ClockMap>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(c) = io.items::<ClockReport>(1).last() {
            if let Some(s) = c.value.correction_s {
                if self.clock.source() == TimeSource::Searching {
                    self.clock.found(s);
                }
            }
        }
        let first = io.input(0).first();
        let n = io.input_len(0);
        if n > 0 {
            self.clock.advance(first, n);
        }
        if let Some(m) = self.clock.map() {
            io.items_out::<ClockMap>(0).push(Timed {
                time: 0.0,
                value: m,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ClockSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ClockSetting::Source => {
                self.choice = TIME_SOURCE.read(id, v)?;
            }
            ClockSetting::Start => {
                let t = TIME_START.read(id, v)?;
                if t.is_empty() {
                    self.start_text.clear();
                    return Ok(Applied::Live);
                }
                self.start = sdrkit_util_geo::time::parse_iso8601_f64(t)
                    .map_err(|_| SettingError::Malformed { id, at: None })?;
                t.clone_into(&mut self.start_text);
            }
            ClockSetting::OnClock => {
                let on = TIME_ON_CLOCK.read(id, v)?;
                // A test signal made again in step with the clock starts now: anchor again, under a new epoch so the
                // cutters start again.
                if on != self.on_clock {
                    self.on_clock = on;
                    let (choice, start) = self.source();
                    self.clock.set_choice(choice, start);
                    self.clock.reset();
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ClockSetting::Source, Value::Choice(self.choice.index())),
            (ClockSetting::Start, Value::Text(self.start_text.clone())),
            (ClockSetting::OnClock, Value::Bool(self.on_clock)),
        ])
    }
}

// The slot cutter.

sdrkit_core::choice_enum! {
    /// A sub-mode letter.
    pub enum Letter {
        /// A.
        A = ("A", "A"),
        /// B.
        B = ("B", "B"),
        /// C.
        C = ("C", "C"),
        /// D.
        D = ("D", "D"),
        /// E.
        E = ("E", "E"),
        /// F.
        F = ("F", "F"),
        /// G.
        G = ("G", "G"),
    }
}

pub(crate) const CUT_MODE: Setting<field::Choice<Mode>> =
    Setting::choice("mode", "Mode", Mode::Ft8).restart();
pub(crate) const ENABLE: Setting<field::Bool> =
    Setting::new("enable", "Decode", "", true).help("Takes effect at the next slot.");
pub(crate) const JS8_NORMAL: Setting<field::Bool> =
    Setting::new("normal", "JS8 Normal (15 s)", "", true);
pub(crate) const JS8_FAST: Setting<field::Bool> = Setting::new("fast", "JS8 Fast (10 s)", "", true);
pub(crate) const JS8_TURBO: Setting<field::Bool> =
    Setting::new("turbo", "JS8 Turbo (6 s)", "", true);
pub(crate) const SUBMODE: Setting<field::Choice<Letter>> =
    Setting::choice("submode", "Sub-mode", Letter::A)
        .help("A letter of the stack's sub-modes. JS8 has none.");
pub(crate) const PERIOD: Setting<field::Duration> = Setting::new("period", "Period", "s", 15.0)
    .range(3.75, 1800.0, 0.0)
    .help("One of the T/R periods of the stack. JS8 has none.");
pub(crate) const EARLY_DECODE: Setting<field::Bool> =
    Setting::new("early_decode", "Early decode", "", true).help(
        "FT8 and FT4: decode the slot once more when 85 % of the transmission is in, while the decoder is idle. Early decodes \
         are shown at once and replaced by the decodes of the whole slot; they are not counted.",
    );

sdrkit_core::settings! {
    /// The settings of [`CutterComponent`].
    pub enum CutterSetting {
        /// The mode.
        Mode = CUT_MODE,
        /// Decode.
        Enable = ENABLE,
        /// JS8 Normal.
        Normal = JS8_NORMAL,
        /// JS8 Fast.
        Fast = JS8_FAST,
        /// JS8 Turbo.
        Turbo = JS8_TURBO,
        /// The sub-mode letter.
        Submode = SUBMODE,
        /// The T/R period.
        Period = PERIOD,
        /// Early decode.
        EarlyDecode = EARLY_DECODE,
    }
}

/// The slot cutter of one mode (`wsjt.cutter`). The JS8 cutter cuts the slots of Normal, Fast and Turbo into one stream. With a
/// [`SlotQueue`] shared with the decoder, it numbers the slots it cuts and makes an early cut only while the decoder is idle;
/// without one, early cuts are made whenever they are due.
pub struct CutterComponent {
    cuts: Vec<Cutter>,
    mode: Mode,
    sub: u8,
    period: f64,
    enabled: bool,
    js8: [bool; 3],
    early: bool,
    queue: Option<std::sync::Arc<SlotQueue>>,
    map: Option<ClockMap>,
    slots: Vec<Slot>,
}

impl CutterComponent {
    /// A cutter for `sub` (for JS8: every sub-mode).
    pub fn new(sub: Sub) -> Self {
        let cuts = if sub.mode == Mode::Js8 {
            (0..3)
                .map(|i| Cutter::new(Sub::new(Mode::Js8).with_sub(i), AUDIO_RATE))
                .collect()
        } else {
            vec![Cutter::new(sub, AUDIO_RATE)]
        };
        let mut c = CutterComponent {
            cuts,
            mode: sub.mode,
            sub: sub.sub,
            period: sub.period,
            enabled: true,
            js8: [true; 3],
            early: EARLY_DECODE.default,
            queue: None,
            map: None,
            slots: Vec::new(),
        };
        c.set_early(c.early);
        c
    }
    /// The same cutter, sharing `queue` with its decoder.
    pub fn with_queue(mut self, queue: std::sync::Arc<SlotQueue>) -> Self {
        self.queue = Some(queue);
        self
    }
    fn set_early(&mut self, on: bool) {
        self.early = on;
        for c in &mut self.cuts {
            c.set_early(on);
        }
    }
    fn set_sub(&mut self) {
        let s = Sub::new(self.mode)
            .with_sub(self.sub)
            .with_period(self.period);
        for c in &mut self.cuts {
            c.set_sub(s);
        }
    }
    fn cut(&mut self, io: &mut Io, flush: bool) {
        if let Some(m) = io.items::<ClockMap>(1).last() {
            self.map = Some(m.value);
        }
        let Some(map) = self.map else {
            return;
        };
        let eos = io
            .input(0)
            .tags()
            .iter()
            .any(|t| t.kind == TagKind::EndOfStream);
        let first = io.input(0).first();
        let audio = io.real(0);
        self.slots.clear();
        let idle = self.queue.as_ref().is_none_or(|q| q.idle());
        for c in &mut self.cuts {
            c.push(audio, first, &map, flush || eos, idle, &mut self.slots);
        }
        let out = io.items_out::<Slot>(0);
        for mut s in self.slots.drain(..) {
            if let Some(q) = &self.queue {
                s.seq = q.next();
            }
            out.push(Timed {
                time: 0.0,
                value: s,
            });
        }
    }
}

impl Default for CutterComponent {
    fn default() -> Self {
        Self::new(Sub::new(Mode::Ft8))
    }
}

impl Component for CutterComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wsjt.cutter",
            vec![Port::real("in"), Port::typed::<ClockMap>("time")],
            vec![typed_out::<Slot>("slots")],
        )
        .with_settings(CutterSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        for c in &mut self.cuts {
            c.reset();
            // The dial is where the user tuned (the suppressed carrier of the upper sideband).
            c.set_dial(inputs[0].tuned_hz);
        }
        Ok(vec![typed_from::<Slot>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        self.cut(io, false);
    }
    /// The end of the stream: the slots whose audio reached the middle of the transmission are cut too.
    fn finish(&mut self, io: &mut Io) {
        self.cut(io, true);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let js8 = self.mode == Mode::Js8;
        match CutterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CutterSetting::Mode => {
                let mode = CUT_MODE.read(id, v)?;
                let (enabled, early) = (self.enabled, self.early);
                let queue = self.queue.take();
                *self = Self::new(Sub::new(mode));
                self.queue = queue;
                self.enabled = enabled;
                for c in &mut self.cuts {
                    c.set_enabled(enabled);
                }
                self.set_early(early);
            }
            CutterSetting::EarlyDecode => {
                let on = EARLY_DECODE.read(id, v)?;
                self.set_early(on);
            }
            CutterSetting::Enable => {
                self.enabled = ENABLE.read(id, v)?;
                for c in &mut self.cuts {
                    c.set_enabled(self.enabled);
                }
            }
            s @ (CutterSetting::Normal | CutterSetting::Fast | CutterSetting::Turbo) => {
                if !js8 {
                    return Err(SettingError::NotPermitted { id });
                }
                let (i, on) = match s {
                    CutterSetting::Normal => (0, JS8_NORMAL.read(id, v)?),
                    CutterSetting::Fast => (1, JS8_FAST.read(id, v)?),
                    _ => (2, JS8_TURBO.read(id, v)?),
                };
                self.js8[i] = on;
                if let Some(c) = self.cuts.get_mut(i) {
                    c.set_enabled(on);
                }
            }
            CutterSetting::Submode => {
                let l = SUBMODE.read(id, v)?;
                if js8 || usize::from(l.index()) >= self.mode.submodes().len() {
                    return Err(SettingError::NotAChoice {
                        id,
                        index: l.index(),
                    });
                }
                let wanted = Sub {
                    mode: self.mode,
                    sub: l.index() as u8,
                    period: self.period,
                };
                if !wanted.defined() {
                    return Err(SettingError::NotPermitted { id });
                }
                self.sub = l.index() as u8;
                self.set_sub();
            }
            CutterSetting::Period => {
                let p = PERIOD.read(id, v)?;
                let wanted = Sub {
                    mode: self.mode,
                    sub: self.sub,
                    period: p,
                };
                if js8 || !self.mode.periods().contains(&p) || !wanted.defined() {
                    return Err(SettingError::NotPermitted { id });
                }
                self.period = p;
                self.set_sub();
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let sub = if self.mode == Mode::Js8 { 0 } else { self.sub };
        let period = if self.mode == Mode::Js8 {
            PERIOD.default
        } else {
            self.period
        };
        values_of([
            (CutterSetting::Mode, Value::Choice(self.mode.index())),
            (CutterSetting::Enable, Value::Bool(self.enabled)),
            (CutterSetting::Normal, Value::Bool(self.js8[0])),
            (CutterSetting::Fast, Value::Bool(self.js8[1])),
            (CutterSetting::Turbo, Value::Bool(self.js8[2])),
            (CutterSetting::Submode, Value::Choice(u16::from(sub))),
            (CutterSetting::Period, Value::Duration(period)),
            (CutterSetting::EarlyDecode, Value::Bool(self.early)),
        ])
    }
}

/// The decoder of `mode` as a component (kind `wsjt.decoder`), telling `queue` (shared with its cutter) which slots it
/// decoded.
pub fn decoder(mode: Mode, queue: Option<std::sync::Arc<SlotQueue>>) -> DecoderComponent<SlotDecoder> {
    let d = SlotDecoder::new(mode);
    DecoderComponent::new(match queue {
        Some(q) => d.with_queue(q),
        None => d,
    })
}

/// Merges the results of several decoders into one stream (`wsjt.merge`).
pub struct MergeComponent {
    n: usize,
}

impl MergeComponent {
    /// With `n` inputs.
    pub fn new(n: usize) -> Self {
        MergeComponent { n: n.max(1) }
    }
}

impl Default for MergeComponent {
    fn default() -> Self {
        Self::new(1)
    }
}

impl Component for MergeComponent {
    fn info(&self) -> ComponentInfo {
        let ins = (0..self.n)
            .map(|i| Port::typed::<SlotResult>(&format!("in{i}")))
            .collect();
        ComponentInfo::processor("wsjt.merge", ins, vec![typed_out::<SlotResult>("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_from::<SlotResult>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        for i in 0..self.n {
            let items = io.items::<SlotResult>(i);
            if !items.is_empty() {
                io.items_out::<SlotResult>(0).extend_from_slice(items);
            }
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

// The display.

pub(crate) const SPOTS: Setting<field::Bool> = Setting::new("spots", "Spot events", "", false)
    .help("Makes a spot event of every decode with a calling station, for the host to forward.");
pub(crate) const BAND_EDGES: Setting<field::Text> = Setting::new("band_edges", "Band edges", "", "")
    .help("The bands, in the order of the multi-band monitor: entries name=low_hz-high_hz separated by \";\", for example \"20m=14000000-14350000\". The monitor takes the first 11. The host fills it from a band file (data/wsjt/bands.csv). A dial belongs to a band from 3 kHz below its lower edge to its upper edge. Empty: no band.");
pub(crate) const DIALS: Setting<field::Text> = Setting::new("dials", "Dials", "", "")
    .help("The dials (upper sideband) of whole-span decoding, the multi-band monitor and band hopping: entries mode=frequency_hz separated by \";\", for example \"ft8=14074000\". The host fills it from a dial file (data/wsjt/dials.csv). Empty: no dial.");

/// The bands of the text of [`BAND_EDGES`], or the error of setting `id`.
fn band_edges(id: SettingId, text: &str) -> Result<Vec<bands::Band>, SettingError> {
    bands::parse_band_edges(text).ok_or(SettingError::Malformed { id, at: None })
}

/// The dials of the text of [`DIALS`], or the error of setting `id`.
fn dials(id: SettingId, text: &str) -> Result<Vec<(Mode, f64)>, SettingError> {
    bands::parse_dials(text).ok_or(SettingError::Malformed { id, at: None })
}

sdrkit_core::settings! {
    /// The settings of [`DisplayComponent`].
    pub enum DisplaySetting {
        /// Spot events.
        Spots = SPOTS,
        /// The band edges.
        BandEdges = BAND_EDGES,
    }
}

sdrkit_core::commands! {
    /// The commands of [`DisplayComponent`].
    pub enum DisplayCommand {
        /// Empties the decodes, counters, stations and the map.
        Clear = sdrkit_util_objects::commands::CLEAR,
        /// Forgets one station.
        Remove = crate::typed::REMOVE.command,
    }
}

/// The decode table, the clock estimator, JS8 assembly, the stations and what the receiver publishes (`wsjt.display`). Its
/// lists are saved and restored through the component's state (format `wsjt.heard`).
pub struct DisplayComponent {
    /// The state.
    pub display: Display,
    map: Option<ClockMap>,
    end_index: u64,
    level: Option<f32>,
    /// Hz between two bins of the waterfall input, 0 when it is not connected.
    wf_df: f64,
    /// The text of the band edges.
    band_edges: String,
}

impl Default for DisplayComponent {
    fn default() -> Self {
        Self::of(None)
    }
}

impl DisplayComponent {
    /// The display of the slot stack of `mode` (`None`: the combined stack).
    pub fn of(mode: Option<Mode>) -> Self {
        DisplayComponent {
            display: Display::of(mode),
            map: None,
            end_index: 0,
            level: None,
            wf_df: 0.0,
            band_edges: String::new(),
        }
    }
}

impl Component for DisplayComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wsjt.display",
            vec![
                Port::typed::<SlotResult>("slots"),
                Port::typed::<ClockMap>("time"),
                Port::typed::<FrontLevels>("levels"),
                Port::spectrum("waterfall", WATERFALL).optional("no waterfall"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                typed_out::<ClockReport>("clock"),
            ],
        )
        .with_settings(DisplaySetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let wf = &inputs[3];
        self.wf_df = if wf.frame_len > 0 {
            wf.bandwidth_hz / wf.frame_len as f64
        } else {
            0.0
        };
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            typed_from::<ClockReport>(&inputs[0]),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(m) = io.items::<ClockMap>(1).last() {
            self.map = Some(m.value);
            self.end_index = m.value.end;
        }
        if let Some(l) = io.items::<FrontLevels>(2).last() {
            self.level = Some(l.value.level_dbfs);
        }
        if self.wf_df > 0.0 {
            let frames = io.spectrum(3);
            let n = frames.len();
            if n > 0 {
                let bins = frames.bins;
                let line = &frames.values[(n - 1) * bins..n * bins];
                self.display.waterfall_line(line, self.wf_df);
            }
        }
        let results = io.items::<SlotResult>(0);
        let events = io.messages_out(0);
        for r in results {
            self.display.slot(&r.value, events);
        }
        if io.tick() || !results.is_empty() {
            let (utc, source) = match self.map {
                Some(m) => (m.utc_of(self.end_index.max(m.at), AUDIO_RATE), m.source),
                None => (0.0, TimeSource::SystemClock),
            };
            self.display.refresh(utc, source, self.level);
            let rep = self.display.clock.report();
            let corr = self.display.clock.take_correction();
            io.items_out::<ClockReport>(1).push(Timed {
                time: 0.0,
                value: ClockReport {
                    correction_s: corr,
                    ..rep
                },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DisplaySetting::Spots => self.display.spots = SPOTS.read(id, v)?,
            DisplaySetting::BandEdges => {
                let t = BAND_EDGES.read(id, v)?;
                self.display.bands.bands = band_edges(id, t)?;
                t.clone_into(&mut self.band_edges);
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DisplaySetting::Spots, Value::Bool(self.display.spots)),
            (
                DisplaySetting::BandEdges,
                Value::Text(self.band_edges.clone()),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        DisplayCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match DisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DisplayCommand::Clear => self.display.clear(),
            DisplayCommand::Remove => self.display.remove_station(args)?,
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.display.results())
    }
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        self.display.forget(kept);
    }
    fn save_state(&self) -> Option<sdrkit_core::StateBlob> {
        let utc = self
            .map
            .map_or(0.0, |m| m.utc_of(self.end_index.max(m.at), AUDIO_RATE));
        self.display.heard.save_state(utc).ok()
    }
    fn restore_state(
        &mut self,
        state: &sdrkit_core::StateBlob,
    ) -> Result<(), sdrkit_core::StateError> {
        self.display.restore(state)
    }
}

// The QSO sequencer.

sdrkit_core::choice_enum! {
    /// The modes the QSO sequencer runs.
    pub enum QsoMode {
        /// FT8.
        Ft8 = ("ft8", "FT8"),
        /// FT4.
        Ft4 = ("ft4", "FT4"),
        /// FT2.
        Ft2 = ("ft2", "FT2"),
        /// JT65.
        Jt65 = ("jt65", "JT65"),
        /// JT9.
        Jt9 = ("jt9", "JT9"),
        /// JT4.
        Jt4 = ("jt4", "JT4"),
    }
}

impl QsoMode {
    /// The mode.
    pub fn mode(self) -> Mode {
        match self {
            QsoMode::Ft8 => Mode::Ft8,
            QsoMode::Ft4 => Mode::Ft4,
            QsoMode::Ft2 => Mode::Ft2,
            QsoMode::Jt65 => Mode::Jt65,
            QsoMode::Jt9 => Mode::Jt9,
            QsoMode::Jt4 => Mode::Jt4,
        }
    }
    /// The option of `mode`, or `None` for a mode that the sequencer does not run.
    pub fn of_mode(mode: Mode) -> Option<QsoMode> {
        match mode {
            Mode::Ft8 => Some(QsoMode::Ft8),
            Mode::Ft4 => Some(QsoMode::Ft4),
            Mode::Ft2 => Some(QsoMode::Ft2),
            Mode::Jt65 => Some(QsoMode::Jt65),
            Mode::Jt9 => Some(QsoMode::Jt9),
            Mode::Jt4 => Some(QsoMode::Jt4),
            _ => None,
        }
    }
    fn of(mode: Mode) -> QsoMode {
        match mode {
            Mode::Ft4 => QsoMode::Ft4,
            Mode::Ft2 => QsoMode::Ft2,
            Mode::Jt65 => QsoMode::Jt65,
            Mode::Jt9 => QsoMode::Jt9,
            Mode::Jt4 => QsoMode::Jt4,
            _ => QsoMode::Ft8,
        }
    }
}

pub(crate) const QSO_MODE: Setting<field::Choice<QsoMode>> =
    Setting::choice("mode", "Mode", QsoMode::Ft8);
pub(crate) const QSO_EVEN: Setting<field::Bool> =
    Setting::new("even", "Transmit in even slots", "", true);
pub(crate) const QSO_AUTO: Setting<field::Bool> = Setting::new("auto", "Auto-sequence", "", true);
pub(crate) const QSO_HOLD: Setting<field::Bool> =
    Setting::new("hold", "Hold TX frequency", "", false);
pub(crate) const QSO_HZ: Setting<field::Hz> =
    Setting::new("hz", "TX audio frequency", "Hz", 1500.0).range(200.0, 3500.0, 1.0);
pub(crate) const QSO_RETRIES: Setting<field::I64> =
    Setting::new("retries", "Retry limit", "", 4).range(1.0, 20.0, 1.0);
pub(crate) const QSO_WATCHDOG: Setting<field::Duration> =
    Setting::new("watchdog", "Watchdog", "s", 360.0).range(30.0, 3600.0, 1.0);
const QSO_JT65_SUBMODE: Setting<field::Choice<Letter>> =
    Setting::choice("jt65_submode", "JT65 sub-mode", Letter::A);
const QSO_JT4_SUBMODE: Setting<field::Choice<Letter>> =
    Setting::choice("jt4_submode", "JT4 sub-mode", Letter::A);

sdrkit_core::settings! {
    /// The settings of [`SequencerComponent`].
    pub enum SequencerSetting {
        /// The mode.
        Mode = QSO_MODE,
        /// Transmit in even slots.
        Even = QSO_EVEN,
        /// Auto-sequence.
        Auto = QSO_AUTO,
        /// Hold the transmit frequency.
        Hold = QSO_HOLD,
        /// The transmit audio frequency.
        Hz = QSO_HZ,
        /// The retry limit.
        Retries = QSO_RETRIES,
        /// The watchdog.
        Watchdog = QSO_WATCHDOG,
        /// The JT65 sub-mode sent, that of the stack.
        Jt65Submode = QSO_JT65_SUBMODE,
        /// The JT4 sub-mode sent, that of the stack.
        Jt4Submode = QSO_JT4_SUBMODE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`SequencerComponent`].
    pub enum SequencerCommand {
        /// Call CQ.
        Cq = crate::typed::CQ,
        /// Halt.
        Halt = crate::typed::HALT,
    }
}

/// The QSO sequencer (`wsjt.qso`). My call and locator come from the station.
pub struct SequencerComponent {
    seq: Sequencer,
    /// The UTC of the end of the last audio, once the clock has given one.
    utc: Option<f64>,
    /// A CQ was asked for before the clock gave a time: it starts at the first time.
    cq_waiting: bool,
    last_tx_slot: Option<f64>,
    /// The sub-mode letters sent in JT65 and JT4.
    jt65_sub: u8,
    jt4_sub: u8,
}

impl Default for SequencerComponent {
    fn default() -> Self {
        SequencerComponent {
            seq: Sequencer::new(QsoSettings::default()),
            utc: None,
            cq_waiting: false,
            last_tx_slot: None,
            jt65_sub: 0,
            jt4_sub: 0,
        }
    }
}

impl SequencerComponent {
    /// The sequencer of the stack of `mode`: it sends that mode. `None` (the combined stack): the mode of its setting.
    pub fn new(mode: Option<Mode>) -> Self {
        let mut c = Self::default();
        if let Some(m) = mode.filter(|m| QsoMode::of_mode(*m).is_some()) {
            c.seq.s.mode = m;
        }
        c
    }
}

/// The index of `l` among the sub-modes of `mode`; a letter that the mode lacks is refused.
fn letter_of(mode: Mode, id: SettingId, l: Letter) -> Result<u8, SettingError> {
    if usize::from(l.index()) >= mode.submodes().len() {
        return Err(SettingError::NotAChoice {
            id,
            index: l.index(),
        });
    }
    Ok(l.index() as u8)
}

/// The station has no callsign or no locator: a QSO or a beacon cannot start.
#[derive(Debug)]
pub struct NoStation;

impl std::fmt::Display for NoStation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the station has no callsign or no locator")
    }
}

impl std::error::Error for NoStation {}

/// Refuses a transmission that sends my call and locator while the station lacks either.
fn station_known(call: &str, grid: &str) -> Result<(), CommandError> {
    if call.is_empty() || grid.is_empty() {
        return Err(CommandError::Failed {
            source: sdrkit_core::StageError::new(NoStation),
        });
    }
    Ok(())
}

/// My callsign and locator from the station, in capitals.
fn my_call_and_grid(st: &sdrkit_core::Station) -> (String, String) {
    (
        st.callsign.trim().to_ascii_uppercase(),
        st.locator().unwrap_or_default().to_ascii_uppercase(),
    )
}

impl Component for SequencerComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wsjt.qso",
            vec![
                Port::typed::<SlotResult>("slots"),
                Port::typed::<ClockMap>("time"),
            ],
            vec![typed_out::<TxMessage>("tx")],
        )
        .with_settings(SequencerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_from::<TxMessage>(&inputs[0])])
    }
    fn attach(&mut self, ctx: &RunContext) {
        let (call, grid) = my_call_and_grid(&ctx.station);
        self.seq.s.my_call = call;
        self.seq.s.my_grid = grid;
        if !ctx.transmit {
            // The QSO ends with the transmitter: switching it on again starts nothing.
            self.cq_waiting = false;
            self.seq.halt();
        }
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(m) = io.items::<ClockMap>(1).last() {
            self.utc = Some(m.value.utc_of(m.value.end, AUDIO_RATE));
        }
        let Some(utc) = self.utc else {
            return;
        };
        if std::mem::take(&mut self.cq_waiting) {
            self.seq.call_cq(utc);
        }
        let mode = self.seq.s.mode;
        for r in io.items::<SlotResult>(0) {
            if r.value.mode == Some(mode) && !r.value.early {
                for d in &r.value.decodes {
                    self.seq.on_decode(d, utc);
                }
            }
        }
        // The message for the next transmit slot, once per slot.
        let period = mode.default_period();
        let next = ((utc / period).floor() + 1.0) * period;
        if self.seq.is_tx_slot(next, period) && self.last_tx_slot != Some(next) {
            if let Some(text) = self.seq.message() {
                self.last_tx_slot = Some(next);
                io.items_out::<TxMessage>(0).push(Timed {
                    time: 0.0,
                    value: TxMessage {
                        mode,
                        text,
                        submode: match mode {
                            Mode::Jt65 => self.jt65_sub,
                            Mode::Jt4 => self.jt4_sub,
                            _ => 0,
                        },
                        period,
                        hz: self.seq.tx_hz,
                        slot: next,
                        js8: None,
                    },
                });
                self.seq.sent_one(utc);
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = &mut self.seq.s;
        match SequencerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SequencerSetting::Mode => s.mode = QSO_MODE.read(id, v)?.mode(),
            SequencerSetting::Even => s.even = QSO_EVEN.read(id, v)?,
            SequencerSetting::Auto => s.auto_seq = QSO_AUTO.read(id, v)?,
            SequencerSetting::Hold => s.hold_tx = QSO_HOLD.read(id, v)?,
            SequencerSetting::Hz => {
                s.tx_hz = QSO_HZ.read(id, v)?;
                self.seq.tx_hz = s.tx_hz;
            }
            SequencerSetting::Retries => s.retries = QSO_RETRIES.read(id, v)? as u32,
            SequencerSetting::Watchdog => s.watchdog_s = QSO_WATCHDOG.read(id, v)?,
            SequencerSetting::Jt65Submode => {
                self.jt65_sub = letter_of(Mode::Jt65, id, QSO_JT65_SUBMODE.read(id, v)?)?;
            }
            SequencerSetting::Jt4Submode => {
                self.jt4_sub = letter_of(Mode::Jt4, id, QSO_JT4_SUBMODE.read(id, v)?)?;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let s = &self.seq.s;
        values_of([
            (
                SequencerSetting::Mode,
                Value::Choice(QsoMode::of(s.mode).index()),
            ),
            (SequencerSetting::Even, Value::Bool(s.even)),
            (SequencerSetting::Auto, Value::Bool(s.auto_seq)),
            (SequencerSetting::Hold, Value::Bool(s.hold_tx)),
            (SequencerSetting::Hz, Value::Hz(s.tx_hz)),
            (SequencerSetting::Retries, Value::I64(i64::from(s.retries))),
            (SequencerSetting::Watchdog, Value::Duration(s.watchdog_s)),
            (
                SequencerSetting::Jt65Submode,
                Value::Choice(u16::from(self.jt65_sub)),
            ),
            (
                SequencerSetting::Jt4Submode,
                Value::Choice(u16::from(self.jt4_sub)),
            ),
        ])
    }
    fn commands(&self) -> &Commands {
        SequencerCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match SequencerCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SequencerCommand::Cq => {
                station_known(&self.seq.s.my_call, &self.seq.s.my_grid)?;
                match self.utc {
                    Some(utc) => self.seq.call_cq(utc),
                    None => self.cq_waiting = true,
                }
            }
            SequencerCommand::Halt => {
                self.cq_waiting = false;
                self.seq.halt();
            }
        }
        Ok(None)
    }
}

// The WSPR beacon.

pub(crate) const BEACON_DBM: Setting<field::I64> = Setting::new("dbm", "Power", "dBm", 10)
    .range(0.0, 60.0, 1.0)
    .help("WSPR sends 0 to 60 dBm, ending in 0, 3 or 7.");
pub(crate) const BEACON_EVERY: Setting<field::I64> =
    Setting::new("every", "One slot in N", "", 5).range(1.0, 100.0, 1.0);
pub(crate) const BEACON_OFFSET: Setting<field::I64> =
    Setting::new("offset", "Offset", "", 0).range(0.0, 99.0, 1.0);
pub(crate) const BEACON_HZ: Setting<field::Hz> =
    Setting::new("hz", "Audio frequency", "Hz", 1500.0).range(1400.0, 1600.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`BeaconComponent`].
    pub enum BeaconSetting {
        /// The power sent.
        Dbm = BEACON_DBM,
        /// One slot in N.
        Every = BEACON_EVERY,
        /// The offset in slots.
        Offset = BEACON_OFFSET,
        /// The audio frequency.
        Hz = BEACON_HZ,
    }
}

sdrkit_core::commands! {
    /// The commands of [`BeaconComponent`].
    pub enum BeaconCommand {
        /// Start the beacon.
        Beacon = crate::typed::BEACON,
        /// Stop the beacon.
        Halt = crate::typed::HALT,
    }
}

/// The WSPR beacon schedule (`wsjt.beacon`). The call and locator sent come from the station.
pub struct BeaconComponent {
    beacon: Option<Beacon>,
    on: bool,
    call: String,
    grid: String,
    dbm: i32,
    every: u32,
    offset: u32,
    hz: f64,
    last: Option<f64>,
}

impl Default for BeaconComponent {
    fn default() -> Self {
        BeaconComponent {
            beacon: None,
            on: false,
            call: String::new(),
            grid: String::new(),
            dbm: BEACON_DBM.default as i32,
            every: BEACON_EVERY.default as u32,
            offset: BEACON_OFFSET.default as u32,
            hz: BEACON_HZ.default,
            last: None,
        }
    }
}

impl BeaconComponent {
    fn rebuild(&mut self) {
        self.beacon = Some(Beacon::new(
            &self.call,
            &self.grid,
            self.dbm,
            self.every,
            self.offset,
            self.hz,
        ));
    }
}

impl Component for BeaconComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wsjt.beacon",
            vec![Port::typed::<ClockMap>("time")],
            vec![typed_out::<TxMessage>("tx")],
        )
        .with_settings(BeaconSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_from::<TxMessage>(&inputs[0])])
    }
    fn attach(&mut self, ctx: &RunContext) {
        (self.call, self.grid) = my_call_and_grid(&ctx.station);
        self.rebuild();
        if !ctx.transmit {
            // The beacon ends with the transmitter: switching it on again starts nothing.
            self.on = false;
        }
    }
    fn process(&mut self, io: &mut Io) {
        let Some(m) = io.items::<ClockMap>(0).last().map(|t| t.value) else {
            return;
        };
        if !self.on {
            return;
        }
        let utc = m.utc_of(m.end, AUDIO_RATE);
        let next = ((utc / 120.0).floor() + 1.0) * 120.0;
        if self.last == Some(next) {
            return;
        }
        self.last = Some(next);
        if let Some(b) = self.beacon.as_mut() {
            if let Some(t) = b.slot(next) {
                io.items_out::<TxMessage>(0).push(Timed {
                    time: 0.0,
                    value: t,
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BeaconSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BeaconSetting::Dbm => {
                let d = BEACON_DBM.read(id, v)? as i32;
                if !crate::proto::wsprmsg::valid_dbm(d) {
                    return Err(SettingError::NotPermitted { id });
                }
                self.dbm = d;
            }
            BeaconSetting::Every => self.every = BEACON_EVERY.read(id, v)? as u32,
            BeaconSetting::Offset => self.offset = BEACON_OFFSET.read(id, v)? as u32,
            BeaconSetting::Hz => self.hz = BEACON_HZ.read(id, v)?,
        }
        self.rebuild();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (BeaconSetting::Dbm, Value::I64(i64::from(self.dbm))),
            (BeaconSetting::Every, Value::I64(i64::from(self.every))),
            (BeaconSetting::Offset, Value::I64(i64::from(self.offset))),
            (BeaconSetting::Hz, Value::Hz(self.hz)),
        ])
    }
    fn commands(&self) -> &Commands {
        BeaconCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match BeaconCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            BeaconCommand::Beacon => {
                station_known(&self.call, &self.grid)?;
                self.on = true;
                if let Some(b) = self.beacon.as_mut() {
                    b.touched();
                }
            }
            BeaconCommand::Halt => self.on = false,
        }
        Ok(None)
    }
}

// The JS8 sender.

sdrkit_core::choice_enum! {
    /// The JS8 speeds.
    pub enum Js8Speed {
        /// Normal (15 s).
        Normal = ("normal", "Normal"),
        /// Fast (10 s).
        Fast = ("fast", "Fast"),
        /// Turbo (6 s).
        Turbo = ("turbo", "Turbo"),
    }
}

pub(crate) const JS8_SPEED: Setting<field::Choice<Js8Speed>> =
    Setting::choice("speed", "Speed", Js8Speed::Normal);
pub(crate) const JS8_HZ: Setting<field::Hz> =
    Setting::new("hz", "Audio frequency", "Hz", 1500.0).range(200.0, 3000.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`Js8SenderComponent`].
    pub enum Js8SenderSetting {
        /// The speed.
        Speed = JS8_SPEED,
        /// The audio frequency.
        Hz = JS8_HZ,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Js8SenderComponent`].
    pub enum Js8SenderCommand {
        /// Queue a message.
        Send = SEND.command,
        /// Drop the queued frames.
        Halt = crate::typed::HALT,
    }
}

/// The JS8 sender (`wsjt.js8_sender`). The `send` command queues a message. My call and locator come from the station.
pub struct Js8SenderComponent {
    sender: Js8Sender,
    call: String,
    grid: String,
    last: Option<f64>,
}

impl Default for Js8SenderComponent {
    fn default() -> Self {
        let mut sender = Js8Sender::default();
        sender.hz = JS8_HZ.default;
        Js8SenderComponent {
            sender,
            call: String::new(),
            grid: String::new(),
            last: None,
        }
    }
}

/// The message of a JS8 text from `call` at `grid` (see [`SendArgs`]).
///
/// # Errors
///
/// [`PackError::UnknownCommand`] for a text to a station that contains no command and no text.
pub fn parse_js8_send(call: &str, grid: &str, text: &str) -> Result<Outgoing, PackError> {
    let t = text.trim().to_ascii_uppercase();
    if t == "HB" || t == "CQ" {
        return Ok(Outgoing::Heartbeat {
            call: call.into(),
            grid: grid.into(),
            cq: (t == "CQ").then_some(0),
        });
    }
    let (head, rest) = t.split_once(' ').unwrap_or((t.as_str(), ""));
    if let Some((via, dest)) = head.split_once('>') {
        return Ok(Outgoing::Relay {
            from: call.into(),
            via: via.into(),
            dest: dest.into(),
            text: rest.into(),
        });
    }
    let r = format!(" {rest}");
    let (cmd, num) = match r.rsplit_once(' ') {
        Some((c, n)) if command_code(c).is_some() => match n.parse::<i32>() {
            Ok(n) => (c.to_string(), n),
            Err(_) => (r.clone(), 0),
        },
        _ => (r.clone(), 0),
    };
    if command_code(&cmd).is_some() {
        return Ok(Outgoing::Command {
            from: call.into(),
            to: head.into(),
            cmd,
            num,
        });
    }
    if rest.is_empty() {
        return Err(PackError::UnknownCommand);
    }
    Ok(Outgoing::Text {
        from: call.into(),
        to: head.into(),
        text: rest.into(),
    })
}

impl Js8SenderComponent {
    fn send(&mut self, args: &Values) -> Result<(), CommandError> {
        let arg = Js8SenderCommand::commands()
            .def(Js8SenderCommand::Send.id())
            .and_then(|d| d.args.id("text").ok())
            .ok_or(CommandError::NotAnArgument { key: "text" })?;
        let SendArgs { text } = SEND.args(args)?;
        if text.trim().is_empty() {
            return Ok(());
        }
        if self.call.is_empty() {
            return Err(CommandError::BadArgument {
                arg,
                error: SettingError::NeedsStation {
                    id: arg,
                    item: StationItem::Callsign,
                },
            });
        }
        let malformed = |_| CommandError::BadArgument {
            arg,
            error: SettingError::Malformed { id: arg, at: None },
        };
        let m = parse_js8_send(&self.call, &self.grid, text).map_err(malformed)?;
        self.sender.send(&m).map_err(malformed)?;
        Ok(())
    }
}

impl Component for Js8SenderComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wsjt.js8_sender",
            vec![Port::typed::<ClockMap>("time")],
            vec![typed_out::<TxMessage>("tx")],
        )
        .with_settings(Js8SenderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed_from::<TxMessage>(&inputs[0])])
    }
    fn attach(&mut self, ctx: &RunContext) {
        (self.call, self.grid) = my_call_and_grid(&ctx.station);
        if !ctx.transmit {
            // The queue ends with the transmitter: switching it on again sends nothing.
            self.sender.clear();
        }
    }
    fn process(&mut self, io: &mut Io) {
        let Some(m) = io.items::<ClockMap>(0).last().map(|t| t.value) else {
            return;
        };
        if self.sender.pending() == 0 {
            return;
        }
        let p = Sub::new(Mode::Js8).with_sub(self.sender.submode).period;
        let utc = m.utc_of(m.end, AUDIO_RATE);
        let next = ((utc / p).floor() + 1.0) * p;
        if self.last == Some(next) {
            return;
        }
        self.last = Some(next);
        if let Some(t) = self.sender.slot(next) {
            io.items_out::<TxMessage>(0).push(Timed {
                time: 0.0,
                value: t,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match Js8SenderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Js8SenderSetting::Speed => self.sender.submode = JS8_SPEED.read(id, v)?.index() as u8,
            Js8SenderSetting::Hz => self.sender.hz = JS8_HZ.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let speed =
            Js8Speed::from_index(u16::from(self.sender.submode)).unwrap_or(Js8Speed::Normal);
        values_of([
            (Js8SenderSetting::Speed, Value::Choice(speed.index())),
            (Js8SenderSetting::Hz, Value::Hz(self.sender.hz)),
        ])
    }
    fn commands(&self) -> &Commands {
        Js8SenderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match Js8SenderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            Js8SenderCommand::Send => self.send(args)?,
            Js8SenderCommand::Halt => self.sender.clear(),
        }
        Ok(None)
    }
}

// The transmitter.

/// The rate at which the sender makes the baseband of a message, before it brings it to the rate of the session.
const BASE_RATE: f64 = 12_000.0;
/// A message whose start passed more than this many seconds ago is dropped.
const LATE_S: f64 = 1.0;

const TX_LEVEL: Setting<field::F64> =
    Setting::new("level", "Transmit level", "", 0.5).range(0.05, 0.9, 0.01);

sdrkit_core::settings! {
    /// The settings of [`SenderComponent`].
    pub enum SenderSetting {
        /// The peak amplitude, full scale 1.
        Level = TX_LEVEL,
    }
}

sdrkit_core::commands! {
    /// The commands of [`SenderComponent`].
    pub enum SenderCommand {
        /// Drop the message on air and those waiting.
        Halt = crate::typed::HALT,
    }
}

/// The baseband samples that a burst brings to the session's rate at one time.
const CHUNK: usize = 64;

/// One message on air: its baseband at [`BASE_RATE`], the samples taken from it, and its output at the session's rate.
struct Burst {
    base: Vec<Complex32>,
    at: usize,
    out: BurstOut,
}

impl Burst {
    /// Takes baseband samples until `room` output samples wait, or the rest of the baseband with the tail of the rate
    /// change.
    fn fill(&mut self, room: usize, scratch: &mut Vec<Complex32>) {
        while self.out.wants(room) {
            let n = CHUNK.min(self.base.len() - self.at);
            self.out.push(&self.base[self.at..self.at + n]);
            self.at += n;
            if self.at == self.base.len() {
                self.out.finish(scratch);
            }
        }
    }

    /// Ends the burst with the samples that wait and the tail of the rate change.
    fn halt(&mut self, scratch: &mut Vec<Complex32>) {
        self.at = self.base.len();
        self.out.finish(scratch);
    }
}

/// The transmitter (`wsjt.sender`): input `in` is the session's samples (their rate, pace and dial), input `time` the slot
/// clock, inputs `msg0`, `msg1`, .. the messages of the sequencer, the beacon and the JS8 sender. Output `out` is the
/// complex baseband for `tx.sink`, centred on the dial: the upper sideband, tone 0 at the audio frequency of the message.
/// A message goes on air at the start of its slot plus the start offset of its mode. On a live radio whose host
/// times bursts and whose slot clock is the clock of the host, the burst is written early by that lead, the age of the
/// newest sample and two passes ([`BurstTiming::lead_before`]), and its first sample states its time on air; else it is
/// written when its time has come. While the transmitter is off the
/// messages are dropped. A burst cut by the transmitter going off or by a new format ends with an end mark on the next
/// pass. A text that cannot be packed sends nothing and is counted.
pub struct SenderComponent {
    inputs: usize,
    amp: f32,
    rate: f64,
    dial: f64,
    transmit: bool,
    utc: Option<f64>,
    queue: Vec<TxMessage>,
    burst: Option<Burst>,
    /// A burst was cut: its end mark goes out on the next pass.
    end_owed: bool,
    errors: u64,
    timing: BurstTiming,
    scratch: Vec<Complex32>,
}

impl Default for SenderComponent {
    fn default() -> Self {
        Self::new(1)
    }
}

impl SenderComponent {
    /// A sender with `inputs` message inputs.
    pub fn new(inputs: usize) -> Self {
        SenderComponent {
            inputs: inputs.max(1),
            amp: TX_LEVEL.default as f32,
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            utc: None,
            queue: Vec::new(),
            burst: None,
            end_owed: false,
            errors: 0,
            timing: BurstTiming::default(),
            scratch: Vec::new(),
        }
    }

    /// Drops the burst on air; its end mark goes out on the next pass.
    fn cut(&mut self) {
        if self.burst.take().is_some() {
            self.end_owed = true;
        }
    }

    /// Messages that sent nothing: the text could not be packed, or the rate of the session could not be reached.
    pub fn errors(&self) -> u64 {
        self.errors
    }

    /// The UTC at which `m` goes on air.
    fn start_of(m: &TxMessage) -> f64 {
        m.slot
            + Sub::new(m.mode)
                .with_sub(m.submode)
                .with_period(m.period)
                .t0()
    }

    /// Starts the first message whose time has come, at `utc`: its time on air less `early`, and with `early` its
    /// start is stated. Messages more than [`LATE_S`] late are dropped.
    fn start_due(&mut self, utc: f64, early: Option<f64>) {
        self.queue.retain(|m| Self::start_of(m) + LATE_S > utc);
        let due = utc + early.unwrap_or(0.0);
        let Some(i) = self.queue.iter().position(|m| Self::start_of(m) <= due) else {
            return;
        };
        let m = self.queue.remove(i);
        let sub = Sub::new(m.mode).with_sub(m.submode).with_period(m.period);
        let t = Transmission {
            sub,
            hz: m.hz,
            amp: self.amp,
            mirrored: false,
        };
        let symbols = match &m.js8 {
            Some((bits, flags)) => Ok(encode::js8_tones(m.submode, bits, *flags)),
            // Only the modes offered for transmission are sent.
            None => encode::channel_symbols(&sub, &m.text),
        };
        let out = BurstOut::with_quality(BASE_RATE, self.rate, 0.35, 70.0, 1.0);
        let (Ok(symbols), Ok(mut out)) = (symbols, out) else {
            self.errors += 1;
            return;
        };
        let mut base = Vec::new();
        encode::baseband_of(&t, &symbols, BASE_RATE, sub.period - 0.5, &mut base);
        // The first sample written is the first of the baseband less the delay of the step to the session rate.
        out.state_start(early.map(|_| Self::start_of(&m) - out.delay_s()));
        self.burst = (!base.is_empty()).then_some(Burst { base, at: 0, out });
    }
}

impl Component for SenderComponent {
    fn info(&self) -> ComponentInfo {
        let mut inputs = vec![Port::iq("in"), Port::typed::<ClockMap>("time")];
        inputs.extend((0..self.inputs).map(|k| Port::typed::<TxMessage>(&format!("msg{k}"))));
        ComponentInfo::processor("wsjt.sender", inputs, vec![Port::iq("out")])
            .with_settings(SenderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.cut();
        Ok(vec![Format::iq(self.rate, self.dial, self.dial)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        if !ctx.transmit {
            // The transmission ends with the transmitter.
            self.queue.clear();
            self.cut();
        }
    }
    fn process(&mut self, io: &mut Io) {
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
        }
        if let Some(m) = io.items::<ClockMap>(1).last() {
            self.utc = Some(m.value.utc_of(m.value.end, AUDIO_RATE));
        }
        for k in 0..self.inputs {
            for m in io.items::<TxMessage>(2 + k) {
                if self.transmit {
                    self.queue.push(m.value.clone());
                }
            }
        }
        if !self.transmit {
            return;
        }
        let room = io.input_len(0);
        if let (None, Some(utc)) = (&self.burst, self.utc) {
            let early = self
                .timing
                .lead_before(utc, room as f64 / self.rate, LATE_S);
            self.start_due(utc, early);
        }
        let Some(b) = self.burst.as_mut() else {
            return;
        };
        b.fill(room, &mut self.scratch);
        if b.out.write(io, 0, room, &self.timing) {
            self.burst = None;
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SenderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SenderSetting::Level => self.amp = TX_LEVEL.read(id, v)? as f32,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(SenderSetting::Level, Value::F64(f64::from(self.amp)))])
    }
    fn commands(&self) -> &Commands {
        SenderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match SenderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SenderCommand::Halt => {
                self.queue.clear();
                if let Some(b) = self.burst.as_mut() {
                    // The burst ends with the samples that wait and the tail of the rate change.
                    b.halt(&mut self.scratch);
                }
            }
        }
        Ok(None)
    }
}

// Whole-span decoding and the multi-band monitor.

pub(crate) const WIDE: Setting<field::Bool> =
    Setting::new("wide", "Decode the whole span", "", false);
pub(crate) const WIDE_SPAN: Setting<field::Hz> =
    Setting::new("span", "Span", "Hz", 250_000.0).range(1000.0, 100e6, 1000.0);
pub(crate) const MULTIBAND: Setting<field::Bool> =
    Setting::new("multiband", "Decode all selected at once", "", false);
pub(crate) const MULTIBAND_SELECT: Setting<field::I64> =
    Setting::new("select", "Bands and modes", "", 0)
        .range(0.0, 17_592_186_044_415.0, 1.0)
        .help("The selection as a mask: bit band x 4 + mode, for the bands of the band edges and the modes FT8, FT4, FT2 and WSPR.");

sdrkit_core::settings! {
    /// The settings of [`WideComponent`].
    pub enum WideSetting {
        /// Decode the whole span.
        Wide = WIDE,
        /// The span.
        Span = WIDE_SPAN,
        /// Decode all selected at once.
        Multiband = MULTIBAND,
        /// The selected bands and modes.
        Select = MULTIBAND_SELECT,
        /// The band edges.
        BandEdges = BAND_EDGES,
        /// The dials.
        Dials = DIALS,
    }
}

sdrkit_core::commands! {
    /// The commands of [`WideComponent`].
    pub enum WideCommand {
        /// Forgets the decodes and hash tables of every dial.
        Clear = sdrkit_util_objects::commands::CLEAR,
    }
}

/// One dial of whole-span decoding or of the multi-band monitor: its channel, front end, cutter and decoder.
struct DialChain {
    mode: Mode,
    dial: f64,
    band: usize,
    fe: FrontEnd,
    cut: Cutter,
    runner: PoolRunner,
    audio: Vec<f32>,
    index: u64,
    started: bool,
    decodes: u64,
}

/// Whole-span decoding and the multi-band monitor (`wsjt.wide`): FT8, FT4, FT2 and WSPR on every dial of the span or of the
/// selection inside the capture, each with its own channel, demodulator, cutter and decoder, sharing the slot clock. The
/// decoders run on the graph's helper threads for a live radio.
pub struct WideComponent {
    wide: bool,
    span: f64,
    multiband: bool,
    mask: u64,
    plan: bands::BandPlan,
    band_edges: String,
    dials: String,
    chains: Vec<DialChain>,
    rate: f64,
    centre: f64,
    tuned: f64,
    ctx: RunContext,
    guard: bands::CpuGuard,
    results: Published,
    slots: Vec<Slot>,
    done: Vec<SlotResult>,
    dropped: Vec<(usize, usize)>,
    key: String,
}

impl Default for WideComponent {
    fn default() -> Self {
        WideComponent {
            wide: WIDE.default,
            span: WIDE_SPAN.default,
            multiband: MULTIBAND.default,
            mask: MULTIBAND_SELECT.default as u64,
            plan: bands::BandPlan::default(),
            band_edges: String::new(),
            dials: String::new(),
            chains: Vec::new(),
            rate: 0.0,
            centre: 0.0,
            tuned: 0.0,
            ctx: RunContext::default(),
            guard: bands::CpuGuard::default(),
            results: Published::new(WideFields::SHAPE.fields).with_tables(&[&MONITOR]),
            slots: Vec::new(),
            done: Vec::new(),
            dropped: Vec::new(),
            key: String::new(),
        }
    }
}

impl WideComponent {
    fn rebuild(&mut self) {
        self.chains.clear();
        if self.rate > 0.0 {
            for (mode, dial, band) in self.dials() {
                if let Ok(fe) = FrontEnd::new(self.rate, dial - self.centre, Demod::Usb) {
                    let mut cut = Cutter::new(Sub::new(mode), AUDIO_RATE);
                    cut.set_dial(dial);
                    self.chains.push(DialChain {
                        mode,
                        dial,
                        band,
                        fe,
                        cut,
                        runner: PoolRunner::new(DecodeState::default(), &self.ctx, 6),
                        audio: Vec::new(),
                        index: 0,
                        started: false,
                        decodes: 0,
                    });
                }
            }
        }
        self.results.clear_rows(&MONITOR);
        self.publish();
    }

    /// The dials to decode: the span's and the covered ones of the selection.
    fn dials(&self) -> Vec<(Mode, f64, usize)> {
        let mut dials: Vec<(Mode, f64, usize)> = Vec::new();
        if self.wide {
            for (m, d) in self
                .plan
                .span_dials(self.tuned, self.span.min(0.8 * self.rate))
            {
                let band = self.plan.band_of(d).unwrap_or(0);
                dials.push((m, d, band));
            }
        }
        if self.multiband {
            for (b, m) in bands::pairs_of(self.mask) {
                if self.dropped.contains(&(b, m)) {
                    continue;
                }
                if let Some(d) = self.plan.monitor_dial(b, m) {
                    let mode = bands::MONITOR_MODES[m];
                    if bands::covered(d, self.centre, self.rate)
                        && !dials.iter().any(|x| (x.1 - d).abs() < 1.0 && x.0 == mode)
                    {
                        dials.push((mode, d, b));
                    }
                }
            }
        }
        dials
    }

    fn publish(&mut self) {
        let p = &mut self.results;
        p.put(&WideFields {
            channels: self.chains.len() as i64,
            load_pct: 100.0 * self.guard.load,
            too_slow: self.guard.too_slow,
        });
        for (b, m) in bands::pairs_of(self.mask) {
            let Some(band) = self.plan.bands.get(b) else {
                continue;
            };
            let mode = bands::MONITOR_MODES[m];
            let d = self.plan.monitor_dial(b, m);
            let running = self.chains.iter().find(|c| c.band == b && c.mode == mode);
            let covered = d.is_some_and(|d| bands::covered(d, self.centre, self.rate));
            self.key.clear();
            {
                use std::fmt::Write;
                // Writing to a String cannot fail.
                let _ = write!(self.key, "{b:02}{m}");
            }
            MONITOR.put(
                p,
                &self.key,
                &MultibandRow {
                    band: &band.name,
                    mode,
                    dial_hz: d.map(Hertz),
                    running: running.is_some(),
                    decodes: running.map_or(0, |c| c.decodes) as i64,
                    covered,
                    span_needed_hz: (!covered).then(|| Hertz(bands::width_needed(d, self.centre))),
                },
            );
        }
    }
}

impl Component for WideComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wsjt.wide",
            vec![Port::iq("in"), Port::typed::<ClockMap>("time")],
            vec![typed_out::<SlotResult>("results")],
        )
        .with_settings(WideSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.centre = f.centre_hz;
        self.tuned = f.tuned_hz;
        self.rebuild();
        Ok(vec![typed_from::<SlotResult>(f)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.ctx = ctx.clone();
    }
    fn process(&mut self, io: &mut Io) {
        if self.chains.is_empty() {
            return;
        }
        let Some(map) = io.items::<ClockMap>(1).last().map(|t| t.value) else {
            return;
        };
        let t = std::time::Instant::now();
        let iq = io.iq(0);
        // The slot clock counts samples of the main 8 kHz audio. Each chain's audio has the same rate and start.
        for c in self.chains.iter_mut() {
            c.audio.clear();
            c.fe.process(iq, &mut c.audio);
            if !c.started {
                // A chain made while running starts at the main audio's position.
                c.started = true;
                c.index = map.end.saturating_sub(c.audio.len() as u64);
            }
            self.slots.clear();
            c.cut
                .push(&c.audio, c.index, &map, false, false, &mut self.slots);
            c.index += c.audio.len() as u64;
            for s in self.slots.drain(..) {
                c.runner.submit(s);
            }
            self.done.clear();
            c.runner.poll(&mut self.done);
            for mut d in self.done.drain(..) {
                c.decodes += d.decodes.len() as u64;
                for x in d.decodes.iter_mut() {
                    x.dial = c.dial;
                }
                io.items_out::<SlotResult>(0).push(Timed {
                    time: 0.0,
                    value: d,
                });
            }
        }
        // The CPU guard of the multi-band monitor.
        if self.multiband && !iq.is_empty() {
            let load = t.elapsed().as_secs_f64() / (iq.len() as f64 / self.rate);
            if self.guard.update(load) {
                let mut per_band: Vec<(usize, u64)> = Vec::new();
                for c in &self.chains {
                    match per_band.iter_mut().find(|b| b.0 == c.band) {
                        Some(b) => b.1 += c.decodes,
                        None => per_band.push((c.band, c.decodes)),
                    }
                }
                if let Some(v) = bands::CpuGuard::victim(&per_band) {
                    for m in 0..4 {
                        self.dropped.push((v, m));
                    }
                    self.rebuild();
                }
            }
        }
        if io.tick() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match WideSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            WideSetting::Wide => self.wide = WIDE.read(id, v)?,
            WideSetting::Span => self.span = WIDE_SPAN.read(id, v)?,
            WideSetting::Multiband => self.multiband = MULTIBAND.read(id, v)?,
            WideSetting::Select => {
                self.mask = MULTIBAND_SELECT.read(id, v)? as u64;
                self.dropped.clear();
            }
            WideSetting::BandEdges => {
                let t = BAND_EDGES.read(id, v)?;
                self.plan.bands = band_edges(id, t)?;
                t.clone_into(&mut self.band_edges);
                self.dropped.clear();
            }
            WideSetting::Dials => {
                let t = DIALS.read(id, v)?;
                self.plan.dials = dials(id, t)?;
                t.clone_into(&mut self.dials);
            }
        }
        self.rebuild();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (WideSetting::Wide, Value::Bool(self.wide)),
            (WideSetting::Span, Value::Hz(self.span)),
            (WideSetting::Multiband, Value::Bool(self.multiband)),
            (WideSetting::Select, Value::I64(self.mask as i64)),
            (WideSetting::BandEdges, Value::Text(self.band_edges.clone())),
            (WideSetting::Dials, Value::Text(self.dials.clone())),
        ])
    }
    fn commands(&self) -> &Commands {
        WideCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match WideCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            WideCommand::Clear => {
                for c in &mut self.chains {
                    c.runner.clear();
                    c.decodes = 0;
                }
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

// Band hopping.

pub(crate) const HOP_ENABLED: Setting<field::Bool> = Setting::new("enabled", "Band hopping", "", false)
    .help("At the start of each 2-minute slot, asks the host to tune to the dial of the band of the plan. Off while the whole span is decoded.");
pub(crate) const HOP_COORDINATED: Setting<field::Bool> =
    Setting::new("coordinated", "Coordinated hopping", "", true)
        .help("Every station takes the same band of the cycle in the same slot.");
pub(crate) const HOP_HOURS: Setting<field::Text> = Setting::new(
    "hours",
    "Bands of each hour",
    "",
    "0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0",
)
.help("24 hexadecimal masks separated by commas, UTC hour 0 first. Bit b of a mask is band b of the band list.");
pub(crate) const HOP_BANDS: Setting<field::Text> = Setting::new("bands", "Band list", "", "")
    .help("The band names in bit order, separated by commas (for example from data/wsjt/band_hopping.csv). Empty: no band.")
    .advanced();
pub(crate) const HOP_CYCLE: Setting<field::Text> =
    Setting::new("cycle", "Coordinated cycle", "", "")
        .help("The bands of the coordinated cycle in their order, by the names of the band list.")
        .advanced();
pub(crate) const HOP_WIDE: Setting<field::Bool> =
    Setting::new("wide", "Whole span decoded", "", false)
        .help("Hopping stops while the whole span is decoded.");

sdrkit_core::settings! {
    /// The settings of [`HopComponent`].
    pub enum HopSetting {
        /// Band hopping.
        Enabled = HOP_ENABLED,
        /// Coordinated.
        Coordinated = HOP_COORDINATED,
        /// The masks of the hours.
        Hours = HOP_HOURS,
        /// The band list.
        Bands = HOP_BANDS,
        /// The cycle.
        Cycle = HOP_CYCLE,
        /// The whole span is decoded.
        Wide = HOP_WIDE,
        /// The band edges.
        BandEdges = BAND_EDGES,
        /// The dials.
        Dials = DIALS,
    }
}

/// Band hopping (`wsjt.hop`): the slot clock in; at the start of each hop slot, a retune request to the stack's dial on the
/// plan's band when that dial is not the one tuned. No request when the stack has no dial on the band, while hopping is off, or
/// while the whole span is decoded.
pub struct HopComponent {
    mode: Mode,
    enabled: bool,
    wide: bool,
    plan: bands::HopPlan,
    dials: bands::BandPlan,
    band_edges_text: String,
    dials_text: String,
    hours_text: String,
    bands_text: String,
    cycle_text: String,
    tuned: f64,
    last: Option<i64>,
    results: Published,
}

impl HopComponent {
    /// The hopping of `mode`.
    pub fn new(mode: Mode) -> Self {
        HopComponent {
            mode,
            enabled: HOP_ENABLED.default,
            wide: HOP_WIDE.default,
            plan: bands::HopPlan {
                coordinated: HOP_COORDINATED.default,
                ..Default::default()
            },
            dials: bands::BandPlan::default(),
            band_edges_text: String::new(),
            dials_text: String::new(),
            hours_text: HOP_HOURS.default.to_owned(),
            bands_text: String::new(),
            cycle_text: String::new(),
            tuned: 0.0,
            last: None,
            results: Published::new(HopFields::SHAPE.fields),
        }
    }
}

impl Default for HopComponent {
    fn default() -> Self {
        Self::new(Mode::Ft8)
    }
}

impl Component for HopComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wsjt.hop",
            vec![Port::typed::<ClockMap>("time")],
            vec![typed_out::<sdrkit_core::HostRequest>("requests")],
        )
        .with_settings(HopSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.tuned = inputs[0].tuned_hz;
        self.last = None;
        Ok(vec![typed_from::<sdrkit_core::HostRequest>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(m) = io.items::<ClockMap>(0).last().map(|t| t.value) else {
            return;
        };
        let utc = m.utc_of(m.end, AUDIO_RATE);
        let slot = bands::HopPlan::slot_of(utc);
        if self.last == Some(slot) {
            return;
        }
        self.last = Some(slot);
        let on = self.enabled && !self.wide;
        let band = on.then(|| self.plan.band_at(utc)).flatten();
        let name = band.and_then(|b| self.plan.name(b));
        self.results.put(&HopFields {
            band: name,
            next_utc: on.then(|| Time(bands::HopPlan::next_change(utc))),
        });
        let Some((n, dial)) = name.and_then(|n| Some((n, self.dials.dial_of(self.mode, n)?)))
        else {
            return;
        };
        if (dial - self.tuned).abs() > 1.0 {
            io.items_out::<sdrkit_core::HostRequest>(0).push(Timed {
                time: 0.0,
                value: sdrkit_core::HostRequest::Retune {
                    tuned_hz: dial,
                    reason: format!("band hopping: {} on {n}", self.mode.name()),
                },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let bad = |_| SettingError::Malformed { id, at: None };
        match HopSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HopSetting::Enabled => self.enabled = HOP_ENABLED.read(id, v)?,
            HopSetting::Wide => self.wide = HOP_WIDE.read(id, v)?,
            HopSetting::Coordinated => self.plan.coordinated = HOP_COORDINATED.read(id, v)?,
            HopSetting::Hours => {
                let t = HOP_HOURS.read(id, v)?;
                self.plan.hours = bands::parse_hours(t).map_err(bad)?;
                self.hours_text = bands::hours_text(&self.plan.hours);
            }
            HopSetting::Bands => {
                let t = HOP_BANDS.read(id, v)?;
                let list = bands::parse_bands(t).map_err(bad)?;
                self.plan.cycle = bands::parse_cycle(&self.cycle_text, &list).unwrap_or_default();
                self.plan.bands = list;
                t.clone_into(&mut self.bands_text);
            }
            HopSetting::Cycle => {
                let t = HOP_CYCLE.read(id, v)?;
                // A cycle that names a band the list does not have yet is kept as text until the list has it.
                self.plan.cycle = bands::parse_cycle(t, &self.plan.bands).unwrap_or_default();
                t.clone_into(&mut self.cycle_text);
            }
            HopSetting::BandEdges => {
                let t = BAND_EDGES.read(id, v)?;
                self.dials.bands = band_edges(id, t)?;
                t.clone_into(&mut self.band_edges_text);
            }
            HopSetting::Dials => {
                let t = DIALS.read(id, v)?;
                self.dials.dials = dials(id, t)?;
                t.clone_into(&mut self.dials_text);
            }
        }
        // The new plan applies from the next process call.
        self.last = None;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (HopSetting::Enabled, Value::Bool(self.enabled)),
            (HopSetting::Coordinated, Value::Bool(self.plan.coordinated)),
            (HopSetting::Hours, Value::Text(self.hours_text.clone())),
            (HopSetting::Bands, Value::Text(self.bands_text.clone())),
            (HopSetting::Cycle, Value::Text(self.cycle_text.clone())),
            (HopSetting::Wide, Value::Bool(self.wide)),
            (
                HopSetting::BandEdges,
                Value::Text(self.band_edges_text.clone()),
            ),
            (HopSetting::Dials, Value::Text(self.dials_text.clone())),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}

/// The standard components and the stack's (kinds `wsjt.*`).
pub fn registry() -> Registry {
    let mut r = sdrkit_components::registry();
    r.add("wsjt.front", || Box::new(FrontComponent::default()));
    r.add("wsjt.slot_clock", || Box::new(ClockComponent::default()));
    r.add("wsjt.cutter", || Box::new(CutterComponent::default()));
    r.add("wsjt.decoder", || {
        Box::new(decoder(Mode::Ft8, None))
    });
    r.add("wsjt.merge", || Box::new(MergeComponent::default()));
    r.add("wsjt.display", || Box::new(DisplayComponent::default()));
    r.add("wsjt.qso", || Box::new(SequencerComponent::default()));
    r.add("wsjt.beacon", || Box::new(BeaconComponent::default()));
    r.add(
        "wsjt.js8_sender",
        || Box::new(Js8SenderComponent::default()),
    );
    r.add("wsjt.sender", || Box::new(SenderComponent::default()));
    r.add("wsjt.wide", || Box::new(WideComponent::default()));
    r.add("wsjt.hop", || Box::new(HopComponent::default()));
    r.add("wsjt.fsk441", || {
        Box::new(DecoderComponent::new(
            crate::scatter::fsk441::Fsk441Decoder::new(),
        ))
    });
    r.add("wsjt.jt6m", || {
        Box::new(DecoderComponent::new(
            crate::scatter::jt6m::Jt6mDecoder::new(),
        ))
    });
    r.add("wsjt.iscat", || {
        Box::new(DecoderComponent::new(
            crate::scatter::iscat::IscatDecoder::new(),
        ))
    });
    r.add("wsjt.pi4", || {
        Box::new(DecoderComponent::new(crate::scatter::pi4::Pi4Decoder::new()))
    });
    r
}
