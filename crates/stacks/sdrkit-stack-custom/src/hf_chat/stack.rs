//! The `hf_chat` stack: its settings, results, receiver component (the upper-sideband channel at 2400 samples a second, the
//! bursts, the messages and beacons, the acknowledgements heard and the blocks to acknowledge), sender component (a message
//! one way in caller slots, or an acknowledged session; an acknowledgement in the next responder slot), test signal and
//! descriptor. Slot times come from the host's clock when the input is live, else from the stream, whose first sample is
//! then the start of a slot.

use super::arq::{self, Responder, Sender};
use super::frame::{self, Ack, Assembler, Block, Chat, FullBeacon, Message, Recipient, ShortBeacon, Taken, VoiceSide};
use super::phy::{self, ChannelUp};
use super::rx::{Burst, Receiver};
use super::{Profile, PROFILES, RATE};
use crate::typed::{
    HfChatAckEvent, HfChatBeaconEvent, HfChatBeaconRow, HfChatFields, HfChatMessageEvent, HfChatMessageRow,
    HfChatVoiceEvent, TxEvent, HF_CHAT_ACK, HF_CHAT_BEACON, HF_CHAT_BEACONS, HF_CHAT_MESSAGE, HF_CHAT_MESSAGES,
    HF_CHAT_TX, HF_CHAT_VOICE,
};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Clock, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Packet, Record,
    Schema, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StackDescriptor, StageError, Tuning,
    Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Decoder, DecoderComponent, Format, Io, Kept, Kind, Outbox, Port,
    RateFrom, RunContext, TagKind, Timed,
};
use std::f64::consts::TAU;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "hf_chat";
/// The lowest input rate.
pub const MIN_RATE: f64 = 250_000.0;
/// The rate the stack asks for.
pub const DEFAULT_RATE: f64 = 2_000_000.0;
/// The radio sits this far above the dial, so its DC spike stays off the signal.
pub const TUNE_OFFSET_HZ: f64 = 20_000.0;
/// What the stack says about its source.
pub const NOTE: &str = crate::slotted_voice::stack::NOTE;
/// The texts of the test signal and of `send` when the text is empty.
pub const TEST_TEXTS: [&str; 3] = ["TEST1 TEST OF THE HF CHAT STACK", "SECOND TEST MESSAGE 0123456789", "THIRD TEST: THE QUICK BROWN FOX"];
/// The amplitude of a burst of unit power at the transmitter output.
pub const TX_LEVEL: f32 = 0.3;

sdrkit_core::choice_enum! {
    /// The transmit profile.
    #[derive(Default)]
    pub enum TxProfile {
        /// Chirps SF 10, rate 1/4, three copies.
        P0 = ("0", "0: Abyss, chirp SF 10, rate 1/4 x 3"),
        /// Chirps SF 10, rate 1/4.
        P1 = ("1", "1: Deep, chirp SF 10, rate 1/4"),
        /// Chirps SF 9, rate 1/3.
        P2 = ("2", "2: Weak, chirp SF 9, rate 1/3"),
        /// Chirps SF 8, rate 1/2.
        #[default]
        P3 = ("3", "3: Chat, chirp SF 8, rate 1/2"),
        /// Chirps SF 6 with a guard, rate 1/2.
        P4 = ("4", "4: Mid, chirp SF 6, rate 1/2"),
        /// OFDM BPSK, rate 1/4.
        P5 = ("5", "5: Fast data, OFDM BPSK, rate 1/4"),
        /// OFDM QPSK voice of 700 bit/s.
        P6 = ("6", "6: Voice-700, OFDM QPSK"),
        /// OFDM QPSK voice of 1200 bit/s.
        P7 = ("7", "7: Voice-1200, OFDM QPSK"),
    }
}

sdrkit_core::choice_enum! {
    /// The profiles the receiver searches for.
    #[derive(Default)]
    pub enum Listen {
        /// All eight.
        #[default]
        All = ("all", "All profiles"),
        /// Profile 0.
        P0 = ("0", "Profile 0"),
        /// Profile 1.
        P1 = ("1", "Profile 1"),
        /// Profile 2.
        P2 = ("2", "Profile 2"),
        /// Profile 3.
        P3 = ("3", "Profile 3"),
        /// Profile 4.
        P4 = ("4", "Profile 4"),
        /// Profile 5.
        P5 = ("5", "Profile 5"),
        /// Profile 6.
        P6 = ("6", "Profile 6"),
        /// Profile 7.
        P7 = ("7", "Profile 7"),
    }
}

impl Listen {
    /// The profiles searched for.
    pub fn mask(self) -> [bool; 8] {
        match self.index() {
            0 => [true; 8],
            i => std::array::from_fn(|p| p + 1 == usize::from(i)),
        }
    }
}

sdrkit_core::choice_enum! {
    /// What `send` transmits.
    #[derive(Default)]
    pub enum MessageKind {
        /// A chat to the recipient.
        #[default]
        Chat = ("chat", "Chat"),
        /// A short beacon: callsign, locator and power.
        ShortBeacon = ("short_beacon", "Short beacon"),
        /// A full beacon: callsign, position, power, locator and up to 10 characters.
        FullBeacon = ("full_beacon", "Full beacon"),
        /// The bytes of the file.
        Data = ("data", "Data"),
    }
}

sdrkit_core::choice_enum! {
    /// How the sender moves a message.
    #[derive(Default)]
    pub enum LinkKind {
        /// The whole message in caller slots, in passes, without acknowledgements.
        #[default]
        OneWay = ("one_way", "One way, repeated passes"),
        /// An acknowledged session; the stack also answers the blocks it hears with an acknowledgement.
        TwoWay = ("two_way", "Two way, acknowledged"),
    }
}

