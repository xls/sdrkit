//! ISDB-T: generator, shared impairments and receiver round trips, acquisition, and that any cut of the input decodes to the
//! same packets.

use super::common::{self, chain, impair, Run};
use sdrkit_algo_sim::chain::ChainConfig;
use sdrkit_algo_sim::fading::Profile;
use sdrkit_core::Complex32;
use sdrkit_proto_mpegts::source::NumberedCheck;
use sdrkit_stack_tvterr::isdbt::chain::BitDelay;
use sdrkit_stack_tvterr::isdbt::gen::{
    config, layer, layer_pattern, IsdbtGenParams, IsdbtGenerator, Payload,
};
use sdrkit_stack_tvterr::isdbt::layout::Band;
use sdrkit_stack_tvterr::isdbt::tmcc::{Config, LayerParams};
use sdrkit_stack_tvterr::isdbt::{frame_seconds, ifft_rate, DESCRIPTOR};

fn fft_size(mode: u8) -> usize {
    Band::FULL.fft_size(mode)
}

fn carriers(mode: u8) -> usize {
    Band::FULL.carriers(mode)
}

const U: LayerParams = LayerParams::UNUSED;

/// The occupied bandwidth of the signal (the SNR is stated in it).
fn bandwidth(p: &IsdbtGenParams) -> f64 {
    carriers(p.mode) as f64 * ifft_rate(p.width_mhz) / fft_size(p.mode) as f64
}

/// The clean signal.
fn clean(p: &IsdbtGenParams, seconds: f64) -> Vec<Complex32> {
    common::generate(IsdbtGenerator::new(p.clone()).unwrap(), seconds)
}

/// The signal with a carrier offset, a clock error and noise.
fn gen_with(
    p: &IsdbtGenParams,
    seconds: f64,
    cfo: f64,
    ppm: f64,
    snr: Option<f64>,
) -> Vec<Complex32> {
    impair(
        &clean(p, seconds),
        p.rate_hz,
        &chain(cfo, ppm, snr, bandwidth(p)),
    )
}

fn gen(p: &IsdbtGenParams, seconds: f64) -> Vec<Complex32> {
    clean(p, seconds)
}

fn run(width: &str, x: &[Complex32], rate: f64, chunk: usize) -> Run {
    let mut r = common::receiver(&DESCRIPTOR, &[("width", width)]);
    common::run(r.as_mut(), rate, x, chunk, "merge.ts")
}

/// Per-layer check of the numbered packets (PID 0x100 + layer).
fn per_layer(r: &Run) -> [NumberedCheck; 3] {
    std::array::from_fn(|li| r.check(&layer_pattern(li)))
}

fn ppf(mode: u8, l: &LayerParams) -> usize {
    sdrkit_stack_tvterr::isdbt::chain::packets_per_frame(mode, l)
}

/// Every used layer: no wrong packet, at most `gaps` breaks, and at least `share` of the packets the signal carried from the
/// layer's first good packet to the end (less one frame for what is still inside the decoder).
fn check_layers(r: &Run, p: &IsdbtGenParams, seconds: f64, share: f64, gaps: usize, what: &str) {
    let fl = frame_seconds(p.mode, p.guard, p.width_mhz);
    let c = per_layer(r);
    let cfg = p.switch.map_or(p.config, |s| s.1);
    for (li, l) in cfg.layers.iter().enumerate() {
        if !l.used() {
            continue;
        }
        let k = c[li];
        assert_eq!(k.wrong, 0, "{what}: layer {li} {k:?}");
        assert!(k.gaps as usize <= gaps, "{what}: layer {li} {k:?}");
        let t0 = k
            .first_time
            .unwrap_or_else(|| panic!("{what}: layer {li} gave nothing: {:?}", r.results.fields()));
        let expect = ((seconds - t0) / fl - 1.0).max(0.0) * ppf(p.mode, l) as f64;
        assert!(
            k.good as f64 >= share * expect,
            "{what}: layer {li} {} good of about {expect:.0} ({k:?})",
            k.good
        );
    }
}

fn params(mode: u8, guard: f64, cfg: Config, rate: f64) -> IsdbtGenParams {
    IsdbtGenParams {
        mode,
        guard,
        config: cfg,
        rate_hz: rate,
        ..Default::default()
    }
}

fn spacing(p: &IsdbtGenParams) -> f64 {
    ifft_rate(p.width_mhz) / fft_size(p.mode) as f64
}

