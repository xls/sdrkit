//! Checks that every `fec.*` component allocates nothing once running: each is built in a small graph, warmed up, then run for
//! many passes under an allocator that counts. (The counting allocator needs `unsafe impl GlobalAlloc`; it is defined in this
//! test only.)

mod fec_support;
use fec_support::*;

use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::conv::{codes, Encoder};
use sdrkit_algo_fec::ldpc::tables::dvb_s2;
use sdrkit_algo_fec::ldpc::LdpcCode;
use sdrkit_algo_fec::rs::{codes as rs_codes, Rs};
use sdrkit_algo_fec::turbo::Turbo;
use sdrkit_components::fec::{
    BchDecode, Deinterleave, Depuncture, Erasures, LdpcDecode, PunctureComponent, Quantize,
    RsDecode, TrellisDecode, TurboDecode, ViterbiDecode,
};
use sdrkit_components::testing::Source;
use sdrkit_core::{Complex32, ConfigError, Output, Value, Values};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Graph, Io, Kind, Port, Timed};
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

/// Runs `warm` passes, then counts the allocations of `count` more.
fn allocations(g: &mut Graph, warm: usize, count: usize) -> usize {
    g.configure_input(Format::iq(1.0, 0.0, 0.0))
        .expect("the graph configures");
    let mut out = Output::default();
    let empty: [Complex32; 0] = [];
    for _ in 0..warm {
        out.clear();
        g.process(&empty, &mut out);
    }
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..count {
        out.clear();
        g.process(&empty, &mut out);
    }
    ALLOCS.load(Ordering::Relaxed) - before
}

/// Emits an empty erasure list each pass (an empty list owns no memory).
struct NoErasures;

impl Component for NoErasures {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator("test.no_erasures", vec![Port::typed::<Erasures>("out")])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Erasures>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        io.items_out::<Erasures>(0).push(Timed {
            time: 0.0,
            value: Erasures(Vec::new()),
        });
    }
}

fn soft_stream(rng: &mut Rng, n: usize) -> Vec<i8> {
    rng.bits(n)
        .iter()
        .map(|&b| if b == 0 { 25 } else { -25 })
        .collect()
}

