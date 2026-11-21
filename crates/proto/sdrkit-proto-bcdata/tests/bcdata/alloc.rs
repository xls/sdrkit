//! Steady state allocates nothing: after a warm-up, 1,000 more calls are counted. The counter is per thread, so tests that run
//! at the same time on other threads do not disturb it.

use sdrkit_algo_dsp::Frame;
use sdrkit_proto_bcdata::aac::{write_loas, AudioSpecificConfig};
use sdrkit_proto_bcdata::datagroup::{self, Header as Dg};
use sdrkit_proto_bcdata::label::{self, Decoder};
use sdrkit_proto_bcdata::mot::{self, Assembler, HeaderWriter};
use sdrkit_proto_bcdata::mp2;
use sdrkit_proto_bcdata::packet::{
    DabAssembler, DabFec, DabPacketWriter, DrmAssembler, DrmPacketWriter, FEC_ADT,
};
use sdrkit_proto_bcdata::playout::{Concealment, Config, Playout};
use sdrkit_proto_bcdata::tags::{parse_rtplus, Item, Tag, TagSet};
use sdrkit_util_text::charset_field::Charset;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Runs `f` 10 times to warm up, then 1,000 times, and returns the allocations of the 1,000.
fn count(mut f: impl FnMut()) -> usize {
    for _ in 0..10 {
        f();
    }
    let before = ALLOCS.with(|c| c.get());
    for _ in 0..1000 {
        f();
    }
    ALLOCS.with(|c| c.get()) - before
}

#[test]
fn data_groups_and_packets() {
    let mut g = Vec::new();
    Dg {
        group_type: 4,
        segment: Some((true, 0)),
        transport_id: Some(1),
        ..Dg::default()
    }
    .write(&[7; 500], &mut g)
    .unwrap();
    assert_eq!(count(|| assert!(datagroup::parse(&g).is_ok())), 0);
    let mut dab = Vec::new();
    DabPacketWriter::new(5)
        .unwrap()
        .write_group(&g, &mut dab)
        .unwrap();
    let mut a = DabAssembler::new();
    let mut n = 0;
    assert_eq!(count(|| a.push(&dab, &mut |_, _| n += 1)), 0);
    assert_eq!(n, 1010);
    let mut drm = Vec::new();
    DrmPacketWriter::new(1, 48)
        .unwrap()
        .write_unit(&g, &mut drm)
        .unwrap();
    let mut d = DrmAssembler::new(48, None).unwrap();
    assert_eq!(count(|| d.push(&drm, &mut |_, _| {})), 0);
    let mut adt = dab.clone();
    adt.resize(FEC_ADT, 0);
    let mut frame = adt.clone();
    let mut fec = DabFec::new();
    fec.encode(&adt, &mut frame).unwrap();
    frame[10] ^= 0x55;
    assert_eq!(count(|| fec.push(&frame, &mut |_, _| {})), 0);
}

#[test]
fn mot_repeats() {
    let body = vec![3u8; 2000];
    let mut h = Vec::new();
    HeaderWriter::new(2000, 2, 1)
        .content_name("x.jpg")
        .write(&mut h)
        .unwrap();
    let mut groups = Vec::new();
    for (t, part) in [
        (datagroup::MOT_HEADER, &h[..]),
        (datagroup::MOT_BODY, &body[..]),
    ] {
        for (i, c) in part.chunks(500).enumerate() {
            let mut seg = Vec::new();
            mot::write_segment(0, c, &mut seg).unwrap();
            let mut g = Vec::new();
            let last = (i + 1) * 500 >= part.len();
            Dg {
                group_type: t,
                segment: Some((last, i as u16)),
                transport_id: Some(3),
                ..Dg::default()
            }
            .write(&seg, &mut g)
            .unwrap();
            groups.push(g);
        }
    }
    let mut a = Assembler::new();
    assert_eq!(
        count(|| {
            for g in &groups {
                a.push(&datagroup::parse(g).unwrap(), &mut |_| {});
            }
        }),
        0
    );
    assert_eq!(a.stats.objects, 1);
}