#[test]
fn loop_back_at_the_ifft_rate() {
    let fs = ifft_rate(6.0);
    let cases = [
        (1u8, 0.125, config(false, [layer(3, 2, 0, 13), U, U]), 1.0),
        (
            2,
            0.125,
            config(true, [layer(1, 1, 0, 1), layer(2, 1, 0, 12), U]),
            1.6,
        ),
        (
            1,
            0.25,
            config(
                true,
                [layer(1, 0, 0, 1), layer(0, 0, 0, 4), layer(3, 4, 0, 8)],
            ),
            1.1,
        ),
        (3, 0.0625, config(false, [layer(1, 3, 0, 13), U, U]), 2.8),
        (
            2,
            0.125,
            config(false, [layer(0, 1, 3, 6), layer(2, 4, 1, 7), U]),
            2.6,
        ),
    ];
    for (mode, guard, cfg, secs) in cases {
        assert!(cfg.valid());
        let p = params(mode, guard, cfg, fs);
        let x = gen(&p, secs);
        let r = run("6", &x, fs, 65536);
        let what = format!("mode {mode} {cfg:?}");
        check_layers(&r, &p, secs, 0.95, 0, &what);
        let c = per_layer(&r);
        assert!(c.iter().all(|k| k.flagged == 0), "{what}: {c:?}");
    }
    // Mode 1 64QAM 3/4 at 28 dB.
    let p = params(1, 0.125, config(false, [layer(3, 2, 0, 13), U, U]), fs);
    let x = gen_with(&p, 1.0, 0.0, 0.0, Some(28.0));
    let r = run("6", &x, fs, 65536);
    check_layers(&r, &p, 1.0, 0.95, 0, "64QAM at 28 dB");
}

#[test]
fn receivers_with_offset_and_noise() {
    // mode 1 13 seg 64QAM 3/4, 10 Msps, CFO 7.3 kHz, 28 dB.
    let p = params(1, 0.125, config(false, [layer(3, 2, 0, 13), U, U]), 10e6);
    let x = gen_with(&p, 1.2, 7_300.0, 0.0, Some(28.0));
    let r = run("6", &x, 10e6, 65536);
    assert_eq!(r.f("mode"), 1.0);
    assert_eq!(r.f("detection"), 3.0, "{:?}", r.results.fields());
    assert!(
        (r.f("offset_hz") - 7_300.0).abs() < 0.2 * spacing(&p) + 50.0,
        "{}",
        r.f("offset_hz")
    );
    check_layers(&r, &p, 1.2, 0.95, 2, "mode 1");
    // mode 2 partial + 12 seg 16QAM 2/3, 8 Msps, CFO −5.1 kHz, 25 dB.
    let p = params(
        2,
        0.125,
        config(true, [layer(1, 1, 0, 1), layer(2, 1, 0, 12), U]),
        8e6,
    );
    let x = gen_with(&p, 1.7, -5_100.0, 0.0, Some(25.0));
    let r = run("6", &x, 8e6, 65536);
    assert_eq!(r.f("mode"), 2.0);
    assert!(
        (r.f("offset_hz") + 5_100.0).abs() < 0.2 * spacing(&p) + 50.0,
        "{}",
        r.f("offset_hz")
    );
    assert!(r.b("partial_reception"));
    check_layers(&r, &p, 1.7, 0.95, 2, "mode 2");
}

#[test]
fn the_two_symbol_bit_delay_model_is_found() {
    let mut p = params(1, 0.125, config(false, [layer(2, 2, 0, 13), U, U]), 8e6);
    p.bit_delay = BitDelay::TwoSymbols;
    let x = gen_with(&p, 1.4, 1_500.0, 0.0, None);
    let r = run("6", &x, 8e6, 65536);
    check_layers(&r, &p, 1.4, 0.9, 2, "mode 3");
}