const TX: &str = "Transmit";
const RX: &str = "Receive";

/// The station callsign.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Station callsign", "", "")
    .section(TX)
    .help("1 to 9 characters of A-Z, 0-9, '-', '/' and '.'. Empty sends the demonstration callsign TEST1.");
/// The transmit profile.
pub const PROFILE: Setting<field::Choice<TxProfile>> = Setting::choice("profile", "Profile", TxProfile::P3).section(TX);
/// What `send` transmits.
pub const KIND: Setting<field::Choice<MessageKind>> = Setting::choice("kind", "Message", MessageKind::Chat).section(TX);
/// The recipient of a chat.
pub const TO: Setting<field::Text> = Setting::new("to", "Recipient", "", "CQ")
    .section(TX)
    .help("A callsign; empty sends to CQ.")
    .refers("beacons", "callsign");
/// The text of a chat or a full beacon.
pub const TEXT: Setting<field::Text> = Setting::new("text", "Text", "", "")
    .section(TX)
    .help("Up to 100 characters for a chat, 10 for a full beacon. Empty sends a test text.");
/// The locator of a beacon.
pub const LOCATOR: Setting<field::Text> = Setting::new("locator", "Locator", "", "").section(TX).help("Six characters, AA00AA.");
/// The latitude of a full beacon.
pub const LAT: Setting<field::F64> = Setting::new("lat", "Latitude", "°", 0.0).range(-90.0, 90.0, 0.0).section(TX);
/// The longitude of a full beacon.
pub const LON: Setting<field::F64> = Setting::new("lon", "Longitude", "°", 0.0).range(-180.0, 180.0, 0.0).section(TX);
/// The power step of a beacon.
pub const POWER: Setting<field::I64> = Setting::new("power_step", "Power step", "", 0)
    .range(0.0, 255.0, 1.0)
    .section(TX)
    .help("EIRP dBm = -40 + 0.5 x step; a short beacon takes 0 to 63.");
/// The file of a data message.
pub const FILE: Setting<field::Text> = Setting::new("file", "File", "", "").section(TX).help("The bytes of a data message.");
/// Passes of a one-way message.
pub const PASSES: Setting<field::I64> = Setting::new("passes", "Passes", "", 2).range(1.0, 9.0, 1.0).section(TX);
/// One way or two way.
pub const LINK: Setting<field::Choice<LinkKind>> = Setting::choice("link", "Link", LinkKind::OneWay).section(TX);
/// The profiles searched for.
pub const LISTEN: Setting<field::Choice<Listen>> = Setting::choice("listen", "Profiles searched", Listen::All).section(RX);

sdrkit_core::settings! {
    /// The transmitter settings.
    pub enum TxSetting {
        /// Callsign.
        Callsign = CALLSIGN,
        /// Profile.
        Profile = PROFILE,
        /// Message kind.
        Kind = KIND,
        /// Recipient.
        To = TO,
        /// Text.
        Text = TEXT,
        /// Locator.
        Locator = LOCATOR,
        /// Latitude.
        Lat = LAT,
        /// Longitude.
        Lon = LON,
        /// Power step.
        Power = POWER,
        /// File.
        File = FILE,
        /// Passes.
        Passes = PASSES,
        /// Link.
        Link = LINK,
    }
}

sdrkit_core::settings! {
    /// The receiver settings.
    pub enum RxSetting {
        /// Profiles searched.
        Listen = LISTEN,
    }
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    static S: OnceLock<Arc<Schema>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Schema::of(&[RxSetting::SETTINGS, TxSetting::SETTINGS].concat()))).clone()
}

/// What the receiver tells the sender.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkNote {
    /// An acknowledgement was heard.
    Ack(Ack),
    /// Blocks of message `id` were heard in profile `profile`: the acknowledgement to send has `bitmap`, the SNR and the
    /// profile suggested for it.
    Answer {
        /// The profile of the burst.
        profile: u8,
        /// The message id.
        id: u8,
        /// The blocks held of the message.
        bitmap: u16,
        /// The SNR of the burst, dB in 2.4 kHz.
        snr_db: f64,
    },
}

impl sdrkit_graph::Item for LinkNote {
    const KIND: &'static str = "hf_chat.link";
}

/// The text of message bytes: printable ASCII as it is, other bytes as `\xNN`, at most 400 bytes.
fn printable(b: &[u8]) -> String {
    crate::arq_packet::stack::printable(b)
}

/// The channel of the upper sideband from a stream at `rate` whose dial is `dial_offset` Hz from its centre: the 12 kHz
/// channel around the audio centre, filtered with the audio taps and cut to 2400 samples a second.
struct FrontEnd {
    chan: Channelizer,
    dec: FirDecimator<Complex32>,
    buf: Vec<Complex32>,
    /// The delay of the filters, seconds.
    delay_s: f64,
}

impl FrontEnd {
    fn new(rate: f64, dial_offset: f64) -> Result<FrontEnd, ConfigError> {
        let mut cfg = ChannelConfig::new(rate, dial_offset + super::AUDIO_CENTRE_HZ, 3000.0, phy::AUDIO_RATE);
        cfg.exact_rate = true;
        let chan = Channelizer::new(cfg).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
        let delay_s = chan.delay() / rate + (phy::AUDIO_TAPS / 2) as f64 / phy::AUDIO_RATE;
        Ok(FrontEnd { chan, dec: FirDecimator::new(phy::audio_taps(), phy::AUDIO_STEP), buf: Vec::new(), delay_s })
    }

    fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        self.buf.clear();
        self.chan.process(x, &mut self.buf);
        self.dec.process(&self.buf, out);
    }
}

/// The `hf_chat.rx` decoder.
pub struct HcDecoder {
    listen: Listen,
    rate: f64,
    dial_offset: f64,
    front: Option<FrontEnd>,
    rx: Receiver,
    bb: Vec<Complex32>,
    found: Vec<Burst>,
    /// UTC of input sample 0, once the host's clock gave it (live input); else the stream's own seconds.
    t0: Option<f64>,
    wall: Option<Clock>,
    samples: u64,
    asm: Assembler,
    responder: Responder,
    bursts: u64,
    good: u64,
    bad: u64,
    messages: u64,
    beacons: u64,
    acks: u64,
    voice: u64,
    last: Option<(u8, f64, f64, f64, String, u8)>,
    last_burst: Option<f64>,
    rows: Vec<(String, String, String, f64)>,
    beacon_rows: Vec<HeardBeacon>,
    row_id: u64,
    /// The message table and the beacon table are to be emptied at the next report.
    cleared: [bool; 2],
    started: bool,
}

/// A completed beacon for the beacon table.
struct HeardBeacon {
    callsign: String,
    locator: String,
    position: (f64, f64),
    power_dbm: f64,
    text: String,
    time: f64,
}

impl Default for HcDecoder {
    fn default() -> Self {
        HcDecoder {
            listen: Listen::All,
            rate: 0.0,
            dial_offset: 0.0,
            front: None,
            rx: Receiver::new([true; 8]),
            bb: Vec::new(),
            found: Vec::new(),
            t0: None,
            wall: None,
            samples: 0,
            asm: Assembler::default(),
            responder: Responder::default(),
            bursts: 0,
            good: 0,
            bad: 0,
            messages: 0,
            beacons: 0,
            acks: 0,
            voice: 0,
            last: None,
            last_burst: None,
            rows: Vec::new(),
            beacon_rows: Vec::new(),
            row_id: 0,
            cleared: [false; 2],
            started: false,
        }
    }
}

impl HcDecoder {
    /// Seconds of the stream so far.
    fn now(&self) -> f64 {
        self.t0.unwrap_or(0.0) + self.samples as f64 / self.rate.max(1.0)
    }

    fn take(&mut self, out: &mut Outbox) {
        let delay = self.front.as_ref().map_or(0.0, |f| f.delay_s);
        for b in std::mem::take(&mut self.found) {
            let p = &PROFILES[usize::from(b.profile)];
            let utc = self.t0.unwrap_or(0.0) + b.start / RATE - delay;
            let clock = arq::clock_error(p, utc);
            self.bursts += 1;
            self.last_burst = Some(self.now());
            let mut from = String::new();
            let mut heard: Option<u8> = None;
            for bits in &b.blocks {
                let Some(bits) = bits else {
                    self.bad += 1;
                    continue;
                };
                self.good += 1;
                match frame::parse_block(bits) {
                    Some(Block::Part { id, index, payload }) => {
                        heard = Some(id);
                        self.responder.take(id, index, p.k);
                        match self.asm.take(id, index, &payload, p.k, utc) {
                            Taken::Complete(_, Some(m)) => from = self.message(&m, utc, out),
                            Taken::Complete(..) | Taken::Kept | Taken::Repeat => {}
                        }
                    }
                    Some(Block::Ack(a)) => {
                        self.acks += 1;
                        from.clone_from(&a.callsign);
                        out.messages(1).push(HF_CHAT_ACK.event(
                            utc,
                            0.0,
                            &HfChatAckEvent {
                                callsign: &a.callsign,
                                message_id: i64::from(a.id),
                                blocks_acked: i64::from(a.bitmap.count_ones()),
                                snr_db: f64::from(a.snr_db),
                                profile: i64::from(a.profile),
                            },
                        ));
                        out.items::<LinkNote>(2).push(Timed { time: utc, value: LinkNote::Ack(a) });
                    }
                    None => {
                        if let Some(side) = VoiceSide::parse(&frame::split_voice(p, bits).1) {
                            self.voice += 1;
                            from.clone_from(&side.callsign);
                            out.messages(1).push(HF_CHAT_VOICE.event(
                                utc,
                                0.0,
                                &HfChatVoiceEvent {
                                    callsign: &side.callsign,
                                    call: i64::from(side.call),
                                    profile: i64::from(p.id),
                                },
                            ));
                        }
                    }
                }
            }
            if let Some(id) = heard {
                let a = self.responder.ack("", id, b.snr_db);
                out.items::<LinkNote>(2).push(Timed {
                    time: utc,
                    value: LinkNote::Answer { profile: p.id, id, bitmap: a.bitmap, snr_db: b.snr_db },
                });
            }
            let id = heard.unwrap_or(0);
            self.last = Some((b.profile, b.snr_db, b.offset_hz, clock, from, id));
        }
    }

    /// Publishes a completed message; returns its sender.
    fn message(&mut self, m: &Message, utc: f64, out: &mut Outbox) -> String {
        match m {
            Message::Chat(c) => {
                self.messages += 1;
                let to = c.to.to_string();
                out.messages(1)
                    .push(HF_CHAT_MESSAGE.event(utc, 0.0, &HfChatMessageEvent { callsign: &c.from, to: &to, text: &c.text }));
                out.bytes(0).push(Packet::new(Arc::from(c.text.as_bytes()), utc));
                self.rows.push((c.from.clone(), to, c.text.clone(), utc));
                c.from.clone()
            }
            Message::Data(d) => {
                self.messages += 1;
                let text = printable(d);
                out.messages(1).push(HF_CHAT_MESSAGE.event(utc, 0.0, &HfChatMessageEvent { callsign: "", to: "", text: &text }));
                out.bytes(0).push(Packet::new(Arc::from(&d[..]), utc));
                self.rows.push((String::new(), String::new(), text, utc));
                String::new()
            }
            Message::ShortBeacon(s) => {
                let (lat, lon) = locator_centre(&s.locator);
                self.beacon(&s.callsign, &s.locator, lat, lon, s.power_step, "", utc, out);
                s.callsign.clone()
            }
            Message::FullBeacon(f) => {
                self.beacon(&f.callsign, &f.locator, f.lat, f.lon, f.power_step, &f.text, utc, out);
                f.callsign.clone()
            }
        }
    }

