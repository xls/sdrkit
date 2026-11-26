//! The `halow` stack: generator to receiver round trips at every width, rate and impairment, cut independence, the coding on
//! the shared code and the known answers.

use super::help::{
    cnum, command, config, feed, halow_plan, halow_regions, impaired, num, rows, set, text,
};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_LEVEL_DBFS, GEN_NOISE, GEN_SNR_DB};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{
    Complex32, InputInfo, Owner, Receiver, Record, Results, SettingSet, StateBlob, Value,
};
use sdrkit_dev_test::Harness;
use sdrkit_graph::StackGraph;
use sdrkit_proto_wlan_phy::mac::push_fcs;
use sdrkit_stack_wlan::halow::coding::DataDecoder;
use sdrkit_stack_wlan::halow::gen::{
    schedule_bss, sent_in, test_schedule, GenPpdu, HalowGenParams, HalowGenerator,
};
use sdrkit_stack_wlan::halow::mac::{s1g_beacon, HalowBss};
use sdrkit_stack_wlan::halow::phy::{self, encode_data, sig_crc_bits, valid, Preamble, WIDTHS};
use sdrkit_stack_wlan::halow::table::{HalowNetwork, Store, WidthState};
use sdrkit_stack_wlan::halow::{self, HalowSetting, Receive};
use sdrkit_stack_wlan::typed::{
    HalowFrameRow as F, HalowPreambleRow, HalowWidthRow, HALOW_BY_PREAMBLE,
};

/// The impairments of a test signal.
#[derive(Clone, Copy)]
struct Imp {
    snr_db: f64,
    offset_hz: f64,
    ppm: f64,
    echo: Option<(f64, f64)>,
}

const CLEAN: Imp = Imp {
    snr_db: 30.0,
    offset_hz: 0.0,
    ppm: 0.0,
    echo: None,
};

fn generate(p: &HalowGenParams, imp: Imp, seconds: f64) -> Vec<Complex32> {
    let mut v: Vec<(&str, Value)> = vec![
        ("gen_noise", Value::Text("snr".into())),
        ("gen_snr_db", Value::F64(imp.snr_db)),
        ("gen_offset_hz", Value::F64(imp.offset_hz)),
        ("gen_clock_ppm", Value::F64(imp.ppm)),
    ];
    if let Some((d, g)) = imp.echo {
        v.push(("gen_fading", Value::Text("echo".into())));
        v.push(("gen_echo_delay_s", Value::F64(d)));
        v.push(("gen_echo_db", Value::F64(g)));
    }
    impaired(
        Box::new(HalowGenerator::new(p.clone())),
        &v,
        p.seed,
        seconds,
    )
}

/// The results of a run of the receiver, read from its results and its saved state.
#[derive(Debug)]
struct D {
    ok: u64,
    bad: u64,
    undecoded: u64,
    sig_ok: u64,
    preambles: u64,
    reasons: [u64; 5],
    per_width: [u64; 5],
    per_pre: [u64; 3],
    width_mask: u32,
    cfo_hz: f64,
    chan_hint: u64,
    tuned_mhz: f64,
    max_width_mhz: u32,
    networks: Vec<HalowNetwork>,
    log: Vec<Record>,
}

impl D {
    fn frames(&self) -> u64 {
        self.ok + self.undecoded
    }
}

/// The saved state of the network table: the signal clock and the store.
#[derive(serde::Deserialize)]
struct Saved {
    data: Store,
}

/// The network records the table keeps, read from the saved state of the network table component.
fn networks(rx: &StackGraph) -> Vec<HalowNetwork> {
    let Some(s) = rx.graph().component("table").and_then(|c| c.save_state()) else {
        return Vec::new();
    };
    let saved: Saved = serde_json::from_slice(s.bytes()).unwrap();
    saved
        .data
        .networks
        .iter()
        .map(|r| r.record.clone())
        .collect()
}

fn read(rx: &StackGraph) -> D {
    let r: &Results = Owner::results(rx);
    let n = |k: &str| num(r, k).unwrap_or(0.0) as u64;
    let mut per_width = [0u64; 5];
    let mut width_mask = 0u32;
    for row in rows(r, "widths") {
        let i = WIDTHS
            .iter()
            .position(|&w| w as f64 * 1e6 == cnum(row, &HalowWidthRow::WIDTH_HZ))
            .unwrap();
        per_width[i] = cnum(row, &HalowWidthRow::SIG_OK) as u64;
        if row.at(&HalowWidthRow::STATE) == Some(WidthState::Listening) {
            width_mask |= 1 << i;
        }
    }
    let mut per_pre = [0u64; 3];
    for row in rows(r, HALOW_BY_PREAMBLE.key) {
        let p: Preamble = row.at(&HalowPreambleRow::PREAMBLE).unwrap();
        per_pre[p.slot()] = cnum(row, &HalowPreambleRow::PPDUS) as u64;
    }
    D {
        ok: n("frames_ok"),
        bad: n("frames_bad"),
        undecoded: n("undecoded"),
        sig_ok: n("sig_ok"),
        preambles: n("preambles"),
        reasons: [
            n("mimo_frames"),
            n("stbc_frames"),
            n("ldpc_frames"),
            n("ampdu_frames"),
            n("ndp_frames"),
        ],
        per_width,
        per_pre,
        width_mask,
        cfo_hz: num(r, "offset_hz").unwrap_or(0.0),
        chan_hint: n("channel"),
        tuned_mhz: num(r, "tuned_hz").unwrap_or(0.0) / 1e6,
        max_width_mhz: (num(r, "max_width_hz").unwrap_or(0.0) / 1e6) as u32,
        networks: networks(rx),
        log: rows(r, "frames").into_iter().cloned().collect(),
    }
}

fn rx(rate: f64, centre: f64, tuned: f64, receive: &str) -> StackGraph {
    rx_in(rate, centre, tuned, receive, "EU")
}

/// A receiver with the plan of `region` of the data file.
fn rx_in(rate: f64, centre: f64, tuned: f64, receive: &str, region: &str) -> StackGraph {
    let plan = halow_plan(region);
    let mut settings: Vec<(&str, Value)> = vec![("receive", Value::Text(receive.into()))];
    settings.extend(plan.iter().map(|(k, v)| (k.as_str(), v.clone())));
    let cfg = config(HalowSetting::schema(), &settings);
    let mut r = halow::receiver(&cfg).unwrap();
    r.configure(&InputInfo::new(rate, centre, tuned).recording())
        .unwrap();
    r
}

fn run(r: &mut sdrkit_graph::StackGraph, x: &[Complex32], chunk: usize) -> D {
    feed(r, x, chunk);
    read(r)
}

