//! The iiod text protocol, client side: one command per line (CR LF). Each answer starts with a decimal integer line, negative
//! for an errno, and some answers carry data after it.

use crate::net::{self, Abort};
use crate::{DriverError, Protocol};
use sdrkit_core::net::{Conn, NetError, Transport};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::Arc;
use std::time::Duration;

/// The iiod port.
pub const IIOD_PORT: u16 = 30431;

/// Why a command failed.
#[derive(Debug)]
pub enum IioError {
    /// The connection failed.
    Io(io::Error),
    /// The server answered with a negated errno.
    Errno(i32),
    /// The answer does not follow the protocol.
    Malformed,
}

impl std::fmt::Display for IioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IioError::Io(e) => write!(f, "{e}"),
            IioError::Errno(e) => write!(f, "{}", errno_text(*e)),
            IioError::Malformed => f.write_str("the answer does not follow the iiod protocol"),
        }
    }
}

impl From<IioError> for DriverError {
    fn from(e: IioError) -> Self {
        match e {
            IioError::Io(e) => DriverError::Net(NetError::from(e)),
            IioError::Errno(code) => DriverError::Device { code },
            IioError::Malformed => DriverError::Protocol(Protocol::Malformed),
        }
    }
}

impl From<io::Error> for IioError {
    fn from(e: io::Error) -> Self {
        IioError::Io(e)
    }
}

/// A short text for the errno values iiod answers with.
pub fn errno_text(e: i32) -> String {
    let t = match e {
        1 => "operation not permitted",
        2 => "no such attribute",
        5 => "input/output error",
        12 => "out of memory",
        16 => "device busy",
        19 => "no such device",
        22 => "invalid value",
        110 => "timed out",
        _ => return format!("error {e}"),
    };
    format!("{t} (errno {e})")
}

/// Where an attribute is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target<'a> {
    /// A device attribute.
    Device,
    /// A channel attribute: direction (true for output) and channel id.
    Channel(bool, &'a str),
    /// A debug attribute.
    Debug,
    /// A buffer attribute.
    Buffer,
}

impl Target<'_> {
    fn words(&self) -> String {
        match self {
            Target::Device => String::new(),
            Target::Channel(out, id) => format!(" {} {id}", if *out { "OUTPUT" } else { "INPUT" }),
            Target::Debug => " DEBUG".into(),
            Target::Buffer => " BUFFER".into(),
        }
    }
}

/// The channel mask as iiod writes it: 8-digit hexadecimal words, the most significant first.
pub fn mask_text(words: &[u32]) -> String {
    words.iter().rev().map(|w| format!("{w:08x}")).collect()
}

/// Parses a mask as iiod writes it (8-digit hexadecimal words, the most significant first) into words, the least significant
/// first.
pub fn parse_mask(s: &str) -> Option<Vec<u32>> {
    if s.is_empty() || !s.len().is_multiple_of(8) || !s.is_ascii() {
        return None;
    }
    (0..s.len() / 8)
        .rev()
        .map(|i| u32::from_str_radix(&s[i * 8..i * 8 + 8], 16).ok())
        .collect()
}

/// Splits a `VERSION` answer ("0.26.v0.26", "1.0.abc1234") into (major, minor).
pub fn parse_version(s: &str) -> Option<(u32, u32)> {
    let mut it = s.trim().split('.');
    Some((
        it.next()?.trim().parse().ok()?,
        it.next()?.trim().parse().ok()?,
    ))
}

/// One connection to iiod.
pub struct IiodConn {
    sock: Box<dyn Conn>,
    rd: BufReader<Box<dyn Conn>>,
}

impl IiodConn {
    /// Connects through `transport` within `timeout`. A fired `abort` cuts it short and later shuts the connection down.
    ///
    /// # Errors
    ///
    /// [`NetError`] if no connection is made.
    pub fn connect(
        transport: &Arc<dyn Transport>,
        host: &str,
        port: u16,
        timeout: Duration,
        abort: &Arc<Abort>,
    ) -> Result<IiodConn, NetError> {
        IiodConn::from_conn(net::connect(transport, host, port, false, timeout, abort)?)
    }

    /// Wraps a connection. A read timeout of 5 s guards against a silent server.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the connection cannot give a second handle.
    pub fn from_conn(mut sock: Box<dyn Conn>) -> Result<IiodConn, NetError> {
        sock.set_read_timeout(Some(Duration::from_secs(5)))?;
        let rd = BufReader::with_capacity(1 << 16, sock.try_clone()?);
        Ok(IiodConn { sock, rd })
    }

