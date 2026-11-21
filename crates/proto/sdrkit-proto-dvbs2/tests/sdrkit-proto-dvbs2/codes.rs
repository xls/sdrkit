//! The codes of the receiver: the DVB-S2 FEC frame decoder and the inner code of DVB-S.

use sdrkit_core::Complex32;
use sdrkit_proto_dvbs2::fec::FrameDecoder;
use sdrkit_proto_dvbs2::modcod::{ModCod, Modulation};
use sdrkit_proto_dvbs2::CodeRate;

/// The codes and the bit interleaver of a MODCOD are made once, not for each frame.
#[test]
fn the_bit_interleaver_is_built_once_per_modcod() {
    let m = ModCod::find(Modulation::Psk8, CodeRate::R35).unwrap();
    let mut dec = FrameDecoder::default();
    let syms = vec![Complex32::new(0.7, 0.7); 16200 / 3];
    let mut out = Vec::new();
    for _ in 0..20 {
        assert!(dec.decode(&m, true, &syms, 0.1, &mut out).is_some());
    }
    assert_eq!(dec.codecs_made(), 1);
    assert!(dec.codec(&m, true).unwrap().deinterleave.is_some());
    assert_eq!(dec.codecs_made(), 1);
}

/// The inner Viterbi decoder corrects errors: a code word of the K = 7 mother code with one wrong soft value in 20 decodes to
/// its bits.
#[test]
fn the_inner_viterbi_corrects_errors() {
    use sdrkit_proto_dvbs2::inner::{encoder, inner_decoder};
    let bits: Vec<u8> = (0..4000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13 & 1) as u8)
        .collect();
    let mut coded = Vec::new();
    encoder().encode(&bits, &mut coded);
    let mut llr: Vec<i8> = coded
        .iter()
        .map(|&b| if b == 0 { 60 } else { -60 })
        .collect();
    let mut flipped = 0;
    for v in llr.iter_mut().step_by(20) {
        *v = -*v;
        flipped += 1;
    }
    let mut v = inner_decoder();
    let mut out = Vec::new();
    v.push(&llr, &mut out);
    let n = out.len().min(3000);
    assert!(n >= 3000, "{} bits out", out.len());
    assert_eq!(&out[..n], &bits[..n], "{flipped} errors");
}
