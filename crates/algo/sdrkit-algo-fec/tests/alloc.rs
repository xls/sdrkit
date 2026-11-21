//! Checks that every steady-state decode path allocates nothing: each runs 1,000 times (fewer for the large decoders) after its
//! buffers are made, under an allocator that counts. (The counting allocator needs `unsafe impl GlobalAlloc`.)
#![allow(unsafe_code)]

use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::conv::{
    codes, tcm, tcm::TcmCodec, Depuncturer, Encoder, Ends, Fano, FanoParams, LabelMetrics, Trellis,
    TrellisDecoder, Viterbi,
};
use sdrkit_algo_fec::cyclic::MajorityLogic;
use sdrkit_algo_fec::interleave::DelayLanes;
use sdrkit_algo_fec::ldpc::{
    tables::dvb_s2, tables::wsjt, BpDecoder, InfoAt, LdpcCode, LdpcDecoder, MinSum, Osd,
};
use sdrkit_algo_fec::polar::{Polar, PolarParams};
use sdrkit_algo_fec::ratematch::{LteCode, LteRateMatch};
use sdrkit_algo_fec::rs::{codes as rscodes, Rs};
use sdrkit_algo_fec::small::{Chase, SmallCode};
use sdrkit_algo_fec::turbo::Turbo;
use sdrkit_algo_fec::Quantizer;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn count(name: &str, times: usize, mut f: impl FnMut()) {
    f();
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..times {
        f();
    }
    let n = ALLOCS.load(Ordering::Relaxed) - before;
    assert_eq!(n, 0, "{name} allocated {n} times");
}

fn llr_of(bits: &[u8], mag: i8) -> Vec<i8> {
    bits.iter()
        .enumerate()
        .map(|(i, &b)| {
            let v = if b == 0 { mag } else { -mag };
            if i % 37 == 5 {
                -v / 2
            } else {
                v
            }
        })
        .collect()
}