fn net<'a>(d: &'a D, ssid: &str) -> Option<&'a HalowNetwork> {
    d.networks.iter().find(|n| n.ssid == ssid)
}

#[test]
fn the_whole_schedule() {
    let p = HalowGenParams {
        rate_hz: 6e6,
        ppdus: test_schedule(6e6),
        ..Default::default()
    };
    let x = generate(&p, CLEAN, 1.0);
    let sent = sent_in(&p, 1.0) as u64;
    let d = run(&mut rx(6e6, 863.5e6, 863.5e6, "auto14"), &x, 65536);
    assert_eq!(d.bad, 0);
    assert!(d.frames() * 10 > sent * 9, "{} of {sent}", d.frames());
    assert_eq!(
        d.networks.len(),
        4,
        "{:?}",
        d.networks.iter().map(|n| &n.ssid).collect::<Vec<_>>()
    );
    let h = net(&d, "HaLow-Home").unwrap();
    assert_eq!(
        (
            h.security.as_str(),
            h.width_mhz,
            h.op_width_mhz,
            h.mhz,
            h.channel
        ),
        ("WPA3-SAE", 1, 1, 864.5, 3)
    );
    let f = net(&d, "Farm-Sensors").unwrap();
    assert_eq!(
        (f.security.as_str(), f.width_mhz, f.mhz, f.interval_tu),
        ("WPA2-PSK", 2, 866.0, 200)
    );
    let c = net(&d, "OpenCam").unwrap();
    assert_eq!(
        (c.security.as_str(), c.width_mhz, c.op_width_mhz, c.mhz),
        ("Open", 4, 4, 904.0)
    );
    assert!(net(&d, "Meter-Net").is_some());
    let vendors: Vec<&str> = d.networks.iter().map(|n| n.vendor.as_str()).collect();
    assert!(
        vendors.contains(&"Espressif Inc.") && vendors.iter().any(|v| v.starts_with("Routerboard")),
        "{vendors:?}"
    );
    assert!(d.per_width[0] > 0 && d.per_width[1] > 0 && d.per_width[2] > 0);
    assert!(d.reasons[0] > 0 && d.undecoded == d.reasons[0]);
    assert_eq!((d.chan_hint, d.tuned_mhz), (1, 863.5));
    assert!(!d.log.is_empty() && d.log.len() <= 600);
    assert!(h.level_dbfs > -30.0 && h.level_dbfs < 1.0);
    assert!(d.log.iter().any(|l| text(l, &F::TYPE) == "PPDU"
        && cnum(l, &F::NSS) == 2.0
        && text(l, &F::DETAILS).contains("2 streams")));
}

fn test_net(width: u32) -> Vec<u8> {
    let mut b = HalowBss::new(
        [0x0C, 0xBF, 0x74, 0x00, 0x00, width as u8],
        "Test-Net",
        "WPA3-SAE",
        width,
        if width == 1 { 6 } else { 7 },
        3,
    );
    b.interval = 100;
    s1g_beacon(&b)
}

fn per_ppdu(g: &GenPpdu, gap: f64) -> f64 {
    let mut sig = g.spec.sig;
    sig.bw = g.spec.width;
    let lay = phy::Layout::of(g.spec.preamble, &sig, g.spec.width);
    (lay.total() + 300) as f64 / phy::grid(g.spec.width).0 + gap
}

fn one(width: u32, mcs: u8, rate: f64, snr: f64, n: usize, seed: u64) -> (D, u64) {
    let g = GenPpdu::new(test_net(width), width, mcs).gap(1e-3);
    let per = per_ppdu(&g, 1e-3);
    let p = HalowGenParams {
        rate_hz: rate,
        ppdus: vec![g],
        seed,
        ..Default::default()
    };
    let secs = p.lead_s + per * n as f64 - 0.5e-3;
    let x = generate(
        &p,
        Imp {
            snr_db: snr,
            ..CLEAN
        },
        secs,
    );
    let mut r = rx(
        rate,
        0.0,
        0.0,
        if width >= 8 { "auto_all" } else { "auto14" },
    );
    (run(&mut r, &x, 50_000), n as u64)
}

#[test]
fn sensitivity_near_the_threshold() {
    for (w, mcs, snr) in [(1, 0, 10.0), (1, 10, 9.0), (2, 0, 11.0), (4, 0, 14.0)] {
        let (d, n) = one(w, mcs, 6e6, snr, 30, 3);
        assert!(
            d.ok * 100 > n * 85,
            "{w} MHz MCS {mcs} at {snr} dB: {} of {n} (preambles {}, SIG {}, bad {})",
            d.ok,
            d.preambles,
            d.sig_ok,
            d.bad
        );
        assert_eq!(d.networks.len(), 1);
        assert_eq!(d.networks[0].width_mhz, w);
    }
    for (w, mcs) in [(1, 9), (2, 8), (4, 9), (8, 9), (16, 9)] {
        let rate = phy::width_rates(w).1;
        let (d, n) = one(w, mcs, rate, 32.0 + 10.0 * (w as f64).log10(), 10, 5);
        assert!(d.ok * 10 >= n * 8, "{w} MHz MCS {mcs}: {} of {n}", d.ok);
        assert!(d.bad <= 1);
    }
}

/// Mixed 1, 2 and 4 MHz PPDUs at MCS 0 for four trials of `n` PPDUs each: frames decoded and frames sent.
fn mixed_trials(imp: Imp, n: usize) -> (u64, u64) {
    let g: Vec<GenPpdu> = [1u32, 2, 4]
        .iter()
        .map(|&w| GenPpdu::new(test_net(w), w, 0).gap(1e-3))
        .collect();
    let per: f64 = g.iter().map(|p| per_ppdu(p, 1e-3)).sum::<f64>() / 3.0;
    let (mut ok, mut sent) = (0, 0);
    for seed in 0..4u64 {
        let p = HalowGenParams {
            rate_hz: 6e6,
            ppdus: g.clone(),
            seed: 100 + seed,
            looped: true,
            ..Default::default()
        };
        let secs = p.lead_s + per * n as f64;
        let x = generate(&p, imp, secs);
        let d = run(&mut rx(6e6, 0.0, 0.0, "auto14"), &x, 65536);
        ok += d.ok;
        sent += sent_in(&p, secs) as u64;
    }
    (ok, sent)
}

