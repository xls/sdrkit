//! Network connections that the host provides: byte streams, datagram sockets and listeners, opened through a [`Transport`].
//! [`FakeTransport`] keeps everything in memory for tests.

use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::io::{self, Read, Write};
use std::time::Duration;

mod fake;
#[cfg(feature = "std-net")]
mod std_net;

pub use fake::FakeTransport;
#[cfg(feature = "std-net")]
pub use std_net::StdNet;

/// Why a network operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NetError {
    /// The text is not an address (`host:port`, `[v6]:port` or a host).
    #[error("not an address")]
    BadAddress,
    /// The host refused the connection: nothing listens on that port.
    #[error("the connection was refused")]
    Refused,
    /// The host name has no address.
    #[error("no such host")]
    NoSuchHost,
    /// Nothing answered in time.
    #[error("timed out")]
    TimedOut,
    /// The connection is closed.
    #[error("the connection is closed")]
    Closed,
    /// TLS was requested and the transport does not support it. Nothing was sent without it.
    #[error("TLS is not available from this transport")]
    TlsUnavailable,
    /// The certificate of the server is not trusted.
    #[error("the certificate of the server is not trusted")]
    Untrusted,
    /// The TLS handshake failed.
    #[error("the TLS handshake failed")]
    Tls,
    /// The caller stopped the operation.
    #[error("stopped")]
    Aborted,
    /// Another error of the system.
    #[error("network error: {kind}")]
    Io {
        /// The kind of the error.
        kind: io::ErrorKind,
    },
}

impl NetError {
    /// True if the same operation can work later (no answer, a dropped link), false for a refusal that stays.
    pub fn retryable(self) -> bool {
        match self {
            NetError::Refused | NetError::NoSuchHost | NetError::TimedOut | NetError::Closed => {
                true
            }
            NetError::Io { kind } => !matches!(
                kind,
                io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported
            ),
            NetError::BadAddress
            | NetError::TlsUnavailable
            | NetError::Untrusted
            | NetError::Tls
            | NetError::Aborted => false,
        }
    }
}

impl From<io::Error> for NetError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::ConnectionRefused => NetError::Refused,
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => NetError::TimedOut,
            io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::NotConnected
            | io::ErrorKind::UnexpectedEof => NetError::Closed,
            kind => NetError::Io { kind },
        }
    }
}

impl From<NetError> for io::Error {
    fn from(e: NetError) -> Self {
        let kind = match e {
            NetError::BadAddress => io::ErrorKind::InvalidInput,
            NetError::Refused => io::ErrorKind::ConnectionRefused,
            NetError::NoSuchHost => io::ErrorKind::NotFound,
            NetError::TimedOut => io::ErrorKind::TimedOut,
            NetError::Closed => io::ErrorKind::BrokenPipe,
            NetError::TlsUnavailable => io::ErrorKind::Unsupported,
            NetError::Untrusted | NetError::Tls => io::ErrorKind::InvalidData,
            NetError::Aborted => io::ErrorKind::Interrupted,
            NetError::Io { kind } => kind,
        };
        io::Error::new(kind, e)
    }
}

/// A byte stream to another program. A read that waits longer than the read timeout fails with
/// [`io::ErrorKind::WouldBlock`] or [`io::ErrorKind::TimedOut`].
pub trait Conn: Read + Write + Send {
    /// Sets how long a read can wait (`None`: no limit).
    ///
    /// # Errors
    ///
    /// [`NetError`] if the system refuses the timeout.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError>;
    /// Sets how long a write can wait (`None`: no limit).
    ///
    /// # Errors
    ///
    /// [`NetError`] if the system refuses the timeout.
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError>;
    /// Asks the system for a receive buffer of `bytes`.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the system refuses the size.
    fn set_recv_buffer(&mut self, bytes: usize) -> Result<(), NetError>;
    /// A second handle on the same connection, for a reader and a writer on two threads.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the system cannot make the handle.
    fn try_clone(&self) -> Result<Box<dyn Conn>, NetError>;
    /// Closes both directions. A read that waits on any handle of the connection returns at once.
    fn shutdown(&self);
    /// The address of the other end. `None` if the system does not give it.
    fn peer_addr(&self) -> Option<SocketAddr>;
}