// One test, so no other test thread allocates while counting.
#[test]
fn steady_state_paths_do_not_allocate() {
    // Viterbi, whole frame and streaming.
    let bits: Vec<u8> = (0..500).map(|i| (i * 7 % 5 > 2) as u8).collect();
    let mut coded = Vec::new();
    let mut enc = Encoder::new(codes::DVB_K7);
    enc.encode(&bits, &mut coded);
    enc.terminate(&mut coded);
    let llr = llr_of(&coded, 30);
    let mut v = Viterbi::new(codes::DVB_K7, 600).unwrap();
    let mut out = vec![0u8; 600];
    count("viterbi decode", 1000, || {
        v.decode(&llr, Ends::Zero, &mut out).unwrap();
    });
    count("viterbi tail-biting", 200, || {
        v.decode(&llr[..1000], Ends::TailBiting, &mut out).unwrap();
    });
    let mut stream = Vec::with_capacity(1 << 20);
    count("viterbi push", 1000, || {
        v.push(&llr, &mut stream);
    });
    let mut g = Viterbi::new(codes::GSM_K5, 600).unwrap();
    count("viterbi generic", 300, || {
        g.decode(&llr[..1000], Ends::Open, &mut out).unwrap();
    });
    let mut d = Depuncturer::new(codes::DVB_3_4);
    let mut dep = vec![0i8; 4000];
    count("depuncture", 1000, || {
        d.push_into(&llr[..600], &mut dep).unwrap();
    });
    let q = Quantizer::default();
    let soft: Vec<f32> = llr.iter().map(|&x| x as f32 * 0.1).collect();
    let mut qo = vec![0i8; soft.len()];
    count("quantize", 1000, || {
        q.quantize(&soft, 3.0, &mut qo).unwrap();
    });
    // Fano.
    let mut fano = Fano::new(codes::WSPR_K32, 81, FanoParams::default()).unwrap();
    let wbits: Vec<u8> = bits[..50].to_vec();
    let mut wc = Vec::new();
    let mut we = Encoder::new(codes::WSPR_K32);
    we.encode(&wbits, &mut wc);
    we.terminate(&mut wc);
    let wl: Vec<f32> = wc
        .iter()
        .map(|&b| if b == 0 { 3.0 } else { -3.0 })
        .collect();
    count("fano", 100, || {
        fano.decode(&wl, &mut out[..50]).unwrap();
    });
    // Trellis.
    let t = Trellis::atsc_8vsb();
    let mut td = TrellisDecoder::new(&t, 1000).unwrap();
    let metrics: Vec<i32> = (0..4000i32).map(|i| -(i * 37 % 11)).collect();
    count("trellis", 300, || {
        td.decode(
            &mut LabelMetrics {
                labels: 4,
                metrics: &metrics,
            },
            1000,
            Ends::Open,
            None,
            &mut out[..],
        )
        .ok();
    });
    // DMR / P25 4-level trellis words, hard and soft.
    let mut tcm = TcmCodec::new(tcm::DMR_3_4).unwrap();
    let info: Vec<u8> = bits[..144].to_vec();
    let mut sent = [0u8; 196];
    tcm.encode(&info, &mut sent).unwrap();
    let levels: Vec<f32> = sent
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| tcm::dibit_symbol((c[0] << 1) | c[1]) as f32 * 0.9)
        .collect();
    let mut got = [0u8; 144];
    count("tcm", 300, || {
        tcm.encode(&info, &mut sent).unwrap();
        tcm.decode_bits(&sent, &mut got).unwrap();
        tcm.decode_levels(&levels, &mut got).unwrap();
    });
    // Reed-Solomon.
    let mut rs = Rs::new(rscodes::CCSDS_255_223).unwrap();
    let mut word = vec![0u8; 255];
    for (i, b) in word[..223].iter_mut().enumerate() {
        *b = i as u8;
    }
    rs.encode_word(&mut word).unwrap();
    let sent = word.clone();
    count("rs decode", 1000, || {
        word.copy_from_slice(&sent);
        for i in [3, 60, 100, 200] {
            word[i] ^= 0x55;
        }
        rs.decode(&mut word, &[10, 11]).unwrap();
    });
    let mut frame = vec![0u8; 255 * 4];
    count("rs interleaved", 300, || {
        rs.decode_interleaved(&mut frame, 4, &[]).unwrap();
    });
    // BCH (DVB-S2 normal).
    let mut bch = Bch::new(16, 0x1002D, 16_200, 12).unwrap();
    let mut bw = vec![0u8; 16_200];
    bch.encode(&mut bw).unwrap();
    count("bch decode", 100, || {
        bw[5] ^= 1;
        bw[9000] ^= 1;
        bch.decode(&mut bw).unwrap();
    });
    // Short codes, Chase, majority logic.
    let gol = SmallCode::golay_24_12();
    let rel = [1.0f32; 24];
    count("small codes", 1000, || {
        std::hint::black_box(gol.decode(0x123_456));
        std::hint::black_box(gol.decode_soft(0x123_456, &rel, Chase::default()));
    });
    let mut ml = MajorityLogic::isdb_t_tmcc().unwrap();
    let mut mw = vec![0u8; 184];
    count("majority logic", 100, || {
        ml.decode(&mut mw).unwrap();
    });
    // LDPC: min-sum, BP and OSD.
    let code = LdpcCode::from_dvb(&dvb_s2::S2_S_1_2).unwrap();
    let mut ld = LdpcDecoder::new(&code, MinSum::default());
    let info: Vec<u8> = (0..code.k()).map(|i| (i % 3 == 0) as u8).collect();
    let mut cw = vec![0u8; code.n()];
    code.encode(&info, &mut cw).unwrap();
    let ll = llr_of(&cw, 12);
    let mut lo = vec![0u8; code.n()];
    count("ldpc min-sum", 20, || {
        ld.decode(&ll, 30, &mut lo).unwrap();
    });
    let ft8 = LdpcCode::from_checks("ft8", 174, 91, &wsjt::FT8_CHECKS, InfoAt::Start).unwrap();
    let mut bp = BpDecoder::new(&ft8);
    let mut osd = Osd::new(&ft8).unwrap();
    let fl = [2.0f32; 174];
    let mut fo = [0u8; 174];
    count("bp and osd", 50, || {
        bp.decode(&fl, 10, &mut fo).unwrap();
        osd.decode(&fl, 2, &mut fo).unwrap();
    });
    // Turbo and rate matching.
    let mut tu = Turbo::new(512).unwrap();
    let tb: Vec<u8> = (0..512).map(|i| (i % 5 == 1) as u8).collect();
    let mut ts = vec![0u8; 3 * 516];
    tu.encode(&tb, &mut ts).unwrap();
    let tl = llr_of(&ts, 20);
    let mut to = vec![0u8; 512];
    count("turbo", 20, || {
        tu.decode(&tl, 4, |b| b == &tb[..], &mut to).unwrap();
    });
    let rm = LteRateMatch::new(LteCode::Turbo, 516, 0).unwrap();
    let mut e = vec![0u8; 2000];
    let mut soft3 = vec![0i8; 3 * 516];
    let el = llr_of(&e, 5);
    count("rate matching", 1000, || {
        rm.rate_match(&ts, 1, rm.buffer_len(), &mut e).unwrap();
        rm.dematch(&el, 1, rm.buffer_len(), &mut soft3).unwrap();
    });
    // Polar.
    let mut p = Polar::new(PolarParams::PBCH, 8).unwrap();
    let pb: Vec<u8> = (0..56).map(|i| (i % 3 == 0) as u8).collect();
    let mut pe = [0u8; 864];
    p.encode(&pb, &mut pe).unwrap();
    let pl: Vec<f32> = pe
        .iter()
        .map(|&b| if b == 0 { 1.0 } else { -1.0 })
        .collect();
    let mut po = [0u8; 56];
    count("polar", 20, || {
        p.decode(&pl, |c| c == &pb[..], &mut po).unwrap();
        p.encode(&pb, &mut pe).unwrap();
    });
    // Interleaver.
    let mut il = DelayLanes::forney(12, 17, true, 0u8);
    let mut buf = vec![7u8; 4096];
    count("forney", 1000, || {
        il.process(&mut buf);
    });
}
