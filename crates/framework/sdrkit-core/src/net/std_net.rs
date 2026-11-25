//! The [`Transport`] of the operating system, through `std::net`. Plain TCP only: it refuses TLS.

use super::{Conn, Datagram, DatagramOptions, Listener, NetError, Transport};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::Duration;

/// The operating system's network, plain TCP and UDP. A host turns on the `std-net` feature to use it.
#[derive(Clone, Copy, Debug, Default)]
pub struct StdNet;

fn addresses(host: &str, port: u16) -> Result<Vec<SocketAddr>, NetError> {
    let h = host.trim_start_matches('[').trim_end_matches(']');
    let v: Vec<SocketAddr> = (h, port)
        .to_socket_addrs()
        .map_err(|_| NetError::NoSuchHost)?
        .collect();
    if v.is_empty() {
        return Err(NetError::NoSuchHost);
    }
    Ok(v)
}

fn no_sigpipe(s: socket2::SockRef<'_>) {
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos"
    ))]
    let _ = s.set_nosigpipe(true);
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos"
    )))]
    let _ = s;
}

struct Tcp(TcpStream);

impl Read for Tcp {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for Tcp {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl Conn for Tcp {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        Ok(self.0.set_read_timeout(timeout)?)
    }
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        Ok(self.0.set_write_timeout(timeout)?)
    }
    fn set_recv_buffer(&mut self, bytes: usize) -> Result<(), NetError> {
        Ok(socket2::SockRef::from(&self.0).set_recv_buffer_size(bytes)?)
    }
    fn try_clone(&self) -> Result<Box<dyn Conn>, NetError> {
        Ok(Box::new(Tcp(self.0.try_clone()?)))
    }
    fn shutdown(&self) {
        let _ = self.0.shutdown(Shutdown::Both);
    }
    fn peer_addr(&self) -> Option<SocketAddr> {
        self.0.peer_addr().ok()
    }
}

fn open(s: TcpStream, timeout: Duration) -> Box<dyn Conn> {
    let _ = s.set_nodelay(true);
    let _ = s.set_write_timeout(Some(timeout));
    no_sigpipe(socket2::SockRef::from(&s));
    Box::new(Tcp(s))
}

struct Udp {
    socket: UdpSocket,
    peer: Option<SocketAddr>,
}

impl Datagram for Udp {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        let to = self.peer.ok_or(io::ErrorKind::NotConnected)?;
        self.socket.send_to(bytes, to).map(|_| ())
    }
    fn send_to(&mut self, bytes: &[u8], to: SocketAddr) -> io::Result<()> {
        self.socket.send_to(bytes, to).map(|_| ())
    }
    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf)
    }
    fn try_recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        self.socket.set_nonblocking(true)?;
        let r = self.socket.recv_from(buf);
        self.socket.set_nonblocking(false)?;
        match r {
            Ok(x) => Ok(Some(x)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn recv_buffer(&self) -> Option<usize> {
        socket2::SockRef::from(&self.socket).recv_buffer_size().ok()
    }
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        Ok(self.socket.set_read_timeout(timeout)?)
    }
    fn local_addr(&self) -> SocketAddr {
        self.socket
            .local_addr()
            .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)))
    }
    fn try_clone(&self) -> Result<Box<dyn Datagram>, NetError> {
        Ok(Box::new(Udp {
            socket: self.socket.try_clone()?,
            peer: self.peer,
        }))
    }
}

struct TcpServer(TcpListener);

impl Listener for TcpServer {
    fn local_addr(&self) -> SocketAddr {
        self.0
            .local_addr()
            .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 0)))
    }
    fn accept(&mut self) -> Result<Option<Box<dyn Conn>>, NetError> {
        match self.0.accept() {
            Ok((s, _)) => {
                s.set_nonblocking(false)?;
                Ok(Some(open(s, Duration::from_millis(200))))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

impl Transport for StdNet {
    fn connect(
        &self,
        host: &str,
        port: u16,
        tls: bool,
        timeout: Duration,
    ) -> Result<Box<dyn Conn>, NetError> {
        if tls {
            return Err(NetError::TlsUnavailable);
        }
        let deadline = std::time::Instant::now() + timeout;
        let mut last = NetError::TimedOut;
        for a in addresses(host, port)? {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                break;
            }
            match TcpStream::connect_timeout(&a, left) {
                Ok(s) => return Ok(open(s, timeout)),
                Err(e) => last = e.into(),
            }
        }
        Err(last)
    }

    fn datagram(&self, host: &str, port: u16) -> Result<Box<dyn Datagram>, NetError> {
        let peer = self.resolve(host, port)?;
        let local = if peer.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        Ok(Box::new(Udp {
            socket: UdpSocket::bind(local)?,
            peer: Some(peer),
        }))
    }

    fn bind(&self, options: &DatagramOptions) -> Result<Box<dyn Datagram>, NetError> {
        use socket2::{Domain, Protocol, Socket, Type};
        let v4 = options.local.is_ipv4();
        let domain = if v4 { Domain::IPV4 } else { Domain::IPV6 };
        let s = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
        if let Some(n) = options.recv_buffer {
            let _ = s.set_recv_buffer_size(n);
        }
        if options.broadcast {
            s.set_broadcast(true)?;
        }
        if let Some(ttl) = options.multicast_ttl {
            if v4 {
                s.set_multicast_ttl_v4(ttl)?;
            } else {
                s.set_multicast_hops_v6(ttl)?;
            }
        }
        if let Some(i) = &options.multicast_interface {
            if v4 {
                s.set_multicast_if_v4(&i.v4.map_or(std::net::Ipv4Addr::UNSPECIFIED, |n| n.addr))?;
            } else {
                s.set_multicast_if_v6(i.index)?;
            }
        }
        if options.reuse_address {
            s.set_reuse_address(true)?;
            // The systems of the BSD family share a port for multicast only with this option too.
            #[cfg(unix)]
            let _ = s.set_reuse_port(true);
        }
        s.bind(&options.local.into())?;
        for group in &options.join {
            match group {
                IpAddr::V4(g) => {
                    let on = options
                        .multicast_interface
                        .as_ref()
                        .and_then(|i| i.v4)
                        .map_or(std::net::Ipv4Addr::UNSPECIFIED, |n| n.addr);
                    s.join_multicast_v4(g, &on)?;
                }
                IpAddr::V6(g) => {
                    let on = options.multicast_interface.as_ref().map_or(0, |i| i.index);
                    s.join_multicast_v6(g, on)?;
                }
            }
        }
        Ok(Box::new(Udp {
            socket: s.into(),
            peer: None,
        }))
    }

    fn listen(&self, host: &str, port: u16) -> Result<Box<dyn Listener>, NetError> {
        let listener = TcpListener::bind(self.resolve(host, port)?)?;
        listener.set_nonblocking(true)?;
        Ok(Box::new(TcpServer(listener)))
    }

    fn resolve(&self, host: &str, port: u16) -> Result<SocketAddr, NetError> {
        Ok(addresses(host, port)?[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_is_refused_not_downgraded() {
        let e = StdNet
            .connect("127.0.0.1", 9, true, Duration::from_millis(100))
            .err();
        assert_eq!(e, Some(NetError::TlsUnavailable));
    }

    #[test]
    fn a_refused_connect_says_so() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let e = StdNet
            .connect("127.0.0.1", port, false, Duration::from_secs(2))
            .err();
        assert_eq!(e, Some(NetError::Refused));
    }
}
