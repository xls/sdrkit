//! Times the hot paths: `cargo run --release -p sdrkit-util-bits --example bench`. Each line is the best of several runs, in millions
//! of input items per second (bytes, bits or LLRs as the name says).

use sdrkit_util_bits::crc::catalog::{specs, CRC16_IBM_SDLC, CRC32_ISO_HDLC};
use sdrkit_util_bits::crc::Sliced32;
use sdrkit_util_bits::hdlc::{hdlc_encode, hdlc_encode_without_fcs, HdlcDeframer};
use sdrkit_util_bits::interleave::{DelayLanes, Permutation};
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::sync::{SoftSync, SyncWord};
use sdrkit_util_bits::{pack, seq, BitOrder, BitReader, Pad};
use std::hint::black_box;
use std::time::Instant;

static FAST32: Sliced32 = Sliced32::new(specs::CRC32_ISO_HDLC);

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
    println!("{name:44} {:10.1} M/s", items as f64 / best / 1e6);
}

fn main() {
    let data: Vec<u8> = (0..65_536u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let mut bits = Vec::with_capacity(data.len() * 8);
    pack::unpack(&data, BitOrder::Msb, &mut bits);

    measure("crc32_iso_hdlc table (bytes)", data.len(), || {
        black_box(CRC32_ISO_HDLC.checksum(black_box(&data)));
    });
    measure("crc32_iso_hdlc slicing-by-8 (bytes)", data.len(), || {
        black_box(FAST32.checksum(black_box(&data)));
    });
    measure("crc16_ibm_sdlc on hard bits (bits)", bits.len(), || {
        black_box(CRC16_IBM_SDLC.bits(black_box(&bits)));
    });
    let mut out = Vec::with_capacity(bits.len());
    measure("unpack msb (bytes)", data.len(), || {
        out.clear();
        pack::unpack(black_box(&data), BitOrder::Msb, &mut out);
    });
    let mut packed = Vec::with_capacity(data.len());
    measure("pack lsb (bits)", bits.len(), || {
        packed.clear();
        pack::pack(black_box(&bits), BitOrder::Lsb, Pad::Zero, &mut packed);
    });
    measure(
        "bit reader, 13-bit fields (fields)",
        data.len() * 8 / 13,
        || {
            let mut r = BitReader::new(&data);
            let mut acc = 0u64;
            while r.left() >= 13 {
                acc ^= r.u(13);
            }
            black_box(acc);
        },
    );
    let mut buf = data.clone();
    let mut scr = Additive::new(seq::DVB_DISPERSAL);
    measure("additive scrambler, bytes (bytes)", buf.len(), || {
        scr.reset();
        scr.apply_bytes(&mut buf, BitOrder::Msb);
    });
    measure("ccsds period table (bytes)", buf.len(), || {
        seq::CCSDS.apply(&mut buf, 0);
    });
    let mut lanes = DelayLanes::forney(12, 17, true, 0u8);
    measure("forney 12x17 (bytes)", buf.len(), || {
        lanes.process(&mut buf);
    });
    let perm = Permutation::dvbt_symbol(true).unwrap();
    let llr_in = vec![3i8; perm.len()];
    let mut llr_out = vec![0i8; perm.len()];
    measure("dvb-t 8k symbol permutation (i8 llrs)", perm.len(), || {
        perm.apply(black_box(&llr_in), &mut llr_out).unwrap();
    });
    let mut sync = SyncWord::new(0x1ACF_FC1D, 32, 3)
        .unwrap()
        .with_inverted(true);
    let mut hits = Vec::with_capacity(1024);
    measure("sync word scan, 32 bits (bits)", bits.len(), || {
        hits.clear();
        sync.scan(black_box(&bits), &mut hits);
    });
    let mut ss = SoftSync::new(0x1ACF_FC1D, 32, 0.95).unwrap();
    let llrs: Vec<i8> = bits
        .iter()
        .map(|&b| if b == 0 { 40 } else { -40 })
        .collect();
    let mut shits = Vec::with_capacity(1024);
    measure("soft sync, 32 bits (llrs)", llrs.len(), || {
        shits.clear();
        ss.scan(black_box(&llrs), &mut shits);
    });
    let mut hdlc_bits = Vec::new();
    let frames: Vec<&[u8]> = data.chunks(250).collect();
    hdlc_encode(&frames, &CRC16_IBM_SDLC, 1, &mut hdlc_bits);
    let mut d = HdlcDeframer::new(&CRC16_IBM_SDLC, 1, 300);
    measure("hdlc deframer (bits)", hdlc_bits.len(), || {
        let mut n = 0;
        for &b in &hdlc_bits {
            n += d.push(b).is_some() as usize;
        }
        black_box(n);
    });
    let mut hdlc32_bits = Vec::new();
    hdlc_encode(&frames, &CRC32_ISO_HDLC, 1, &mut hdlc32_bits);
    let mut d32 = HdlcDeframer::new(&CRC32_ISO_HDLC, 1, 300);
    measure(
        "hdlc deframer, 32-bit fcs (bits)",
        hdlc32_bits.len(),
        || {
            let mut n = 0;
            for &b in &hdlc32_bits {
                n += d32.push(b).is_some() as usize;
            }
            black_box(n);
        },
    );
    let mut nofcs_bits = Vec::new();
    hdlc_encode_without_fcs(&frames, 1, &mut nofcs_bits);
    let mut dn = HdlcDeframer::without_fcs(1, 300).unwrap();
    measure("hdlc deframer, no fcs (bits)", nofcs_bits.len(), || {
        let mut n = 0;
        for &b in &nofcs_bits {
            n += dn.push(b).is_some() as usize;
        }
        black_box(n);
    });
    let by_key = sdrkit_util_bits::crc::catalog::by_key("crc32_mpeg_2")
        .unwrap()
        .engine;
    measure(
        "crc32_mpeg_2 by key, any-width engine (bytes)",
        data.len(),
        || {
            black_box(by_key.checksum(black_box(&data)));
        },
    );
    let mut buf = data.clone();
    for key in ["dvb_dispersal", "ccsds"] {
        let mut s = seq::ByteSequence::named(key).unwrap();
        measure(&format!("byte sequence {key} (bytes)"), buf.len(), || {
            s.reset();
            s.apply(black_box(&mut buf));
        });
    }
    let mut gold = vec![0u8; 65_536];
    measure("3gpp gold sequence (bits)", gold.len(), || {
        seq::gold_3gpp(0x1234, &mut gold);
    });
    measure("dvb-s2 pl scrambling (symbols)", gold.len(), || {
        seq::gold_dvbs2(1234, 0, &mut gold).unwrap();
    });
}
