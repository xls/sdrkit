//! The typed records of the digital voice stacks: calls, messages, positions, sites, grants and control messages. Calls,
//! messages and positions leave the graph as events.

use crate::typed::{
    CallEvent, GrantRow, MessageEvent, MessageRow, PositionEvent, PositionRow, SiteRow, CALL,
    MESSAGE, POSITION,
};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::{EventDef, Record, Value};
use sdrkit_graph::Item;
use sdrkit_util_objects::calls::{Call, CallEnd, SameCall};
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{Seen, Shift};
use serde::{Deserialize, Serialize};
use std::fmt;

/// An identity: a number (radio ID, talkgroup), a callsign, or nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Ident {
    /// No identity.
    #[default]
    None,
    /// A radio ID or a talkgroup.
    Number(u64),
    /// A callsign.
    Text(String),
}

impl Ident {
    /// Returns true when there is no identity.
    pub fn is_none(&self) -> bool {
        matches!(self, Ident::None)
    }
    /// The identity as text: the number in decimal, the callsign, or empty.
    pub fn text(&self) -> String {
        self.to_string()
    }
    /// The number, for a numeric identity.
    pub fn number(&self) -> Option<u64> {
        match self {
            Ident::Number(n) => Some(*n),
            _ => None,
        }
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ident::None => Ok(()),
            Ident::Number(n) => write!(f, "{n}"),
            Ident::Text(t) => f.write_str(t),
        }
    }
}

impl From<u64> for Ident {
    fn from(n: u64) -> Self {
        Ident::Number(n)
    }
}
impl From<u32> for Ident {
    fn from(n: u32) -> Self {
        Ident::Number(u64::from(n))
    }
}
impl From<&str> for Ident {
    fn from(t: &str) -> Self {
        Ident::Text(t.into())
    }
}
impl From<String> for Ident {
    fn from(t: String) -> Self {
        Ident::Text(t)
    }
}

sdrkit_core::choice_enum! {
    /// What a transmission carries.
    #[derive(Default, Serialize, Deserialize)]
    pub enum CallKind {
        /// Speech to one radio or to a group that the stack does not mark.
        #[default]
        Voice = ("voice", "Voice"),
        /// Speech to a talkgroup.
        GroupVoice = ("group_voice", "Group voice"),
        /// Speech to one radio.
        PrivateVoice = ("private_voice", "Private voice"),
        /// Speech to every radio.
        AllCall = ("all_call", "All call"),
        /// Data.
        Data = ("data", "Data"),
        /// A packet.
        Packet = ("packet", "Packet"),
        /// A status message.
        Status = ("status", "Status"),
        /// A short text message.
        ShortMessage = ("short_message", "Short message"),
        /// Short data.
        ShortData = ("short_data", "Short data"),
        /// A control message.
        Control = ("control", "Control"),
        /// A channel grant.
        Grant = ("grant", "Grant"),
    }
}

impl CallKind {
    /// Returns true for speech.
    pub fn is_voice(self) -> bool {
        matches!(
            self,
            CallKind::Voice | CallKind::GroupVoice | CallKind::PrivateVoice | CallKind::AllCall
        )
    }
}

sdrkit_core::choice_enum! {
    /// Whether the content of a call is in the clear.
    #[derive(Default, Serialize, Deserialize)]
    pub enum EncState {
        /// Not known yet. It counts as encrypted for the media output.
        #[default]
        Unknown = ("unknown", "Unknown"),
        /// In the clear.
        Clear = ("clear", "Clear"),
        /// Encrypted.
        Encrypted = ("encrypted", "Encrypted"),
        /// Decrypted with a key that the user loaded.
        Decrypted = ("decrypted", "Decrypted"),
    }
}

/// The encryption of a call.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Encryption {
    /// Encrypted or not.
    pub state: EncState,
    /// The algorithm id as the stack sends it, when it does.
    pub alg_id: Option<u32>,
    /// A name of the algorithm for people ("AES-256", "DES-OFB", "basic privacy").
    pub alg: String,
    /// The key id, when the stack sends it.
    pub key_id: Option<u32>,
}

