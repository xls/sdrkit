//! RBDS against RDS, retune and reset, the descriptor and the receiver contract.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::{
    ConfigError, GenConfig, InputInfo, LockState, Output, Receiver, SettingSet, StackConfig,
    TableId, Value, Values,
};
use sdrkit_stack_fm::pty::pi_from_call_letters;
use sdrkit_stack_fm::typed::{FmFields, STATION_NAMED, TEXT_DONE};
use sdrkit_stack_fm::FmSetting;
use sdrkit_stack_fm::{GenParams, DESCRIPTOR};

const RATE: f64 = 2e6;

fn rbds_case(tuned_mhz: f64, region: &str, pi: u16, pty: u8) -> sdrkit_stack_fm::Reading {
    let mut rx = receiver(RATE, tuned_mhz * 1e6, &[("rds_variant", region)]);
    let p = sig(
        GenParams {
            rate_hz: RATE,
            pi,
            pty,
            gain: 0.3,
            ..Default::default()
        },
        40.0,
    );
    run(&mut rx, p, 2.0, |_| {}).last
}

#[test]
fn rbds_against_rds() {
    let wxyz = pi_from_call_letters("WXYZ");
    let d = rbds_case(101.1, "auto", wxyz, 10);
    assert_eq!(
        (d.pi, d.rbds, d.programme.as_str(), d.call_letters.as_str()),
        (wxyz, true, "Country", "WXYZ"),
        "auto on the US raster"
    );
    let d = rbds_case(100.0, "auto", wxyz, 10);
    assert_eq!(
        (d.rbds, d.programme.as_str(), d.call_letters.as_str()),
        (false, "Pop Music", ""),
        "auto off the US raster"
    );
    let d = rbds_case(101.1, "rds", wxyz, 10);
    assert_eq!(
        (d.pi, d.rbds, d.programme.as_str(), d.call_letters.as_str()),
        (wxyz, false, "Pop Music", ""),
        "forced RDS"
    );
    let d = rbds_case(100.0, "rbds", wxyz, 10);
    assert_eq!(
        (d.rbds, d.programme.as_str(), d.call_letters.as_str()),
        (true, "Country", "WXYZ"),
        "forced RBDS"
    );
    let d = rbds_case(100.0, "rbds", 0xD318, 1);
    assert_eq!(
        (d.pi, d.programme.as_str(), d.call_letters.as_str()),
        (0xD318, "News", ""),
        "forced RBDS, a PI without call letters"
    );
}

/// A station outside North America on an odd tenth (Sydney on 104.1 MHz) whose PI falls in the K range: "KGEJ" must never be shown
/// once the station's ECC or the user's band says where it is.
#[test]
fn call_letters_only_in_north_america() {
    let pi = pi_from_call_letters("KGEJ");
    assert_eq!(pi, 0x2049);
    let station = |extra_groups: Vec<[u16; 3]>| {
        sig(
            GenParams {
                rate_hz: RATE,
                pi,
                pty: 10,
                extra_groups,
                gain: 0.3,
                ..Default::default()
            },
            40.0,
        )
    };
    // Group 1A, variant 0: the extended country code of Australia (0xF0); B carries TP and PTY 10.
    let ecc_au = vec![[0x1000 | 1 << 10 | 10 << 5, 0x00F0, 0x0000]];
    let call = |rx: &Rx| {
        Owner::results(rx)
            .fields()
            .at(&FmFields::CALL_LETTERS)
            .unwrap_or_default()
            .to_owned()
    };

    // Auto on the US raster, nothing says otherwise: RBDS.
    let mut rx = receiver(RATE, 104.1e6, &[("rds_variant", "auto")]);
    let d = run(&mut rx, station(Vec::new()), 2.0, |_| {}).last;
    assert_eq!((d.rbds, d.call_letters.as_str()), (true, "KGEJ"));

    // The station sends the Australian ECC: RDS meanings, no call letters.
    let mut rx = receiver(RATE, 104.1e6, &[("rds_variant", "auto")]);
    let d = run(&mut rx, station(ecc_au.clone()), 3.0, |_| {}).last;
    assert_eq!(d.ecc, Some(0xF0));
    assert_eq!(
        (d.rbds, d.programme.as_str(), d.call_letters.as_str()),
        (false, "Pop Music", "")
    );
    assert!(call(&rx).is_empty());

    // No ECC, but the host chose RDS: the same.
    let mut rx = receiver(RATE, 104.1e6, &[("rds_variant", "rds")]);
    let d = run(&mut rx, station(Vec::new()), 2.0, |_| {}).last;
    assert_eq!(
        (d.rbds, d.programme.as_str(), d.call_letters.as_str()),
        (false, "Pop Music", "")
    );
    assert_eq!(
        Owner::results(&rx).fields().at(&FmFields::PROGRAMME),
        Some("Pop Music")
    );
    assert!(call(&rx).is_empty());

    // Forced RBDS wins over both.
    let mut rx = receiver(RATE, 104.1e6, &[("rds_variant", "rbds")]);
    let d = run(&mut rx, station(ecc_au), 3.0, |_| {}).last;
    assert_eq!((d.rbds, d.call_letters.as_str()), (true, "KGEJ"));

    // Changing the choice to RDS takes the call letters away at once, field included (they were never cleared before).
    let mut rx = receiver(RATE, 104.1e6, &[("rds_variant", "rbds")]);
    let mut g = station(Vec::new()).source();
    let mut feed = |rx: &mut Rx, seconds: f64| {
        let mut out = Output::default();
        for _ in 0..(seconds * 10.0) as usize {
            let mut x = vec![sdrkit_core::Complex32::default(); (RATE / 10.0) as usize];
            g.generate(&mut x);
            out.clear();
            rx.feed(&x, &mut out);
        }
    };
    feed(&mut rx, 2.0);
    assert_eq!(call(&rx), "KGEJ");
    set(&mut rx, "rds_variant", "rds").unwrap();
    feed(&mut rx, 0.5);
    assert!(call(&rx).is_empty());
    assert_eq!(detail(&rx).call_letters, "");
}

