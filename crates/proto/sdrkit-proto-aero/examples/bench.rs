//! Times the hot paths: `cargo run --release -p sdrkit-proto-aero --example bench`. Each line is the best of several runs, in
//! thousands of items per second.

use sdrkit_proto_aero::acars::assemble::Quality;
use sdrkit_proto_aero::acars::{self, Block, BlockSpec, End, Message};
use sdrkit_proto_aero::{adsc, arinc622, textpos};
use std::hint::black_box;
use std::time::Instant;

fn measure(name: &str, items: usize, mut step: impl FnMut()) {
    step();
    let start = Instant::now();
    let mut best = f64::INFINITY;
    let mut runs = 0;
    while runs < 5 || start.elapsed().as_secs_f64() < 0.5 {
        let t = Instant::now();
        step();
        best = best.min(t.elapsed().as_secs_f64());
        runs += 1;
    }
    println!("{name:48} {:10.1} k/s", items as f64 / best / 1e3);
}

const ADS: &str = "/PIKCPYA.ADS.HB-JHM07040B000C000D010E011000440D";

fn main() {
    let text = "M01AXX0001#M1BPOSN43312W123174,EASON,215754,370 THE REST OF A LONGER MESSAGE TEXT FOR TIMING";
    let mut frame = Vec::new();
    BlockSpec::new('2', "N123AB", b'1', End::Etx)
        .label("H1")
        .text(text)
        .frame(2, true, &mut frame)
        .unwrap();
    let f = acars::split_frame(&frame).unwrap();
    let mut body = f.body.to_vec();
    let n = 1000;
    measure("block: split, BCS test, strip, parse (blocks)", n, || {
        for _ in 0..n {
            let f = acars::split_frame(black_box(&frame)).unwrap();
            body.copy_from_slice(f.body);
            black_box(acars::check_and_repair(&mut body, f.bcs));
            acars::strip_parity(&mut body);
            black_box(Block::parse(&body).unwrap());
        }
    });
    let good = f.body.to_vec();
    measure("repair: one bit (blocks)", n, || {
        for i in 0..n {
            body.copy_from_slice(&good);
            body[i % good.len()] ^= 4;
            black_box(acars::check_and_repair(&mut body, f.bcs));
        }
    });
    measure("repair: three bad bytes (blocks)", n, || {
        for i in 0..n {
            body.copy_from_slice(&good);
            body[i % 30] ^= 1;
            body[40] ^= 8;
            body[60] ^= 64;
            black_box(acars::check_and_repair(&mut body, f.bcs));
        }
    });
    measure("repair: two bits in one byte (blocks)", n / 10, || {
        for i in 0..n / 10 {
            body.copy_from_slice(&good);
            body[i % good.len()] ^= 0x24;
            black_box(acars::check_and_repair(&mut body, f.bcs));
        }
    });
    let mut bin = [0u8; 64];
    measure(
        "ARINC 622 envelope find and ADS-C request (messages)",
        n,
        || {
            for _ in 0..n {
                let e = arinc622::find(black_box(ADS), "HB-JHM").unwrap();
                let d = e.binary(&mut bin).unwrap();
                let (_, g) = adsc::request(d).unwrap();
                black_box(g.count());
            }
        },
    );
    let mut report = Vec::new();
    adsc::Group::flight_id("QFA1")
        .unwrap()
        .encode(&mut report)
        .unwrap();
    adsc::Group::EarthReference {
        track_deg: 90.0,
        valid: true,
        gs_kt: 450.0,
        vs_fpm: 0.0,
    }
    .encode(&mut report)
    .unwrap();
    measure("ADS-C downlink groups (messages)", n, || {
        for _ in 0..n {
            black_box(adsc::groups(black_box(&report)).count());
        }
    });
    measure("text position (texts)", n, || {
        for _ in 0..n {
            black_box(textpos::read("H1", black_box(&text[10..])));
        }
    });
    acars::strip_parity(&mut body);
    body.copy_from_slice(&good);
    acars::strip_parity(&mut body);
    let b = Block::parse(&body).unwrap();
    measure("owned message with reading (messages)", n, || {
        for _ in 0..n {
            black_box(Message::from_block(black_box(&b), Quality::GOOD));
        }
    });
}
