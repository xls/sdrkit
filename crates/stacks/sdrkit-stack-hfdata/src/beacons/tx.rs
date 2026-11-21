//! The own beacon (`hfdata.hfb_tx`): a message in Morse code and a long dash, sent again at a set interval.
//!
//! The beacon is a proposal of the specification, not the behaviour of a standard or of a beacon project: an on-off keyed
//! tone at the pitch above the dial (upper sideband), with raised-cosine edges of 5 ms, at the Morse timing of ITU-R
//! M.1677-1 (dot 1 unit, dash 3, gaps 1, 3 and 7; a unit is 1.2 / wpm s), keyed through the Morse keyer of the library;
//! then the long dash and, when set, four dashes of 1 s at 0, -10, -20 and -30 dB with no gap. Each sending is one
//! transmission with its stated start and end mark; between sendings the transmitter is free. `start` sends at
//! `offset_s` after each interval boundary of UTC (at once when the start falls on that second), and then each interval,
//! counted in the session's samples, until `count` sendings; `send` sends once at once.

use crate::transmit::{Air, TxError, LEVEL};
use crate::typed::{HfbTxEvent, HfbTxFields, HFB_SEND, HFB_START, HFB_STOP, HFB_TX};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::Ook;
use sdrkit_components::tx::{end_mark, start_mark};
use sdrkit_core::interface::{Fields, Seconds};
use sdrkit_core::publish::{field, keys::Family, Published};
use sdrkit_core::{
    Clock, CommandError, CommandId, Commands, Complex32, ConfigError, Event, Record, Results,
    Setting, SettingError, SettingId, StageError, Value, Values,
};
use sdrkit_core::{CommandSet, SettingSet};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
};
use sdrkit_util_text::morse::{Key, Keyer};

/// The section of the transmit settings.
pub const TX_SECTION: &str = "Transmit";
/// The rise and fall of each Morse element (raised cosine), s.
pub const RISE_S: f64 = 0.005;
/// The longest key interval written at one time, so that `stop` acts within it, s.
const PIECE_S: f64 = 0.05;
/// The placeholder of the call sign in the message.
pub const CALL_MARK: &str = "{CALL}";
/// The length of each power step, s.
pub const STEP_S: f64 = 1.0;
/// The levels of the four power steps, dB.
pub const STEPS_DB: [f64; 4] = [0.0, -10.0, -20.0, -30.0];

/// The call sign.
pub const CALL: Setting<field::Text> = Setting::new("call", "Call sign", "", "")
    .help("1 to 12 characters of A-Z, 0-9 and /. Empty refuses start.")
    .section(TX_SECTION);
/// The message.
pub const MESSAGE: Setting<field::Text> = Setting::new("message", "Message", "", "VVV DE {call}")
    .help("Up to 60 characters of A-Z, 0-9, space and / ? . , =. {call} stands for the call sign; a message without it is followed by DE and the call sign; empty sends the call sign alone.")
    .section(TX_SECTION);
/// The speed.
pub const WPM: Setting<field::F64> = Setting::new("wpm", "Speed", "wpm", 22.0)
    .range(5.0, 40.0, 0.0)
    .help("Words a minute of the word PARIS: a dot is 1.2 / wpm s.")
    .section(TX_SECTION);
/// The pitch.
pub const PITCH_HZ: Setting<field::Hz> = Setting::new("pitch_hz", "Tone", "Hz", 800.0)
    .range(300.0, 2500.0, 0.0)
    .help("The tone above the dial: the beacon is on the dial plus this frequency.")
    .section(TX_SECTION);
/// The long dash.
pub const LONG_DASH_S: Setting<field::F64> = Setting::new("long_dash_s", "Long dash", "s", 3.0)
    .range(0.0, 10.0, 0.0)
    .help(
        "A steady carrier after the message, a word gap after it, for level checks. 0 sends none.",
    )
    .section(TX_SECTION);
