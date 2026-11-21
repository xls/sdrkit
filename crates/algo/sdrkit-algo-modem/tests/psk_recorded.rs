//! The symbols of [`PskDemodulator`] against a record of its output on fixed signals with noise, a carrier offset and a
//! clock error: every order, every carrier detector, every size of the matched filter bank and the filtered path. The
//! record is `tests/data/psk_recorded.bin`: for each case its symbol count (u32) and the symbols (two f32 each), little
//! endian. A symbol must fall on the same point as its record and lie within 1e-3 of the record's size from it. No
//! recorded symbol is nearer than 2e-3 of its size to a decision boundary.

use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_modem::lock::Gears;
use sdrkit_algo_modem::psk::{PskCarrier, PskDemodConfig, PskDemodulator, PskOrder, PskPulse};
use sdrkit_algo_modem::Complex32;

const RECORD: &[u8] = include_bytes!("data/psk_recorded.bin");

/// A linear congruential generator: the same numbers on every target.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 32) as u32
    }
    /// About normal, variance 1: twelve uniform values.
    fn normal(&mut self) -> f64 {
        (0..12).map(|_| self.next() as f64 / 4_294_967_296.0).sum::<f64>() - 6.0
    }
}

/// One signal and the demodulator for it.
struct Case {
    cfg: PskDemodConfig,
    roll_off: f64,
    symbols: usize,
    /// Carrier offset, cycles per sample.
    carrier: f64,
    clock_ppm: f64,
    es_n0_db: f64,
    seed: u64,
}

impl Case {
    /// Pseudo-random points of the order through root raised cosine pulses, at the unit power of a sample, with the
    /// clock error, the carrier and the noise.
    fn signal(&self) -> Vec<Complex32> {
        const SPAN: i64 = 8;
        let order = self.cfg.order;
        let sps = self.cfg.sps * (1.0 + self.clock_ppm * 1e-6);
        let pulse = Shape::Rrc { beta: self.roll_off };
        let mut g = Lcg(self.seed);
        let points: Vec<Complex32> = (0..self.symbols + 2 * SPAN as usize + 2)
            .map(|_| order.point((g.next() >> 8) as usize))
            .collect();
        let n = (self.symbols as f64 * sps) as usize;
        let sigma = (0.5 * 10f64.powf(-self.es_n0_db / 10.0) * sps).sqrt();
        let offset = order == PskOrder::Oqpsk;
        (0..n)
            .map(|i| {
                let t = i as f64 / sps;
                let k0 = t.floor() as i64;
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for k in k0 - SPAN..=k0 + SPAN {
                    let a = points[(k + SPAN) as usize];
                    let h = pulse.value(t - k as f64);
                    let hq = if offset { pulse.value(t - k as f64 - 0.5) } else { h };
                    re += a.re as f64 * h;
                    im += a.im as f64 * hq;
                }
                let (s, c) = (std::f64::consts::TAU * self.carrier * i as f64).sin_cos();
                let (re, im) = (re * c - im * s, re * s + im * c);
                let scale = 1.0 / sps.sqrt();
                Complex32::new(
                    ((re + sigma * g.normal()) * scale) as f32,
                    ((im + sigma * g.normal()) * scale) as f32,
                )
            })
            .collect()
    }

    /// The symbols of the signal, the input in pieces of 1000 samples.
    fn run(&self) -> Vec<Complex32> {
        let mut d = PskDemodulator::new(self.cfg.clone()).unwrap();
        let mut out = Vec::new();
        for piece in self.signal().chunks(1000) {
            d.process(piece, &mut out);
        }
        out
    }
}