#[test]
fn harder_channels() {
    // Mode 2 13 seg DQPSK 2/3 (no scattered pilots), 9 Msps, CFO 12 kHz, 20 dB, 10 ppm.
    let p = params(2, 0.125, config(false, [layer(0, 1, 0, 13), U, U]), 9e6);
    let y = gen_with(&p, 1.8, 12_000.0, 10.0, Some(20.0));
    let r = run("6", &y, 9e6, 65536);
    check_layers(&r, &p, 1.8, 0.9, 2, "DQPSK 13 segments");
    // Mode 1 partial + DQPSK + 64QAM at 12.288 Msps, CFO 9.1 kHz, 30 dB.
    let p = params(
        1,
        0.25,
        config(
            true,
            [layer(1, 1, 0, 1), layer(0, 0, 0, 4), layer(3, 2, 0, 8)],
        ),
        12.288e6,
    );
    let x = gen_with(&p, 1.2, 9_100.0, 0.0, Some(30.0));
    let r = run("6", &x, 12.288e6, 65536);
    check_layers(&r, &p, 1.2, 0.9, 2, "three layers");
    // Mode 2 16QAM 3/4 with an echo −6 dB at 40 samples, 22 dB.
    let p = params(2, 0.125, config(false, [layer(2, 2, 0, 13), U, U]), 8e6);
    let mut cfg = chain(0.0, 0.0, Some(22.0), bandwidth(&p));
    cfg.fading = Profile::Echo {
        delay_s: 40.0 / 8e6,
        level_db: -6.0,
        phase_deg: 0.0,
    };
    let x = impair(&clean(&p, 1.7), 8e6, &cfg);
    let r = run("6", &x, 8e6, 65536);
    check_layers(&r, &p, 1.7, 0.9, 2, "echo");
    // Mode 1 partial + QPSK 2/3 at 7 dB, 20 Msps, CFO −9 kHz, 5 ppm.
    let p = params(
        1,
        0.125,
        config(true, [layer(1, 1, 0, 1), layer(1, 1, 0, 12), U]),
        20e6,
    );
    let y = gen_with(&p, 1.2, -9_000.0, 5.0, Some(7.0));
    let r = run("6", &y, 20e6, 65536);
    check_layers(&r, &p, 1.2, 0.85, 4, "QPSK at 7 dB");
}

fn fault_case(
    what: &str,
    p: &IsdbtGenParams,
    secs: f64,
    f: impl Fn(Vec<Complex32>) -> Vec<Complex32>,
) -> Run {
    let x = f(gen(p, secs));
    let r = run("6", &x, p.rate_hz, 65536);
    check_layers(&r, p, secs, 0.9, 3, what);
    r
}

#[test]
fn real_world_faults() {
    let base = params(
        1,
        0.125,
        config(true, [layer(1, 1, 0, 1), layer(2, 1, 0, 12), U]),
        10e6,
    );
    // +80 ppm sample clock: the estimate within 15 ppm.
    let r = fault_case("+80 ppm", &base, 2.0, |x| {
        impair(&x, 10e6, &chain(0.0, 80.0, None, 0.0))
    });
    let ppm = r.f("clock_ppm");
    assert!((ppm.abs() - 80.0).abs() < 15.0, "clock {ppm}");
    // −43 kHz tuning error.
    let r = fault_case("-43 kHz", &base, 1.2, |x| {
        impair(&x, 10e6, &chain(-43_000.0, 0.0, None, 0.0))
    });
    assert!(
        (r.f("offset_hz") + 43_000.0).abs() < 500.0,
        "{}",
        r.f("offset_hz")
    );
    // Channel 1.3 MHz off centre.
    let r = fault_case("1.3 MHz off centre", &base, 1.2, |x| {
        impair(&x, 10e6, &chain(1.3e6, 0.0, None, 0.0))
    });
    assert!(
        (r.f("offset_hz") - 1.3e6).abs() < 2_000.0,
        "{}",
        r.f("offset_hz")
    );
    // I and Q swapped, +6 kHz.
    let r = fault_case("I and Q swapped", &base, 1.6, |x| {
        impair(&x, 10e6, &chain(6_000.0, 0.0, None, 0.0))
            .iter()
            .map(|v| Complex32::new(v.im, v.re))
            .collect()
    });
    assert!(r.b("mirrored"), "{:?}", r.results.fields());
    // 64 NaN and infinite samples at 0.4 s: more than 22 frames' worth of packets after them.
    let fl = frame_seconds(1, 0.125, 6.0);
    let mut x = gen(&base, 2.0);
    for v in x[4_000_000..4_000_064].iter_mut() {
        *v = Complex32::new(f32::NAN, f32::INFINITY);
    }
    let r = run("6", &x, 10e6, 65536);
    for (li, l) in base.config.layers.iter().enumerate().take(2) {
        let after = r
            .packets
            .iter()
            .filter(|(p, t)| *t > 0.401 && p[2] as usize == li && p[1] & 0x80 == 0)
            .count();
        assert!(
            after as f64 > 22.0 * ppf(1, l) as f64,
            "NaN: layer {li}: {after}"
        );
        assert_eq!(per_layer(&r)[li].wrong, 0);
    }
    // Starting mid-frame, then 12345 samples lost at 0.57 s: more than 8 frames' worth after the loss.
    let x = gen(&base, 2.0);
    let mut y = x[300_000..6_000_000].to_vec();
    y.extend_from_slice(&x[6_012_345..]);
    let r = run("6", &y, 10e6, 65536);
    for (li, l) in base.config.layers.iter().enumerate().take(2) {
        let after = r
            .packets
            .iter()
            .filter(|(p, t)| *t > 0.571 && p[2] as usize == li && p[1] & 0x80 == 0)
            .count();
        assert!(
            after as f64 > 8.0 * ppf(1, l) as f64,
            "drop: layer {li}: {after}"
        );
        assert_eq!(per_layer(&r)[li].wrong, 0);
    }
    let _ = fl;
    // Combined: +43 kHz, +80 ppm, echo 300 samples −6 dB (inside the guard of 1/4), DC −10 dB, 8-bit clip.
    let mut p = base.clone();
    p.guard = 0.25;
    let bw = bandwidth(&p);
    fault_case("combined", &p, 2.0, |x| {
        let cfg = ChainConfig {
            fading: Profile::Echo {
                delay_s: 300.0 / 10e6,
                level_db: -6.0,
                phase_deg: 0.0,
            },
            dc: Complex32::new(0.316, 0.0),
            adc_bits: Some(8),
            level_dbfs: -10.0,
            ..chain(43_000.0, 80.0, Some(30.0), bw)
        };
        impair(&x, 10e6, &cfg)
    });
}