impl Encryption {
    /// Content in the clear.
    pub fn clear() -> Self {
        Encryption {
            state: EncState::Clear,
            ..Default::default()
        }
    }
    /// Encrypted content with the algorithm id `alg_id`, the algorithm name `alg` and the key id `key_id`.
    pub fn encrypted(alg_id: Option<u32>, alg: &str, key_id: Option<u32>) -> Self {
        Encryption {
            state: EncState::Encrypted,
            alg_id,
            alg: alg.into(),
            key_id,
        }
    }
    /// Returns true when the content is encrypted.
    pub fn is_encrypted(&self) -> bool {
        matches!(self.state, EncState::Encrypted)
    }
}

sdrkit_core::choice_enum! {
    /// Which access code a stack sends with a transmission.
    #[derive(Serialize, Deserialize)]
    pub enum AccessKind {
        /// The colour code (DMR, dPMR).
        ColourCode = ("colour_code", "Colour code"),
        /// The network access code (P25).
        Nac = ("nac", "NAC"),
        /// The radio access number (NXDN).
        Ran = ("ran", "RAN"),
        /// The channel access number (M17).
        Can = ("can", "CAN"),
        /// The squelch code (System Fusion).
        SquelchCode = ("squelch_code", "Squelch code"),
    }
}

/// The access code of a transmission: its kind and its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Access {
    /// Which code.
    pub kind: AccessKind,
    /// Its value.
    pub code: u32,
}

impl Access {
    /// The code `code` of kind `kind`.
    pub fn new(kind: AccessKind, code: u32) -> Self {
        Access { kind, code }
    }
}

sdrkit_core::choice_enum! {
    /// A fact about one call that only some stacks send. Each is a row of the `call_details` table.
    #[derive(Serialize, Deserialize)]
    pub enum CallDetail {
        /// The symbol rate the call was heard at, Hz (NXDN).
        Baud = ("baud", "Symbol rate"),
        /// The call type (NXDN, dPMR).
        CallType = ("call_type", "Call type"),
        /// The communication mode (dPMR).
        CommMode = ("comm_mode", "Communication mode"),
        /// The status code (NXDN).
        Status = ("status", "Status"),
        /// The channel of a grant ("1-0010", P25).
        Channel = ("channel", "Channel"),
        /// The frequency of a grant, Hz (P25).
        FreqHz = ("freq_hz", "Frequency"),
        /// The service access point (P25 data).
        Sap = ("sap", "SAP"),
        /// The blocks of a data packet (P25).
        Blocks = ("blocks", "Blocks"),
        /// The caller (D-STAR MY).
        My = ("my", "MY"),
        /// The caller's suffix (D-STAR).
        Suffix = ("suffix", "Suffix"),
        /// The called station (D-STAR UR).
        Ur = ("ur", "UR"),
        /// Repeater 1 (D-STAR).
        Rpt1 = ("rpt1", "RPT1"),
        /// Repeater 2 (D-STAR).
        Rpt2 = ("rpt2", "RPT2"),
        /// The first flag byte of the header (D-STAR).
        Flag1 = ("flag1", "Flag 1"),
        /// The header flags for people (D-STAR).
        Flags = ("flags", "Flags"),
        /// The header decoded (D-STAR).
        HeaderOk = ("header_ok", "Header good"),
        /// The header came from the slow data (D-STAR).
        HeaderFromSlowData = ("header_from_slow_data", "Header from slow data"),
        /// The digital squelch code (D-STAR).
        Squelch = ("squelch", "Squelch code"),
        /// Where the caller is.
        Position = ("position", "Position"),
        /// The stream type (M17).
        StreamType = ("stream_type", "Stream type"),
        /// The encryption type (M17).
        EncryptionType = ("encryption_type", "Encryption type"),
        /// The link setup came from the LICH (M17).
        LsfFromLich = ("lsf_from_lich", "Link setup from LICH"),
        /// The stream is signed (M17).
        Signed = ("signed", "Signed"),
        /// The data text of a stream (M17).
        Data = ("data", "Data"),
        /// Frames of a packet (M17).
        Frames = ("frames", "Frames"),
        /// The packet protocol (M17).
        Protocol = ("protocol", "Protocol"),
        /// The feature set id (DMR).
        Fid = ("fid", "Feature set"),
        /// The service options (DMR).
        ServiceOptions = ("service_options", "Service options"),
        /// A note for people (DMR control).
        Note = ("note", "Note"),
        /// The control opcode (DMR).
        Opcode = ("opcode", "Opcode"),
        /// The short data format code (DMR).
        Format = ("format", "Format"),
        /// The packet CRC was good (DMR).
        CrcOk = ("crc_ok", "CRC good"),
        /// The data packet format (DMR).
        Dpf = ("dpf", "Packet format"),
        /// The downlink callsign (System Fusion).
        Downlink = ("downlink", "Downlink"),
        /// The uplink callsign (System Fusion).
        Uplink = ("uplink", "Uplink"),
        /// The data type (System Fusion).
        DataType = ("data_type", "Data type"),
        /// The call identifier (TETRA).
        CallId = ("call_id", "Call identifier"),
        /// The address of the called group (TETRA).
        Address = ("address", "Address"),
        /// The mobile country code of the cell (TETRA).
        Mcc = ("mcc", "MCC"),
        /// The mobile network code of the cell (TETRA).
        Mnc = ("mnc", "MNC"),
    }
}

