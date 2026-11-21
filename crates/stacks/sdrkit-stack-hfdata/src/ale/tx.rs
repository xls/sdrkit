//! The ALE transmitter (`hfdata.ale_tx`): calls, soundings, AMD texts and the three-way handshake on one channel, as
//! continuous-phase 8-FSK on the upper sideband of the dial.
//!
//! A call is the called address in TO words, sent for the scan time of the called station (its channels times its
//! dwell) and then the set number of times (the calling cycle), the LQA word and the AMD words when asked, then the own
//! address with TIS first (TWS after the allcall `@?@`). A sounding is the own address in TWS or TIS words, sent the
//! calling cycle number of times. The handshake: a call to the own address that ends in TIS is answered by the response
//! (TO caller twice, the LQA of the call when the call asked for it, TIS own address); a response to the own call is
//! answered by the acknowledgement (TO called twice, the LQA of the response when asked, TWS own address). The words are
//! coded, interleaved and sent three times as the receiver of the stack reads them ([`super::word::word_tones`]), and
//! written by the function of the test signal ([`super::gen::tone_sample`]).
//!
//! Standard: MIL-STD-188-141 Appendix A (word format, Golay code, 8-FSK, calls, LQA, AMD).

use super::calls::{AleCall, CallKind};
use super::gen::{frame_words, tone_sample, RAMP_S, SYMBOL_S, WORD_S};
use super::word::{self, address_words, sendable_address, sendable_amd, Lqa, TIS, TO, TWS};
use crate::transmit::{Air, TxError, LEVEL};
use crate::typed::{
    AleAmdArgs, AleCallArgs, AleLinkEvent, AleLinkState, AleTxEvent, AleTxFields, AleTxKind,
    ALE_AMD, ALE_CALL_TX, ALE_LINK, ALE_SOUND, ALE_STOP, ALE_TX,
};
use sdrkit_components::tx::{end_mark, start_mark};
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::{field, keys::Family, Published};
use sdrkit_core::{
    Choice, CommandError, CommandId, Commands, ConfigError, Event, Record, Results, Setting,
    SettingError, SettingId, StageError, Value, Values,
};
use sdrkit_core::{CommandSet, SettingSet};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
};

/// The section of the transmit settings.
pub const TX_SECTION: &str = "Transmit";

sdrkit_core::choice_enum! {
    /// The first word of a sounding.
    pub enum SoundingWord {
        /// TWS: the station asks for no answer.
        Tws = ("tws", "TWS, no answer"),
        /// TIS: the station welcomes an answer.
        Tis = ("tis", "TIS, answer welcome"),
    }
}

/// The own address.
pub const SELF_ADDRESS: Setting<field::Text> = Setting::new("self_address", "Own address", "", "")
    .help("1 to 15 characters of A-Z and 0-9, sent in the TIS or TWS words. Empty refuses call and sound.")
    .section(TX_SECTION);
/// The times the address is sent before the own address (a call) or in all (a sounding).
pub const CALL_REPEATS: Setting<field::I64> =
    Setting::new("call_repeats", "Calling cycle repeats", "", 6)
        .range(1.0, 100.0, 1.0)
        .help("How many times a call sends the called address before the own address, and a sounding the own address. 6 is the figure of the specification for one channel.")
        .section(TX_SECTION);
/// The first word of a sounding.
pub const SOUNDING: Setting<field::Choice<SoundingWord>> =
    Setting::choice("sounding", "Sounding word", SoundingWord::Tws).section(TX_SECTION);
/// The channels that the called station scans.
pub const SCAN_CHANNELS: Setting<field::I64> =
    Setting::new("scan_channels", "Scanned channels", "", 0)
        .range(0.0, 100.0, 1.0)
        .help("The channels in the scan list of the called station. A call first sends the called address for the scan time (channels x dwell), so that the station finds the call when it next listens on this channel. 0 sends no scanning part.")
        .section(TX_SECTION);
/// The dwell of the called station on each channel.
pub const SCAN_DWELL_S: Setting<field::F64> =
    Setting::new("scan_dwell_s", "Dwell on a channel", "s", 0.5)
        .range(0.1, 1.0, 0.0)
        .help("How long the called station listens on each channel of its scan: 0.5 s at 2 channels a second, 0.2 s at 5.")
        .section(TX_SECTION);