/// The interval.
pub const INTERVAL_MIN: Setting<field::I64> = Setting::new("interval_min", "Interval", "min", 10)
    .range(1.0, 60.0, 1.0)
    .help("From the start of one sending to the start of the next. A sending longer than the interval is followed at once by the next.")
    .section(TX_SECTION);
/// The power steps.
pub const POWER_STEPS: Setting<field::Bool> = Setting::new("power_steps", "Power steps", "", false)
    .help("Four dashes of 1 s at 0, -10, -20 and -30 dB after the long dash, with no gap, as in the slot of the international beacon project.")
    .section(TX_SECTION);
/// The second of the interval at which `start` sends.
pub const OFFSET_S: Setting<field::F64> = Setting::new("offset_s", "Start in the interval", "s", 0.0)
    .range(0.0, 3599.0, 0.0)
    .help("The sendings of start begin this many seconds after each interval boundary of UTC (an interval of 10 min: on the hour, 10 past, ...). Taken modulo the interval.")
    .section(TX_SECTION);
/// The number of sendings of `start`.
pub const COUNT: Setting<field::I64> = Setting::new("count", "Transmissions", "", 0)
    .range(0.0, 1000.0, 1.0)
    .help("The beacon ends after this many sendings. 0 sends until stop.")
    .section(TX_SECTION);

sdrkit_core::settings! {
    /// The settings of [`BeaconTransmitter`].
    pub enum HfbTxSetting {
        /// The call sign.
        Call = CALL,
        /// The message.
        Message = MESSAGE,
        /// The speed.
        Wpm = WPM,
        /// The pitch.
        Pitch = PITCH_HZ,
        /// The long dash.
        LongDash = LONG_DASH_S,
        /// The interval.
        Interval = INTERVAL_MIN,
        /// The power steps.
        PowerSteps = POWER_STEPS,
        /// The start in the interval.
        Offset = OFFSET_S,
        /// The number of sendings.
        Count = COUNT,
    }
}

sdrkit_core::commands! {
    /// The commands of [`BeaconTransmitter`].
    pub enum HfbTxCommand {
        /// Starts the beacon.
        Start = HFB_START,
        /// Stops the beacon.
        Stop = HFB_STOP,
        /// One sending now.
        Send = HFB_SEND,
    }
}

/// What the beacon is, in the result fields.
pub const TX_NOTE: &str = "The own beacon is a proposal of the specification, not taken from a standard: Morse at the \
ITU-R M.1677-1 timing, a long dash and the power steps, sent at a set second of each interval of UTC for a set count. \
The receiver of the stack does not name an own beacon by its keying.";

/// Whether `c` is a call sign the beacon can key: 1 to 12 characters of A-Z, 0-9 and /.
pub fn call_ok(c: &str) -> bool {
    (1..=12).contains(&c.len())
        && c.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'/')
}

/// Whether `m` (upper case) is a message the beacon can key: up to 60 characters of A-Z, 0-9, space and / ? . , =,
/// with [`CALL_MARK`] anywhere.
pub fn message_ok(m: &str) -> bool {
    let rest = m.replace(CALL_MARK, "");
    m.len() <= 60
        && rest
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b" /?.,=".contains(&b))
}

/// The text of a sending: `message` with [`CALL_MARK`] replaced by `call`; a message without the mark is followed by
/// "DE" and the call sign; an empty message is the call sign alone.
pub fn beacon_text(message: &str, call: &str) -> String {
    let m = message.trim();
    if m.is_empty() {
        call.to_owned()
    } else if m.contains(CALL_MARK) {
        m.replace(CALL_MARK, call)
    } else {
        format!("{m} DE {call}")
    }
}