impl CallDetail {
    /// The key of the detail (`"my"`).
    pub fn key(self) -> &'static str {
        match &<Self as sdrkit_core::Choice>::CHOICES[usize::from(sdrkit_core::Choice::index(self))]
            .key
        {
            std::borrow::Cow::Borrowed(k) => k,
            std::borrow::Cow::Owned(_) => "",
        }
    }
}

/// What the stacks record of one transmission. The call log keeps its id, its times, its frame count, the late entry flag and
/// how it ended.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CallRecord {
    /// The carrier frequency, Hz.
    pub carrier_hz: f64,
    /// The time slot or logical channel, 0 when the stack has none.
    pub slot: u8,
    /// What the transmission carries.
    pub kind: CallKind,
    /// Who sends.
    pub src: Ident,
    /// Who is called.
    pub dst: Ident,
    /// The target is a group.
    pub group: bool,
    /// An emergency call.
    pub emergency: bool,
    /// The encryption.
    pub encryption: Encryption,
    /// The access code.
    pub access: Option<Access>,
    /// The talker alias.
    pub alias: String,
    /// Text carried by the transmission (a message, slow data text).
    pub text: String,
    /// Voice frames received.
    pub voice_frames: u32,
    /// Voice frames passed to the media output.
    pub media_frames: u32,
    /// Bit errors counted.
    pub bit_errors: u64,
    /// Bits counted.
    pub bits: u64,
    /// The facts that only some stacks send, at most one value for each.
    pub details: Vec<(CallDetail, Value)>,
}

impl CallRecord {
    /// Sets the detail `key` to `v`, in place of an earlier value.
    pub fn set(&mut self, key: CallDetail, v: Value) {
        match self.details.iter_mut().find(|(k, _)| *k == key) {
            Some(d) => d.1 = v,
            None => self.details.push((key, v)),
        }
    }
    /// The value of detail `key`.
    pub fn detail(&self, key: CallDetail) -> Option<&Value> {
        self.details.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }
    /// The bit error rate of the call, or 0 before any bit was counted.
    pub fn ber(&self) -> f32 {
        if self.bits > 0 {
            self.bit_errors as f32 / self.bits as f32
        } else {
            0.0
        }
    }
}

/// The same call: the same kind, source, target and group flag.
impl SameCall for CallRecord {
    fn same_call(&self, o: &Self) -> bool {
        self.kind == o.kind && self.src == o.src && self.dst == o.dst && self.group == o.group
    }
}

