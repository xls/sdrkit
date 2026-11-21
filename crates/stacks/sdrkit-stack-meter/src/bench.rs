//! Benchmarks of the crate's stacks on their own test signals, for `sdrkit bench` (see [`sdrkit_core::bench`]).
//! `items_per_second` counts input samples. `cpu_fraction_of_realtime` is for the input rate in the name.

use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark, Complex32, GenConfig, InputInfo, Output, StackConfig, StackDescriptor, Values};

/// Runs the receiver of `d` at its defaults over `signal` at `rate`, tuned to `centre`, for `seconds` of processor time.
pub fn run(d: &StackDescriptor, rate: f64, centre: f64, seconds: f64, signal: &[Complex32]) -> BenchResult {
    let cfg = StackConfig::new(Values::defaults(&(d.settings)()));
    let Ok(mut rx) = (d.make_receiver)(&cfg) else {
        return BenchResult::default();
    };
    let _ = rx.configure(&InputInfo::new(rate, centre, centre).recording());
    let mut out = Output::default();
    let sig_s = signal.len() as f64 / rate;
    measure(seconds, signal.len(), sig_s, || {
        for b in signal.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

/// `secs` seconds of the test signal of `d` at `rate` with the default settings.
pub fn gen(d: &StackDescriptor, rate: f64, secs: f64) -> Vec<Complex32> {
    let cfg = GenConfig::new(rate, Values::defaults(&(d.settings)())).with_seed(1);
    let Some(Ok(mut g)) = d.make_generator.map(|make| make(&cfg)) else {
        return Vec::new();
    };
    let mut x = vec![Complex32::default(); (rate * secs) as usize];
    g.generate(&mut x);
    x
}

/// The AM monitor at 250 ksps: the front end to 50 ksps and the readings of each second.
fn am_monitor_250k(s: f64) -> BenchResult {
    let d = &crate::am_monitor::DESCRIPTOR;
    let x = gen(d, 250e3, 4.0);
    run(d, 250e3, 1e6, s, &x)
}

/// The FM monitor at 1 Msps: the front end to 250 ksps, the discriminator and the spectrum of each second.
fn fm_monitor_1000k(s: f64) -> BenchResult {
    let d = &crate::fm_monitor::DESCRIPTOR;
    let x = gen(d, 1e6, 3.0);
    run(d, 1e6, 100e6, s, &x)
}

/// The occupancy monitor at 2 Msps: 2048-point spectra and the decisions of 70 channels every 10 ms.
fn occupancy_2000k(s: f64) -> BenchResult {
    let d = &crate::occupancy::DESCRIPTOR;
    let x = gen(d, 2e6, 2.0);
    run(d, 2e6, 0.0, s, &x)
}

/// The noise floor logger at 2 Msps: 4096-point spectra and the medians of 16 sub-bands.
fn noise_floor_2000k(s: f64) -> BenchResult {
    let d = &crate::noise_floor::DESCRIPTOR;
    let x = gen(d, 2e6, 2.0);
    run(d, 2e6, 0.0, s, &x)
}

/// The burst finder at 2 Msps: 256-point frames every 128 samples, the groups and the bursts of the default scene.
fn burst_finder_2000k(s: f64) -> BenchResult {
    let d = &crate::burst_finder::DESCRIPTOR;
    let x = gen(d, 2e6, 2.0);
    run(d, 2e6, 0.0, s, &x)
}

/// The hopper analyser at 2 Msps: 256-point frames every 128 samples and the dwells of 500 hops a second.
fn hopper_2000k(s: f64) -> BenchResult {
    let d = &crate::hopper::DESCRIPTOR;
    let x = gen(d, 2e6, 2.0);
    run(d, 2e6, 0.0, s, &x)
}

/// The persistent carrier finder at 2 Msps: 16384-point spectra, the peaks and the tracks of the default scene.
fn birdie_2000k(s: f64) -> BenchResult {
    let d = &crate::birdie::DESCRIPTOR;
    let x = gen(d, 2e6, 2.0);
    run(d, 2e6, 0.0, s, &x)
}

/// The occupied bandwidth meter at 1 Msps: 16384-point Blackman-Harris spectra averaged over 1 s.
fn obw_1000k(s: f64) -> BenchResult {
    let d = &crate::obw::DESCRIPTOR;
    let x = gen(d, 1e6, 3.0);
    run(d, 1e6, 0.0, s, &x)
}

/// The PTT logger at 48 ksps: the channel filter to 16 ksps and the 5 ms blocks.
fn ptt_log_48k(s: f64) -> BenchResult {
    let d = &crate::ptt_log::DESCRIPTOR;
    let x = gen(d, 48e3, 25.0);
    run(d, 48e3, 0.0, s, &x)
}

/// The noise locator at 250 ksps: the filter to 50 ksps, the impulse slicer and the pair spacings.
fn noise_locator_250k(s: f64) -> BenchResult {
    let d = &crate::noise_locator::DESCRIPTOR;
    let x = gen(d, 250e3, 6.0);
    run(d, 250e3, 0.0, s, &x)
}

/// The frequency accuracy meter at 48 ksps: the filter to 16 ksps and one 16384-point look a second.
fn freq_accuracy_48k(s: f64) -> BenchResult {
    let d = &crate::freq_accuracy::DESCRIPTOR;
    let x = gen(d, 48e3, 4.0);
    run(d, 48e3, 10e6, s, &x)
}

/// The SINAD meter at 48 ksps: the filter to 24 ksps, the discriminator and one 32768-point spectrum a window.
fn sinad_48k(s: f64) -> BenchResult {
    let d = &crate::sinad::DESCRIPTOR;
    let x = gen(d, 48e3, 4.0);
    run(d, 48e3, 146.52e6, s, &x)
}

/// The NFM meter at 48 ksps: the filter to 24 ksps, the discriminator, the voice band-pass and the CTCSS spectrum of each window.
fn nfm_meter_48k(s: f64) -> BenchResult {
    let d = &crate::nfm_meter::DESCRIPTOR;
    let x = gen(d, 48e3, 4.0);
    run(d, 48e3, 146.52e6, s, &x)
}

/// The stability meter at 48 ksps: the filter to 24 ksps, the 20 ms blocks and the frequency readings of the transmissions.
fn tx_stability_48k(s: f64) -> BenchResult {
    let d = &crate::tx_stability::DESCRIPTOR;
    let x = gen(d, 48e3, 12.0);
    run(d, 48e3, 146.52e6, s, &x)
}

/// The SSB test meter at 48 ksps: the filter to 12 ksps and one 32768-point spectrum a window.
fn ssb_test_48k(s: f64) -> BenchResult {
    let d = &crate::ssb_test::DESCRIPTOR;
    let x = gen(d, 48e3, 6.0);
    run(d, 48e3, 14.2e6, s, &x)
}

/// The AFSK tuner at 48 ksps: the filter to 24 ksps, the discriminator, the 512-point frames and the analysis of each burst.
fn afsk_tuner_48k(s: f64) -> BenchResult {
    let d = &crate::afsk_tuner::DESCRIPTOR;
    let x = gen(d, 48e3, 6.0);
    run(d, 48e3, 144.39e6, s, &x)
}

/// The phase noise meter at 250 ksps: one 262144-point Blackman-Harris spectrum a second, the average, the readings and the spur search.
fn phase_noise_250k(s: f64) -> BenchResult {
    let d = &crate::phase_noise::DESCRIPTOR;
    let x = gen(d, 250e3, 6.0);
    run(d, 250e3, 10e6, s, &x)
}

/// The CW keying meter at 48 ksps: the filter to 12 ksps, the carrier finder, the envelope at 2 ksps and the marks.
fn cw_keying_48k(s: f64) -> BenchResult {
    let d = &crate::cw_keying::DESCRIPTOR;
    let x = gen(d, 48e3, 12.0);
    run(d, 48e3, 7.03e6, s, &x)
}

/// The FSK analyser at 250 ksps: the filter to 125 ksps, the discriminator and the analysis of each 2 s block.
fn fsk_analyser_250k(s: f64) -> BenchResult {
    let d = &crate::fsk_analyser::DESCRIPTOR;
    let x = gen(d, 250e3, 6.0);
    run(d, 250e3, 433.92e6, s, &x)
}

/// The modulation quality meter at 250 ksps: the filter to 125 ksps and the blind M-PSK chain on each 2 s block.
fn mer_250k(s: f64) -> BenchResult {
    let d = &crate::mer::DESCRIPTOR;
    let x = gen(d, 250e3, 6.0);
    run(d, 250e3, 435e6, s, &x)
}

/// The crest factor meter at 1 Msps: the powers of each block and the CCDF of 2^20 samples every 16 blocks.
fn papr_1000k(s: f64) -> BenchResult {
    let d = &crate::papr::DESCRIPTOR;
    let x = gen(d, 1e6, 3.0);
    run(d, 1e6, 145e6, s, &x)
}

/// The input health meter at 2 Msps: the sums of each sample and a reading every second.
fn input_health_2000k(s: f64) -> BenchResult {
    let d = &crate::input_health::DESCRIPTOR;
    let x = gen(d, 2e6, 3.0);
    run(d, 2e6, 145e6, s, &x)
}

/// The noise figure meter at 2 Msps: the filter to 250 ksps, power blocks of 25 ms and the plateau rule.
fn noise_figure_2000k(s: f64) -> BenchResult {
    let d = &crate::noise_figure::DESCRIPTOR;
    let x = gen(d, 2e6, 3.0);
    run(d, 2e6, 145e6, s, &x)
}

/// The spectral kurtosis finder at 2 Msps: a 1024-point FFT of each frame and the sums of each bin.
fn kurtosis_2000k(s: f64) -> BenchResult {
    let d = &crate::kurtosis::DESCRIPTOR;
    let x = gen(d, 2e6, 3.0);
    run(d, 2e6, 145e6, s, &x)
}

/// The stream glitch detector at 2 Msps: the phase tracker of the tone, the zero, block and level checks on each sample.
fn glitch_2000k(s: f64) -> BenchResult {
    let d = &crate::glitch::DESCRIPTOR;
    let x = gen(d, 2e6, 3.0);
    run(d, 2e6, 145e6, s, &x)
}

/// The intermodulation finder at 1 Msps: 16384-point snapshots, the peak tracker and the fits of each evaluation.
fn intermod_1000k(s: f64) -> BenchResult {
    let d = &crate::intermod::DESCRIPTOR;
    let x = gen(d, 1e6, 6.0);
    run(d, 1e6, 145e6, s, &x)
}

/// The spur and image finder at 1 Msps: 16384-point snapshots, the peak tracker and the fits of each evaluation.
fn spur_1000k(s: f64) -> BenchResult {
    let d = &crate::spur::DESCRIPTOR;
    let x = gen(d, 1e6, 6.0);
    run(d, 1e6, 145e6, s, &x)
}

/// The fading analyser at 48 ksps: the filter to 16 ksps, the carrier finder each second and the envelope at 1 ksps.
fn fading_48k(s: f64) -> BenchResult {
    let d = &crate::fading::DESCRIPTOR;
    let x = gen(d, 48e3, 40.0);
    run(d, 48e3, 144.4e6, s, &x)
}

/// The beacon monitor at 250 ksps: the filter to 2.4 ksps and a 2048-point Blackman-Harris spectrum every 0.85 s.
fn beacon_monitor_250k(s: f64) -> BenchResult {
    let d = &crate::beacon_monitor::DESCRIPTOR;
    let x = gen(d, 250e3, 10.0);
    run(d, 250e3, 144.4e6, s, &x)
}

// NEXT BENCH

/// The benchmarks of the crate.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "meter.fm_monitor_1000k",
            run: fm_monitor_1000k,
        },
        Benchmark {
            name: "meter.am_monitor_250k",
            run: am_monitor_250k,
        },
        Benchmark {
            name: "meter.occupancy_2000k",
            run: occupancy_2000k,
        },
        Benchmark {
            name: "meter.noise_floor_2000k",
            run: noise_floor_2000k,
        },
        Benchmark {
            name: "meter.burst_finder_2000k",
            run: burst_finder_2000k,
        },
        Benchmark {
            name: "meter.hopper_2000k",
            run: hopper_2000k,
        },
        Benchmark {
            name: "meter.birdie_2000k",
            run: birdie_2000k,
        },
        Benchmark {
            name: "meter.obw_1000k",
            run: obw_1000k,
        },
        Benchmark {
            name: "meter.ptt_log_48k",
            run: ptt_log_48k,
        },
        Benchmark {
            name: "meter.noise_locator_250k",
            run: noise_locator_250k,
        },
        Benchmark {
            name: "meter.freq_accuracy_48k",
            run: freq_accuracy_48k,
        },
        Benchmark {
            name: "meter.sinad_48k",
            run: sinad_48k,
        },
        Benchmark {
            name: "meter.nfm_meter_48k",
            run: nfm_meter_48k,
        },
        Benchmark {
            name: "meter.tx_stability_48k",
            run: tx_stability_48k,
        },
        Benchmark {
            name: "meter.ssb_test_48k",
            run: ssb_test_48k,
        },
        Benchmark {
            name: "meter.afsk_tuner_48k",
            run: afsk_tuner_48k,
        },
        Benchmark {
            name: "meter.phase_noise_250k",
            run: phase_noise_250k,
        },
        Benchmark {
            name: "meter.cw_keying_48k",
            run: cw_keying_48k,
        },
        Benchmark {
            name: "meter.fsk_analyser_250k",
            run: fsk_analyser_250k,
        },
        Benchmark {
            name: "meter.mer_250k",
            run: mer_250k,
        },
        Benchmark {
            name: "meter.papr_1000k",
            run: papr_1000k,
        },
        Benchmark {
            name: "meter.input_health_2000k",
            run: input_health_2000k,
        },
        Benchmark {
            name: "meter.noise_figure_2000k",
            run: noise_figure_2000k,
        },
        Benchmark {
            name: "meter.kurtosis_2000k",
            run: kurtosis_2000k,
        },
        Benchmark {
            name: "meter.glitch_2000k",
            run: glitch_2000k,
        },
        Benchmark {
            name: "meter.intermod_1000k",
            run: intermod_1000k,
        },
        Benchmark {
            name: "meter.spur_1000k",
            run: spur_1000k,
        },
        Benchmark {
            name: "meter.fading_48k",
            run: fading_48k,
        },
        Benchmark {
            name: "meter.beacon_monitor_250k",
            run: beacon_monitor_250k,
        },
        // NEXT ENTRY
    ]
}