#[test]
fn reconfiguration_is_followed() {
    let mut p = params(1, 0.125, config(false, [layer(3, 2, 0, 13), U, U]), 8e6);
    let new = config(true, [layer(1, 1, 0, 1), layer(2, 1, 0, 12), U]);
    p.switch = Some((18, new));
    let secs = 1.9;
    let x = gen(&p, secs);
    let r = run("6", &x, 8e6, 65536);
    assert!(r.b("partial_reception"), "{:?}", r.results.fields());
    let c = per_layer(&r);
    assert_eq!(c[1].wrong, 0);
    let fl = frame_seconds(1, 0.125, 6.0);
    assert!(
        c[1].good as usize >= 6 * ppf(1, &new.layers[1]),
        "{c:?} vs {}",
        (secs / fl) as usize
    );
}

#[test]
fn seven_and_eight_mhz_and_automatic_width() {
    let demo = IsdbtGenParams {
        payload: Payload::Programme,
        ..Default::default()
    };
    for (gw, setting, rate, cfo) in [
        (8.0, "8", 10e6, 0.0),
        (7.0, "7", 10e6, 20e3),
        (8.0, "auto", 10e6, 20e3),
        (7.0, "auto", 10e6, 0.0),
        (6.0, "auto", 10e6, 0.0),
    ] {
        let p = IsdbtGenParams {
            width_mhz: gw,
            rate_hz: rate,
            ..demo.clone()
        };
        let x = gen_with(&p, 3.6, cfo, 0.0, Some(28.0));
        let r = run(setting, &x, rate, 65536);
        let what = format!("{gw} MHz with {setting}");
        assert_eq!(r.f("width"), gw * 1e6, "{what}: {:?}", r.results.fields());
        assert_eq!(r.f("mode"), 3.0, "{what}");
        assert!(
            (r.f("offset_hz") - cfo).abs() < 500.0,
            "{what}: {}",
            r.f("offset_hz")
        );
        assert!(r.b("partial_reception"));
        let segs = r
            .results
            .decl()
            .table_id(sdrkit_stack_tvterr::typed::ISDBT_LAYERS.key)
            .and_then(|id| r.results.table(id))
            .and_then(|t| t.row("B"))
            .and_then(|row| row.at(&sdrkit_stack_tvterr::typed::IsdbtLayerRow::SEGMENTS));
        assert_eq!(segs, Some(12), "{what}");
        let pat = r
            .packets
            .iter()
            .filter(|(p, _)| p[1] & 0x1F == 0 && p[2] == 0)
            .count();
        assert!(pat > 0, "{what}: no PAT in {} packets", r.packets.len());
    }
}