#[test]
fn a_retune_keeps_nothing_of_the_old_station() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let first = run(
        &mut rx,
        sig(
            GenParams {
                rate_hz: RATE,
                gain: 0.3,
                ..Default::default()
            },
            40.0,
        ),
        1.5,
        |_| {},
    );
    assert_eq!(first.last.station, "SDRKIT  ");
    rx.retuned(&InputInfo {
        rate_hz: RATE,
        centre_hz: 101.1e6,
        tuned_hz: 101.1e6,
        live: true,
    });
    // Straight after the retune: the empty values.
    let d = detail(&rx);
    assert_eq!(
        (d.state, d.station.as_str(), d.text.as_str(), d.pi, d.groups),
        (0, "", "", 0, 0)
    );
    let r = Owner::results(&rx);
    assert_eq!(r.status().lock_state, LockState::Searching);
    assert!((0..r.decl().tables().len())
        .all(|i| r.table(TableId(i as u16)).is_none_or(|t| t.is_empty())));
    assert!((0..r.decl().series_decls().len()).all(|i| r
        .series(sdrkit_core::SeriesId(i as u16))
        .is_none_or(|s| s.points.is_empty())));
    // A different station on the new frequency: only its own data, with RBDS meanings at 101.1 MHz.
    let other = sig(
        GenParams {
            rate_hz: RATE,
            ps: "OTHER".into(),
            pi: 0x1234,
            rt: "Second".into(),
            gain: 0.3,
            ..Default::default()
        },
        40.0,
    );
    let second = run(&mut rx, other, 3.5, |_| {});
    for r in &second.reports {
        assert!(
            r.station.is_empty() || r.station == "OTHER   ",
            "{:?}",
            r.station
        );
        assert!(r.pi == 0 || r.pi == 0x1234);
    }
    assert_eq!(second.last.station, "OTHER   ");
    assert_eq!(second.last.text, "Second");
    assert!(second.last.rbds);
    assert_eq!(second.last.programme, "Country");
}

#[test]
fn rds_data_survives_a_loss_of_signal_but_not_a_retune() {
    let mut rx = receiver(RATE, 100e6, &[]);
    run(
        &mut rx,
        sig(
            GenParams {
                rate_hz: RATE,
                gain: 0.3,
                ..Default::default()
            },
            40.0,
        ),
        1.5,
        |_| {},
    );
    // The station stops: noise only.
    let mut noise = noise_sigma(0.01, 1);
    let gone = run_with(&mut rx, RATE, 1.5, |buf, n| {
        buf.resize(n, sdrkit_core::Complex32::default());
        noise.process(buf);
    });
    let d = &gone.last;
    assert!(!d.rds && d.state == 0);
    assert_eq!(d.station, "SDRKIT  ", "the name is kept when sync is lost");
}

#[test]
fn settings_change_rds_meanings_and_correction_live() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let p = sig(
        GenParams {
            rate_hz: RATE,
            gain: 0.3,
            ..Default::default()
        },
        40.0,
    );
    run(&mut rx, p.clone(), 1.0, |_| {});
    assert_eq!(detail(&rx).programme, "Pop Music");
    set(&mut rx, "rds_variant", "rbds").unwrap();
    set(&mut rx, "rds_correction", "0").unwrap();
    set(&mut rx, "rds_charset", "0").unwrap();
    run(&mut rx, p, 0.5, |_| {});
    assert_eq!(detail(&rx).programme, "Country");
    assert!(detail(&rx).rbds);
}

