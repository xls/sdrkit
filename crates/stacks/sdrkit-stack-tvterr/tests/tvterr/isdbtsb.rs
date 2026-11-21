//! ISDB-TSB: the geometry of the narrow bands, the TMCC of the test signal, and round trips through the shared impairments at
//! the limits of ITU-R BS.1114 Annex 3 and the stack's acceptance list.

use super::common::{self, impair, Run};
use sdrkit_algo_sim::chain::ChainConfig;
use sdrkit_algo_sim::noise::NoiseLevel;
use sdrkit_core::{Complex32, Values};
use sdrkit_proto_mpegts::source::NumberedCheck;
use sdrkit_stack_tvterr::isdbt::chain::packets_per_frame;
use sdrkit_stack_tvterr::isdbt::gen::{
    config, layer, layer_pattern, IsdbtGenParams, IsdbtGenerator,
};
use sdrkit_stack_tvterr::isdbt::layout::{is_sp, Band, EdgePilot, Layout};
use sdrkit_stack_tvterr::isdbt::tmcc::{decode_soft, Config, LayerParams, Tmcc};
use sdrkit_stack_tvterr::isdbt::{frame_seconds, GUARDS};
use sdrkit_stack_tvterr::isdbtsb::{test_params, DESCRIPTOR};

const U: LayerParams = LayerParams::UNUSED;

/// Acceptance 1: the carrier roles of one symbol over the band, with the upper-edge pilot, give the counts of BS.1114 Annex 3
/// table 9 for modes 1 to 3, 1 and 3 segments and every count of differential segments; the FFT sizes too.
#[test]
fn geometry_follows_bs1114_table_9() {
    for mode in 1..=3u8 {
        let m = 1usize << (mode - 1);
        for ns in [1usize, 3] {
            let band = Band::of(ns, EdgePilot::Sequence);
            assert_eq!(band.fft_size(mode), if ns == 1 { 128 } else { 256 } << mode);
            for nd in 0..=ns {
                let nc = ns - nd;
                let mut layers = [U; 3];
                if nd > 0 {
                    layers[0] = layer(0, 0, 0, nd as u8);
                }
                if nc > 0 {
                    layers[usize::from(nd > 0)] = layer(1, 0, 0, nc as u8);
                }
                let cfg = config(false, layers);
                assert!(cfg.within(ns).is_some(), "{cfg:?}");
                let lay = Layout::new(mode, &cfg, band);
                let (mut data, mut sp, mut cp, mut tmcc, mut ac1, mut ac2) = (0, 0, 1, 0, 0, 0);
                for s in 0..ns {
                    let t = &lay.tables[s];
                    data += lay.data[s * 4].len();
                    tmcc += t.tmcc.len();
                    ac1 += t.ac1.len();
                    ac2 += t.ac2.len();
                    if lay.diff[s] {
                        cp += 1;
                    } else {
                        sp += (0..lay.ls).filter(|&k| is_sp(k, 0)).count();
                    }
                }
                let what = format!("mode {mode} Ns {ns} nd {nd}");
                assert_eq!(lay.nc, 108 * m * ns + 1, "{what}");
                assert_eq!(data, 96 * m * ns, "{what}");
                assert_eq!(sp, 9 * m * nc, "{what}");
                assert_eq!(cp, nd + 1, "{what}");
                assert_eq!(tmcc, m * nc + 5 * m * nd, "{what}");
                assert_eq!(ac1, 2 * m * ns, "{what}");
                let ac2_want = [4, 9, 19][mode as usize - 1] * nd;
                assert_eq!(ac2, ac2_want, "{what}");
                assert_eq!(data + sp + cp + tmcc + ac1 + ac2, lay.nc, "{what}");
            }
        }
    }
    // The example: mode 1, Ns = 3, nd = 1, nc = 2: 325 = 288 + 18 + 2 + 7 + 6 + 4.
    let lay = Layout::new(
        1,
        &config(false, [layer(0, 0, 0, 1), layer(1, 0, 0, 2), U]),
        Band::of(3, EdgePilot::Sequence),
    );
    assert_eq!(lay.nc, 325);
    // The band's segments are 1, 0, 2 from low to high, and the narrow pilot sequence is the stretch of the full band's.
    assert_eq!(Band::of(3, EdgePilot::Sequence).segments(), &[1, 0, 2]);
    let full = Layout::new(1, &config(false, [layer(1, 0, 0, 13), U, U]), Band::FULL);
    let one = Layout::new(
        1,
        &config(false, [layer(1, 0, 0, 1), U, U]),
        Band::of(1, EdgePilot::Sequence),
    );
    for c in 0..one.nc {
        assert_eq!(one.pilot(c), full.pilot(6 * 108 + c));
    }
    let plus = Layout::new(
        1,
        &config(false, [layer(1, 0, 0, 1), U, U]),
        Band::of(1, EdgePilot::Plus),
    );
    assert!(plus.top_pilot() > 0.0);
}

