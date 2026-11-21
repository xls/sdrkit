//! What the instrument decoders of `eodb` share: a space packet as they take it, the decoder interface, and the packets of a
//! test source.

use super::planes::Planes;

/// A space packet: its APID, sequence flags and count, and its data field (after the 6-byte primary header).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketIn<'a> {
    /// APID.
    pub apid: u16,
    /// Sequence flags: 1 first, 0 continuation, 2 last, 3 unsegmented.
    pub flags: u8,
    /// The 14-bit sequence count.
    pub seq: u16,
    /// The data field.
    pub data: &'a [u8],
}

/// An instrument decoder: space packets in, channel pictures out.
pub trait PacketInstrument: Send {
    /// Takes one packet.
    fn push(&mut self, p: &PacketIn<'_>);
    /// Ends the pass: finishes what is still open.
    fn finish(&mut self) {}
    /// The channel pictures.
    fn planes(&self) -> &Planes;
    /// The channel pictures, to take strips or start new pictures.
    fn planes_mut(&mut self) -> &mut Planes;
    /// Packets that went into a picture.
    fn used(&self) -> u64;
    /// Starts new pictures.
    fn clear(&mut self) {
        self.planes_mut().clear();
    }
}

/// One packet of a test source: APID, sequence flags and data field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestPacket {
    /// APID.
    pub apid: u16,
    /// Sequence flags.
    pub flags: u8,
    /// The data field.
    pub data: Vec<u8>,
}

/// A source of the test packets of an instrument, in sending order.
pub trait TestSource: Send {
    /// Appends the next packets (at least one).
    fn next(&mut self, out: &mut Vec<TestPacket>);
}
