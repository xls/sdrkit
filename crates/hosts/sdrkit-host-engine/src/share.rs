//! A share: the samples of one radio served to network clients over rtl_tcp and SoapyRemote.
//!
//! A share is an owner of the engine with a session of its own on the radio. The session never steers the radio: it
//! takes each block as the radio gives it and queues it in the hub of `sdrkit-serve-core`, which never waits. A client
//! that is slow loses only its own samples. The requests of the clients go to the [`Policy`] of the share, on the
//! reader threads of the clients; it answers from values that the control thread keeps up to date and never takes a
//! lock of the engine. What a client may change on the radio is queued and carried out at the next tick.

use crate::engine_owner::EngineOwner;
use crate::feed::{lock, BLOCK};
use crate::owners::ShareId;
use crate::published::Kept;
use crate::streams::Streams;
use crate::{radio_key, session_key, EngineError, OwnerRef, RadioId, SessionId, Shared};
use sdrkit_core::command::CommandSet;
use sdrkit_core::interface::{Arguments, CommandKind, EventKind, Fields, Hertz, Table, Time};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Clock, CommandError, CommandId, Commands, Complex32, ConfigError, Event, EventCatalog,
    InputInfo, Output, Owner, Published, Receiver, Record, Results, ResultsDecl, Schema, Setting,
    SettingError, SettingId, SettingSet, Transport, Value, Values,
};
use sdrkit_driver_core::sample::{from_iq, size, to_iq};
use sdrkit_driver_core::Capabilities;
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::SampleType;
use sdrkit_serve_core::{
    Answer, Block, Cidr, ClientHandle, Config, Control, Feeder, Hub, Leave, Listen, Protocol,
    Refusal, Request, ServeError, Slice, TxReader, TxRequest,
};
use sdrkit_serve_soapyremote::{Device, Range, Side, SoapyRemote, Ssdp};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, SystemTime};

/// The address ranges that may connect to a share unless the host sets others: this machine and the private networks.
pub const DEFAULT_ALLOW: &str =
    "127.0.0.0/8, ::1, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16, fc00::/7, fe80::/10";
/// The local address that a listener takes unless the command names another: every interface.
pub const DEFAULT_HOST: &str = "0.0.0.0";
/// The events a share keeps for the host. The oldest go first.
const EVENT_CAP: usize = 256;
/// The lowest rate that a client gets from a share. Below the lowest rate of the radio, the share brings the capture
/// down to the client's rate in the client's writer thread.
pub const OWN_RATE_FLOOR_HZ: f64 = 48e3;

sdrkit_core::choice_enum! {
    /// Who sets the frequency, the rate and the gain of a shared radio.
    pub enum ShareControl {
        /// The host alone. A request of a client to tune, to set the rate or the gain is not carried out and is counted;
        /// the client is told the value in force.
        Follow = ("follow", "Follow the host"),
        /// One client steers the radio while no session of the host reads it.
        Remote = ("remote", "Remote control"),
        /// Each client gets its own frequency and rate inside the capture of the radio.
        Slice = ("slice", "A slice for each client"),
    }
}

sdrkit_core::choice_enum! {
    /// The sample format that a share carries. A client gets the format of its protocol, converted from this one.
    pub enum ShareFormat {
        /// The format of the radio where the share carries it (8-bit and 16-bit samples go out as the radio gave them),
        /// else 32-bit floats.
        Native = ("native", "As the radio gives"),
        /// Unsigned 8-bit I/Q.
        Cu8 = ("cu8", "8-bit unsigned"),
        /// Signed 8-bit I/Q.
        Cs8 = ("cs8", "8-bit signed"),
        /// Signed 16-bit I/Q.
        Cs16 = ("cs16", "16-bit"),
        /// 32-bit float I/Q.
        Cf32 = ("cf32", "32-bit float"),
    }
}

sdrkit_core::choice_enum! {
    /// A protocol that a share serves.
    pub enum ShareProtocol {
        /// rtl_tcp: unsigned 8-bit I/Q over one TCP connection.
        RtlTcp = ("rtl_tcp", "rtl_tcp"),
        /// SoapyRemote: calls over TCP, streams over UDP, receive and transmit.
        SoapyRemote = ("soapyremote", "SoapyRemote"),
    }
}

sdrkit_core::choice_enum! {
    /// Why a client of a share left.
    pub enum LeftReason {
        /// The client hung up or its connection failed.
        Closed = ("closed", "Closed"),
        /// The client lost samples for longer than `drop_after_s`.
        TooSlow = ("too_slow", "Too slow"),
        /// The host disconnected it.
        Kicked = ("kicked", "Kicked"),
        /// The share closed, or its settings or the rate of the radio changed.
        Shutdown = ("shutdown", "Share stopped"),
    }
}

sdrkit_core::choice_enum! {
    /// Why a share refused a connection.
    pub enum RefusedReason {
        /// The address is outside the setting `allow`.
        NotAllowed = ("not_allowed", "Address not allowed"),
        /// `max_clients` clients are connected.
        Full = ("full", "Too many clients"),
    }
}

/// The control of a new share: the first client steers while no session of the host reads the radio, and follows
/// otherwise.
const DEFAULT_CONTROL: ShareControl = ShareControl::Remote;
/// Who steers the radio.
pub const CONTROL: Setting<field::Choice<ShareControl>> =
    Setting::choice("control", "Control", DEFAULT_CONTROL).help(
        "Follow: the host alone tunes; a request of a client to tune, to set the rate or the gain is not carried out, and \
         the client is told the value in force. Remote: \
         the client that the command `grant` names, or else the client that connected first, steers the radio while no \
         session of the host reads it. Slice: each client gets its own frequency and rate inside the capture.",
    );
/// The most clients at one time.
pub const MAX_CLIENTS: Setting<field::I64> = Setting::new("max_clients", "Clients at most", "", 8)
    .range(1.0, 64.0, 1.0)
    .help("The most clients at one time, over every listener. A change disconnects the clients.");
/// The signal that the queue of a client holds.
pub const CLIENT_QUEUE_MS: Setting<field::I64> =
    Setting::new("client_queue_ms", "Client queue", "ms", 500)
        .range(50.0, 5000.0, 1.0)
        .help("The time of signal that waits for a client that is slow. What does not fit is dropped for that client only. A change disconnects the clients.");
/// How long a client may lose samples.
pub const DROP_AFTER_S: Setting<field::F64> =
    Setting::new("drop_after_s", "Drop a slow client after", "s", 5.0)
        .range(0.5, 120.0, 0.5)
        .help("A client that loses samples for this long without a break is disconnected. A change disconnects the clients.");
/// The address ranges that may connect.
pub const ALLOW: Setting<field::Text> = Setting::new("allow", "Allowed addresses", "", DEFAULT_ALLOW)
    .help(
        "The address ranges that may connect, separated by commas: 192.168.1.0/24, fd00::/8, one address. Empty allows \
         every address. A change disconnects the clients.",
    );
/// A steering client may set the gain.
pub const REMOTE_GAIN: Setting<field::Bool> =
    Setting::new("remote_gain", "Remote gain", "", true).help(
        "With the control Remote, the client that steers may also set the gain and the automatic gain of the radio.",
    );
/// The sample format that the share carries.
pub const FORMAT: Setting<field::Choice<ShareFormat>> =
    Setting::choice("format", "Sample format", ShareFormat::Native).help(
        "The format in which the samples wait for the clients. A change disconnects the clients.",
    );

sdrkit_core::settings! {
    /// The settings of a share.
    pub enum ShareSetting {
        /// [`CONTROL`].
        Control = CONTROL,
        /// [`MAX_CLIENTS`].
        MaxClients = MAX_CLIENTS,
        /// [`CLIENT_QUEUE_MS`].
        ClientQueueMs = CLIENT_QUEUE_MS,
        /// [`DROP_AFTER_S`].
        DropAfterS = DROP_AFTER_S,
        /// [`ALLOW`].
        Allow = ALLOW,
        /// [`REMOTE_GAIN`].
        RemoteGain = REMOTE_GAIN,
        /// [`FORMAT`].
        Format = FORMAT,
    }
}

/// The state and the counts of a share.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ShareFields<'a> {
    /// The radio that the share serves.
    #[field(label = "Radio")]
    pub radio: &'a str,
    /// The session of the share on the radio.
    #[field(label = "Session")]
    pub session: &'a str,
    /// The sample format that the share carries: `cu8`, `cs8`, `cs16` or `cf32`.
    #[field(label = "Format")]
    pub format: &'a str,
    /// The sample rate of the capture.
    #[field(label = "Sample rate", unit = "Hz")]
    pub rate_hz: Hertz,
    /// The centre frequency of the capture.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The listeners that run.
    #[field(label = "Listeners")]
    pub listeners: i64,
    /// The clients that are connected.
    #[field(label = "Clients")]
    pub clients: i64,
    /// The blocks of samples that the share queued for its clients.
    #[field(label = "Blocks")]
    pub blocks: i64,
    /// The samples that the share queued for its clients.
    #[field(label = "Samples")]
    pub samples: i64,
    /// The blocks that found no free buffer and reached no client.
    #[field(label = "Blocks without a buffer")]
    pub pool_misses: i64,
    /// A session of the host reads the radio: with the control Remote, no client steers.
    #[field(label = "Host session on the radio")]
    pub host_reads: bool,
    /// The requests of clients that changed the radio or a slice.
    #[field(label = "Requests done")]
    pub requests_done: i64,
    /// The requests of clients that the share refused.
    #[field(label = "Requests refused")]
    pub requests_refused: i64,
    /// A client transmits through the share now.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The requests to transmit that the share took.
    #[field(label = "Transmit requests taken")]
    pub tx_taken: i64,
    /// The requests to transmit that the share refused.
    #[field(label = "Transmit requests refused")]
    pub tx_refused: i64,
    /// The samples of clients that went to the transmit side of the session.
    #[field(label = "Transmit samples")]
    pub tx_samples: i64,
    /// Why the share does not serve: the hub or a listener could not be made.
    #[field(label = "Problem")]
    pub problem: Option<&'a str>,
}

