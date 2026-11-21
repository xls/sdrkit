//! Meteor-M LRPT: generator, standard impairments, receiver, the settings and the saved state.

use super::common::{
    assert_declared, feed, fields_without, impaired, invoke, run, set, table_rows, Ran,
};
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{Complex32, InputInfo, Owner, StateBlob};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::instruments::msumr::{decode_blocks, encode_blocks};
use sdrkit_stack_satimg::modes::meteor::{MeteorSource, DESCRIPTOR};

fn harness(rate: f64, secs: f64) -> Harness {
    Harness::new(&DESCRIPTOR).rate(rate).seconds(secs)
}

/// Runs `h`'s impaired signal through its receiver, with the strips of `msumr.out`.
fn go(h: &Harness, rate: f64) -> Ran {
    let x = h.generate().expect("a signal");
    run::<Strip>(h, &x, rate, 137.9e6, 137.9e6, 40_000, Some("msumr.out"))
}

/// The number of strips of channel 1 that equal the JPEG round trip of the test picture (of `period` rows). Every strip of
/// channel 1 must equal it, at one offset of rows found from the first strip.
fn strips_exact(strips: &[Strip], period: u32) -> usize {
    let expect = |gr: u32, k: u8| -> Vec<u8> {
        let px = MeteorSource::strip_pixels(1, gr, k * 14);
        let mut out = vec![0u8; 8 * 112];
        decode_blocks(&encode_blocks(&px, 14, 80), 14, 80, &mut out).unwrap();
        out
    };
    let ch1: Vec<&Strip> = strips.iter().filter(|s| s.channel == 1).collect();
    let Some(first) = ch1.first() else { return 0 };
    let row0 = first.first_row / 8;
    let k0 = (first.x / 112) as u8;
    let off = (0..period)
        .find(|&o| *first.samples == expect((row0 + o) % period, k0)[..])
        .expect("a matching row");
    for s in &ch1 {
        let row = s.first_row / 8;
        let k = (s.x / 112) as u8;
        assert_eq!(
            &*s.samples,
            &expect((row + off) % period, k)[..],
            "row {row} part {k}"
        );
    }
    ch1.len()
}

#[test]
fn oqpsk_72k_with_offset_clock_error_and_noise() {
    let h = impaired(
        harness(1e6, 8.0).set(
            &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
            sdrkit_stack_satimg::modes::meteor::ModulationChoice::Oqpsk72,
        ),
        Some(14.0),
        1800.0,
        40.0,
    );
    let r = go(&h, 1e6);
    let strips = r.of::<Strip>();
    assert!(strips_exact(&strips, 40) >= 20 * 14);
    assert!(r.num("frames_ok") > 30.0, "{}", r.num("frames_ok"));
    assert_eq!(r.num("frames_bad"), 0.0);
    assert!(
        (r.num("offset_hz") - 1800.0).abs() < 60.0,
        "{}",
        r.num("offset_hz")
    );
    assert_eq!(r.num("packets_lost"), 0.0);
    assert!(r.num("strips") >= 20.0 * 14.0);
}

#[test]
fn automatic_finds_oqpsk_at_2_4_msps_with_the_signal_offset() {
    // The signal 200 kHz below the centre (the radio tuned above it).
    let h = impaired(harness(2.4e6, 6.0), Some(25.0), -200e3 - 4500.0, 0.0);
    let x = h.generate().unwrap();
    let r = run::<Strip>(&h, &x, 2.4e6, 138.1e6, 137.9e6, 65_536, None);
    assert!(r.num("frames_ok") > 20.0, "{}", r.num("frames_ok"));
    assert_eq!(r.num("frames_bad"), 0.0);
    assert_eq!(r.text("modulation"), "oqpsk72");
}

#[test]
fn qpsk_72k_night_channels() {
    let h = impaired(
        harness(1e6, 8.0)
            .set(
                &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
                sdrkit_stack_satimg::modes::meteor::ModulationChoice::Qpsk72,
            )
            .set(
                &sdrkit_stack_satimg::modes::meteor::GEN_MODULATION,
                sdrkit_stack_satimg::modes::meteor::MeteorModulation::Qpsk72,
            )
            .set(&sdrkit_stack_satimg::modes::meteor::GEN_NIGHT, true),
        Some(14.0),
        -2500.0,
        -60.0,
    );
    let r = go(&h, 1e6);
    assert!(r.num("frames_ok") > 30.0);
    let strips = r.of::<Strip>();
    assert!(strips.iter().filter(|s| s.channel == 1).count() >= 20 * 14);
    assert!(strips.iter().any(|s| s.channel == 5));
    assert!(!strips.iter().any(|s| s.channel == 3));
    assert_eq!(r.text("colour_channels"), "1,2,5");
}