    /// Registers the connection with `abort`.
    pub fn register(&self, abort: &Abort) {
        abort.register(self.sock.as_ref());
    }

    /// Asks for the first receive buffer of `sizes` (largest first) that the system takes.
    pub fn recv_buffer_size(&mut self, sizes: &[usize]) {
        net::recv_buffer(self.sock.as_mut(), sizes);
    }

    /// Sets the read timeout on the client side.
    pub fn set_read_timeout(&mut self, d: Duration) {
        let _ = self.sock.set_read_timeout(Some(d));
    }

    fn send(&mut self, line: &str) -> io::Result<()> {
        let mut b = Vec::with_capacity(line.len() + 2);
        b.extend_from_slice(line.as_bytes());
        b.extend_from_slice(b"\r\n");
        self.sock.write_all(&b)
    }

    fn line(&mut self) -> io::Result<String> {
        let mut s = String::new();
        if self.rd.read_line(&mut s)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(s)
    }

    fn int(&mut self) -> Result<i64, IioError> {
        let l = self.line()?;
        l.trim().parse().map_err(|_| IioError::Malformed)
    }

    fn status(&mut self) -> Result<i64, IioError> {
        let v = self.int()?;
        if v < 0 {
            Err(IioError::Errno((-v) as i32))
        } else {
            Ok(v)
        }
    }

    /// Sends a command whose answer is one integer.
    pub fn command(&mut self, line: &str) -> Result<i64, IioError> {
        self.send(line)?;
        self.status()
    }

    /// `VERSION`: "MAJOR.MINOR.TAG".
    pub fn version(&mut self) -> Result<String, IioError> {
        self.send("VERSION")?;
        Ok(self.line()?.trim().to_string())
    }

    /// `TIMEOUT <ms>`: the server's timeout for this connection.
    pub fn timeout(&mut self, ms: u32) -> Result<(), IioError> {
        self.command(&format!("TIMEOUT {ms}")).map(|_| ())
    }

    /// `PRINT`: the context XML.
    pub fn print(&mut self) -> Result<String, IioError> {
        self.send("PRINT")?;
        let n = self.status()? as usize;
        let mut b = vec![0u8; n];
        self.rd.read_exact(&mut b)?;
        self.skip_lf()?;
        String::from_utf8(b).map_err(|_| IioError::Malformed)
    }

    fn skip_lf(&mut self) -> io::Result<()> {
        let mut lf = [0u8; 1];
        self.rd.read_exact(&mut lf)
    }

    /// Reads an attribute, without its trailing NUL and whitespace.
    pub fn read_attr(&mut self, dev: &str, at: Target, attr: &str) -> Result<String, IioError> {
        self.send(&format!("READ {dev}{} {attr}", at.words()))?;
        let n = self.status()? as usize;
        let mut b = vec![0u8; n];
        self.rd.read_exact(&mut b)?;
        self.skip_lf()?;
        let s = String::from_utf8_lossy(&b);
        Ok(s.trim_end_matches('\0').trim().to_string())
    }

    /// Writes an attribute. Returns the number of bytes the server accepted; a negative answer is [`IioError::Errno`].
    pub fn write_attr(
        &mut self,
        dev: &str,
        at: Target,
        attr: &str,
        value: &str,
    ) -> Result<i64, IioError> {
        let mut b = format!("WRITE {dev}{} {attr} {}\r\n", at.words(), value.len()).into_bytes();
        b.extend_from_slice(value.as_bytes());
        self.sock.write_all(&b)?;
        self.status()
    }

    /// `SET <dev> BUFFERS_COUNT <n>`.
    pub fn set_buffers_count(&mut self, dev: &str, n: u32) -> Result<(), IioError> {
        self.command(&format!("SET {dev} BUFFERS_COUNT {n}"))
            .map(|_| ())
    }

    /// `OPEN <dev> <samples> <mask> [CYCLIC]`.
    pub fn open_buffer(
        &mut self,
        dev: &str,
        samples: usize,
        mask: &[u32],
        cyclic: bool,
    ) -> Result<(), IioError> {
        let c = if cyclic { " CYCLIC" } else { "" };
        self.command(&format!("OPEN {dev} {samples} {}{c}", mask_text(mask)))
            .map(|_| ())
    }