/// Whether a call to the own address is answered.
pub const AUTO_ANSWER: Setting<field::Bool> =
    Setting::new("auto_answer", "Answer calls", "", false)
        .help("Answers a call to the own address that ends in TIS with the response, and so completes the three-way handshake, with no key press. Off at the start, so that nothing is sent unattended. The engine's transmit switch must be on.")
        .section(TX_SECTION);
/// Whether a call asks for the link quality.
pub const LQA_EXCHANGE: Setting<field::Bool> =
    Setting::new("lqa", "Exchange link quality", "", false)
        .help("A call carries the LQA command, so that the response carries the figures of the call as heard; the acknowledgement then carries the figures of the response. Answers carry them also when the other station asks.")
        .section(TX_SECTION);
/// How long the stack waits for an answer to start.
pub const ANSWER_WAIT_S: Setting<field::F64> =
    Setting::new("answer_wait_s", "Wait for an answer", "s", 2.0)
        .range(0.5, 30.0, 0.0)
        .help("The longest time from the end of a call or a response to the start of its answer. The other station hears the end of a transmission two words (0.784 s) after its last word.")
        .section(TX_SECTION);

sdrkit_core::settings! {
    /// The settings of [`AleTransmitter`].
    pub enum AleTxSetting {
        /// The own address.
        SelfAddress = SELF_ADDRESS,
        /// The calling cycle repeats.
        CallRepeats = CALL_REPEATS,
        /// The first word of a sounding.
        Sounding = SOUNDING,
        /// The channels of the called station.
        ScanChannels = SCAN_CHANNELS,
        /// The dwell of the called station.
        ScanDwell = SCAN_DWELL_S,
        /// Answer calls.
        AutoAnswer = AUTO_ANSWER,
        /// Exchange link quality.
        Lqa = LQA_EXCHANGE,
        /// Wait for an answer.
        AnswerWait = ANSWER_WAIT_S,
    }
}

sdrkit_core::commands! {
    /// The commands of [`AleTransmitter`].
    pub enum AleTxCommand {
        /// One individual call.
        Call = ALE_CALL_TX.command,
        /// One sounding.
        Sound = ALE_SOUND,
        /// Ends the transmission.
        Stop = ALE_STOP,
        /// An AMD text.
        Amd = ALE_AMD.command,
    }
}

/// What is not built, in the result fields.
pub const TX_NOTE: &str = "Sends calls with a scanning part, soundings, AMD texts and the response and acknowledgement \
of the three-way handshake on one channel. Not built: scanning a list of channels, soundings on a schedule, the \
multipath figure of LQA, and listening before transmitting. The timing of the handshake is a setting: the \
specification marks the timing of the standard as not confirmed.";

/// The time from the last word of a transmission to the moment the receiver of the stack closes it: two words.
const CLOSE_S: f64 = 2.0 * WORD_S;

/// One transmission: its words as tones, the samples written, and its end, s.
struct Burst {
    kind: AleTxKind,
    to: String,
    tones: Vec<u8>,
    words: usize,
    end: f64,
    n: u64,
    stopped: bool,
    /// The answer this transmission waits for: its length, s. `None` for none.
    answer_s: Option<f64>,
}

impl Burst {
    /// The samples of the whole transmission at `rate`.
    fn total(&self, rate: f64) -> u64 {
        (self.end * rate).ceil() as u64
    }
}

/// A transmission that waits for the end of the one on air.
struct Planned {
    kind: AleTxKind,
    to: String,
    words: Vec<u32>,
    answer_s: Option<f64>,
}

/// The state of the three-way handshake.
struct Link {
    state: AleLinkState,
    station: String,
    reason: String,
    /// The session time at which the answer is late, s.
    deadline: Option<f64>,
}

/// The words of an individual call from `own` to `to` with the called address sent `repeats` times.
pub fn call_words(own: &str, to: &str, repeats: usize) -> Vec<u32> {
    super::gen::individual(own, to, false, None, false, repeats)
        .into_iter()
        .flat_map(|t| t.words)
        .collect()
}

/// The words of a sounding of `own`, TIS first when `tis`, the address sent `repeats` times.
pub fn sounding_words(own: &str, tis: bool, repeats: usize) -> Vec<u32> {
    super::gen::sounding(own, tis, repeats)
        .into_iter()
        .flat_map(|t| t.words)
        .collect()
}

/// The tones of `words`, 49 a word, the first tone first.
pub fn tones_of(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|&w| word::word_tones(w)).collect()
}