#[test]
fn automatic_moves_to_qpsk() {
    let h = impaired(
        harness(1e6, 18.0).set(
            &sdrkit_stack_satimg::modes::meteor::GEN_MODULATION,
            sdrkit_stack_satimg::modes::meteor::MeteorModulation::Qpsk72,
        ),
        Some(20.0),
        700.0,
        0.0,
    );
    let r = go(&h, 1e6);
    assert!(r.num("frames_ok") > 20.0, "{}", r.num("frames_ok"));
    assert_eq!(r.text("modulation"), "qpsk72");
    assert!(r.num("frames_bad") < r.num("frames_ok"));
}

#[test]
fn oqpsk_at_8_db() {
    let h = impaired(
        harness(250e3, 8.0).set(
            &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
            sdrkit_stack_satimg::modes::meteor::ModulationChoice::Oqpsk72,
        ),
        Some(8.0),
        0.0,
        0.0,
    );
    let r = go(&h, 250e3);
    assert!(r.num("frames_ok") > 10.0, "{}", r.num("frames_ok"));
    assert_eq!(r.num("frames_bad"), 0.0);
}

#[test]
fn doppler_sweep_through_zero() {
    // From +3.5 kHz at -7000/60 Hz/s (ten times the real rate), QPSK, 14 dB, 250 ksps.
    let h = impaired(
        harness(250e3, 30.0)
            .set(
                &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
                sdrkit_stack_satimg::modes::meteor::ModulationChoice::Qpsk72,
            )
            .set(
                &sdrkit_stack_satimg::modes::meteor::GEN_MODULATION,
                sdrkit_stack_satimg::modes::meteor::MeteorModulation::Qpsk72,
            )
            .set(&sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S, -7000.0 / 60.0),
        Some(14.0),
        3500.0,
        0.0,
    );
    let r = go(&h, 250e3);
    // About 8.8 frames a second after the first 3 s.
    assert!(
        r.num("frames_ok") > 0.9 * 27.0 * 8.8 * 0.95,
        "{}",
        r.num("frames_ok")
    );
    assert_eq!(r.num("frames_bad"), 0.0);
}

#[test]
fn interleaved_80k() {
    // The memory of the deinterleaver takes about 18 s to fill. The case runs 26 s.
    let h = impaired(
        harness(400e3, 26.0)
            .set(
                &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
                sdrkit_stack_satimg::modes::meteor::ModulationChoice::Oqpsk80,
            )
            .set(
                &sdrkit_stack_satimg::modes::meteor::GEN_MODULATION,
                sdrkit_stack_satimg::modes::meteor::MeteorModulation::Oqpsk80,
            ),
        Some(22.0),
        1200.0,
        0.0,
    );
    let r = go(&h, 400e3);
    assert!(r.num("frames_ok") > 20.0, "{}", r.num("frames_ok"));
    assert_eq!(r.num("frames_bad"), 0.0);
    assert_eq!(r.num("interleaver_locked"), 1.0);
    // With the interleaver fixed, every strip decodes exactly (no bit of the zero-delay branch comes from a whole ring earlier).
    assert!(strips_exact(&r.of::<Strip>(), 40) >= 10 * 14);
}

#[test]
fn noise_only_returns_nothing() {
    let h = harness(1e6, 4.0)
        .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Floor,
        )
        .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -20.0);
    let r = go(&h, 1e6);
    assert_eq!(r.num("frames_ok"), 0.0);
    assert_eq!(r.num("picture_rows"), 0.0);
}

#[test]
fn any_cut_returns_the_same_result() {
    let h = impaired(
        harness(250e3, 4.0).set(
            &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
            sdrkit_stack_satimg::modes::meteor::ModulationChoice::Oqpsk72,
        ),
        Some(12.0),
        900.0,
        0.0,
    );
    let x = h.generate().unwrap();
    let mut all = Vec::new();
    for piece in [1_000, 7_919, 65_536, x.len()] {
        let r = run::<Strip>(&h, &x, 250e3, 137.9e6, 137.9e6, piece, None);
        // The BER estimate of the code synchroniser is sampled at the end of each call, so it depends on the cuts.
        let v = fields_without(&r.results, &["signal_time", "ber"]);
        all.push((v, table_rows(&r.results)));
    }
    for a in &all[1..] {
        assert_eq!(a, &all[0]);
    }
    let r = run::<Strip>(&h, &x, 250e3, 137.9e6, 137.9e6, 65_536, None);
    assert!(r.num("frames_ok") > 10.0);
}

