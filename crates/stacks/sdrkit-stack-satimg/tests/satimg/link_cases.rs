//! Cases of the link layers: the Meteor interleaver, a zone of idle data, and the Reed-Solomon check after a correction.

use sdrkit_algo_fec::rs::{codes, Rs};
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::tm::FirstHeader;
use sdrkit_stack_satimg::components::link::VcduComponent;
use sdrkit_stack_satimg::gen::{aos_frame, space_packet};
use sdrkit_stack_satimg::modes::meteor::{Deinterleaver, Interleaver, DATA_BITS, PERIOD_BITS};

/// On the transmit side the branch with no delay passes its bit straight through, so the first data bit after each marker (and
/// the 37th, two whole turns of 36 branches in a period) leaves as itself. A ring read before it is written sends a bit of a
/// whole ring earlier instead.
#[test]
fn meteor_interleaver_passes_the_zero_delay_branch_through() {
    let mut i = Interleaver::new(7);
    let data: Vec<u8> = (0..72 * 100)
        .map(|k| ((k * 7 + k / 5) % 3 == 0) as u8)
        .collect();
    let mut out = Vec::new();
    i.push(&data, &mut out);
    assert_eq!(out.len(), 100 * PERIOD_BITS);
    for p in 0..100 {
        assert_eq!(
            &out[p * 80..p * 80 + 8],
            &[0, 0, 1, 0, 0, 1, 1, 1],
            "marker"
        );
        for b in [0usize, 36] {
            assert_eq!(
                out[p * 80 + 8 + b],
                data[p * DATA_BITS + b],
                "period {p} bit {b}"
            );
        }
    }
}

/// The deinterleaver undoes the interleaver exactly: after the end-to-end delay every data bit comes back.
#[test]
fn meteor_interleaver_round_trip_returns_every_bit() {
    let mut tx = Interleaver::new(3);
    let n = 80_000 * DATA_BITS / 2;
    let data: Vec<u8> = (0..n).map(|k| ((k * 13 + k / 7) % 5 < 2) as u8).collect();
    let mut chan = Vec::new();
    tx.push(&data, &mut chan);
    // Bits to LLRs (positive means 0), two at a time as the rails of the demodulator give them.
    let llr: Vec<i8> = chan
        .iter()
        .map(|&b| if b == 0 { 60 } else { -60 })
        .collect();
    let mut rx = Deinterleaver::new();
    let mut out = Vec::new();
    let mut total = 0;
    rx.process(&llr, &mut out, &mut total);
    assert!(rx.locked());
    let got: Vec<u8> = out.iter().map(|&v| (v < 0) as u8).collect();
    // The data bits that came out after the memory filled: find where they line up with the input.
    let tail = &got[got.len() - 20_000..];
    assert!(
        (0..data.len() - 20_000).any(|s| data[s..s + 20_000] == *tail),
        "the bits come back in order"
    );
}

/// A frame whose zone holds only idle data (first header pointer 0x7FE, CCSDS 732.0-B-4 clause 4.1.4.2.2) is taken: it is not
/// refused, and the next frame shows no counter gap.
#[test]
fn an_idle_only_zone_is_not_a_refused_frame() {
    let layout = Layout::METEOR;
    let zl = layout.data_zone_len();
    let mut vc = VcduComponent::new(layout, false);
    let a = space_packet(64, 3, 1, &vec![0x11; zl - 6], false);
    let idle = space_packet(0x7FF, 3, 0, &vec![0; zl - 6], false);
    let c = space_packet(65, 3, 2, &vec![0x22; zl - 6], false);
    let frames = [
        aos_frame(&layout, 156, 5, 0, FirstHeader::At(0), &a),
        aos_frame(&layout, 156, 5, 1, FirstHeader::IdleOnly, &idle),
        aos_frame(&layout, 156, 5, 2, FirstHeader::At(0), &c),
    ];
    let (mut items, mut packets) = (Vec::new(), Vec::new());
    for (i, f) in frames.iter().enumerate() {
        vc.push_frame(f, i as f64, &mut items, &mut packets);
    }
    let s = vc.stats();
    assert_eq!(s.refused, 0);
    assert_eq!(s.idle_frames, 1);
    assert_eq!(items.len(), 3);
    let apids: Vec<u16> = packets.iter().map(|p| p.apid).collect();
    assert_eq!(apids, [64, 65]);
    assert!(packets.iter().all(|p| !p.gap_before));
}

/// The Reed-Solomon decoder computes the syndromes again after a correction: a correction that puts a symbol in the virtual
/// fill of a shortened code (where the sender has only zeros) leaves a word that is not a code word, and it is refused.
#[test]
fn reed_solomon_checks_the_syndromes_after_a_correction() {
    let full = codes::CCSDS_255_223;
    let q = 32usize;
    let short = full.shortened(255 - q as u16);
    // A code word of the full code with one symbol in the fill (index 0) and 32 in the sent part, made by erasure decoding.
    let mut w = vec![0u8; 255];
    w[0] = 0x5A;
    let erased: Vec<u16> = (0..32).map(|i| (q + 3 + 7 * i) as u16).collect();
    Rs::new(full).unwrap().decode(&mut w, &erased).unwrap();
    assert!(Rs::new(full).unwrap().check(&w).unwrap());
    let support: Vec<usize> = (q..255).filter(|&i| w[i] != 0).collect();
    assert_eq!(support.len(), 32);
    // The zero word with 17 of those symbols as errors: 16 symbols from w, so a decoder of the full code moves it to w.
    let mut word = vec![0u8; 255 - q];
    for &i in &support[..17] {
        word[i - q] = w[i];
    }
    let sent = word.clone();
    let r = Rs::new(short).unwrap().decode(&mut word, &[]);
    assert!(r.is_err(), "{r:?}");
    assert_eq!(word, sent);
}