/// Acceptance 2: the TMCC of the default test signal, its 102 information bits, and the (184,102) code with four errors.
#[test]
fn the_tmcc_of_the_test_signal() {
    let p = test_params(&Values::defaults(&(DESCRIPTOR.settings)()), 2.048e6, 1);
    let next = Config {
        partial: true,
        layers: [U; 3],
    };
    let t = Tmcc {
        system: 1,
        countdown: 15,
        emergency: false,
        current: p.config,
        next,
    };
    assert_eq!(p.config.layers[0], layer(1, 1, 2, 1));
    assert_eq!(p.config.layers[1], layer(3, 2, 1, 2));
    let info = t.info_bits();
    let mut hex = String::new();
    let bits: Vec<bool> = info.iter().copied().chain([false, false]).collect();
    for nib in bits.chunks(4) {
        let v = nib.iter().fold(0u8, |a, &b| a << 1 | b as u8);
        hex.push(char::from_digit(v as u32, 16).unwrap().to_ascii_uppercase());
    }
    assert_eq!(hex, "7D250B44BFFFFFFFFFFFFFFFFC");
    let back = Tmcc::from_info(&info);
    assert_eq!(back.current, t.current);
    assert_eq!(back.system, 1);
    assert_eq!(back.current.within(3), Some(t.current));
    assert_eq!(back.current.within(1).map(|c| c.total()), Some(1));
    // Four errors on the least reliable bits still decode.
    let word = t.frame_bits(false, false);
    let mut soft: Vec<f32> = word[20..]
        .iter()
        .enumerate()
        .map(|(i, &b)| if b { -1.0 } else { 1.0 } * (2.0 + (i % 7) as f32))
        .collect();
    for i in [3usize, 50, 101, 170] {
        soft[i] = -soft[i].signum() * 0.2;
    }
    let dec = decode_soft(&soft).expect("decodes");
    assert_eq!(Tmcc::from_info(&dec[..102]), back);
}

struct Case {
    what: &'static str,
    ns: usize,
    mode: u8,
    guard: f64,
    cfg: Config,
    rate: f64,
    width: f64,
    place: f64,
    cfo: f64,
    ppm: f64,
    drift: f64,
    snr: f64,
    setting: &'static str,
}

fn case(what: &'static str, ns: usize, mode: u8, guard: f64, cfg: Config, rate: f64) -> Case {
    Case {
        what,
        ns,
        mode,
        guard,
        cfg,
        rate,
        width: 6.0,
        place: 0.0,
        cfo: 0.0,
        ppm: 0.0,
        drift: 0.0,
        snr: 40.0,
        setting: "auto",
    }
}

fn gen_params(c: &Case) -> IsdbtGenParams {
    IsdbtGenParams {
        rate_hz: c.rate,
        mode: c.mode,
        guard: c.guard,
        width_mhz: c.width,
        config: c.cfg,
        band: Band::of(c.ns, EdgePilot::Sequence),
        next_unset: c.ns != 13,
        ..Default::default()
    }
}

/// The occupied width of the signal (the SNR is stated in it).
fn bandwidth(p: &IsdbtGenParams) -> f64 {
    let b = p.band;
    b.carriers(p.mode) as f64 * b.ifft_rate(p.width_mhz) / b.fft_size(p.mode) as f64
}

/// Seconds of signal for about `want` packets of the slowest layer after lock and the interleavers.
fn seconds_for(c: &Case, want: f64) -> f64 {
    let fl = frame_seconds(c.mode, c.guard, c.width);
    let least = c
        .cfg
        .layers
        .iter()
        .filter(|l| l.used())
        .map(|l| packets_per_frame(c.mode, l))
        .min()
        .unwrap_or(1) as f64;
    // A change-over from one segment to three locks twice.
    let twice = if c.ns == 3 && c.setting == "auto" {
        1.5
    } else {
        0.0
    };
    1.0 + twice + fl * (want / least + 4.5)
}

fn signal(c: &Case, seconds: f64) -> Vec<Complex32> {
    let p = gen_params(c);
    let x = common::generate(IsdbtGenerator::new(p.clone()).unwrap(), seconds);
    let chain = ChainConfig {
        offset_hz: c.place + c.cfo,
        clock_ppm: c.ppm,
        drift_hz_s: c.drift,
        noise: NoiseLevel::SnrDb {
            snr_db: c.snr,
            bandwidth_hz: bandwidth(&p),
        },
        ..Default::default()
    };
    impair(&x, c.rate, &chain)
}

