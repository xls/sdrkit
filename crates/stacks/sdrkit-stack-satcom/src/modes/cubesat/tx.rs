//! The AX.25 sender of the cubesat stack (`cubesat.tx`): UI frames (AX.25 2.2) for an amateur satellite uplink, sent as
//! Bell 202 AFSK at 1200 bit/s or as G3RUH-scrambled FSK at 9600 bit/s on an FM carrier at the tuned frequency, with the
//! deviation of the stack's receiver. The frames, the HDLC framing, the FCS, the bit stuffing and the two modems come from
//! `sdrkit-proto-link`; the source address is the station callsign.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Output `out` is complex baseband at the session rate for
//! `tx.sink`, the carrier at 0 Hz, the envelope constant at `tx_level` between a 5 ms rise and fall. Output `events` carries
//! the `cubesat.tx` events. Each transmission ends with an end mark: at its end, after `halt`, the transmitter going off, a
//! new input format and the end of the input. On a live radio whose host gives a transmit lead, the first sample states its
//! time on air. The engine's transmit setting is the only switch. Doppler correction of the uplink is the host's.

use super::gen::{DEVIATION_HZ, WR};
use crate::typed::{
    CubesatSendArgs, CubesatTxEvent, CubesatTxFields, CUBESAT_HALT, CUBESAT_SEND, CUBESAT_TX,
};
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
use sdrkit_proto_link::ax25::{Address, Control, Frame, MAX_REPEATERS, PID_NONE};
use sdrkit_proto_link::modem::{afsk1200_audio, g3ruh9600_audio};
use sdrkit_util_bits::pack::{hex_to_bits, pack};
use sdrkit_util_bits::{BitOrder, Pad};
use std::collections::VecDeque;

/// The section of the transmit settings.
pub const TX: &str = "Transmit";
/// The most frames that wait for the next transmission.
pub const MAX_WAITING: usize = 16;
/// The most octets of the information field.
pub const MAX_INFO: usize = 256;
/// The largest magnitude of a sample written.
pub const PEAK: f32 = 0.9;
/// The rise and the fall of the carrier, seconds.
pub const RAMP_S: f64 = 0.005;
/// The samples at [`WR`] made at one time.
const CHUNK: usize = 1024;

sdrkit_core::choice_enum! {
    /// The modulation and bit rate sent.
    #[derive(Default)]
    pub enum TxRate {
        /// Bell 202 AFSK, 1200 bit/s.
        #[default]
        Afsk1200 = ("1200", "1200 bit/s AFSK"),
        /// G3RUH-scrambled FSK, 9600 bit/s.
        G3ruh9600 = ("9600", "9600 bit/s G3RUH"),
    }
}

impl TxRate {
    /// The bits per second.
    pub const fn bits_per_s(self) -> f64 {
        match self {
            TxRate::Afsk1200 => 1200.0,
            TxRate::G3ruh9600 => 9600.0,
        }
    }
}

/// The modulation sent.
pub const TX_RATE: Setting<field::Choice<TxRate>> =
    Setting::choice("tx_rate", "Speed", TxRate::Afsk1200)
        .help("1200 bit/s Bell 202 AFSK or 9600 bit/s G3RUH FSK, both on FM with 3 kHz deviation.")
        .section(TX);
/// The flags sent before the first frame.
pub const TX_DELAY: Setting<field::F64> = Setting::new("tx_delay", "Flags before", "ms", 300.0)
    .range(10.0, 1000.0, 10.0)
    .help("The time of HDLC flags before the first frame, for the receiver to lock.")
    .section(TX);
/// The peak of the signal sent.
pub const TX_LEVEL: Setting<field::F64> = Setting::new("tx_level", "Level", "of full scale", 0.5)
    .range(0.05, 0.9, 0.0)
    .help("The magnitude of the signal sent, of full scale.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum CubesatTxSetting {
        /// The modulation.
        Rate = TX_RATE,
        /// The flags before.
        Delay = TX_DELAY,
        /// The level.
        Level = TX_LEVEL,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum CubesatTxCommand {
        /// Sends a frame.
        Send = CUBESAT_SEND.command,
        /// Ends the transmission.
        Halt = CUBESAT_HALT,
    }
}

