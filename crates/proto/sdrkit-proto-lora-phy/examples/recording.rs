//! Runs one LoRa channel of a recording of 16-bit IQ samples through the receive chain and prints every frame:
//! `cargo run --profile shipped -p sdrkit-proto-lora-phy --example recording -- <file.sigmf-data> [--rate 2100000]
//! [--offset -300000] [--rf 919875000] [--sf 11 --bw 250000 --preamble 16 --sync 43]`.
//!
//! `--offset` is the channel centre less the centre of the recording, Hz. Each line is the start time, the payload length, the
//! CRC result, the SNR, the level, the carrier offset at the start and at the end of the frame, its rate of change, the clock
//! error, and the clock error that a crystal with this carrier offset at `--rf` has.

use sdrkit_core::Complex32;
use sdrkit_proto_lora_phy::{ChannelRx, DetectorConfig};
use std::io::Read;

fn arg<T: std::str::FromStr>(name: &str, default: T) -> T {
    let a: Vec<String> = std::env::args().collect();
    a.iter()
        .position(|x| x == name)
        .and_then(|i| a.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: recording <file of 16-bit IQ samples> [options]");
        return;
    };
    let rate: f64 = arg("--rate", 2.1e6);
    let offset: f64 = arg("--offset", -300e3);
    let rf: f64 = arg("--rf", 919.875e6);
    let bw: f64 = arg("--bw", 250e3);
    let mut d = DetectorConfig::new(arg("--sf", 11), bw, arg("--sync", 0x2b));
    d.preamble = arg("--preamble", 16);
    let mut rx = match ChannelRx::new(rate, offset, bw, vec![d]) {
        Ok(rx) => rx,
        Err(e) => {
            eprintln!("{e:?}");
            return;
        }
    };
    let mut file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{path}: {e}");
            return;
        }
    };
    let mut bytes = vec![0u8; 4 * 65536];
    let mut x = Vec::with_capacity(65536);
    let mut out = Vec::new();
    println!("start s | bytes | crc | snr dB | level dBFS | carrier Hz | carrier at end Hz | Hz/s | clock ppm | crystal ppm | repaired");
    loop {
        let mut got = 0;
        while got < bytes.len() {
            match file.read(&mut bytes[got..]) {
                Ok(0) | Err(_) => break,
                Ok(k) => got += k,
            }
        }
        if got < 4 {
            break;
        }
        x.clear();
        x.extend(bytes[..got].as_chunks::<4>().0.iter().map(|c| {
            Complex32::new(
                i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0,
                i16::from_le_bytes([c[2], c[3]]) as f32 / 32768.0,
            )
        }));
        out.clear();
        rx.process(&x, &mut out);
        for (f, _) in &out {
            println!(
                "{:.2} | {} | {} | {:.1} | {:.1} | {:.0} | {:.0} | {:.1} | {:.1} | {:.1} | {}",
                f.start as f64 / rate,
                f.payload.len(),
                if f.crc_ok { "good" } else { "bad" },
                f.snr_db,
                f.level_db,
                f.cfo_hz,
                f.cfo_end_hz,
                f.cfo_rate_hz_s,
                f.ppm,
                -f.cfo_hz / rf * 1e6,
                f.list_fixed || f.jumps > 0,
            );
        }
    }
}