fn cases() -> Vec<Case> {
    // BPSK at 30.72 Msym/s and 45 Msps as a low-orbit link sets it: 16 taps, two timing gears, decisions, hand-over.
    let mut fast = PskDemodConfig::new(PskOrder::Bpsk, 45.0 / 30.72);
    fast.pulse = PskPulse::Rrc(0.5);
    fast.max_ppm = 5000.0;
    fast.timing_bn_t = 0.01;
    fast.timing_gears = Some(Gears {
        pull_in_bn_t: 0.01,
        track_bn_t: 0.0025,
    });
    fast.carrier = PskCarrier::DecisionDirected;
    fast.fll_bn_t = None;
    fast.max_freq = 0.25 / std::f64::consts::TAU;
    fast.handover = Some(0.003 / std::f64::consts::TAU);
    // QPSK with the defaults at 4 samples a symbol: 32 taps, Costas with the frequency loop, hand-over.
    let qpsk = PskDemodConfig::new(PskOrder::Qpsk, 4.0);
    // Offset QPSK at 25 Msym/s and 40 Msps with decisions and the held start.
    let mut offset = PskDemodConfig::new(PskOrder::Oqpsk, 1.6);
    offset.pulse = PskPulse::Rrc(0.5);
    offset.timing_bn_t = 0.0025;
    offset.timing_gears = None;
    offset.carrier = PskCarrier::DecisionDirected;
    offset.fll_bn_t = None;
    // 8PSK with the defaults at 8 samples a symbol: 64 taps.
    let psk8 = PskDemodConfig::new(PskOrder::Psk8, 8.0);
    // BPSK with the maximum-likelihood detector, one timing bandwidth, no hand-over.
    let mut ml = PskDemodConfig::new(PskOrder::Bpsk, 2.5);
    ml.timing_gears = None;
    ml.carrier = PskCarrier::MlTanh { n0: 0.2 };
    ml.fll_bn_t = None;
    ml.handover = None;
    // QPSK at 10 samples a symbol: the filter at the sample rate and the cubic interpolator.
    let mut filtered = PskDemodConfig::new(PskOrder::Qpsk, 10.0);
    filtered.carrier = PskCarrier::DecisionDirected;
    filtered.fll_bn_t = None;
    let case = |cfg: PskDemodConfig, roll_off, carrier, clock_ppm, es_n0_db, seed| Case {
        cfg,
        roll_off,
        symbols: 3000,
        carrier,
        clock_ppm,
        es_n0_db,
        seed,
    };
    vec![
        case(fast, 0.5, 0.001, 120.0, 12.0, 1),
        case(qpsk, 0.35, 0.003, -300.0, 14.0, 2),
        case(offset, 0.5, 0.0004, 80.0, 14.0, 13),
        case(psk8, 0.35, 0.0015, 200.0, 20.0, 4),
        case(ml, 0.35, 0.0006, -150.0, 9.0, 5),
        case(filtered, 0.35, 0.0002, 250.0, 14.0, 6),
    ]
}

#[test]
fn the_symbols_are_those_of_the_record() {
    let mut at = 0;
    let mut word = || {
        let w: [u8; 4] = RECORD[at..at + 4].try_into().unwrap();
        at += 4;
        w
    };
    for (k, c) in cases().iter().enumerate() {
        let n = u32::from_le_bytes(word()) as usize;
        let want: Vec<Complex32> = (0..n)
            .map(|_| Complex32::new(f32::from_le_bytes(word()), f32::from_le_bytes(word())))
            .collect();
        let got = c.run();
        assert_eq!(got.len(), n, "case {k}");
        assert!(n > 2900, "case {k}: {n}");
        let mut worst = 0.0f32;
        for (i, (&g, &w)) in got.iter().zip(&want).enumerate() {
            assert_eq!(c.cfg.order.decide(g), c.cfg.order.decide(w), "case {k} symbol {i}");
            let e = (g - w).norm() / w.norm();
            assert!(e <= 1e-3, "case {k} symbol {i}: {g} for {w}");
            worst = worst.max(e);
        }
        println!("case {k}: {n} symbols, largest difference {worst:e}");
    }
}
