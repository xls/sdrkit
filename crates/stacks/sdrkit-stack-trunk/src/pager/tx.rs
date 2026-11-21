//! The POCSAG sender of the pager stack (`pager.tx`): pages coded as ITU-R M.584-2 batches and sent as 2-FSK on an FM
//! carrier at the tuned frequency. The code words come from `sdrkit-proto-pager` through the stack's test signal path
//! ([`super::gen::pocsag_words`], [`super::gen::pocsag_symbols`]): a preamble of 576 reversals, then batches of the sync word
//! and 8 frames of 2 code words, each address word in the frame of the 3 low bits of its address, the message words after
//! it over as many batches as they need, and idle words as fill. With normal polarity a 1 is the lower frequency. Each
//! change of frequency is a raised sine over 30 % of a bit either side of the boundary; the carrier rises and falls with a
//! 1 ms raised cosine. FLEX is not sent: it is a licensed paging service.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Output `out` is complex baseband at the session rate for
//! `tx.sink`, the carrier at 0 Hz, the envelope constant at `tx_level`. Output `events` carries the `pager.tx` events. Each
//! transmission ends with an end mark: at its end, after `halt`, the transmitter going off and a new input format. On a
//! live radio whose host gives a transmit lead, the first sample states its time on air. The engine's transmit setting is
//! the only switch.

use super::gen::{pocsag_symbols, pocsag_words, shaped_freq, SymClock, TestPage, Tx};
use super::tables::PagerProtocol;
use crate::typed::{PagerSendArgs, PagerTxEvent, PagerTxFields, PAGER_HALT, PAGER_SEND, PAGER_TX};
use sdrkit_algo_sim::modulate::Fm;
use sdrkit_components::tx::{end_mark, BurstOut, BurstTiming, Ramp};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Complex32, ConfigError, Event, Record, Results, Setting, SettingError, SettingId,
    SettingSet, StageError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
};
use sdrkit_proto_pager::pocsag::{IDLE, MAX_RIC};
use sdrkit_proto_pager::Kind as PageKind;
use std::collections::VecDeque;

/// The section of the transmit settings.
pub const TX: &str = "Transmit";
/// The most pages that wait for the next transmission.
pub const MAX_WAITING: usize = 16;
/// The most characters of one page.
pub const MAX_TEXT: usize = 1000;
/// The rise and the fall of the carrier, seconds.
pub const RAMP_S: f64 = 0.001;
/// The samples made at one time.
const CHUNK: usize = 1024;

sdrkit_core::choice_enum! {
    /// What a page carries.
    #[derive(Default)]
    pub enum SendType {
        /// The address word only: an alert.
        Tone = ("tone", "Tone only"),
        /// 4-bit characters.
        Numeric = ("numeric", "Numeric"),
        /// 7-bit characters.
        #[default]
        Alpha = ("alpha", "Alphanumeric"),
    }
}

sdrkit_core::choice_enum! {
    /// The bit rate of POCSAG.
    #[derive(Default)]
    pub enum TxRate {
        /// 512 bit/s.
        B512 = ("512", "512 bit/s"),
        /// 1200 bit/s.
        #[default]
        B1200 = ("1200", "1200 bit/s"),
        /// 2400 bit/s.
        B2400 = ("2400", "2400 bit/s"),
    }
}

impl TxRate {
    /// The bits per second.
    pub const fn bits_per_s(self) -> f64 {
        match self {
            TxRate::B512 => 512.0,
            TxRate::B1200 => 1200.0,
            TxRate::B2400 => 2400.0,
        }
    }
}

sdrkit_core::choice_enum! {
    /// Which frequency carries a 1.
    #[derive(Default)]
    pub enum TxPolarity {
        /// A 1 is the lower frequency (ITU-R M.584-2).
        #[default]
        Normal = ("normal", "Normal (1 is the lower frequency)"),
        /// A 1 is the upper frequency.
        Inverted = ("inverted", "Inverted (1 is the upper frequency)"),
    }
}

