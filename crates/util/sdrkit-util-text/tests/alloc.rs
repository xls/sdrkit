//! Steady state allocates nothing: after a warm-up pass, 1,000 more calls of every decoder are counted.

use sdrkit_util_text::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Runs `f` once to warm up, then 1,000 times, and returns the allocations of the 1,000.
fn count(mut f: impl FnMut()) -> usize {
    f();
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..1000 {
        f();
    }
    ALLOCS.load(Ordering::Relaxed) - before
}

#[test]
fn decoders_do_not_allocate() {
    // Inputs built before counting.
    let mut ita = Vec::new();
    ita2::Encoder::new(&ita2::ITU)
        .encode_str("CQ DE K0TST 599 73", &mut ita)
        .unwrap();
    let mut sitor = Vec::new();
    ccir476::FecBEncoder::new()
        .message("ZCZC SA01 TEST", &mut sitor)
        .unwrap();
    let mut psk_bits = Vec::new();
    varicode::psk::encode(b"cq cq de k0tst", &mut |b| psk_bits.push(b)).unwrap();
    let mut mfsk_bits = Vec::new();
    varicode::mfsk::encode(b"cq cq de k0tst", &mut |b| mfsk_bits.push(b)).unwrap();
    let keyer = morse::Keyer::new(20.0).unwrap();
    let keys: Vec<morse::Key> = keyer.key("CQ DE K0TST").collect();
    let mut dvb_bytes = vec![0x15];
    dvb_bytes.extend_from_slice("\u{86}Nachrichten\u{87} über Ελλάδα".as_bytes());
    let gb = [0x13, 0xD6, 0xD0, 0xCE, 0xC4];
    let p100 = teletext::PageNo::from_decimal(100).unwrap();
    let tt = [
        teletext::packet::build_header(p100, 0, Default::default(), 0, b"SDRKIT"),
        teletext::packet::build_row(1, 1, b"Hello"),
        teletext::packet::build_filler(1),
    ];

    let mut ita_d = ita2::Decoder::new(&ita2::ITU);
    assert_eq!(
        count(|| ita.iter().for_each(|&c| {
            std::hint::black_box(ita_d.push(c));
        })),
        0,
        "ita2"
    );

    let mut fec = ccir476::FecBDecoder::new();
    assert_eq!(
        count(|| sitor.iter().for_each(|&w| {
            std::hint::black_box(fec.push(w));
        })),
        0,
        "fec-b"
    );

    let mut psk = varicode::psk::Decoder::new();
    assert_eq!(
        count(|| psk_bits.iter().for_each(|&b| {
            std::hint::black_box(psk.push(b));
        })),
        0,
        "psk"
    );

    let mut mfsk = varicode::mfsk::Decoder::new();
    assert_eq!(
        count(|| mfsk_bits.iter().for_each(|&b| {
            std::hint::black_box(mfsk.push(b));
        })),
        0,
        "mfsk"
    );

    let mut dom = varicode::dominoex::Decoder::new();
    assert_eq!(
        count(|| (0..16u8).for_each(|n| {
            std::hint::black_box(dom.push(n));
        })),
        0,
        "dominoex"
    );

    let mut fsq = varicode::fsq::Decoder::new(varicode::fsq::Alphabet::Fsq);
    assert_eq!(
        count(|| (0..32u8).for_each(|n| {
            std::hint::black_box(fsq.push(n));
        })),
        0,
        "fsq"
    );

    let mut md = morse::Decoder::new(morse::DecoderConfig::following(20.0)).unwrap();
    let mut mi = morse::Decoder::new(morse::DecoderConfig::idents()).unwrap();
    let mut idents = morse::IdentTracker::new(2);
    assert_eq!(
        count(|| keys.iter().for_each(|k| {
            md.push(k.on, k.seconds, &mut |e| {
                std::hint::black_box(e);
            });
            mi.push(k.on, k.seconds, &mut |e| {
                std::hint::black_box(e);
            });
            mi.idle(3.0, &mut |e| {
                std::hint::black_box(e);
            });
            idents.heard("K0TST", k.seconds);
        })),
        0,
        "morse decoder"
    );
    assert_eq!(
        count(|| keyer.key("CQ DE K0TST <SK>").for_each(|k| {
            std::hint::black_box(k);
        })),
        0,
        "morse keyer"
    );

    let mut t = dvb::DvbText::default();
    let o = dvb::Options::default();
    assert_eq!(
        count(|| t.decode(&dvb_bytes, dvb::Usage::Name, &o)),
        0,
        "dvb utf-8"
    );
    assert_eq!(
        count(|| t.decode(&gb, dvb::Usage::Name, &o)),
        0,
        "dvb gb2312"
    );
    assert_eq!(
        count(|| t.decode(b"Pr\xC8ufbild \xA4", dvb::Usage::Name, &o)),
        0,
        "dvb table 00"
    );

    let mut s = String::with_capacity(256);
    assert_eq!(
        count(|| {
            s.clear();
            ebu::RDS.decode_into(b"RADIO \x8a\x8d", &mut s);
            ebu::rds_text_into(&gb[1..], ebu::RdsCharset::Auto, &mut s);
            iso6937::decode_into(b"caf\xC2e", iso6937::Variant::Dvb, &mut s);
        }),
        0,
        "ebu, gb2312, iso 6937"
    );
    assert_eq!(
        count(|| {
            s.clear();
            charset_field::decode_into(b"Radio \x24\x8a", charset_field::Charset::EbuLatin, &mut s);
            charset_field::display_into(
                &[0x04, 0x1F, 0x00, 0x20],
                charset_field::Charset::Ucs2,
                &mut s,
            );
            charset_field::display_field_into(15, b"Caf\xC3\xA9 \x0A ", &mut s);
        }),
        0,
        "charset field"
    );

    let mut store = teletext::PageStore::default();
    let mut page = teletext::Page::default();
    assert_eq!(
        count(|| {
            tt.iter().for_each(|p| store.push_packet(p));
            while store.take_completed().is_some() {}
            store.render(p100, None, &mut page);
        }),
        0,
        "teletext"
    );

    let mut ts = store::TextStore::new(store::Limits::default());
    let mut clock = 0.0;
    assert_eq!(
        count(|| {
            for c in "CQ DE K0TST K\n".chars() {
                clock += 0.1;
                ts.push(c, clock, 7_040_000.0, 1);
            }
        }),
        0,
        "text store"
    );

    assert_eq!(
        count(|| assert_eq!(callsign::find("CQ DE K0TST K").count(), 1)),
        0,
        "callsigns"
    );
}