/// The key intervals of one sending of `text` at `wpm`: the Morse code of the text, then, when `long_dash_s` is above 0
/// or `steps` is set, a word gap, the long dash and the four power steps of [`STEP_S`], with no gap. Also the time from
/// the first key to the first power step, s, when `steps`.
///
/// # Errors
///
/// [`TxError::Invalid`] for a speed the keyer refuses or a text with no Morse code.
pub fn beacon_keys(
    text: &str,
    wpm: f64,
    long_dash_s: f64,
    steps: bool,
) -> Result<(Vec<Key>, Option<f64>), TxError> {
    let keyer = Keyer::new(wpm).map_err(|_| TxError::Invalid(WPM.key()))?;
    let mut keys: Vec<Key> = keyer.key(text).collect();
    if keys.is_empty() {
        return Err(TxError::Invalid(MESSAGE.key()));
    }
    if long_dash_s > 0.0 || steps {
        keys.push(Key {
            on: false,
            seconds: 7.0 * keyer.dot(),
        });
    }
    if long_dash_s > 0.0 {
        keys.push(Key {
            on: true,
            seconds: long_dash_s,
        });
    }
    let mut first_step = None;
    if steps {
        first_step = Some(keys.iter().map(|k| k.seconds).sum());
        keys.extend(STEPS_DB.iter().map(|_| Key {
            on: true,
            seconds: STEP_S,
        }));
    }
    Ok((keys, first_step))
}

/// The seconds from `now_utc` to the next instant that is `offset_s` after a boundary of intervals of `interval_s` of UTC;
/// 0 when `now_utc` is that instant to 1 ms.
pub fn wait_to_slot(now_utc: f64, interval_s: f64, offset_s: f64) -> f64 {
    if interval_s <= 0.0 {
        return 0.0;
    }
    let w = (offset_s - now_utc).rem_euclid(interval_s);
    if w < 1e-3 || interval_s - w < 1e-3 {
        0.0
    } else {
        w
    }
}

/// The time on air of a sending of `keys`, s: the keys with the rise before and the fall after.
pub fn sending_s(keys: &[Key]) -> f64 {
    keys.iter().map(|k| k.seconds).sum::<f64>() + 2.0 * RISE_S
}

/// One sending on air.
struct Sending {
    text: String,
    keys: Vec<Key>,
    next: usize,
    ook: Ook,
    nco: Nco,
    env: Vec<f32>,
    read: usize,
    flushed: bool,
    stopping: bool,
    made: u64,
    total: u64,
    rate: f64,
    /// The time of the first power step from the first sample, s.
    steps_at: Option<f64>,
    gain: f32,
}

impl Sending {
    fn new(
        text: String,
        keys: &[Key],
        steps_at: Option<f64>,
        rate: f64,
        pitch: f64,
    ) -> Result<Sending, TxError> {
        let mut ook = Ook::new(rate, RISE_S).map_err(|_| TxError::Rate)?;
        let mut pieces = Vec::with_capacity(keys.len());
        for k in keys {
            let n = (k.seconds / PIECE_S).ceil().max(1.0);
            for _ in 0..n as usize {
                pieces.push(Key {
                    on: k.on,
                    seconds: k.seconds / n,
                });
            }
        }
        let mut env = Vec::new();
        // Silence first, so that the first rise is whole.
        let _ = ook.push(false, RISE_S, &mut env);
        Ok(Sending {
            text,
            total: (sending_s(keys) * rate).ceil() as u64,
            keys: pieces,
            next: 0,
            ook,
            nco: Nco::new(rate, pitch),
            env,
            read: 0,
            flushed: false,
            stopping: false,
            made: 0,
            rate,
            steps_at: steps_at.map(|t| t + RISE_S),
            gain: 1.0,
        })
    }

    /// The gain of the power steps at `t` seconds from the first sample.
    fn step_gain(&self, t: f64) -> f32 {
        let Some(at) = self.steps_at.filter(|&at| t >= at) else {
            return 1.0;
        };
        let i = (((t - at) / STEP_S) as usize).min(STEPS_DB.len() - 1);
        10f64.powf(STEPS_DB[i] / 20.0) as f32
    }