/// The cases of the preamble gate's figures that fall short: for each level (dB in 1 MHz, least share in %), on tune, with
/// +18 kHz and with a 50 ppm clock, the share of mixed 1, 2 and 4 MHz PPDUs decoded over four trials of 60.
fn gate_shortfalls(need: &[(f64, f64)]) -> Vec<String> {
    let mut short = Vec::new();
    for (offset_hz, ppm) in [(0.0, 0.0), (18e3, 0.0), (0.0, 50.0)] {
        for &(snr_db, pct) in need {
            let imp = Imp {
                snr_db,
                offset_hz,
                ppm,
                echo: None,
            };
            let (ok, sent) = mixed_trials(imp, 60);
            let got = 100.0 * ok as f64 / sent as f64;
            println!("{offset_hz} Hz {ppm} ppm {snr_db} dB: {ok} of {sent} ({got:.1} %)");
            if got < pct {
                short.push(format!(
                    "{offset_hz} Hz {ppm} ppm {snr_db} dB: {got:.1} % < {pct} %"
                ));
            }
        }
    }
    short
}

/// The preamble gate fires at 0.30 and re-arms under 0.20: the share of mixed 1, 2 and 4 MHz PPDUs decoded at 9, 7.5, 6 and
/// 4.5 dB in 1 MHz is at least 98, 87, 60 and 30 %, the same with +18 kHz and with a 50 ppm clock. A clean run decodes every
/// PPDU (as the 0.55 gate did), and noise alone gives no frame.
#[test]
fn preamble_gate_near_the_threshold() {
    let (ok, sent) = mixed_trials(CLEAN, 30);
    assert_eq!(ok, sent);
    let short = gate_shortfalls(&[(9.0, 98.0), (7.5, 87.0), (6.0, 60.0), (4.5, 30.0)]);
    assert!(short.is_empty(), "{short:#?}");
    let empty = HalowGenParams {
        rate_hz: 6e6,
        ppdus: Vec::new(),
        ..Default::default()
    };
    let x = impaired(Box::new(HalowGenerator::new(empty)), &floor(-23.0), 11, 2.0);
    let d = run(&mut rx(6e6, 0.0, 0.0, "auto14"), &x, 65536);
    assert_eq!(d.frames() + d.bad, 0);
}

/// The row's figure at 3 dB in 1 MHz: at least 12.6 % of mixed 1, 2 and 4 MHz PPDUs, on tune, with +18 kHz and with a 50 ppm
/// clock. There the 1 MHz signal is under the gate in the 4 MHz chain (normalised correlation about 0.29); the gate behind the
/// 1 MHz moving sum finds it.
#[test]
fn preamble_gate_at_3_db() {
    let short = gate_shortfalls(&[(3.0, 12.6)]);
    assert!(short.is_empty(), "{short:#?}");
}

#[test]
fn offsets_and_clock_error() {
    for w in [1u32, 2, 4] {
        for off in [-30e3, -12e3, 20e3, 30e3] {
            let p = HalowGenParams {
                rate_hz: 6e6,
                ppdus: vec![GenPpdu::new(test_net(w), w, 0).gap(1e-3)],
                ..Default::default()
            };
            let x = generate(
                &p,
                Imp {
                    snr_db: 28.0,
                    offset_hz: off,
                    ..CLEAN
                },
                0.12,
            );
            let sent = sent_in(&p, 0.12) as u64;
            let d = run(&mut rx(6e6, 0.0, 0.0, "auto14"), &x, 65536);
            assert!(d.ok * 10 > sent * 9, "{w} {off}: {} of {sent}", d.ok);
            // The signal can end inside a PPDU: that one fails its FCS.
            assert!(d.bad <= 1, "{w} {off}: {}", d.bad);
            assert!((d.cfo_hz - off).abs() < 1.5e3, "{w} {off}: {}", d.cfo_hz);
        }
    }
    for (w, mcs) in [(1u32, 10u8), (2, 0), (4, 0)] {
        for ppm in [40.0, -40.0] {
            let mut psdu = vec![0x33u8; 296];
            push_fcs(&mut psdu);
            let p = HalowGenParams {
                rate_hz: 6e6,
                ppdus: vec![GenPpdu::new(psdu, w, mcs).gap(1e-3)],
                ..Default::default()
            };
            let secs = if w == 1 { 0.25 } else { 0.1 };
            let x = generate(&p, Imp { ppm, ..CLEAN }, secs);
            let sent = sent_in(&p, secs) as u64;
            let d = run(&mut rx(6e6, 0.0, 0.0, "auto14"), &x, 65536);
            assert!(
                d.frames() * 10 > sent * 9,
                "{w} {ppm}: {} of {sent}",
                d.frames()
            );
            // The signal ends inside a PPDU: that one fails its FCS.
            assert!(d.bad <= 1, "{w} {ppm}: {}", d.bad);
        }
    }
}

#[test]
fn radio_rates_offsets_bad_fcs_and_back_to_back() {
    for rate in [2.0e6, 2.4e6, 5e6, 8e6, 10e6] {
        let mut ws = vec![1u32, 2];
        if rate >= 4e6 {
            ws.push(4);
        }
        for w in ws {
            let p = HalowGenParams {
                rate_hz: rate,
                ppdus: vec![GenPpdu::new(test_net(w), w, 0).gap(1e-3)],
                ..Default::default()
            };
            let x = generate(&p, CLEAN, 0.08);
            let sent = sent_in(&p, 0.08) as u64;
            let d = run(&mut rx(rate, 0.0, 0.0, "auto14"), &x, 65536);
            assert!(d.ok * 10 > sent * 8, "{rate} {w}: {} of {sent}", d.ok);
        }
    }
    let p = HalowGenParams {
        rate_hz: 6e6,
        ppdus: vec![GenPpdu::new(test_net(2), 2, 0).gap(1e-3)],
        ..Default::default()
    };
    let x = generate(
        &p,
        Imp {
            offset_hz: -400e3,
            ..CLEAN
        },
        0.1,
    );
    let sent = sent_in(&p, 0.1) as u64;
    let d = run(&mut rx(6e6, 866.4e6, 866.0e6, "auto14"), &x, 65536);
    assert!(d.ok * 10 > sent * 9, "{} of {sent}", d.ok);
    let p = HalowGenParams {
        rate_hz: 6e6,
        ppdus: vec![GenPpdu::new(test_net(2), 2, 0).gap(1e-3).bad_fcs()],
        ..Default::default()
    };
    let x = generate(&p, CLEAN, 0.1);
    let d = run(&mut rx(6e6, 0.0, 0.0, "auto14"), &x, 65536);
    assert_eq!(d.ok, 0);
    assert!(d.bad > 20, "{}", d.bad);
    assert!(d.networks.is_empty());
    let p = HalowGenParams {
        rate_hz: 6e6,
        ppdus: vec![
            GenPpdu::new(test_net(1), 1, 0).gap(1e-4),
            GenPpdu::new(test_net(2), 2, 0).gap(1e-4),
        ],
        ..Default::default()
    };
    let x = generate(&p, CLEAN, 0.1);
    let sent = sent_in(&p, 0.1) as u64;
    let d = run(&mut rx(6e6, 0.0, 0.0, "auto14"), &x, 65536);
    assert!(d.ok * 100 > sent * 85, "{} of {sent}", d.ok);
}

