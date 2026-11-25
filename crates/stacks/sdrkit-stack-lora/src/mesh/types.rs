//! The typed records of the mesh receiver: packet rows, node updates, messages, readings, and transmit requests.

use sdrkit_core::value::{Choice, ChoiceDecl};
use sdrkit_core::{EventDef, Record};
use sdrkit_proto_lora_phy::LoraParams;
use serde::{Deserialize, Serialize};

/// The protocols of the mesh receiver, and APRS on AX.25 (the `aprs` stack) for the rows of the messages it shares.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum Protocol {
    /// Meshtastic.
    #[default]
    Meshtastic,
    /// MeshCore.
    MeshCore,
    /// LoRa APRS.
    LoraAprs,
    /// MeshCom.
    MeshCom,
    /// LoRaWAN.
    Lorawan,
    /// Plain LoRa (no protocol accepted the frame).
    Lora,
    /// APRS on AX.25 (1200 bit/s AFSK and 9600 bit/s on FM).
    Aprs,
}

impl Choice for Protocol {
    const CHOICES: &'static [ChoiceDecl] = &[
        ChoiceDecl::new("meshtastic", "Meshtastic"),
        ChoiceDecl::new("meshcore", "MeshCore"),
        ChoiceDecl::new("loraaprs", "LoRa APRS"),
        ChoiceDecl::new("meshcom", "MeshCom"),
        ChoiceDecl::new("lorawan", "LoRaWAN"),
        ChoiceDecl::new("lora", "LoRa"),
        ChoiceDecl::new("aprs", "APRS"),
    ];
    fn index(self) -> u16 {
        self as u16
    }
    fn from_index(index: u16) -> Option<Self> {
        const ALL: [Protocol; 7] = [
            Protocol::Meshtastic,
            Protocol::MeshCore,
            Protocol::LoraAprs,
            Protocol::MeshCom,
            Protocol::Lorawan,
            Protocol::Lora,
            Protocol::Aprs,
        ];
        ALL.get(usize::from(index)).copied()
    }
}

impl Protocol {
    /// The mask bit.
    pub fn bit(self) -> u32 {
        match self {
            Protocol::Meshtastic => 1,
            Protocol::MeshCore => 2,
            Protocol::LoraAprs => 4,
            Protocol::MeshCom => 8,
            Protocol::Lorawan => 16,
            Protocol::Lora | Protocol::Aprs => 0,
        }
    }
    /// The name shown.
    pub fn name(self) -> &'static str {
        match self {
            Protocol::Meshtastic => "Meshtastic",
            Protocol::MeshCore => "MeshCore",
            Protocol::LoraAprs => "LoRa APRS",
            Protocol::MeshCom => "MeshCom",
            Protocol::Lorawan => "LoRaWAN",
            Protocol::Lora => "LoRa",
            Protocol::Aprs => "APRS",
        }
    }
    /// The key of the protocol in event kinds and row keys.
    pub fn kind(self) -> &'static str {
        match self {
            Protocol::Meshtastic => "meshtastic",
            Protocol::MeshCore => "meshcore",
            Protocol::LoraAprs => "loraaprs",
            Protocol::MeshCom => "meshcom",
            Protocol::Lorawan => "lorawan",
            Protocol::Lora => "lora",
            Protocol::Aprs => "aprs",
        }
    }
    /// The protocols of a mask, in the order they are tried (strict formats first).
    pub fn of_mask(mask: u32) -> impl Iterator<Item = Protocol> {
        [
            Protocol::LoraAprs,
            Protocol::MeshCom,
            Protocol::Lorawan,
            Protocol::Meshtastic,
            Protocol::MeshCore,
        ]
        .into_iter()
        .filter(move |p| mask & p.bit() != 0)
    }
}