    /// Appends at most `room` samples to `out`; true once the sending, with its fall, is all written.
    fn fill(&mut self, room: usize, out: &mut Vec<Complex32>, buf: &mut Vec<f32>) -> bool {
        let mut done = 0;
        while done < room {
            if self.read == self.env.len() {
                if self.flushed {
                    break;
                }
                self.env.clear();
                self.read = 0;
                match self.keys.get(self.next).filter(|_| !self.stopping) {
                    Some(k) => {
                        self.next += 1;
                        let _ = self.ook.push(k.on, k.seconds, &mut self.env);
                    }
                    None => {
                        let _ = self.ook.push(false, RISE_S, &mut self.env);
                        self.ook.flush(&mut self.env);
                        self.flushed = true;
                    }
                }
                continue;
            }
            let k = (self.env.len() - self.read).min(room - done);
            buf.clear();
            if self.steps_at.is_none() {
                buf.extend(self.env[self.read..self.read + k].iter().map(|e| e * LEVEL));
            } else {
                // A step moves the level over one rise time, in a straight line.
                let slope = (1.0 / (RISE_S * self.rate)) as f32;
                for (i, e) in self.env[self.read..self.read + k].iter().enumerate() {
                    let target = self.step_gain((self.made + i as u64) as f64 / self.rate);
                    self.gain += (target - self.gain).clamp(-slope, slope);
                    buf.push(e * LEVEL * self.gain);
                }
            }
            let start = out.len();
            out.resize(start + k, Complex32::default());
            self.nco.mix_real(buf, &mut out[start..]);
            self.read += k;
            done += k;
            self.made += k as u64;
        }
        self.flushed && self.read == self.env.len()
    }
}

/// The transmitter of the own beacon. See the module. Input `in` is the session's samples (their rate, pace and dial);
/// output `out` the complex baseband for `tx.sink` centred on the dial; output `events` the `hfb.tx` events. A sending
/// writes as many samples as each pass brings in; its last sample carries the end mark. On a live radio whose host times
/// bursts (`HostStreams::transmit_lead`), the first sample of each sending states its time on air (`TagKind::BurstStart`).
/// `stop` ends the sending with its fall within 50 ms and ends the schedule; the transmitter going off and a new format or
/// frequency end both at once, with the end mark on the next pass. A setting change applies to the next sending.
pub struct BeaconTransmitter {
    air: Air,
    call: String,
    message: String,
    wpm: f64,
    pitch: f64,
    long_dash_s: f64,
    interval_min: i64,
    power_steps: bool,
    offset_s: f64,
    count: i64,
    wall: Clock,
    running: bool,
    /// Samples of the session since `start`.
    t: u64,
    /// The sample of the session at which the next sending is due.
    next_at: u64,
    sendings: u64,
    sending: Option<Sending>,
    buf: Vec<f32>,
    events: Vec<Event>,
    results: Published,
}

impl Default for BeaconTransmitter {
    fn default() -> Self {
        let mut t = BeaconTransmitter {
            air: Air::default(),
            call: String::new(),
            message: MESSAGE.default.to_ascii_uppercase(),
            wpm: WPM.default,
            pitch: PITCH_HZ.default,
            long_dash_s: LONG_DASH_S.default,
            interval_min: INTERVAL_MIN.default,
            power_steps: POWER_STEPS.default,
            offset_s: OFFSET_S.default,
            count: COUNT.default,
            wall: Clock::fixed(0.0),
            running: false,
            t: 0,
            next_at: 0,
            sendings: 0,
            sending: None,
            buf: Vec::new(),
            events: Vec::new(),
            results: Published::new(HfbTxFields::SHAPE.fields),
        };
        t.publish();
        t
    }
}

impl BeaconTransmitter {
    fn event(&mut self, state: &str, reason: &str, text: &str) {
        self.events.push(HFB_TX.event(
            0.0,
            self.air.dial + self.pitch,
            &HfbTxEvent {
                state,
                reason,
                text,
                family: Family::Message,
            },
        ));
    }

    /// The text, the keys and the time of the first power step of a sending with the present settings.
    fn keys(&self) -> Result<(String, Vec<Key>, Option<f64>), TxError> {
        if self.call.is_empty() {
            return Err(TxError::Empty(CALL.key()));
        }
        let text = beacon_text(&self.message, &self.call);
        let (keys, steps) = beacon_keys(&text, self.wpm, self.long_dash_s, self.power_steps)?;
        Ok((text, keys, steps))
    }