/// A row of the table `listeners`. The row key is the number of the listener, which the command `listen` returns.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ListenerRow<'a> {
    /// The protocol.
    #[field(label = "Protocol")]
    pub protocol: ShareProtocol,
    /// The local address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The port, as the system gave it for port 0.
    #[field(label = "Port")]
    pub port: i64,
    /// The clients of the listener now.
    #[field(label = "Clients")]
    pub clients: i64,
    /// The connections that the listener took.
    #[field(label = "Connections taken")]
    pub accepted: i64,
    /// The connections refused because the address is outside the setting `allow`.
    #[field(label = "Refused: address")]
    pub refused_allow: i64,
    /// The connections refused because `max_clients` clients were connected.
    #[field(label = "Refused: full")]
    pub refused_full: i64,
    /// The bytes sent to the clients of the listener, past and present.
    #[field(label = "Sent", unit = "B")]
    pub bytes: i64,
}

/// A row of the table `clients`. The row key is the number of the client.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ClientRow<'a> {
    /// The listener that took the client.
    #[field(label = "Listener", refers_to = "listeners")]
    pub listener: &'a str,
    /// The protocol.
    #[field(label = "Protocol")]
    pub protocol: ShareProtocol,
    /// The address of the client.
    #[field(label = "Address")]
    pub address: Option<&'a str>,
    /// The sample format of its stream. No value while it has no stream.
    #[field(label = "Format")]
    pub format: Option<&'a str>,
    /// When it connected.
    #[field(label = "Connected")]
    pub since: Time,
    /// The bytes sent to it.
    #[field(label = "Sent", unit = "B")]
    pub bytes: i64,
    /// The samples sent to it.
    #[field(label = "Samples")]
    pub samples: i64,
    /// The samples dropped for this client because it was slow.
    #[field(label = "Dropped samples")]
    pub dropped_samples: i64,
    /// The times that it began to lose samples.
    #[field(label = "Loss runs")]
    pub drop_events: i64,
    /// The breaks in its stream: its own losses, the losses of the radio and retunes.
    #[field(label = "Breaks")]
    pub gaps: i64,
    /// The blocks that wait for it.
    #[field(label = "Queued")]
    pub queue_fill: i64,
    /// The blocks that its queue holds.
    #[field(label = "Queue size")]
    pub queue_capacity: i64,
    /// The requests it made.
    #[field(label = "Requests")]
    pub requests: i64,
    /// The requests that the share refused outright. A request that was answered with the value in force is counted
    /// in the field `requests_refused` of the share.
    #[field(label = "Requests refused")]
    pub refused_requests: i64,
    /// The centre of its slice, with the control Slice.
    #[field(label = "Slice centre", unit = "Hz")]
    pub slice_centre_hz: Option<Hertz>,
    /// The rate of its slice, with the control Slice.
    #[field(label = "Slice rate", unit = "Hz")]
    pub slice_rate_hz: Option<Hertz>,
    /// With the control Remote, this client steers the radio while no session of the host reads it.
    #[field(label = "Steers")]
    pub steers: bool,
    /// The command `grant` let this client transmit.
    #[field(label = "May transmit")]
    pub may_transmit: bool,
}

/// The listeners of a share.
pub static LISTENERS: Table<ListenerRow<'static>> = Table::new("listeners", "Listeners", 64);
/// The clients of a share.
pub static CLIENTS: Table<ClientRow<'static>> = Table::new("clients", "Clients", 256);

/// The fields of the event `share.client_joined`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ClientJoinedEvent<'a> {
    /// The number of the client: its row key in the table `clients`.
    #[field(label = "Client")]
    pub client: i64,
    /// The number of the listener that took it.
    #[field(label = "Listener")]
    pub listener: i64,
    /// The address of the client.
    #[field(label = "Address")]
    pub address: Option<&'a str>,
}

/// The fields of the event `share.client_left`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ClientLeftEvent<'a> {
    /// The number of the client.
    #[field(label = "Client")]
    pub client: i64,
    /// The number of the listener that took it.
    #[field(label = "Listener")]
    pub listener: i64,
    /// The address of the client.
    #[field(label = "Address")]
    pub address: Option<&'a str>,
    /// Why it left.
    #[field(label = "Reason")]
    pub reason: LeftReason,
}

/// The fields of the event `share.refused`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RefusedEvent<'a> {
    /// The number of the listener.
    #[field(label = "Listener")]
    pub listener: i64,
    /// The address that connected.
    #[field(label = "Address")]
    pub address: Option<&'a str>,
    /// Why it was refused.
    #[field(label = "Reason")]
    pub reason: RefusedReason,
}

/// The fields of the event `share.listen_failed`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ListenFailedEvent<'a> {
    /// The protocol.
    #[field(label = "Protocol")]
    pub protocol: ShareProtocol,
    /// The local address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The port.
    #[field(label = "Port")]
    pub port: i64,
    /// What the network said.
    #[field(label = "Reason")]
    pub reason: &'a str,
}

/// A listener took a client.
pub static CLIENT_JOINED: EventKind<ClientJoinedEvent<'static>> =
    EventKind::new("share.client_joined", "Client joined");
/// A client left.
pub static CLIENT_LEFT: EventKind<ClientLeftEvent<'static>> =
    EventKind::new("share.client_left", "Client left");
/// A listener refused a connection.
pub static REFUSED: EventKind<RefusedEvent<'static>> =
    EventKind::new("share.refused", "Connection refused");
/// A listener could not start again after the share made its queues again.
pub static LISTEN_FAILED: EventKind<ListenFailedEvent<'static>> =
    EventKind::new("share.listen_failed", "Listener failed");

/// The arguments of `listen`.
#[derive(Arguments)]
pub struct ListenArgs<'a> {
    /// The protocol.
    #[arg(label = "Protocol", help = "rtl_tcp or SoapyRemote.")]
    pub protocol: ShareProtocol,
    /// The port.
    #[arg(
        label = "Port",
        help = "The TCP port. 0 lets the system choose; the reply states the port. The usual ports are 1234 for rtl_tcp and 55132 for SoapyRemote."
    )]
    pub port: i64,
    /// The local address.
    #[arg(
        label = "Address",
        help = "The local address to listen on. The default is every interface; 127.0.0.1 takes clients of this machine only.",
        default = "0.0.0.0"
    )]
    pub host: Option<&'a str>,
}

/// The reply of `listen`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ListenReply {
    /// The number of the listener: its row key in the table `listeners`.
    #[field(label = "Listener")]
    pub listener: i64,
    /// The port that the listener has.
    #[field(label = "Port")]
    pub port: i64,
}

/// The arguments of `unlisten`.
#[derive(Arguments)]
pub struct UnlistenArgs {
    /// The number of the listener.
    #[arg(label = "Listener", help = "The number that `listen` returned.")]
    pub listener: i64,
}

/// The arguments of `kick`.
#[derive(Arguments)]
pub struct KickArgs {
    /// The number of the client.
    #[arg(
        label = "Client",
        help = "The row key of the client in the table `clients`."
    )]
    pub client: i64,
}

/// The arguments of `grant`.
#[derive(Arguments)]
pub struct GrantArgs {
    /// The number of the client.
    #[arg(
        label = "Client",
        help = "The row key of the client in the table `clients`."
    )]
    pub client: i64,
    /// The client steers the radio.
    #[arg(
        label = "Steer",
        help = "With the control Remote, this client and no other steers the radio. Off gives the choice back to the client that connected first.",
        default = false
    )]
    pub steer: Option<bool>,
    /// The client may transmit.
    #[arg(
        label = "Transmit",
        help = "The client may transmit through the radio while the engine setting `transmit_allowed` is on. Off takes the grant back.",
        default = false
    )]
    pub transmit: Option<bool>,
}

/// Starts a listener.
pub const LISTEN: CommandKind<ListenArgs<'static>, ListenReply> = CommandKind::new(
    "listen", "Listen",
)
.help("Starts a listener of one protocol on the share. A share can have several listeners.");
/// Stops a listener.
pub const UNLISTEN: CommandKind<UnlistenArgs> = CommandKind::new("unlisten", "Stop listening")
    .help("Stops a listener. Its clients stay connected.");
/// Disconnects a client.
pub const KICK: CommandKind<KickArgs> =
    CommandKind::new("kick", "Disconnect").help("Disconnects one client.");
/// Gives or takes the rights of a client.
pub const GRANT: CommandKind<GrantArgs> = CommandKind::new("grant", "Grant").help(
    "Sets what one client may do beyond listening: steer the radio, transmit. No client may transmit until this command \
     says so.",
);

sdrkit_core::commands! {
    /// The commands of a share.
    pub enum ShareCommand {
        /// [`LISTEN`].
        Listen = LISTEN.command,
        /// [`UNLISTEN`].
        Unlisten = UNLISTEN.command,
        /// [`KICK`].
        Kick = KICK.command,
        /// [`GRANT`].
        Grant = GRANT.command,
    }
}

