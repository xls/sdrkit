//! The transport of the components of a session's graph (`net.sink`): a connection returns at once and connects on a
//! thread of its own, so that a session never waits for the network.

use crate::feed::lock;
use sdrkit_core::net::{Conn, Datagram, DatagramOptions, Listener, NetError, Transport};
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A transport whose [`Transport::connect`] does not wait. The other calls go to the transport it wraps.
pub(crate) struct GraphNet(pub(crate) Arc<dyn Transport>);

/// Where a connection of [`GraphNet`] is.
enum Link {
    /// The thread is connecting. The timeouts and the receive buffer to set once it is up.
    Connecting {
        read: Option<Duration>,
        write: Option<Duration>,
        recv_buffer: Option<usize>,
    },
    Up(Box<dyn Conn>),
    Failed(NetError),
}

/// A connection of [`GraphNet`]. Before the connection is made, a write is dropped and returns its full length, and a read
/// fails with [`io::ErrorKind::WouldBlock`]. After a failed connection, every call fails with the error of the connection.
struct Pending(Arc<Mutex<Link>>);

impl Transport for GraphNet {
    fn connect(
        &self,
        host: &str,
        port: u16,
        tls: bool,
        timeout: Duration,
    ) -> Result<Box<dyn Conn>, NetError> {
        let link = Arc::new(Mutex::new(Link::Connecting {
            read: None,
            write: None,
            recv_buffer: None,
        }));
        let (net, shared, host) = (Arc::clone(&self.0), Arc::clone(&link), host.to_owned());
        std::thread::Builder::new()
            .name("sdrkit-connect".into())
            .spawn(move || {
                let made = net.connect(&host, port, tls, timeout);
                let mut l = lock(&shared);
                *l = match (made, &*l) {
                    (
                        Ok(mut c),
                        Link::Connecting {
                            read,
                            write,
                            recv_buffer,
                        },
                    ) => {
                        let set = c
                            .set_read_timeout(*read)
                            .and_then(|()| c.set_write_timeout(*write))
                            .and_then(|()| recv_buffer.map_or(Ok(()), |n| c.set_recv_buffer(n)));
                        match set {
                            Ok(()) => Link::Up(c),
                            Err(e) => Link::Failed(e),
                        }
                    }
                    (Ok(c), _) => {
                        c.shutdown();
                        Link::Failed(NetError::Closed)
                    }
                    (Err(e), _) => Link::Failed(e),
                };
            })
            .map_err(|_| NetError::Io {
                kind: io::ErrorKind::OutOfMemory,
            })?;
        Ok(Box::new(Pending(link)))
    }

    fn datagram(&self, host: &str, port: u16) -> Result<Box<dyn Datagram>, NetError> {
        self.0.datagram(host, port)
    }

    fn bind(&self, options: &DatagramOptions) -> Result<Box<dyn Datagram>, NetError> {
        self.0.bind(options)
    }

    fn listen(&self, host: &str, port: u16) -> Result<Box<dyn Listener>, NetError> {
        self.0.listen(host, port)
    }

    fn resolve(&self, host: &str, port: u16) -> Result<SocketAddr, NetError> {
        self.0.resolve(host, port)
    }

    fn interfaces(&self) -> Vec<sdrkit_core::net::Interface> {
        self.0.interfaces()
    }
}

impl Read for Pending {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut *lock(&self.0) {
            Link::Up(c) => c.read(buf),
            Link::Connecting { .. } => Err(io::ErrorKind::WouldBlock.into()),
            Link::Failed(e) => Err((*e).into()),
        }
    }
}

impl Write for Pending {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut *lock(&self.0) {
            Link::Up(c) => c.write(buf),
            Link::Connecting { .. } => Ok(buf.len()),
            Link::Failed(e) => Err((*e).into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut *lock(&self.0) {
            Link::Up(c) => c.flush(),
            Link::Connecting { .. } => Ok(()),
            Link::Failed(e) => Err((*e).into()),
        }
    }
}

impl Conn for Pending {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        match &mut *lock(&self.0) {
            Link::Up(c) => c.set_read_timeout(timeout),
            Link::Connecting { read, .. } => {
                *read = timeout;
                Ok(())
            }
            Link::Failed(e) => Err(*e),
        }
    }

    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        match &mut *lock(&self.0) {
            Link::Up(c) => c.set_write_timeout(timeout),
            Link::Connecting { write, .. } => {
                *write = timeout;
                Ok(())
            }
            Link::Failed(e) => Err(*e),
        }
    }

    fn set_recv_buffer(&mut self, bytes: usize) -> Result<(), NetError> {
        match &mut *lock(&self.0) {
            Link::Up(c) => c.set_recv_buffer(bytes),
            Link::Connecting { recv_buffer, .. } => {
                *recv_buffer = Some(bytes);
                Ok(())
            }
            Link::Failed(e) => Err(*e),
        }
    }

    /// Returns a second handle once the connection is up. While it connects, fails with [`io::ErrorKind::WouldBlock`].
    fn try_clone(&self) -> Result<Box<dyn Conn>, NetError> {
        match &*lock(&self.0) {
            Link::Up(c) => c.try_clone(),
            Link::Connecting { .. } => Err(NetError::Io {
                kind: io::ErrorKind::WouldBlock,
            }),
            Link::Failed(e) => Err(*e),
        }
    }

    fn shutdown(&self) {
        let mut l = lock(&self.0);
        if let Link::Up(c) = &*l {
            c.shutdown();
        }
        *l = Link::Failed(NetError::Closed);
    }

    fn peer_addr(&self) -> Option<SocketAddr> {
        match &*lock(&self.0) {
            Link::Up(c) => c.peer_addr(),
            Link::Connecting { .. } | Link::Failed(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::net::FakeTransport;
    use std::time::Instant;

    #[test]
    fn a_connection_returns_at_once_and_sends_once_it_is_up() {
        let net = FakeTransport::new();
        let got = Arc::new(Mutex::new(Vec::new()));
        let into = Arc::clone(&got);
        net.serve("127.0.0.1", 7000, move |mut s| {
            let mut buf = [0u8; 64];
            while let Ok(n) = s.read(&mut buf) {
                if n == 0 {
                    break;
                }
                lock(&into).extend_from_slice(&buf[..n]);
            }
        });
        let g = GraphNet(Arc::new(net));
        let mut c = g
            .connect("127.0.0.1", 7000, false, Duration::from_secs(1))
            .unwrap();
        let t = Instant::now();
        while lock(&got).is_empty() && t.elapsed() < Duration::from_secs(5) {
            c.write_all(b"x").unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !lock(&got).is_empty(),
            "the server got the writes after the connection"
        );
    }

    #[test]
    fn a_failed_connection_fails_the_writes() {
        let g = GraphNet(Arc::new(FakeTransport::new()));
        let mut c = g
            .connect("127.0.0.1", 7001, false, Duration::from_millis(50))
            .unwrap();
        let t = Instant::now();
        let failed = loop {
            if c.write(b"x").is_err() {
                break true;
            }
            if t.elapsed() > Duration::from_secs(5) {
                break false;
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        assert!(failed);
    }
}
