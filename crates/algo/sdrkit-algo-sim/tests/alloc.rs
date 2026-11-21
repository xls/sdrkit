//! Checks that the steady-state paths allocate nothing: each runs 1,000 times after a warm-up call, under an allocator that counts.
//! (The counting allocator needs `unsafe impl GlobalAlloc`; it is defined in this test only.)

use sdrkit_algo_sim::chain::{
    Chain, ChainConfig, HumSpec, Impaired, ImpulseSpec, PhaseNoiseSpec, SignalInfo,
};
use sdrkit_algo_sim::clock::ClockError;
use sdrkit_algo_sim::fading::profiles::{Gsm, Watterson};
use sdrkit_algo_sim::fading::{Profile, Tdl};
use sdrkit_algo_sim::front::{Hum, ImpulseNoise, IqImbalance, Quantiser};
use sdrkit_algo_sim::modulate::{Afsk, Am, Cpm, Fm, Linear, Morse, Ook, Ramp, Sideband, Ssb};
use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_algo_sim::phase_noise::PhaseNoise;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::Scene;
use sdrkit_core::{Complex32, Generator};
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

fn count(f: impl FnOnce()) -> usize {
    let before = ALLOCS.load(Ordering::Relaxed);
    f();
    ALLOCS.load(Ordering::Relaxed) - before
}

struct Carrier;
impl Generator for Carrier {
    fn rate_hz(&self) -> f64 {
        48_000.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::new(0.5, 0.0));
        out.len()
    }
}