fn floor(db: f64) -> [(&'static str, Value); 2] {
    [
        ("gen_noise", Value::Text("floor".into())),
        ("gen_noise_floor_dbfs", Value::F64(db)),
    ]
}

#[test]
fn noise_tones_low_rate_save_restore_and_clear() {
    let empty = HalowGenParams {
        rate_hz: 6e6,
        ppdus: Vec::new(),
        ..Default::default()
    };
    let x = impaired(
        Box::new(HalowGenerator::new(empty.clone())),
        &floor(-23.0),
        9,
        1.5,
    );
    let d = run(&mut rx(6e6, 863.5e6, 863.5e6, "auto14"), &x, 65536);
    assert_eq!(d.frames() + d.bad, 0);
    assert!(d.networks.is_empty());
    let tone: Vec<Complex32> = x[..3_000_000]
        .iter()
        .enumerate()
        .map(|(i, n)| {
            Complex32::from_polar(0.5, (std::f64::consts::TAU * 100e3 * i as f64 / 6e6) as f32) + n
        })
        .collect();
    let d = run(&mut rx(6e6, 863.5e6, 863.5e6, "auto14"), &tone, 65536);
    assert_eq!(d.frames(), 0);
    assert!(d.preambles < 5, "{}", d.preambles);
    let wide = HalowGenParams {
        rate_hz: 20e6,
        ..empty
    };
    let mut y = impaired(Box::new(HalowGenerator::new(wide)), &floor(-23.0), 9, 0.2);
    y.extend((0..4_000_000).map(|i| {
        Complex32::from_polar(
            0.5,
            (std::f64::consts::TAU * 300e3 * i as f64 / 20e6) as f32,
        )
    }));
    let d = run(&mut rx(20e6, 905e6, 905e6, "auto_all"), &y, 65536);
    assert_eq!(d.frames(), 0);
    assert!(d.preambles < 10, "{}", d.preambles);
    let mut low = halow::receiver(&config(HalowSetting::schema(), &[])).unwrap();
    assert!(low
        .configure(&InputInfo::new(1e6, 863.5e6, 863.5e6))
        .is_err());
    let p = HalowGenParams {
        rate_hz: 6e6,
        ppdus: test_schedule(6e6),
        ..Default::default()
    };
    let x = generate(&p, CLEAN, 0.15);
    let mut r = rx(6e6, 863.5e6, 863.5e6, "auto14");
    let d = run(&mut r, &x, 65536);
    let state = r.save_state().unwrap();
    let mut o = rx(6e6, 863.5e6, 863.5e6, "auto14");
    o.restore_state(&state).unwrap();
    let e = read(&o);
    assert_eq!(e.networks.len(), d.networks.len());
    assert_eq!(e.log.len(), d.log.len());
    assert!(o
        .restore_state(&StateBlob::new("x", 1, b"{}".to_vec()))
        .is_err());
    command(&mut r, "clear", &[]).unwrap();
    let c = read(&r);
    assert!(c.networks.is_empty() && c.log.is_empty());
}

#[test]
fn the_schedule_at_20_10_and_4_msps() {
    let strong = Imp {
        snr_db: 45.0,
        ..CLEAN
    };
    let p = HalowGenParams {
        rate_hz: 20e6,
        ppdus: test_schedule(20e6),
        ..Default::default()
    };
    let x = generate(&p, strong, 1.2);
    let d = run(&mut rx(20e6, 905e6, 905e6, "auto_all"), &x, 65536);
    assert_eq!(d.bad, 0);
    assert_eq!(
        d.networks.len(),
        6,
        "{:?}",
        d.networks.iter().map(|n| &n.ssid).collect::<Vec<_>>()
    );
    assert_eq!(d.max_width_mhz, 16);
    assert!(d.per_width.iter().all(|&c| c > 0), "{:?}", d.per_width);
    let l = net(&d, "Lab-8MHz").unwrap();
    assert_eq!(
        (
            l.width_mhz,
            l.op_width_mhz,
            l.mhz,
            l.mcs,
            l.security.as_str()
        ),
        (8, 8, 906.0, 2, "WPA3-SAE")
    );
    let w = net(&d, "Wide-16MHz").unwrap();
    assert_eq!(
        (w.width_mhz, w.mhz, w.mcs, w.security.as_str()),
        (16, 910.0, 1, "Open")
    );
    let p = HalowGenParams {
        rate_hz: 10e6,
        ppdus: test_schedule(10e6),
        ..Default::default()
    };
    let x = generate(&p, strong, 0.5);
    let d = run(&mut rx(10e6, 905e6, 905e6, "auto_all"), &x, 65536);
    assert_eq!(d.networks.len(), 5);
    assert!(net(&d, "Lab-8MHz").is_some() && net(&d, "Wide-16MHz").is_none());
    assert_eq!(d.max_width_mhz, 8);
    let p = HalowGenParams {
        rate_hz: 4e6,
        ppdus: test_schedule(4e6),
        ..Default::default()
    };
    let x = generate(&p, strong, 0.3);
    let d = run(&mut rx(4e6, 905e6, 905e6, "auto_all"), &x, 65536);
    assert_eq!(d.max_width_mhz, 4);
    assert_eq!(d.networks.len(), 4);
    assert_eq!(d.per_width[3] + d.per_width[4], 0);
}

#[test]
fn one_width_only() {
    let p = HalowGenParams {
        rate_hz: 20e6,
        ppdus: test_schedule(20e6),
        ..Default::default()
    };
    let x = generate(
        &p,
        Imp {
            snr_db: 40.0,
            ..CLEAN
        },
        0.3,
    );
    let d = run(&mut rx(20e6, 905e6, 905e6, "2"), &x, 65536);
    assert_eq!(d.width_mask, 2);
    assert!(d.per_width[1] > 0);
    assert_eq!(
        d.per_width[0] + d.per_width[2] + d.per_width[3] + d.per_width[4],
        0
    );
    assert!(net(&d, "Farm-Sensors").is_some());
    assert!(net(&d, "HaLow-Home").is_none() && net(&d, "OpenCam").is_none());
}

fn random_psdu(rng: &mut Rng, len: usize) -> Vec<u8> {
    let mut psdu = vec![0u8; len];
    rng.fill_bytes(&mut psdu);
    push_fcs(&mut psdu);
    psdu
}

#[allow(clippy::too_many_arguments)]
fn count_good(
    width: u32,
    mcs: u8,
    rate: f64,
    offset: f64,
    n: usize,
    pre: Preamble,
    sgi: bool,
    snr: f64,
) -> (u64, u64) {
    let mut rng = Rng::new(width as u64 * 100 + mcs as u64);
    let mut g = GenPpdu::new(random_psdu(&mut rng, 120), width, mcs)
        .gap(0.6e-3)
        .preamble(pre);
    g.spec.sig.sgi = sgi;
    let per = per_ppdu(&g, 0.6e-3);
    let p = HalowGenParams {
        rate_hz: rate,
        ppdus: vec![g],
        ..Default::default()
    };
    let x = generate(
        &p,
        Imp {
            snr_db: snr,
            offset_hz: offset,
            ..CLEAN
        },
        p.lead_s + per * n as f64 - 0.3e-3,
    );
    let d = run(&mut rx(rate, 900e6, 900e6 + offset, "auto_all"), &x, 65536);
    (d.ok, d.bad)
}

#[test]
fn wide_channels_at_their_rates() {
    let cases: [(u32, f64, f64); 14] = [
        (8, 8e6, 0.0),
        (8, 10e6, 0.0),
        (8, 12.5e6, 0.0),
        (8, 16e6, 0.0),
        (8, 20e6, -5e6),
        (16, 16e6, 0.0),
        (16, 20e6, 0.0),
        (16, 25e6, 0.0),
        (16, 20e6, 1.5e6),
        (4, 5e6, 0.0),
        (4, 8e6, -2e6),
        (2, 2e6, 0.0),
        (2, 2.4e6, 0.0),
        (1, 2.4e6, 0.0),
    ];
    for (w, rate, off) in cases {
        let mcs = if w <= 2 { 4 } else { 9 };
        let pre = if w == 1 {
            Preamble::OneM
        } else {
            Preamble::Short
        };
        let (ok, bad) = count_good(w, mcs, rate, off, 10, pre, false, 50.0);
        assert!(
            ok >= 9 && bad == 0,
            "{w} MHz at {rate} offset {off}: {ok} good, {bad} bad"
        );
    }
    let (ok, bad) = count_good(1, 4, 10e6, 3e6, 10, Preamble::OneM, false, 50.0);
    assert!(
        ok >= 9 && bad == 0,
        "1 MHz 3 MHz above at 10 Msps: {ok} {bad}"
    );
}

#[test]
fn sig_only_ppdus() {
    for w in WIDTHS {
        for (k, reason) in ["spatial streams", "STBC", "LDPC", "A-MPDU", "NDP"]
            .iter()
            .enumerate()
        {
            let mcs = if w == 1 && *reason == "spatial streams" {
                1
            } else {
                2
            };
            let mut psdu = vec![0x5Au8; 60];
            push_fcs(&mut psdu);
            let mut g = GenPpdu::new(psdu, w, mcs).gap(1e-3);
            match *reason {
                "spatial streams" => g.spec.sig.nsts = 2,
                "STBC" => {
                    g.spec.sig.stbc = true;
                    g.spec.sig.nsts = 2;
                }
                "LDPC" => g.spec.sig.ldpc = true,
                "A-MPDU" => {
                    g.spec.sig.aggregated = true;
                    g.spec.sig.length = 6;
                }
                _ => {
                    g.spec.sig.ndp = true;
                    g.spec.sig.length = 0;
                }
            }
            let rate = phy::width_rates(w).1;
            let per = per_ppdu(&g, 1e-3);
            let p = HalowGenParams {
                rate_hz: rate,
                ppdus: vec![g],
                ..Default::default()
            };
            let x = generate(
                &p,
                Imp {
                    snr_db: 40.0,
                    ..CLEAN
                },
                p.lead_s + per * 10.0 - 0.5e-3,
            );
            let d = run(&mut rx(rate, 0.0, 0.0, "auto_all"), &x, 65536);
            assert!(
                d.reasons[k] >= 8,
                "{w} MHz {reason}: {} ({:?})",
                d.reasons[k],
                d.per_width
            );
            assert_eq!(d.ok + d.bad, 0, "{w} {reason}");
            let l = d
                .log
                .iter()
                .find(|l| matches!(text(l, &F::TYPE).as_str(), "PPDU" | "NDP"))
                .unwrap();
            assert_eq!(cnum(l, &F::WIDTH_HZ), w as f64 * 1e6);
            let want_nss = if *reason == "spatial streams" {
                2.0
            } else {
                1.0
            };
            assert_eq!(cnum(l, &F::NSS), want_nss, "{w} {reason}");
        }
    }
}

#[test]
fn beamformed_long_preamble() {
    for w in [2u32, 4, 8, 16] {
        for tilt in [0.05f32, -0.12] {
            let mcs = if w == 2 { 4 } else { 6 };
            let mut rng = Rng::new(7);
            let mut g = GenPpdu::new(random_psdu(&mut rng, 100), w, mcs)
                .gap(0.6e-3)
                .preamble(Preamble::Long);
            g.spec.sig.beamformed = true;
            g.spec.tilt = tilt;
            let rate = phy::width_rates(w).1;
            let per = per_ppdu(&g, 0.6e-3);
            let p = HalowGenParams {
                rate_hz: rate,
                ppdus: vec![g],
                ..Default::default()
            };
            let x = generate(
                &p,
                Imp {
                    snr_db: 50.0,
                    ..CLEAN
                },
                p.lead_s + per * 10.0 - 0.3e-3,
            );
            let d = run(&mut rx(rate, 0.0, 0.0, "auto_all"), &x, 65536);
            assert!(
                d.ok >= 9 && d.bad == 0,
                "{w} MHz tilt {tilt}: {} {}",
                d.ok,
                d.bad
            );
            assert!(d.per_pre[1] >= 9);
        }
    }
}

#[test]
fn invalid_combinations_are_refused() {
    for (w, mcs) in [(2u32, 9u8), (4, 10), (8, 10), (16, 10)] {
        assert!(!valid(w, mcs, 1));
        let mut psdu = vec![1u8; 40];
        push_fcs(&mut psdu);
        let rate = phy::width_rates(w).1;
        let p = HalowGenParams {
            rate_hz: rate,
            ppdus: vec![GenPpdu::new(psdu, w, mcs).gap(1e-3)],
            ..Default::default()
        };
        let x = generate(
            &p,
            Imp {
                snr_db: 40.0,
                ..CLEAN
            },
            0.03,
        );
        let d = run(&mut rx(rate, 0.0, 0.0, "auto_all"), &x, 65536);
        assert_eq!(d.sig_ok, 0, "{w} {mcs}");
        assert_eq!(d.frames() + d.bad, 0);
    }
}

#[test]
fn long_psdus_under_impairments() {
    let (w, mcs) = (16u32, 9u8);
    let mut rng = Rng::new(11);
    let g = GenPpdu::new(random_psdu(&mut rng, 507), w, mcs).gap(0.6e-3);
    let rate = phy::width_rates(w).1;
    let per = per_ppdu(&g, 0.6e-3);
    let p = HalowGenParams {
        rate_hz: rate,
        ppdus: vec![g],
        ..Default::default()
    };
    // The chain does not follow a clock error above about 15 ppm at 8 and 16 MHz from MCS 7.
    let x = generate(
        &p,
        Imp {
            snr_db: 55.0,
            offset_hz: 30e3 - 400e3,
            ppm: 10.0,
            echo: None,
        },
        p.lead_s + per * 6.0 - 0.3e-3,
    );
    let d = run(&mut rx(rate, 900.4e6, 900.0e6, "auto_all"), &x, 65536);
    assert!(
        d.ok >= 5 && d.bad == 0,
        "{w} MHz MCS {mcs}: {} {}",
        d.ok,
        d.bad
    );
}

#[test]
fn a_random_mixture_at_20_msps() {
    let mut rng = Rng::new(21);
    let mut ppdus = Vec::new();
    while ppdus.len() < 160 {
        let w = WIDTHS[rng.below(5) as usize];
        let mcs = rng.below(11) as u8;
        if !valid(w, mcs, 1) {
            continue;
        }
        let pre = if w == 1 {
            Preamble::OneM
        } else if rng.bit() == 1 {
            Preamble::Long
        } else {
            Preamble::Short
        };
        let len = 20 + rng.below(100) as usize;
        let mut g = GenPpdu::new(random_psdu(&mut rng, len), w, mcs)
            .preamble(pre)
            .gap(0.1e-3 + 1.2e-3 * rng.uniform())
            .offset(60e3 * rng.uniform() - 30e3);
        g.spec.sig.sgi = rng.bit() == 1;
        g.scale = 0.5 + rng.uniform();
        ppdus.push(g);
    }
    let p = HalowGenParams {
        rate_hz: 20e6,
        ppdus,
        looped: false,
        ..Default::default()
    };
    let x = generate(
        &p,
        Imp {
            snr_db: 55.0,
            offset_hz: 8e3 + 250e3,
            ..CLEAN
        },
        2.0,
    );
    let d = run(&mut rx(20e6, 905.25e6, 905.5e6, "auto_all"), &x, 65536);
    assert!(d.ok >= 144, "{} good of 160", d.ok);
    assert!(d.bad <= 1, "{}", d.bad);
}

#[test]
fn every_combination_under_three_impairment_sets() {
    let mut failures = Vec::new();
    for w in WIDTHS {
        let base_rate = phy::width_rates(w).1;
        for mcs in 0..=10u8 {
            if !valid(w, mcs, 1) {
                continue;
            }
            for pre in [Preamble::OneM, Preamble::Short, Preamble::Long] {
                if (pre == Preamble::OneM) != (w == 1) {
                    continue;
                }
                for sgi in [false, true] {
                    for set in 0..3 {
                        let rate = base_rate;
                        // The chain does not follow a clock error above about 15 ppm at 8 and 16 MHz from MCS 7.
                        let ppm = if w >= 8 && mcs >= 7 { 10.0 } else { 40.0 };
                        let mut rng = Rng::new(mcs as u64 * 7 + w as u64);
                        let mut g = GenPpdu::new(random_psdu(&mut rng, 60), w, mcs)
                            .preamble(pre)
                            .gap(0.5e-3);
                        g.spec.sig.sgi = sgi;
                        let per = per_ppdu(&g, 0.5e-3);
                        let p = HalowGenParams {
                            rate_hz: rate,
                            ppdus: vec![g],
                            ..Default::default()
                        };
                        let (imp, centre, tuned) = match set {
                            1 => (
                                Imp {
                                    snr_db: 55.0,
                                    offset_hz: 30e3 - 400e3,
                                    ppm,
                                    echo: None,
                                },
                                900.4e6,
                                900.0e6,
                            ),
                            2 => (
                                Imp {
                                    snr_db: 55.0,
                                    offset_hz: -30e3,
                                    ppm: -ppm,
                                    echo: Some((2e-6, -10.0)),
                                },
                                0.0,
                                0.0,
                            ),
                            _ => (
                                Imp {
                                    snr_db: 55.0,
                                    ..CLEAN
                                },
                                0.0,
                                0.0,
                            ),
                        };
                        let mut x = generate(&p, imp, p.lead_s + per * 5.0 - 0.3e-3);
                        let mut want = 4;
                        if set == 2 {
                            // Start in the middle of the first PPDU.
                            let skip = ((p.lead_s + per * 0.5) * rate) as usize;
                            x.drain(..skip.min(x.len()));
                            want = 3;
                        }
                        let d = run(&mut rx(rate, centre, tuned, "auto_all"), &x, 65536);
                        if d.ok < want || d.bad > 0 {
                            failures.push(format!(
                                "{w} MHz MCS {mcs} {pre:?} sgi {sgi} set {set}: {} good {} bad",
                                d.ok, d.bad
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn the_sub_channel_scan() {
    let mut morse = HalowBss::new(
        [0x0C, 0xBF, 0x74, 0x01, 0x02, 0x03],
        "MorseNet",
        "WPA3-SAE",
        4,
        3,
        16,
    );
    morse.primary_width = 1;
    for (off, rate) in [
        (0.5e6, 4e6),
        (-0.5e6, 4e6),
        (1.5e6, 6e6),
        (-1.5e6, 6e6),
        (2.5e6, 8e6),
        (-3.5e6, 10e6),
    ] {
        let p = HalowGenParams {
            rate_hz: rate,
            ppdus: vec![GenPpdu::new(s1g_beacon(&morse), 1, 0).gap(1e-3)],
            ..Default::default()
        };
        let x = generate(
            &p,
            Imp {
                offset_hz: off,
                ..CLEAN
            },
            0.05,
        );
        let d = run(&mut rx(rate, 0.0, 0.0, "auto14"), &x, 65536);
        assert_eq!(d.networks.len(), 1, "{off} at {rate}");
        assert_eq!(d.networks[0].security, "WPA3-SAE");
        // The signal can end inside a PPDU: that one fails its FCS.
        assert!(d.bad <= 1);
    }
    let mut wide2 = HalowBss::new(
        [0x0C, 0xBF, 0x74, 0x0A, 0x0B, 0x0C],
        "Wide2",
        "WPA3-SAE",
        4,
        3,
        16,
    );
    wide2.primary = 7;
    wide2.primary_width = 2;
    wide2.vendor = Some([0x0C, 0xBF, 0x74]);
    for (off, rate) in [(0.5e6, 4e6), (-1.0e6, 6e6), (1.5e6, 8e6), (-3.0e6, 10e6)] {
        let p = HalowGenParams {
            rate_hz: rate,
            ppdus: vec![GenPpdu::new(s1g_beacon(&wide2), 2, 0).gap(1e-3)],
            ..Default::default()
        };
        let x = generate(
            &p,
            Imp {
                offset_hz: off,
                ..CLEAN
            },
            0.05,
        );
        let d = run(&mut rx(rate, 0.0, 0.0, "auto14"), &x, 65536);
        assert_eq!(d.networks.len(), 1, "{off} at {rate}");
        assert!(d.networks[0].vendor_elements.starts_with("0C:BF:74"));
        assert_eq!(d.networks[0].primary_mhz, 905.5);
    }
    let bss = schedule_bss();
    let p = HalowGenParams {
        rate_hz: 6e6,
        ppdus: vec![
            GenPpdu::new(s1g_beacon(&bss[0]), 1, 0)
                .gap(1e-3)
                .offset(1.5e6),
            GenPpdu::new(s1g_beacon(&bss[1]), 2, 0)
                .gap(1e-3)
                .offset(-1.0e6),
        ],
        ..Default::default()
    };
    let x = generate(&p, CLEAN, 0.05);
    let d = run(&mut rx(6e6, 0.0, 0.0, "auto14"), &x, 65536);
    assert!(net(&d, "HaLow-Home").is_some() && net(&d, "Farm-Sensors").is_some());
}

#[test]
fn cuts_do_not_change_the_result() {
    let p = HalowGenParams {
        rate_hz: 6e6,
        ppdus: test_schedule(6e6),
        ..Default::default()
    };
    let x = generate(&p, CLEAN, 0.12);
    let a = run(&mut rx(6e6, 863.5e6, 863.5e6, "auto14"), &x, 997);
    let b = run(&mut rx(6e6, 863.5e6, 863.5e6, "auto14"), &x, 65536);
    assert!(a.ok > 10);
    assert_eq!((a.ok, a.undecoded, a.sig_ok), (b.ok, b.undecoded, b.sig_ok));
    let la: Vec<_> = a
        .log
        .iter()
        .map(|l| (cnum(l, &F::SIGNAL_S), text(l, &F::DETAILS)))
        .collect();
    let lb: Vec<_> = b
        .log
        .iter()
        .map(|l| (cnum(l, &F::SIGNAL_S), text(l, &F::DETAILS)))
        .collect();
    assert_eq!(la, lb);
}

#[test]
fn the_harness_runs_the_descriptor() {
    let d = &halow::HALOW;
    assert_eq!(
        (d.tuning.min_rate_hz, d.tuning.rate_hz, d.tuning.default_hz),
        (2e6, 6e6, 0.0)
    );
    let run = sdrkit_dev_test::Harness::new(d)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 30.0)
        .seconds(0.8)
        .run()
        .unwrap();
    assert!(num(&run.results, "frames_ok").unwrap() > 15.0);
    assert_eq!(halow::rate_for(Receive::AutoAll), 20e6);
    assert_eq!(halow::rate_for(Receive::Auto14), 6e6);
}

#[test]
fn scan_and_channel_ask_the_host() {
    let mut r = rx(6e6, 863.5e6, 863.5e6, "auto14");
    assert!(set(&mut r, "channel", Value::F64(99.0)).is_err());
    set(&mut r, "scan", Value::Bool(true)).unwrap();
    let fed = feed(&mut r, &vec![Complex32::default(); 60_000], 60_000);
    assert_eq!(fed.requests.len(), 1);
    match &fed.requests[0] {
        sdrkit_core::HostRequest::Hop {
            channels, dwell_s, ..
        } => {
            assert!(
                !channels.is_empty() && channels.iter().all(|&f| (863e6..869e6).contains(&f)),
                "{channels:?}"
            );
            assert_eq!(*dwell_s, 6.0);
        }
        q => panic!("{q:?}"),
    }
}

/// The plans of two regions of the data file give the scan, the channel and the hint of each.
#[test]
fn the_plan_of_two_regions_drives_scan_channel_and_hint() {
    let regions = halow_regions();
    for code in ["EU", "US"] {
        let row = regions.iter().find(|r| r.code == code).unwrap();
        let first = row.centre_hz(row.channels[0][0]);
        let mut r = rx_in(6e6, first, first, "auto14", code);
        set(&mut r, "scan", Value::Bool(true)).unwrap();
        set(&mut r, "list_width", Value::Text("4".into())).unwrap();
        set(&mut r, "scan", Value::Bool(false)).unwrap();
        let fed = feed(&mut r, &vec![Complex32::default(); 60_000], 60_000);
        let hops: Vec<Vec<f64>> = fed
            .requests
            .iter()
            .filter_map(|q| match q {
                sdrkit_core::HostRequest::Hop { channels, .. } => Some(channels.clone()),
                _ => None,
            })
            .collect();
        let want =
            |w: usize| -> Vec<f64> { row.channels[w].iter().map(|&n| row.centre_hz(n)).collect() };
        let mut expect = vec![want(0)];
        if !row.channels[2].is_empty() {
            expect.push(want(2));
        }
        assert_eq!(hops, expect, "{code}");
        assert_eq!(
            num(Owner::results(&r), "channel"),
            Some(f64::from(row.channels[0][0])),
            "{code}"
        );
        let last = *row.channels[2].last().unwrap_or(&0);
        assert_eq!(
            set(&mut r, "channel", Value::F64(f64::from(last))).is_ok(),
            last > 0,
            "{code}"
        );
    }
}

/// The SIG CRC-4, the BLE CRC-24 and whitening and the 802.11b PLCP CRC give their published check values over "123456789".
#[test]
fn the_named_crcs_have_known_answers() {
    use sdrkit_util_bits::crc::catalog::{CRC16_GENIBUS, CRC24_BLE, CRC4_INTERLAKEN};
    assert_eq!(CRC4_INTERLAKEN.checksum(b"123456789"), 0xB);
    let mut bits = Vec::new();
    sdrkit_util_bits::pack::unpack(b"123456789", sdrkit_util_bits::BitOrder::Msb, &mut bits);
    assert_eq!(sig_crc_bits(&bits), [1, 0, 1, 1]);
    assert_eq!(CRC24_BLE.checksum(b"123456789"), 0xC25A56);
    assert_eq!(CRC16_GENIBUS.checksum(b"123456789"), 0xD64E);
    // The whitening of channel 37 (Core Vol 6 Part B clause 3.2): the register starts at 1 followed by the channel index, so
    // whitening zeros produces the sequence itself; doing it twice restores the data.
    let mut z = [0u8; 4];
    sdrkit_proto_wlan_phy::ble::whiten(&mut z, 37).unwrap();
    assert_ne!(z, [0; 4]);
    sdrkit_proto_wlan_phy::ble::whiten(&mut z, 37).unwrap();
    assert_eq!(z, [0; 4]);
}

/// No source file of the crate reads the environment.
#[test]
fn the_receivers_do_not_read_the_environment() {
    fn walk(d: &std::path::Path, hits: &mut Vec<String>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, hits);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let s = std::fs::read_to_string(&p).unwrap();
                if s.contains("env::var") {
                    hits.push(p.display().to_string());
                }
            }
        }
    }
    let mut hits = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut hits,
    );
    assert!(hits.is_empty(), "{hits:?}");
}

/// The data decoder allocates nothing once it has decoded a frame.
#[test]
fn the_data_decoder_allocates_nothing_after_the_first_frame() {
    for (w, mcs) in [(1u32, 10u8), (2, 3), (4, 7), (16, 9)] {
        let mut rng = Rng::new(3);
        let psdu = random_psdu(&mut rng, 200);
        let set = sdrkit_stack_wlan::halow::coding::points(
            sdrkit_stack_wlan::halow::coding::mcs_params(mcs).unwrap().0,
        );
        let mut dmap = sdrkit_stack_wlan::halow::coding::Demapper::new(set.bits() as usize);
        let syms = encode_data(&psdu, w, mcs, 0x5D);
        let mut llrs: Vec<Vec<f32>> = Vec::new();
        for s in &syms {
            let mut v = Vec::new();
            for &p in s {
                dmap.demap(p, 10.0, &mut v);
            }
            llrs.push(v);
        }
        let d = phy::n_dbps(w, mcs, 1).unwrap();
        let mut dec = DataDecoder::new();
        let decode = |dec: &mut DataDecoder| {
            dec.start(phy::n_es(d));
            for l in &llrs {
                dec.symbol(l, w, phy::n_sd(w), mcs);
            }
            dec.finish(psdu.len(), mcs).map(|p| p.to_vec())
        };
        assert_eq!(
            decode(&mut dec).as_deref(),
            Some(&psdu[..]),
            "{w} MHz MCS {mcs}"
        );
        let before = super::alloc::count();
        super::alloc::counting(true);
        for _ in 0..5 {
            dec.start(phy::n_es(d));
            for l in &llrs {
                dec.symbol(l, w, phy::n_sd(w), mcs);
            }
            assert!(dec.finish(psdu.len(), mcs).is_some());
        }
        super::alloc::counting(false);
        assert_eq!(super::alloc::count() - before, 0, "{w} MHz MCS {mcs}");
    }
}

/// Each display filter of the `wifi`, `ble` and `halow` tables limits the rows its table publishes.
#[test]
fn the_display_filters_of_all_three_tables_limit_their_rows() {
    let count = |d: &'static sdrkit_core::StackDescriptor,
                 table: &str,
                 set: &dyn Fn(Harness) -> Harness,
                 seconds: f64,
                 freq: f64| {
        let mut h = Harness::new(d).seconds(seconds).freq(freq);
        h = set(h);
        rows(&h.run().unwrap().results, table).len()
    };
    let none = |h: Harness| h;
    let ble = &sdrkit_stack_wlan::ble::BLE;
    assert_eq!(count(ble, "devices", &none, 0.6, 2402e6), 17);
    assert_eq!(
        count(
            ble,
            "devices",
            &|h: Harness| h.set(&sdrkit_stack_wlan::ble::table::FILTER_TEXT, "c0:ff:ee"),
            0.6,
            2402e6
        ),
        1
    );
    assert_eq!(
        count(
            ble,
            "devices",
            &|h: Harness| h.set(&sdrkit_stack_wlan::ble::table::MIN_LEVEL, 0.0),
            0.6,
            2402e6
        ),
        0
    );
    let wifi = &sdrkit_stack_wlan::wifi::WIFI;
    assert_eq!(count(wifi, "networks", &none, 0.3, 2437e6), 3);
    assert_eq!(
        count(
            wifi,
            "networks",
            &|h: Harness| h.set(&sdrkit_stack_wlan::wifi::table::OPEN_ONLY, true),
            0.3,
            2437e6
        ),
        1
    );
    assert_eq!(
        count(
            wifi,
            "networks",
            &|h: Harness| h.set(&sdrkit_stack_wlan::wifi::table::FILTER_TEXT, "cafe"),
            0.3,
            2437e6
        ),
        1
    );
    let h = &halow::HALOW;
    let all = count(h, "networks", &none, 0.6, 863.5e6);
    assert!(all >= 3, "{all}");
    assert_eq!(
        count(
            h,
            "networks",
            &|h: Harness| h.set(&halow::table::FILTER_TEXT, "farm"),
            0.6,
            863.5e6
        ),
        1
    );
    assert_eq!(
        count(
            h,
            "networks",
            // The PPDUs are put at -10 dBFS, so the highest floor (0 dBFS) hides them all.
            &|h: Harness| h
                .set(&halow::table::MIN_LEVEL, 0.0)
                .set(&GEN_LEVEL_DBFS, -10.0),
            0.6,
            863.5e6
        ),
        0
    );
}

/// Noise alone, 300 trials of 0.1 s at 6 Msps: no frame, and the preamble gates open fewer than 10 times (2 measured, all by
/// the gate behind the 1 MHz moving sum; the chain's own gate does not open).
#[test]
fn preamble_gates_on_noise() {
    let (mut gates, mut frames) = (0, 0);
    for seed in 0..300u64 {
        let empty = HalowGenParams {
            rate_hz: 6e6,
            ppdus: Vec::new(),
            ..Default::default()
        };
        let x = impaired(
            Box::new(HalowGenerator::new(empty)),
            &floor(-23.0),
            500 + seed,
            0.1,
        );
        let d = run(&mut rx(6e6, 0.0, 0.0, "auto14"), &x, 65536);
        gates += d.preambles;
        frames += d.frames() + d.bad;
    }
    println!("gates {gates}, frames {frames}");
    assert_eq!(frames, 0);
    assert!(gates < 10, "{gates}");
}
