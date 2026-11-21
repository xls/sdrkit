//! Mode adaptation of packetised streams: user packets out of BBFRAME data fields ([`UpDeframer`]), with normal mode, high
//! efficiency mode, ISSY and null packet deletion, and data fields from user packets of any length in normal or high
//! efficiency mode ([`TsFramer`]).
//!
//! Standard: ETSI EN 302 307-1 clause 5.1 and annex D, ETSI EN 302 755 clause 5.1.8.

use crate::bbheader::{BbHeader, Mode, StreamFormat, BBHEADER_LEN};
use crate::error::{Error, Result};
use crate::error::{Feature, Malformed};
use sdrkit_util_bits::crc::catalog::CRC8_DVB_S2;

/// The longest user packet that the deframer accepts, in bytes (UPL is 16 bits).
pub const MAX_UP: usize = 8192;

/// The 188 bytes of an MPEG-2 null packet (PID 0x1FFF, payload only, continuity counter 0).
pub const NULL_PACKET: [u8; 188] = {
    let mut p = [0xFFu8; 188];
    p[0] = 0x47;
    p[1] = 0x1F;
    p[2] = 0xFF;
    p[3] = 0x10;
    p
};

/// One user packet out of the deframer.
#[derive(Clone, Copy, Debug)]
pub struct UserPacket<'a> {
    /// The packet with its sync byte put back (for a stream whose SYNC is 0, the packet without a sync byte). For a transport
    /// stream with a wrong CRC-8, the transport_error_indicator (byte 1, bit 0x80) is set.
    pub bytes: &'a [u8],
    /// False when the CRC-8 of the packet was wrong. Always true in HEM, which sends no CRC-8.
    pub crc_ok: bool,
    /// The ISSY field that followed the packet in normal mode (2 or 3 bytes, see [`Issy`]).
    pub issy: Option<Issy>,
    /// True for a null packet that the deframer put back.
    pub null: bool,
}

/// An ISSY field (EN 302 307-1 table D.1, EN 302 755 annex C), decoded from its first bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issy {
    /// ISCRshort: the 15 least significant bits of the input stream clock reference.
    IscrShort(u16),
    /// ISCRlong: the 22 least significant bits of the input stream clock reference.
    IscrLong(u32),
    /// BUFS: the receiver buffer size, `value` in `unit` (0 bits, 1 kbit, 2 Mbit).
    Bufs {
        /// The unit code.
        unit: u8,
        /// The 10-bit value.
        value: u16,
    },
    /// BUFSTAT: the buffer status, `value` in `unit` (0 bits, 1 kbit, 2 Mbit).
    Bufstat {
        /// The unit code.
        unit: u8,
        /// The 10-bit value.
        value: u16,
    },
    /// A code that the standard reserves, or a T2-only code (TTO) that this decoder keeps raw.
    Other([u8; 3]),
}

impl Issy {
    /// Decodes the first bytes of an ISSY field. `b` holds 2 or 3 bytes.
    pub fn parse(b: &[u8]) -> Result<Issy> {
        if b.len() < 2 {
            return Err(Error::Short);
        }
        let b2 = b.get(2).copied().unwrap_or(0);
        Ok(if b[0] & 0x80 == 0 {
            Issy::IscrShort(u16::from_be_bytes([b[0], b[1]]))
        } else if b[0] & 0x40 == 0 {
            Issy::IscrLong(u32::from_be_bytes([0, b[0] & 0x3F, b[1], b2]))
        } else {
            let value = ((b[0] as u16 & 3) << 8) | b[1] as u16;
            let unit = (b[0] >> 2) & 3;
            match (b[0] >> 4) & 3 {
                0 => Issy::Bufs { unit, value },
                2 => Issy::Bufstat { unit, value },
                _ => Issy::Other([b[0], b[1], b2]),
            }
        })
    }
}