// One test function, so no other test thread allocates while counting.
#[test]
fn steady_state_allocates_nothing() {
    const WARM: usize = 50;
    const COUNT: usize = 1000;
    let mut rng = Rng(1);
    let mut cases: Vec<(&str, Graph, usize, usize)> = Vec::new();

    // Quantizer, adaptive and fixed.
    let x: Vec<f32> = (0..(WARM + COUNT) * 500)
        .map(|i| ((i * 7919) % 200) as f32 / 40.0 - 2.5)
        .collect();
    for scale in [0.0, 6.0] {
        let g = finish(
            Tb::new()
                .add(
                    "src",
                    Source::new(Format::new(Kind::Real, 4.0), Data::Real(x.clone()), 500),
                )
                .add("q", Quantize::default())
                .connect("src", "q")
                .set("q.scale", Value::F64(scale))
                .set("q.window", Value::I64(256)),
            "q",
        );
        cases.push(("fec.quantize", g, WARM, COUNT));
    }

    // Puncture, depuncture and a streaming Viterbi decoder.
    let bits = rng.bits((WARM + COUNT) * 300);
    let mut coded = Vec::new();
    Encoder::new(codes::DVB_K7).encode(&bits, &mut coded);
    let llr = llr_of(&coded, 30);
    let p = PunctureComponent::<i8>::default();
    let d = Depuncture::default();
    let (pp, dp) = (
        tv(&p, "pattern", txt("dvb_3_4")),
        tv(&d, "pattern", txt("dvb_3_4")),
    );
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::llr(4.0, None), Data::Llr(llr.clone()), 600),
            )
            .add("p", p)
            .add("d", d)
            .add("v", ViterbiDecode::default())
            .connect("src", "p")
            .connect("p", "d")
            .connect("d", "v")
            .set("p.pattern", pp)
            .set("d.pattern", dp),
        "v",
    );
    cases.push(("fec.puncture, fec.depuncture, fec.viterbi", g, WARM, COUNT));

    // Viterbi on whole frames.
    let v = ViterbiDecode::default();
    let ends = tv(&v, "ends", txt("open"));
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::llr(4.0, None), Data::Llr(llr), 600),
            )
            .add("v", v)
            .connect("src", "v")
            .set("v.frame", Value::I64(500))
            .set("v.ends", ends),
        "v",
    );
    cases.push(("fec.viterbi frames", g, WARM, COUNT));

    // Trellis.
    let levels: Vec<f32> = (0..(WARM + COUNT) * 600)
        .map(|i| ((i * 31) % 8) as f32 * 2.0 - 7.0)
        .collect();
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::new(Kind::Real, 4.0), Data::Real(levels), 600),
            )
            .add("t", TrellisDecode::default())
            .connect("src", "t"),
        "t.out",
    );
    cases.push(("fec.trellis", g, WARM, COUNT));
    // DMR trellis words, 98 levels each, 70 symbols a pass.
    let levels: Vec<f32> = (0..(WARM + COUNT) * 70)
        .map(|i| ((i * 13) % 4) as f32 * 2.0 - 3.0)
        .collect();
    let t = TrellisDecode::default();
    let name = tv(&t, "trellis", txt("dmr"));
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::new(Kind::Real, 4.0), Data::Real(levels), 70),
            )
            .add("t", t)
            .connect("src", "t")
            .set("t.trellis", name),
        "t.out",
    );
    cases.push(("fec.trellis dmr", g, WARM, COUNT));

    // Deinterleavers: Forney on packets, a permutation on soft bits.
    let pk: Vec<Vec<u8>> = (0..(WARM + COUNT) * 3).map(|_| rng.bytes(204)).collect();
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::new(Kind::Bytes, 0.0), bytes_data(&pk), 3),
            )
            .add("d", Deinterleave::on_bytes())
            .connect("src", "d"),
        "d",
    );
    cases.push(("fec.deinterleave forney", g, WARM, COUNT));
    let soft = soft_stream(&mut rng, (WARM + COUNT) * 704);
    let d = Deinterleave::default();
    let (kind, name) = (
        tv(&d, "kind", txt("permutation")),
        tv(&d, "name", txt("nicam")),
    );
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::llr(4.0, None), Data::Llr(soft), 500),
            )
            .add("d", d)
            .connect("src", "d")
            .set("d.kind", kind)
            .set("d.name", name),
        "d",
    );
    cases.push(("fec.deinterleave permutation", g, WARM, COUNT));

    // Reed-Solomon, with errors, without and with an erasure input.
    let rs = Rs::new(rs_codes::DVB_204_188).unwrap();
    let words: Vec<Vec<u8>> = (0..(WARM + COUNT) * 2)
        .map(|i| {
            let mut w = rng.bytes(204);
            rs.encode_word(&mut w).unwrap();
            for p in rng.positions(204, i % 9) {
                w[p] ^= 0x11;
            }
            w
        })
        .collect();
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::new(Kind::Bytes, 0.0), bytes_data(&words), 2),
            )
            .add("rs", RsDecode::default())
            .connect("src", "rs"),
        "rs.out",
    );
    cases.push(("fec.rs_decode", g, WARM, COUNT));
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::new(Kind::Bytes, 0.0), bytes_data(&words), 1),
            )
            .add("er", NoErasures)
            .add("rs", RsDecode::default())
            .connect("src", "rs.in")
            .connect("er", "rs.erasures"),
        "rs.out",
    );
    cases.push(("fec.rs_decode with erasures", g, WARM, COUNT));

    // BCH, LDPC and turbo frames, with errors (fewer passes: each pass decodes a whole frame).
    const FRAMES: usize = 120;
    let bch = Bch::new(14, 0x402B, 7200, 12).unwrap();
    let mut stream = Vec::new();
    for _ in 0..FRAMES {
        let mut w = rng.bits(7200);
        bch.encode(&mut w).unwrap();
        for p in rng.positions(7200, 5) {
            w[p] ^= 1;
        }
        stream.extend_from_slice(&w);
    }
    let b = BchDecode::default();
    let bch_code = tv(&b, "code", txt("dvb_s2_short_1_2"));
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::hard_bits(4.0), Data::HardBits(stream), 7200),
            )
            .add("bch", b)
            .connect("src", "bch")
            .set("bch.code", bch_code),
        "bch.out",
    );
    cases.push(("fec.bch_decode", g, 20, FRAMES - 20));

    let code = LdpcCode::from_dvb(&dvb_s2::S2_S_1_2).unwrap();
    let mut stream = Vec::new();
    for _ in 0..FRAMES {
        let info = rng.bits(code.k());
        let mut cw = vec![0u8; code.n()];
        code.encode(&info, &mut cw).unwrap();
        let mut l = llr_of(&cw, 12);
        for p in rng.positions(code.n(), 200) {
            l[p] = -l[p];
        }
        stream.extend_from_slice(&l);
    }
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::llr(4.0, Some(4.0)), Data::Llr(stream), code.n()),
            )
            .add("ldpc", LdpcDecode::default())
            .connect("src", "ldpc"),
        "ldpc.out",
    );
    cases.push(("fec.ldpc_decode", g, 20, FRAMES - 20));

    let t = Turbo::new(1024).unwrap();
    let mut stream = Vec::new();
    for _ in 0..FRAMES {
        let mut coded = vec![0u8; 3 * 1028];
        t.encode(&rng.bits(1024), &mut coded).unwrap();
        stream.extend(llr_of(&coded, 20));
    }
    let t = TurboDecode::default();
    let stop = tv(&t, "stop", txt("stable"));
    let g = finish(
        Tb::new()
            .add(
                "src",
                Source::new(Format::llr(4.0, None), Data::Llr(stream), 3 * 1028),
            )
            .add("t", t)
            .connect("src", "t")
            .set("t.k", Value::I64(1024))
            .set("t.stop", stop),
        "t.out",
    );
    cases.push(("fec.turbo_decode", g, 20, FRAMES - 20));

    for (name, mut g, warm, count) in cases {
        assert_eq!(allocations(&mut g, warm, count), 0, "{name} allocated");
    }
}