/// What a page carries: the argument `page_type` of `send`.
pub const PAGE_TYPE: Setting<field::Choice<SendType>> =
    Setting::choice("page_type", "Page", SendType::Alpha).help(
        "Tone only, numeric (4-bit characters) or alphanumeric (7-bit characters). An empty text sends a tone-only page.",
    );
/// The protocol sent.
pub const PROTOCOL: Setting<field::Choice<PagerProtocol>> =
    Setting::choice("protocol", "Protocol", PagerProtocol::Pocsag)
        .help("The protocol that send uses. Only POCSAG is sent: FLEX is a licensed paging service, and send refuses it.")
        .section(TX);
/// The bit rate sent.
pub const POCSAG_RATE: Setting<field::Choice<TxRate>> =
    Setting::choice("pocsag_rate", "Speed", TxRate::B1200)
        .help("The POCSAG bit rate sent: 512, 1200 or 2400 bit/s.")
        .section(TX);
/// The peak deviation sent.
pub const DEVIATION: Setting<field::Hz> = Setting::new("deviation", "Deviation", "Hz", 4500.0)
    .range(2000.0, 6000.0, 100.0)
    .help("The frequency of a steady bit from the carrier: 4500 Hz in ITU-R M.584-2.")
    .section(TX);
/// The polarity sent.
pub const POLARITY: Setting<field::Choice<TxPolarity>> =
    Setting::choice("polarity", "Polarity", TxPolarity::Normal)
        .help("Normal sends a 1 as the lower frequency; some transmitters invert it.")
        .section(TX);
/// The peak of the signal sent.
pub const TX_LEVEL: Setting<field::F64> = Setting::new("tx_level", "Level", "of full scale", 0.5)
    .range(0.05, 0.9, 0.0)
    .help("The magnitude of the signal sent, of full scale.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum PagerTxSetting {
        /// The protocol sent.
        Protocol = PROTOCOL,
        /// The bit rate.
        Rate = POCSAG_RATE,
        /// The deviation.
        Deviation = DEVIATION,
        /// The polarity.
        Polarity = POLARITY,
        /// The level.
        Level = TX_LEVEL,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum PagerTxCommand {
        /// Sends a page.
        Send = PAGER_SEND.command,
        /// Ends the transmission.
        Halt = PAGER_HALT,
    }
}

