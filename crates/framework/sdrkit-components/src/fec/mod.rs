//! `fec.*`: the components over `sdrkit-algo-fec` (forward error correction). Named codes, patterns and interleavers are
//! chosen by the names that `sdrkit-algo-fec` uses for them, in snake case. The frame decoders report each frame on a typed
//! `status` output ([`FrameStatus`]).

mod bench;
mod common;
mod deinterleave;
mod frame;
mod puncture;
mod quantize;
mod rs;
mod trellis;
mod viterbi;

pub use bench::benchmarks;
pub use common::{Erasures, FrameStatus};
pub use deinterleave::{Deinterleave, DeinterleaveSetting, DeinterleaverKind, InterleaverName};
pub use frame::{
    bch_codes, BchDecode, BchDecodeSetting, LdpcAlgo, LdpcDecode, LdpcDecodeSetting, TurboDecode,
    TurboDecodeSetting, TurboStop, ITERATIONS,
};
pub use puncture::{patterns, Depuncture, PunctureComponent, PunctureName, PunctureSetting};
pub use quantize::{Quantize, QuantizeSetting};
pub use rs::{
    RsCodeName, RsDecode, RsDecodeSetting, SymbolBasis, PACKETS_CLEAN, PACKETS_CORRECTED,
    PACKETS_FAILED, SYMBOLS_CORRECTED,
};
pub use trellis::{TrellisDecode, TrellisDecodeSetting, TrellisName};
pub use viterbi::{ConvCodeName, TrellisEnds, ViterbiDecode, ViterbiDecodeSetting, MEAN_GAIN};

use sdrkit_graph::Registry;

/// Adds the `fec.*` components.
pub fn register(r: &mut Registry) {
    r.add("fec.quantize", || Box::new(Quantize::default()));
    r.add("fec.puncture", || {
        Box::new(PunctureComponent::<u8>::default())
    });
    r.add("fec.puncture_llr", || {
        Box::new(PunctureComponent::<i8>::default())
    });
    r.add("fec.depuncture", || Box::new(Depuncture::default()));
    r.add("fec.viterbi", || Box::new(ViterbiDecode::default()));
    r.add("fec.trellis", || Box::new(TrellisDecode::default()));
    r.add("fec.trellis_symbols", || {
        Box::new(TrellisDecode::on_symbols())
    });
    r.add("fec.deinterleave", || Box::new(Deinterleave::default()));
    r.add("fec.deinterleave_bytes", || {
        Box::new(Deinterleave::on_bytes())
    });
    r.add("fec.rs_decode", || Box::new(RsDecode::default()));
    r.add("fec.bch_decode", || Box::new(BchDecode::default()));
    r.add("fec.ldpc_decode", || Box::new(LdpcDecode::default()));
    r.add("fec.turbo_decode", || Box::new(TurboDecode::default()));
}