/// Why a frame is not sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit setting is off.
    Off,
    /// The station callsign is not set.
    NoCall,
    /// The station callsign is not an AX.25 address.
    BadCall(String),
    /// The destination is not an AX.25 address.
    Destination(String),
    /// A digipeater is not an AX.25 address.
    Path(String),
    /// More than [`MAX_REPEATERS`] digipeaters.
    LongPath,
    /// Neither text nor bytes.
    Empty,
    /// Both text and bytes.
    Both,
    /// The bytes are not pairs of hexadecimal digits.
    Hex,
    /// The information is longer than [`MAX_INFO`] octets.
    Long,
    /// The PID is not 0 to 255, or is a value that marks an escape (0xFF) for layer 3.
    Pid(i64),
    /// [`MAX_WAITING`] frames wait already.
    Waiting,
    /// The stack has no input yet, so it has no rate.
    NoInput,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::NoCall => f.write_str("the station callsign is not set"),
            TxError::BadCall(c) => write!(
                f,
                "the station callsign {c:?} is not an AX.25 address (1 to 6 letters or digits, SSID 0 to 15)"
            ),
            TxError::Destination(c) => write!(f, "the destination {c:?} is not an AX.25 address"),
            TxError::Path(c) => write!(f, "the digipeater {c:?} is not an AX.25 address"),
            TxError::LongPath => write!(f, "more than {MAX_REPEATERS} digipeaters"),
            TxError::Empty => f.write_str("the frame has neither text nor bytes"),
            TxError::Both => f.write_str("give either text or bytes, not both"),
            TxError::Hex => f.write_str("the bytes are not pairs of hexadecimal digits"),
            TxError::Long => write!(f, "the information is longer than {MAX_INFO} octets"),
            TxError::Pid(p) => write!(f, "the PID {p} is not 0 to 254"),
            TxError::Waiting => write!(f, "{MAX_WAITING} frames wait already"),
            TxError::NoInput => f.write_str("the stack has no input yet"),
        }
    }
}

impl std::error::Error for TxError {}

/// One AX.25 address from user text: upper case, no repeated mark, `C` and `H` bits clear.
fn address(text: &str) -> Option<Address> {
    let t = text.trim().to_ascii_uppercase();
    if t.ends_with('*') {
        return None;
    }
    let mut a = Address::parse(&t).ok()?;
    a.flag = false;
    Some(a)
}

/// The station's source address from the station callsign `call`.
///
/// # Errors
///
/// [`TxError::NoCall`] for an empty callsign, [`TxError::BadCall`] for one that is not an AX.25 address.
pub fn source(call: &str) -> Result<Address, TxError> {
    let c = call.trim();
    if c.is_empty() {
        return Err(TxError::NoCall);
    }
    address(c).ok_or_else(|| TxError::BadCall(c.to_owned()))
}

/// The information field of `text` or `hex` (hexadecimal digit pairs, spaces allowed).
///
/// # Errors
///
/// [`TxError`] when both or neither are given, the digits are not pairs, or the field is longer than [`MAX_INFO`].
pub fn info(text: &str, hex: &str) -> Result<Vec<u8>, TxError> {
    let digits: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    let out = match (text.is_empty(), digits.is_empty()) {
        (true, true) => return Err(TxError::Empty),
        (false, false) => return Err(TxError::Both),
        (false, true) => text.as_bytes().to_vec(),
        (true, false) => {
            if !digits.len().is_multiple_of(2) {
                return Err(TxError::Hex);
            }
            let mut bits = Vec::with_capacity(digits.len() * 4);
            hex_to_bits(&digits, &mut bits).map_err(|_| TxError::Hex)?;
            let mut b = Vec::with_capacity(digits.len() / 2);
            pack(&bits, BitOrder::Msb, Pad::Drop, &mut b);
            b
        }
    };
    if out.len() > MAX_INFO {
        return Err(TxError::Long);
    }
    Ok(out)
}

/// The bytes (without the FCS) of the UI command frame from `src` to `destination` through `path` (addresses separated by
/// commas) with `pid` and `info`. The destination has the `C` bit set, the source has it clear (AX.25 2.2 clause 6.1.2).
///
/// # Errors
///
/// [`TxError`] for an address that is not an AX.25 address, too many digipeaters or a PID out of range.
pub fn ui_frame(
    src: Address,
    destination: &str,
    path: &str,
    pid: i64,
    info: &[u8],
) -> Result<Vec<u8>, TxError> {
    let dest = if destination.trim().is_empty() {
        "CQ"
    } else {
        destination
    };
    let mut d = address(dest).ok_or_else(|| TxError::Destination(dest.trim().to_owned()))?;
    d.flag = true;
    let mut reps = Vec::new();
    for p in path.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        reps.push(address(p).ok_or_else(|| TxError::Path(p.to_owned()))?);
    }
    if reps.len() > MAX_REPEATERS {
        return Err(TxError::LongPath);
    }
    let pid = u8::try_from(pid)
        .ok()
        .filter(|&p| p != 0xFF)
        .ok_or(TxError::Pid(pid))?;
    let f = Frame::new(d, src, &reps, Control::UI, Some(pid), info).map_err(|_| TxError::LongPath)?;
    let mut out = Vec::new();
    f.encode(&mut out).map_err(|_| TxError::Long)?;
    Ok(out)
}

