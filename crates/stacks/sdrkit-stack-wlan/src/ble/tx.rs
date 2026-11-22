//! The advertising sender of the `ble` and `bletracker` stacks (`wlan.ble_tx`, `wlan.ble_tracker_tx`): advertising packets
//! (non-extended PDUs) on the LE 1M PHY, on the primary advertising channel of the tuned frequency (2402, 2426 and 2480 MHz are 37, 38
//! and 39). The PDU comes from [`build_adv`]; the bits on air (preamble, access address 0x8E89BED6, PDU, CRC-24 from
//! 0x555555, whitening seeded from the channel index) and the GFSK of BT 0.5 and modulation index 0.5 come from the stack's
//! test signal path ([`super::gen::packet_samples`]). Each packet is one burst with its own end mark, at `tx_level` of full
//! scale, with a raised-cosine rise and fall of [`RAMP_S`] inside the unmodulated carrier around it, its carrier on the
//! centre of the channel.
//!
//! `send` sends `count` packets, `interval_ms` apart from start to start, counted in input samples; with a host that times
//! bursts, each packet states its time on air at that interval from the first. A send during another waits for it and
//! starts its interval after the last packet of it. `halt`
//! drops the packets that wait. A new frequency, a new input format or the transmitter going off drops them too.
//! Standard: Bluetooth Core Specification v5.4 Vol 6 Part B clause 2.3; Core Specification Supplement v11 Part A.
//!
//! Input `in` is the session's samples: their rate, pace and dial. Output `out` is complex baseband at the session rate for
//! `tx.sink`, centred on the tuned frequency. Output `events` carries the `ble.tx` events. The engine's transmit setting is
//! the only switch.

use super::gen::{packet_samples, parse_addr, GenPacket};
use crate::testgen::GenError;
use crate::typed::{
    BleSendArgs, BleTxEvent, BleTxFields, BletrackerSendArgs, BLETRACKER_SEND, BLE_HALT, BLE_SEND,
    BLE_TX,
};
use sdrkit_components::tx::{BurstOut, BurstTiming, Ramp};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Complex32, ConfigError, Event, Record, Results, Setting, SettingError, SettingId, SettingSet,
    StageError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
};
use sdrkit_proto_wlan_phy::ble::{ad, advertising_channel_at, build_adv, channel_hz, PduType};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The section of the transmit settings.
pub const TX: &str = "Transmit";
/// The most sends that wait behind the one on air.
pub const MAX_WAITING: usize = 16;
/// The most bytes of advertising data in a non-extended advertising PDU.
pub const MAX_DATA: usize = 31;
/// The rise and the fall of the carrier of each packet, s.
pub const RAMP_S: f64 = 2e-6;
/// How near the centre of an advertising channel the tuned frequency must be, Hz.
pub const CHANNEL_TOLERANCE_HZ: f64 = 0.1e6;
/// The company identifier of Find My frames.
pub const FIND_MY_COMPANY: u16 = 0x004C;
/// The type and the length of the Find My offline finding frame.
pub const FIND_MY_HEAD: [u8; 2] = [0x12, 25];
/// The bytes of a Find My frame after the status.
pub const FIND_MY_PAYLOAD: usize = 24;
/// The 16-bit service UUID of Google tracker frames.
pub const GOOGLE_UUID: u16 = 0xFEAA;
/// The Google frame type, and the frame type with unwanted-tracking protection.
pub const GOOGLE_FRAME: [u8; 2] = [0x40, 0x41];
/// The most bytes of a Google frame after the frame type: the data less the service data header and the frame type.
pub const GOOGLE_PAYLOAD: usize = MAX_DATA - 5;

sdrkit_core::choice_enum! {
    /// The PDU type that `send` sends.
    #[derive(Default)]
    pub enum SendPdu {
        /// Not connectable, not scannable.
        #[default]
        NonconnInd = ("adv_nonconn_ind", "ADV_NONCONN_IND (not connectable)"),
        /// Connectable and scannable.
        Ind = ("adv_ind", "ADV_IND (connectable)"),
        /// Scannable.
        ScanInd = ("adv_scan_ind", "ADV_SCAN_IND (scannable)"),
    }
}

