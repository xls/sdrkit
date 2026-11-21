//! The transport stream item: how a transport stream moves between graph components. Each [`TsPacket`] holds one transport
//! packet. Every stack provides its transport stream as a stream of these items, and every consumer takes it so.
//!
//! Standard: ISO/IEC 13818-1 2.4.3.

use crate::packet::{SYNC, TS_SIZE};
use sdrkit_graph::{Item, Kind, Packet, PacketPool, Port, Timed};

/// The transport_error_indicator bit in byte 1 of a transport packet (ISO/IEC 13818-1 2.4.3.2).
pub const TEI: u8 = 0x80;

/// One transport packet: 188 bytes that start with the sync byte 0x47. The bytes are shared, so passing a packet on does not
/// copy them.
///
/// If the decoder before it cannot correct a packet, it sets the transport_error_indicator and still sends the packet. If
/// packets were lost, the first item after the loss has a `TagKind::Discontinuity` tag.
#[derive(Clone, Debug, PartialEq)]
pub struct TsPacket(Packet);

impl TsPacket {
    /// The packet that `p` holds. `None` if `p` does not hold exactly 188 bytes that start with the sync byte.
    pub fn new(p: Packet) -> Option<TsPacket> {
        (p.bytes().len() == TS_SIZE && p.bytes()[0] == SYNC).then_some(TsPacket(p))
    }

    /// The 188 bytes.
    pub fn bytes(&self) -> &[u8; TS_SIZE] {
        self.0
            .bytes()
            .try_into()
            .expect("a TsPacket holds 188 bytes")
    }

    /// Whether the transport_error_indicator is set.
    pub fn has_error(&self) -> bool {
        self.bytes()[1] & TEI != 0
    }

    /// The shared bytes as a packet.
    pub fn packet(&self) -> &Packet {
        &self.0
    }
}

impl Item for TsPacket {
    const KIND: &'static str = "mpeg.ts";
}

/// The stream kind of a transport stream.
pub fn kind() -> Kind {
    Kind::typed::<TsPacket>()
}

/// A port that carries a transport stream.
pub fn port(name: &str) -> Port {
    Port::typed::<TsPacket>(name)
}

/// Makes the items of a transport stream. The bytes come from a [`PacketPool`], so nothing is allocated once the pool has
/// grown.
///
/// ```
/// use sdrkit_proto_mpegts::stream::TsOut;
///
/// let mut out = TsOut::default();
/// let mut items = Vec::new();
/// let mut p = [0xFFu8; 188];
/// p[0] = 0x47;
/// assert!(out.push(&mut items, &p, 0.5, true));
/// assert!(items[0].value.has_error());
/// ```
#[derive(Debug, Default)]
pub struct TsOut {
    pool: PacketPool,
}

impl TsOut {
    /// Appends one transport packet at signal time `time`. If `uncorrectable` is true, the copy gets the
    /// transport_error_indicator. Returns false and appends nothing if `packet` does not start with the sync byte.
    pub fn push(
        &mut self,
        out: &mut Vec<Timed<TsPacket>>,
        packet: &[u8; TS_SIZE],
        time: f64,
        uncorrectable: bool,
    ) -> bool {
        if packet[0] != SYNC {
            return false;
        }
        let p = self.pool.make(TS_SIZE, time, |b| {
            b.copy_from_slice(packet);
            if uncorrectable {
                b[1] |= TEI;
            }
        });
        out.push(Timed {
            time,
            value: TsPacket(p),
        });
        true
    }

    /// Appends packets with their times (`times[i]` for `packets[i]`). Returns the number of packets that did not start with
    /// the sync byte and were not appended.
    pub fn extend(
        &mut self,
        out: &mut Vec<Timed<TsPacket>>,
        packets: &[[u8; TS_SIZE]],
        times: &[f64],
    ) -> usize {
        packets
            .iter()
            .zip(times)
            .filter(|(p, &t)| !self.push(out, p, t, false))
            .count()
    }
}
