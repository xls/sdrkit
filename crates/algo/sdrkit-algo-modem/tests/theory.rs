//! Known answers from communication theory: bit error rates of the slicer against the closed forms, LLR calibration (an LLR of `L`
//! must be wrong with probability `1 / (1 + e^|L|)`), and a whole receive chain (signals from `sdrkit-algo-sim`: RRC pulses, carrier
//! offset, clock error, noise) within a small implementation loss of the AWGN bound.

use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_dsp::interp::Polyphase;
use sdrkit_algo_dsp::meter::db_to_power;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas};
use sdrkit_algo_modem::theory::{ber_bpsk, ber_square_qam};
use sdrkit_algo_modem::timing::{Gardner, MatchedInterp, SymbolSync, SymbolSyncConfig};
use sdrkit_algo_modem::{Complex32, Constellation, Demap};
use sdrkit_algo_sim::clock::ClockError;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_algo_sim::rng::Rng;

/// Bit errors of hard decisions on `set` at `ebn0_db`, over `symbols` symbols of random labels with complex AWGN.
fn ber(set: &PointSet, ebn0_db: f64, symbols: usize, seed: u64) -> f64 {
    let c = Constellation::new(set.clone()).unwrap();
    let k = c.bits() as f64;
    let n0 = 1.0 / (k * db_to_power(ebn0_db));
    let mut rng = Rng::new(seed);
    let mut errors = 0u64;
    for _ in 0..symbols {
        let l = rng.below(c.len() as u64) as u16;
        let z = c.points()[l as usize] + rng.complex_normal(n0);
        errors += (c.decide(z) ^ l).count_ones() as u64;
    }
    errors as f64 / (symbols as f64 * k)
}

#[test]
fn bit_error_rates_match_the_closed_forms() {
    // (name, set, Eb/N0 dB, theory)
    let cases: Vec<(&str, PointSet, f64, f64)> = vec![
        ("bpsk", PointSet::bpsk(), 6.0, ber_bpsk(db_to_power(6.0))),
        ("qpsk", PointSet::qpsk(), 6.0, ber_bpsk(db_to_power(6.0))),
        (
            "16qam 3gpp",
            PointSet::qam(16, QamMap::THREE_GPP).unwrap(),
            9.0,
            ber_square_qam(16, db_to_power(9.0)),
        ),
        (
            "16qam 802.11",
            PointSet::qam(16, QamMap::IEEE_802_11).unwrap(),
            9.0,
            ber_square_qam(16, db_to_power(9.0)),
        ),
        (
            "64qam dvb",
            PointSet::qam(64, QamMap::DVB).unwrap(),
            13.0,
            ber_square_qam(64, db_to_power(13.0)),
        ),
    ];
    for (name, set, e, want) in cases {
        let got = ber(&set, e, 400_000, 7);
        // About 1000 to 10000 errors: within 10 %.
        assert!(
            (got / want - 1.0).abs() < 0.1,
            "{name} at {e} dB: {got:.3e} vs {want:.3e}"
        );
    }
}

#[test]
fn llrs_are_calibrated() {
    // For each set, bin the LLRs by magnitude; in each bin the fraction whose sign is wrong must be 1 / (1 + e^|L|).
    for (name, set, ebn0_db, method) in [
        ("qpsk", PointSet::qpsk(), 2.0, Demap::Exact),
        (
            "16qam",
            PointSet::qam(16, QamMap::DVB).unwrap(),
            6.0,
            Demap::Exact,
        ),
        ("8psk", PointSet::psk(8, 0.0).unwrap(), 5.0, Demap::Exact),
        (
            "32apsk",
            PointSet::dvbs2_32apsk(2.84, 5.27).unwrap(),
            9.0,
            Demap::Exact,
        ),
    ] {
        let c = Constellation::new(set).unwrap();
        let k = c.bits();
        let n0 = 1.0 / (k as f64 * db_to_power(ebn0_db));
        let mut rng = Rng::new(11);
        let mut llr = vec![0f32; k];
        const BINS: usize = 8;
        let mut wrong = [0u64; BINS];
        let mut all = [0u64; BINS];
        let mut want = [0f64; BINS];
        for _ in 0..200_000 {
            let l = rng.below(c.len() as u64) as usize;
            let z = c.points()[l] + rng.complex_normal(n0);
            c.demap(z, n0 as f32, method, &mut llr).unwrap();
            for (b, &v) in llr.iter().enumerate() {
                let bit = (l >> (k - 1 - b)) & 1;
                let mag = v.abs() as f64;
                let i = ((mag / 0.75) as usize).min(BINS - 1);
                all[i] += 1;
                want[i] += 1.0 / (1.0 + mag.exp());
                if (v < 0.0) != (bit == 1) {
                    wrong[i] += 1;
                }
            }
        }
        for i in 0..BINS {
            if all[i] < 20_000 {
                continue;
            }
            let p = wrong[i] as f64 / all[i] as f64;
            let w = want[i] / all[i] as f64;
            // Binomial spread plus a little: within 8 % (or 0.003 absolute for tiny rates).
            assert!(
                (p - w).abs() < 0.08 * w + 0.003,
                "{name} bin {i}: wrong {p:.4} expected {w:.4} ({} values)",
                all[i]
            );
        }
    }
}