impl Shift for CallRecord {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// One call of the log as it goes out on the `call` stream and as a `dvoice.call` event: the call with its record.
#[derive(Clone, Debug, PartialEq)]
pub struct CallItem(pub Call<u32, CallRecord>);

impl std::ops::Deref for CallItem {
    type Target = Call<u32, CallRecord>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CallItem {
    /// Signal time of the start, seconds.
    pub fn start_s(&self) -> f64 {
        self.0.start.t
    }
    /// Signal time of the last frame or the terminator, seconds.
    pub fn end_s(&self) -> f64 {
        self.0.last_s
    }
}

/// Returns true when the call ended by its terminator, or was a single transmission.
pub fn end_seen(e: CallEnd) -> bool {
    matches!(e, CallEnd::Terminator | CallEnd::Single)
}

impl Item for CallItem {
    const KIND: &'static str = "dvoice.call";
    fn event_def() -> Option<&'static EventDef> {
        Some(CALL.def())
    }
    fn write_fields(&self, f: &mut Record) {
        let c = &self.0;
        let r = &c.record;
        let (src, dst) = (r.src.text(), r.dst.text());
        let slot = r.slot.to_string();
        CallEvent {
            source: &src,
            target: &dst,
            channel: if r.group {
                Some(&dst)
            } else {
                (r.slot > 0).then_some(slot.as_str())
            },
            text: (!r.text.is_empty()).then_some(r.text.as_str()),
            urgency: if r.emergency {
                Urgency::Emergency
            } else {
                Urgency::None
            },
            family: Family::Voice,
            private: r.encryption.is_encrypted(),
            id: c.id as i64,
            kind: r.kind,
            group: r.group,
            end: c.end,
            duration: (!c.is_active()).then(|| Seconds(c.duration_s())),
        }
        .write(f);
    }
}

sdrkit_core::choice_enum! {
    /// How a message was sent.
    #[derive(Default, Serialize, Deserialize)]
    pub enum MessageFormat {
        /// DMR short data.
        #[default]
        ShortData = ("short_data", "Short data"),
        /// Packet data.
        PacketData = ("packet_data", "Packet data"),
        /// D-STAR slow data.
        SlowData = ("slow_data", "Slow data"),
        /// An M17 SMS packet.
        Sms = ("sms", "SMS"),
        /// M17 META text.
        MetaText = ("meta_text", "META text"),
        /// FreeDV text.
        FreedvText = ("freedv_text", "FreeDV text"),
        /// System Fusion text.
        YsfText = ("ysf_text", "System Fusion text"),
        /// A dPMR short message.
        ShortMessage = ("short_message", "Short message"),
        /// A dPMR packet.
        Packet = ("packet", "Packet"),
    }
}

/// A text message (`dvoice.message`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MessageRecord {
    /// Who sent it.
    pub src: Ident,
    /// Who it is for.
    pub dst: Ident,
    /// The target is a group.
    pub group: bool,
    /// How it was sent.
    pub format: MessageFormat,
    /// The DMR short data format code, when the message is DMR short data.
    pub data_format: Option<u8>,
    /// The text.
    pub text: String,
    /// The error check was good.
    pub crc_ok: bool,
    /// Signal time, seconds.
    pub time: f64,
    /// The time slot, 0 when the stack has none.
    pub slot: u8,
    /// An SMS.
    pub sms: bool,
}

impl Shift for MessageRecord {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Item for MessageRecord {
    const KIND: &'static str = "dvoice.message";
    fn event_def() -> Option<&'static EventDef> {
        Some(MESSAGE.def())
    }
    fn write_fields(&self, f: &mut Record) {
        MessageEvent {
            source: &self.src.text(),
            target: &self.dst.text(),
            text: &self.text,
            family: Family::Message,
            format: self.format,
            group: self.group,
        }
        .write(f);
    }
}