/// The number of HDLC flags of `ms` milliseconds at `rate` (at least 1).
pub fn flags_of(ms: f64, rate: TxRate) -> usize {
    ((ms / 1000.0 * rate.bits_per_s() / 8.0).ceil() as usize).max(1)
}

/// The FM baseband at [`WR`] with unit magnitude of `frames` sent at `rate` after `flags` flags.
pub fn baseband(frames: &[&[u8]], flags: usize, rate: TxRate) -> Vec<Complex32> {
    let mut audio = Vec::new();
    // The frames were built here and fit the limits of the modems.
    let _ = match rate {
        TxRate::Afsk1200 => afsk1200_audio(frames, flags, WR, &mut audio),
        TxRate::G3ruh9600 => g3ruh9600_audio(frames, flags, WR, &mut audio),
    };
    let mut iq = vec![Complex32::default(); audio.len()];
    if let Ok(mut fm) = Fm::new(WR, DEVIATION_HZ) {
        // The two slices have the same length.
        let _ = fm.process(&audio, &mut iq);
    }
    iq
}

/// The transmit settings as the sender reads them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxConfig {
    /// The modulation.
    pub rate: TxRate,
    /// The flags before, ms.
    pub delay_ms: f64,
    /// The level, of full scale.
    pub level: f64,
}

impl Default for TxConfig {
    fn default() -> Self {
        TxConfig {
            rate: TX_RATE.default,
            delay_ms: TX_DELAY.default,
            level: TX_LEVEL.default,
        }
    }
}

/// A frame waiting: its bytes and what the event shows of it.
struct Waiting {
    bytes: Vec<u8>,
    target: String,
}

/// One transmission.
struct Burst {
    signal: Vec<Complex32>,
    made: usize,
    ramp: Ramp,
    /// The sample at [`WR`] from which the carrier falls after `halt`.
    halt_at: Option<usize>,
    /// The samples at [`WR`] of one bit.
    per_bit: f64,
    out: BurstOut,
    frames: usize,
    targets: String,
}

impl Burst {
    /// Makes samples until `room` output samples wait, or the rest of the signal with its fall.
    fn fill(&mut self, room: usize, level: f32, base: &mut Vec<Complex32>) {
        let fall = self.ramp.samples();
        let total = self.signal.len();
        while self.out.wants(room) {
            let end = (self.made + CHUNK).min(total);
            base.clear();
            for n in self.made..end {
                if !self.ramp.falling()
                    && (n + fall >= total || self.halt_at.is_some_and(|h| n >= h))
                {
                    self.ramp.fall();
                }
                match self.ramp.gain(level) {
                    Some(g) => base.push(self.signal[n] * g),
                    None => break,
                }
            }
            self.made += base.len();
            let done = self.ramp.done() || self.made >= total;
            self.out.push(base);
            if done {
                self.out.finish(base);
                break;
            }
        }
    }

    /// The sample at [`WR`] where the bit on air ends.
    fn bit_end(&self) -> usize {
        let k = (self.made as f64 / self.per_bit).ceil();
        (k * self.per_bit).ceil() as usize
    }
}

/// The sender (`cubesat.tx`). See the module.
pub struct CubesatSender {
    cfg: TxConfig,
    rate: f64,
    dial: f64,
    transmit: bool,
    station: String,
    timing: BurstTiming,
    queue: VecDeque<Waiting>,
    burst: Option<Burst>,
    end_owed: bool,
    events: Vec<Event>,
    results: Published,
    shown: Option<(bool, usize, usize)>,
    base: Vec<Complex32>,
}

impl Default for CubesatSender {
    fn default() -> Self {
        let mut s = CubesatSender {
            cfg: TxConfig::default(),
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            station: String::new(),
            timing: BurstTiming::default(),
            queue: VecDeque::new(),
            burst: None,
            end_owed: false,
            events: Vec::new(),
            results: Published::new(CubesatTxFields::SHAPE.fields),
            shown: None,
            base: Vec::new(),
        };
        s.publish();
        s
    }
}

impl CubesatSender {
    fn event(&mut self, state: &str, reason: &str, target: &str, text: &str) {
        self.events.push(CUBESAT_TX.event(
            0.0,
            self.dial,
            &CubesatTxEvent {
                state,
                reason,
                target,
                text,
                family: Family::Satellite,
            },
        ));
    }