/// One row of the packet list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeshPacket {
    /// Signal time, s.
    pub time: f64,
    /// Computer clock time (Unix s, 0 when times follow the signal).
    pub wall: f64,
    /// Protocol.
    pub protocol: Protocol,
    /// Preset name.
    pub preset: String,
    /// Frequency (channel centre + measured offset), Hz.
    pub freq_hz: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// SF.
    pub sf: u8,
    /// CR (from the header).
    pub cr: u8,
    /// BW, Hz.
    pub bw: f64,
    /// Level, dBFS.
    pub level_db: f64,
    /// SNR, dB.
    pub snr_db: f64,
    /// Size, bytes.
    pub size: usize,
    /// CRC right (or none).
    pub crc_ok: bool,
    /// A protocol parsed it.
    pub parsed: bool,
    /// Type.
    pub ptype: String,
    /// From.
    pub from: String,
    /// To.
    pub to: String,
    /// Channel.
    pub channel: String,
    /// Packet id.
    pub packet_id: String,
    /// Hop limit.
    pub hop_limit: Option<u8>,
    /// Hop start.
    pub hop_start: Option<u8>,
    /// Via MQTT.
    pub via_mqtt: bool,
    /// Next hop byte.
    pub next_hop: Option<u8>,
    /// Relay node byte.
    pub relay: Option<u8>,
    /// Decrypted.
    pub decrypted: bool,
    /// Note.
    pub note: String,
    /// Detail.
    pub summary: String,
    /// Path (MeshCore hashes, LoRa APRS and MeshCom relays).
    pub path: String,
    /// Raw payload text .
    pub raw: String,
    /// Hops (hop start - hop limit, the path hash count, or -1).
    pub hops: i32,
    /// LoRaWAN: network, MType, FPort, FRMPayload length, flags, MAC command names, MIC.
    pub lorawan: Option<LorawanCols>,
    /// The frame was sent by the own node.
    pub own: bool,
}

/// The LoRaWAN columns of a packet row.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LorawanCols {
    /// Network.
    pub network: String,
    /// MType name.
    pub mtype: String,
    /// FPort.
    pub fport: Option<u8>,
    /// FRMPayload length.
    pub frm_len: usize,
    /// Flags text.
    pub flags: String,
    /// MAC command names.
    pub mac: Vec<String>,
    /// MIC.
    pub mic: String,
}

impl sdrkit_graph::Item for MeshPacket {
    const KIND: &'static str = "mesh.packet";
}

sdrkit_core::choice_enum! {
    /// The delivery state of an own message, as the messages table shows it.
    pub enum DeliveryState {
        /// Built, waiting for the channel or the duty-cycle limit.
        Queued = ("queued", "Queued"),
        /// The duty-cycle limit holds it.
        Waiting = ("waiting", "Waiting for the duty cycle"),
        /// On the air at least once.
        Sent = ("sent", "Sent"),
        /// Another node was heard sending it again.
        AckedByMesh = ("acked_by_mesh", "Acknowledged by the mesh"),
        /// The destination acknowledged it.
        Delivered = ("delivered", "Delivered"),
        /// Only an implicit acknowledgement came for a direct message.
        Relayed = ("relayed", "Relayed, not confirmed"),
        /// The addressee refused it (APRS "rej").
        Rejected = ("rejected", "Rejected"),
        /// It failed.
        Failed = ("failed", "Failed"),
    }
}

