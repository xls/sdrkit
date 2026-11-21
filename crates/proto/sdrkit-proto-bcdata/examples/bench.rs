//! Runs the benchmarks of the hot paths: `cargo run --release -p sdrkit-proto-bcdata --example bench`.

use sdrkit_algo_dsp::Frame;
use sdrkit_proto_bcdata::aac::{write_loas, AudioSpecificConfig};
use sdrkit_proto_bcdata::datagroup::{self, Header as Dg};
use sdrkit_proto_bcdata::label::{self, Decoder};
use sdrkit_proto_bcdata::mot::{self, Assembler, HeaderWriter};
use sdrkit_proto_bcdata::packet::{
    dab_padding, DabAssembler, DabFec, DabPacketWriter, DrmAssembler, DrmPacketWriter, FEC_ADT,
};
use sdrkit_proto_bcdata::playout::{Concealment, Config, Playout};
use sdrkit_proto_bcdata::tags;
use sdrkit_util_text::charset_field::Charset;
use std::hint::black_box;
use std::time::Instant;

/// Calls `f` (which handles `items` items) for about half a second, and prints the items per second.
fn bench(name: &str, items: usize, unit: &str, mut f: impl FnMut()) {
    f();
    let start = Instant::now();
    let mut n = 0usize;
    while start.elapsed().as_secs_f64() < 0.5 {
        f();
        n += items;
    }
    let rate = n as f64 / start.elapsed().as_secs_f64();
    println!("{name:44} {:10.2} M {unit}/s", rate / 1e6);
}

fn main() {
    let mut g = Vec::new();
    Dg {
        group_type: 4,
        segment: Some((true, 0)),
        transport_id: Some(1),
        ..Dg::default()
    }
    .write(&[7; 1000], &mut g)
    .unwrap();
    bench("data group parse + CRC (bytes)", g.len(), "bytes", || {
        black_box(datagroup::parse(black_box(&g)).unwrap());
    });

    let mut dab = Vec::new();
    let mut w = DabPacketWriter::new(5).unwrap();
    for _ in 0..20 {
        w.write_group(&g, &mut dab).unwrap();
    }
    let mut a = DabAssembler::new();
    bench("DAB packet assembly (bytes)", dab.len(), "bytes", || {
        a.push(&dab, &mut |_, x| {
            black_box(x);
        })
    });

    let mut drm = Vec::new();
    let mut w = DrmPacketWriter::new(1, 48).unwrap();
    for _ in 0..20 {
        w.write_unit(&g, &mut drm).unwrap();
    }
    let mut d = DrmAssembler::new(48, None).unwrap();
    bench("DRM packet assembly (bytes)", drm.len(), "bytes", || {
        d.push(&drm, &mut |_, x| {
            black_box(x);
        })
    });

    let mut adt = Vec::new();
    DabPacketWriter::new(5)
        .unwrap()
        .write_group(&g, &mut adt)
        .unwrap();
    while adt.len() < FEC_ADT {
        dab_padding(&mut adt);
    }
    adt.truncate(FEC_ADT);
    let mut frame = adt.clone();
    let mut fec = DabFec::new();
    fec.encode(&adt, &mut frame).unwrap();
    for i in [10, 400, 900] {
        frame[i] ^= 0x33;
    }
    bench(
        "DAB packet FEC frame, 3 errors (bytes)",
        frame.len(),
        "bytes",
        || {
            fec.push(&frame, &mut |_, x| {
                black_box(x);
            })
        },
    );

    let body = vec![3u8; 20_000];
    let mut h = Vec::new();
    HeaderWriter::new(body.len() as u32, 2, 1)
        .content_name("slide.jpg")
        .write(&mut h)
        .unwrap();
    let mut groups = Vec::new();
    for (t, part) in [
        (datagroup::MOT_HEADER, &h[..]),
        (datagroup::MOT_BODY, &body[..]),
    ] {
        let n = part.len().div_ceil(1000);
        for (i, c) in part.chunks(1000).enumerate() {
            let mut seg = Vec::new();
            mot::write_segment(0, c, &mut seg).unwrap();
            let mut g = Vec::new();
            Dg {
                group_type: t,
                segment: Some((i + 1 == n, i as u16)),
                transport_id: Some(3),
                ..Dg::default()
            }
            .write(&seg, &mut g)
            .unwrap();
            groups.push(g);
        }
    }
    let mut m = Assembler::new();
    bench(
        "MOT object assembly (body bytes)",
        body.len(),
        "bytes",
        || {
            for g in &groups {
                m.push(&datagroup::parse(g).unwrap(), &mut |o| {
                    black_box(o.body);
                });
            }
        },
    );

    let mut segs = Vec::new();
    label::write_label(
        "Artist name - The title of this song (radio edit)",
        Charset::Utf8,
        false,
        &mut segs,
    )
    .unwrap();
    let mut dl = Decoder::new();
    bench(
        "dynamic label segments (bytes)",
        segs.len(),
        "bytes",
        || {
            let mut at = 0;
            while at < segs.len() {
                let n = label::segment_len(&segs[at..]).unwrap();
                black_box(dl.push(&segs[at..at + n]).unwrap());
                at += n;
            }
        },
    );

    let rt = *b"Artist - Title of the song\r                                     ";
    let seen = [true; 16];
    let (item, rtp) = tags::parse_rtplus(0x0008, (4 << 13) | (5 << 1), (1 << 11) | (9 << 5) | 16);
    let mut set = tags::TagSet::default();
    let decode = |b: &[u8], out: &mut String| out.extend(b.iter().map(|&c| c as char));
    bench(
        "RT+ group with the segment rule (groups)",
        1,
        "groups",
        || {
            black_box(set.merge_received(item, &rtp, &rt, |i| seen[i / 4], decode));
        },
    );

    let asc = AudioSpecificConfig::dab_plus(true, true, true, false).unwrap();
    let au = vec![0x5Au8; 600];
    let mut out = Vec::with_capacity(1024);
    bench("LOAS framing of 600-byte units (units)", 1, "units", || {
        out.clear();
        write_loas(&asc, true, &au, &mut out).unwrap();
        black_box(&out);
    });

    for c in [Concealment::Repeat, Concealment::Pitch] {
        let mut p = Playout::new(Config::new(48_000, c));
        let sound: Vec<Frame<2>> = (0..1920)
            .map(|i| Frame([(i as f32 * 0.05).sin() * 0.3, (i as f32 * 0.07).sin() * 0.3]))
            .collect();
        let mut out = Vec::with_capacity(1920);
        let mut k = 0;
        let name = format!("playout {c:?}, 1 loss in 8 (frames)");
        bench(&name, 1920, "frames", || {
            if k % 8 == 5 {
                p.push_lost(1920);
            } else {
                p.push_sound(&sound);
            }
            k += 1;
            out.clear();
            p.pull(1920, &mut out);
            black_box(&out);
        });
        let mut buf = vec![Frame::default(); 1920];
        let name = format!("playout {c:?}, resampled pull (frames)");
        bench(&name, 1920, "frames", || {
            p.push_sound(&sound);
            p.pull_resampled(&mut buf);
            black_box(&buf);
        });
    }
}