fn receive(c: &Case, x: &[Complex32]) -> Run {
    let width = format!("{}", c.width as u32);
    let mut r = common::receiver(&DESCRIPTOR, &[("segments", c.setting), ("width", &width)]);
    common::run(r.as_mut(), c.rate, x, 65536, "merge.ts")
}

fn per_layer(r: &Run) -> [NumberedCheck; 3] {
    std::array::from_fn(|li| r.check(&layer_pattern(li)))
}

/// "Clean": at least `least` good packets on each of `layers` and at most 2 % bad (wrong or flagged).
fn clean(r: &Run, layers: &[usize], least: u64, what: &str) {
    let k = per_layer(r);
    for &li in layers {
        let c = k[li];
        let bad = c.wrong + c.flagged;
        assert!(
            c.good >= least && bad as f64 <= 0.02 * (c.good + bad) as f64,
            "{what}: layer {li} {c:?}; detection {} segments {} offset {}; {:?}",
            r.f("detection"),
            r.f("band_segments"),
            r.f("offset_hz"),
            r.results.log_lines()
        );
    }
}

fn run_case(c: &Case, layers: &[usize], least: u64) -> Run {
    let x = signal(c, seconds_for(c, least as f64 * 1.2));
    let r = receive(c, &x);
    clean(&r, layers, least, c.what);
    r
}

/// Acceptance 3: one-segment round trips.
#[test]
fn one_segment_round_trips() {
    let one = |m, r, i| config(false, [layer(m, r, i, 1), U, U]);
    let mut c = case("mode 3 QPSK 1/2", 1, 3, 0.125, one(1, 0, 2), 1.2e6);
    (c.ppm, c.cfo, c.snr) = (25.0, 3_170.0, 18.0);
    run_case(&c, &[0], 150);
    let mut c = case(
        "mode 2 16QAM 2/3 off centre",
        1,
        2,
        0.0625,
        one(2, 1, 1),
        2.048e6,
    );
    (c.place, c.ppm, c.cfo, c.drift, c.snr) = (400e3, -40.0, -1_234.0, 20.0, 24.0);
    run_case(&c, &[0], 150);
    let mut c = case("mode 1 DQPSK 1/2", 1, 1, 0.25, one(0, 0, 1), 1.0e6);
    (c.cfo, c.ppm, c.snr, c.setting) = (900.0, 10.0, 16.0, "1");
    run_case(&c, &[0], 150);
    let mut c = case("mode 3 64QAM 3/4 8 MHz", 1, 3, 0.0625, one(3, 2, 1), 1.5e6);
    (c.width, c.cfo, c.snr) = (8.0, 2_500.0, 30.0);
    run_case(&c, &[0], 150);
}

/// Acceptance 3: three-segment round trips, automatic change-over and the timing-step case of all-DQPSK signals.
#[test]
fn three_segment_round_trips() {
    let mut c = case(
        "A QPSK 1/2 + B 16QAM 1/2, automatic",
        3,
        3,
        0.125,
        config(true, [layer(1, 0, 1, 1), layer(2, 0, 1, 2), U]),
        2.4e6,
    );
    (c.cfo, c.ppm, c.snr) = (-2_700.0, 15.0, 22.0);
    let r = run_case(&c, &[0, 1], 150);
    assert_eq!(r.f("band_segments"), 3.0);
    let mut c = case(
        "one layer 64QAM 2/3 off centre",
        3,
        2,
        0.125,
        config(false, [layer(3, 1, 1, 3), U, U]),
        3.2e6,
    );
    (c.place, c.cfo, c.snr, c.setting) = (-500e3, 7_300.0, 28.0, "3");
    run_case(&c, &[0], 150);
    let mut c = case(
        "all DQPSK, guard 1/4",
        3,
        3,
        0.25,
        config(false, [layer(0, 0, 1, 1), layer(0, 1, 1, 2), U]),
        2.048e6,
    );
    (c.cfo, c.snr) = (1_500.0, 20.0);
    run_case(&c, &[0, 1], 150);
    let mut c = case(
        "A DQPSK 2/3 + B 16QAM 1/2 with drift",
        3,
        3,
        0.125,
        config(true, [layer(0, 1, 1, 1), layer(2, 0, 1, 2), U]),
        2.4e6,
    );
    (c.ppm, c.cfo, c.drift, c.snr) = (30.0, -2_300.0, 15.0, 21.0);
    run_case(&c, &[0, 1], 150);
    let mut c = case(
        "all DQPSK mode 2 off centre",
        3,
        2,
        0.125,
        config(false, [layer(0, 0, 1, 1), layer(0, 1, 1, 2), U]),
        2.048e6,
    );
    (c.ppm, c.place, c.cfo, c.snr) = (-35.0, 300e3, 3_700.0, 19.0);
    run_case(&c, &[0, 1], 100);
}