#[test]
fn isdbt_any_cut_returns_the_same_packets() {
    let p = params(
        1,
        0.125,
        config(true, [layer(1, 1, 0, 1), layer(3, 2, 0, 12), U]),
        8e6,
    );
    let x = gen_with(&p, 0.9, 2_000.0, 0.0, Some(26.0));
    let a = run("6", &x, 8e6, 65536);
    let b = run("6", &x, 8e6, 4097);
    let c = run("6", &x, 8e6, 1000);
    assert!(!a.packets.is_empty());
    assert_eq!(a.packets, b.packets);
    assert_eq!(a.packets, c.packets);
}

#[test]
fn packet_times_follow_the_signal() {
    let p = params(1, 0.125, config(false, [layer(2, 1, 0, 13), U, U]), 8e6);
    let x = gen(&p, 1.0);
    let r = run("6", &x, 8e6, 65536);
    let t: Vec<f64> = r.packets.iter().map(|x| x.1).collect();
    assert!(t.windows(2).all(|w| w[1] >= w[0]));
    // The packets of a frame span the frame's time (within 50 ms of the signal span).
    let fl = frame_seconds(1, 0.125, 6.0);
    let n = ppf(1, &p.config.layers[0]) as f64;
    let implied = (t.len() as f64 / n) * fl;
    assert!(
        (implied - (t[t.len() - 1] - t[0])).abs() < 0.05 + fl,
        "{implied} vs {}",
        t[t.len() - 1] - t[0]
    );
}

#[test]
fn the_generator_has_unit_power() {
    for mode in 1..=3u8 {
        let p = params(mode, 0.125, config(false, [layer(2, 1, 0, 13), U, U]), 8e6);
        let x = gen(&p, 0.1);
        let pw: f64 = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
        assert!((pw - 1.0).abs() < 0.05, "mode {mode}: power {pw}");
    }
}

#[test]
fn mode_guard_and_offset_are_found() {
    for (mode, guard, rate, cfo, snr, width) in [
        (1u8, 0.25, 10e6, 7_300.0, 28.0, "6"),
        (2, 0.125, 8e6, -5_100.0, 25.0, "6"),
        (3, 0.125, 8e6, 1_500.0, 20.0, "6"),
        (3, 0.03125, 10e6, 300.0, 28.0, "6"),
        (3, 0.125, 10e6, 400.0, 28.0, "8"),
        (3, 0.125, 10e6, -200.0, 28.0, "7"),
    ] {
        let w = width.parse::<f64>().unwrap_or(6.0);
        let p = IsdbtGenParams {
            rate_hz: rate,
            mode,
            guard,
            width_mhz: w,
            ..Default::default()
        };
        let x = gen_with(&p, 0.4, cfo, 0.0, Some(snr));
        let r = run(width, &x, rate, 50_000);
        assert_eq!(
            r.f("mode"),
            mode as f64,
            "{mode} {guard}: {:?}",
            r.results.fields()
        );
        let g = sdrkit_stack_tvterr::isdbt::GUARDS
            .iter()
            .position(|&v| v == guard)
            .map(|i| ["1/4", "1/8", "1/16", "1/32"][i]);
        assert_eq!(Some(r.t("guard").as_str()), g, "{mode} {guard}");
        // Only the fractional part of the offset is known before the TMCC (after it, all of it): compare modulo one spacing.
        let spacing = ifft_rate(w) / fft_size(mode) as f64;
        let e = r.f("offset_hz");
        let diff = ((e - cfo) / spacing + 0.5).rem_euclid(1.0) - 0.5;
        assert!(
            diff.abs() * spacing < 0.2 * spacing + 50.0,
            "{mode}: cfo {e} vs {cfo}"
        );
        assert!(r.f("detection") >= 1.0);
    }
}

#[test]
fn noise_alone_finds_nothing_and_rates_are_checked() {
    let x = common::noise(4_000_000, 0.09, 2);
    let r = run("6", &x, 8e6, 65536);
    assert!(r.f("mode").is_nan(), "{:?}", r.results.fields());
    let mut rx = common::receiver(&DESCRIPTOR, &[("width", "6")]);
    assert!(rx
        .configure(&sdrkit_core::InputInfo::new(5e6, 473e6, 473e6))
        .is_err());
    let mut rx8 = common::receiver(&DESCRIPTOR, &[("width", "8")]);
    assert!(rx8
        .configure(&sdrkit_core::InputInfo::new(7e6, 473e6, 473e6))
        .is_err());
    assert!(rx8
        .configure(&sdrkit_core::InputInfo::new(8e6, 473e6, 473e6))
        .is_ok());
}