/// A datagram socket. [`Datagram::send`] goes to the address it was opened for.
pub trait Datagram: Send {
    /// Sends one datagram to the address the socket was opened for.
    ///
    /// # Errors
    ///
    /// The system's error. A socket from [`Transport::bind`] has no such address and returns [`io::ErrorKind::NotConnected`].
    fn send(&mut self, bytes: &[u8]) -> io::Result<()>;
    /// Sends one datagram to `to`.
    ///
    /// # Errors
    ///
    /// The system's error.
    fn send_to(&mut self, bytes: &[u8], to: SocketAddr) -> io::Result<()>;
    /// Receives one datagram into `buf`. It returns the length and the sender.
    ///
    /// # Errors
    ///
    /// [`io::ErrorKind::WouldBlock`] or [`io::ErrorKind::TimedOut`] after the read timeout, else the system's error.
    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>;
    /// Receives one datagram into `buf` if one waits. It never blocks. It returns the length and the sender, or `None` if
    /// no datagram waits.
    ///
    /// # Errors
    ///
    /// The system's error.
    fn try_recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>>;
    /// Sets how long a receive can wait (`None`: no limit).
    ///
    /// # Errors
    ///
    /// [`NetError`] if the system refuses the timeout.
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError>;
    /// The local address, with the port the system chose for port 0.
    fn local_addr(&self) -> SocketAddr;
    /// The size of the receive buffer that the system reported, in bytes. It can differ from
    /// [`DatagramOptions::recv_buffer`]. `None` if the system does not say.
    fn recv_buffer(&self) -> Option<usize>;
    /// A second handle on the same socket.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the system cannot make the handle.
    fn try_clone(&self) -> Result<Box<dyn Datagram>, NetError>;
}

/// A socket that takes connections from other programs.
pub trait Listener: Send {
    /// The address it listens on, with the port the system chose for port 0.
    fn local_addr(&self) -> SocketAddr;
    /// A connection that waits, if one does. It never blocks.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the system fails.
    fn accept(&mut self) -> Result<Option<Box<dyn Conn>>, NetError>;
}

/// How [`Transport::bind`] opens a datagram socket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatagramOptions {
    /// The local address. Port 0 lets the system choose.
    pub local: SocketAddr,
    /// The socket can send to a broadcast address.
    pub broadcast: bool,
    /// The receive buffer to ask for, if any.
    pub recv_buffer: Option<usize>,
    /// The time to live of multicast datagrams, if not the system's default.
    pub multicast_ttl: Option<u32>,
    /// The interface that sends multicast datagrams, if not the default one.
    pub multicast_interface: Option<Interface>,
}

impl DatagramOptions {
    /// A socket on any IPv4 address and a port that the system chooses.
    pub fn any_v4() -> Self {
        DatagramOptions::at(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))
    }
    /// A socket at `local`, with no other options.
    pub fn at(local: SocketAddr) -> Self {
        DatagramOptions {
            local,
            broadcast: false,
            recv_buffer: None,
            multicast_ttl: None,
            multicast_interface: None,
        }
    }
}

/// A network interface that is up and can send multicast datagrams.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Interface {
    /// The system's name (`en0`).
    pub name: String,
    /// The interface index.
    pub index: u32,
    /// Its first IPv4 address with the netmask, if it has one.
    pub v4: Option<Ipv4Net>,
    /// It has an IPv6 address.
    pub v6: bool,
}

/// An IPv4 address of an interface and the netmask of its network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv4Net {
    /// The address.
    pub addr: Ipv4Addr,
    /// The netmask (`255.255.255.0`).
    pub netmask: Ipv4Addr,
}

impl Ipv4Net {
    /// The directed broadcast address of the network: the address with every host bit set.
    pub fn broadcast(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.addr) | !u32::from(self.netmask))
    }
}