    fn beacon(&mut self, call: &str, loc: &str, lat: f64, lon: f64, step: u8, text: &str, utc: f64, out: &mut Outbox) {
        self.beacons += 1;
        let dbm = -40.0 + 0.5 * f64::from(step);
        out.messages(1).push(HF_CHAT_BEACON.event(
            utc,
            0.0,
            &HfChatBeaconEvent { callsign: call, locator: loc, position: (lat, lon), power_dbm: dbm, text },
        ));
        self.beacon_rows.push(HeardBeacon {
            callsign: call.to_string(),
            locator: loc.to_string(),
            position: (lat, lon),
            power_dbm: dbm,
            text: text.to_string(),
            time: utc,
        });
    }
}

/// The centre of a 6-character locator square, degrees.
fn locator_centre(loc: &str) -> (f64, f64) {
    let b = loc.as_bytes();
    if b.len() != 6 {
        return (0.0, 0.0);
    }
    let v = |i: usize, base: u8| f64::from(b[i].to_ascii_uppercase().wrapping_sub(base));
    let lon = -180.0 + v(0, b'A') * 20.0 + v(2, b'0') * 2.0 + (v(4, b'A') + 0.5) / 12.0;
    let lat = -90.0 + v(1, b'A') * 10.0 + v(3, b'0') + (v(5, b'A') + 0.5) / 24.0;
    (lat, lon)
}

impl Decoder for HcDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hf_chat.rx"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::bytes("messages"),
            Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<LinkNote>("link").rate_from(RateFrom::Fixed(0.0)),
        ]
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
                let l = LISTEN.read(id, v)?;
                if l != self.listen {
                    self.listen = l;
                    self.rx = Receiver::new(l.mask());
                }
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::new(HfChatFields::SHAPE.fields).with_tables(&[HF_CHAT_MESSAGES.def(), HF_CHAT_BEACONS.def()])
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow { need: MIN_RATE, got: input.rate });
        }
        self.rate = input.rate;
        let dial = if input.tuned_hz > 0.0 { input.tuned_hz } else { input.centre_hz };
        self.dial_offset = dial - input.centre_hz;
        self.front = Some(FrontEnd::new(self.rate, self.dial_offset)?);
        self.rx = Receiver::new(self.listen.mask());
        Ok(vec![
            Format::new(Kind::Bytes, 0.0),
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<LinkNote>(), 0.0),
        ])
    }
    fn retune(&mut self, input: &Format) {
        let dial = if input.tuned_hz > 0.0 { input.tuned_hz } else { input.centre_hz };
        self.dial_offset = dial - input.centre_hz;
        self.front = FrontEnd::new(self.rate, self.dial_offset).ok();
        self.rx.reset();
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.wall = ctx.live.then(|| ctx.clock.clone());
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        if self.t0.is_none() {
            if let Some(w) = &self.wall {
                self.t0 = Some(w.now() - first as f64 / self.rate.max(1.0));
            }
        }
        self.samples = first + x.len() as u64;
        self.bb.clear();
        if let Some(f) = self.front.as_mut() {
            f.process(x, &mut self.bb);
        }
        self.rx.process(&self.bb, &mut self.found);
        self.take(out);
    }
    fn finish(&mut self, out: &mut Outbox) {
        self.rx.finish(&mut self.found);
        self.take(out);
    }
    fn report(&mut self, p: &mut Published) {
        let [messages, beacons] = std::mem::take(&mut self.cleared);
        if messages {
            p.clear_rows(&HF_CHAT_MESSAGES);
        }
        if beacons {
            p.clear_rows(&HF_CHAT_BEACONS);
        }
        let receiving = self.last_burst.is_some_and(|t| self.now() - t <= 30.0);
        let last = self.last.as_ref();
        p.put(&HfChatFields {
            profile: last.map(|l| i64::from(l.0)),
            snr_db: last.map(|l| l.1),
            offset_hz: last.map(|l| Hertz(l.2)),
            clock_error_s: last.map(|l| l.3),
            callsign: last.map(|l| l.4.as_str()),
            bursts: self.bursts as i64,
            blocks_good: self.good as i64,
            blocks_bad: self.bad as i64,
            messages: self.messages as i64,
            beacons: self.beacons as i64,
            acks_heard: self.acks as i64,
            voice_blocks: self.voice as i64,
            locked: receiving,
            note: NOTE,
            message_id: last.map(|l| i64::from(l.5)),
        });
        for (callsign, to, text, time) in std::mem::take(&mut self.rows) {
            self.row_id += 1;
            HF_CHAT_MESSAGES.put(
                p,
                &format!("{}", self.row_id),
                &HfChatMessageRow { callsign: &callsign, to: &to, text: &text, time: Time(time) },
            );
        }
        for b in std::mem::take(&mut self.beacon_rows) {
            self.row_id += 1;
            HF_CHAT_BEACONS.put(
                p,
                &format!("{}", self.row_id),
                &HfChatBeaconRow {
                    callsign: &b.callsign,
                    locator: &b.locator,
                    position: b.position,
                    power_dbm: b.power_dbm,
                    text: &b.text,
                    time: Time(b.time),
                },
            );
        }
        let (lock, summary) = if receiving {
            let call = self.last.as_ref().map_or("", |l| l.4.as_str());
            (LockState::Locked, format!("Receiving bursts from {call}"))
        } else if self.started {
            (LockState::Searching, format!("Listening; {} messages, {} beacons", self.messages, self.beacons))
        } else {
            (LockState::Off, String::new())
        };
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock || s.summary != summary;
            s.lock_state = lock;
            s.summary.clone_from(&summary);
            changed
        });
    }
    fn report_interval_s(&self) -> f64 {
        0.5
    }
    fn clear(&mut self) {
        self.bursts = 0;
        self.good = 0;
        self.bad = 0;
        self.messages = 0;
        self.beacons = 0;
        self.acks = 0;
        self.voice = 0;
        self.last = None;
        self.asm.clear();
        self.responder.clear();
        self.cleared = [true; 2];
    }
    fn forget(&mut self, kept: &Kept) {
        self.rx.reset();
        self.found.clear();
        self.bursts = 0;
        self.good = 0;
        self.bad = 0;
        self.acks = 0;
        self.voice = 0;
        self.last = None;
        self.last_burst = None;
        self.asm.clear();
        self.responder.clear();
        if !kept.field("messages") {
            self.messages = 0;
        }
        if !kept.field("beacons") {
            self.beacons = 0;
        }
        if !kept.table(HF_CHAT_MESSAGES.key) {
            self.rows.clear();
            self.cleared[0] = true;
        }
        if !kept.table(HF_CHAT_BEACONS.key) {
            self.beacon_rows.clear();
            self.cleared[1] = true;
        }
    }
}