/// The times the address `to` is sent so that the scanning part lasts at least `channels` x `dwell_s` seconds: 0 for no
/// channels.
pub fn scan_times(to: &str, channels: usize, dwell_s: f64) -> usize {
    if channels == 0 {
        return 0;
    }
    let per = address_words(TO, to).len() as f64 * WORD_S;
    (channels as f64 * dwell_s / per - 1e-9).ceil().max(1.0) as usize
}

/// Whether `c` is addressed to the address `own`.
fn addressed(c: &AleCall, own: &str) -> bool {
    c.to.iter().any(|t| t == own)
}

/// The transmitter. See the module. Input `in` is the session's samples (their rate, pace and dial); input `calls` the
/// calls the receiver of the stack hears (`ale.call`); output `out` the complex baseband for `tx.sink` centred on the
/// dial; output `events` the `ale.tx` and `ale.link` events. Each pass writes as many samples as the pass brings in; the
/// last sample carries the end mark. `stop` ends the transmission with a 2 ms ramp and ends the handshake; the
/// transmitter going off and a new format end the transmission at once with the end mark on the next pass. On a live
/// radio whose host times bursts (`HostStreams::transmit_lead`), the first sample states its time on air
/// (`TagKind::BurstStart`).
pub struct AleTransmitter {
    air: Air,
    own: String,
    repeats: usize,
    sounding: SoundingWord,
    scan_channels: usize,
    scan_dwell_s: f64,
    auto_answer: bool,
    lqa: bool,
    answer_wait_s: f64,
    burst: Option<Burst>,
    planned: Option<Planned>,
    link: Link,
    /// The session time, s: the samples of `in` since the start.
    now: f64,
    phase: f64,
    events: Vec<Event>,
    results: Published,
}

impl Default for AleTransmitter {
    fn default() -> Self {
        let mut t = AleTransmitter {
            air: Air::default(),
            own: String::new(),
            repeats: CALL_REPEATS.default as usize,
            sounding: SoundingWord::Tws,
            scan_channels: 0,
            scan_dwell_s: SCAN_DWELL_S.default,
            auto_answer: AUTO_ANSWER.default,
            lqa: LQA_EXCHANGE.default,
            answer_wait_s: ANSWER_WAIT_S.default,
            burst: None,
            planned: None,
            link: Link {
                state: AleLinkState::Idle,
                station: String::new(),
                reason: String::new(),
                deadline: None,
            },
            now: 0.0,
            phase: 0.0,
            events: Vec::new(),
            results: Published::new(AleTxFields::SHAPE.fields),
        };
        t.publish();
        t
    }
}

impl AleTransmitter {
    fn event(&mut self, state: &str, reason: &str, kind: AleTxKind, to: &str) {
        self.events.push(ALE_TX.event(
            0.0,
            self.air.dial,
            &AleTxEvent {
                state,
                reason,
                kind,
                to,
                family: Family::Message,
            },
        ));
    }

    /// Sets the link state and reports the step as an `ale.link` event.
    fn step(&mut self, state: AleLinkState, station: &str, reason: &str) {
        self.link.state = state;
        station.clone_into(&mut self.link.station);
        reason.clone_into(&mut self.link.reason);
        if state != AleLinkState::Calling && state != AleLinkState::Responding {
            self.link.deadline = None;
        }
        self.events.push(ALE_LINK.event(
            0.0,
            self.air.dial,
            &AleLinkEvent {
                state,
                source: station,
                reason,
                family: Family::Message,
            },
        ));
    }

    /// Ends the transmission at once; its end mark goes out on the next pass.
    fn cut(&mut self, reason: &str) {
        self.planned = None;
        if let Some(b) = self.burst.take() {
            self.air.owe_end();
            self.event("finished", reason, b.kind, &b.to);
        }
        if matches!(
            self.link.state,
            AleLinkState::Calling | AleLinkState::Responding
        ) {
            let station = std::mem::take(&mut self.link.station);
            self.step(AleLinkState::NotAnswered, &station, reason);
        }
    }

    /// The words of a call from the own address to `to` with the AMD text `amd`: the scanning part, the calling cycle,
    /// the LQA request when set, the text, then TIS (TWS after the allcall).
    fn call_frame(&self, to: &str, amd: Option<&str>) -> Vec<u32> {
        let allcall = to == "@?@";
        let times = scan_times(to, self.scan_channels, self.scan_dwell_s) + self.repeats;
        let lqa = (self.lqa && !allcall).then_some(Lqa::REQUEST);
        frame_words(
            to,
            times,
            lqa,
            amd,
            if allcall { TWS } else { TIS },
            &self.own,
        )
    }

