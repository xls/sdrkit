//! The two forms of a stream path: UDP datagrams, or messages on a TCP connection, each framed by its 24-byte header. The
//! stream argument `remote:prot` chooses the form (`udp` is the default). For TCP the server listens, sends its port in an
//! early reply to the set-up call, takes the data connection first and the status connection second, and then sends the
//! usual reply. Both forms carry the same headers, acknowledgements and window.

use crate::rpc::{self, Packer};
use crate::stream::HEADER_LEN;
use core::net::{IpAddr, SocketAddr};
use sdrkit_core::net::{timed_out, Conn, Datagram, NetError, Transport};
use sdrkit_driver_core::net::Abort;
use std::io::{self, ErrorKind};
use std::sync::Arc;
use std::time::Duration;

/// The stream argument that chooses the transport.
pub const PROT_KEY: &str = "remote:prot";
/// How long one write on a TCP path waits before it checks for a stop.
pub const TCP_WRITE_SLICE: Duration = Duration::from_millis(100);
/// How long [`Wire::try_recv`] waits on a TCP path.
const TCP_POLL: Duration = Duration::from_micros(10);

/// One path of a stream (data or status).
pub enum Wire {
    /// Datagrams to `peer`; datagrams from another address are skipped.
    Udp {
        /// The socket.
        sock: Box<dyn Datagram>,
        /// Where datagrams go and come from.
        peer: SocketAddr,
    },
    /// Messages on a connection.
    Tcp(TcpPath),
}

/// A TCP path: the connection and the state of the message being read.
pub struct TcpPath {
    conn: Box<dyn Conn>,
    /// Bytes of the message being read that are already in the caller's buffer.
    have: usize,
    /// The read timeout on the connection.
    set: Option<Duration>,
    /// The read timeout that [`Wire::recv`] uses.
    wait: Option<Duration>,
    /// A write stopped inside a message: the framing is lost.
    cut: bool,
}

impl Wire {
    /// A UDP path on `sock` to `peer`.
    pub fn udp(sock: Box<dyn Datagram>, peer: SocketAddr) -> Wire {
        Wire::Udp { sock, peer }
    }

    /// A TCP path on `conn`. Its writes wait [`TCP_WRITE_SLICE`] at a time.
    pub fn tcp(mut conn: Box<dyn Conn>) -> Wire {
        let _ = conn.set_write_timeout(Some(TCP_WRITE_SLICE));
        Wire::Tcp(TcpPath {
            conn,
            have: 0,
            set: None,
            wait: None,
            cut: false,
        })
    }

    /// True for a TCP path.
    pub fn is_tcp(&self) -> bool {
        matches!(self, Wire::Tcp(_))
    }

    /// The local address of a UDP path.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        match self {
            Wire::Udp { sock, .. } => Some(sock.local_addr()),
            Wire::Tcp(_) => None,
        }
    }

    /// Sets how long [`Wire::recv`] waits (`None`: no limit).
    ///
    /// # Errors
    ///
    /// [`NetError`] if the system refuses the timeout.
    pub fn set_read_timeout(&mut self, t: Option<Duration>) -> Result<(), NetError> {
        match self {
            Wire::Udp { sock, .. } => sock.set_read_timeout(t),
            Wire::Tcp(p) => {
                p.wait = t;
                p.timeout(t)
            }
        }
    }

    /// Sends the whole message `msg`. On TCP a write that times out is tried again until `stop` says to stop; a message cut
    /// that way ends the path.
    ///
    /// # Errors
    ///
    /// The system's error; [`ErrorKind::TimedOut`] if `stop` cut the message; [`ErrorKind::BrokenPipe`] once a message was
    /// cut.
    pub fn send(&mut self, msg: &[u8], stop: &mut dyn FnMut() -> bool) -> io::Result<()> {
        match self {
            Wire::Udp { sock, peer } => sock.send_to(msg, *peer),
            Wire::Tcp(p) => p.write_all(msg, stop),
        }
    }

    /// Receives one message into `buf` within the read timeout and returns its length. `Ok(None)`: none within the time,
    /// or (UDP) a datagram from another address. On TCP a message that is not complete stays part-read for the next call,
    /// so the caller passes the same buffer until a message returns.
    ///
    /// # Errors
    ///
    /// The system's error. On TCP: [`ErrorKind::UnexpectedEof`] when the peer closed, [`ErrorKind::InvalidData`] for a
    /// length below the header or above `buf`.
    pub fn recv(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        match self {
            Wire::Udp { sock, peer } => match sock.recv_from(buf) {
                Ok((n, from)) if from.ip() == peer.ip() => Ok(Some(n)),
                Ok(_) => Ok(None),
                Err(e) if timed_out(&e) || e.kind() == ErrorKind::Interrupted => Ok(None),
                Err(e) => Err(e),
            },
            Wire::Tcp(p) => {
                if p.set != p.wait {
                    p.timeout(p.wait)?;
                }
                p.read_message(buf)
            }
        }
    }

    /// Like [`Wire::recv`], without waiting (TCP: a few microseconds; a millisecond on systems with a coarser timeout).
    ///
    /// # Errors
    ///
    /// As [`Wire::recv`].
    pub fn try_recv(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        match self {
            Wire::Udp { sock, peer } => loop {
                match sock.try_recv_from(buf)? {
                    Some((n, from)) if from.ip() == peer.ip() => return Ok(Some(n)),
                    Some(_) => {}
                    None => return Ok(None),
                }
            },
            Wire::Tcp(p) => {
                if p.set != Some(TCP_POLL) {
                    p.timeout(Some(TCP_POLL))?;
                }
                p.read_message(buf)
            }
        }
    }
}

