//! The generator, set to the recording's parameters and fed the recording's transport stream, reproduces the recorded signal: the
//! correlation per symbol with the recording (offset removed) is close to what its noise allows. This checks the whole transmit chain
//! (energy dispersal, RS, interleavers, puncturing, mapping, pilots, TPS) against an independent transmitter.

use sdrkit_core::Complex32;
use sdrkit_proto_mpegts::source::FileSource;
use sdrkit_stack_dvbt::generator::{DvbtGenerator, GenParams};
use sdrkit_stack_dvbt::params::{Bandwidth, TpsParams};

#[test]
#[allow(clippy::needless_range_loop)]
fn the_generator_matches_the_recording() {
    let Some(rec) = sdrkit_dev_test::read_recording("dvbt/dvbt_7mhz_2k_16qam_8msps") else {
        return;
    };
    let Some(ts) = sdrkit_dev_test::recording("dvbt/mux_two_services.ts") else {
        return;
    };
    let ts = std::fs::read(ts).unwrap();
    let mut best = (0.0f64, 0usize);
    for phase in 0..3 {
        let mut p = GenParams::new(Bandwidth::B7, TpsParams::default());
        p.hp = Box::new(FileSource::new(&ts));
        p.puncture_phase = phase;
        let mut g = DvbtGenerator::new(p);
        let sym = 2304usize;
        let start = 20000usize;
        let skip = sym / 3; // the signal starts a third into symbol 0
        let mut sig = Vec::new();
        for _ in 0..80 {
            sig.extend_from_slice(g.next_symbol());
        }
        let sig = &sig[skip..];
        let f = 10_078.125 / 8e6;
        // Correlate symbols 20 to 70 (after the outer interleaver has filled).
        let (a, b) = (20 * sym, 70 * sym);
        let mut c = Complex32::default();
        let (mut e1, mut e2) = (0f64, 0f64);
        for n in a..b {
            let r = rec.samples[start + n];
            let ph = -2.0 * std::f64::consts::PI * f * n as f64;
            let rot = Complex32::new(ph.cos() as f32, ph.sin() as f32);
            let x = r * rot;
            c += x * sig[n].conj();
            e1 += x.norm_sqr() as f64;
            e2 += sig[n].norm_sqr() as f64;
        }
        let rho = c.norm() as f64 / (e1 * e2).sqrt();
        eprintln!("phase {phase}: correlation {rho:.4}");
        if rho > best.0 {
            best = (rho, phase);
        }
    }
    // 24 dB over the sample band: |rho| = 1 / sqrt(1 + 1/SNR) = 0.998.
    assert!(
        best.0 > 0.99,
        "correlation {:.4} (phase {})",
        best.0,
        best.1
    );
}