/// Why `send` was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit switch is off.
    Off,
    /// A transmission is under way.
    Busy,
    /// The output rate is below 250 ksps.
    Rate,
    /// Profiles 6 and 7 carry speech, and sdrkit has no speech codec of their frame sizes.
    Voice,
    /// A field of the message is not valid.
    Frame(frame::FrameError),
    /// The message needs more than 16 blocks of the profile.
    Long,
    /// The file cannot be read or is empty.
    File,
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::Busy => f.write_str("a transmission is under way"),
            TxError::Rate => f.write_str("the transmit rate must be at least 250 ksps"),
            TxError::Voice => f.write_str("profiles 6 and 7 carry speech, and no speech codec of their frame sizes is installed"),
            TxError::Frame(e) => e.fmt(f),
            TxError::Long => f.write_str("the message needs more than 16 blocks of the profile"),
            TxError::File => f.write_str("the file cannot be read, is empty or is larger than 65535 bytes"),
        }
    }
}

impl std::error::Error for TxError {}

/// Starts a transmission.
pub const SEND: Command = Command::new("send", "Send")
    .help("Sends the message in the next caller slots. Transmit must be on.")
    .transmits(sdrkit_core::TxContent::Text)
    .stopped_by("stop");
/// Ends the transmission.
pub const STOP: Command = Command::new("stop", "Stop").help("Ends the transmission after the burst on the air.");

sdrkit_core::commands! {
    /// The commands of the sender.
    pub enum TxCommand {
        /// Send.
        Send = SEND,
        /// Stop.
        Stop = STOP,
    }
}

/// Baseband at 2400 samples a second to a stream at `rate` whose dial is `dial_offset` Hz from its centre, on demand.
struct Upconverter {
    bb: Vec<Complex32>,
    at: usize,
    up: ChannelUp,
    rs: Resampler<Complex32>,
    work: Vec<Complex32>,
    out: std::collections::VecDeque<Complex32>,
    phase: f64,
    step: f64,
    gain: f32,
}

impl Upconverter {
    fn new(bb: Vec<Complex32>, rate: f64, dial_offset: f64, gain: f32) -> Result<Upconverter, sdrkit_algo_dsp::DspError> {
        Ok(Upconverter {
            bb,
            at: 0,
            up: ChannelUp::default(),
            rs: Resampler::new(phy::AUDIO_RATE, rate)?,
            work: Vec::new(),
            out: std::collections::VecDeque::new(),
            phase: 0.0,
            step: dial_offset / rate,
            gain,
        })
    }

    /// Up to `n` samples; fewer when the burst and the tails of the filters are out.
    fn take(&mut self, n: usize, dst: &mut Vec<Complex32>) {
        // The filters' tails: 64 baseband samples of silence after the burst.
        let total = self.bb.len() + 64;
        while self.out.len() < n && self.at < total {
            self.work.clear();
            let mut ch = Vec::with_capacity(5 * 256);
            for _ in 0..256 {
                if self.at >= total {
                    break;
                }
                let x = self.bb.get(self.at).copied().unwrap_or_default();
                self.at += 1;
                self.up.push(x, &mut ch);
            }
            self.rs.process(&ch, &mut self.work);
            for &v in &self.work {
                let a = TAU * self.phase;
                self.phase = (self.phase + self.step).rem_euclid(1.0);
                self.out.push_back(v * Complex32::new(a.cos() as f32, a.sin() as f32) * self.gain);
            }
        }
        let k = n.min(self.out.len());
        dst.extend(self.out.drain(..k));
    }

    fn done(&self) -> bool {
        self.at >= self.bb.len() + 64 && self.out.is_empty()
    }
}

/// A burst waiting for its slot: its first sample (UTC s) and its baseband samples.
struct Queued {
    start: f64,
    bb: Vec<Complex32>,
}