// One test function, so no other test thread allocates while counting.
#[test]
fn steady_state_paths_do_not_allocate() {
    let x: Vec<Complex32> = (0..256)
        .map(|i| Complex32::from_polar(0.5, i as f32 * 0.1))
        .collect();
    let mut buf = x.clone();

    // Random numbers and noise.
    let mut rng = Rng::new(1);
    let mut normals = vec![0f32; 256];
    let mut bits = vec![0u8; 256];
    let mut awgn = Awgn::complex(NoiseLevel::Cn0DbHz(50.0), 1.0, 48e3, 1).unwrap();
    let n = count(|| {
        for _ in 0..1000 {
            rng.fill_normal(&mut normals);
            rng.fill_bits(&mut bits);
            let _ = rng.below(7) + rng.next_u64();
            awgn.process(&mut buf);
        }
    });
    assert_eq!(n, 0, "random numbers and noise allocated {n} times");

    // Per-sample impairments.
    let mut off = FrequencyOffset::new(48e3, 1000.0, 5.0).unwrap();
    let mut pn = PhaseNoise::from_dbc_hz(48e3, -80.0, 1e3, Some(-120.0), 2).unwrap();
    let mut hum = Hum::new(48e3, 50.0, 0.1)
        .unwrap()
        .with_harmonics(&[0.3])
        .unwrap();
    let iq = IqImbalance::new(0.5, 2.0).unwrap();
    let q = Quantiser::new(8).unwrap();
    let mut imp = ImpulseNoise::new(48e3, 100.0, 1e-3, 0.0, 3).unwrap();
    let mut fading = Tdl::from_profile(&Profile::Drm(3), 48e3, 4).unwrap();
    let mut gsm = Tdl::from_profile(
        &Profile::Gsm {
            model: Gsm::Tu6,
            max_doppler_hz: 100.0,
        },
        1e6,
        5,
    )
    .unwrap();
    let mut clock = ClockError::new(123.0).unwrap();
    let mut out = Vec::with_capacity(4096);
    let n = count(|| {
        for _ in 0..1000 {
            off.process(&mut buf);
            pn.process(&mut buf);
            hum.process(&mut buf);
            iq.process(&mut buf);
            let _ = q.process(&mut buf);
            imp.process(&mut buf);
            fading.process(&mut buf);
            gsm.process(&mut buf);
            out.clear();
            clock.process(&x, &mut out);
        }
    });
    assert_eq!(n, 0, "impairments allocated {n} times");

    // The whole chain, directly and around a generator.
    let cfg = ChainConfig {
        level_dbfs: -10.0,
        inverted: true,
        fading: Profile::Watterson(Watterson::Moderate),
        offset_hz: 300.0,
        drift_hz_s: 1.0,
        phase_noise: Some(PhaseNoiseSpec {
            dbc_hz: -80.0,
            offset_hz: 1e3,
            floor_dbc_hz: None,
        }),
        clock_ppm: 40.0,
        bit_jitter: 0.1,
        hum: Some(HumSpec {
            freq_hz: 50.0,
            depth: 0.01,
        }),
        noise: NoiseLevel::SnrDb {
            snr_db: 10.0,
            bandwidth_hz: 3e3,
        },
        impulse: Some(ImpulseSpec {
            rate_hz: 10.0,
            width_s: 1e-4,
            level_dbfs: -10.0,
        }),
        iq_gain_db: 0.2,
        iq_phase_deg: 1.0,
        dc: Complex32::new(0.01, 0.0),
        adc_bits: Some(12),
        seed: 9,
    };
    let sig = SignalInfo::new(48e3)
        .with_bandwidth(3e3)
        .with_bit_rate(1200.0);
    let mut chain = Chain::new(&cfg, &sig).unwrap();
    let mut gen = Impaired::new(Carrier, &cfg, sig).unwrap();
    let mut gout = vec![Complex32::default(); 700];
    out.clear();
    chain.process(&x, &mut out);
    gen.generate(&mut gout);
    let n = count(|| {
        for _ in 0..1000 {
            out.clear();
            chain.process(&x, &mut out);
            gen.generate(&mut gout);
        }
    });
    assert_eq!(n, 0, "the chain allocated {n} times");

    // Modulators.
    let mut lin = Linear::rrc(0.35, 12.0, 4.7).unwrap().with_offset_q();
    let mut cpm = Cpm::gmsk(0.3, 4.0).unwrap();
    let mut ramped = Cpm::gfsk(2.0, 1.0, 16.0)
        .unwrap()
        .with_held_edges()
        .with_ramp(Ramp::RaisedCosine(2.0))
        .unwrap();
    let mut afsk = Afsk::new(48e3, 1200.0, 1200.0, 2200.0).unwrap();
    let mut ook = Ook::new(48e3, 0.002).unwrap();
    let mut fm = Fm::new(48e3, 5e3)
        .unwrap()
        .with_preemphasis(50e-6, 48e3)
        .unwrap();
    let mut ssb = Ssb::new(Sideband::Upper, 48e3, 200.0, 60.0).unwrap();
    let am = Am::new(0.8).unwrap();
    let audio: Vec<f32> = (0..256).map(|i| (i as f32 * 0.05).sin()).collect();
    let levels: Vec<f32> = (0..32)
        .map(|i| if i % 3 == 0 { 1.0 } else { -1.0 })
        .collect();
    let bits8 = [1u8, 0, 1, 1, 0, 0, 1, 0];
    let mut sym_out = Vec::with_capacity(4096);
    let mut audio_out = Vec::with_capacity(4096);
    let mut iq_out = vec![Complex32::default(); 256];
    lin.push(&x[..16], &mut sym_out);
    ramped.push(&levels[..4], &mut sym_out);
    ramped.flush(&mut sym_out);
    let n = count(|| {
        for _ in 0..1000 {
            sym_out.clear();
            audio_out.clear();
            lin.push(&x[..16], &mut sym_out);
            cpm.push(&levels, &mut sym_out);
            ramped.push(&levels[..4], &mut sym_out);
            ramped.flush(&mut sym_out);
            afsk.push(&bits8, &mut audio_out);
            ook.push(true, 0.01, &mut audio_out).unwrap();
            ook.push(false, 0.01, &mut audio_out).unwrap();
            fm.process(&audio, &mut iq_out).unwrap();
            ssb.process(&audio, &mut iq_out).unwrap();
            am.process(&audio, &mut iq_out).unwrap();
        }
    });
    assert_eq!(n, 0, "modulators allocated {n} times");

    // Morse and a scene, as generators.
    let mut morse = Morse::new("CQ TEST", 25.0, 8000.0, 600.0, 0.004)
        .unwrap()
        .repeating(0.5)
        .unwrap();
    let mut scene = Scene::new(48e3).unwrap();
    scene
        .add_repeating(0.0, 0.01, 1000.0, -3.0, vec![Complex32::new(1.0, 0.0); 200])
        .unwrap();
    scene
        .add_stream(0.0, None, -2000.0, 0.0, Box::new(Carrier))
        .unwrap();
    let mut mout = vec![Complex32::default(); 500];
    for _ in 0..100 {
        morse.generate(&mut mout);
    }
    scene.generate(&mut mout);
    let n = count(|| {
        for _ in 0..1000 {
            morse.generate(&mut mout);
            scene.generate(&mut mout);
        }
    });
    assert_eq!(n, 0, "Morse and the scene allocated {n} times");
}
