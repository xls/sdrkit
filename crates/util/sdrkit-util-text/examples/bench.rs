//! Runs the text benchmarks: `cargo run --release -p sdrkit-util-text --example bench`.

use sdrkit_util_text::*;
use std::hint::black_box;
use std::time::Instant;

/// Calls `f` (which handles `items` items) for about half a second; prints items per second.
fn bench(name: &str, items: usize, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    let mut n = 0usize;
    while start.elapsed().as_secs_f64() < 0.5 {
        f();
        n += items;
    }
    let rate = n as f64 / start.elapsed().as_secs_f64();
    println!("{name:40} {:10.1} M items/s", rate / 1e6);
}

fn main() {
    let text = "CQ CQ DE K0TST K0TST PSE K 599 73 THE QUICK BROWN FOX 0123456789\n";

    let mut ita = Vec::new();
    ita2::Encoder::new(&ita2::ITU)
        .encode_str(text, &mut ita)
        .unwrap();
    let mut d = ita2::Decoder::new(&ita2::ITU);
    bench("ita2 decode (codes)", ita.len(), || {
        ita.iter().for_each(|&c| {
            black_box(d.push(c));
        })
    });

    let mut air = Vec::new();
    ccir476::FecBEncoder::new().message(text, &mut air).unwrap();
    let mut fec = ccir476::FecBDecoder::new();
    bench("ccir476 fec-b decode (words)", air.len(), || {
        air.iter().for_each(|&w| {
            black_box(fec.push(w));
        })
    });

    let mut bits = Vec::new();
    varicode::psk::encode(text.to_lowercase().as_bytes(), &mut |b| bits.push(b)).unwrap();
    let mut psk = varicode::psk::Decoder::new();
    bench("psk31 varicode decode (bits)", bits.len(), || {
        bits.iter().for_each(|&b| {
            black_box(psk.push(b));
        })
    });

    let mut mbits = Vec::new();
    varicode::mfsk::encode(text.to_lowercase().as_bytes(), &mut |b| mbits.push(b)).unwrap();
    let mut mfsk = varicode::mfsk::Decoder::new();
    bench("mfsk varicode decode (bits)", mbits.len(), || {
        mbits.iter().for_each(|&b| {
            black_box(mfsk.push(b));
        })
    });

    let keys: Vec<morse::Key> = morse::Keyer::new(25.0).unwrap().key(text.trim()).collect();
    let mut md = morse::Decoder::new(morse::DecoderConfig::following(20.0)).unwrap();
    bench("morse decode (intervals)", keys.len(), || {
        keys.iter().for_each(|k| {
            md.push(k.on, k.seconds, &mut |e| {
                black_box(e);
            });
        })
    });
    let mut mi = morse::Decoder::new(morse::DecoderConfig::idents()).unwrap();
    bench(
        "morse decode, every word fitted (intervals)",
        keys.len(),
        || {
            keys.iter().for_each(|k| {
                mi.push(k.on, k.seconds, &mut |e| {
                    black_box(e);
                });
            })
        },
    );

    let mut dvb_bytes = b"\x86BBC\x87 News: Pr\xC8ufbild caf\xC2e \xA4 ".repeat(4);
    dvb_bytes.truncate(200);
    let mut t = dvb::DvbText::default();
    let o = dvb::Options::default();
    bench("dvb text table 00 (bytes)", dvb_bytes.len(), || {
        t.decode(black_box(&dvb_bytes), dvb::Usage::Description, &o)
    });

    let p = teletext::PageNo::from_decimal(100).unwrap();
    let mut pk = vec![teletext::packet::build_header(
        p,
        0,
        Default::default(),
        0,
        b"SDRKIT TELETEXT",
    )];
    for r in 1..=24 {
        pk.push(teletext::packet::build_row(
            1,
            r,
            b"The quick brown fox jumps over the lazy",
        ));
    }
    let mut store = teletext::PageStore::default();
    bench("teletext page store (packets)", pk.len(), || {
        pk.iter().for_each(|x| store.push_packet(x))
    });
    let mut page = teletext::Page::default();
    bench("teletext render level 1.5 (pages)", 1, || {
        store.render(p, None, &mut page);
    });

    let mut s = String::with_capacity(4096);
    let rds = b"RADIO \x8a\x8d ".repeat(8);
    bench("ebu latin rds (bytes)", rds.len(), || {
        s.clear();
        ebu::RDS.decode_into(&rds, &mut s);
    });

    let dl = b"Now playing: \x24\x8a Artist - Title\x0Bheadline\x1F  ".repeat(4);
    bench("charset field display, ebu latin (bytes)", dl.len(), || {
        s.clear();
        charset_field::display_into(black_box(&dl), charset_field::Charset::EbuLatin, &mut s);
    });
}