#[test]
fn label_repeats() {
    let mut s = Vec::new();
    label::write_label(
        "Artist - Title of the song",
        Charset::EbuLatin,
        false,
        &mut s,
    )
    .unwrap();
    label::write_dl_plus(
        false,
        false,
        Item::default(),
        &[Tag {
            content_type: 4,
            start: 0,
            length_marker: 5,
        }],
        &mut s,
    )
    .unwrap();
    let mut d = Decoder::new();
    assert_eq!(
        count(|| {
            let mut at = 0;
            while at < s.len() {
                let n = label::segment_len(&s[at..]).unwrap();
                d.push(&s[at..at + n]).unwrap();
                at += n;
            }
        }),
        0
    );
    assert_eq!(d.tags().text(4), Some("Artist"));
}

#[test]
fn rt_plus_tags_with_the_segment_rule() {
    let rt = *b"Artist - Title of the song\r                                     ";
    let seen = [true; 16];
    let (item, tags) = parse_rtplus(0x0008, (4 << 13) | (5 << 1), (1 << 11) | (9 << 5) | 16);
    let mut s = TagSet::default();
    let decode = |b: &[u8], out: &mut String| out.extend(b.iter().map(|&c| c as char));
    assert_eq!(
        count(|| {
            s.merge_received(item, &tags, &rt, |i| seen[i / 4], decode);
            s.refresh(&rt, |i| seen[i / 4], decode);
        }),
        0
    );
    assert_eq!(s.text(1), Some("Title of the song"));
}

#[test]
fn audio_packaging_and_headers() {
    let c = AudioSpecificConfig::dab_plus(true, true, true, true).unwrap();
    let mut out = Vec::with_capacity(4096);
    let au = [0u8; 600];
    assert_eq!(
        count(|| {
            out.clear();
            write_loas(&c, true, &au, &mut out).unwrap();
        }),
        0
    );
    let mut lf = vec![0u8; 192];
    lf[..4].copy_from_slice(&[0xFF, 0xF5, 0x84, 0xC4]);
    let (mut framer, mut xpad) = (mp2::Framer::default(), Vec::with_capacity(400));
    assert_eq!(
        count(|| {
            let _ = framer.push(&lf, false);
            if let Some(f) = framer.push(&lf, false) {
                assert!(mp2::pad_of_frame(f, &mut xpad).is_some());
            }
        }),
        0
    );
}

#[test]
fn playout_steady_state() {
    for c in [Concealment::Repeat, Concealment::Pitch] {
        let mut p = Playout::new(Config::new(48_000, c));
        let sound = vec![Frame([0.1, -0.1]); 1920];
        let mut out = Vec::with_capacity(1920);
        let mut buf = vec![Frame::default(); 480];
        let mut k = 0;
        assert_eq!(
            count(|| {
                if k % 7 == 3 {
                    p.push_lost(1920);
                } else {
                    p.push_sound(&sound);
                }
                k += 1;
                out.clear();
                p.pull(1440, &mut out);
                p.pull_resampled(&mut buf);
            }),
            0,
            "{c:?}"
        );
    }
}

#[test]
fn publishing_a_label_again() {
    use sdrkit_core::Published;
    use sdrkit_proto_bcdata::publish::{self, Publisher};
    let shape = publish::interface().stack_shape("bcdata").unwrap();
    let mut p = Published::from_decl(std::sync::Arc::new(shape.results_decl()));
    let mut w = Publisher::new();
    let ascii = |b: &[u8], s: &mut String| s.extend(b.iter().map(|&c| c as char));
    let (item, t) = parse_rtplus(0x0008, (4 << 13) | (3 << 1), (1 << 11) | (7 << 5) | 2);
    let mut set = TagSet::default();
    set.merge_received(item, &t, b"ABBA - SOS", |_| true, ascii);
    // The text of each RadioText group published again, as a stack does after each group.
    assert_eq!(count(|| w.label(&mut p, "ABBA - SOS", &set)), 0);
}
