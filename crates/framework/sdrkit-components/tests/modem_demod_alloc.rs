//! Checks that every `modem_demod` component allocates nothing in steady state: each runs in a small graph, is warmed up, then
//! runs many more passes under an allocator that counts. (The counting allocator needs `unsafe impl GlobalAlloc`; it is defined
//! in this test only.)

mod set_util;
use set_util::Tb;

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_modem::chirp::Dechirp;
use sdrkit_algo_modem::dsss::BARKER_11;
use sdrkit_algo_modem::ofdm::OfdmLayout;
use sdrkit_algo_sim::modulate::{Afsk, Cpm};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::modem::demod::{
    DechirpDemod, Despread, FskDemod, GmskDemod, OfdmDemod, OfdmSync, SyncSearch, ToneDetect,
};
use sdrkit_core::{Complex32, ConfigError, Output, Value, Values};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{
    components::NullSink, no_values, Component, ComponentInfo, Format, Graph, Io, Kind, Port,
};
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

/// A generator that emits its data over and over, `per_pass` items a pass (and allocates nothing once its output has grown).
struct Loop {
    format: Format,
    data: Data,
    pos: usize,
    per_pass: usize,
}

impl Component for Loop {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator("test.loop", vec![Port::new("out", self.format.kind)])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![self.format.clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let mut left = self.per_pass;
        while left > 0 {
            let n = self.data.len();
            let k = left.min(n - self.pos);
            let (a, b) = (self.pos, self.pos + k);
            match (&mut io.output(0).data, &self.data) {
                (Data::Real(o), Data::Real(d)) => o.extend_from_slice(&d[a..b]),
                (Data::Llr(o), Data::Llr(d)) => o.extend_from_slice(&d[a..b]),
                (Data::Symbols(o), Data::Symbols(d)) => o.extend_from_slice(&d[a..b]),
                _ => unreachable!(),
            }
            self.pos = if b == n { 0 } else { b };
            left -= k;
        }
    }
}

fn with(mut b: Tb, id: &str, set: &[(&str, Value)]) -> Tb {
    for (k, v) in set {
        b = b.set(&format!("{id}.{k}"), v.clone());
    }
    b
}

/// A graph feeding `comp` (output `port`) from the graph's IQ input.
fn iq_graph(comp: impl Component + 'static, port: &str, set: &[(&str, Value)]) -> Graph {
    let b = Tb::new()
        .add("c", comp)
        .add("sink", NullSink::default())
        .input("c")
        .connect(&format!("c.{port}"), "sink");
    with(b, "c", set).build().unwrap()
}

/// A graph feeding `comp` from a looping source.
fn loop_graph(src: Loop, comp: impl Component + 'static, set: &[(&str, Value)]) -> Graph {
    let b = Tb::new()
        .add("src", src)
        .add("c", comp)
        .add("sink", NullSink::default())
        .connect("src", "c")
        .connect("c.out", "sink");
    with(b, "c", set).build().unwrap()
}

/// Allocations over `passes` passes of `x` (cyclically, `piece` samples a pass) after `warm` passes.
fn allocations(
    g: &mut Graph,
    input: Format,
    x: &[Complex32],
    piece: usize,
    warm: usize,
    passes: usize,
) -> usize {
    g.configure_input(input).unwrap();
    let mut out = Output::default();
    let mut pos = 0;
    let run = |g: &mut Graph, out: &mut Output, pos: &mut usize| {
        if x.is_empty() {
            out.clear();
            g.process(&[], out);
            return;
        }
        let end = (*pos + piece).min(x.len());
        out.clear();
        g.process(&x[*pos..end], out);
        *pos = if end == x.len() { 0 } else { end };
    };
    for _ in 0..warm {
        run(g, &mut out, &mut pos);
    }
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..passes {
        run(g, &mut out, &mut pos);
    }
    ALLOCS.load(Ordering::Relaxed) - before
}

