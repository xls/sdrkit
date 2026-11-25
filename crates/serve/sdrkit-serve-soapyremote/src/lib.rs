//! The SoapyRemote server protocol on the serving core: the RPC calls through which a SoapySDR client finds the radio,
//! reads its ranges, gains, antennas, rates and formats, sets it up and streams; receive and transmit streams over UDP with
//! the window flow control of the protocol; and the SSDP answer of discovery. Each RPC connection is one client of the hub.
//! Settings go to the owner's `Control` as requests; a refusal returns to the client as an exception.
//!
//! Standard: the SoapyRemote protocol, RPC version 0x0400, with the wire codec of `sdrkit-driver-soapyremote` (`rpc`,
//! `stream`, `discovery`, `format`).

mod session;
mod ssdp;
mod stream;

pub use sdrkit_driver_soapyremote::rpc::{Kwargs, Range};
pub use ssdp::{ssdp_answer, Ssdp};

use sdrkit_core::net::{Conn, Transport};
use sdrkit_driver_soapyremote::format::Format;
use sdrkit_serve_core::{Protocol, SampleType, Session};
use std::sync::Arc;

/// The SoapyRemote server port.
pub const DEFAULT_PORT: u16 = 55132;

/// What one direction of the radio offers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Side {
    /// Tunable ranges, Hz.
    pub freqs: Vec<Range>,
    /// Sample rate ranges, Hz.
    pub rates: Vec<Range>,
    /// The overall gain range, dB.
    pub gain: Range,
    /// The gain elements and their ranges, dB. One element is the overall gain.
    pub elements: Vec<(String, Range)>,
    /// Filter bandwidth ranges, Hz; empty when the radio has no filter setting.
    pub bandwidths: Vec<Range>,
    /// Antenna names.
    pub antennas: Vec<String>,
}

/// The radio as the server announces it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Device {
    /// The driver key (`rtlsdr`).
    pub driver: String,
    /// The hardware key.
    pub hardware: String,
    /// The hardware information; a `serial` key is matched by a make.
    pub info: Kwargs,
    /// The receive side.
    pub rx: Side,
    /// The transmit side, if the radio transmits.
    pub tx: Option<Side>,
    /// True if the radio has a bias tee, offered as the setting `biastee`.
    pub bias_tee: bool,
}

/// The SoapyRemote protocol for one radio.
pub struct SoapyRemote {
    transport: Arc<dyn Transport>,
    device: Arc<Device>,
    uuid: String,
}

impl SoapyRemote {
    /// A server for `device` whose stream sockets open through `transport`, with the server id `uuid`.
    pub fn new(transport: Arc<dyn Transport>, device: Device, uuid: &str) -> SoapyRemote {
        SoapyRemote {
            transport,
            device: Arc::new(device),
            uuid: uuid.to_owned(),
        }
    }
}

impl Protocol for SoapyRemote {
    fn name(&self) -> &'static str {
        "soapyremote"
    }
    fn session(&self, conn: &dyn Conn, _source: SampleType) -> Session {
        Session {
            greeting: Vec::new(),
            outlet: None,
            parser: Box::new(session::Rpc::new(
                self.transport.clone(),
                self.device.clone(),
                &self.uuid,
                conn.peer_addr(),
            )),
        }
    }
}

/// The receive formats that a source of `source` gives exactly, the source's own first: CS16 and CF32 for 16 bits, CS8,
/// CS16 and CF32 for 8 bits, CF32 for floats.
pub fn rx_formats(source: SampleType) -> Vec<Format> {
    match source {
        SampleType::Cs16 => vec![Format::Cs16, Format::Cf32],
        SampleType::Cs8 | SampleType::Cu8 => vec![Format::Cs8, Format::Cs16, Format::Cf32],
        _ => vec![Format::Cf32],
    }
}

/// The source's own format as SoapySDR names it, and its full scale.
pub fn native(source: SampleType) -> (Format, f64) {
    match source {
        SampleType::Cs16 => (Format::Cs16, 32768.0),
        SampleType::Cs8 => (Format::Cs8, 128.0),
        SampleType::Cu8 => (Format::Cu8, 128.0),
        _ => (Format::Cf32, 1.0),
    }
}