/// Why a page is not sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit setting is off.
    Off,
    /// The setting `protocol` selects FLEX.
    Flex,
    /// The address is not 0 to 2 097 151.
    Address(i64),
    /// The function is not 0 to 3.
    Function(i64),
    /// With function 0 the address word of the address is the idle word, which a pager ignores.
    IdleWord,
    /// A tone-only page has a text.
    ToneText,
    /// The page type cannot carry this character.
    Character(char),
    /// The text is longer than [`MAX_TEXT`] characters.
    Long,
    /// [`MAX_WAITING`] pages wait already.
    Waiting,
    /// The stack has no input yet, so it has no rate.
    NoInput,
    /// The encoder refused the page.
    Code(sdrkit_proto_pager::Error),
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::Flex => f.write_str(
                "FLEX is a licensed paging service and is not sent: set protocol to POCSAG",
            ),
            TxError::Address(a) => write!(f, "the address {a} is not 0 to {MAX_RIC}"),
            TxError::Function(v) => write!(f, "the function {v} is not 0 to 3"),
            TxError::IdleWord => f.write_str(
                "with function 0 the address word of this address is the idle word: use another function",
            ),
            TxError::ToneText => f.write_str("a tone-only page carries no text"),
            TxError::Character(c) => write!(f, "the character {c:?} cannot be sent in this page type"),
            TxError::Long => write!(f, "the text is longer than {MAX_TEXT} characters"),
            TxError::Waiting => write!(f, "{MAX_WAITING} pages wait already"),
            TxError::NoInput => f.write_str("the stack has no input yet"),
            TxError::Code(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TxError {}

/// The page that `send` queues for `address`, `function`, `kind` and `text`. An empty text is a tone-only page. Numeric text
/// takes `)` and `(` for `]` and `[` (the same values 14 and 15).
///
/// # Errors
///
/// [`TxError`] for an address, a function or a character that cannot be sent.
pub fn page(address: i64, function: i64, kind: SendType, text: &str) -> Result<TestPage, TxError> {
    let ric = u32::try_from(address)
        .ok()
        .filter(|&a| a <= MAX_RIC)
        .ok_or(TxError::Address(address))?;
    let function = u8::try_from(function)
        .ok()
        .filter(|&f| f <= 3)
        .ok_or(TxError::Function(function))?;
    if sdrkit_proto_pager::pocsag::address_word(ric, function).ok() == Some(IDLE) {
        return Err(TxError::IdleWord);
    }
    if text.chars().count() > MAX_TEXT {
        return Err(TxError::Long);
    }
    let set = sdrkit_proto_pager::pocsag::NumericSet::Itu;
    let (kind, text) = match kind {
        _ if text.is_empty() => (PageKind::Tone, String::new()),
        SendType::Tone => return Err(TxError::ToneText),
        SendType::Numeric => {
            let mut s = String::with_capacity(text.len());
            for c in text.chars() {
                let c = match c {
                    ')' => ']',
                    '(' => '[',
                    c => c,
                };
                set.value(c).ok_or(TxError::Character(c))?;
                s.push(c);
            }
            (PageKind::Numeric, s)
        }
        SendType::Alpha => {
            if let Some(c) = text
                .chars()
                .find(|c| !c.is_ascii() || (c.is_ascii_control() && !matches!(c, '\n' | '\r')))
            {
                return Err(TxError::Character(c));
            }
            (PageKind::Alpha, text.to_owned())
        }
    };
    Ok(TestPage {
        address: ric,
        function,
        kind,
        text,
    })
}

/// The transmit settings as the sender reads them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxConfig {
    /// The protocol.
    pub protocol: PagerProtocol,
    /// The bit rate.
    pub rate: TxRate,
    /// The deviation, Hz.
    pub deviation_hz: f64,
    /// The polarity.
    pub polarity: TxPolarity,
    /// The level, of full scale.
    pub level: f64,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            protocol: PROTOCOL.default,
            rate: POCSAG_RATE.default,
            deviation_hz: DEVIATION.default,
            polarity: POLARITY.default,
            level: TX_LEVEL.default,
        }
    }
}

/// One transmission.
struct Burst {
    tx: Tx,
    clk: SymClock,
    fm: Fm,
    ramp: Ramp,
    /// The samples of the whole signal.
    total: u64,
    /// The samples made.
    made: u64,
    /// The time from which the carrier falls after `halt`, s.
    halt_at: Option<f64>,
    out: BurstOut,
    pages: usize,
    targets: String,
}

impl Burst {
    /// Makes samples until `room` output samples wait, or the rest of the signal with its fall.
    fn fill(
        &mut self,
        room: usize,
        rate: f64,
        level: f32,
        freq: &mut Vec<f32>,
        base: &mut Vec<Complex32>,
    ) {
        let fall = self.ramp.samples() as u64;
        while self.out.wants(room) {
            freq.clear();
            while freq.len() < CHUNK {
                let n = self.made + freq.len() as u64;
                if n >= self.total {
                    break;
                }
                self.clk.t = n as f64 / rate;
                freq.push(shaped_freq(&mut self.clk, &self.tx) as f32);
            }
            base.clear();
            base.resize(freq.len(), Complex32::default());
            // The two slices have the same length, so the modulator cannot fail.
            let _ = self.fm.process(freq, base);
            let mut end = base.len();
            for (i, z) in base.iter_mut().enumerate() {
                let n = self.made + i as u64;
                let halted = self.halt_at.is_some_and(|h| n as f64 / rate >= h);
                if !self.ramp.falling() && (n + fall >= self.total || halted) {
                    self.ramp.fall();
                }
                match self.ramp.gain(level) {
                    Some(g) => *z *= g,
                    None => {
                        end = i;
                        break;
                    }
                }
            }
            let done = end < freq.len() || self.made + end as u64 >= self.total;
            base.truncate(end);
            self.made += end as u64;
            self.out.push(base);
            if done {
                self.out.finish(base);
                break;
            }
        }
    }