    /// The length of an answer of the station `other` to the own address with an LQA word when `lqa`, ending in
    /// `conclusion`, s.
    fn answer_s(&self, other: &str, lqa: bool, conclusion: u8) -> f64 {
        let w = frame_words(
            &self.own,
            2,
            lqa.then_some(Lqa::REQUEST),
            None,
            conclusion,
            other,
        );
        w.len() as f64 * WORD_S
    }

    /// Puts `words` on the air (or after the transmission on air).
    fn send(&mut self, kind: AleTxKind, to: &str, words: Vec<u32>, answer_s: Option<f64>) {
        if self.burst.is_some() {
            self.planned = Some(Planned {
                kind,
                to: to.to_owned(),
                words,
                answer_s,
            });
            return;
        }
        let tones = tones_of(&words);
        self.burst = Some(Burst {
            kind,
            end: tones.len() as f64 * SYMBOL_S,
            tones,
            words: words.len(),
            to: to.to_owned(),
            n: 0,
            stopped: false,
            answer_s,
        });
        self.event("started", "", kind, to);
    }

    /// Starts a transmission of `kind` from a command; `to` is the called address (empty: the allcall for a call, the
    /// linked station for an AMD text), `amd` the text of an AMD.
    ///
    /// # Errors
    ///
    /// [`TxError`]: the transmitter is off, a transmission runs, the rate is too low, the own address is empty, an
    /// address or the text cannot be sent, or an AMD text has no station to go to.
    pub fn start(&mut self, kind: AleTxKind, to: &str, amd: &str) -> Result<String, TxError> {
        self.air.ready()?;
        if self.burst.is_some() || self.planned.is_some() {
            return Err(TxError::Busy);
        }
        if self.own.is_empty() {
            return Err(TxError::Empty(SELF_ADDRESS.key()));
        }
        let to = to.trim().to_ascii_uppercase();
        match kind {
            AleTxKind::Sounding => {
                let words =
                    sounding_words(&self.own, self.sounding == SoundingWord::Tis, self.repeats);
                self.send(kind, "", words, None);
                Ok(String::new())
            }
            AleTxKind::Amd => {
                let text = sendable_amd(amd).ok_or(TxError::Invalid("text"))?;
                let linked = self.link.state == AleLinkState::Linked;
                let to = if to.is_empty() && linked {
                    self.link.station.clone()
                } else {
                    to
                };
                if to.is_empty() {
                    return Err(TxError::NoLink);
                }
                if !sendable_address(&to) {
                    return Err(TxError::Invalid("to"));
                }
                if linked && to == self.link.station {
                    let words = frame_words(&to, 2, None, Some(&text), TWS, &self.own);
                    self.send(kind, &to, words, None);
                } else {
                    let words = self.call_frame(&to, Some(&text));
                    let answer = self.answer_s(&to, self.lqa, TIS);
                    self.send(kind, &to, words, Some(answer));
                    self.step(AleLinkState::Calling, &to, "call sent");
                }
                Ok(to)
            }
            _ => {
                let to = if to.is_empty() { "@?@".to_owned() } else { to };
                let allcall = to == "@?@";
                if !allcall && !sendable_address(&to) {
                    return Err(TxError::Invalid("to"));
                }
                let words = self.call_frame(&to, None);
                if allcall {
                    self.send(AleTxKind::Call, &to, words, None);
                } else {
                    let answer = self.answer_s(&to, self.lqa, TIS);
                    self.send(AleTxKind::Call, &to, words, Some(answer));
                    self.step(AleLinkState::Calling, &to, "call sent");
                }
                Ok(to)
            }
        }
    }

    /// Ends the transmission with its ramp, drops a planned one and ends the handshake or the link.
    pub fn stop(&mut self) {
        let rate = self.air.rate.max(1.0);
        self.planned = None;
        if let Some(b) = &mut self.burst {
            b.end = b.end.min(b.n as f64 / rate + RAMP_S);
            b.stopped = true;
            b.answer_s = None;
        }
        if self.link.state != AleLinkState::Idle {
            let station = std::mem::take(&mut self.link.station);
            self.step(AleLinkState::Idle, &station, "stopped");
        }
    }

