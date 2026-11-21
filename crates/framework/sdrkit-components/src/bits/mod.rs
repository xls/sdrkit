//! `bits.*` and `bytes.*`: the components over `sdrkit-util-bits` (line codes, descramblers, sync words, deinterleavers,
//! packing and unpacking, HDLC and CRC checks). Packing and unpacking are also adapters of the graph: bits to bytes in a
//! declared bit order, and back. A component that works on hard bits and on LLRs is registered twice: `bits.<name>` takes hard
//! bits and `bits.<name>_llr` takes LLRs.

mod bench;
mod convert;
mod deinterleave;
mod descramble;
mod frame;
mod line;
mod sync;

pub use convert::SoftFromHard;
pub use deinterleave::{BitInterleaver, Deinterleaver, DeinterleaverSetting};
pub use descramble::{
    ByteDescrambler, ByteDescramblerSetting, Descrambler, DescramblerSetting, RegisterForm,
    SelfSyncDescrambler, SelfSyncDescramblerSetting, SelfSyncScrambler, SequenceOutput,
    BIT_SEQUENCES, BYTE_SEQUENCES,
};
pub use frame::{
    crc_key, BitOrderChoice, CrcAction, CrcCheck, CrcCheckSetting, CrcStatus, CrcTrailer,
    HdlcFramer, HdlcFramerSetting, NoBitOrder, Packer, PackerSetting, Unpacker,
};
pub use line::{
    LineDecoder, LineDecoderSetting, ManchesterConvention, ManchesterDecoder,
    ManchesterDecoderSetting, PAIRS_BAD,
};
pub use sync::{SyncSearch, SyncSearchSetting, SYNC_WORDS};

use sdrkit_core::Benchmark;
use sdrkit_graph::{Adapter, AdapterClass, Io, Kind, Port, Registry};
use sdrkit_util_bits::line::{Manchester, ManchesterConv, NrzM, Nrzi};

/// Adds the `bits.*` and `bytes.*` components.
pub fn register(r: &mut Registry) {
    r.add("bits.nrzi", || Box::new(LineDecoder::<u8>::nrzi()));
    r.add("bits.nrzi_llr", || Box::new(LineDecoder::<i8>::nrzi()));
    r.add("bits.nrzm", || Box::new(LineDecoder::<u8>::nrzm()));
    r.add("bits.nrzm_llr", || Box::new(LineDecoder::<i8>::nrzm()));
    r.add("bits.diff", || Box::new(LineDecoder::<u8>::diff()));
    r.add("bits.diff_llr", || Box::new(LineDecoder::<i8>::diff()));
    r.add("bits.manchester", || {
        Box::new(ManchesterDecoder::<u8>::default())
    });
    r.add("bits.manchester_llr", || {
        Box::new(ManchesterDecoder::<i8>::default())
    });
    r.add("bits.descramble", || Box::new(Descrambler::<u8>::default()));
    r.add("bits.descramble_llr", || {
        Box::new(Descrambler::<i8>::default())
    });
    r.add("bits.self_sync", || {
        Box::new(SelfSyncDescrambler::<u8>::default())
    });
    r.add("bits.self_sync_llr", || {
        Box::new(SelfSyncDescrambler::<i8>::default())
    });
    r.add("bits.sync", || Box::new(SyncSearch::<u8>::default()));
    r.add("bits.sync_llr", || Box::new(SyncSearch::<i8>::default()));
    r.add("bits.deinterleave", || {
        Box::new(Deinterleaver::<u8>::default())
    });
    r.add("bits.deinterleave_llr", || {
        Box::new(Deinterleaver::<i8>::default())
    });
    r.add("bits.pack", || Box::new(Packer::default()));
    r.add("bits.unpack", || Box::new(Unpacker::default()));
    r.add_adapter(Adapter {
        kind: "bits.pack",
        from: Kind::HardBits,
        to: Kind::Bytes,
        class: AdapterClass::Lossless,
        cost: 1,
        settings: frame::pack_step,
        make: || Box::new(Packer::default()),
    });
    r.add_adapter(Adapter {
        kind: "bits.unpack",
        from: Kind::Bytes,
        to: Kind::HardBits,
        class: AdapterClass::Lossless,
        cost: 1,
        settings: |f, _| f.bit_order.map(|_| sdrkit_graph::no_values()),
        make: || Box::new(Unpacker::default()),
    });
    r.add("bits.hdlc", || Box::new(HdlcFramer::default()));
    r.add("bytes.crc", || Box::new(CrcCheck::default()));
    r.add("bytes.descramble", || Box::new(ByteDescrambler::default()));
    r.add("bits.soft", || Box::new(SoftFromHard));
}

/// The benchmarks of the bit-level hot paths: CRC-32 and CRC-16 over 1500-byte frames, an LFSR descrambler per bit and per byte,
/// HDLC deframing per bit and the sync-word search per bit.
pub fn benchmarks() -> Vec<Benchmark> {
    bench::all()
}