#[test]
fn the_descriptor_and_the_contract() {
    assert_eq!(DESCRIPTOR.id, "fm");
    assert_eq!(DESCRIPTOR.group, "Broadcast radio");
    assert_eq!(DESCRIPTOR.tuning.rate_hz, 4e6);
    assert_eq!(DESCRIPTOR.tuning.min_rate_hz, 500e3);
    assert_eq!(DESCRIPTOR.tuning.bandwidth_hz, 250e3);
    let schema = (DESCRIPTOR.settings)();
    for key in [
        "stereo",
        "force_mono",
        "deemphasis",
        "rds_variant",
        "squelch",
        "channel_width",
        "audio_bandwidth",
        "bass",
        "treble",
        "warm",
        "rds_correction",
        "rds_charset",
    ] {
        let id = schema.id(key).unwrap_or_else(|_| panic!("{key} missing"));
        assert!(
            !schema.setting(id).unwrap().help.is_empty(),
            "{key} has no help"
        );
    }
    let values = Values::defaults(&schema);
    let mut rx = (DESCRIPTOR.make_receiver)(&StackConfig::new(values.clone())).unwrap();
    assert!(matches!(
        rx.configure(&InputInfo {
            rate_hz: 250e3,
            centre_hz: 100e6,
            tuned_hz: 100e6,
            live: true,
        }),
        Err(ConfigError::RateTooLow { .. })
    ));
    rx.configure(&InputInfo {
        rate_hz: RATE,
        centre_hz: 100e6,
        tuned_hz: 100e6,
        live: true,
    })
    .unwrap();
    // The generator from the descriptor, configured through the settings.
    let mut settings = values;
    settings
        .set(schema.id("gen_ps").unwrap(), &Value::Text("DESCR".into()))
        .unwrap();
    let mut g = (DESCRIPTOR.make_generator.unwrap())(&GenConfig::new(RATE, settings).with_seed(1))
    .unwrap();
    assert_eq!(g.rate_hz(), RATE);
    let mut out = Output::default();
    let mut buf = vec![sdrkit_core::Complex32::default(); 200_000];
    for _ in 0..15 {
        g.generate(&mut buf);
        out.clear();
        rx.feed(&buf, &mut out);
        assert!(out
            .audio_runs()
            .all(|p| p.samples.len() % usize::from(p.channels.max(1)) == 0));
    }
    let d = sdrkit_stack_fm::Reading::from_results(Owner::results(rx.as_ref()));
    assert_eq!(d.station, "DESCR   ");
    // The generic results and the scopes.
    let r = Owner::results(rx.as_ref());
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert!(r.status().snr_db.unwrap() > 40.0);
    assert_eq!(r.fields().at(&FmFields::STATION), Some("DESCR   "));
    let mpx = r.series(r.decl().series_id("mpx").unwrap()).unwrap();
    assert!(
        mpx.line && (640..=670).contains(&mpx.points.len()),
        "{} MPX points",
        mpx.points.len()
    );
    let peak19 = mpx
        .points
        .iter()
        .filter(|p| (p[0] - 19e3).abs() < 150.0)
        .map(|p| p[1])
        .fold(f32::MIN, f32::max);
    assert!(
        (peak19 - 20.0 * 0.09f32.log10()).abs() < 2.0,
        "pilot at {peak19} dB"
    );
    let sym = r
        .series(r.decl().series_id("rds_symbols").unwrap())
        .unwrap();
    assert_eq!(sym.points.len(), 200);
    assert!(sym.points.iter().all(|p| (p[0].abs() - 1.0).abs() < 0.3));
    // Feeding before configure does nothing. A NaN is harmless.
    let mut fresh =
        (DESCRIPTOR.make_receiver)(&StackConfig::new(Values::defaults(&FmSetting::schema())))
            .unwrap();
    fresh.feed(&[sdrkit_core::Complex32::new(f32::NAN, 0.0)], &mut out);
    assert_eq!(
        Owner::results(fresh.as_ref()).status().lock_state,
        LockState::Off
    );
}

#[test]
fn the_group_monitor_logs_groups() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let monitor = rx.graph().target("rds.monitor").unwrap();
    rx.graph_mut()
        .apply_to(monitor, &Value::Bool(true))
        .unwrap();
    run(
        &mut rx,
        sig(
            GenParams {
                rate_hz: RATE,
                gain: 0.3,
                ..Default::default()
            },
            40.0,
        ),
        1.0,
        |_| {},
    );
    let log = Owner::results(&rx).log_lines();
    assert!(log.iter().any(|l| l == "4A21 0548 E26A 5344"), "{log:?}");
}

/// Every event the receiver sends on the test signal is declared, with its fields, and carries the tuned frequency.
#[test]
fn every_event_is_declared() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let mut g = sig(
        GenParams {
            rate_hz: RATE,
            ..Default::default()
        },
        40.0,
    )
    .source();
    let mut out = Output::default();
    let mut kinds = Vec::new();
    for _ in 0..40 {
        let mut x = vec![sdrkit_core::Complex32::default(); (RATE / 10.0) as usize];
        g.generate(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
        for e in &out.events {
            assert!(e.is(&STATION_NAMED) || e.is(&TEXT_DONE), "{}", e.kind());
            assert_eq!(e.freq_hz, 100e6, "{}", e.kind());
            kinds.push(e.kind().to_owned());
        }
    }
    assert!(kinds.iter().any(|k| k == "fm.station"));
    assert!(kinds.iter().any(|k| k == "fm.text"));
}
