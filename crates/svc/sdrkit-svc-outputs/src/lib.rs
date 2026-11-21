//! Feeds of sdrkit events to other programs: MQTT, webhooks, files, UDP, a TCP server, APRS-IS gating and spot reports. The
//! owner `service.outputs` holds the outputs that the user set up. Nothing connects anywhere until the user turns an output on.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod aprsis;
pub mod ax25;
pub mod format;
pub mod http;
pub mod hub;
pub mod mqtt;
pub mod net;
pub mod runner;
pub mod spots;
pub mod stream;

pub use format::LineFormat;
pub use hub::{OutputCommand, OutputKind, Outputs, KEY};
pub use net::{Backoff, Url};
pub use runner::{Item, Sink, SinkRunner, SinkStats};
pub use sdrkit_core::net::{Conn, Datagram, Listener, Transport};

sdrkit_core::choice_enum! {
    /// What the other side did that the output did not expect.
    pub enum Protocol {
        /// The MQTT broker did not answer CONNECT.
        NoConnack = ("no_connack", "No answer to CONNECT"),
        /// The MQTT broker refused the protocol version.
        BadVersion = ("bad_version", "Protocol version refused"),
        /// The MQTT broker refused the client id.
        BadClientId = ("bad_client_id", "Client id refused"),
        /// The MQTT broker is not available.
        Unavailable = ("unavailable", "Server not available"),
        /// The MQTT broker refused the user name or password.
        BadLogin = ("bad_login", "Bad user name or password"),
        /// The MQTT broker refused the client.
        NotAuthorised = ("not_authorised", "Not authorised"),
        /// The MQTT broker refused for another reason.
        Refused = ("refused", "Refused"),
        /// An MQTT packet that the output does not expect.
        UnexpectedPacket = ("unexpected_packet", "Unexpected packet"),
        /// An MQTT length that does not fit.
        LengthTooLong = ("length_too_long", "Length too long"),
        /// No answer to a keep-alive.
        NoPing = ("no_ping", "No answer to a keep-alive"),
        /// The output is not connected.
        NotConnected = ("not_connected", "Not connected"),
        /// No HTTP status line.
        NoStatus = ("no_status", "No HTTP status"),
        /// The APRS-IS server did not answer the login.
        NoLogin = ("no_login", "No answer to the login"),
        /// The APRS-IS server refused the login.
        LoginRefused = ("login_refused", "Login refused"),
        /// The test hook of the runner.
        Down = ("down", "Down"),
    }
}

sdrkit_core::choice_enum! {
    /// Why a setting of an output is refused.
    pub enum BadSetting {
        /// The output has no name.
        NoName = ("no_name", "No name"),
        /// The address is not `scheme://host[:port]` or `host:port`.
        Address = ("address", "Not an address"),
        /// The address has another scheme than the kind of output takes.
        Scheme = ("scheme", "Wrong scheme"),
        /// The QoS is not 0 or 1.
        Qos = ("qos", "QoS 0 or 1"),
        /// A header holds a line break.
        Header = ("header", "Line break in a header"),
        /// A call sign is needed.
        Callsign = ("callsign", "No call sign"),
        /// A TCP server outside the computer was not allowed.
        Remote = ("remote", "Remote listening not allowed"),
    }
}

/// An output error. It never holds a password, a token or a body.
#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    /// A network error.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// A setting is refused.
    #[error("setting: {}", sdrkit_svc_events::choice_key(*.0))]
    Setting(BadSetting),
    /// A name in a format, a filter or a template does not resolve.
    #[error("{arg}: {error}")]
    Name {
        /// The argument that holds the name.
        arg: &'static str,
        /// What does not resolve.
        error: sdrkit_svc_events::NameError,
    },
    /// No output has this row key.
    #[error("no output {0}")]
    NoSuchOutput(String),
    /// The output is off.
    #[error("the output is off")]
    Off,
    /// The queue of the output is full.
    #[error("the queue of the output is full")]
    QueueFull,
    /// The other side said something unexpected.
    #[error("protocol: {}", sdrkit_svc_events::choice_key(*.0))]
    Protocol(Protocol),
    /// An HTTP status outside 200 to 299.
    #[error("HTTP status {0}")]
    Status(u16),
    /// The network refused a connection or a socket ([`NetError::TlsUnavailable`](sdrkit_core::NetError::TlsUnavailable)
    /// if TLS was requested and the transport does not support it).
    #[error("{0}")]
    Net(#[from] sdrkit_core::NetError),
    /// JSON could not be written.
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The system refused to start the thread of a sink.
    #[error("the sink thread did not start: {0}")]
    Thread(#[source] std::io::Error),
    /// A file in the host's storage could not be read or written.
    #[error("output file {path}: {source}")]
    Storage {
        /// The file.
        path: sdrkit_core::StorePath,
        /// What the storage said.
        source: sdrkit_core::StorageError,
    },
}

impl OutputError {
    /// True if a new try can help (network trouble, a server error), false for a setting or a refusal.
    pub fn retryable(&self) -> bool {
        match self {
            OutputError::Io(_) | OutputError::Protocol(_) | OutputError::Storage { .. } => true,
            OutputError::Net(e) => e.retryable(),
            OutputError::Status(s) => *s >= 500 || *s == 429 || *s == 408,
            _ => false,
        }
    }
}