sdrkit_core::choice_enum! {
    /// Why the delivery of an own message failed, as the messages table shows it. The keys of the Meshtastic routing
    /// errors are the lower-case names of `Routing.Error`.
    pub enum FailureReason {
        /// The transmit switch of the host is off, or the session has no radio that can transmit.
        TransmitOff = ("transmit_off", "Transmit is off"),
        /// The session listens to no channel of the network of the message, so there is no channel to send it on.
        NotListening = ("not_listening", "No channel of the network is received"),
        /// No acknowledgement came after the last try.
        NoAck = ("no_ack", "No acknowledgement"),
        /// Meshtastic: no route to the destination.
        NoRoute = ("no_route", "No route to the destination"),
        /// Meshtastic: a node refused to forward it.
        GotNak = ("got_nak", "Refused by a node on the way"),
        /// Meshtastic: the wait for an answer ended.
        Timeout = ("timeout", "Timed out"),
        /// Meshtastic: no interface can send it.
        NoInterface = ("no_interface", "No interface can send it"),
        /// Meshtastic: the last retransmission went unanswered.
        MaxRetransmit = ("max_retransmit", "No answer after the last retransmission"),
        /// Meshtastic: the channel of the message is not available at the node.
        NoChannel = ("no_channel", "The channel is not available"),
        /// Meshtastic: the packet is too large for the interface.
        TooLarge = ("too_large", "Too large"),
        /// Meshtastic: the destination has no service that answers the request.
        NoResponse = ("no_response", "No service answers"),
        /// Meshtastic: the duty-cycle limit holds it.
        DutyCycleLimit = ("duty_cycle_limit", "Duty-cycle limit"),
        /// Meshtastic: the service at the destination found the request not valid.
        BadRequest = ("bad_request", "Bad request"),
        /// Meshtastic: the service at the destination refused the request.
        NotAuthorized = ("not_authorized", "Not authorized"),
        /// Meshtastic: the node could not send it with public-key encryption.
        PkiFailed = ("pki_failed", "Public-key encryption failed"),
        /// Meshtastic: the destination has no public key to decrypt it with.
        PkiUnknownPubkey = ("pki_unknown_pubkey", "Public key unknown"),
        /// Meshtastic: an admin message with a bad or expired session key.
        AdminBadSessionKey = ("admin_bad_session_key", "Bad admin session key"),
        /// Meshtastic: an admin message from a public key that is not an admin key.
        AdminPublicKeyUnauthorized = ("admin_public_key_unauthorized", "Admin key not authorized"),
        /// Meshtastic: the airtime rate limit is exceeded.
        RateLimitExceeded = ("rate_limit_exceeded", "Rate limit exceeded"),
        /// Meshtastic: the node has no public key of the destination to send with.
        PkiSendFailPublicKey = ("pki_send_fail_public_key", "No public key of the destination"),
        /// Meshtastic: a routing error number that this stack does not know.
        RoutingError = ("routing_error", "Other routing error"),
    }
}

/// What a station answers when it is asked to send while its transmitter is off.
pub const TRANSMIT_OFF: &str =
    "transmit is off: the transmit switch of the host is off, or the session has no radio that can transmit";

/// The channel a client sends on: the channel of its network that the session listens to. The plan of the session is its only
/// source ([`super::plan::Plan::tx_channel`]); a client has no frequency of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct TxChannel {
    /// The LoRa setting.
    pub params: sdrkit_proto_lora_phy::LoraParams,
    /// Centre, Hz.
    pub freq_hz: f64,
}

/// What a station answers when it is asked to send while the session listens to no channel of its network.
pub const NO_CHANNEL: &str =
    "nothing is sent: the session listens to no channel of this network (none is in the captured band)";

/// Why an own message failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Failure {
    /// The transmitter is off.
    TransmitOff,
    /// The session listens to no channel of the network of the message, so there is no channel to send it on.
    NoChannel,
    /// No acknowledgement came after the last try.
    NoAck,
    /// A Meshtastic routing error, by its number.
    Routing(u32),
}

impl Failure {
    /// The reason in words.
    pub fn text(self) -> std::borrow::Cow<'static, str> {
        match self {
            Failure::TransmitOff => TRANSMIT_OFF.into(),
            Failure::NoChannel => NO_CHANNEL.into(),
            Failure::NoAck => "no acknowledgement".into(),
            Failure::Routing(n) => crate::proto::meshtastic::routing_error_name(n).into(),
        }
    }
    /// The reason as the messages table shows it. A routing error number that is not known gives
    /// [`FailureReason::RoutingError`].
    pub fn reason(self) -> FailureReason {
        match self {
            Failure::TransmitOff => FailureReason::TransmitOff,
            Failure::NoChannel => FailureReason::NotListening,
            Failure::NoAck => FailureReason::NoAck,
            Failure::Routing(n) => match n {
                1 => FailureReason::NoRoute,
                2 => FailureReason::GotNak,
                3 => FailureReason::Timeout,
                4 => FailureReason::NoInterface,
                5 => FailureReason::MaxRetransmit,
                6 => FailureReason::NoChannel,
                7 => FailureReason::TooLarge,
                8 => FailureReason::NoResponse,
                9 => FailureReason::DutyCycleLimit,
                32 => FailureReason::BadRequest,
                33 => FailureReason::NotAuthorized,
                34 => FailureReason::PkiFailed,
                35 => FailureReason::PkiUnknownPubkey,
                36 => FailureReason::AdminBadSessionKey,
                37 => FailureReason::AdminPublicKeyUnauthorized,
                38 => FailureReason::RateLimitExceeded,
                39 => FailureReason::PkiSendFailPublicKey,
                _ => FailureReason::RoutingError,
            },
        }
    }
}