/// A bit of a bit stream: a hard bit (`u8`, 0 or 1, [`Kind::HardBits`]) or an LLR (`i8`, [`Kind::Llr`]). The components of this
/// module that take either are generic over it.
pub trait BitItem: sdrkit_util_bits::Soft + Default + sealed::Sealed {
    /// The stream kind.
    const KIND: Kind;
    /// Which of a pair of registered names belongs to this kind: 0 for hard bits, 1 for LLRs.
    #[doc(hidden)]
    const INDEX: usize;
    /// The items of input `i`.
    #[doc(hidden)]
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self];
    /// Output `o`, to append to.
    #[doc(hidden)]
    fn write<'b>(io: &'b mut Io, o: usize) -> &'b mut Vec<Self>;
    /// Undoes NRZI (`nrzi` true) or NRZ-M on `y` in place. `prev` is the last level LLR, for LLRs.
    #[doc(hidden)]
    fn line_decode(nrzi: bool, i: &mut Nrzi, m: &mut NrzM, prev: &mut i8, y: &mut [Self]);
    /// Appends one bit per pair of `chips` (an even number) and returns the number of invalid pairs.
    #[doc(hidden)]
    fn manchester(chips: &[Self], conv: ManchesterConv, out: &mut Vec<Self>) -> u64;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for i8 {}
}

impl BitItem for u8 {
    const KIND: Kind = Kind::HardBits;
    const INDEX: usize = 0;
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [u8] {
        io.hard_bits(i)
    }
    fn write<'b>(io: &'b mut Io, o: usize) -> &'b mut Vec<u8> {
        io.hard_bits_out(o)
    }
    fn line_decode(nrzi: bool, i: &mut Nrzi, m: &mut NrzM, _: &mut i8, y: &mut [u8]) {
        if nrzi {
            i.decode(y);
        } else {
            m.decode(y);
        }
    }
    fn manchester(chips: &[u8], conv: ManchesterConv, out: &mut Vec<u8>) -> u64 {
        let mut bad = 0;
        for p in chips.as_chunks::<2>().0 {
            if Manchester::decode(p, conv, out).is_err() {
                bad += 1;
                out.push(match conv {
                    ManchesterConv::Ieee => p[1] & 1,
                    ManchesterConv::Thomas => p[0] & 1,
                });
            }
        }
        bad
    }
}

impl BitItem for i8 {
    const KIND: Kind = Kind::Llr;
    const INDEX: usize = 1;
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [i8] {
        io.llrs(i)
    }
    fn write<'b>(io: &'b mut Io, o: usize) -> &'b mut Vec<i8> {
        io.llrs_out(o)
    }
    fn line_decode(nrzi: bool, i: &mut Nrzi, m: &mut NrzM, prev: &mut i8, y: &mut [i8]) {
        if nrzi {
            i.decode_soft(y, prev);
        } else {
            m.decode_soft(y, prev);
        }
    }
    fn manchester(chips: &[i8], conv: ManchesterConv, out: &mut Vec<i8>) -> u64 {
        Manchester::decode_soft(chips, conv, out);
        0
    }
}

/// The input port `in` and the output port `out` of kind `T`, the output in the format of the input.
pub(crate) fn bit_ports<T: BitItem>() -> (Vec<Port>, Vec<Port>) {
    (
        vec![Port::new("in", T::KIND)],
        vec![Port::new("out", T::KIND).same_format_as(0)],
    )
}

sdrkit_core::choice_enum! {
    /// Where the frames or blocks of a component begin.
    pub enum FrameStartChoice {
        /// At each `FrameStart` tag.
        FrameStart = ("frame_start", "At frame start tags"),
        /// Back to back from the first item.
        Continuous = ("continuous", "Back to back"),
    }
}

pub(crate) use sdrkit_graph::PacketPool;
pub(crate) use sdrkit_graph::{walk_tags as walk, TagStep as Step};

/// Parses a whole-number setting given as text in hex (`"0x1ACFFC1D"` or `"1ACFFC1D"`) or binary (`"0b1011"`): the value and the
/// number of bits its digits stand for. `None` for anything else or more than 128 bits.
pub(crate) fn parse_bits_text(t: &str) -> Option<(u128, u32)> {
    let t = t.trim();
    let (digits, radix, per) =
        if let Some(b) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
            (b, 2, 1)
        } else if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            (h, 16, 4)
        } else {
            (t, 16, 4)
        };
    let digits: &str = digits;
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    let n = digits.len() as u32 * per;
    if n > 128 {
        return None;
    }
    u128::from_str_radix(digits, radix).ok().map(|v| (v, n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_text_parses_hex_and_binary() {
        assert_eq!(parse_bits_text("1ACFFC1D"), Some((0x1ACF_FC1D, 32)));
        assert_eq!(parse_bits_text("0x7e"), Some((0x7E, 8)));
        assert_eq!(parse_bits_text("0b0110"), Some((6, 4)));
        assert_eq!(parse_bits_text("0b012"), None);
        assert_eq!(parse_bits_text(""), None);
        assert_eq!(parse_bits_text("xyz"), None);
        assert_eq!(parse_bits_text(&"F".repeat(33)), None);
    }
}