    /// `READBUF <dev> <bytes>`: fills `out` (cleared first) with up to `bytes` bytes. See [`IiodConn::recv_buffer`].
    pub fn read_buffer(
        &mut self,
        dev: &str,
        bytes: usize,
        out: &mut Vec<u8>,
    ) -> Result<usize, IioError> {
        self.send_readbuf(dev, bytes)?;
        let (n, _) = self.recv_buffer(bytes, out)?;
        Ok(n)
    }

    /// Sends `READBUF <dev> <bytes>` without waiting for the answer, so the next request can be on its way while the last answer
    /// is converted.
    pub fn send_readbuf(&mut self, dev: &str, bytes: usize) -> Result<(), IioError> {
        Ok(self.send(&format!("READBUF {dev} {bytes}"))?)
    }

    /// Reads the answer to a `READBUF` of `bytes` bytes into `out`, straight from the socket (its length is kept at `bytes` or more
    /// between calls, so it is never zero-filled again). The server answers in chunks: a length, the mask line on the first chunk
    /// only, then the bytes, until the request is met or a chunk of 0 ends it early. Returns the bytes read and the mask the data
    /// carries (with another client streaming, the server may send more channels than were requested).
    pub fn recv_buffer(
        &mut self,
        bytes: usize,
        out: &mut Vec<u8>,
    ) -> Result<(usize, Option<Vec<u32>>), IioError> {
        if out.len() < bytes {
            out.resize(bytes, 0);
        }
        let mut got = 0;
        let mut mask = None;
        while got < bytes {
            let k = self.status()? as usize;
            if k == 0 {
                break;
            }
            if mask.is_none() {
                let line = self.line()?;
                mask = Some(parse_mask(line.trim()).ok_or(IioError::Malformed)?);
            }
            if got + k > bytes {
                return Err(IioError::Malformed);
            }
            self.rd.read_exact(&mut out[got..got + k])?;
            got += k;
        }
        Ok((got, mask))
    }

    /// `WRITEBUF <dev> <bytes>`, then the bytes (transmit). The server answers 0 (or an errno) before it takes the data, and the
    /// bytes it accepted after. Returns those.
    pub fn write_buffer(&mut self, dev: &str, data: &[u8]) -> Result<usize, IioError> {
        self.send(&format!("WRITEBUF {dev} {}", data.len()))?;
        self.status()?;
        self.sock.write_all(data)?;
        Ok(self.status()? as usize)
    }

    /// `BINARY`: requests that the server switch this connection to its binary protocol (libiio 1.x servers). True when it did; a
    /// server without it answers an errno and the connection stays on the text protocol.
    pub fn binary(&mut self) -> Result<bool, IioError> {
        match self.command("BINARY") {
            Ok(0) => Ok(true),
            Ok(_) | Err(IioError::Errno(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Writes raw bytes (the binary protocol).
    pub fn write_raw(&mut self, b: &[u8]) -> io::Result<()> {
        self.sock.write_all(b)
    }

    /// Reads exactly `b.len()` raw bytes (the binary protocol).
    pub fn read_raw(&mut self, b: &mut [u8]) -> io::Result<()> {
        self.rd.read_exact(b)
    }

    /// `CLOSE <dev>`: frees the buffer.
    pub fn close_buffer(&mut self, dev: &str) -> Result<(), IioError> {
        self.command(&format!("CLOSE {dev}")).map(|_| ())
    }

    /// Ends the connection without a word (a connection in binary mode, or one already broken).
    pub fn shutdown_quiet(&mut self) {
        self.sock.shutdown();
    }

    /// Ends the connection (`EXIT`, then shut down).
    pub fn shutdown(&mut self) {
        let _ = self.send("EXIT");
        self.sock.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks() {
        assert_eq!(mask_text(&[3]), "00000003");
        assert_eq!(mask_text(&[0xc]), "0000000c");
        assert_eq!(mask_text(&[1, 0x10]), "0000001000000001");
        assert_eq!(errno_text(22), "invalid value (errno 22)");
        assert_eq!(parse_mask("00000003"), Some(vec![3]));
        assert_eq!(parse_mask("0000001000000001"), Some(vec![1, 0x10]));
        assert_eq!(parse_mask("3"), None);
        assert_eq!(parse_version("0.26.v0.26  "), Some((0, 26)));
        assert_eq!(parse_version("1.0.abc"), Some((1, 0)));
    }
}