/// The sender (`hf_chat.tx`).
pub struct HcSender {
    callsign: String,
    profile: TxProfile,
    kind: MessageKind,
    to: String,
    text: String,
    locator: String,
    lat: f64,
    lon: f64,
    power: i64,
    file: String,
    passes: i64,
    link: LinkKind,
    rate: f64,
    dial_offset: f64,
    transmit: bool,
    wall: Option<Clock>,
    t0: Option<f64>,
    samples: u64,
    queue: Vec<Queued>,
    playing: Option<Upconverter>,
    /// The acknowledged session and the earliest time (UTC s) of its next caller slot.
    session: Option<(Sender, f64)>,
    stopping: bool,
    events: Vec<sdrkit_core::Event>,
}

impl Default for HcSender {
    fn default() -> Self {
        HcSender {
            callsign: String::new(),
            profile: TxProfile::P3,
            kind: MessageKind::Chat,
            to: "CQ".into(),
            text: String::new(),
            locator: String::new(),
            lat: 0.0,
            lon: 0.0,
            power: 0,
            file: String::new(),
            passes: 2,
            link: LinkKind::OneWay,
            rate: 0.0,
            dial_offset: 0.0,
            transmit: false,
            wall: None,
            t0: None,
            samples: 0,
            queue: Vec::new(),
            playing: None,
            session: None,
            stopping: false,
            events: Vec::new(),
        }
    }
}

/// The callsign a transmitter sends: the setting, or TEST1 when it is empty.
fn tx_callsign(call: &str) -> String {
    let c = call.trim();
    if c.is_empty() { crate::family::DEMO_CALLSIGN.to_string() } else { c.to_ascii_uppercase() }
}

impl HcSender {
    fn event(&mut self, state: &str, reason: &str) {
        self.events.push(HF_CHAT_TX.event(0.0, 0.0, &TxEvent { state, reason }));
    }

    /// UTC (or stream seconds) of the next input sample.
    fn now(&self) -> f64 {
        self.t0.unwrap_or(0.0) + self.samples as f64 / self.rate.max(1.0)
    }

    fn busy(&self) -> bool {
        !self.queue.is_empty() || self.playing.is_some() || self.session.is_some()
    }

    /// The bits of the message of the settings.
    pub fn message_bits(&self) -> Result<Vec<u8>, TxError> {
        let call = tx_callsign(&self.callsign);
        let m = match self.kind {
            MessageKind::Chat => Message::Chat(Chat {
                from: call,
                to: Recipient::Call(if self.to.trim().is_empty() { "CQ".into() } else { self.to.trim().to_ascii_uppercase() }),
                raw: false,
                more: false,
                seq: 0,
                ack: 0,
                text: if self.text.is_empty() { TEST_TEXTS[0].to_string() } else { self.text.clone() },
            }),
            MessageKind::ShortBeacon => Message::ShortBeacon(ShortBeacon {
                callsign: call,
                locator: self.locator.clone(),
                power_step: self.power.clamp(0, 63) as u8,
                fine_lat: 0,
                fine_lon: 0,
                link: 0,
            }),
            MessageKind::FullBeacon => Message::FullBeacon(FullBeacon {
                callsign: call,
                lat: self.lat,
                lon: self.lon,
                power_step: self.power.clamp(0, 255) as u8,
                link: 0,
                locator: self.locator.clone(),
                text: self.text.chars().take(10).collect(),
            }),
            MessageKind::Data => {
                let b = std::fs::read(&self.file).map_err(|_| TxError::File)?;
                if b.is_empty() || b.len() > 65_535 {
                    return Err(TxError::File);
                }
                Message::Data(b)
            }
        };
        m.bits().map_err(TxError::Frame)
    }

    /// Starts a transmission.
    pub fn send(&mut self) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.busy() {
            return Err(TxError::Busy);
        }
        if self.rate < MIN_RATE {
            return Err(TxError::Rate);
        }
        let p = &PROFILES[usize::from(self.profile.index())];
        if p.slot_s.is_none() {
            return Err(TxError::Voice);
        }
        let bits = self.message_bits()?;
        let now = self.now();
        let id = arq::message_id(now);
        self.stopping = false;
        match self.link {
            LinkKind::OneWay => {
                let plan = arq::one_way(&bits, p, id, self.passes.clamp(1, 9) as usize, now + 0.5).ok_or(TxError::Long)?;
                for (slot, blocks) in plan {
                    self.queue.push(Queued { start: arq::slot_start(p, slot) + arq::START_DELAY_S, bb: phy::burst(p, &blocks) });
                }
            }
            LinkKind::TwoWay => {
                let s = Sender::new(&bits, id, p.id).ok_or(TxError::Long)?;
                self.session = Some((s, now + 0.5));
            }
        }
        Ok(())
    }

    fn take_notes(&mut self, notes: &[Timed<LinkNote>]) {
        for n in notes {
            match &n.value {
                LinkNote::Ack(a) => {
                    if let Some((s, _)) = self.session.as_mut() {
                        s.on_ack(a);
                    }
                }
                LinkNote::Answer { profile, id, bitmap, snr_db } => {
                    if !self.transmit || self.link != LinkKind::TwoWay {
                        continue;
                    }
                    let p = &PROFILES[usize::from(*profile)];
                    if p.slot_s.is_none() {
                        continue;
                    }
                    let ack = Ack { callsign: tx_callsign(&self.callsign), id: *id, bitmap: *bitmap, snr_db: snr_db.round() as i32, profile: arq::suggest(*snr_db) };
                    let Ok(bits) = ack.bits(p.k) else {
                        continue;
                    };
                    let slot = arq::next_slot(p, self.now() + 0.5, false);
                    self.queue.push(Queued { start: arq::slot_start(p, slot) + arq::START_DELAY_S, bb: phy::burst(p, &[bits]) });
                }
            }
        }
    }

    /// Queues the next burst of the session when its caller slot is near; ends the session when it is over. The slots are
    /// those of the session's profile at the time, which an acknowledgement may change.
    fn session_step(&mut self) {
        let now = self.now();
        let Some((s, next_at)) = self.session.as_mut() else {
            return;
        };
        let p = s.profile();
        let idx = arq::next_slot(p, *next_at, true);
        let start = arq::slot_start(p, idx) + arq::START_DELAY_S;
        if now + 0.5 < start {
            return;
        }
        let next = if self.stopping { None } else { s.next_burst() };
        match next {
            Some(blocks) => {
                let bb = phy::burst(p, &blocks);
                // A slot already begun is skipped.
                let idx = if now <= start { idx } else { arq::next_slot(p, now + 0.5, true) };
                // The next caller slot leaves the responder slot between for the acknowledgement.
                *next_at = arq::slot_start(p, idx + 2);
                self.queue.push(Queued { start: arq::slot_start(p, idx) + arq::START_DELAY_S, bb });
            }
            None => {
                let done = s.done();
                self.session = None;
                self.event("finished", if done { "" } else if self.stopping { "stopped" } else { "no acknowledgement after 8 sends of a block" });
            }
        }
    }
}