/// Opens connections and sockets. The host supplies one to every owner that uses the network.
pub trait Transport: Send + Sync {
    /// Connects to `host:port`, with TLS if `tls` is set, within `timeout`. Writes wait at most `timeout` until the caller sets
    /// another limit. A transport that cannot do TLS fails with [`NetError::TlsUnavailable`] and sends nothing.
    ///
    /// # Errors
    ///
    /// [`NetError`] if no connection is made.
    fn connect(
        &self,
        host: &str,
        port: u16,
        tls: bool,
        timeout: Duration,
    ) -> Result<Box<dyn Conn>, NetError>;
    /// A datagram socket that sends to `host:port`, on a local port that the system chooses.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the host has no address or the socket cannot open.
    fn datagram(&self, host: &str, port: u16) -> Result<Box<dyn Datagram>, NetError>;
    /// A datagram socket opened with `options`, for any number of peers.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the socket cannot open with these options.
    fn bind(&self, options: &DatagramOptions) -> Result<Box<dyn Datagram>, NetError>;
    /// A socket that listens on `host:port`. Port 0 lets the system choose.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the address is refused.
    fn listen(&self, host: &str, port: u16) -> Result<Box<dyn Listener>, NetError>;
    /// The first address of `host`.
    ///
    /// # Errors
    ///
    /// [`NetError::NoSuchHost`] if the name has no address.
    fn resolve(&self, host: &str, port: u16) -> Result<SocketAddr, NetError>;
    /// The interfaces that are up and can send multicast datagrams. The default returns none, and a search then uses the default
    /// interface.
    fn interfaces(&self) -> Vec<Interface> {
        Vec::new()
    }
}

/// The host and the port of `host:port`, `[v6]:port`, a bare IPv6 address or a bare host. A text without a port takes
/// `default_port`.
///
/// # Errors
///
/// [`NetError::BadAddress`] if the host is empty, the port is not a number from 0 to 65535, or the text has no port and
/// `default_port` is `None`. Port 0 is for a listener that lets the system choose.
pub fn host_port(text: &str, default_port: Option<u16>) -> Result<(&str, u16), NetError> {
    let t = text.trim();
    let (host, port) = if let Some(rest) = t.strip_prefix('[') {
        let (h, tail) = rest.split_once(']').ok_or(NetError::BadAddress)?;
        match tail.strip_prefix(':') {
            Some(p) => (h, Some(p)),
            None if tail.is_empty() => (h, None),
            None => return Err(NetError::BadAddress),
        }
    } else if t.matches(':').count() > 1 {
        (t, None)
    } else if let Some((h, p)) = t.rsplit_once(':') {
        (h, Some(p))
    } else {
        (t, None)
    };
    if host.is_empty() {
        return Err(NetError::BadAddress);
    }
    let port = match port {
        None => default_port.ok_or(NetError::BadAddress)?,
        Some(p) => p.parse::<u16>().map_err(|_| NetError::BadAddress)?,
    };
    Ok((host, port))
}

/// True if `host` is this device (`localhost`, `::1`, `127.x.y.z`).
pub fn is_local(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// `host:port` for people, with brackets around an IPv6 host.
pub fn show(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// True if `e` is the end of a read timeout, not a failure.
pub fn timed_out(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_split_and_are_checked() {
        assert_eq!(host_port("10.0.0.2:1234", None), Ok(("10.0.0.2", 1234)));
        assert_eq!(host_port("[::1]:99", None), Ok(("::1", 99)));
        assert_eq!(
            host_port("radio.local", Some(1234)),
            Ok(("radio.local", 1234))
        );
        assert_eq!(host_port("::1", Some(7)), Ok(("::1", 7)));
        assert_eq!(host_port("radio.local", None), Err(NetError::BadAddress));
        assert_eq!(host_port(":1234", Some(1)), Err(NetError::BadAddress));
        assert_eq!(host_port("h:0", None), Ok(("h", 0)));
        assert_eq!(host_port("h:70000", Some(1)), Err(NetError::BadAddress));
        assert_eq!(host_port("h:x", Some(1)), Err(NetError::BadAddress));
        assert!(is_local("127.0.0.5") && is_local("::1") && is_local("LocalHost"));
        assert!(!is_local("192.168.1.2"));
        assert_eq!(show("::1", 5), "[::1]:5");
    }

    #[test]
    fn io_errors_map_to_their_kind() {
        let e = |k| NetError::from(io::Error::from(k));
        assert_eq!(e(io::ErrorKind::ConnectionRefused), NetError::Refused);
        assert_eq!(e(io::ErrorKind::WouldBlock), NetError::TimedOut);
        assert_eq!(e(io::ErrorKind::BrokenPipe), NetError::Closed);
        assert_eq!(
            e(io::ErrorKind::PermissionDenied),
            NetError::Io {
                kind: io::ErrorKind::PermissionDenied
            }
        );
        assert!(NetError::TimedOut.retryable() && !NetError::TlsUnavailable.retryable());
        let back: io::Error = NetError::Refused.into();
        assert_eq!(back.kind(), io::ErrorKind::ConnectionRefused);
    }
}
