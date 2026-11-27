//! The iiod binary protocol of libiio 1.x servers, for the receive stream: 8-byte headers, and several buffer blocks in
//! flight at once.

use super::proto::{IioError, IiodConn};
use std::io;

/// Opcodes used here (libiio 1.0 numbering).
pub mod op {
    /// An answer.
    pub const RESPONSE: u8 = 0;
    /// Set the timeout.
    pub const TIMEOUT: u8 = 2;
    /// Open a buffer: code = buffer number, payload = the mask words.
    pub const OPEN_BUFFER: u8 = 17;
    /// Close a buffer: code = buffer number.
    pub const CLOSE_BUFFER: u8 = 18;
    /// Start a buffer streaming: code = buffer number.
    pub const ENABLE_BUFFER: u8 = 19;
    /// Stop a buffer: code = buffer number.
    pub const DISABLE_BUFFER: u8 = 20;
    /// Create a block: code = buffer | block << 16, payload = the size in bytes (u64).
    pub const CREATE_BLOCK: u8 = 21;
    /// Free a block.
    pub const FREE_BLOCK: u8 = 22;
    /// Queue a block: payload = the bytes used (u64), then (transmit only) the data. Receive: the answer carries the data.
    pub const TRANSFER_BLOCK: u8 = 23;
    /// Queue a block to repeat (transmit).
    pub const ENQUEUE_BLOCK_CYCLIC: u8 = 24;
    /// Ask again for a block whose dequeue failed.
    pub const RETRY_DEQUEUE_BLOCK: u8 = 25;
}

/// A message header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The client id: 0 for the connection's default, block number + 1 for a block.
    pub client: u16,
    /// The opcode ([`op`]).
    pub op: u8,
    /// The device number.
    pub dev: u8,
    /// A status, an argument, or a data length.
    pub code: i32,
}

impl Header {
    /// The 8 bytes on the wire.
    pub fn bytes(&self) -> [u8; 8] {
        let mut b = [0u8; 8];
        b[..2].copy_from_slice(&self.client.to_le_bytes());
        b[2] = self.op;
        b[3] = self.dev;
        b[4..].copy_from_slice(&self.code.to_le_bytes());
        b
    }

    /// From the 8 bytes on the wire.
    pub fn parse(b: &[u8; 8]) -> Header {
        Header {
            client: u16::from_le_bytes([b[0], b[1]]),
            op: b[2],
            dev: b[3],
            code: i32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        }
    }
}

fn bad(what: &str) -> IioError {
    IioError::Io(io::Error::new(io::ErrorKind::InvalidData, what.to_string()))
}

/// A receive buffer streamed over the binary protocol.
pub struct BinaryRx {
    conn: IiodConn,
    dev: u8,
    blocks: u16,
    block_bytes: usize,
    /// The mask the server enabled (it may differ from the requested mask).
    pub mask: Vec<u32>,
    open: bool,
}

impl BinaryRx {
    fn send(&mut self, h: Header, payload: &[u8]) -> Result<(), IioError> {
        let mut b = Vec::with_capacity(8 + payload.len());
        b.extend_from_slice(&h.bytes());
        b.extend_from_slice(payload);
        Ok(self.conn.write_raw(&b)?)
    }

    fn header(&mut self) -> Result<Header, IioError> {
        let mut b = [0u8; 8];
        self.conn.read_raw(&mut b)?;
        let h = Header::parse(&b);
        if h.op != op::RESPONSE {
            return Err(bad("iiod sent a command where an answer was expected"));
        }
        Ok(h)
    }

    /// Waits for the answer to the default client's command, skipping (and discarding the data of) block answers on the way.
    fn status(&mut self) -> Result<i32, IioError> {
        loop {
            let h = self.header()?;
            if h.client == 0 {
                return Ok(h.code);
            }
            if h.code > 0 {
                self.discard(h.code as usize)?;
            }
        }
    }

    fn discard(&mut self, mut n: usize) -> Result<(), IioError> {
        let mut scratch = [0u8; 4096];
        while n > 0 {
            let k = n.min(scratch.len());
            self.conn.read_raw(&mut scratch[..k])?;
            n -= k;
        }
        Ok(())
    }

    fn check(code: i32) -> Result<i32, IioError> {
        if code < 0 {
            Err(IioError::Errno(-code))
        } else {
            Ok(code)
        }
    }

    /// Opens buffer 0 of device number `dev` with the channels of `mask` (one bit per channel by position), creates `blocks` blocks of
    /// `block_bytes`, starts the buffer, and queues every block. The connection must already be in binary mode
    /// ([`IiodConn::binary`]). On failure the connection is closed: it cannot go back to the text protocol.
    pub fn open(
        conn: IiodConn,
        dev: u8,
        mask: &[u32],
        blocks: u16,
        block_bytes: usize,
    ) -> Result<BinaryRx, IioError> {
        let mut s = BinaryRx {
            conn,
            dev,
            blocks: blocks.max(1),
            block_bytes,
            mask: mask.to_vec(),
            open: false,
        };
        s.start(mask)?;
        Ok(s)
    }