impl TcpPath {
    fn timeout(&mut self, t: Option<Duration>) -> Result<(), NetError> {
        self.conn.set_read_timeout(t)?;
        self.set = t;
        Ok(())
    }

    fn read_message(&mut self, buf: &mut [u8]) -> io::Result<Option<usize>> {
        loop {
            let need = if self.have < HEADER_LEN {
                HEADER_LEN
            } else {
                let total = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
                if !(HEADER_LEN..=buf.len()).contains(&total) {
                    return Err(ErrorKind::InvalidData.into());
                }
                if self.have == total {
                    self.have = 0;
                    return Ok(Some(total));
                }
                total
            };
            if buf.len() < need {
                return Err(ErrorKind::InvalidData.into());
            }
            match self.conn.read(&mut buf[self.have..need]) {
                Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
                Ok(n) => self.have += n,
                Err(e) if timed_out(&e) || e.kind() == ErrorKind::Interrupted => return Ok(None),
                Err(e) => return Err(e),
            }
        }
    }

    fn write_all(&mut self, mut msg: &[u8], stop: &mut dyn FnMut() -> bool) -> io::Result<()> {
        if self.cut {
            return Err(ErrorKind::BrokenPipe.into());
        }
        let whole = msg.len();
        while !msg.is_empty() {
            match self.conn.write(msg) {
                Ok(0) => return Err(ErrorKind::WriteZero.into()),
                Ok(n) => msg = &msg[n..],
                Err(e) if timed_out(&e) || e.kind() == ErrorKind::Interrupted => {
                    if stop() {
                        self.cut = msg.len() != whole;
                        return Err(ErrorKind::TimedOut.into());
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

/// Sends the set-up call `req` of a TCP stream on `ctl` and makes its two connections: it reads the server's port from the
/// early reply, connects the data connection and then the status connection to `server` at that port (both registered
/// with `abort`), and reads the stream id from the usual reply. The data connection asks for a receive buffer of
/// `recv_buffer` bytes. It returns the stream id, the data path and the status path.
///
/// # Errors
///
/// [`rpc::RpcError`] of either reply (as a radio error of call 300), or the error of a connect.
pub(crate) fn setup_tcp(
    ctl: &mut rpc::Conn,
    req: &Packer,
    transport: &Arc<dyn Transport>,
    server: IpAddr,
    timeout: Duration,
    abort: &Abort,
    recv_buffer: Option<usize>,
) -> Result<(i32, Wire, Wire), sdrkit_driver_core::DriverError> {
    use sdrkit_driver_core::DriverError;
    let err = |e: rpc::RpcError| e.driver_error(rpc::call::SETUP_STREAM);
    let port = ctl.call(req, timeout, |u| u.string()).map_err(err)?;
    let port: u16 = port
        .trim()
        .parse()
        .map_err(|_| DriverError::driver(crate::SoapyError::StreamPort))?;
    let host = server.to_string();
    let open = || sdrkit_driver_core::net::connect(transport, &host, port, false, timeout, abort);
    let connected = open().and_then(|mut data| {
        if let Some(b) = recv_buffer {
            let _ = data.set_recv_buffer(b);
        }
        Ok((data, open()?))
    });
    let (data, status) = match connected {
        Ok(c) => c,
        Err(e) => {
            // The server waits for both connections and then answers: read that answer so the next call finds its own.
            let _ = ctl.next(timeout, |_| Ok(()));
            return Err(DriverError::Net(e));
        }
    };
    let id = ctl.next(timeout, |u| {
        let id = u.i32()?;
        let _ = u.string()?;
        Ok(id)
    });
    Ok((id.map_err(err)?, Wire::tcp(data), Wire::tcp(status)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{ack, DgHeader};
    use sdrkit_core::net::FakeTransport;
    use std::io::Write;

    fn pair() -> (Box<dyn Conn>, Wire) {
        let net = FakeTransport::new();
        let mut l = net.listen("127.0.0.1", 0).unwrap();
        let port = l.local_addr().port();
        let c = net
            .connect("127.0.0.1", port, false, Duration::from_secs(1))
            .unwrap();
        let mut w = Wire::tcp(l.accept().unwrap().unwrap());
        w.set_read_timeout(Some(Duration::from_millis(10))).unwrap();
        (c, w)
    }

    #[test]
    fn a_tcp_message_in_parts_arrives_whole_after_timeouts() {
        let (mut c, mut w) = pair();
        let h = DgHeader {
            bytes: (HEADER_LEN + 8) as u32,
            seq: 7,
            elems: 2,
            flags: 0,
            time: 0,
        };
        let mut msg = h.to_bytes().to_vec();
        msg.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let mut buf = [0u8; 64];
        for part in [&msg[..10], &msg[10..27], &msg[27..]] {
            assert_eq!(w.recv(&mut buf).unwrap(), None);
            c.write_all(part).unwrap();
        }
        c.write_all(&ack(9, 4)).unwrap();
        assert_eq!(w.recv(&mut buf).unwrap(), Some(msg.len()));
        assert_eq!(&buf[..msg.len()], &msg[..]);
        assert_eq!(w.try_recv(&mut buf).unwrap(), Some(HEADER_LEN));
        assert_eq!(DgHeader::parse(&buf).unwrap().seq, 9);
    }

    #[test]
    fn a_bad_length_and_a_close_end_a_tcp_path() {
        let (mut c, mut w) = pair();
        let mut buf = [0u8; 64];
        let mut big = ack(0, 1);
        big[..4].copy_from_slice(&100u32.to_be_bytes());
        c.write_all(&big).unwrap();
        let e = w.recv(&mut buf).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::InvalidData);
        let (c, mut w) = pair();
        drop(c);
        let e = w.recv(&mut buf).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::UnexpectedEof);
    }
}