/// Counters of the deframer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeframerStats {
    /// User packets delivered (null packets put back not included).
    pub packets: u64,
    /// User packets whose CRC-8 was wrong.
    pub crc_errors: u64,
    /// Null packets put back.
    pub nulls: u64,
    /// Times the packet chain was lost and found again at SYNCD.
    pub resyncs: u64,
    /// Data fields refused (a header that does not fit the stream, or a short field).
    pub refused: u64,
}

/// User packets out of the data fields of one packetised stream (one ISI or PLP). It keeps the packet chain across data
/// fields, so a packet cut by a frame boundary comes out whole. It does not allocate after [`UpDeframer::new`].
#[derive(Clone, Debug)]
pub struct UpDeframer {
    cur: Vec<u8>,
    pending: Vec<u8>,
    out: Vec<u8>,
    has_pending: bool,
    synced: bool,
    /// The unit length and mode of the stream the chain belongs to.
    shape: (usize, bool),
    /// The counters.
    pub stats: DeframerStats,
}

impl Default for UpDeframer {
    fn default() -> Self {
        Self::new()
    }
}

impl UpDeframer {
    /// A deframer with its buffers made.
    pub fn new() -> UpDeframer {
        UpDeframer {
            cur: Vec::with_capacity(MAX_UP),
            pending: Vec::with_capacity(MAX_UP),
            out: Vec::with_capacity(MAX_UP + 1),
            has_pending: false,
            synced: false,
            shape: (0, false),
            stats: DeframerStats::default(),
        }
    }

    /// Breaks the packet chain, for example after a lost BBFRAME. The next data field starts again at its SYNCD.
    pub fn reset(&mut self) {
        self.cur.clear();
        self.pending.clear();
        self.has_pending = false;
        self.synced = false;
    }