    /// A new sending with the present settings.
    fn sending(&self) -> Result<Sending, TxError> {
        let (text, keys, steps) = self.keys()?;
        Sending::new(text, &keys, steps, self.air.rate, self.pitch)
    }

    /// Ends the sending on air and the schedule at once; the end mark goes out on the next pass.
    fn cut(&mut self, reason: &str) {
        let was = self.running || self.sending.is_some();
        if self.sending.take().is_some() {
            self.air.owe_end();
        }
        self.running = false;
        if was {
            self.event("finished", reason, "");
        }
    }

    /// Starts the beacon: the first sending at the next second `offset_s` of an interval of UTC (at once when that is
    /// now), then each interval.
    ///
    /// # Errors
    ///
    /// [`TxError`]: the transmitter is off, the beacon runs, the rate is too low, the call sign is empty, or the message
    /// has no Morse code.
    pub fn start(&mut self) -> Result<(), TxError> {
        self.air.ready()?;
        if self.running || self.sending.is_some() {
            return Err(TxError::Busy);
        }
        let s = self.sending()?;
        let interval_s = self.interval_min as f64 * 60.0;
        let wait = wait_to_slot(self.wall.now(), interval_s, self.offset_s);
        self.event("started", "", &s.text);
        self.running = true;
        self.t = 0;
        if wait == 0.0 {
            self.sending = Some(s);
            self.next_at = self.interval();
            self.sendings = 1;
        } else {
            self.next_at = (wait * self.air.rate).round() as u64;
            self.sendings = 0;
        }
        Ok(())
    }

    /// Sends once at once, outside the schedule.
    ///
    /// # Errors
    ///
    /// As [`BeaconTransmitter::start`].
    pub fn send_once(&mut self) -> Result<(), TxError> {
        self.air.ready()?;
        if self.running || self.sending.is_some() {
            return Err(TxError::Busy);
        }
        let s = self.sending()?;
        self.event("started", "", &s.text);
        self.sending = Some(s);
        Ok(())
    }

    /// Ends the sending on air with its fall, and the schedule.
    pub fn stop(&mut self) {
        if let Some(s) = &mut self.sending {
            s.stopping = true;
        }
        if self.running {
            self.running = false;
            self.event("finished", "stopped", "");
        }
    }

    /// The interval in samples of the session.
    fn interval(&self) -> u64 {
        (self.interval_min as f64 * 60.0 * self.air.rate).round() as u64
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let rate = self.air.rate.max(1.0);
        let on_air = self.keys().map(|(_, k, _)| sending_s(&k)).unwrap_or(0.0);
        let f = HfbTxFields {
            tx_beacon_on: self.running,
            tx_sending: self.sending.is_some(),
            tx_progress_pct: self
                .sending
                .as_ref()
                .map(|s| (100.0 * s.made as f64 / s.total.max(1) as f64).min(100.0)),
            tx_next_s: (self.running && self.sending.is_none())
                .then(|| Seconds(self.next_at.saturating_sub(self.t) as f64 / rate)),
            tx_sendings: self.sendings as i64,
            tx_sending_s: Seconds(on_air),
            tx_duty_pct: 100.0 * on_air / (self.interval_min as f64 * 60.0),
            tx_note: TX_NOTE,
        };
        self.results.put(&f);
    }