/// The delivery state of an own message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Delivery {
    /// Built, waiting for the channel or the duty-cycle limit.
    Queued,
    /// The duty-cycle limit holds it.
    WaitingForDutyCycle,
    /// On the air at least once.
    Sent,
    /// Another node was heard rebroadcasting it.
    AckedByMesh,
    /// A routing ACK came from the destination (or the expected code, MeshCore, the ack, APRS).
    Delivered,
    /// Only an implicit acknowledgement for a direct message.
    RelayedNotConfirmed,
    /// Rejected (APRS "rej").
    Rejected,
    /// Failed, with the reason.
    Failed(Failure),
}

impl Delivery {
    /// The state as the table shows it.
    pub fn state(self) -> DeliveryState {
        match self {
            Delivery::Queued => DeliveryState::Queued,
            Delivery::WaitingForDutyCycle => DeliveryState::Waiting,
            Delivery::Sent => DeliveryState::Sent,
            Delivery::AckedByMesh => DeliveryState::AckedByMesh,
            Delivery::Delivered => DeliveryState::Delivered,
            Delivery::RelayedNotConfirmed => DeliveryState::Relayed,
            Delivery::Rejected => DeliveryState::Rejected,
            Delivery::Failed(_) => DeliveryState::Failed,
        }
    }
    /// The state in words, with the reason of a failure.
    pub fn text(self) -> String {
        let label = &DeliveryState::CHOICES[usize::from(self.state().index())].label;
        match self.failure() {
            Some(f) => format!("{}: {}", label.to_lowercase(), f.text()),
            None => label.to_lowercase(),
        }
    }
    /// The reason of a failure.
    pub fn failure(self) -> Option<Failure> {
        match self {
            Delivery::Failed(f) => Some(f),
            _ => None,
        }
    }
    /// The state is final.
    pub fn done(self) -> bool {
        matches!(
            self,
            Delivery::Delivered | Delivery::Failed(_) | Delivery::Rejected
        )
    }
}

/// One text message, sent or received.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeshText {
    /// Signal time, s.
    pub time: f64,
    /// Protocol.
    pub protocol: Protocol,
    /// Channel ("LongFast", "DM", "Public", ...).
    pub channel: String,
    /// From (node id, MeshCore: the sender name).
    pub from: String,
    /// The sender's name.
    pub from_name: String,
    /// To ("^all" or a node id).
    pub to: String,
    /// Text (at most 240 bytes).
    pub text: String,
    /// Hops.
    pub hops: i32,
    /// Packet id.
    pub packet_id: u32,
    /// Reply id.
    pub reply_id: u32,
    /// Emoji (a reaction).
    pub emoji: u32,
    /// The sender's clock (MeshCore).
    pub sender_time: u32,
    /// An own message.
    pub own: bool,
    /// Delivery state of an own message.
    pub state: Option<Delivery>,
    /// Distinct relays heard (own broadcasts).
    pub relays: u32,
    /// A number the client uses to update this message in place.
    pub key: u64,
    /// The signal to noise ratio of the frame of a received text, dB.
    #[serde(default)]
    pub snr_db: Option<f64>,
    /// The level of the frame of a received text, dB relative to full scale.
    #[serde(default)]
    pub level_dbfs: Option<f64>,
    /// The wall time when the text was received or sent (s since 1970), when the clock is known.
    #[serde(default)]
    pub wall: Option<f64>,
    /// A received text from a frame whose CRC failed. A later copy with a good CRC replaces it.
    #[serde(default)]
    pub damaged: bool,
    /// The copies of a received packet heard while the message is remembered, this one included. 0 for an own message.
    #[serde(default)]
    pub copies: u32,
    /// The fingerprint of the de-duplication key of a received text, which the store sets. 0 for an own message.
    #[serde(skip)]
    pub copy_key: u64,
}

