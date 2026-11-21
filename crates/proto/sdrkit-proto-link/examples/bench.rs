//! Times the hot paths: `cargo run --release -p sdrkit-proto-link --example bench`. Each line is the best of several runs, in millions
//! of input items per second (audio samples, line bits, bytes or packets as the name says). For the demodulators the line also
//! reports how many times faster than real time they run at 48 kHz.

use sdrkit_proto_link::ax25::Frame;
use sdrkit_proto_link::csp::{Ax100, FLAG_RS, FLAG_SCRAMBLER};
use sdrkit_proto_link::hdlc::{Line, Receiver, Transmitter};
use sdrkit_proto_link::kiss::{self, Command, Decoder};
use sdrkit_proto_link::modem::{afsk1200_audio, g3ruh9600_audio, Afsk1200, G3ruh9600};
use sdrkit_proto_link::{aprs, Result};
use std::hint::black_box;
use std::time::Instant;

fn measure(name: &str, items: usize, realtime: Option<f64>, mut step: impl FnMut()) {
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
    let rate = items as f64 / best;
    match realtime {
        Some(r) => println!(
            "{name:40} {:10.2} M/s {:8.0} x real time",
            rate / 1e6,
            rate / r
        ),
        None => println!("{name:40} {:10.2} M/s", rate / 1e6),
    }
}

fn main() -> Result<()> {
    let f = Frame::from_tnc2(
        "N0CALL-9>APRS,WIDE1-1,WIDE2-1:!4903.50N/07201.75W-PHG5132 bench test frame",
    )?;
    let mut frame = Vec::new();
    f.encode(&mut frame)?;
    let frames: Vec<&[u8]> = vec![&frame; 8];

    let mut bits = Vec::new();
    Transmitter::new(Line::G3ruh).encode(&frames, 16, &mut bits);
    let mut rx = Receiver::ax25(Line::G3ruh);
    measure("hdlc receive, G3RUH line (bits)", bits.len(), None, || {
        rx.push_bits(black_box(&bits), |f| {
            black_box(f);
        });
    });

    let mut audio = Vec::new();
    afsk1200_audio(&frames, 30, 48_000.0, &mut audio)?;
    let mut afsk = Afsk1200::new(48_000.0)?;
    measure(
        "afsk1200 demodulator (samples)",
        audio.len(),
        Some(48_000.0),
        || {
            afsk.process(black_box(&audio), |f| {
                black_box(f);
            });
        },
    );

    let mut afsk_all = Afsk1200::new(48_000.0)?;
    measure(
        "afsk1200 with bad frames and tones (samples)",
        audio.len(),
        Some(48_000.0),
        || {
            afsk_all.process_frames(black_box(&audio), |f| {
                black_box(f);
            });
        },
    );

    let mut with_fcs = frame.clone();
    with_fcs.extend_from_slice(
        &sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC
            .checksum(&frame)
            .to_le_bytes(),
    );
    let last = 8 * with_fcs.len() - 9;
    measure(
        "hdlc repair, worst case: last pair (frames)",
        1,
        None,
        || {
            let mut b = with_fcs.clone();
            b[last / 8] ^= 1 << (last % 8);
            b[(last + 1) / 8] ^= 1 << ((last + 1) % 8);
            black_box(sdrkit_proto_link::hdlc::repair_ax25(&mut b, 2));
        },
    );

    let mut audio = Vec::new();
    g3ruh9600_audio(&frames, 30, 48_000.0, &mut audio)?;
    let mut g = G3ruh9600::new(48_000.0)?;
    measure(
        "g3ruh9600 demodulator (samples)",
        audio.len(),
        Some(48_000.0),
        || {
            g.process(black_box(&audio), |f| {
                black_box(f);
            });
        },
    );

    measure("ax25 frame parse (frames)", 1000, None, || {
        for _ in 0..1000 {
            black_box(Frame::parse(black_box(&frame)).ok());
        }
    });
    measure(
        "aprs parse, uncompressed position (packets)",
        1000,
        None,
        || {
            for _ in 0..1000 {
                black_box(aprs::parse(black_box(f.info), "APRS").ok());
            }
        },
    );
    let mice = [b'`', 100, 32, 59, 36, 89, 51, b'>', b'/', b'h', b'i'];
    measure("aprs parse, Mic-E (packets)", 1000, None, || {
        for _ in 0..1000 {
            black_box(aprs::parse(black_box(&mice), "S32U6T").ok());
        }
    });

    let mut stream = Vec::new();
    for _ in 0..64 {
        kiss::encode(0, Command::Data, &frame, &mut stream)?;
    }
    let mut d = Decoder::new(512);
    measure("kiss decode (bytes)", stream.len(), None, || {
        d.push_bytes(black_box(&stream), |f| {
            black_box(f);
        });
    });

    let mut ax = Ax100::new()?;
    let mut f5 = Vec::new();
    ax.mode5_encode(&[7u8; 200], &mut f5)?;
    let mut coded = f5[3..].to_vec();
    measure(
        "ax100 mode 5 decode, 8 byte errors (frames)",
        1,
        None,
        || {
            coded.copy_from_slice(&f5[3..]);
            for k in 0..8 {
                coded[k * 25] ^= 0x5A;
            }
            black_box(ax.mode5_decode(&mut coded, FLAG_RS | FLAG_SCRAMBLER).ok());
        },
    );
    Ok(())
}
