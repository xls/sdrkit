//! Packet mode of one sub-channel: the shared assembler, with the packet-mode FEC when FIG 0/14 signals it.
//!
//! Standard: EN 300 401 clauses 5.3.2 to 5.3.5.

use sdrkit_proto_bcdata::packet::{DabAssembler, DabFec, Stats};

/// Counters of the packet decoder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacketStats {
    /// Packets with a good CRC.
    pub packets_ok: u64,
    /// Packets with a bad CRC.
    pub packets_bad: u64,
    /// Data groups out.
    pub groups: u64,
    /// Bytes corrected by the packet-mode FEC.
    pub fec_corrected: u64,
    /// FEC frames decoded.
    pub fec_frames: u64,
}

impl From<Stats> for PacketStats {
    fn from(s: Stats) -> Self {
        PacketStats {
            packets_ok: s.packets_ok,
            packets_bad: s.packets_bad,
            groups: s.groups,
            fec_corrected: s.fec_corrected,
            fec_frames: s.fec_frames,
        }
    }
}

enum Inner {
    Plain(Box<DabAssembler>),
    Fec(Box<DabFec>),
}

/// The packet decoder of one sub-channel.
pub struct PacketDecoder {
    inner: Inner,
}

impl PacketDecoder {
    /// A decoder, with the packet-mode FEC when `fec` (FEC scheme 1 in FIG 0/14).
    pub fn new(fec: bool) -> Self {
        PacketDecoder {
            inner: if fec {
                Inner::Fec(Box::default())
            } else {
                Inner::Plain(Box::default())
            },
        }
    }

    /// The counters.
    pub fn stats(&self) -> PacketStats {
        match &self.inner {
            Inner::Plain(a) => a.stats.into(),
            Inner::Fec(f) => f.stats().into(),
        }
    }

    /// Takes one logical frame. Each complete data group goes to `out` with its packet address.
    pub fn push(&mut self, lf: &[u8], out: &mut dyn FnMut(u16, &[u8])) {
        match &mut self.inner {
            Inner::Plain(a) => a.push(lf, out),
            Inner::Fec(f) => f.push(lf, out),
        }
    }
}