    /// Takes one BBFRAME (header and data field, descrambled). Packets go to `out`.
    pub fn push_frame(&mut self, frame: &[u8], out: &mut impl FnMut(UserPacket<'_>)) -> Result<()> {
        let h = BbHeader::parse(frame)?;
        self.push(&h, frame.get(BBHEADER_LEN..).unwrap_or(&[]), out)
    }

    /// Takes the header and the data field of one BBFRAME. Only the first DFL / 8 bytes of `df` are read. Packets go to `out`.
    /// If the header does not describe a packetised stream, or the field is shorter than DFL, it returns an error and keeps
    /// its state.
    pub fn push(
        &mut self,
        h: &BbHeader,
        df: &[u8],
        out: &mut impl FnMut(UserPacket<'_>),
    ) -> Result<()> {
        let n = h.data_field_len()?;
        let Some(df) = df.get(..n) else {
            self.stats.refused += 1;
            return Err(Error::Short);
        };
        let layout = match Layout::of(h) {
            Ok(l) => l,
            Err(e) => {
                self.stats.refused += 1;
                return Err(e);
            }
        };
        if h.syncd != 0xFFFF && !h.syncd.is_multiple_of(8) {
            self.stats.refused += 1;
            return Err(Error::Invalid(Malformed::SyncdNotWholeBytes));
        }
        let mut i = 0;
        if self.synced && self.shape != (layout.unit, layout.hem) {
            // The stream changed (UPL, ISSY, NPD or mode): the bytes kept belong to the old one.
            self.stats.resyncs += 1;
            self.reset();
        }
        self.shape = (layout.unit, layout.hem);
        if self.synced && h.syncd != 0xFFFF {
            // The next unit must start where SYNCD says. If it does not, the chain is broken.
            let expect = (layout.unit - self.cur.len()) % layout.unit;
            if expect != h.syncd as usize / 8 {
                self.stats.resyncs += 1;
                self.reset();
            }
        }
        if !self.synced {
            if h.syncd == 0xFFFF {
                return Ok(());
            }
            i = h.syncd as usize / 8;
            if i > df.len() {
                self.stats.refused += 1;
                return Err(Error::Invalid(Malformed::SyncdPastDataField));
            }
            self.synced = true;
        }
        let mut rest = &df[i..];
        while !rest.is_empty() {
            let take = (layout.unit - self.cur.len()).min(rest.len());
            self.cur.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.has_pending && !layout.hem {
                // The CRC-8 of the pending unit is the first byte of this one.
                self.check_pending(&layout, h, out);
            }
            if self.cur.len() == layout.unit {
                self.unit_done(&layout, h, out);
            }
        }
        Ok(())
    }

    fn unit_done(&mut self, l: &Layout, h: &BbHeader, out: &mut impl FnMut(UserPacket<'_>)) {
        if l.hem {
            // HEM: [187 bytes][DNP]. No CRC.
            let body = std::mem::take(&mut self.cur);
            self.emit(l, h, &body, true, out);
            self.cur = body;
            self.cur.clear();
            return;
        }
        // Normal mode: [CRC-8 of the previous unit][the rest]. The CRC-8 of this unit is the first byte of the next
        // unit, so the rest waits in `pending` until that byte arrives (see `check_pending`).
        self.pending.clear();
        self.pending.extend_from_slice(&self.cur[1..]);
        self.has_pending = true;
        self.cur.clear();
    }

    fn check_pending(&mut self, l: &Layout, h: &BbHeader, out: &mut impl FnMut(UserPacket<'_>)) {
        let ok = CRC8_DVB_S2.checksum(&self.pending) == self.cur[0];
        let body = std::mem::take(&mut self.pending);
        self.emit(l, h, &body, ok, out);
        self.pending = body;
        self.pending.clear();
        self.has_pending = false;
    }

    /// `body` is the unit after the sync slot: the packet without its sync byte, then ISSY and DNP.
    fn emit(
        &mut self,
        l: &Layout,
        h: &BbHeader,
        body: &[u8],
        ok: bool,
        out: &mut impl FnMut(UserPacket<'_>),
    ) {
        let plen = body.len() - l.issy - l.dnp;
        if l.dnp == 1 {
            let n = body[body.len() - 1];
            for _ in 0..n {
                self.stats.nulls += 1;
                out(UserPacket {
                    bytes: &NULL_PACKET,
                    crc_ok: true,
                    issy: None,
                    null: true,
                });
            }
        }
        let issy = (l.issy > 0)
            .then(|| Issy::parse(&body[plen..plen + l.issy]).ok())
            .flatten();
        self.out.clear();
        if l.sync != 0 || l.hem {
            self.out.push(if l.hem { 0x47 } else { l.sync });
        }
        self.out.extend_from_slice(&body[..plen]);
        if !ok {
            self.stats.crc_errors += 1;
            if h.format == StreamFormat::Transport && self.out.len() > 1 {
                self.out[1] |= 0x80;
            }
        }
        self.stats.packets += 1;
        out(UserPacket {
            bytes: &self.out,
            crc_ok: ok,
            issy,
            null: false,
        });
    }
}

/// How the units of a stream are laid out.
struct Layout {
    /// Bytes per transmitted unit.
    unit: usize,
    issy: usize,
    dnp: usize,
    sync: u8,
    hem: bool,
}

impl Layout {
    fn of(h: &BbHeader) -> Result<Layout> {
        let dnp = (h.npd && h.format == StreamFormat::Transport) as usize;
        match (h.mode, h.format) {
            (Mode::HighEfficiency, StreamFormat::Transport) => Ok(Layout {
                unit: 187 + dnp,
                issy: 0,
                dnp,
                sync: 0x47,
                hem: true,
            }),
            (Mode::HighEfficiency, _) => Err(Error::Unsupported(Feature::HemGenericStream)),
            (Mode::Normal, StreamFormat::Transport | StreamFormat::GenericPacketised) => {
                if h.upl == 0 || !h.upl.is_multiple_of(8) || h.upl as usize / 8 > MAX_UP {
                    return Err(Error::Invalid(Malformed::Upl));
                }
                let unit = h.upl as usize / 8;
                let issy = if !h.issyi {
                    0
                } else if h.format == StreamFormat::Transport {
                    // UPL = (188 + ISSY + DNP) x 8 (EN 302 307-1 clause D.3), so the ISSY length is 2 or 3.
                    unit.checked_sub(188 + dnp)
                        .filter(|&n| n == 2 || n == 3)
                        .ok_or(Error::Invalid(Malformed::UplWithIssy))?
                } else {
                    // Generic packetised streams use only ISCRshort (EN 302 307-1 table D.1 note).
                    2
                };
                if unit < 2 + issy + dnp {
                    return Err(Error::Invalid(Malformed::Upl));
                }
                Ok(Layout {
                    unit,
                    issy,
                    dnp,
                    sync: h.sync,
                    hem: false,
                })
            }
            _ => Err(Error::Invalid(Malformed::TsGs)),
        }
    }
}

/// The transmit side of one packetised stream (transport stream or GFPS) without ISSY or null packet deletion: it fills data
/// fields with user packets of a fixed length and returns SYNCD (EN 302 307-1 clause 5.1, table 4; EN 302 755 clause 5.1.6).
/// In normal mode the sync byte of each packet is replaced by the CRC-8 of the packet before it (0 before the first). In high
/// efficiency mode the sync byte is left out and no CRC-8 is sent. Packets are cut across data fields. It does not allocate
/// after it is made.
#[derive(Clone, Debug)]
pub struct TsFramer {
    unit: Vec<u8>,
    used: usize,
    last_crc: u8,
    hem: bool,
}

impl Default for TsFramer {
    /// The framer of 188-byte transport packets in normal mode.
    fn default() -> Self {
        TsFramer::normal(188)
    }
}

impl TsFramer {
    /// A framer of `upl` byte user packets in normal mode (UPL = 8 `upl`). `upl` is clamped to 2 ..= [`MAX_UP`].
    pub fn normal(upl: usize) -> TsFramer {
        let n = upl.clamp(2, MAX_UP);
        TsFramer {
            unit: vec![0; n],
            used: n,
            last_crc: 0,
            hem: false,
        }
    }

    /// A framer of `upl` byte user packets in high efficiency mode: each unit is the packet without its sync byte. `upl` is
    /// clamped to 2 ..= [`MAX_UP`].
    pub fn high_efficiency(upl: usize) -> TsFramer {
        let n = upl.clamp(2, MAX_UP) - 1;
        TsFramer {
            unit: vec![0; n],
            used: n,
            last_crc: 0,
            hem: true,
        }
    }

    /// The length of the user packets.
    pub fn upl(&self) -> usize {
        self.unit.len() + usize::from(self.hem)
    }

    /// Fills one data field from `next`, which returns the next user packet, sync byte first, of [`TsFramer::upl`] bytes (a
    /// shorter packet is padded with zeros, a longer one cut). Returns SYNCD in bits (65535 when no packet starts in the
    /// field).
    pub fn fill<P: AsRef<[u8]>>(&mut self, df: &mut [u8], next: &mut impl FnMut() -> P) -> u16 {
        let n = self.unit.len();
        let mut i = 0;
        let mut syncd = None;
        while i < df.len() {
            if self.used == n {
                syncd.get_or_insert(i);
                let p = next();
                let body = p.as_ref().get(1..).unwrap_or(&[]);
                let at = usize::from(!self.hem);
                let take = body.len().min(n - at);
                self.unit[at..at + take].copy_from_slice(&body[..take]);
                self.unit[at + take..].fill(0);
                if !self.hem {
                    self.unit[0] = self.last_crc;
                    self.last_crc = CRC8_DVB_S2.checksum(&self.unit[1..]);
                }
                self.used = 0;
            }
            let take = (n - self.used).min(df.len() - i);
            df[i..i + take].copy_from_slice(&self.unit[self.used..self.used + take]);
            self.used += take;
            i += take;
        }
        match syncd {
            Some(s) if s * 8 < 0xFFFF => (s * 8) as u16,
            _ => 0xFFFF,
        }
    }
}