    fn run(&mut self, io: &mut Io) {
        self.air.settle(io);
        let room = io.input_len(0);
        let mut changed = false;
        if self.running && self.sending.is_none() && self.t >= self.next_at {
            match self.sending() {
                Ok(s) => {
                    self.event("sending", "", &s.text);
                    self.sending = Some(s);
                    self.next_at = self.t + self.interval();
                    self.sendings += 1;
                }
                Err(e) => {
                    self.running = false;
                    self.event("finished", &e.to_string(), "");
                }
            }
            changed = true;
        }
        let stated = self
            .sending
            .as_ref()
            .filter(|s| s.made == 0)
            .and_then(|_| self.air.start_time(room));
        if let Some(s) = &mut self.sending {
            let fresh = s.made == 0;
            let first = io.iq_out(0).len();
            let done = s.fill(room, io.iq_out(0), &mut self.buf);
            if fresh && io.iq_out(0).len() > first {
                if let Some(unix_s) = stated {
                    start_mark(io, 0, first, unix_s);
                }
            }
            if done {
                end_mark(io, 0);
                if let Some(s) = self.sending.take() {
                    self.event("sent", "", &s.text);
                }
                if self.running && self.count > 0 && self.sendings >= self.count as u64 {
                    self.running = false;
                    self.event("finished", "the count is sent", "");
                }
                changed = true;
            }
        }
        if self.running {
            self.t += room as u64;
        }
        if io.tick() || changed {
            self.publish();
        }
        io.messages_out(1).append(&mut self.events);
    }
}

impl Component for BeaconTransmitter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfb_tx",
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(HfbTxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let out = self.air.configure(&inputs[0]);
        self.cut("the format or the frequency changed");
        self.publish();
        Ok(vec![out, Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.air.attach(ctx);
        self.wall = ctx.clock.clone();
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
        self.cut("the input ended");
        self.air.settle(io);
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match HfbTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HfbTxSetting::Call => {
                let c = CALL.read(id, v)?.trim().to_ascii_uppercase();
                if !c.is_empty() && !call_ok(&c) {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.call = c;
            }
            HfbTxSetting::Message => {
                let m = MESSAGE.read(id, v)?.trim().to_ascii_uppercase();
                if !message_ok(&m) {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.message = m;
            }
            HfbTxSetting::Wpm => self.wpm = WPM.read(id, v)?,
            HfbTxSetting::Pitch => self.pitch = PITCH_HZ.read(id, v)?,
            HfbTxSetting::LongDash => self.long_dash_s = LONG_DASH_S.read(id, v)?,
            HfbTxSetting::Interval => self.interval_min = INTERVAL_MIN.read(id, v)?,
            HfbTxSetting::PowerSteps => self.power_steps = POWER_STEPS.read(id, v)?,
            HfbTxSetting::Offset => self.offset_s = OFFSET_S.read(id, v)?,
            HfbTxSetting::Count => self.count = COUNT.read(id, v)?,
        }
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (HfbTxSetting::Call, Value::Text(self.call.clone())),
            (HfbTxSetting::Message, Value::Text(self.message.clone())),
            (HfbTxSetting::Wpm, Value::F64(self.wpm)),
            (HfbTxSetting::Pitch, Value::Hz(self.pitch)),
            (HfbTxSetting::LongDash, Value::F64(self.long_dash_s)),
            (HfbTxSetting::Interval, Value::I64(self.interval_min)),
            (HfbTxSetting::PowerSteps, Value::Bool(self.power_steps)),
            (HfbTxSetting::Offset, Value::F64(self.offset_s)),
            (HfbTxSetting::Count, Value::I64(self.count)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        HfbTxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        let r = match HfbTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            HfbTxCommand::Start => self.start(),
            HfbTxCommand::Send => self.send_once(),
            HfbTxCommand::Stop => {
                self.stop();
                Ok(())
            }
        };
        if let Err(e) = r {
            self.event("refused", &e.to_string(), "");
            return Err(CommandError::Failed {
                source: StageError::new(e),
            });
        }
        self.publish();
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slot_of_the_interval() {
        assert_eq!(wait_to_slot(600.0, 600.0, 0.0), 0.0);
        assert!((wait_to_slot(610.0, 600.0, 0.0) - 590.0).abs() < 1e-9);
        assert!((wait_to_slot(610.0, 600.0, 30.0) - 20.0).abs() < 1e-9);
        assert!((wait_to_slot(640.0, 600.0, 30.0) - 590.0).abs() < 1e-9);
        assert!((wait_to_slot(600.0, 60.0, 90.0) - 30.0).abs() < 1e-9);
    }
}
