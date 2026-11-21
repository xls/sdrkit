//! Edge cases of the standards: protection runs, label line breaks, Reed-Solomon corrections, erased frames, wide bit fields,
//! the LATM configuration and a slide that changes under the same transport id.

use super::common::*;
use sdrkit_algo_fec::rs::{codes::DAB_PLUS_120_110, Rs};
use sdrkit_core::StreamKind;
use sdrkit_stack_dab::audio::dabplus::{latm, AuEvent, AudioParams, SuperframeDecoder};
use sdrkit_stack_dab::bits::{written, BitReader, Fields};
use sdrkit_stack_dab::data::pad;
use sdrkit_stack_dab::fec::{encode_profile, ProfileDecoder};
use sdrkit_stack_dab::generator::{DabGenerator, GenParams};
use sdrkit_stack_dab::tables;
use std::sync::Arc;

/// EN 300 401 table 18: protection level 2-A at 8 kbit/s has its own runs, 5 blocks of PI 13 and 1 of PI 12, in 8 CUs.
#[test]
fn eep_2a_at_8_kbits_has_its_own_runs() {
    let p = tables::eep_profile(0, 2, 8).expect("2-A at 8 kbit/s is in table 18");
    assert_eq!(
        (p.runs.clone(), p.size_cu, p.coded_bits()),
        (vec![(5, 13), (1, 12)], 8, 512)
    );
    let data: Vec<u8> = (0..24u8).map(|i| i.wrapping_mul(37)).collect();
    let mut coded = Vec::new();
    encode_profile(&p, &data, &mut coded);
    let llr: Vec<i8> = coded
        .iter()
        .map(|&b| if b == 0 { 50 } else { -50 })
        .collect();
    let mut out = [0u8; 24];
    ProfileDecoder::new(&p)
        .unwrap()
        .decode(&llr, &mut out)
        .unwrap();
    assert_eq!(&out[..], &data[..]);
}

/// TS 101 756 Annex C assigns 0x0A of the DAB character set the meaning "preferred line break": a label keeps it.
#[test]
fn a_label_keeps_its_preferred_line_break() {
    let mut d = pad::PadDecoder::new();
    let seg = pad::write::dl_segment(false, true, true, 0, 0, b"Now\x0ANext");
    d.push(&pad::PadItem::new(
        pad::write::fpad(2, true),
        &pad::write::variable_xpad(&[(2, seg)]),
        true,
    ));
    assert_eq!(d.label.text, "Now\nNext");
    assert_eq!(sdrkit_stack_dab::registry::label_text(&[0x24], 0), "ł");
}

/// After a correction the word is a code word: the decoder puts as many corrections as the locator has roots, so the
/// corrected word always has zero syndromes. A search over many error patterns finds no word "corrected" into a non code word.
#[test]
fn rs_corrections_always_leave_a_code_word() {
    let mut rs = Rs::new(DAB_PLUS_120_110).unwrap();
    let mut seed = 0x1234_5678_9abc_def1u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let data: Vec<u8> = (0..110u8)
        .map(|i| i.wrapping_mul(7).wrapping_add(3))
        .collect();
    let mut word = data.clone();
    word.resize(120, 0);
    rs.encode_word(&mut word).unwrap();
    for _ in 0..20_000 {
        let mut w = word.clone();
        let errors = 1 + (next() % 9) as usize;
        for _ in 0..errors {
            let at = (next() % 120) as usize;
            w[at] ^= (next() % 255 + 1) as u8;
        }
        if rs.decode(&mut w, &[]).is_ok() {
            assert!(rs.check(&w).unwrap());
        }
    }
}