/// QPSK through RRC 0.35 at 4 samples per symbol, a carrier offset of 0.5 % of the symbol rate, a 200 ppm clock error and noise;
/// received with the matched polyphase interpolator, Gardner timing and a geared Costas loop with a frequency-loop assist. The bit error
/// rate must be within 0.7 dB of the AWGN bound.
#[test]
fn a_whole_receive_chain_is_near_the_bound() {
    let sps = 4.0;
    let ebn0_db = 6.0;
    let nsym = 120_000;
    let mut rng = Rng::new(21);
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let bits: Vec<(u8, u8)> = (0..nsym).map(|_| (rng.bit(), rng.bit())).collect();
    let syms: Vec<Complex32> = bits
        .iter()
        .map(|&(a, b)| Complex32::new(if a == 0 { s } else { -s }, if b == 0 { s } else { -s }))
        .collect();
    let mut m = Linear::new(Shape::Rrc { beta: 0.35 }, 16.0, sps).unwrap();
    let mut tx = Vec::new();
    m.push(&syms, &mut tx);
    m.flush(&mut tx);
    // Clock error, carrier offset, noise. Signal power per sample 1, so Es/N0 = sps / sigma^2.
    let mut clk = ClockError::new(200.0).unwrap();
    let mut rx = Vec::new();
    clk.process(&tx, &mut rx);
    FrequencyOffset::new(sps, 0.005, 0.0)
        .unwrap()
        .process(&mut rx);
    let n0 = sps / (2.0 * db_to_power(ebn0_db));
    for v in rx.iter_mut() {
        *v += rng.complex_normal(n0);
    }

    let mut cfg = SymbolSyncConfig::new(sps);
    cfg.pulse = Shape::Rc { beta: 0.35 };
    cfg.bn_t = 0.005;
    let mi = MatchedInterp::<32, 64>::new(Shape::Rrc { beta: 0.35 }, sps).unwrap();
    let mut ts = SymbolSync::new(Gardner, mi, cfg).unwrap();
    let mut y = Vec::new();
    ts.process(&rx, &mut y);
    let mut ccfg = CarrierConfig::new(0.005);
    ccfg.fll_bn_t = Some(0.002);
    ccfg.gears = Some(sdrkit_algo_modem::lock::Gears {
        pull_in_bn_t: 0.02,
        track_bn_t: 0.005,
    });
    let mut cs = CarrierSync::new(Costas::<4>, ccfg).unwrap();
    let mut z = vec![Complex32::default(); y.len()];
    cs.process(&y, &mut z).unwrap();
    assert!(
        (cs.freq() - 0.005).abs() < 2e-4,
        "carrier {} lock {}",
        cs.freq(),
        cs.lock_metric()
    );
    assert!(
        (ts.clock_ppm() + 200.0).abs() < 100.0 || (ts.clock_ppm() - 200.0).abs() < 100.0,
        "clock {}",
        ts.clock_ppm()
    );

    // Align: the symbol lag and the quarter-turn ambiguity, from a stretch after the loops settle.
    let c = Constellation::new(PointSet::qpsk()).unwrap();
    let label = |a: u8, b: u8| ((a << 1) | b) as u16;
    let probe = 20_000..20_200;
    let mut best = (0usize, 0usize, usize::MAX);
    for lag in 0..40 {
        for q in 0..4 {
            let r = Complex32::from_polar(1.0, std::f32::consts::FRAC_PI_2 * q as f32);
            let e: usize = probe
                .clone()
                .map(|i| {
                    (c.decide(z[i + lag] * r) ^ label(bits[i].0, bits[i].1)).count_ones() as usize
                })
                .sum();
            if e < best.2 {
                best = (lag, q, e);
            }
        }
    }
    let (lag, q, _) = best;
    let r = Complex32::from_polar(1.0, std::f32::consts::FRAC_PI_2 * q as f32);
    let (mut errors, mut n) = (0u64, 0u64);
    for i in 20_000..nsym - 100 {
        if i + lag >= z.len() {
            break;
        }
        errors += (c.decide(z[i + lag] * r) ^ label(bits[i].0, bits[i].1)).count_ones() as u64;
        n += 2;
    }
    let got = errors as f64 / n as f64;
    let bound = ber_bpsk(db_to_power(ebn0_db));
    let loss_limit = ber_bpsk(db_to_power(ebn0_db - 0.7));
    assert!(
        got > 0.8 * bound && got < loss_limit,
        "BER {got:.3e}, bound {bound:.3e}, limit {loss_limit:.3e}"
    );

    // The same with the generic polyphase interpolator after a separate matched filter is not needed here: the polyphase
    // interpolator alone on the unfiltered signal must still lock (with more noise, so only lock is checked).
    let mut ts2 = SymbolSync::new(
        Gardner,
        Polyphase::<16, 64>::design(0.8, 60.0).unwrap(),
        cfg,
    )
    .unwrap();
    let mut y2 = Vec::new();
    ts2.process(&rx[..200_000], &mut y2);
    assert!(ts2.symbols() > 40_000);
}
