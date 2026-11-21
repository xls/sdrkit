//! Times the hot paths: `cargo run --release -p sdrkit-proto-ccsds --example bench`. Each line is the best of several runs, in millions
//! of input items per second (code symbols, bits, bytes or samples as the name says).

use sdrkit_algo_fec::conv::{codes, Encoder};
use sdrkit_proto_ccsds::channel::{ChannelCoder, ChannelConfig, ChannelDecoder};
use sdrkit_proto_ccsds::codesync::{CodeConfig, CodeSync};
use sdrkit_proto_ccsds::framesync::{FrameSync, SyncConfig};
use sdrkit_proto_ccsds::wordsync::{WordSync, WordSyncConfig};
use sdrkit_proto_ccsds::packet::{self, Assembler, AssemblerConfig, PrimaryHeader, SeqFlags};
use sdrkit_proto_ccsds::{aos, rice};
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
    println!("{name:44} {:10.2} M/s", items as f64 / best / 1e6);
}

fn main() {
    // About twenty CADUs with packets, as a GOES link sends them.
    let layout = aos::Layout::RS_I4;
    let mut pkts = Vec::new();
    for i in 0..40u16 {
        packet::build(
            &PrimaryHeader::telemetry(77, SeqFlags::Unsegmented, i),
            &[i as u8; 400],
            &mut pkts,
        )
        .unwrap();
    }
    let zones = packet::pack_zones(&[&pkts], layout.data_zone_len()).unwrap();
    let mut coder = ChannelCoder::new(ChannelConfig::rs_223(4)).unwrap();
    let mut stream = Vec::new();
    let mut frame = vec![0u8; layout.frame_len];
    for (i, (fh, z)) in zones.iter().enumerate() {
        let h = aos::PrimaryHeader {
            vcid: 2,
            counter: i as u32,
            ..aos::PrimaryHeader::default()
        };
        layout.build(&h, &[], *fh, z, &mut frame).unwrap();
        coder.encode(&frame, &mut stream).unwrap();
    }
    let bits: Vec<u8> = stream
        .iter()
        .flat_map(|b| (0..8).rev().map(move |k| (b >> k) & 1))
        .collect();
    let mut coded = Vec::new();
    Encoder::new(codes::CCSDS_K7).encode(&bits, &mut coded);
    let llr: Vec<i8> = coded
        .iter()
        .map(|&c| if c == 0 { 40 } else { -40 })
        .collect();

    let mut out = Vec::with_capacity(bits.len() + 4096);
    measure("code sync + Viterbi K=7 (code symbols)", llr.len(), || {
        let mut cs = CodeSync::new(CodeConfig::GEO).unwrap();
        out.clear();
        cs.push(black_box(&llr), &mut out);
    });
    measure("frame sync on hard bits (bits)", bits.len(), || {
        let mut fs = FrameSync::new(SyncConfig::GEO).unwrap();
        let mut n = 0;
        fs.push_bits(black_box(&bits), |_| n += 1);
        black_box(n);
    });
    // CADUs of 8192 bits behind the marker, as an uncoded BPSK link gives them, and bits with no marker.
    let rules = WordSyncConfig {
        word: 0x1ACF_FC1D,
        word_bits: 32,
        frame_bits: 8192,
        acquire_errors: 2,
        track_errors: 8,
        complement: true,
        max_misses: 3,
    };
    let mut s = 1u32;
    let mut noise = |n: usize| -> Vec<u8> {
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (s >> 24) as u8 & 1
            })
            .collect()
    };
    let mut framed = Vec::new();
    for _ in 0..40 {
        framed.extend((0..32).rev().map(|k| (rules.word >> k) as u8 & 1));
        framed.extend(noise(8192 - 32));
    }
    let unframed = noise(framed.len());
    let mut ws = WordSync::new(rules).unwrap();
    measure("word sync, locked (bits)", framed.len(), || {
        let mut n = 0;
        ws.push_bits(black_box(&framed), |_| n += 1);
        black_box(n);
    });
    let mut ws = WordSync::new(rules).unwrap();
    measure("word sync, searching (bits)", unframed.len(), || {
        let mut n = 0;
        ws.push_bits(black_box(&unframed), |_| n += 1);
        black_box(n);
    });
    let body: Vec<u8> = stream[4..1024].to_vec();
    let mut dec = ChannelDecoder::new(ChannelConfig::rs_223(4)).unwrap();
    let mut work = body.clone();
    measure("derandomise + RS(255,223) I=4, clean (bytes)", 1020, || {
        work.copy_from_slice(&body);
        black_box(dec.decode(&mut work).unwrap());
    });
    let mut noisy = body.clone();
    for e in 0..64 {
        noisy[(e * 15 + 1) % 1020] ^= 0x5A;
    }
    measure(
        "derandomise + RS(255,223) I=4, 64 errors (bytes)",
        1020,
        || {
            work.copy_from_slice(&noisy);
            black_box(dec.decode(&mut work).unwrap());
        },
    );
    let mut asm = Assembler::new(AssemblerConfig::default());
    measure(
        "M_PDU packet reassembly (zone bytes)",
        zones.len() * layout.data_zone_len(),
        || {
            let mut n = 0;
            asm.reset();
            for (i, (fh, z)) in zones.iter().enumerate() {
                asm.push_mpdu(2, Some(i as u32), *fh, z, |_| n += 1);
            }
            black_box(n);
        },
    );
    let p = rice::Params::new(10, 16, 170).unwrap();
    let line: Vec<u32> = (0..2712)
        .map(|i| ((i * 7 + (i * i) % 13) % 1024) as u32)
        .collect();
    let mut c = Vec::new();
    rice::encode(&line, &p, &mut c).unwrap();
    let mut samples = vec![0u32; 2712];
    measure(
        "Rice decode, 10-bit GOES line (samples)",
        line.len(),
        || {
            black_box(rice::decode(black_box(&c), &p, &mut samples).unwrap());
        },
    );
}