/// The arguments of the engine command `share.open`.
#[derive(Arguments)]
pub struct ShareOpenArgs<'a> {
    /// The radio.
    #[arg(
        label = "Radio",
        help = "The row key of the radio in the table `radios`.",
        refers_to = "radios"
    )]
    pub radio: &'a str,
}

/// The reply of the engine command `share.open`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ShareOpenReply<'a> {
    /// The key of the share (`share.h1`).
    #[field(label = "Share")]
    pub share: &'a str,
}

/// The arguments of the engine command `share.close`.
#[derive(Arguments)]
pub struct ShareCloseArgs<'a> {
    /// The share.
    #[arg(
        label = "Share",
        help = "The key of the share, as `share.open` returned it."
    )]
    pub share: &'a str,
}

/// Opens the share of a radio.
pub const SHARE_OPEN: CommandKind<ShareOpenArgs<'static>, ShareOpenReply<'static>> =
    CommandKind::new("share.open", "Share a radio").help(
        "Opens the share of a radio: an owner `share.h<n>` that serves the samples of the radio to network clients. A \
         radio has one share. The share serves nobody until its command `listen` starts a listener.",
    );
/// Closes a share.
pub const SHARE_CLOSE: CommandKind<ShareCloseArgs<'static>> =
    CommandKind::new("share.close", "Stop sharing")
        .help("Closes a share: its listeners stop and its clients are disconnected.");

/// A row of the engine table `shares`. The row key is the key of the share.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ShareRow<'a> {
    /// The radio that the share serves.
    #[field(label = "Radio", refers_to = "radios")]
    pub radio: &'a str,
    /// The session of the share on the radio.
    #[field(label = "Session", refers_to = "sessions")]
    pub session: &'a str,
}

/// The open shares.
pub static SHARES: Table<ShareRow<'static>> = Table::new("shares", "Shares", 256);

/// The key of the share `id` as an owner and in the table `shares`.
pub(crate) fn share_key(id: ShareId) -> String {
    format!("share.h{}", id.to_u64())
}

/// The share of a key that [`share_key`] made.
pub(crate) fn parse_share_key(key: &str) -> Option<ShareId> {
    key.strip_prefix("share.h")?
        .parse()
        .ok()
        .map(ShareId::from_u64)
}

/// The text key of a sample type that a share carries.
fn format_key(t: SampleType) -> &'static str {
    match t {
        SampleType::Cu8 => "cu8",
        SampleType::Cs8 => "cs8",
        SampleType::Cs16 => "cs16",
        _ => "cf32",
    }
}

/// The address ranges of the setting `allow`.
fn parse_allow(text: &str) -> Result<Vec<Cidr>, usize> {
    let mut out = Vec::new();
    let mut at = 0;
    for part in text.split(',') {
        let t = part.trim();
        if !t.is_empty() {
            out.push(t.parse::<Cidr>().map_err(|_| at)?);
        }
        at += part.len() + 1;
    }
    Ok(out)
}