fn bits(n: usize, seed: u64) -> Vec<u8> {
    let mut r = Rng::new(seed);
    (0..n).map(|_| r.bit()).collect()
}

fn cpm(mut m: Cpm, levels: &[f32]) -> Vec<Complex32> {
    let mut x = Vec::new();
    m.push(levels, &mut x);
    x
}

#[test]
fn every_component_runs_without_allocating() {
    let b = bits(20_000, 1);
    let a: Vec<f32> = b.iter().map(|&v| 1.0 - 2.0 * v as f32).collect();
    let iq = |rate: f64| Format::iq(rate, 0.0, 0.0);

    // FSK by the correlators and by the discriminator (4 levels), GMSK by both methods.
    let fsk = cpm(Cpm::cpfsk(1.0, 10.0).unwrap(), &a);
    let mut g = iq_graph(
        FskDemod::default(),
        "out",
        &[
            ("shift_hz", Value::F64(1200.0)),
            ("baud", Value::F64(1200.0)),
        ],
    );
    assert_eq!(
        allocations(&mut g, iq(12_000.0), &fsk, 1000, 400, 1000),
        0,
        "fsk correlator"
    );
    let four: Vec<f32> = b
        .chunks(2)
        .map(|p| [3.0, 1.0, -1.0, -3.0][(p[0] * 2 + p[1]) as usize])
        .collect();
    let c4 = cpm(
        Cpm::new(sdrkit_algo_dsp::pulse::Shape::Rect, 1.0, 0.25, 10.0).unwrap(),
        &four,
    );
    let mut g = iq_graph(
        FskDemod::default(),
        "out",
        &[
            ("detector", Value::Text("discriminator".into())),
            ("levels", Value::F64(4.0)),
            ("deviation_hz", Value::F64(1800.0)),
            ("baud", Value::F64(4800.0)),
        ],
    );
    assert_eq!(
        allocations(&mut g, iq(48_000.0), &c4, 1000, 300, 1000),
        0,
        "fsk discriminator"
    );
    let gm = cpm(Cpm::gmsk(0.3, 4.0).unwrap(), &a);
    for method in ["discriminator", "mlse"] {
        let mut g = iq_graph(
            GmskDemod::default(),
            "out",
            &[
                ("baud", Value::F64(9600.0)),
                ("bt", Value::F64(0.3)),
                ("method", Value::Text(method.into())),
            ],
        );
        assert_eq!(
            allocations(&mut g, iq(38_400.0), &gm, 1000, 300, 1000),
            0,
            "gmsk {method}"
        );
    }

    // AFSK audio and tones from a looping source.
    let mut afsk = Afsk::new(48_000.0, 1200.0, 1200.0, 2200.0).unwrap();
    let mut audio = Vec::new();
    afsk.push(&b[..4000], &mut audio);
    let src = |data: Data, format: Format| Loop {
        format,
        data,
        pos: 0,
        per_pass: 1000,
    };
    let mut g = loop_graph(
        src(Data::Real(audio.clone()), Format::new(Kind::Real, 48_000.0)),
        FskDemod::default(),
        &[
            ("tones_hz", Value::Text("1200, 2200".into())),
            ("baud", Value::F64(1200.0)),
        ],
    );
    assert_eq!(allocations(&mut g, iq(1.0), &[], 0, 300, 1000), 0, "afsk");
    let mut g = loop_graph(
        src(Data::Real(audio), Format::new(Kind::Real, 48_000.0)),
        ToneDetect::default(),
        &[
            ("tones_hz", Value::Text("1200, 2200, 1700".into())),
            ("block_s", Value::F64(0.005)),
        ],
    );
    assert_eq!(allocations(&mut g, iq(1.0), &[], 0, 300, 1000), 0, "tones");

    // Sync search on soft bits and on soft symbols, with the marker in the stream.
    let mut sb = b[..4000].to_vec();
    for k in 0..32 {
        sb[100 + k] = ((0x1ACF_FC1Du32 >> (31 - k)) & 1) as u8;
        sb[2100 + k] = 1 ^ ((0x1ACF_FC1Du32 >> (31 - k)) & 1) as u8;
    }
    let llr: Vec<i8> = sb.iter().map(|&v| if v == 0 { 50 } else { -50 }).collect();
    let sym: Vec<Complex32> = sb
        .iter()
        .map(|&v| Complex32::new(1.0 - 2.0 * v as f32, 0.0))
        .collect();
    let set = [
        ("word", Value::Text("0x1ACFFC1D".into())),
        ("polarity", Value::Text("both".into())),
    ];
    let mut g = loop_graph(
        src(Data::Llr(llr), Format::llr(1200.0, Some(4.0))),
        SyncSearch::default(),
        &set,
    );
    assert_eq!(
        allocations(&mut g, iq(1.0), &[], 0, 300, 1000),
        0,
        "sync bits"
    );
    let mut g = loop_graph(
        src(Data::Symbols(sym), Format::new(Kind::Symbols, 1200.0)),
        SyncSearch::on_symbols(),
        &set,
    );
    assert_eq!(
        allocations(&mut g, iq(1.0), &[], 0, 300, 1000),
        0,
        "sync symbols"
    );

    // Dechirp.
    let d = Dechirp::new(7, 1).unwrap();
    let chirps: Vec<Complex32> = (0..64 * 128)
        .map(|n| d.base_chirp(n % 128 + (n / 128) * 37))
        .collect();
    let mut g = iq_graph(
        DechirpDemod::default(),
        "out",
        &[("bw_hz", Value::F64(125_000.0))],
    );
    assert_eq!(
        allocations(&mut g, iq(125_000.0), &chirps, 1000, 300, 1000),
        0,
        "dechirp"
    );

    // Despreading with acquisition, at a fixed code phase and with the delay-locked loop.
    let code: Vec<f32> = BARKER_11.iter().map(|&c| c as f32).collect();
    let spread: Vec<Complex32> = (0..2200 * 22)
        .map(|n| Complex32::new(a[n / 22] * code[(n % 22) / 2], 0.0))
        .collect();
    for tracking in ["none", "dll"] {
        let mut g = iq_graph(
            Despread::default(),
            "out",
            &[
                ("chip_rate_hz", Value::F64(11e6)),
                ("acquire_periods", Value::F64(4.0)),
                ("tracking", Value::Text(tracking.into())),
            ],
        );
        assert_eq!(
            allocations(&mut g, iq(22e6), &spread, 1000, 300, 1000),
            0,
            "despread {tracking}"
        );
    }

    // OFDM: 802.11a symbols back to back, through the sync and the demodulator.
    let layout = OfdmLayout::ieee80211a();
    let mut ifft = Fft::inverse(64);
    let mut ofdm = Vec::new();
    for l in 0..127u64 {
        let mut bins = vec![Complex32::default(); 64];
        for &i in layout.data(l) {
            bins[layout.bin(i as usize)] =
                Complex32::new(a[(l as usize * 48 + i as usize) % a.len()], a[i as usize]);
        }
        for p in layout.pilots(l) {
            bins[layout.bin(p.index as usize)] = p.value;
        }
        ifft.process(&mut bins);
        ofdm.extend_from_slice(&bins[48..]);
        ofdm.extend_from_slice(&bins);
    }
    let mut g = Tb::new()
        .add("sync", OfdmSync::default())
        .add("demod", OfdmDemod::default())
        .add("sink", NullSink::default())
        .input("sync")
        .connect("sync", "demod")
        .connect("demod", "sink")
        .build()
        .unwrap();
    // At 20 MHz the graph first merges its results (every 1/60 s of signal) after 333 passes: warm up past it.
    assert_eq!(
        allocations(&mut g, iq(20e6), &ofdm, 1000, 400, 1500),
        0,
        "ofdm"
    );
}