/// Packets of layer B (good, wrong) through the channel `cfg` gives, mode 3, 64-QAM 3/4, time interleaving 1.
fn layer_b_through(p: &IsdbtGenParams, cfg: &ChainConfig, seconds: f64) -> (u64, u64) {
    let x = impair(&clean(p, seconds), 8e6, cfg);
    let r = run("6", &x, 8e6, 65536);
    let c = per_layer(&r)[1];
    (c.good, c.wrong)
}

/// The generator of the S4 cases and its channel at `snr` dB, both seeded by `seed`.
fn s4_case(snr: Option<f64>, seed: u64) -> (IsdbtGenParams, ChainConfig) {
    let mut p = params(
        3,
        0.125,
        config(true, [layer(1, 1, 1, 1), layer(3, 2, 1, 12), U]),
        8e6,
    );
    p.seed = seed;
    let mut cfg = chain(0.0, 0.0, snr, bandwidth(&p));
    cfg.seed = seed;
    (p, cfg)
}

/// Packets of layer B (good, wrong) through TU6 at `doppler` Hz and `snr` dB, mode 3, 64-QAM 3/4, time interleaving 1, with
/// the channel and noise seeded by `seed`.
fn tu6_layer_b(doppler: f64, snr: Option<f64>, seconds: f64, seed: u64) -> (u64, u64) {
    use sdrkit_algo_sim::fading::profiles::Gsm;
    let (p, mut cfg) = s4_case(snr, seed);
    if doppler > 0.0 {
        cfg.fading = Profile::Gsm {
            model: Gsm::Tu6,
            max_doppler_hz: doppler,
        };
    }
    layer_b_through(&p, &cfg, seconds)
}

#[test]
fn time_interpolation_follows_tu6_at_10_hz() {
    // Mode 3, 64-QAM 3/4 in layer B with time interleaving 1, 3 s trials: a trial passes at half the clean run's packets and
    // no wrong packet. Each symbol's channel is interpolated in time between the pilots before and after it.
    let clean = tu6_layer_b(0.0, None, 3.0, 1).0;
    assert!(clean > 8000, "clean run {clean}");
    for seed in 1..=3 {
        let (good, wrong) = tu6_layer_b(10.0, Some(24.0), 3.0, seed);
        assert!(
            2 * good >= clean && wrong == 0,
            "TU6 10 Hz 24 dB seed {seed}: {good} of {clean}, {wrong} wrong"
        );
    }
    let (good, wrong) = tu6_layer_b(0.0, Some(17.0), 3.0, 1);
    assert!(
        2 * good >= clean && wrong == 0,
        "AWGN 17 dB: {good} of {clean}"
    );
}

#[test]
fn the_common_phase_follows_phase_noise() {
    // Mode 3, 64-QAM 3/4 in layer B, 22 dB with phase noise of -80 dBc/Hz at 10 kHz, 3 s: at least half the clean run's packets.
    use sdrkit_algo_sim::chain::PhaseNoiseSpec;
    let p = params(
        3,
        0.125,
        config(true, [layer(1, 1, 1, 1), layer(3, 2, 1, 12), U]),
        8e6,
    );
    let clean_good = tu6_layer_b(0.0, None, 3.0, 1).0;
    let mut cfg = chain(0.0, 0.0, Some(22.0), bandwidth(&p));
    cfg.phase_noise = Some(PhaseNoiseSpec {
        dbc_hz: -80.0,
        offset_hz: 10e3,
        floor_dbc_hz: None,
    });
    let x = impair(&clean(&p, 3.0), 8e6, &cfg);
    let c = per_layer(&run("6", &x, 8e6, 65536))[1];
    assert!(
        2 * c.good >= clean_good && c.wrong == 0,
        "{} of {clean_good}: {c:?}",
        c.good
    );
}

/// Trials of seeds 1 to `trials` that pass: at least half of `clean` packets of layer B and no wrong packet, 2 s each, through
/// the channel that `set` makes of the plain one at `snr` dB.
fn s4_passes(clean: u64, snr: f64, trials: u64, set: fn(&mut ChainConfig)) -> u64 {
    (1..=trials)
        .filter(|&seed| {
            let (p, mut cfg) = s4_case(Some(snr), seed);
            set(&mut cfg);
            let (good, wrong) = layer_b_through(&p, &cfg, 2.0);
            2 * good >= clean && wrong == 0
        })
        .count() as u64
}