fn int(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn unix(t: SystemTime) -> f64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// What the radio of a share can do, as it said when the share opened.
struct Limits {
    min_hz: f64,
    max_hz: f64,
    min_rate_hz: f64,
    max_rate_hz: f64,
    gain_db: Option<(f64, f64)>,
    bandwidth: bool,
    agc: bool,
    transmit: bool,
}

impl Limits {
    fn of(caps: &Capabilities) -> Limits {
        Limits {
            min_hz: caps.min_freq_hz,
            max_hz: caps.max_freq_hz,
            min_rate_hz: caps.rates.min_hz,
            max_rate_hz: caps.rates.max_hz,
            gain_db: caps.gain_db.map(|(lo, hi, _)| (lo, hi)),
            bandwidth: caps.bandwidth_hz.is_some(),
            agc: caps.has_agc,
            transmit: caps.transmit.is_some(),
        }
    }
}

/// What the sink of the share session, the policy on the reader threads of the clients and the control thread share.
/// None of them waits for another here: the values are atomics, and the two short lists have locks that no engine
/// lock is taken under.
pub(crate) struct ShareState {
    /// A new feeder for the sink, with the format it takes.
    feeder: Mutex<Option<(Feeder, SampleType)>>,
    feeder_new: AtomicBool,
    control: AtomicU8,
    remote_gain: AtomicBool,
    /// A session of the host reads the radio.
    host_reads: AtomicBool,
    /// The host set the rate of the radio: no client changes it.
    rate_fixed: AtomicBool,
    /// The client that `grant` lets steer, plus 1. 0 for none.
    steerer: AtomicU64,
    /// The client that connected first of those connected, plus 1. 0 for none.
    first: AtomicU64,
    limits: Limits,
    /// The centre and the rate of the capture, the gain, the automatic gain and the bandwidth of the radio, as of the
    /// last tick: what a client is told when it asks for a value that the share does not set.
    centre_bits: AtomicU64,
    rate_bits: AtomicU64,
    gain_bits: AtomicU64,
    agc: AtomicBool,
    bandwidth_bits: AtomicU64,
    transmit_allowed: Arc<AtomicBool>,
    /// The clients that `grant` lets transmit.
    tx_grants: Mutex<Vec<u64>>,
    tx_busy: Arc<AtomicBool>,
    streams: Arc<Streams>,
    /// The changes of the radio that clients asked for and the control thread carries out.
    pending: Mutex<Vec<Request>>,
    done: AtomicU64,
    refused: AtomicU64,
    tx_taken: AtomicU64,
    tx_refused: AtomicU64,
    tx_samples: Arc<AtomicU64>,
    engine: Weak<Shared>,
}

impl ShareState {
    fn control(&self) -> ShareControl {
        ShareControl::from_index(u16::from(self.control.load(Ordering::Relaxed)))
            .unwrap_or(ShareControl::Follow)
    }

    /// True if `client` steers the radio now: the control is Remote, no session of the host reads the radio, and the
    /// client is the one that `grant` named, or the first one if `grant` named none.
    fn steers(&self, client: u64) -> bool {
        if self.control() != ShareControl::Remote || self.host_reads.load(Ordering::Relaxed) {
            return false;
        }
        let me = client + 1;
        match self.steerer.load(Ordering::Relaxed) {
            0 => {
                // The first request of the first client can come before the tick that notes the client.
                let _ = self
                    .first
                    .compare_exchange(0, me, Ordering::Relaxed, Ordering::Relaxed);
                self.first.load(Ordering::Relaxed) == me
            }
            granted => granted == me,
        }
    }

    fn may_transmit(&self, client: u64) -> bool {
        lock(&self.tx_grants).contains(&client)
    }

    /// Answers a request of a client that steers. What the radio can take is queued for the control thread. A rate
    /// below what the radio can be set to, or below a rate that the host set, is the client's own: its writer brings
    /// the capture down to it around the centre of the radio.
    fn remote(&self, client: &ClientHandle, request: &Request) -> Answer {
        let l = &self.limits;
        if let Request::Rate { hz } = *request {
            if let Some(a) = self.own_rate(client, hz) {
                return a;
            }
        }
        let within =
            |v: f64, lo: f64, hi: f64| v.is_finite() && (hi <= lo || (lo..=hi).contains(&v));
        let gain = self.remote_gain.load(Ordering::Relaxed);
        let ok = match *request {
            Request::Frequency { hz } => l.max_hz > l.min_hz && within(hz, l.min_hz, l.max_hz),
            Request::Rate { hz } => {
                !self.rate_fixed.load(Ordering::Relaxed)
                    && hz > 0.0
                    && within(hz, l.min_rate_hz, l.max_rate_hz)
            }
            Request::Bandwidth { hz } => l.bandwidth && hz.is_finite() && hz >= 0.0,
            Request::Gain { db } => gain && l.gain_db.is_some_and(|(lo, hi)| within(db, lo, hi)),
            Request::Agc { .. } => gain && l.agc,
            Request::BiasTee { .. } | Request::Correction { .. } | Request::Other { .. } => false,
        };
        // A value that the share does not let the client set (the host fixed the rate, the gain stays with the host, the
        // radio has no automatic gain) is answered with the value in force, as in Follow. A value that the radio
        // cannot take is refused.
        let held = match *request {
            Request::Rate { .. } => self.rate_fixed.load(Ordering::Relaxed),
            Request::Gain { .. } => !gain,
            Request::Agc { .. } => !gain || !l.agc,
            _ => false,
        };
        match (ok, held) {
            (false, true) => return self.in_force(request),
            (false, false) => return Answer::Refused,
            (true, _) => {}
        }
        lock(&self.pending).push(*request);
        if let Some(sh) = self.engine.upgrade() {
            sh.wake.notify_all();
        }
        Answer::Accepted
    }

    /// The answer to a rate request `hz` of a steering client that the radio does not carry out: the radio goes to its
    /// lowest rate (unless the host set the rate) and the client gets `hz` from it. `None` for a rate that the radio
    /// takes; the client then gets the capture rate again.
    fn own_rate(&self, client: &ClientHandle, hz: f64) -> Option<Answer> {
        let l = &self.limits;
        let fixed = self.rate_fixed.load(Ordering::Relaxed);
        let below = !fixed && hz < l.min_rate_hz && l.min_rate_hz > 0.0;
        let under_fixed = fixed && hz < client.capture().rate_hz;
        if !(hz.is_finite() && hz >= OWN_RATE_FLOOR_HZ && (below || under_fixed)) {
            let _ = client.set_rate(None);
            return None;
        }
        if below {
            lock(&self.pending).push(Request::Rate { hz: l.min_rate_hz });
            if let Some(sh) = self.engine.upgrade() {
                sh.wake.notify_all();
            }
        }
        // The capture is at least the lowest rate of the radio, which is above `hz`.
        Some(match client.set_rate(Some(hz)) {
            Ok(()) => Answer::Accepted,
            Err(_) => Answer::Refused,
        })
    }

    /// Answers a request of a client for its own slice of the capture. A request for another setting of the radio gets
    /// the value in force.
    fn slice(&self, client: &ClientHandle, request: &Request) -> Answer {
        let now = client.slice().unwrap_or_else(|| client.capture());
        let wanted = match *request {
            Request::Frequency { hz } => Slice {
                centre_hz: hz,
                ..now
            },
            Request::Rate { hz } => Slice { rate_hz: hz, ..now },
            _ => return self.in_force(request),
        };
        match client.set_slice(Some(wanted)) {
            Ok(()) => Answer::Accepted,
            Err(_) => Answer::Refused,
        }
    }

    /// Answers a request that the share does not carry out: the radio stays as it is, and the client is told the value
    /// in force, so that a client that sets the radio when it connects stays connected and shows what it gets.
    /// A radio without automatic gain has manual gain in force. A request for another setting that the radio does not
    /// have is refused.
    fn in_force(&self, request: &Request) -> Answer {
        let bits = |a: &AtomicU64| f64::from_bits(a.load(Ordering::Relaxed));
        let l = &self.limits;
        match *request {
            Request::Frequency { .. } => Answer::Adjusted(Request::Frequency {
                hz: bits(&self.centre_bits),
            }),
            Request::Rate { .. } if bits(&self.rate_bits) > 0.0 => {
                Answer::Adjusted(Request::Rate {
                    hz: bits(&self.rate_bits),
                })
            }
            Request::Gain { .. } if l.gain_db.is_some() => Answer::Adjusted(Request::Gain {
                db: bits(&self.gain_bits),
            }),
            Request::Agc { .. } => Answer::Adjusted(Request::Agc {
                on: l.agc && self.agc.load(Ordering::Relaxed),
            }),
            Request::Bandwidth { .. } if l.bandwidth => Answer::Adjusted(Request::Bandwidth {
                hz: bits(&self.bandwidth_bits),
            }),
            _ => Answer::Refused,
        }
    }
}

/// The policy of a share: the answers to the requests of its clients.
struct Policy(Arc<ShareState>);

impl Control for Policy {
    fn request(&self, client: &ClientHandle, request: &Request) -> Answer {
        let s = &self.0;
        // A radio of a share has one antenna on each side: the choice of it changes nothing.
        if matches!(
            request,
            Request::Other {
                name: "antenna",
                args: [0, _]
            }
        ) {
            return Answer::Accepted;
        }
        let answer = match s.control() {
            ShareControl::Slice => s.slice(client, request),
            ShareControl::Remote if s.steers(client.id()) => s.remote(client, request),
            ShareControl::Follow | ShareControl::Remote => s.in_force(request),
        };
        let count = if answer == Answer::Accepted {
            &s.done
        } else {
            &s.refused
        };
        count.fetch_add(1, Ordering::Relaxed);
        answer
    }

    fn in_force(&self, _client: &ClientHandle, request: &Request) -> Option<Request> {
        match self.0.in_force(request) {
            Answer::Adjusted(r) => Some(r),
            _ => None,
        }
    }

    /// Takes the transmit ring of a client that `grant` lets transmit, while the engine setting `transmit_allowed` is
    /// on, the radio can transmit and no other client transmits. The samples go to the transmit side of the share
    /// session, which reads the same switch for each burst.
    fn transmit(&self, client: &ClientHandle, request: &TxRequest, reader: TxReader) -> Answer {
        let s = &self.0;
        let may = s.limits.transmit
            && s.may_transmit(client.id())
            && s.transmit_allowed.load(Ordering::Acquire);
        if !may || s.tx_busy.swap(true, Ordering::AcqRel) {
            s.tx_refused.fetch_add(1, Ordering::Relaxed);
            return Answer::Refused;
        }
        let centre_hz = if request.centre_hz > 0.0 {
            request.centre_hz
        } else {
            f64::from_bits(s.centre_bits.load(Ordering::Relaxed))
        };
        let (streams, busy, samples) = (
            Arc::clone(&s.streams),
            Arc::clone(&s.tx_busy),
            Arc::clone(&s.tx_samples),
        );
        let started = std::thread::Builder::new()
            .name("sdrkit-share-tx".into())
            .spawn(move || {
                send(reader, &streams, centre_hz, &samples);
                busy.store(false, Ordering::Release);
            });
        if started.is_err() {
            s.tx_busy.store(false, Ordering::Release);
            s.tx_refused.fetch_add(1, Ordering::Relaxed);
            return Answer::Refused;
        }
        s.tx_taken.fetch_add(1, Ordering::Relaxed);
        Answer::Accepted
    }
}

/// The samples that the transmit thread of a share moves at one time.
const TX_CHUNK: usize = 4096;

/// Moves the samples of a client from its transmit ring into the transmit stream of the share session, until the
/// client's side of the ring is gone. The end of a burst of the client ends the burst of the session.
fn send(mut reader: TxReader, streams: &Streams, centre_hz: f64, count: &AtomicU64) {
    let req = *reader.request();
    let ss = size(req.format);
    let mut bytes = vec![0u8; TX_CHUNK * ss];
    let mut iq = vec![Complex32::default(); TX_CHUNK];
    let mut open = false;
    let write = |samples: &[Complex32], end: bool| {
        streams.write(
            HostPort::Transmit,
            HostData::Iq {
                samples,
                rate_hz: req.rate_hz,
                centre_hz,
                end,
                start: None,
            },
        );
    };
    loop {
        if reader.available() > 0 {
            let n = reader.read(&mut bytes);
            let k = to_iq(req.format, &bytes[..n], &mut iq);
            let end = reader.ended();
            write(&iq[..k], end);
            count.fetch_add(k as u64, Ordering::Relaxed);
            open = !end;
            continue;
        }
        let gone = reader.closed();
        // The client can write its last samples between the look above and this one.
        if gone && reader.available() > 0 {
            continue;
        }
        if open && (reader.ended() || gone) {
            write(&[], true);
            open = false;
        }
        if gone {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// The receiver of the share session: it writes each block in the format of the share and queues it in the hub. It
/// never waits and, once it runs, does not allocate.
struct ShareSink {
    state: Arc<ShareState>,
    feeder: Option<(Feeder, SampleType)>,
    rate_hz: f64,
    centre_hz: f64,
    /// The samples lost since the last block.
    lost: u64,
    bytes: Vec<u8>,
}

impl ShareSink {
    fn tuned(&mut self, input: &InputInfo) {
        self.rate_hz = input.rate_hz;
        self.centre_hz = input.centre_hz;
        self.state
            .centre_bits
            .store(input.centre_hz.to_bits(), Ordering::Relaxed);
    }
}

impl Owner for ShareSink {}

impl Receiver for ShareSink {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        self.tuned(input);
        Ok(())
    }

    fn retuned(&mut self, input: &InputInfo) {
        self.tuned(input);
    }

    fn feed(&mut self, iq: &[Complex32], _out: &mut Output) {
        if self.state.feeder_new.load(Ordering::Acquire) {
            // The control thread holds this lock only to put a feeder in.
            if let Ok(mut new) = self.state.feeder.try_lock() {
                if let Some(f) = new.take() {
                    self.feeder = Some(f);
                }
                self.state.feeder_new.store(false, Ordering::Release);
            }
        }
        let Some((feeder, format)) = self.feeder.as_mut() else {
            return;
        };
        self.bytes.clear();
        from_iq(*format, iq, &mut self.bytes);
        feeder.push(
            &self.bytes,
            &Block {
                format: *format,
                centre_hz: self.centre_hz,
                rate_hz: self.rate_hz,
                lost: std::mem::take(&mut self.lost),
            },
        );
    }

    fn discontinuity(&mut self, lost: u64) {
        self.lost += lost;
    }
}

/// A listener that the share keeps, also when it makes its hub again.
struct KeptListener {
    number: u64,
    protocol: ShareProtocol,
    host: String,
    port: u16,
    /// Its id in the hub that runs.
    hub_id: u64,
}

/// A share as an owner: its settings, its results and its commands, and the hub with its listeners.
pub(crate) struct ShareOwner {
    values: Values,
    results: Published,
    commands: Commands,
    state: Arc<ShareState>,
    net: Arc<dyn Transport>,
    clock: Clock,
    catalog: Arc<EventCatalog>,
    events: Arc<Mutex<VecDeque<Event>>>,
    hub: Option<Hub>,
    /// The format and the rate that the hub was made for.
    made: (SampleType, f64),
    native: SampleType,
    rate_hz: f64,
    centre_hz: f64,
    allow: Vec<Cidr>,
    listeners: Vec<KeptListener>,
    next_listener: u64,
    device: Device,
    uuid: String,
    ssdp: Option<Ssdp>,
    radio: String,
    session: String,
    problem: Option<String>,
    /// The row keys of the clients in the table.
    shown_clients: Vec<u64>,
    /// The refusals of each client that the status line names: the client, when, and the lines.
    refusals: Vec<(u64, f64, Vec<String>)>,
    /// The tuner type that the rtl_tcp greeting states.
    rtl_tuner: u32,
    /// The counters of each client at the start of its loss window.
    loss_seen: std::collections::HashMap<u64, LossSeen>,
}

/// The counters of one client at the start of a loss window, and the part it lost over the window before.
struct LossSeen {
    at: f64,
    sent: u64,
    dropped: u64,
    part: f64,
}

/// The window over which the status line counts the samples that a client lost, s.
const LOSS_WINDOW_S: f64 = 3.0;
/// How long the status line names the refusals of a client that left, s.
const REFUSAL_KEEP_S: f64 = 30.0;
/// The part of its samples that a client loses before the status line names it.
const LOSS_NOTE: f64 = 0.01;

/// A request as the status line names it: "106.5 MHz", "a rate of 2.4 Msps".
fn asked_text(r: &Request) -> String {
    match *r {
        Request::Frequency { hz } => format!("{} MHz", sdrkit_driver_core::fmt_m(hz)),
        Request::Rate { hz } => format!("a rate of {} Msps", sdrkit_driver_core::fmt_m(hz)),
        Request::Bandwidth { hz } => {
            format!("a bandwidth of {} MHz", sdrkit_driver_core::fmt_m(hz))
        }
        Request::Gain { db } => format!("a gain of {db} dB"),
        Request::Agc { on } => format!("automatic gain {}", if on { "on" } else { "off" }),
        Request::BiasTee { on } => format!("the bias tee {}", if on { "on" } else { "off" }),
        Request::Correction { ppm } => format!("a correction of {ppm} ppm"),
        Request::Other { name, .. } => format!("the setting {name}"),
    }
}

/// Why a request was not carried out as asked, and what the client got.
fn why_not(control: ShareControl, steers: bool, host_reads: bool, answer: &Answer) -> String {
    let why = match control {
        ShareControl::Follow => "this share follows the server's own tuning",
        ShareControl::Remote if host_reads => {
            "a session of the server reads the radio and steers it"
        }
        ShareControl::Remote if !steers => "another client steers the radio",
        ShareControl::Remote => "the radio or the server does not take this value",
        ShareControl::Slice => "it does not fit inside the capture",
    };
    match answer {
        Answer::Adjusted(got) => format!("not carried out, {why}; it gets {}", asked_text(got)),
        _ => format!("not carried out, {why}"),
    }
}

impl ShareOwner {
    fn keep(&self, mut e: Event) {
        if self.catalog.stamp(&mut e).is_none() {
            return;
        }
        let mut kept = lock(&self.events);
        if kept.len() == EVENT_CAP {
            kept.pop_front();
        }
        kept.push_back(e);
    }

    /// The format that the hub carries: the setting, or for `native` the format of the radio where a hub carries it.
    fn wanted_format(&self) -> SampleType {
        match self.values.get(&FORMAT) {
            ShareFormat::Cu8 => SampleType::Cu8,
            ShareFormat::Cs8 => SampleType::Cs8,
            ShareFormat::Cs16 => SampleType::Cs16,
            ShareFormat::Cf32 => SampleType::Cf32,
            ShareFormat::Native => match self.native {
                t @ (SampleType::Cu8 | SampleType::Cs8 | SampleType::Cs16) => t,
                _ => SampleType::Cf32,
            },
        }
    }

    /// The protocol `p` of a listener on `host`. The TCP streams of SoapyRemote listen on the same host, so that a
    /// listener on one address opens no stream port on the others; a listener on every address leaves it to the client.
    fn protocol(&self, p: ShareProtocol, host: &str) -> Arc<dyn Protocol> {
        match p {
            ShareProtocol::RtlTcp => Arc::new(sdrkit_serve_rtltcp::RtlTcp::rtl_sdr(self.rtl_tuner)),
            ShareProtocol::SoapyRemote => {
                let s = SoapyRemote::new(Arc::clone(&self.net), self.device.clone(), &self.uuid);
                let every = host.trim().is_empty()
                    || host
                        .trim_matches(['[', ']'])
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_unspecified());
                Arc::new(if every { s } else { s.with_stream_host(host) })
            }
        }
    }

    /// Starts listener `l` on the hub that runs, and the discovery answer for a SoapyRemote listener that other
    /// machines can reach. Returns the port.
    fn start_listener(&mut self, l: &mut KeptListener) -> Result<u16, ServeError> {
        let hub = self.hub.as_ref().ok_or(ServeError::Thread)?;
        let info = hub.listen(
            &self.net,
            &Listen {
                host: l.host.clone(),
                port: l.port,
                allow: self.allow.clone(),
            },
            self.protocol(l.protocol, &l.host),
            Arc::new(Policy(Arc::clone(&self.state))),
        )?;
        l.hub_id = info.id;
        l.port = info.addr.port();
        if l.protocol == ShareProtocol::SoapyRemote
            && self.ssdp.is_none()
            && !info.addr.ip().is_loopback()
        {
            // A search finds the server only if the port of the discovery is free; a client can still name the address.
            let name = self.device.info.get("name").map_or("", String::as_str);
            self.ssdp = Ssdp::start(&self.net, &self.uuid, &l.host, l.port, name).ok();
        }
        Ok(l.port)
    }

    /// Makes the hub for the format and the rate in force, hands its feeder to the sink of the session, and starts the
    /// kept listeners again on their ports. The clients of the hub before are disconnected.
    fn make_hub(&mut self) {
        self.ssdp = None;
        if let Some(mut old) = self.hub.take() {
            old.shutdown();
            // The clients of the hub before left: the host hears of it.
            self.keep_all(old.take_events(), self.clock.now());
        }
        lock(&self.state.tx_grants).clear();
        self.state.steerer.store(0, Ordering::Relaxed);
        self.state.first.store(0, Ordering::Relaxed);
        let format = self.wanted_format();
        // A radio that has not said its rate yet gets queues for a low rate; the hub is made again when it does.
        let rate_hz = if self.rate_hz > 0.0 {
            self.rate_hz
        } else {
            48e3
        };
        let config = Config {
            queue_ms: u32::try_from(self.values.get(&CLIENT_QUEUE_MS)).unwrap_or(500),
            drop_after_s: self.values.get(&DROP_AFTER_S),
            max_clients: usize::try_from(self.values.get(&MAX_CLIENTS)).unwrap_or(8),
            block_bytes: BLOCK * size(format),
            ..Config::default()
        };
        let first = Block {
            format,
            centre_hz: self.centre_hz,
            rate_hz,
            lost: 0,
        };
        match Hub::new(config, &first) {
            Ok((hub, feeder)) => {
                *lock(&self.state.feeder) = Some((feeder, format));
                self.state.feeder_new.store(true, Ordering::Release);
                self.hub = Some(hub);
                self.made = (format, self.rate_hz);
                self.problem = None;
            }
            Err(e) => {
                self.problem = Some(e.to_string());
                return;
            }
        }
        let mut kept = std::mem::take(&mut self.listeners);
        kept.retain_mut(|l| match self.start_listener(l) {
            Ok(_) => true,
            Err(e) => {
                let reason = e.to_string();
                self.keep(LISTEN_FAILED.event(
                    self.clock.now(),
                    0.0,
                    &ListenFailedEvent {
                        protocol: l.protocol,
                        address: &l.host,
                        port: i64::from(l.port),
                        reason: &reason,
                    },
                ));
                false
            }
        });
        self.listeners = kept;
    }

    /// The number that the share gave the listener with the id `hub_id` of the hub.
    fn listener_number(&self, hub_id: u64) -> u64 {
        self.listeners
            .iter()
            .find(|l| l.hub_id == hub_id)
            .map_or(0, |l| l.number)
    }

    /// Keeps the events `events` of the hub for the host, as events of the share at wall time `now`.
    fn keep_all(&mut self, events: Vec<sdrkit_serve_core::Event>, now: f64) {
        for e in events {
            let text = |p: Option<std::net::SocketAddr>| p.map(|a| a.to_string());
            let ev = match e {
                sdrkit_serve_core::Event::ClientJoined {
                    client,
                    listener,
                    peer,
                } => CLIENT_JOINED.event(
                    now,
                    0.0,
                    &ClientJoinedEvent {
                        client: int(client),
                        listener: int(self.listener_number(listener)),
                        address: text(peer).as_deref(),
                    },
                ),
                sdrkit_serve_core::Event::ClientLeft {
                    client,
                    listener,
                    peer,
                    reason,
                } => {
                    lock(&self.state.tx_grants).retain(|c| *c != client);
                    let _ = self.state.steerer.compare_exchange(
                        client + 1,
                        0,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    );
                    CLIENT_LEFT.event(
                        now,
                        0.0,
                        &ClientLeftEvent {
                            client: int(client),
                            listener: int(self.listener_number(listener)),
                            address: text(peer).as_deref(),
                            reason: match reason {
                                Leave::Closed => LeftReason::Closed,
                                Leave::TooSlow => LeftReason::TooSlow,
                                Leave::Kicked => LeftReason::Kicked,
                                Leave::Shutdown => LeftReason::Shutdown,
                            },
                        },
                    )
                }
                sdrkit_serve_core::Event::Refused {
                    listener,
                    peer,
                    reason,
                } => REFUSED.event(
                    now,
                    0.0,
                    &RefusedEvent {
                        listener: int(self.listener_number(listener)),
                        address: text(peer).as_deref(),
                        reason: match reason {
                            Refusal::NotAllowed => RefusedReason::NotAllowed,
                            Refusal::Full => RefusedReason::Full,
                        },
                    },
                ),
            };
            self.keep(ev);
        }
    }

    /// Brings the share up to date at a tick: takes the tuning of the radio and whether a session of the host reads
    /// it, makes the hub again when the rate changed and no client is connected, and copies the counters and the
    /// events of the hub into the results. `now` is the wall time.
    fn tick(&mut self, now: f64, tuning: (f64, f64), host_reads: bool, rate_fixed: bool) {
        (self.rate_hz, self.centre_hz) = tuning;
        self.state.host_reads.store(host_reads, Ordering::Relaxed);
        self.state.rate_fixed.store(rate_fixed, Ordering::Relaxed);
        let idle = self
            .hub
            .as_ref()
            .is_none_or(|h| h.snapshot().clients.iter().all(|c| c.closed));
        let stale = self.made != (self.wanted_format(), self.rate_hz);
        if self.rate_hz > 0.0 && stale && idle {
            self.make_hub();
        }
        self.publish(now);
    }

    fn publish(&mut self, now: f64) {
        let Some(hub) = self.hub.as_ref() else {
            self.write_fields(None);
            return;
        };
        let snap = hub.snapshot();
        let events = hub.take_events();
        self.keep_all(events, now);
        // The client that connected first steers when `grant` named none.
        let first = snap
            .clients
            .iter()
            .filter(|c| !c.closed && c.engaged)
            .map(|c| c.id)
            .min();
        self.state
            .first
            .store(first.map_or(0, |c| c + 1), Ordering::Relaxed);
        for l in &self.listeners {
            let Some(s) = snap.listeners.iter().find(|s| s.id == l.hub_id) else {
                continue;
            };
            LISTENERS.put(
                &mut self.results,
                &l.number.to_string(),
                &ListenerRow {
                    protocol: l.protocol,
                    address: &l.host,
                    port: i64::from(s.addr.port()),
                    clients: int(s.clients as u64),
                    accepted: int(s.accepted),
                    refused_allow: int(s.refused_allow),
                    refused_full: int(s.refused_full),
                    bytes: int(s.bytes),
                },
            );
        }
        let grants = lock(&self.state.tx_grants).clone();
        let mut shown = Vec::with_capacity(snap.clients.len());
        // A connection that only read the device information, or carries the log of a client, is no client.
        for c in snap.clients.iter().filter(|c| !c.closed && c.engaged) {
            let protocol = if c.protocol == "rtl_tcp" {
                ShareProtocol::RtlTcp
            } else {
                ShareProtocol::SoapyRemote
            };
            let peer = c.peer.map(|a| a.to_string());
            let listener = self.listener_number(c.listener).to_string();
            let steers = self.state.steers(c.id);
            CLIENTS.put(
                &mut self.results,
                &c.id.to_string(),
                &ClientRow {
                    listener: &listener,
                    protocol,
                    address: peer.as_deref(),
                    format: c.wire.map(format_key),
                    since: Time(unix(c.since)),
                    bytes: int(c.bytes),
                    samples: int(c.samples),
                    dropped_samples: int(c.dropped_samples),
                    drop_events: int(c.drop_events),
                    gaps: int(c.gaps),
                    queue_fill: int(c.queue_fill as u64),
                    queue_capacity: int(c.queue_capacity as u64),
                    requests: int(c.requests),
                    refused_requests: int(c.refused_requests),
                    slice_centre_hz: c.slice.map(|s| Hertz(s.centre_hz)),
                    slice_rate_hz: c.slice.map(|s| Hertz(s.rate_hz)),
                    steers,
                    may_transmit: grants.contains(&c.id),
                },
            );
            shown.push(c.id);
        }
        for gone in self.shown_clients.iter().filter(|c| !shown.contains(c)) {
            self.results.remove_row(&CLIENTS, &gone.to_string());
        }
        self.shown_clients = shown;
        let line = self.client_notes(&snap, now);
        self.results.results_mut().update_status(|st| {
            let changed = st.summary != line;
            st.summary = line;
            changed
        });
        self.write_fields(Some(&snap));
    }

    /// The status line of the share: for each client, what it asked for and did not get, once per kind of request, and
    /// the part of its samples that it lost over the last few seconds.
    fn client_notes(&mut self, snap: &sdrkit_serve_core::Snapshot, now: f64) -> String {
        let control = self.state.control();
        let host_reads = self.state.host_reads.load(Ordering::Relaxed);
        let mut parts = Vec::new();
        for c in snap.clients.iter().filter(|c| c.engaged) {
            let steers = self.state.steers(c.id);
            let said: Vec<String> = c
                .not_done
                .iter()
                .map(|(asked, answer)| {
                    format!(
                        "client {} asked for {}: {}",
                        c.id,
                        asked_text(asked),
                        why_not(control, steers, host_reads, answer)
                    )
                })
                .collect();
            if !said.is_empty() {
                self.refusals.retain(|(id, _, _)| *id != c.id);
                self.refusals.push((c.id, now, said));
            }
            if c.closed {
                continue;
            }
            let (rate, part) = self.loss_of(c, snap.capture, now);
            if part >= LOSS_NOTE {
                parts.push(format!(
                    "client {}: {:.0} % dropped, the link is too slow for {} Msps",
                    c.id,
                    part * 100.0,
                    sdrkit_driver_core::fmt_m(rate)
                ));
            }
        }
        self.loss_seen
            .retain(|id, _| snap.clients.iter().any(|c| c.id == *id && !c.closed));
        // A client that left after a refusal (ours fails its start) is still named for a while.
        let open = |id: u64| snap.clients.iter().any(|c| c.id == id && !c.closed);
        self.refusals
            .retain(|(id, at, _)| open(*id) || now - *at < REFUSAL_KEEP_S);
        let mut all: Vec<String> = self.refusals.iter().flat_map(|r| r.2.clone()).collect();
        all.extend(parts);
        all.join("; ")
    }

    /// The rate of client `c` and the part of its samples that it lost over the last window of [`LOSS_WINDOW_S`].
    fn loss_of(
        &mut self,
        c: &sdrkit_serve_core::ClientStats,
        capture: Slice,
        now: f64,
    ) -> (f64, f64) {
        let rate = c.slice.map_or(capture.rate_hz, |s| s.rate_hz);
        // Dropped samples are counted at the capture rate, sent samples at the client's rate.
        let scale = if capture.rate_hz > 0.0 {
            rate / capture.rate_hz
        } else {
            1.0
        };
        let e = self.loss_seen.entry(c.id).or_insert(LossSeen {
            at: now,
            sent: c.samples,
            dropped: c.dropped_samples,
            part: 0.0,
        });
        if now - e.at >= LOSS_WINDOW_S {
            let sent = c.samples.saturating_sub(e.sent) as f64;
            let dropped = c.dropped_samples.saturating_sub(e.dropped) as f64 * scale;
            e.part = if sent + dropped > 0.0 {
                dropped / (sent + dropped)
            } else {
                0.0
            };
            (e.at, e.sent, e.dropped) = (now, c.samples, c.dropped_samples);
        }
        (rate, e.part)
    }

    fn write_fields(&mut self, snap: Option<&sdrkit_serve_core::Snapshot>) {
        let s = &self.state;
        let n = |a: &AtomicU64| int(a.load(Ordering::Relaxed));
        let fields = ShareFields {
            radio: &self.radio,
            session: &self.session,
            format: format_key(self.made.0),
            rate_hz: Hertz(self.rate_hz),
            centre_hz: Hertz(self.centre_hz),
            listeners: int(self.listeners.len() as u64),
            clients: int(self.shown_clients.len() as u64),
            blocks: snap.map_or(0, |s| int(s.blocks)),
            samples: snap.map_or(0, |s| int(s.samples)),
            pool_misses: snap.map_or(0, |s| int(s.pool_misses)),
            host_reads: s.host_reads.load(Ordering::Relaxed),
            requests_done: n(&s.done),
            requests_refused: n(&s.refused),
            transmitting: s.tx_busy.load(Ordering::Relaxed),
            tx_taken: n(&s.tx_taken),
            tx_refused: n(&s.tx_refused),
            tx_samples: n(&s.tx_samples),
            problem: self.problem.as_deref(),
        };
        self.results.put(&fields);
    }

    /// Stops the listeners, disconnects the clients and joins their threads.
    pub(crate) fn shut(&mut self) {
        self.ssdp = None;
        if let Some(mut hub) = self.hub.take() {
            hub.shutdown();
            self.keep_all(hub.take_events(), self.clock.now());
        }
        self.listeners.clear();
    }

    fn failed(e: impl std::error::Error + Send + Sync + 'static) -> CommandError {
        CommandError::Failed {
            source: sdrkit_core::StageError::new(e),
        }
    }
}

impl Owner for ShareOwner {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    /// A setting of the queues, of the format or of the allowed addresses makes the hub again: the clients are
    /// disconnected and the listeners start again on their ports.
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let which = ShareSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        if which == ShareSetting::Allow {
            let text = match value {
                Value::Text(t) => t.as_str(),
                _ => "",
            };
            self.allow =
                parse_allow(text).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
        }
        self.values.set(id, value)?;
        match which {
            ShareSetting::Control => {
                self.state
                    .control
                    .store(self.values.get(&CONTROL).index() as u8, Ordering::Relaxed);
            }
            ShareSetting::RemoteGain => self
                .state
                .remote_gain
                .store(self.values.get(&REMOTE_GAIN), Ordering::Relaxed),
            ShareSetting::MaxClients
            | ShareSetting::ClientQueueMs
            | ShareSetting::DropAfterS
            | ShareSetting::Allow
            | ShareSetting::Format => {
                self.make_hub();
                self.publish(self.clock.now());
            }
        }
        Ok(())
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.results().decl()
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
    fn commands(&self) -> &Commands {
        &self.commands
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = ShareCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        let number = |n: i64| u64::try_from(n).unwrap_or(u64::MAX);
        match cmd {
            ShareCommand::Listen => {
                let a: ListenArgs<'_> = LISTEN.args(args)?;
                let port = u16::try_from(a.port)
                    .map_err(|_| Self::failed(sdrkit_core::net::NetError::BadAddress))?;
                self.next_listener += 1;
                let mut l = KeptListener {
                    number: self.next_listener,
                    protocol: a.protocol,
                    host: a.host.unwrap_or(DEFAULT_HOST).to_owned(),
                    port,
                    hub_id: 0,
                };
                let port = self.start_listener(&mut l).map_err(Self::failed)?;
                let reply = ListenReply {
                    listener: int(l.number),
                    port: i64::from(port),
                };
                self.listeners.push(l);
                self.publish(self.clock.now());
                Ok(LISTEN.reply(&self.commands, &reply))
            }
            ShareCommand::Unlisten => {
                let a: UnlistenArgs = UNLISTEN.args(args)?;
                let n = number(a.listener);
                let at = self.listeners.iter().position(|l| l.number == n);
                let Some(at) = at else {
                    return Err(CommandError::NoSuchKey { arg: SettingId(0) });
                };
                let l = self.listeners.remove(at);
                if let Some(hub) = &self.hub {
                    hub.unlisten(l.hub_id);
                }
                if l.protocol == ShareProtocol::SoapyRemote
                    && !self
                        .listeners
                        .iter()
                        .any(|x| x.protocol == ShareProtocol::SoapyRemote)
                {
                    self.ssdp = None;
                }
                self.results.remove_row(&LISTENERS, &n.to_string());
                self.publish(self.clock.now());
                Ok(None)
            }
            ShareCommand::Kick => {
                let a: KickArgs = KICK.args(args)?;
                let kicked = self.hub.as_ref().is_some_and(|h| h.kick(number(a.client)));
                if !kicked {
                    return Err(CommandError::NoSuchKey { arg: SettingId(0) });
                }
                Ok(None)
            }
            ShareCommand::Grant => {
                let a: GrantArgs = GRANT.args(args)?;
                let client = number(a.client);
                let known = self
                    .hub
                    .as_ref()
                    .is_some_and(|h| h.client(client).is_some());
                if !known {
                    return Err(CommandError::NoSuchKey { arg: SettingId(0) });
                }
                if a.steer.unwrap_or(false) {
                    self.state.steerer.store(client + 1, Ordering::Relaxed);
                } else {
                    let _ = self.state.steerer.compare_exchange(
                        client + 1,
                        0,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    );
                }
                {
                    let mut g = lock(&self.state.tx_grants);
                    g.retain(|c| *c != client);
                    if a.transmit.unwrap_or(false) {
                        g.push(client);
                    }
                }
                self.publish(self.clock.now());
                Ok(None)
            }
        }
    }
}

/// An open share: its owner, its radio and its session.
pub(crate) struct Share {
    pub(crate) owner: Kept<ShareOwner>,
    pub(crate) radio: RadioId,
    pub(crate) session: SessionId,
    events: Arc<Mutex<VecDeque<Event>>>,
    state: Arc<ShareState>,
}

impl Share {
    /// Moves the events of the share into `out`. Does not wait for the lock of the owner.
    pub(crate) fn take_events(&self, out: &mut Vec<Event>) {
        out.extend(lock(&self.events).drain(..));
    }
}

/// The tuner type of the rtl_tcp greeting for a radio named `name`: the tuner of an RTL dongle, else the R820T, whose
/// greeting every rtl_tcp client takes.
fn rtl_tuner(name: &str) -> u32 {
    use sdrkit_driver_core::rtl_tcp::TUNERS;
    name.split_whitespace()
        .find_map(|w| TUNERS.iter().position(|t| t.eq_ignore_ascii_case(w)))
        .map_or(5, |i| i as u32 + 1)
}

/// The model of a radio named `name`: the name without a last word of hexadecimal digits, which is part of a serial
/// number ("HackRF One 0000c66c" is a "HackRF One").
fn model(name: &str) -> &str {
    match name.trim_end().rsplit_once(' ') {
        Some((head, tail)) if tail.len() >= 4 && tail.chars().all(|c| c.is_ascii_hexdigit()) => {
            head
        }
        _ => name.trim_end(),
    }
}

/// The name of the one gain element of a share: the overall gain of the radio, which the radio's own driver spreads
/// over its stages.
const GAIN_ELEMENT: &str = "GAIN";

/// The radio as a SoapyRemote server announces it: what its capabilities declare and nothing more. A gain that can be
/// set is one element; a radio without automatic gain has no gain mode; the share never sets a frequency correction.
fn soapy_device(driver: &str, name: &str, caps: &Capabilities) -> Device {
    let range = |lo: f64, hi: f64| Range {
        min: lo,
        max: hi,
        step: 0.0,
    };
    let elements = |g: Range| {
        if g.max > g.min {
            vec![(GAIN_ELEMENT.to_owned(), g)]
        } else {
            Vec::new()
        }
    };
    let rates = |r: &sdrkit_driver_core::RateLimits| {
        if r.ranges.is_empty() {
            vec![range(r.min_hz, r.max_hz)]
        } else {
            r.ranges.iter().map(|(lo, hi)| range(*lo, *hi)).collect()
        }
    };
    // Below its lowest rate a client gets its own rate out of the capture (`ShareState::own_rate`).
    let rx_rates = |r: &sdrkit_driver_core::RateLimits| {
        let mut v = rates(r);
        let lowest = v.iter().map(|x| x.min).fold(f64::INFINITY, f64::min);
        if lowest.is_finite() && lowest > OWN_RATE_FLOOR_HZ {
            v.insert(0, range(OWN_RATE_FLOOR_HZ, lowest));
        }
        v
    };
    let gain = caps
        .gain_db
        .map_or(range(0.0, 0.0), |(lo, hi, step)| Range {
            min: lo,
            max: hi,
            step,
        });
    let rx = Side {
        freqs: vec![range(caps.min_freq_hz, caps.max_freq_hz)],
        rates: rx_rates(&caps.rates),
        gain,
        elements: elements(gain),
        agc: caps.has_agc,
        correction: false,
        bandwidths: caps
            .bandwidth_hz
            .map(|(lo, hi)| vec![range(lo, hi)])
            .unwrap_or_default(),
        antennas: vec!["RX".into()],
    };
    let tx = caps.transmit.as_ref().map(|t| Side {
        freqs: vec![range(t.min_freq_hz, t.max_freq_hz)],
        rates: rates(&t.rates),
        gain: range(t.power_db.0, t.power_db.1),
        elements: elements(range(t.power_db.0, t.power_db.1)),
        agc: false,
        correction: false,
        bandwidths: t
            .bandwidth_hz
            .map(|(lo, hi)| vec![range(lo, hi)])
            .unwrap_or_default(),
        antennas: vec!["TX".into()],
    });
    let mut info = sdrkit_serve_soapyremote::Kwargs::new();
    info.insert("name".into(), name.to_owned());
    info.insert("model".into(), model(name).to_owned());
    Device {
        driver: driver.to_owned(),
        hardware: model(name).to_owned(),
        info,
        rx,
        tx,
        bias_tee: false,
    }
}

impl Shared {
    pub(crate) fn share(&self, id: ShareId) -> Option<Arc<Share>> {
        lock(&self.shares).get(id.handle()).map(Arc::clone)
    }

    /// The share of radio `radio`, if it has one.
    pub(crate) fn share_of(&self, radio: RadioId) -> Option<ShareId> {
        lock(&self.shares)
            .iter()
            .find(|(_, s)| s.radio == radio)
            .map(|(h, _)| ShareId::new(h))
    }

    /// Opens the share of radio `radio`: starts its session on the radio and makes its owner. The share has no listener
    /// yet. The rows of the share and of its session go into `held`, the engine owner that the caller holds, or into
    /// the engine owner after a lock.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`] for a radio that is not open, [`EngineError::ShareOpen`] if the radio has a share,
    /// else the error of the session.
    pub(crate) fn open_share(
        self: &Arc<Self>,
        mut held: Option<&mut EngineOwner>,
        radio: RadioId,
    ) -> Result<ShareId, EngineError> {
        let r = self
            .radio(radio)
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Radio(radio)))?;
        if let Some(open) = self.share_of(radio) {
            return Err(EngineError::ShareOpen {
                share: open,
                key: share_key(open),
            });
        }
        let (caps, native, tuning, set) = {
            let o = r.owner.lock();
            (
                o.capabilities().clone(),
                o.sample_type(),
                o.tuning(),
                o.gain_settings(),
            )
        };
        // The transmit stream of the session takes the samples of a client that transmits.
        let room = self.transmit_room(radio).unwrap_or(1);
        let streams = Arc::new(Streams::new(crate::AUDIO_CAP, room));
        let state = Arc::new(ShareState {
            feeder: Mutex::new(None),
            feeder_new: AtomicBool::new(false),
            control: AtomicU8::new(DEFAULT_CONTROL.index() as u8),
            remote_gain: AtomicBool::new(true),
            host_reads: AtomicBool::new(r.feed.readers() > 0),
            rate_fixed: AtomicBool::new(!r.rate_is_automatic()),
            steerer: AtomicU64::new(0),
            first: AtomicU64::new(0),
            limits: Limits::of(&caps),
            centre_bits: AtomicU64::new(tuning.1.to_bits()),
            rate_bits: AtomicU64::new(tuning.0.to_bits()),
            gain_bits: AtomicU64::new(set.0.to_bits()),
            agc: AtomicBool::new(set.1),
            bandwidth_bits: AtomicU64::new(set.2.to_bits()),
            transmit_allowed: Arc::clone(&self.transmit_allowed),
            tx_grants: Mutex::default(),
            tx_busy: Arc::default(),
            streams: Arc::clone(&streams),
            pending: Mutex::default(),
            done: AtomicU64::new(0),
            refused: AtomicU64::new(0),
            tx_taken: AtomicU64::new(0),
            tx_refused: AtomicU64::new(0),
            tx_samples: Arc::default(),
            engine: Arc::downgrade(self),
        });
        let sink = ShareSink {
            state: Arc::clone(&state),
            feeder: None,
            rate_hz: tuning.0,
            centre_hz: tuning.1,
            lost: 0,
            bytes: Vec::with_capacity(BLOCK * 8),
        };
        let session = self.start_on(
            held.as_deref_mut(),
            crate::session::By::Share,
            Box::new(sink),
            (crate::Input::Radio(radio), Arc::clone(&r.feed)),
            None,
            None,
            streams,
        )?;
        let decl = ResultsDecl::new()
            .with_fields(ShareFields::SHAPE.fields)
            .with_table(&LISTENERS)
            .with_table(&CLIENTS)
            .event(&CLIENT_JOINED)
            .event(&CLIENT_LEFT)
            .event(&REFUSED)
            .event(&LISTEN_FAILED);
        let events = Arc::new(Mutex::new(VecDeque::with_capacity(EVENT_CAP)));
        let values = Values::defaults(&ShareSetting::schema());
        let mut seed = [0u8; 8];
        self.facilities.random.fill(&mut seed);
        let mut owner = ShareOwner {
            allow: parse_allow(DEFAULT_ALLOW).unwrap_or_default(),
            values,
            results: Published::from_decl(Arc::new(decl)),
            commands: ShareCommand::commands().clone(),
            state: Arc::clone(&state),
            net: Arc::clone(&self.net),
            clock: self.facilities.clock.clone(),
            catalog: Arc::clone(&self.events),
            events: Arc::clone(&events),
            hub: None,
            made: (SampleType::Cf32, 0.0),
            native,
            rate_hz: tuning.0,
            centre_hz: tuning.1,
            listeners: Vec::new(),
            next_listener: 0,
            device: soapy_device(r.driver, &caps.name, &caps),
            uuid: format!("sdrkit-{:016x}", u64::from_le_bytes(seed)),
            ssdp: None,
            radio: radio_key(radio),
            session: session_key(session),
            problem: None,
            shown_clients: Vec::new(),
            loss_seen: std::collections::HashMap::new(),
            refusals: Vec::new(),
            rtl_tuner: rtl_tuner(&caps.name),
        };
        owner.make_hub();
        owner.publish(self.facilities.clock.now());
        let share = Arc::new(Share {
            owner: Kept::new(owner),
            radio,
            session,
            events,
            state,
        });
        let id = ShareId::new(lock(&self.shares).insert(share));
        let row = ShareRow {
            radio: &radio_key(radio),
            session: &session_key(session),
        };
        match held {
            Some(e) => SHARES.put(&mut e.results, &share_key(id), &row),
            None => SHARES.put(&mut self.engine.lock().results, &share_key(id), &row),
        }
        Ok(id)
    }

    /// Closes share `id`: its listeners stop, its clients are disconnected and its session stops. The control thread
    /// closes the session once its thread has ended. The rows change in `held`, the engine owner that the caller holds,
    /// or in the engine owner after a lock.
    ///
    /// # Errors
    ///
    /// [`EngineError::NoSuchOwner`].
    pub(crate) fn close_share(
        &self,
        held: Option<&mut EngineOwner>,
        id: ShareId,
    ) -> Result<(), EngineError> {
        let share = lock(&self.shares)
            .remove(id.handle())
            .ok_or(EngineError::NoSuchOwner(OwnerRef::Share(id)))?;
        // The threads of the clients take no lock of the engine, so they end while this thread waits for them.
        share.owner.lock().shut();
        let session = self.session(share.session);
        if let Some(s) = &session {
            s.stop();
            s.link.feed.unsubscribe(share.session.to_u64());
            lock(&self.closing).push(share.session);
        }
        let note = |e: &mut EngineOwner| {
            if session.is_some() {
                crate::set_state(e, share.session, crate::session::SessionState::Stopped);
            }
            e.results.remove_row(&SHARES, &share_key(id));
        };
        match held {
            Some(e) => note(e),
            None => note(&mut self.engine.lock()),
        }
        Ok(())
    }

    /// Closes the share of radio `radio`, if it has one.
    pub(crate) fn close_share_of(&self, radio: RadioId) {
        if let Some(id) = self.share_of(radio) {
            let _ = self.close_share(None, id);
        }
    }

    /// Runs the shares at a tick: carries out what the steering client of each asked of its radio, tells each share the
    /// tuning of its radio and whether a session of the host reads it, and lets it copy its counters into its results. A
    /// share whose radio or session is gone is closed.
    pub(crate) fn step_shares(&self, now: f64) {
        let shares: Vec<(ShareId, Arc<Share>)> = lock(&self.shares)
            .iter()
            .map(|(h, s)| (ShareId::new(h), Arc::clone(s)))
            .collect();
        for (id, share) in shares {
            let radio = self.radio(share.radio);
            let (Some(radio), Some(_)) = (radio, self.session(share.session)) else {
                let _ = self.close_share(None, id);
                continue;
            };
            let host_reads = radio.feed.readers() > 0;
            let asked = std::mem::take(&mut *lock(&share.state.pending));
            let (tuning, set) = {
                let mut o = radio.owner.lock();
                if !host_reads {
                    for r in asked {
                        // A value that the radio refuses stays as it was; the client reads the result back.
                        let _ = o.remote(&r);
                    }
                }
                (o.tuning(), o.gain_settings())
            };
            let st = &share.state;
            st.rate_bits.store(tuning.0.to_bits(), Ordering::Relaxed);
            st.gain_bits.store(set.0.to_bits(), Ordering::Relaxed);
            st.agc.store(set.1, Ordering::Relaxed);
            st.bandwidth_bits.store(set.2.to_bits(), Ordering::Relaxed);
            share
                .owner
                .lock()
                .tick(now, tuning, host_reads, !radio.rate_is_automatic());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, Setup};
    use sdrkit_core::interface::ReadFields;
    use sdrkit_core::Facilities;
    use sdrkit_driver_core::rtl_tcp::fake::{FakeOpts, FakeRtl};
    use std::time::Instant;

    /// The time in ms that `read` takes when it starts 30 ms after `hold` started on another thread.
    fn read_during(hold: impl FnOnce() + Send, read: impl FnOnce()) -> f64 {
        std::thread::scope(|s| {
            s.spawn(hold);
            std::thread::sleep(Duration::from_millis(30));
            let t = Instant::now();
            read();
            t.elapsed().as_secs_f64() * 1e3
        })
    }

    /// The host reads the fields and the tables of a share, and the tables `devices`, `shares` and `drivers` of the
    /// engine, while another thread holds the lock of that owner for 150 ms, as the control thread does when it makes
    /// the hub of a share again or lists the USB bus. The read takes the published copy and does not wait.
    #[test]
    fn a_read_of_a_share_and_of_the_device_table_does_not_wait_for_the_control_thread() {
        let server = FakeRtl::start(FakeOpts::default());
        let e = Engine::new(Setup {
            facilities: Facilities::default(),
            transport: server.transport(),
            stacks: Vec::new(),
            families: Vec::new(),
            components: sdrkit_graph::Registry::standard(),
        })
        .unwrap();
        let kind = e.find("driver.rtl_tcp").unwrap();
        let values = e.owner(kind, |o| Values::defaults(o.schema())).unwrap();
        let radio = e.open_radio("rtl_tcp", &server.addr(), &values).unwrap();
        let id = e.open_share(radio).unwrap();
        let share = e.shared.share(id).unwrap();
        e.owner(OwnerRef::Share(id), |o| {
            LISTEN.call(
                o,
                &ListenArgs {
                    protocol: ShareProtocol::RtlTcp,
                    port: 0,
                    host: Some("127.0.0.1"),
                },
            )
        })
        .unwrap()
        .unwrap();
        let hold = Duration::from_millis(150);
        let read_share = || {
            e.owner(OwnerRef::Share(id), |o| {
                let f = ShareFields::read(o.results().fields()).map(|f| f.listeners);
                let rows = LISTENERS.rows::<ListenerRow<'_>>(o.results()).count();
                (f, rows)
            })
            .unwrap()
        };
        assert_eq!(read_share(), (Some(1), 1));
        let ms = read_during(
            || {
                let _held = share.owner.lock();
                std::thread::sleep(hold);
            },
            || assert_eq!(read_share(), (Some(1), 1)),
        );
        eprintln!("a read of a share while its owner is held for 150 ms: {ms:.3} ms");
        assert!(ms < 10.0, "{ms}");
        let read_engine = || {
            e.owner(OwnerRef::Engine, |o| {
                let res = o.results();
                let rows = |key: &str| {
                    res.decl()
                        .table_id(key)
                        .and_then(|t| res.table(t))
                        .map(|t| t.rows_newest_first().count())
                };
                (
                    SHARES.rows::<ShareRow<'_>>(res).count(),
                    rows(crate::DEVICES.key),
                    rows(crate::DRIVERS.key).is_some_and(|n| n > 0),
                )
            })
            .unwrap()
        };
        assert_eq!(read_engine(), (1, Some(0), true));
        let ms = read_during(
            || {
                let _held = e.shared.engine.lock();
                std::thread::sleep(hold);
            },
            || assert_eq!(read_engine(), (1, Some(0), true)),
        );
        eprintln!(
            "a read of the engine tables while the engine owner is held for 150 ms: {ms:.3} ms"
        );
        assert!(ms < 10.0, "{ms}");
        e.close_radio(radio).unwrap();
    }
}