impl SendPdu {
    /// The PDU type.
    pub fn pdu_type(self) -> PduType {
        match self {
            SendPdu::NonconnInd => PduType::AdvNonconnInd,
            SendPdu::Ind => PduType::AdvInd,
            SendPdu::ScanInd => PduType::AdvScanInd,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The type of the advertiser address: the TxAdd bit of the header.
    #[derive(Default)]
    pub enum SendAddress {
        /// A public device address.
        Public = ("public", "Public"),
        /// A random device address.
        #[default]
        Random = ("random", "Random"),
    }
}

sdrkit_core::choice_enum! {
    /// The tracker families that `send` of `bletracker` builds: those whose frame the detector reads.
    #[derive(Default)]
    pub enum SendTracker {
        /// Apple Find My.
        #[default]
        FindMy = ("find_my", "Apple Find My"),
        /// Google Find My Device.
        Google = ("google", "Google Find My Device"),
    }
}

/// The argument `address_type` of `send`.
pub const ADDRESS_TYPE: Setting<field::Choice<SendAddress>> =
    Setting::choice("address_type", "Address type", SendAddress::Random)
        .help("Public or random: the TxAdd bit of the header.");
/// The argument `pdu_type` of `send`.
pub const PDU_TYPE: Setting<field::Choice<SendPdu>> =
    Setting::choice("pdu_type", "PDU type", SendPdu::NonconnInd)
        .help("ADV_NONCONN_IND, ADV_IND or ADV_SCAN_IND.");
/// The argument `tracker` of `send` of `bletracker`.
pub const TRACKER: Setting<field::Choice<SendTracker>> =
    Setting::choice("tracker", "Tracker", SendTracker::FindMy)
        .help("The family whose frame is sent: Find My or Google.");
/// The peak of the signal sent.
pub const TX_LEVEL: Setting<field::F64> = Setting::new("tx_level", "Level", "of full scale", 0.5)
    .range(0.05, 0.9, 0.0)
    .help("The magnitude of the signal sent, of full scale.")
    .section(TX);

sdrkit_core::settings! {
    /// The settings of the sender.
    pub enum BleTxSetting {
        /// The level.
        Level = TX_LEVEL,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender of `ble`.
    pub enum BleTxCommand {
        /// Sends advertising packets.
        Send = BLE_SEND.command,
        /// Ends the send.
        Halt = BLE_HALT,
    }
}

sdrkit_core::commands! {
    /// The commands of the sender of `bletracker`.
    pub enum TrackerTxCommand {
        /// Sends tracker advertising packets.
        Send = BLETRACKER_SEND.command,
        /// Ends the send.
        Halt = BLE_HALT,
    }
}

/// Why a send is refused.
#[derive(Clone, Debug, PartialEq)]
pub enum TxError {
    /// The engine's transmit setting is off.
    Off,
    /// The stack has no input yet, so it has no rate.
    NoInput,
    /// The tuned frequency is not on a primary advertising channel.
    NotAdvertising(f64),
    /// The address is not 12 hex digits.
    Address,
    /// The argument of this key is not hex.
    Hex(&'static str),
    /// The data and the fields that build data are both given.
    DataAndFields,
    /// Manufacturer data without a company.
    NoCompany,
    /// The advertising data is longer than [`MAX_DATA`] bytes.
    Long(usize),
    /// The Find My payload is not [`FIND_MY_PAYLOAD`] bytes.
    FindMyPayload(usize),
    /// The Google payload is longer than [`GOOGLE_PAYLOAD`] bytes.
    GooglePayload(usize),
    /// An argument of this key is out of its range.
    Range(&'static str),
    /// [`MAX_WAITING`] sends wait already.
    Waiting,
    /// The packet cannot be made.
    Make(GenError),
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::NoInput => f.write_str("the stack has no input yet"),
            TxError::NotAdvertising(hz) => write!(
                f,
                "{:.3} MHz is not an advertising channel: tune to 2402, 2426 or 2480 MHz",
                hz / 1e6
            ),
            TxError::Address => f.write_str("the address is not 12 hex digits"),
            TxError::Hex(k) => write!(f, "{k} is not hex"),
            TxError::DataAndFields => {
                f.write_str("give the data or the name, UUID and manufacturer fields, not both")
            }
            TxError::NoCompany => f.write_str("manufacturer data needs the company"),
            TxError::Long(n) => write!(f, "the advertising data is {n} bytes, over {MAX_DATA}"),
            TxError::FindMyPayload(n) => {
                write!(f, "the Find My payload is {n} bytes, not {FIND_MY_PAYLOAD}")
            }
            TxError::GooglePayload(n) => {
                write!(f, "the Google payload is {n} bytes, over {GOOGLE_PAYLOAD}")
            }
            TxError::Range(k) => write!(f, "{k} is out of its range"),
            TxError::Waiting => write!(f, "{MAX_WAITING} sends wait already"),
            TxError::Make(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TxError {}

/// True if `v` is within the range of the argument `s`.
fn in_range(s: &Setting<field::I64>, v: i64) -> bool {
    s.range
        .is_none_or(|r| (r.min..=r.max).contains(&(v as f64)))
}

/// The bytes of hex text, with spaces, `:` and `-` between them allowed. `key` names the argument in the error.
fn hex(text: &str, key: &'static str) -> Result<Vec<u8>, TxError> {
    let t: String = text
        .chars()
        .filter(|c| !(c.is_whitespace() || matches!(c, ':' | '-')))
        .collect();
    let mut out = Vec::new();
    sdrkit_core::base64::unhex_into(&t, &mut out).map_err(|_| TxError::Hex(key))?;
    Ok(out)
}

/// A 16-bit value from 4 hex digits.
fn hex16(text: &str, key: &'static str) -> Result<u16, TxError> {
    match hex(text, key)?.as_slice() {
        [a, b] => Ok(u16::from_be_bytes([*a, *b])),
        _ => Err(TxError::Hex(key)),
    }
}

/// The advertising data of `send` of `ble`: the hex of `data`, or the structures that the name, the UUID and the
/// manufacturer fields build, in that order.
///
/// # Errors
///
/// [`TxError`] for text that is not hex, data together with the fields, manufacturer data without a company, or more
/// than [`MAX_DATA`] bytes.
pub fn ble_data(a: &BleSendArgs<'_>) -> Result<Vec<u8>, TxError> {
    let data = a.data.unwrap_or("").trim();
    let name = a.name.unwrap_or("");
    let uuid = a.uuid16.unwrap_or("").trim();
    let company = a.company.unwrap_or("").trim();
    let mdata = a.manufacturer_data.unwrap_or("").trim();
    let fields = !name.is_empty() || !uuid.is_empty() || !company.is_empty() || !mdata.is_empty();
    let d = if !data.is_empty() {
        if fields {
            return Err(TxError::DataAndFields);
        }
        hex(data, "data")?
    } else {
        let long = |_| TxError::Long(MAX_DATA + 1);
        let mut d = Vec::new();
        if !name.is_empty() {
            ad::push(&mut d, ad::COMPLETE_NAME, name.as_bytes()).map_err(long)?;
        }
        if !uuid.is_empty() {
            ad::push_uuids16(&mut d, &[hex16(uuid, "uuid16")?]).map_err(long)?;
        }
        if !company.is_empty() {
            let m = hex(mdata, "manufacturer_data")?;
            ad::push_manufacturer(&mut d, hex16(company, "company")?, &m).map_err(long)?;
        } else if !mdata.is_empty() {
            return Err(TxError::NoCompany);
        }
        d
    };
    if d.len() > MAX_DATA {
        return Err(TxError::Long(d.len()));
    }
    Ok(d)
}

/// The advertising data of `send` of `bletracker`: the Find My manufacturer data or the Google service data.
///
/// # Errors
///
/// [`TxError`] for a payload that is not hex or not of its length, or a status out of range.
pub fn tracker_data(a: &BletrackerSendArgs<'_>) -> Result<Vec<u8>, TxError> {
    let payload = hex(a.payload.unwrap_or(""), "payload")?;
    let mut d = Vec::new();
    match a.tracker.unwrap_or_default() {
        SendTracker::FindMy => {
            if payload.len() != FIND_MY_PAYLOAD {
                return Err(TxError::FindMyPayload(payload.len()));
            }
            let status =
                u8::try_from(a.status.unwrap_or(0)).map_err(|_| TxError::Range("status"))?;
            let mut m = FIND_MY_HEAD.to_vec();
            m.push(status);
            m.extend_from_slice(&payload);
            ad::push_manufacturer(&mut d, FIND_MY_COMPANY, &m)
                .map_err(|_| TxError::Long(m.len() + 4))?;
        }
        SendTracker::Google => {
            if payload.len() > GOOGLE_PAYLOAD {
                return Err(TxError::GooglePayload(payload.len()));
            }
            let mut s = vec![GOOGLE_FRAME[usize::from(a.protection.unwrap_or(false))]];
            s.extend_from_slice(&payload);
            ad::push_service_data16(&mut d, GOOGLE_UUID, &s)
                .map_err(|_| TxError::Long(s.len() + 4))?;
        }
    }
    Ok(d)
}

/// The packets of one `send`.
struct Job {
    /// Header and payload.
    pdu: Vec<u8>,
    /// The advertising channel.
    channel: u8,
    /// The packets.
    count: u32,
    /// From the start of one packet to the start of the next, s.
    interval_s: f64,
    /// The address text.
    target: String,
    /// What the events say of it.
    text: String,
}

/// The send on air.
struct OnAir {
    job: Job,
    /// The input sample at which the first packet is due.
    start: u64,
    /// The packets written.
    sent: u32,
    /// The stated time on air of the first packet, UTC s.
    first_at: Option<f64>,
}

impl OnAir {
    /// The input sample at which packet `k` is due.
    fn due(&self, k: u32, rate: f64) -> u64 {
        self.start + (f64::from(k) * self.job.interval_s * rate).round() as u64
    }
}

/// Which stack the sender belongs to: which `send` it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    Ble,
    Tracker,
}

/// The sender (`wlan.ble_tx`, `wlan.ble_tracker_tx`). See the module.
pub struct BleSender {
    form: Form,
    level: f64,
    rate: f64,
    dial: f64,
    transmit: bool,
    timing: BurstTiming,
    /// The input samples of the passes so far.
    clock: u64,
    /// The input sample from which a send that waited may start: the interval after the last packet of the send before it.
    free_from: u64,
    queue: VecDeque<Job>,
    air: Option<OnAir>,
    events: Vec<Event>,
    results: Published,
    shown: Option<(bool, u32, u64)>,
    scratch: Vec<Complex32>,
}

impl BleSender {
    fn new(form: Form) -> Self {
        let mut s = BleSender {
            form,
            level: TX_LEVEL.default,
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            timing: BurstTiming::default(),
            clock: 0,
            free_from: 0,
            queue: VecDeque::new(),
            air: None,
            events: Vec::new(),
            results: Published::new(BleTxFields::SHAPE.fields),
            shown: None,
            scratch: Vec::new(),
        };
        s.publish();
        s
    }

    /// The sender of `ble`: `send` takes the advertising data.
    pub fn ble() -> Self {
        Self::new(Form::Ble)
    }

    /// The sender of `bletracker`: `send` takes a tracker family and its frame.
    pub fn tracker() -> Self {
        Self::new(Form::Tracker)
    }

    fn event(&mut self, state: &str, reason: &str, target: &str, text: &str) {
        self.events.push(BLE_TX.event(
            0.0,
            self.dial,
            &BleTxEvent {
                state,
                reason,
                target,
                text,
                family: Family::Other,
            },
        ));
    }

    /// Drops the sends that wait and the rest of the send on air.
    fn cut(&mut self, reason: &str) {
        self.queue.clear();
        if let Some(a) = self.air.take() {
            self.event("finished", reason, &a.job.target, "");
        }
    }

    /// Checks a send and makes its job from the PDU type, the address, its type, the data, the count and the interval.
    fn job(
        &self,
        pdu: Option<SendPdu>,
        address: &str,
        kind: Option<SendAddress>,
        data: &[u8],
        count: Option<i64>,
        interval_ms: Option<i64>,
    ) -> Result<Job, TxError> {
        let random = kind.unwrap_or_default() == SendAddress::Random;
        let addr = parse_addr(address.trim(), random).ok_or(TxError::Address)?;
        let count = count.unwrap_or(BleSendArgs::COUNT.default);
        if !in_range(&BleSendArgs::COUNT, count) {
            return Err(TxError::Range("count"));
        }
        let interval_ms = interval_ms.unwrap_or(BleSendArgs::INTERVAL_MS.default);
        if !in_range(&BleSendArgs::INTERVAL_MS, interval_ms) {
            return Err(TxError::Range("interval_ms"));
        }
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.rate <= 0.0 {
            return Err(TxError::NoInput);
        }
        let channel = advertising_channel_at(self.dial, CHANNEL_TOLERANCE_HZ)
            .ok_or(TxError::NotAdvertising(self.dial))?;
        let pdu = pdu.unwrap_or_default();
        let mut p = Vec::new();
        build_adv(&mut p, pdu.pdu_type(), &addr, data)
            .map_err(|e| TxError::Make(GenError::Phy(e)))?;
        let mut text = format!("{} ch {channel} x{count} ", pdu.pdu_type().name());
        sdrkit_core::base64::hex_into(data, &mut text);
        Ok(Job {
            pdu: p,
            channel,
            count: count as u32,
            interval_s: interval_ms as f64 * 1e-3,
            target: addr.text(),
            text,
        })
    }

    /// Queues `job`; it goes on air at once when no send is on or waits. True when it went on air.
    fn enqueue(&mut self, job: Job) -> Result<bool, TxError> {
        if self.air.is_some() || !self.queue.is_empty() {
            if self.queue.len() >= MAX_WAITING {
                return Err(TxError::Waiting);
            }
            self.queue.push_back(job);
            return Ok(false);
        }
        self.air = Some(OnAir {
            job,
            start: self.clock,
            sent: 0,
            first_at: None,
        });
        Ok(true)
    }

    /// The samples of one packet of `job` at the session rate: its level, its rise and fall and its carrier on the
    /// channel.
    fn samples(&self, job: &Job) -> Result<Vec<Complex32>, GenError> {
        let mut s = packet_samples(
            &GenPacket::new(job.pdu.clone(), 0.0),
            job.channel,
            self.rate,
        )?;
        let shift = channel_hz(job.channel).unwrap_or(self.dial) - self.dial;
        let w = TAU * shift / self.rate;
        let mut ramp = Ramp::of_seconds(RAMP_S, self.rate);
        let (n, fall) = (s.len(), ramp.samples());
        for (i, z) in s.iter_mut().enumerate() {
            if !ramp.falling() && i + fall >= n {
                ramp.fall();
            }
            let g = ramp.gain(self.level as f32).unwrap_or(0.0);
            *z *= Complex32::from_polar(g, (w * i as f64) as f32);
        }
        Ok(s)
    }

    /// One pass: the packets due in it, each a burst with its end mark.
    fn run(&mut self, io: &mut Io) {
        let room = io.input_len(0);
        let end = self.clock + room as u64;
        loop {
            if self.air.is_none() {
                let Some(job) = self.queue.pop_front() else {
                    break;
                };
                self.event("started", "", &job.target, &job.text);
                self.air = Some(OnAir {
                    job,
                    start: self.clock.max(self.free_from),
                    sent: 0,
                    first_at: None,
                });
            }
            let Some(a) = &self.air else { break };
            if a.sent >= a.job.count {
                let a = self.air.take().expect("a send on air");
                self.free_from = a.due(a.sent, self.rate);
                self.event("finished", "", &a.job.target, "");
                continue;
            }
            if a.due(a.sent, self.rate) >= end {
                break;
            }
            let made = self.samples(&a.job);
            let pass_s = room as f64 / self.rate;
            let Some(a) = &mut self.air else { break };
            let samples = match made {
                Ok(s) => s,
                Err(e) => {
                    let target = std::mem::take(&mut a.job.target);
                    self.air = None;
                    self.event("finished", &e.to_string(), &target, "");
                    continue;
                }
            };
            if a.sent == 0 {
                a.first_at = self.timing.start_time(pass_s);
            }
            let at = a.first_at.map(|t| t + f64::from(a.sent) * a.job.interval_s);
            a.sent += 1;
            // Equal rates: the samples pass unchanged, so the burst cannot fail to build.
            if let Ok(mut out) = BurstOut::new(self.rate, self.rate, 1.0) {
                out.push(&samples);
                out.finish(&mut self.scratch);
                out.state_start(at);
                out.write(io, 0, room, &self.timing);
            }
        }
        self.clock = end;
        self.publish();
        io.messages_out(1).append(&mut self.events);
    }

    /// Writes the result fields when they changed.
    fn publish(&mut self) {
        let sent = self.air.as_ref().map_or(0, |a| a.sent);
        let left = self
            .air
            .as_ref()
            .map_or(0, |a| u64::from(a.job.count - a.sent))
            + self.queue.iter().map(|j| u64::from(j.count)).sum::<u64>();
        let now = (self.air.is_some(), sent, left);
        if self.shown == Some(now) {
            return;
        }
        self.shown = Some(now);
        self.results.put(&BleTxFields {
            tx_sending: now.0,
            tx_sent: i64::from(now.1),
            tx_left: now.2 as i64,
        });
    }

    /// Checks the arguments of `send`, makes the job and queues it.
    fn send(&mut self, args: &Values) -> Result<(), CommandError> {
        let (job, target) = match self.form {
            Form::Ble => {
                let a: BleSendArgs = BLE_SEND.args(args)?;
                let job = ble_data(&a).and_then(|d| {
                    self.job(
                        a.pdu_type,
                        a.address,
                        a.address_type,
                        &d,
                        a.count,
                        a.interval_ms,
                    )
                });
                (job, a.address.trim().to_owned())
            }
            Form::Tracker => {
                let a: BletrackerSendArgs = BLETRACKER_SEND.args(args)?;
                let job = tracker_data(&a).and_then(|d| {
                    self.job(
                        a.pdu_type,
                        a.address,
                        a.address_type,
                        &d,
                        a.count,
                        a.interval_ms,
                    )
                });
                (job, a.address.trim().to_owned())
            }
        };
        let r = job.and_then(|job| {
            let (t, text) = (job.target.clone(), job.text.clone());
            let started = self.enqueue(job)?;
            self.event(if started { "started" } else { "queued" }, "", &t, &text);
            Ok(())
        });
        if let Err(e) = &r {
            self.event("refused", &e.to_string(), &target, "");
        }
        r.map_err(|e| CommandError::Failed {
            source: StageError::new(e),
        })
    }
}

impl Default for BleSender {
    fn default() -> Self {
        Self::ble()
    }
}

impl Component for BleSender {
    fn info(&self) -> ComponentInfo {
        let name = match self.form {
            Form::Ble => "wlan.ble_tx",
            Form::Tracker => "wlan.ble_tracker_tx",
        };
        ComponentInfo::processor(
            name,
            vec![Port::iq("in")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(BleTxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        self.clock = 0;
        self.free_from = 0;
        if self.air.is_some() || !self.queue.is_empty() {
            self.cut("the format changed");
        }
        Ok(vec![
            Format::iq(self.rate, self.dial, self.dial),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn retuned(&mut self, centre_hz: f64, tuned_hz: f64) {
        self.dial = if tuned_hz > 0.0 { tuned_hz } else { centre_hz };
        if self.air.is_some() || !self.queue.is_empty() {
            self.cut("the frequency changed");
        }
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.timing.attach(ctx);
        if !ctx.transmit && (self.air.is_some() || !self.queue.is_empty()) {
            self.cut("the transmitter is off");
        }
    }
    fn process(&mut self, io: &mut Io) {
        self.run(io);
    }
    fn finish(&mut self, io: &mut Io) {
        self.run(io);
        if self.air.is_some() || !self.queue.is_empty() {
            self.cut("the input ended");
        }
        self.publish();
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match BleTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BleTxSetting::Level => self.level = TX_LEVEL.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(BleTxSetting::Level, Value::F64(self.level))])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        match self.form {
            Form::Ble => BleTxCommand::commands(),
            Form::Tracker => TrackerTxCommand::commands(),
        }
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let halt = match self.form {
            Form::Ble => match BleTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
                BleTxCommand::Send => false,
                BleTxCommand::Halt => true,
            },
            Form::Tracker => {
                match TrackerTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
                    TrackerTxCommand::Send => false,
                    TrackerTxCommand::Halt => true,
                }
            }
        };
        let r = if halt {
            self.cut("halted");
            Ok(())
        } else {
            self.send(args)
        };
        self.publish();
        r.map(|()| None)
    }
}