/// TS 102 563 clause 6: a logical frame marked erased produces erasures, not zeros. When the bytes of such a frame are right,
/// the superframe still decodes.
#[test]
fn a_frame_marked_erased_but_intact_still_decodes() {
    let g = DabGenerator::from_loop(GenParams::default(), default_loop());
    let lfs = g.logical_frames(1).unwrap();
    let mut d = SuperframeDecoder::new();
    let mut out = Vec::new();
    for (k, f) in lfs.iter().take(60).enumerate() {
        let data: Arc<[u8]> = f.as_slice().into();
        d.push(&data, k > 20 && k % 5 == 2, k as f64 * 0.024, &mut out);
    }
    let lost = out
        .iter()
        .filter(|e| matches!(e, AuEvent::Lost { .. }))
        .count();
    assert_eq!((d.stats.superframes_bad, lost), (0, 0), "{:?}", d.stats);
    assert!(d.stats.access_units_ok >= 50, "{:?}", d.stats);
}

/// One bit reader and writer for every family: fields up to 64 bits, the value before the width. A read that does not fit
/// returns nothing, never a truncated value.
#[test]
fn a_field_wider_than_32_bits_is_never_cut() {
    let v = 0x1_2345_6789u64;
    let b = written(|w| {
        w.put(v, 33);
        w.put(5, 3);
    });
    let mut r = BitReader::new(&b);
    assert_eq!(r.read(33), None);
    assert_eq!(r.read_wide(33), Some(v));
    assert_eq!(r.read(3), Some(5));
}

/// ISO/IEC 14496-3 clause 1.7.3: the AudioSpecificConfig is embedded in the StreamMuxConfig bit for bit. With SBR and PS it is
/// 25 bits, and frameLengthType follows at once.
#[test]
fn dab_plus_latm_carries_the_sbr_config_unpadded() {
    let p = AudioParams::from_header(0x68).unwrap();
    let au = [0xAB, 0xCD, 0xEF];
    let mut got = Vec::new();
    latm(&p, &au, &mut got);
    let want = written(|w| {
        w.put(0, 1); // useSameStreamMux
        w.put(0, 1); // audioMuxVersion
        w.put(1, 1); // allStreamsSameTimeFraming
        w.put(0, 6); // numSubFrames
        w.put(0, 4); // numProgram
        w.put(0, 3); // numLayer
        w.put(29, 5); // AOT PS
        w.put(6, 4); // 24 kHz core
        w.put(1, 4); // mono core
        w.put(3, 4); // 48 kHz output
        w.put(2, 5); // AOT AAC LC
        w.put(0b100, 3); // GASpecificConfig: 960-sample frames
        w.put(0, 3); // frameLengthType
        w.put(0xFF, 8); // latmBufferFullness
        w.put(0, 1); // otherDataPresent
        w.put(0, 1); // crcCheckPresent
        w.put(3, 8); // MuxSlotLengthBytes
        w.bytes(&au);
    });
    assert_eq!(got, want);
}

/// EN 301 234: an object whose body changes under the same transport id is a new object, and the slideshow shows it.
#[test]
fn a_changed_slide_under_the_same_transport_id_comes_again() {
    let mut p = GenParams::default();
    let slide = sdrkit_stack_dab::generator::png::test_slide();
    let mut other = slide.clone();
    // The same picture with another last byte (the IEND CRC), as a changed body.
    let n = other.len();
    other[n - 1] ^= 0xFF;
    let svc = p.services.iter_mut().find(|s| s.subch == 3).unwrap();
    svc.pad.slide = Some(("First".into(), slide));
    svc.pad.next_slide = Some(("Second".into(), other));
    let x = generate(p, 12.0);
    let r = run(&x, 2.048e6, 3, 65_536);
    let names: Vec<String> = r
        .events
        .iter()
        .filter(|e| e.is(&sdrkit_stack_dab::typed::SLIDE_SHOWN))
        .filter_map(|e| e.get(&sdrkit_core::publish::keys::TEXT).map(str::to_string))
        .collect();
    assert!(names.iter().any(|n| n == "First"), "{names:?}");
    assert!(names.iter().any(|n| n == "Second"), "{names:?}");
    assert!(!r.units(StreamKind::Picture).is_empty());
}