impl Shaped for MessageRecord {
    type Row = MessageRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&MessageRow {
            time_s: self.time,
            src: &self.src.text(),
            dst: &self.dst.text(),
            group: self.group,
            format: self.format,
            data_format: self.data_format.map(i64::from),
            text: &self.text,
            crc_ok: self.crc_ok,
            slot: i64::from(self.slot),
            sms: self.sms,
        });
    }
}

sdrkit_core::choice_enum! {
    /// How a position was sent.
    #[derive(Default, Serialize, Deserialize)]
    pub enum PositionFormat {
        /// DMR location report (LRRP).
        #[default]
        Lrrp = ("lrrp", "LRRP"),
        /// NMEA sentences.
        Nmea = ("nmea", "NMEA"),
        /// D-STAR D-PRS.
        Dprs = ("dprs", "D-PRS"),
        /// M17 GNSS META.
        M17Gnss = ("m17_gnss", "M17 GNSS"),
        /// System Fusion GPS.
        YsfGps = ("ysf_gps", "System Fusion GPS"),
        /// DMR GPS.
        DmrGps = ("dmr_gps", "DMR GPS"),
    }
}

sdrkit_core::choice_enum! {
    /// What kind of station sent a position (M17).
    #[derive(Serialize, Deserialize)]
    pub enum StationType {
        /// A fixed station.
        Fixed = ("fixed", "Fixed"),
        /// A mobile station.
        Mobile = ("mobile", "Mobile"),
        /// A handheld.
        Handheld = ("handheld", "Handheld"),
        /// Another kind.
        Other = ("other", "Other"),
    }
}

/// Kilometres an hour in one knot.
const KMH_PER_KT: f64 = 1.852;

/// A position (`dvoice.position`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PositionRecord {
    /// Who sent it.
    pub src: Ident,
    /// Latitude, degrees north.
    pub lat: f64,
    /// Longitude, degrees east.
    pub lon: f64,
    /// Altitude, metres.
    pub alt_m: Option<f64>,
    /// Speed, km/h.
    pub speed_kmh: Option<f64>,
    /// Course, degrees.
    pub course_deg: Option<f64>,
    /// Accuracy, metres.
    pub accuracy_m: Option<f64>,
    /// How it was sent.
    pub source: PositionFormat,
    /// The comment sent with it.
    pub comment: String,
    /// The kind of station (M17).
    pub station: Option<StationType>,
    /// The data source code (M17).
    pub data_source: Option<u8>,
    /// Signal time, seconds.
    pub time: f64,
}

impl Shift for PositionRecord {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Item for PositionRecord {
    const KIND: &'static str = "dvoice.position";
    fn event_def() -> Option<&'static EventDef> {
        Some(POSITION.def())
    }
    fn write_fields(&self, f: &mut Record) {
        PositionEvent {
            source: &self.src.text(),
            position: (self.lat, self.lon),
            height_m: self.alt_m,
            speed_kt: self.speed_kmh.map(|v| v / KMH_PER_KT),
            course_deg: self.course_deg,
            text: (!self.comment.is_empty()).then_some(self.comment.as_str()),
            family: Family::Voice,
            format: self.source,
        }
        .write(f);
    }
}

impl Shaped for PositionRecord {
    type Row = PositionRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&PositionRow {
            time_s: self.time,
            src: &self.src.text(),
            position: (self.lat, self.lon),
            height_m: self.alt_m,
            speed_kt: self.speed_kmh.map(|v| v / KMH_PER_KT),
            course_deg: self.course_deg,
            accuracy_m: self.accuracy_m,
            format: self.source,
            comment: &self.comment,
            station_type: self.station,
            data_source: self.data_source.map(i64::from),
        });
    }
}