impl Component for HcSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hf_chat.tx",
            vec![Port::iq("in"), Port::typed::<LinkNote>("link")],
            vec![Port::iq("out"), Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        let dial = if f.tuned_hz > 0.0 { f.tuned_hz } else { f.centre_hz };
        self.dial_offset = dial - f.centre_hz;
        Ok(vec![Format::iq(self.rate, f.centre_hz, dial), Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.wall = ctx.live.then(|| ctx.clock.clone());
        if !ctx.transmit && self.busy() {
            self.queue.clear();
            self.playing = None;
            self.session = None;
            self.event("finished", "the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        if self.t0.is_none() {
            if let Some(w) = &self.wall {
                self.t0 = Some(w.now() - self.samples as f64 / self.rate.max(1.0));
            }
        }
        let notes = io.items::<LinkNote>(1).to_vec();
        self.take_notes(&notes);
        self.session_step();
        let now = self.now();
        let end = now + room as f64 / self.rate.max(1.0);
        let mut written = 0usize;
        if self.playing.is_none() {
            self.queue.sort_by(|a, b| a.start.total_cmp(&b.start));
            // A burst whose start has passed by more than a second missed its slot.
            while self.queue.first().is_some_and(|q| q.start + 1.0 < now) {
                self.queue.remove(0);
                self.event("skipped", "the slot had begun");
            }
            if self.queue.first().is_some_and(|q| q.start < end) {
                let q = self.queue.remove(0);
                let lead = (((q.start - now) * self.rate).round().max(0.0) as usize).min(room);
                io.iq_out(0).extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), lead));
                written = lead;
                self.playing = Upconverter::new(q.bb, self.rate, self.dial_offset, TX_LEVEL).ok();
            }
        }
        if let Some(u) = self.playing.as_mut() {
            let mut chunk = Vec::with_capacity(room - written);
            u.take(room - written, &mut chunk);
            io.iq_out(0).extend(chunk);
            if u.done() {
                if let Some(last) = io.iq_out(0).len().checked_sub(1) {
                    io.output(0).tag_here(last, TagKind::BurstEnd);
                }
                self.playing = None;
                if self.queue.is_empty() && self.session.is_none() {
                    self.event("finished", "");
                }
            }
        }
        self.samples += room as u64;
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TxSetting::Callsign => self.callsign = CALLSIGN.read(id, v)?.to_owned(),
            TxSetting::Profile => self.profile = PROFILE.read(id, v)?,
            TxSetting::Kind => self.kind = KIND.read(id, v)?,
            TxSetting::To => self.to = TO.read(id, v)?.to_owned(),
            TxSetting::Text => self.text = TEXT.read(id, v)?.to_owned(),
            TxSetting::Locator => self.locator = LOCATOR.read(id, v)?.to_owned(),
            TxSetting::Lat => self.lat = LAT.read(id, v)?,
            TxSetting::Lon => self.lon = LON.read(id, v)?,
            TxSetting::Power => self.power = POWER.read(id, v)?,
            TxSetting::File => self.file = FILE.read(id, v)?.to_owned(),
            TxSetting::Passes => self.passes = PASSES.read(id, v)?,
            TxSetting::Link => self.link = LINK.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (TxSetting::Callsign, Value::Text(self.callsign.clone())),
            (TxSetting::Profile, Value::Choice(self.profile.index())),
            (TxSetting::Kind, Value::Choice(self.kind.index())),
            (TxSetting::To, Value::Text(self.to.clone())),
            (TxSetting::Text, Value::Text(self.text.clone())),
            (TxSetting::Locator, Value::Text(self.locator.clone())),
            (TxSetting::Lat, Value::F64(self.lat)),
            (TxSetting::Lon, Value::F64(self.lon)),
            (TxSetting::Power, Value::I64(self.power)),
            (TxSetting::File, Value::Text(self.file.clone())),
            (TxSetting::Passes, Value::I64(self.passes.clamp(1, 9))),
            (TxSetting::Link, Value::Choice(self.link.index())),
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
                    return Err(CommandError::Failed { source: StageError::new(e) });
                }
            },
            TxCommand::Stop => {
                self.stopping = true;
                self.queue.clear();
            }
        }
        Ok(None)
    }
}