/// Acceptance 3: the one-segment service of a wider signal: a 3-segment ISDB-TSB signal at a rate too low for three segments,
/// and a 13-segment ISDB-T signal.
#[test]
fn the_one_segment_service_of_wider_signals() {
    let mut c = case(
        "3 segments at 1 Msps",
        3,
        3,
        0.125,
        config(true, [layer(1, 1, 1, 1), layer(3, 4, 1, 2), U]),
        1.0e6,
    );
    c.snr = 30.0;
    let x = signal(&c, seconds_for(&c, 180.0));
    let r = receive(&c, &x);
    clean(&r, &[0], 150, c.what);
    assert_eq!(per_layer(&r)[1].good, 0);
    assert_eq!(r.f("band_segments"), 1.0);
    // A 13-segment ISDB-T signal at its IFFT rate, resampled to 1.2 Msps.
    let p = IsdbtGenParams {
        rate_hz: 8e6,
        mode: 3,
        guard: 0.125,
        config: config(true, [layer(1, 1, 1, 1), layer(3, 2, 1, 12), U]),
        ..Default::default()
    };
    let secs = 1.0 + frame_seconds(3, 0.125, 6.0) * (130.0 / 32.0 + 4.5);
    let full = common::generate(IsdbtGenerator::new(p.clone()).unwrap(), secs);
    let mut rs = sdrkit_stack_tvterr::band::resampler(8e6, 2.4e6, 0.5e6, 1.2e6).unwrap();
    let mut mid = Vec::new();
    rs.process(&full, &mut mid);
    let mut rs = sdrkit_stack_tvterr::band::resampler(2.4e6, 1.2e6, 0.26e6, 0.6e6).unwrap();
    let mut x = Vec::new();
    rs.process(&mid, &mut x);
    let mut r = common::receiver(&DESCRIPTOR, &[("segments", "1")]);
    let r = common::run(r.as_mut(), 1.2e6, &x, 65536, "merge.ts");
    let k = per_layer(&r);
    assert!(k[0].good > 100, "{:?} {:?}", k[0], r.results.fields());
    assert_eq!(k[1].good + k[2].good, 0);
}

/// Acceptance 4: every mode and guard with one segment of QPSK 1/2.
#[test]
fn all_modes_and_guards() {
    for mode in 1..=3u8 {
        for (gi, &g) in GUARDS.iter().enumerate() {
            let mut c = case(
                "modes",
                1,
                mode,
                g,
                config(false, [layer(1, 0, 0, 1), U, U]),
                1.024e6,
            );
            (c.cfo, c.snr, c.setting) = (1_100.0, 25.0, "1");
            let secs = if mode == 3 && gi == 3 { 3.0 } else { 2.5 };
            let r = receive(&c, &signal(&c, secs));
            let k = per_layer(&r)[0];
            assert!(k.good >= 20 && k.wrong == 0, "mode {mode} guard {g}: {k:?}");
        }
    }
}

/// Acceptance 5: every modulation and code rate of layer A in a 3-segment signal.
#[test]
fn all_modulations_and_code_rates() {
    for m in 0..4u8 {
        for r in 0..5u8 {
            let b = if m == 0 {
                layer(0, 0, 1, 2)
            } else {
                layer(2, 0, 1, 2)
            };
            let mut c = case(
                "rates",
                3,
                3,
                0.125,
                config(true, [layer(m, r, 1, 1), b, U]),
                2.2e6,
            );
            (c.cfo, c.snr, c.setting) = (-800.0, 32.0, "3");
            let x = signal(&c, seconds_for(&c, 24.0));
            let k = per_layer(&receive(&c, &x))[0];
            assert!(k.good >= 20 && k.wrong == 0, "A mod {m} rate {r}: {k:?}");
        }
    }
}

/// Acceptance 6: the stack's own test signal at 2.048 Msps for 7 s.
#[test]
fn the_own_test_signal() {
    let d = &DESCRIPTOR;
    let mut g = (d.make_generator.unwrap())(
        &sdrkit_core::GenConfig::new(2.048e6, Values::defaults(&(d.settings)())).with_seed(3),
    )
    .unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (7.0 * 2.048e6) as usize];
    g.generate(&mut x);
    let mut r = common::receiver(d, &[]);
    let r = common::run(r.as_mut(), 2.048e6, &x, 65536, "merge.ts");
    let bad = r.packets.iter().filter(|(p, _)| p[1] & 0x80 != 0).count();
    let good = r.packets.len() - bad;
    assert_eq!(r.f("band_segments"), 3.0, "{:?}", r.results.fields());
    assert_eq!(r.f("detection"), 3.0);
    assert!(
        good > 300 && bad as f64 <= 0.02 * r.packets.len() as f64,
        "{good} good {bad} bad"
    );
}