    /// Drops the waiting frames and the transmission on air; its end mark goes out on the next pass when samples of it were
    /// written.
    fn cut(&mut self, reason: &str) {
        self.queue.clear();
        if let Some(b) = self.burst.take() {
            self.end_owed |= b.out.sent() > 0;
            self.event("finished", reason, &b.targets, "");
        }
    }

    /// Starts a transmission of every waiting frame.
    fn start(&mut self) -> Result<(), TxError> {
        let waiting: Vec<Waiting> = self.queue.drain(..).collect();
        let frames: Vec<&[u8]> = waiting.iter().map(|w| w.bytes.as_slice()).collect();
        let signal = baseband(
            &frames,
            flags_of(self.cfg.delay_ms, self.cfg.rate),
            self.cfg.rate,
        );
        let out = BurstOut::new(WR, self.rate, PEAK).map_err(|_| TxError::NoInput)?;
        let targets = waiting
            .iter()
            .map(|w| w.target.as_str())
            .collect::<Vec<_>>()
            .join(",");
        self.burst = Some(Burst {
            signal,
            made: 0,
            ramp: Ramp::of_seconds(RAMP_S, WR),
            halt_at: None,
            per_bit: WR / self.cfg.rate.bits_per_s(),
            out,
            frames: waiting.len(),
            targets,
        });
        Ok(())
    }

    /// Checks the frame of `a` and queues it; starts a transmission when none is on air. True when it started one.
    fn send(&mut self, a: &CubesatSendArgs) -> Result<bool, TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        let src = source(&self.station)?;
        let info = info(a.text.unwrap_or(""), a.hex.unwrap_or(""))?;
        let bytes = ui_frame(
            src,
            a.destination.unwrap_or("CQ"),
            a.path.unwrap_or(""),
            a.pid.unwrap_or(i64::from(PID_NONE)),
            &info,
        )?;
        if self.rate <= 0.0 {
            return Err(TxError::NoInput);
        }
        if self.queue.len() >= MAX_WAITING {
            return Err(TxError::Waiting);
        }
        let target = target_of(a);
        self.queue.push_back(Waiting { bytes, target });
        if self.burst.is_none() {
            self.start()?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Drops the waiting frames and ends the transmission after the bit on air with the fall of the carrier.
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
            self.burst.as_ref().map_or(0, |b| b.frames),
            self.queue.len(),
        );
        if self.shown == Some(now) {
            return;
        }
        self.shown = Some(now);
        self.results.put(&CubesatTxFields {
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
            b.fill(room, self.cfg.level as f32, &mut self.base);
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

/// The destination and path of `a` as the event shows them: `DEST` or `DEST,PATH`.
fn target_of(a: &CubesatSendArgs) -> String {
    let d = a.destination.unwrap_or("CQ").trim();
    let d = if d.is_empty() { "CQ" } else { d };
    match a.path.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => format!("{},{}", d.to_ascii_uppercase(), p.to_ascii_uppercase()),
        None => d.to_ascii_uppercase(),
    }
}

impl Component for CubesatSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "cubesat.tx",
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(CubesatTxSetting::schema())
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
        ctx.station.callsign.trim().clone_into(&mut self.station);
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
        match CubesatTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CubesatTxSetting::Rate => self.cfg.rate = TX_RATE.read(id, v)?,
            CubesatTxSetting::Delay => self.cfg.delay_ms = TX_DELAY.read(id, v)?,
            CubesatTxSetting::Level => self.cfg.level = TX_LEVEL.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (CubesatTxSetting::Rate, Value::Choice(self.cfg.rate.index())),
            (CubesatTxSetting::Delay, Value::F64(self.cfg.delay_ms)),
            (CubesatTxSetting::Level, Value::F64(self.cfg.level)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        CubesatTxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let r = match CubesatTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            CubesatTxCommand::Send => {
                let a: CubesatSendArgs = CUBESAT_SEND.args(args)?;
                let text = match (a.text.unwrap_or(""), a.hex.unwrap_or("")) {
                    (t, "") => t.to_owned(),
                    (_, h) => h.to_owned(),
                };
                let target = target_of(&a);
                match self.send(&a) {
                    Ok(started) => {
                        let state = if started { "started" } else { "queued" };
                        self.event(state, "", &target, &text);
                        Ok(())
                    }
                    Err(e) => {
                        self.event("refused", &e.to_string(), &target, &text);
                        Err(e)
                    }
                }
            }
            CubesatTxCommand::Halt => {
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