#[test]
fn settings_take_effect_and_bad_values_are_refused() {
    let h = impaired(harness(250e3, 6.0), Some(15.0), 0.0, 0.0);
    let x = h.generate().unwrap();
    // A fixed QPSK modulation never decodes an OQPSK signal.
    let hq = h.clone().set(
        &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
        sdrkit_stack_satimg::modes::meteor::ModulationChoice::Qpsk72,
    );
    let r = run::<Strip>(&hq, &x, 250e3, 137.9e6, 137.9e6, 50_000, None);
    assert_eq!(r.num("frames_ok"), 0.0);
    assert_eq!(r.text("modulation"), "qpsk72");
    // Switching the setting decodes it, and the preset requests the host to retune.
    let mut rx = r.rx;
    set(&mut *rx, "modulation", "oqpsk72").unwrap();
    set(&mut *rx, "frequency_preset", "137.100").unwrap();
    let r = feed::<Strip>(rx, &x, 50_000, None);
    assert!(r.num("frames_ok") > 10.0);
    assert!(r.requests.iter().any(|q| matches!(
        q,
        sdrkit_core::HostRequest::Retune { tuned_hz, .. } if *tuned_hz == 137.1e6
    )));
    // A bad value is refused and changes nothing.
    let mut rx = r.rx;
    assert!(set(&mut *rx, "modulation", "8psk").is_err());
    assert!(set(&mut *rx, "frequency_preset", "137.5").is_err());
    assert!(set(&mut *rx, "view", "ch9").is_err());
    // A new picture starts again.
    let rows = r.results.decl().field_id("rows").expect("the rows field");
    assert!(matches!(
        r.results.fields().value(rows),
        Some(sdrkit_core::Value::I64(_))
    ));
    invoke(&mut *rx, "clear", &[]).unwrap();
}

#[test]
fn a_retune_keeps_the_picture() {
    let h = impaired(
        harness(250e3, 6.0).set(
            &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
            sdrkit_stack_satimg::modes::meteor::ModulationChoice::Oqpsk72,
        ),
        Some(16.0),
        0.0,
        0.0,
    );
    let x = h.generate().unwrap();
    let r = run::<Strip>(&h, &x, 250e3, 137.9e6, 137.9e6, 50_000, None);
    let rows = r.num("picture_rows");
    assert!(rows > 0.0);
    let mut rx = r.rx;
    rx.retuned(&InputInfo::new(250e3, 137.1e6, 137.1e6).recording());
    let r = feed::<Strip>(rx, &x[..50_000], 50_000, None);
    assert!(r.num("picture_rows") >= rows);
    assert!(r.num("strips") > 0.0);
}

#[test]
fn the_pass_picture_is_kept_in_the_saved_state() {
    let h = impaired(
        harness(250e3, 6.0).set(
            &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
            sdrkit_stack_satimg::modes::meteor::ModulationChoice::Oqpsk72,
        ),
        Some(16.0),
        0.0,
        0.0,
    );
    let x = h.generate().unwrap();
    let r = run::<Strip>(&h, &x, 250e3, 137.9e6, 137.9e6, 50_000, None);
    let strips = r.num("strips");
    assert!(strips > 0.0);
    let state = r.rx.save_state().expect("a saved state");
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(250e3, 137.9e6, 137.9e6).recording())
        .unwrap();
    rx.restore_state(&state).unwrap();
    let r = feed::<Strip>(rx, &vec![Complex32::default(); 125_000], 25_000, None);
    assert_eq!(r.num("strips"), strips);
    // A state of another stack is refused.
    let mut rx = h.receiver().unwrap();
    let other = StateBlob::new("hrpt", state.version(), state.bytes().to_vec());
    assert!(rx.restore_state(&other).is_err());
}

#[test]
fn bad_samples_are_counted_and_fixed() {
    let h = impaired(
        harness(250e3, 4.0).set(
            &sdrkit_stack_satimg::modes::meteor::MODULATION_CHOICE,
            sdrkit_stack_satimg::modes::meteor::ModulationChoice::Oqpsk72,
        ),
        Some(14.0),
        0.0,
        0.0,
    );
    let mut x = h.generate().unwrap();
    for i in (0..x.len()).step_by(10_007) {
        x[i] = Complex32::new(f32::NAN, 0.0);
    }
    let r = run::<Strip>(&h, &x, 250e3, 137.9e6, 137.9e6, 40_000, None);
    assert!(r.num("bad_samples") > 0.0);
    assert!(r.num("frames_ok") > 10.0);
}

#[test]
fn what_it_publishes_is_declared() {
    let h = impaired(harness(1e6, 6.0), Some(16.0), 0.0, 0.0);
    let r = go(&h, 1e6);
    assert_declared(&r, &sdrkit_stack_satimg::typed::shape("meteor_lrpt").results_decl());
}

#[test]
fn a_rate_too_low_is_refused_with_the_reason() {
    let mut rx = Harness::new(&DESCRIPTOR).receiver().unwrap();
    assert!(rx
        .configure(&InputInfo::new(100e3, 137.9e6, 137.9e6))
        .is_err());
    assert!(Owner::results(&*rx).status().summary.contains("250 kHz"));
}
