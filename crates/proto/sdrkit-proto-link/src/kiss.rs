//! KISS: the byte protocol between a host and a packet TNC (terminal node controller), in both directions.
//!
//! Standard: The KISS TNC, M. Chepponis and P. Karn (1987)

use crate::Param;
use crate::{Error, Result};
use sdrkit_util_bits::hdlc::{ByteStuffing, Unstuffer};

/// Frame end.
pub const FEND: u8 = 0xC0;
/// Frame escape.
pub const FESC: u8 = 0xDB;
/// Transposed frame end (after FESC).
pub const TFEND: u8 = 0xDC;
/// Transposed frame escape (after FESC).
pub const TFESC: u8 = 0xDD;

/// A KISS command (the low nibble of the type byte).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    /// 0: the data is a frame to send or a frame received.
    Data,
    /// 1: the keyup delay, in units of 10 ms.
    TxDelay,
    /// 2: the persistence value p for CSMA (p = (value + 1) / 256).
    Persistence,
    /// 3: the slot interval, in units of 10 ms.
    SlotTime,
    /// 4: the time to hold the transmitter after the frame, in units of 10 ms (obsolete).
    TxTail,
    /// 5: 0 is half duplex, any other value is full duplex.
    FullDuplex,
    /// 6: a hardware-specific command.
    SetHardware,
    /// 0xFF (with no port): leave KISS mode.
    Return,
    /// Any other command nibble (7 to 15, used by protocol extensions).
    Other(u8),
}

impl Command {
    fn nibble(self) -> u8 {
        match self {
            Command::Data => 0,
            Command::TxDelay => 1,
            Command::Persistence => 2,
            Command::SlotTime => 3,
            Command::TxTail => 4,
            Command::FullDuplex => 5,
            Command::SetHardware => 6,
            Command::Return => 0x0F,
            Command::Other(n) => n & 0x0F,
        }
    }

    fn from_type(t: u8) -> Command {
        if t == 0xFF {
            return Command::Return;
        }
        match t & 0x0F {
            0 => Command::Data,
            1 => Command::TxDelay,
            2 => Command::Persistence,
            3 => Command::SlotTime,
            4 => Command::TxTail,
            5 => Command::FullDuplex,
            6 => Command::SetHardware,
            n => Command::Other(n),
        }
    }
}

/// Appends one KISS frame to `out`. The port must be 0 to 15. For [`Command::Return`] the type byte is 0xFF and the port is
/// not used.
///
/// ```
/// use sdrkit_proto_link::kiss::{self, Command, Decoder};
/// let mut out = Vec::new();
/// kiss::encode(0, Command::Data, b"TEST", &mut out).unwrap();
/// assert_eq!(out, [0xC0, 0x00, b'T', b'E', b'S', b'T', 0xC0]);
/// let mut d = Decoder::new(400);
/// let mut got = Vec::new();
/// d.push_bytes(&out, |f| got.push((f.port, f.command, f.data.to_vec())));
/// assert_eq!(got, [(0, Command::Data, b"TEST".to_vec())]);
/// ```
pub fn encode(port: u8, command: Command, data: &[u8], out: &mut Vec<u8>) -> Result<()> {
    if port > 15 {
        return Err(Error::BadParameter(Param::KissPort));
    }
    let t = match command {
        Command::Return => 0xFF,
        c => (port << 4) | c.nibble(),
    };
    out.push(FEND);
    ByteStuffing::KISS.escape(&[t], out);
    ByteStuffing::KISS.escape(data, out);
    out.push(FEND);
    Ok(())
}

/// One frame from a KISS byte stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame<'a> {
    /// The port (0 for [`Command::Return`]).
    pub port: u8,
    /// The command.
    pub command: Command,
    /// The data after the type byte, unescaped.
    pub data: &'a [u8],
}

/// A streaming KISS decoder. Feed it bytes in pieces of any size. It does not allocate after [`Decoder::new`]. Empty frames
/// (two FENDs in a row) are skipped, and a frame longer than the limit is dropped.
#[derive(Clone, Debug)]
pub struct Decoder {
    u: Unstuffer,
}

impl Decoder {
    /// A decoder that keeps frames of up to `max` bytes (type byte included).
    pub fn new(max: usize) -> Self {
        Decoder {
            u: Unstuffer::new(ByteStuffing::KISS, max),
        }
    }

    /// Takes one byte. Returns a frame when the byte ends one.
    pub fn push(&mut self, b: u8) -> Option<Frame<'_>> {
        self.u.push(b)?;
        let f = self.u.frame();
        let (&t, data) = f.split_first()?;
        Some(Frame {
            port: if t == 0xFF { 0 } else { t >> 4 },
            command: Command::from_type(t),
            data,
        })
    }

    /// Takes bytes and calls `on_frame` for each frame that ends in them.
    pub fn push_bytes(&mut self, bytes: &[u8], mut on_frame: impl FnMut(Frame<'_>)) {
        for &b in bytes {
            if let Some(f) = self.push(b) {
                on_frame(f);
            }
        }
    }
}