impl sdrkit_graph::Item for MeshText {
    const KIND: &'static str = "mesh.text";
    fn event_def() -> Option<&'static EventDef> {
        Some(crate::typed::MESH_TEXT.def())
    }
    fn write_fields(&self, r: &mut Record) {
        use sdrkit_core::interface::Fields;
        crate::typed::MeshTextEvent {
            source: &self.from,
            target: &self.to,
            channel: &self.channel,
            text: &self.text,
            net: self.protocol,
            sender: (!self.from_name.is_empty()).then_some(self.from_name.as_str()),
            hops: (self.hops >= 0).then_some(i64::from(self.hops)),
            packet_id: i64::from(self.packet_id),
            reply_id: i64::from(self.reply_id),
            emoji: i64::from(self.emoji),
            own: self.own,
            state: self.state.map(Delivery::state),
            family: crate::results::family(self.protocol),
            snr_db: self.snr_db,
            level_dbfs: self.level_dbfs,
            crc_ok: (!self.own).then_some(!self.damaged),
        }
        .write(r);
    }
}

/// An update of one node record.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeshNodeUpdate {
    /// Protocol.
    pub protocol: Protocol,
    /// Node id ("!xxxxxxxx", or the 12 hex digits of a MeshCore key).
    pub id: String,
}

impl sdrkit_graph::Item for MeshNodeUpdate {
    const KIND: &'static str = "mesh.node";
}

/// A telemetry or position report (the readings table).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeshReading {
    /// Signal time.
    pub time: f64,
    /// Computer clock time.
    pub wall: f64,
    /// Node id.
    pub node: String,
    /// Battery percent.
    pub battery: Option<u32>,
    /// Volts.
    pub voltage: Option<f32>,
    /// Channel use.
    pub ch_util: Option<f32>,
    /// Air time.
    pub air_tx: Option<f32>,
    /// Temperature.
    pub temp: Option<f32>,
    /// Humidity.
    pub humidity: Option<f32>,
    /// Pressure.
    pub pressure: Option<f32>,
    /// Position.
    pub position: Option<(f64, f64, Option<f64>)>,
}

impl sdrkit_graph::Item for MeshReading {
    const KIND: &'static str = "mesh.reading";
}

sdrkit_core::choice_enum! {
    /// What a frame to send is.
    pub enum TxKind {
        /// A text message.
        Text = ("text", "Text"),
        /// A text message sent again after no acknowledgement.
        Retransmission = ("retransmission", "Retransmission"),
        /// An acknowledgement.
        Ack = ("ack", "Acknowledgement"),
        /// The own node information.
        NodeInfo = ("nodeinfo", "Node information"),
        /// The own position.
        Position = ("position", "Position"),
        /// A route trace.
        Traceroute = ("traceroute", "Route trace"),
        /// A telemetry request.
        Telemetry = ("telemetry", "Telemetry"),
        /// A packet of another node sent again.
        Rebroadcast = ("rebroadcast", "Rebroadcast"),
        /// A MeshCore advert to the whole mesh.
        Advert = ("advert", "Advert"),
        /// A MeshCore advert to the nodes in range.
        LocalAdvert = ("local_advert", "Local advert"),
        /// An APRS position beacon.
        Beacon = ("beacon", "Beacon"),
        /// Bytes that the user supplied.
        Raw = ("raw", "Raw"),
        /// Withdraws a waiting request of the same id.
        Cancel = ("cancel", "Cancel"),
    }
}