    /// Answers `c` when the handshake asks for it: the acknowledgement of the response to the own call, the end of the
    /// handshake on the acknowledgement of the own response, the response to a call to the own address.
    fn heard(&mut self, c: &AleCall) {
        if self.own.is_empty()
            || c.from.is_empty()
            || c.from == self.own
            || !addressed(c, &self.own)
        {
            return;
        }
        let from = c.from.clone();
        let peer = self.link.station == from;
        match self.link.state {
            AleLinkState::Calling if peer && self.link.deadline.is_some() => {
                if let Err(e) = self.air.ready() {
                    let why = format!("the response came; no acknowledgement: {e}");
                    self.step(AleLinkState::NotAnswered, &from, &why);
                    return;
                }
                let lqa = (self.lqa || c.lqa.is_some()).then(|| c.measured());
                let words = frame_words(&from, 2, lqa, None, TWS, &self.own);
                self.link.deadline = None;
                self.step(
                    AleLinkState::Calling,
                    &from,
                    "response heard, acknowledgement sent",
                );
                self.send(AleTxKind::Ack, &from, words, None);
            }
            AleLinkState::Responding if peer => {
                self.step(AleLinkState::Linked, &from, "acknowledgement heard");
            }
            AleLinkState::Calling | AleLinkState::Responding => {}
            _ => {
                if !c.tis || c.kind != CallKind::Individual || c.to.len() != 1 {
                    return;
                }
                if !self.auto_answer {
                    self.step(
                        AleLinkState::NotAnswered,
                        &from,
                        "call heard; automatic answer is off",
                    );
                    return;
                }
                if let Err(e) = self.air.ready() {
                    let why = format!("call heard; no response: {e}");
                    self.step(AleLinkState::NotAnswered, &from, &why);
                    return;
                }
                let lqa = (self.lqa || c.lqa.is_some()).then(|| c.measured());
                let words = frame_words(&from, 2, lqa, None, TIS, &self.own);
                let answer = self.answer_s(&from, lqa.is_some(), TWS);
                self.step(AleLinkState::Responding, &from, "call heard, response sent");
                self.send(AleTxKind::Response, &from, words, Some(answer));
            }
        }
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let rate = self.air.rate.max(1.0);
        let l = &self.link;
        let station =
            (l.state != AleLinkState::Idle && !l.station.is_empty()).then_some(l.station.as_str());
        let f = match &self.burst {
            None => AleTxFields {
                tx_sending: false,
                tx_kind: None,
                tx_to: None,
                tx_word: None,
                tx_words: None,
                tx_note: TX_NOTE,
                tx_link: l.state,
                tx_link_station: station,
                tx_link_reason: &l.reason,
            },
            Some(b) => {
                let at = ((b.n as f64 / rate / WORD_S) as usize + 1).min(b.words);
                AleTxFields {
                    tx_sending: true,
                    tx_kind: Some(b.kind),
                    tx_to: (b.kind != AleTxKind::Sounding).then_some(b.to.as_str()),
                    tx_word: Some(at as i64),
                    tx_words: Some(b.words as i64),
                    tx_note: TX_NOTE,
                    tx_link: l.state,
                    tx_link_station: station,
                    tx_link_reason: &l.reason,
                }
            }
        };
        self.results.put(&f);
    }

    /// The end of the transmission `b` on air: the wait for its answer, or the link.
    fn ended(&mut self, b: Burst) {
        let reason = if b.stopped { "stopped" } else { "" };
        self.event("finished", reason, b.kind, &b.to);
        if b.stopped {
            return;
        }
        if let Some(a) = b.answer_s {
            self.link.deadline = Some(self.now + self.answer_wait_s + a + CLOSE_S);
        } else if b.kind == AleTxKind::Ack {
            self.step(AleLinkState::Linked, &b.to, "acknowledgement sent");
        }
    }