    fn start(&mut self, mask: &[u32]) -> Result<(), IioError> {
        let words: Vec<u8> = mask.iter().flat_map(|w| w.to_le_bytes()).collect();
        let h = Header {
            client: 0,
            op: op::OPEN_BUFFER,
            dev: self.dev,
            code: 0,
        };
        self.send(h, &words)?;
        let n = Self::check(self.status()?)? as usize;
        if n > 0 {
            let mut back = vec![0u8; n];
            self.conn.read_raw(&mut back)?;
            self.mask = back
                .as_chunks::<4>()
                .0
                .iter()
                .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
                .collect();
        }
        self.open = true;
        for b in 0..self.blocks {
            let h = Header {
                client: b + 1,
                op: op::CREATE_BLOCK,
                dev: self.dev,
                code: (b as i32) << 16,
            };
            self.send(h, &(self.block_bytes as u64).to_le_bytes())?;
            let a = self.header()?;
            if a.client != b + 1 {
                return Err(bad("iiod answered another block"));
            }
            Self::check(a.code)?;
        }
        let h = Header {
            client: 0,
            op: op::ENABLE_BUFFER,
            dev: self.dev,
            code: 0,
        };
        self.send(h, &[])?;
        Self::check(self.status()?)?;
        for b in 0..self.blocks {
            self.enqueue(b)?;
        }
        Ok(())
    }

    fn enqueue(&mut self, b: u16) -> Result<(), IioError> {
        let h = Header {
            client: b + 1,
            op: op::TRANSFER_BLOCK,
            dev: self.dev,
            code: (b as i32) << 16,
        };
        self.send(h, &(self.block_bytes as u64).to_le_bytes())
    }

    /// Waits for the next filled block and reads its data into `out` (kept at the block size or more, never zero-filled again),
    /// then queues that block again at once, before the data is converted. Returns the bytes read. An errno is the server's: the
    /// block has been queued again, so the stream continues.
    pub fn recv(&mut self, out: &mut Vec<u8>) -> Result<usize, IioError> {
        let asked = std::time::Instant::now();
        let h = self.header()?;
        self.conn.answer_began(asked);
        if h.client == 0 || h.client > self.blocks {
            if h.code > 0 {
                self.discard(h.code as usize)?;
            }
            return Ok(0);
        }
        let b = h.client - 1;
        if h.code < 0 {
            // Low 16 bits clear: the enqueue failed (errno in the high half); else the dequeue failed and can be retried.
            let (retry, errno) = if h.code & 0xffff == 0 {
                (false, -(h.code >> 16))
            } else {
                (true, -h.code)
            };
            if retry {
                let r = Header {
                    client: b + 1,
                    op: op::RETRY_DEQUEUE_BLOCK,
                    dev: self.dev,
                    code: (b as i32) << 16,
                };
                self.send(r, &[])?;
            } else {
                self.enqueue(b)?;
            }
            return Err(IioError::Errno(errno));
        }
        let n = h.code as usize;
        if out.len() < n {
            out.resize(n, 0);
        }
        self.conn.read_raw(&mut out[..n])?;
        self.enqueue(b)?;
        Ok(n)
    }

    /// The connection, to register with an abort or tune.
    pub fn conn_mut(&mut self) -> &mut IiodConn {
        &mut self.conn
    }

    /// The connection.
    pub fn conn(&self) -> &IiodConn {
        &self.conn
    }

    /// Stops and closes the buffer (draining block answers on the way, within the connection's read timeout), then ends the
    /// connection. The server frees everything when the connection ends anyway; closing first lets a new buffer open at once.
    pub fn close(&mut self) {
        if self.open && !self.conn.torn() {
            self.open = false;
            for (o, wait) in [(op::DISABLE_BUFFER, true), (op::CLOSE_BUFFER, true)] {
                let h = Header {
                    client: 0,
                    op: o,
                    dev: self.dev,
                    code: 0,
                };
                if self.send(h, &[]).is_err() || (wait && self.status().is_err()) {
                    break;
                }
            }
        }
        self.conn.shutdown_quiet();
    }
}

impl Drop for BinaryRx {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_are_little_endian() {
        let h = Header {
            client: 0x0102,
            op: op::TRANSFER_BLOCK,
            dev: 4,
            code: -(5 << 16),
        };
        let b = h.bytes();
        assert_eq!(&b[..4], &[0x02, 0x01, 23, 4]);
        assert_eq!(Header::parse(&b), h);
    }
}