#[test]
fn the_time_interpolated_estimate_reaches_its_cases() {
    // Mode 3, 64-QAM 3/4 in layer B with time interleaving 1, 2 s trials; a trial passes at half the clean run's packets.
    use sdrkit_algo_sim::chain::PhaseNoiseSpec;
    use sdrkit_algo_sim::fading::profiles::Gsm;
    let clean = tu6_layer_b(0.0, None, 2.0, 1).0;
    assert!(clean > 3500, "clean run {clean}");
    type Set = fn(&mut ChainConfig);
    // (case, dB, trials, trials that must pass, the channel).
    let cases: [(&str, f64, u64, u64, Set); 6] = [
        ("TU6 10 Hz", 22.0, 8, 8, |c| {
            c.fading = Profile::Gsm {
                model: Gsm::Tu6,
                max_doppler_hz: 10.0,
            }
        }),
        ("AWGN", 17.0, 4, 4, |_| {}),
        ("echo -6 dB 10 us", 18.0, 4, 4, |c| {
            c.fading = Profile::Echo {
                delay_s: 10e-6,
                level_db: -6.0,
                phase_deg: 40.0,
            }
        }),
        ("Rician K 10 dB, 10 Hz", 18.0, 4, 4, |c| {
            c.fading = Profile::Rician {
                k_db: 10.0,
                max_doppler_hz: 10.0,
                los_hz: 0.0,
            }
        }),
        ("phase noise -80 dBc/Hz at 10 kHz", 18.0, 4, 4, |c| {
            c.phase_noise = Some(PhaseNoiseSpec {
                dbc_hz: -80.0,
                offset_hz: 10e3,
                floor_dbc_hz: None,
            })
        }),
        ("Rician K 10 dB, 70 Hz", 22.0, 8, 7, |c| {
            c.fading = Profile::Rician {
                k_db: 10.0,
                max_doppler_hz: 70.0,
                los_hz: 0.0,
            }
        }),
    ];
    for (name, snr, trials, need, set) in cases {
        let pass = s4_passes(clean, snr, trials, set);
        assert!(pass >= need, "{name} at {snr} dB: {pass} of {trials}");
    }
}

#[test]
fn noise_alone_gives_no_packet_in_16_runs() {
    for seed in 1..=16 {
        let x = common::noise(4_000_000, 0.09, seed);
        let r = run("6", &x, 8e6, 65536);
        assert!(r.packets.is_empty(), "seed {seed}: {}", r.packets.len());
    }
}

#[test]
fn a_change_of_frequency_keeps_the_guide_and_forgets_the_signal() {
    use sdrkit_stack_tvterr::typed::shape;
    for id in ["isdbt", "isdbtsb"] {
        assert_eq!(
            shape(id).kept_keys(),
            Ok(vec![
                "table guide".to_owned(),
                "table guide_ratings".to_owned()
            ])
        );
    }
    let fs = ifft_rate(6.0);
    let p = params(1, 0.125, config(false, [layer(3, 2, 0, 13), U, U]), fs);
    let x = gen(&p, 1.0);
    let mut r = common::receiver(&DESCRIPTOR, &[("width", "6")]);
    let res = common::run(r.as_mut(), fs, &x, 65536, "");
    assert_eq!(res.f("detection"), 3.0);
    assert_eq!(res.f("mode"), 1.0);
    assert!(res.f("frames_ok") > 0.0 && res.rows("layers") == 1);
    let guide = res.rows("guide");
    r.forget();
    let after = Run {
        packets: Vec::new(),
        results: sdrkit_core::Owner::results(r.as_ref()).clone(),
        events: Vec::new(),
        requests: Vec::new(),
    };
    assert_eq!(after.f("detection"), 0.0);
    assert_eq!(after.f("frames_ok"), 0.0);
    assert!(after.f("mode").is_nan(), "no mode without a signal");
    assert!(after.f("frame_symbol").is_nan(), "no symbol without a TMCC");
    assert_eq!(after.rows("layers"), 0);
    assert_eq!(after.rows("services"), 0);
    assert_eq!(after.rows("guide"), guide);
}