    fn run(&mut self, io: &mut Io) {
        self.air.settle(io);
        let calls: Vec<AleCall> = io
            .items::<AleCall>(1)
            .iter()
            .map(|c| c.value.clone())
            .collect();
        let mut changed = !calls.is_empty();
        for c in &calls {
            self.heard(c);
        }
        let room = io.input_len(0);
        let rate = self.air.rate;
        if rate > 0.0 {
            self.now += room as f64 / rate;
        }
        if let Some(d) = self.link.deadline {
            if self.burst.is_none() && self.now > d {
                let station = std::mem::take(&mut self.link.station);
                let why = if self.link.state == AleLinkState::Calling {
                    "no response"
                } else {
                    "no acknowledgement"
                };
                self.step(AleLinkState::TimedOut, &station, why);
                changed = true;
            }
        }
        if self.burst.is_none() {
            if let Some(p) = self.planned.take() {
                self.send(p.kind, &p.to, p.words, p.answer_s);
            }
        }
        let stated = self
            .burst
            .as_ref()
            .filter(|b| b.n == 0)
            .and_then(|_| self.air.start_time(room));
        let mut ended = false;
        if let Some(b) = &mut self.burst {
            let total = b.total(rate);
            let k = (room as u64).min(total.saturating_sub(b.n)) as usize;
            let out = io.iq_out(0);
            let first = out.len();
            let fresh = b.n == 0;
            for _ in 0..k {
                let dt = b.n as f64 / rate;
                let z = tone_sample(&b.tones, dt, b.end, rate, f64::from(LEVEL), &mut self.phase);
                out.push(z);
                b.n += 1;
            }
            if k > 0 && fresh {
                if let Some(unix_s) = stated {
                    start_mark(io, 0, first, unix_s);
                }
            }
            ended = b.n >= total;
        }
        if ended {
            end_mark(io, 0);
            if let Some(b) = self.burst.take() {
                self.ended(b);
            }
        }
        if io.tick() || ended || changed {
            self.publish();
        }
        io.messages_out(1).append(&mut self.events);
    }
}

impl Component for AleTransmitter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.ale_tx",
            vec![Port::iq("in"), Port::typed::<AleCall>("calls")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(AleTxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let out = self.air.configure(&inputs[0]);
        self.cut("the format changed");
        if self.link.state == AleLinkState::Linked {
            let station = std::mem::take(&mut self.link.station);
            self.step(AleLinkState::Idle, &station, "the format changed");
        }
        self.publish();
        Ok(vec![out, Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.air.attach(ctx);
        if !ctx.transmit {
            self.cut("the transmitter is off");
            self.publish();
        }
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        if self.burst.is_some() {
            self.cut("the input ended");
            self.air.settle(io);
        }
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AleTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AleTxSetting::SelfAddress => {
                let a = SELF_ADDRESS.read(id, v)?.trim().to_ascii_uppercase();
                if !a.is_empty() && !sendable_address(&a) {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.own = a;
            }
            AleTxSetting::CallRepeats => self.repeats = CALL_REPEATS.read(id, v)?.max(1) as usize,
            AleTxSetting::Sounding => self.sounding = SOUNDING.read(id, v)?,
            AleTxSetting::ScanChannels => {
                self.scan_channels = SCAN_CHANNELS.read(id, v)?.max(0) as usize;
            }
            AleTxSetting::ScanDwell => self.scan_dwell_s = SCAN_DWELL_S.read(id, v)?,
            AleTxSetting::AutoAnswer => self.auto_answer = AUTO_ANSWER.read(id, v)?,
            AleTxSetting::Lqa => self.lqa = LQA_EXCHANGE.read(id, v)?,
            AleTxSetting::AnswerWait => self.answer_wait_s = ANSWER_WAIT_S.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (AleTxSetting::SelfAddress, Value::Text(self.own.clone())),
            (AleTxSetting::CallRepeats, Value::I64(self.repeats as i64)),
            (AleTxSetting::Sounding, Value::Choice(self.sounding.index())),
            (
                AleTxSetting::ScanChannels,
                Value::I64(self.scan_channels as i64),
            ),
            (AleTxSetting::ScanDwell, Value::F64(self.scan_dwell_s)),
            (AleTxSetting::AutoAnswer, Value::Bool(self.auto_answer)),
            (AleTxSetting::Lqa, Value::Bool(self.lqa)),
            (AleTxSetting::AnswerWait, Value::F64(self.answer_wait_s)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        AleTxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let (kind, to, text) =
            match AleTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
                AleTxCommand::Call => {
                    let a: AleCallArgs = ALE_CALL_TX.args(args)?;
                    (AleTxKind::Call, a.to.to_owned(), String::new())
                }
                AleTxCommand::Sound => (AleTxKind::Sounding, String::new(), String::new()),
                AleTxCommand::Amd => {
                    let a: AleAmdArgs = ALE_AMD.args(args)?;
                    (AleTxKind::Amd, a.to.to_owned(), a.text.to_owned())
                }
                AleTxCommand::Stop => {
                    self.stop();
                    self.publish();
                    return Ok(None);
                }
            };
        if let Err(e) = self.start(kind, &to, &text) {
            self.event("refused", &e.to_string(), kind, &to);
            self.publish();
            return Err(CommandError::Failed {
                source: StageError::new(e),
            });
        }
        self.publish();
        Ok(None)
    }
}