/// The identity and broadcast parameters of a system or site. Each value is `None` when the stack does not send it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SiteRecord {
    /// Every part of the identity that the stack sends is known.
    pub valid: bool,
    /// The system id (P25, NXDN).
    pub system: Option<u32>,
    /// The wide area communications network (P25).
    pub wacn: Option<u32>,
    /// The RF subsystem (P25).
    pub rfss: Option<u32>,
    /// The site (P25, NXDN).
    pub site: Option<u32>,
    /// The network access code (P25).
    pub nac: Option<u32>,
    /// The control channel ("1-0010", P25).
    pub control_channel: Option<String>,
    /// The location id (NXDN).
    pub location: Option<u32>,
    /// The radio access number (NXDN).
    pub ran: Option<u32>,
    /// The country code (TETRAPOL, the MCC of TETRA).
    pub country: Option<u32>,
    /// The network code (TETRAPOL, the MNC of TETRA).
    pub network: Option<u32>,
    /// The base station (TETRAPOL).
    pub base_station: Option<u32>,
    /// The channel (TETRAPOL).
    pub channel: Option<u32>,
    /// The colour code (TETRA).
    pub colour_code: Option<u32>,
    /// The location area (TETRA).
    pub location_area: Option<u32>,
    /// The main carrier number (TETRA).
    pub main_carrier: Option<u32>,
    /// The frequency band (TETRA).
    pub band: Option<u32>,
    /// The main carrier frequency, Hz (TETRA).
    pub main_carrier_hz: Option<f64>,
    /// Signal time, seconds.
    pub time: f64,
}

impl Shift for SiteRecord {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl SiteRecord {
    /// The row of the site in the `sites` table.
    pub fn row(&self) -> SiteRow<'_> {
        fn n(v: Option<u32>) -> Option<i64> {
            v.map(i64::from)
        }
        SiteRow {
            time_s: self.time,
            valid: self.valid,
            system: n(self.system),
            wacn: n(self.wacn),
            rfss: n(self.rfss),
            site: n(self.site),
            nac: n(self.nac),
            control_channel: self.control_channel.as_deref(),
            location: n(self.location),
            ran: n(self.ran),
            country: n(self.country),
            network: n(self.network),
            base_station: n(self.base_station),
            channel: n(self.channel),
            colour_code: n(self.colour_code),
            location_area: n(self.location_area),
            main_carrier: n(self.main_carrier),
            band: n(self.band),
            main_carrier_hz: self.main_carrier_hz.map(Hertz),
        }
    }
}

/// A channel grant.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GrantRecord {
    /// The talkgroup or radio granted.
    pub target: u64,
    /// The target is a group.
    pub group: bool,
    /// The source of the request.
    pub src: u64,
    /// The channel number as sent.
    pub channel: u32,
    /// The channel for people ("1-0010").
    pub channel_text: String,
    /// The channel frequency, Hz, or 0 when it cannot be placed.
    pub freq_hz: f64,
    /// The time slot.
    pub slot: Option<u8>,
    /// An emergency call.
    pub emergency: bool,
    /// The call is encrypted.
    pub encrypted: bool,
    /// 1 high to 7 low, 4 by default.
    pub priority: u8,
    /// An update of a running call.
    pub update: bool,
    /// Signal time, seconds.
    pub time: f64,
}

impl Shift for GrantRecord {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for GrantRecord {
    type Row = GrantRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&GrantRow {
            time_s: self.time,
            target: self.target as i64,
            group: self.group,
            source_id: self.src as i64,
            channel_number: i64::from(self.channel),
            channel: &self.channel_text,
            freq_hz: (self.freq_hz > 0.0).then_some(Hertz(self.freq_hz)),
            slot: self.slot.map(i64::from),
            emergency: self.emergency,
            encrypted: self.encrypted,
            priority: i64::from(self.priority),
            update: self.update,
        });
    }
}

/// One decoded control message. Its text goes to the results log.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControlRecord {
    /// The opcode.
    pub opcode: u32,
    /// The manufacturer id.
    pub mfid: u32,
    /// The name of the message.
    pub name: String,
    /// The message for people.
    pub text: String,
    /// Signal time, seconds.
    pub time: f64,
    /// The time slot, 0 when the stack has none.
    pub slot: u8,
}