/// How an AX.25 frame goes on FM: the speed, the flags before it and the emphasis of the audio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AfskTx {
    /// 1200 (AFSK, Bell 202 tones) or 9600 (G3RUH), bit/s.
    pub baud: u32,
    /// The flags before the frame (TX delay), s.
    pub txdelay_s: f64,
    /// The 1200 bit/s audio is pre-emphasised (6 dB an octave above 300 Hz), as the microphone path of an FM radio does.
    pub pre_emphasis: bool,
}

impl Default for AfskTx {
    fn default() -> Self {
        AfskTx {
            baud: 1200,
            txdelay_s: 0.3,
            pre_emphasis: false,
        }
    }
}

impl AfskTx {
    /// The flags of the TX delay, at least one.
    pub fn flags(&self) -> usize {
        (self.txdelay_s * f64::from(self.baud) / 8.0)
            .ceil()
            .max(1.0) as usize
    }

    /// The air time of a frame of `len` bytes without the FCS, s: the TX delay, the frame and its FCS with the most bit
    /// stuffing (one bit in five), and three flags after it.
    pub fn air_s(&self, len: usize) -> f64 {
        let bits = self.flags() * 8 + (len + 2) * 8 * 6 / 5 + 24;
        bits as f64 / f64::from(self.baud)
    }
}

/// A frame to send (`tx.request`).
#[derive(Clone, Debug, PartialEq)]
pub struct TxRequest {
    /// A number for the reply.
    pub id: u64,
    /// The payload.
    pub payload: Vec<u8>,
    /// The LoRa setting (`None` for AX.25 on FM).
    pub lora: Option<LoraParams>,
    /// How an AX.25 frame goes on FM, when not LoRa.
    pub afsk: Option<AfskTx>,
    /// Absolute carrier frequency, Hz.
    pub freq_hz: f64,
    /// Earliest transmit time, signal seconds (0: as soon as the channel is free).
    pub not_before: f64,
    /// Contention window exponent (3 for a new packet, up to 7 for a rebroadcast).
    pub cw: u8,
    /// What it is.
    pub what: TxKind,
    /// The own message it carries (its key), when any.
    pub message_key: Option<u64>,
}

impl TxRequest {
    /// The air time of the frame, s: of the LoRa frame, or of the AX.25 frame with its flags. 0.5 s for an AX.25 frame
    /// without its speed.
    pub fn air_s(&self) -> f64 {
        match (&self.lora, &self.afsk) {
            (Some(p), _) => p.airtime(self.payload.len()),
            (None, Some(a)) => a.air_s(self.payload.len()),
            (None, None) => 0.5,
        }
    }
}

impl sdrkit_graph::Item for TxRequest {
    const KIND: &'static str = "tx.request";
    fn event_def() -> Option<&'static EventDef> {
        Some(crate::typed::TX_REQUEST.def())
    }
    fn write_fields(&self, r: &mut Record) {
        use sdrkit_core::interface::{Fields, Hertz, Seconds};
        let setting = self.lora.as_ref().map(LoraParams::describe);
        crate::typed::TxRequestEvent {
            id: i64::try_from(self.id).unwrap_or(i64::MAX),
            what: self.what,
            payload: self.payload.as_slice().into(),
            freq_hz: Hertz(self.freq_hz),
            not_before_s: self.not_before,
            air_time: Some(Seconds(self.air_s())),
            message_key: self
                .message_key
                .map(|k| i64::try_from(k).unwrap_or(i64::MAX)),
            setting: setting.as_deref(),
            afsk_baud: self.afsk.map(|a| i64::from(a.baud)),
        }
        .write(r);
    }
}

/// The printable form of a payload : 0x20..0x7E as they are, every other byte as `\xNN`, at most 255 bytes.
pub fn printable_raw(b: &[u8]) -> String {
    let mut s = String::new();
    for &c in b.iter().take(255) {
        if (0x20..0x7F).contains(&c) {
            s.push(c as char);
        } else {
            s += &format!("\\x{c:02X}");
        }
    }
    s
}

/// Text with control characters (but newline) as blanks, cut to 240 bytes on a character boundary.
pub fn message_text(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect();
    crate::proto::meshtastic::cut_bytes(&t, 240).to_string()
}