    /// The time at which the bit on air ends, s.
    fn bit_end(&self) -> f64 {
        let k = self.clk.k.min(self.tx.symbols.len().saturating_sub(1));
        self.clk.k_start + self.tx.symbols.get(k).map_or(0.0, |s| s.1)
    }
}

/// The sender (`pager.tx`). See the module.
pub struct PagerSender {
    cfg: TxConfig,
    rate: f64,
    dial: f64,
    transmit: bool,
    timing: BurstTiming,
    queue: VecDeque<TestPage>,
    burst: Option<Burst>,
    end_owed: bool,
    events: Vec<Event>,
    results: Published,
    shown: Option<(bool, usize, usize)>,
    freq: Vec<f32>,
    base: Vec<Complex32>,
}

impl Default for PagerSender {
    fn default() -> Self {
        let mut s = PagerSender {
            cfg: TxConfig::default(),
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            timing: BurstTiming::default(),
            queue: VecDeque::new(),
            burst: None,
            end_owed: false,
            events: Vec::new(),
            results: Published::new(PagerTxFields::SHAPE.fields),
            shown: None,
            freq: Vec::new(),
            base: Vec::new(),
        };
        s.publish();
        s
    }
}

impl PagerSender {
    fn event(&mut self, state: &str, reason: &str, target: &str, text: &str) {
        self.events.push(PAGER_TX.event(
            0.0,
            self.dial,
            &PagerTxEvent {
                state,
                reason,
                target,
                text,
                family: Family::Pager,
            },
        ));
    }

    /// Drops the waiting pages and the transmission on air; its end mark goes out on the next pass when samples of it
    /// were written.
    fn cut(&mut self, reason: &str) {
        self.queue.clear();
        if let Some(b) = self.burst.take() {
            self.end_owed |= b.out.sent() > 0;
            self.event("finished", reason, &b.targets, "");
        }
    }

    /// Starts a transmission of every waiting page.
    fn start(&mut self) -> Result<(), TxError> {
        let pages: Vec<TestPage> = self.queue.drain(..).collect();
        let words = pocsag_words(&pages).map_err(TxError::Code)?;
        let tx = pocsag_symbols(
            &words,
            self.cfg.rate.bits_per_s(),
            self.cfg.deviation_hz,
            self.cfg.polarity == TxPolarity::Inverted,
        );
        let len_s: f64 = tx.symbols.iter().map(|s| s.1).sum();
        let fm = Fm::new(self.rate, 1.0).map_err(|_| TxError::NoInput)?;
        let out = BurstOut::new(self.rate, self.rate, 1.0).map_err(|_| TxError::NoInput)?;
        let targets = pages
            .iter()
            .map(|p| p.address.to_string())
            .collect::<Vec<_>>()
            .join(",");
        self.burst = Some(Burst {
            tx,
            clk: SymClock::default(),
            fm,
            ramp: Ramp::of_seconds(RAMP_S, self.rate),
            total: (len_s * self.rate).round() as u64,
            made: 0,
            halt_at: None,
            out,
            pages: pages.len(),
            targets,
        });
        Ok(())
    }

