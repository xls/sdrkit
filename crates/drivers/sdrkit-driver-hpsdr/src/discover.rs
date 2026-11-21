//! Discovery: the request that every HPSDR radio on the local networks answers, and the probe of one address.
//! Standard: HPSDR USB data protocol over Metis (protocol 1), openHPSDR Ethernet protocol (protocol 2).

use crate::hub::{Facts, Hub};
use crate::p1;
use crate::p2;
use crate::profile::Profile;
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use sdrkit_core::net::{Datagram, DatagramOptions, NetError, Transport};
use sdrkit_driver_core::DriverError;
use std::time::{Duration, Instant};

/// The port every HPSDR radio listens on.
pub const DEFAULT_PORT: u16 = 1024;

/// How long one receive of a discovery waits.
const WAIT: Duration = Duration::from_millis(20);

/// A radio that answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The address it answered from.
    pub address: SocketAddr,
    /// It streams to some program now.
    pub busy: bool,
    /// Its board id.
    pub board: u8,
    /// The receivers it says it has.
    pub receivers: Option<u8>,
    /// The MAC address.
    pub mac: [u8; 6],
    /// The gateware (protocol 1, `73.2`) or firmware (protocol 2) version.
    pub version: String,
    /// The protocol version a protocol 2 radio supports, times ten.
    pub protocol: Option<u8>,
}

impl Found {
    /// What the stream of the radio keeps of the answer.
    pub(crate) fn facts(&self) -> Facts {
        Facts {
            board: Some(self.board),
            mac: Some(self.mac),
            version: self.version.clone(),
            protocol: self.protocol,
            receivers: self.receivers,
            busy: self.busy,
        }
    }
}

/// Reads an answer of the protocol of `p` from `from`, if its board is one of the profile.
pub(crate) fn parse(p: &Profile, b: &[u8], from: SocketAddr) -> Option<Found> {
    let found = if p.protocol == 1 {
        let r = p1::Reply::parse(b)?;
        let version = match r.minor {
            Some(m) => format!("{}.{m}", r.gateware),
            None => r.gateware.to_string(),
        };
        Found {
            address: from,
            busy: r.busy,
            board: r.board,
            receivers: r.receivers,
            mac: r.mac,
            version,
            protocol: None,
        }
    } else {
        let r = p2::Reply::parse(b)?;
        Found {
            address: from,
            busy: r.busy,
            board: r.board,
            receivers: r.receivers,
            mac: r.mac,
            version: r.firmware.to_string(),
            protocol: Some(r.protocol),
        }
    };
    p.accepts(found.board).then_some(found)
}

fn request(p: &Profile) -> &'static [u8] {
    static P1: [u8; p1::DISCOVERY_LEN] = p1::discovery_request();
    static P2: [u8; p2::SHORT] = p2::discovery_request();
    if p.protocol == 1 {
        &P1
    } else {
        &P2
    }
}

/// Collects the answers on `socks` until `timeout`. Each radio (source address) is listed once, in the order it answered.
/// Answers of another board are left out. What the radios say is kept in `hub`. With `only`, the first answer from that
/// address ends the wait.
fn collect(
    p: &Profile,
    hub: &Hub,
    socks: &mut [Box<dyn Datagram>],
    timeout: Duration,
    only: Option<IpAddr>,
) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    let end = Instant::now() + timeout;
    let mut buf = [0u8; 1500];
    while Instant::now() < end {
        for s in socks.iter_mut() {
            let Ok((len, from)) = s.recv_from(&mut buf) else {
                continue;
            };
            if only.is_some_and(|ip| ip != from.ip())
                || out.iter().any(|f| f.address.ip() == from.ip())
            {
                continue;
            }
            if let Some(f) = parse(p, &buf[..len], from) {
                hub.remember(&from.to_string(), f.board, f.receivers);
                out.push(f);
                if only.is_some() {
                    return out;
                }
            }
        }
    }
    out
}

/// Opens a broadcast socket at `local`.
fn broadcast_socket(net: &dyn Transport, local: IpAddr) -> Result<Box<dyn Datagram>, NetError> {
    let mut s = net.bind(&DatagramOptions {
        broadcast: true,
        ..DatagramOptions::at(SocketAddr::new(local, 0))
    })?;
    s.set_read_timeout(Some(WAIT))?;
    Ok(s)
}

/// Discovery on the local networks: the request goes to the limited broadcast address on port 1024, from one socket on any
/// address and one on each interface that the transport lists.
pub(crate) fn discover(
    p: &Profile,
    hub: &Hub,
    net: &dyn Transport,
    timeout: Duration,
) -> Vec<Found> {
    let mut locals = vec![IpAddr::V4(Ipv4Addr::UNSPECIFIED)];
    locals.extend(
        net.interfaces()
            .iter()
            .filter_map(|i| i.v4.map(|n| n.addr))
            .filter(|ip| !ip.is_loopback())
            .map(IpAddr::V4),
    );
    let mut socks: Vec<Box<dyn Datagram>> = locals
        .into_iter()
        .filter_map(|ip| broadcast_socket(net, ip).ok())
        .collect();
    let to = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), DEFAULT_PORT);
    for s in &mut socks {
        let _ = s.send_to(request(p), to);
    }
    collect(p, hub, &mut socks, timeout, None)
}

/// Asks the radio at `host:port` to answer.
///
/// # Errors
///
/// [`DriverError::Net`]: [`NetError::NoSuchHost`] if the host has no address, [`NetError::TimedOut`] if no radio of the
/// profile answers within `timeout`.
pub(crate) fn probe(
    p: &Profile,
    hub: &Hub,
    net: &dyn Transport,
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<Found, DriverError> {
    let addr = net.resolve(host, port)?;
    let mut s = net.bind(&DatagramOptions::any_v4())?;
    s.set_read_timeout(Some(WAIT))?;
    s.send_to(request(p), addr).map_err(NetError::from)?;
    let mut found = collect(p, hub, &mut [s], timeout, Some(addr.ip()));
    if found.is_empty() {
        return Err(NetError::TimedOut.into());
    }
    let f = found.swap_remove(0);
    hub.remember(&format!("{host}:{port}"), f.board, f.receivers);
    Ok(f)
}