/// The baseband of the test signal of profile `p`: the test texts as chats from TEST1 to CQ, one way in one pass, each in
/// the caller slots after the last (a voice profile: a burst of 5 voice blocks of call 1 every 10 s); the stream starts at a
/// slot start. Returns the samples of one cycle.
pub fn test_baseband(p: &Profile) -> Vec<Complex32> {
    if p.slot_s.is_none() {
        let side = VoiceSide { callsign: crate::family::DEMO_CALLSIGN.into(), call: 1, slow: vec![0; 108] };
        let per = p.k / frame::VOICE_FRAMES - frame::SIDE_SLICE;
        let blocks: Vec<Vec<u8>> = (0..5)
            .map(|b| {
                let codec: Vec<Vec<u8>> = (0..frame::VOICE_FRAMES).map(|f| (0..per).map(|i| ((i + 3 * f + b) % 4 == 0) as u8).collect()).collect();
                frame::voice_block(p, &codec, &side).unwrap_or_default()
            })
            .collect();
        let mut x = vec![Complex32::new(0.0, 0.0); (arq::START_DELAY_S * RATE) as usize];
        x.extend(phy::burst(p, &blocks));
        x.resize((arq::VOICE_DATA_SLOT_S * RATE) as usize, Complex32::new(0.0, 0.0));
        return x;
    }
    let mut x = Vec::new();
    let mut slot = 0i64;
    for (i, t) in TEST_TEXTS.iter().enumerate() {
        let m = Message::Chat(Chat {
            from: crate::family::DEMO_CALLSIGN.into(),
            to: Recipient::Call("CQ".into()),
            raw: false,
            more: false,
            seq: i as u8,
            ack: 0,
            text: (*t).into(),
        });
        let bits = m.bits().unwrap_or_default();
        let plan = arq::one_way(&bits, p, i as u8, 1, arq::slot_start(p, slot)).unwrap_or_default();
        for (s, blocks) in plan {
            let at = ((arq::slot_start(p, s) + arq::START_DELAY_S) * RATE) as usize;
            let b = phy::burst(p, &blocks);
            if x.len() < at + b.len() {
                x.resize(at + b.len(), Complex32::new(0.0, 0.0));
            }
            x[at..at + b.len()].copy_from_slice(&b);
            slot = s + 2;
        }
    }
    x.resize((arq::slot_start(p, slot) * RATE) as usize, Complex32::new(0.0, 0.0));
    x
}

/// The test signal at the generator's rate: [`test_baseband`] of the profile setting, again and again, as a radio tuned
/// [`TUNE_OFFSET_HZ`] above the dial sees it: the dial at -20 kHz.
pub struct HcGenerator {
    rate: f64,
    bb: Arc<Vec<Complex32>>,
    up: Upconverter,
}

impl Generator for HcGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(2400.0))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut v = Vec::with_capacity(out.len());
        while v.len() < out.len() {
            let before = v.len();
            self.up.take(out.len() - v.len(), &mut v);
            if self.up.done() || v.len() == before {
                // The next cycle; the filters keep their state.
                self.up.bb = self.bb.to_vec();
                self.up.at = 0;
            }
        }
        out.copy_from_slice(&v[..out.len()]);
        out.len()
    }
}

/// The generator of the stack.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = &PROFILES[usize::from(cfg.settings.get(&PROFILE).index())];
    let bb = Arc::new(test_baseband(p));
    let up = Upconverter::new(bb.to_vec(), cfg.rate_hz, -TUNE_OFFSET_HZ, 1.0).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    Ok(Box::new(HcGenerator { rate: cfg.rate_hz, bb, up }))
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
        .add("rx", DecoderComponent::new(HcDecoder::default()))
        .add("sender", HcSender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("rx")
        .input("sender.in")
        .connect("rx.link", "sender.link")
        .connect("sender.out", "tx.in")
        .output("rx.messages")
        .output("rx.events")
        .output("sender.events")
        .status_from("rx")
        .clock(0.5);
    let mut g = StackGraph::of(crate::typed::shape(ID), b.build()?)
        .min_rate(MIN_RATE, "hf_chat needs a sample rate of at least 250 ksps");
    for s in RxSetting::SETTINGS {
        let k = s.key();
        g = g.route(k, &[&format!("rx.{k}")])?;
    }
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
pub static HF_CHAT: StackDescriptor = StackDescriptor {
    id: ID,
    name: "HF weak-signal chat",
    group: crate::GROUP,
    blurb: "Short messages, beacons and voice in 2.4 kHz of upper-sideband audio, in UTC time slots: five chirp profiles \
            for weak signals and three OFDM profiles, each block coded by its own LDPC code; one way or acknowledged. \
            Receive and transmit.",
    tuning: Tuning {
        tune_offset_hz: TUNE_OFFSET_HZ,
        ..Tuning::anywhere(14.1e6, DEFAULT_RATE, MIN_RATE, 2400.0)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::HF_CHAT_VERIFY),
    benchmarks: Some(crate::verify::hf_chat_benchmarks),
};

/// The profile of a profile setting.
pub fn profile_of(p: TxProfile) -> &'static Profile {
    &PROFILES[usize::from(p.index())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listen_masks_and_locator_centres() {
        assert_eq!(Listen::All.mask(), [true; 8]);
        assert_eq!(Listen::P3.mask(), [false, false, false, true, false, false, false, false]);
        let (lat, lon) = locator_centre("KO24RM");
        assert!((lat - 54.52).abs() < 0.05 && (lon - 25.46).abs() < 0.05, "{lat} {lon}");
    }
}