    /// Checks the page of `a` and queues it; starts a transmission when none is on air. True when it started one.
    fn send(&mut self, a: &PagerSendArgs) -> Result<bool, TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.cfg.protocol == PagerProtocol::Flex {
            return Err(TxError::Flex);
        }
        let p = page(
            a.address,
            a.function.unwrap_or(3),
            a.page_type.unwrap_or_default(),
            a.text.unwrap_or(""),
        )?;
        if self.rate <= 0.0 {
            return Err(TxError::NoInput);
        }
        if self.queue.len() >= MAX_WAITING {
            return Err(TxError::Waiting);
        }
        self.queue.push_back(p);
        if self.burst.is_none() {
            self.start()?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Drops the waiting pages and ends the transmission after the bit on air with the fall of the carrier.
    fn halt(&mut self) {
        self.queue.clear();
        let Some(b) = &mut self.burst else {
            return;
        };
        if b.made == 0 {
            let b = self.burst.take().expect("a transmission");
            self.event("finished", "halted", &b.targets, "");
            return;
        }
        if b.halt_at.is_none() {
            b.halt_at = Some(b.bit_end());
        }
    }

    /// Writes the result fields when they changed.
    fn publish(&mut self) {
        let now = (
            self.burst.is_some(),
            self.burst.as_ref().map_or(0, |b| b.pages),
            self.queue.len(),
        );
        if self.shown == Some(now) {
            return;
        }
        self.shown = Some(now);
        self.results.put(&PagerTxFields {
            tx_sending: now.0,
            tx_sent: now.1 as i64,
            tx_waiting: now.2 as i64,
        });
    }

    /// One pass: the samples of the transmission and the end mark.
    fn run(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
        }
        if self.burst.is_none() && !self.queue.is_empty() && self.transmit {
            if let Err(e) = self.start() {
                self.queue.clear();
                self.event("refused", &e.to_string(), "", "");
            }
        }
        if let Some(b) = &mut self.burst {
            b.fill(
                room,
                self.rate,
                self.cfg.level as f32,
                &mut self.freq,
                &mut self.base,
            );
            if b.out.write(io, 0, room, &self.timing) {
                let reason = if b.halt_at.is_some() { "halted" } else { "" };
                let targets = std::mem::take(&mut b.targets);
                self.burst = None;
                self.event("finished", reason, &targets, "");
            }
        }
        self.publish();
        io.messages_out(1).append(&mut self.events);
    }
}

impl Component for PagerSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "pager.tx",
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(PagerTxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        if self.burst.is_some() || !self.queue.is_empty() {
            self.cut("the format changed");
        }
        Ok(vec![
            Format::iq(self.rate, self.dial, self.dial),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        if !ctx.transmit && (self.burst.is_some() || !self.queue.is_empty()) {
            self.cut("the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        if let Some(b) = self.burst.take() {
            self.end_owed |= b.out.sent() > 0;
            self.event("finished", "the input ended", &b.targets, "");
        }
        if std::mem::take(&mut self.end_owed) {
            end_mark(io, 0);
        }
        self.publish();
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PagerTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PagerTxSetting::Protocol => self.cfg.protocol = PROTOCOL.read(id, v)?,
            PagerTxSetting::Rate => self.cfg.rate = POCSAG_RATE.read(id, v)?,
            PagerTxSetting::Deviation => self.cfg.deviation_hz = DEVIATION.read(id, v)?,
            PagerTxSetting::Polarity => self.cfg.polarity = POLARITY.read(id, v)?,
            PagerTxSetting::Level => self.cfg.level = TX_LEVEL.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                PagerTxSetting::Protocol,
                Value::Choice(self.cfg.protocol.index()),
            ),
            (PagerTxSetting::Rate, Value::Choice(self.cfg.rate.index())),
            (PagerTxSetting::Deviation, Value::Hz(self.cfg.deviation_hz)),
            (
                PagerTxSetting::Polarity,
                Value::Choice(self.cfg.polarity.index()),
            ),
            (PagerTxSetting::Level, Value::F64(self.cfg.level)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        PagerTxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let r = match PagerTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            PagerTxCommand::Send => {
                let a: PagerSendArgs = PAGER_SEND.args(args)?;
                let text = a.text.unwrap_or("");
                let target = a.address.to_string();
                match self.send(&a) {
                    Ok(started) => {
                        let state = if started { "started" } else { "queued" };
                        self.event(state, "", &target, text);
                        Ok(())
                    }
                    Err(e) => {
                        self.event("refused", &e.to_string(), &target, text);
                        Err(e)
                    }
                }
            }
            PagerTxCommand::Halt => {
                self.halt();
                Ok(())
            }
        };
        self.publish();
        r.map(|()| None).map_err(|e| CommandError::Failed {
            source: StageError::new(e),
        })
    }
}
