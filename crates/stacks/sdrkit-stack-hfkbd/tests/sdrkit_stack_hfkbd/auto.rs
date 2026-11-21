//! Automatic settings: the RTTY analysis on every speed, shift and polarity, at low SNR, on noise, and through the stack after
//! the `auto_lock` command; the PSK modulation choice.

use crate::common::*;
use hf::decoder::Modulator;
use hf::rtty::{analyse, Baud, Shift, BAUDS, SHIFTS};
use sdrkit_core::{owner, Complex32, InputInfo};
use sdrkit_stack_hfkbd as hf;

const RATE8: f64 = 8000.0;

/// Complex white noise of `power` added to `x`.
fn add_noise(x: &mut [Complex32], power: f64, seed: u64) {
    sdrkit_algo_sim::noise::Awgn::with_power(power, seed)
        .unwrap()
        .process(x);
}

/// The noise power of a tone of amplitude 0.5 (power 0.25) at `snr_db` against the noise in 3 kHz, at `rate`.
fn noise_power(snr_db: f64, rate: f64) -> f64 {
    0.25 / 10f64.powf(snr_db / 10.0) * rate / 3000.0
}

/// `seconds` of RTTY at 8 kHz: tone amplitude 0.5, the test text, noise for `snr_db`.
fn rtty_audio(
    baud: Baud,
    shift: Shift,
    reverse: bool,
    lower: f64,
    seconds: f64,
    snr_db: f64,
    seed: u64,
) -> Vec<Complex32> {
    let mut m = hf::rtty::RttyModulator::new(
        RATE8,
        hf::rtty::TEST_TEXT,
        lower,
        baud.baud(),
        shift.hz(),
        reverse,
    );
    let mut x = vec![Complex32::default(); (seconds * RATE8) as usize];
    m.fill(&mut x);
    x.iter_mut().for_each(|v| *v *= 0.5);
    add_noise(&mut x, noise_power(snr_db, RATE8), seed);
    x
}

/// Shifts 25 to 30 Hz apart decode alike.
fn neighbours(a: Shift, b: Shift) -> bool {
    let pair = |s: Shift| match s {
        Shift::S170 | Shift::S200 => 0,
        Shift::S425 | Shift::S450 => 1,
        Shift::S850 => 2,
    };
    pair(a) == pair(b)
}

#[test]
fn rtty_auto_finds_every_speed_shift_and_polarity() {
    let (mut right, mut exact, mut tone) = (0, 0, 0);
    let mut wrong = Vec::new();
    for (si, &baud) in BAUDS.iter().enumerate() {
        for (hi, &shift) in SHIFTS.iter().enumerate() {
            for (pol, reverse) in [false, true].into_iter().enumerate() {
                let lower = 400.0 + 45.0 * (((si * 5 + hi) * 2 + pol) as f64);
                let x = rtty_audio(
                    baud,
                    shift,
                    reverse,
                    lower,
                    14.0,
                    15.0,
                    (si * 10 + hi * 2 + pol) as u64 + 1,
                );
                let g = analyse(&x, RATE8);
                let Some(first) = g.first() else {
                    wrong.push(format!("{baud:?}/{shift:?}/{reverse}: none"));
                    continue;
                };
                if first.baud == baud && first.reverse == reverse && neighbours(first.shift, shift)
                {
                    right += 1;
                } else {
                    wrong.push(format!("{baud:?}/{shift:?}/{reverse}: {first:?}"));
                }
                if first.baud == baud && first.reverse == reverse && first.shift == shift {
                    exact += 1;
                }
                if (first.lower_hz - lower).abs() <= 15.0 {
                    tone += 1;
                }
            }
        }
    }
    assert_eq!(right, 40, "{wrong:?}");
    assert!(exact >= 36, "{exact} exact");
    assert!(tone >= 39, "{tone} lower tones within 15 Hz");
}

#[test]
fn rtty_auto_at_10_db() {
    let mut wrong = Vec::new();
    for (baud, shift, reverse) in [
        (Baud::B45, Shift::S170, false),
        (Baud::B75, Shift::S425, true),
    ] {
        for k in 0..4u32 {
            let lower = 1400.0 + 130.0 * f64::from(k);
            let x = rtty_audio(baud, shift, reverse, lower, 16.0, 10.0, 100 + u64::from(k));
            let g = analyse(&x, RATE8);
            let ok = g.first().is_some_and(|f| {
                f.baud == baud && f.reverse == reverse && neighbours(f.shift, shift)
            });
            if !ok {
                wrong.push(format!(
                    "{baud:?}/{shift:?}/{reverse} at {lower}: {:?}",
                    g.first()
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:?}");
}

#[test]
fn rtty_auto_finds_nothing_in_noise_or_a_steady_tone() {
    for seed in 1..=3u64 {
        let mut x = vec![Complex32::default(); (16.0 * RATE8) as usize];
        add_noise(&mut x, noise_power(10.0, RATE8), seed);
        assert!(analyse(&x, RATE8).is_empty(), "noise {seed}");
        let mut ph = 0.0f64;
        let mut y: Vec<Complex32> = (0..x.len())
            .map(|_| {
                ph += std::f64::consts::TAU * 1500.0 / RATE8;
                Complex32::from_polar(0.5, ph as f32)
            })
            .collect();
        add_noise(&mut y, noise_power(10.0, RATE8), seed + 10);
        assert!(analyse(&y, RATE8).is_empty(), "tone {seed}");
    }
}

#[test]
fn rtty_auto_lock_command_fixes_wrong_settings() {
    // The stack at 12 ksps, set by hand to 45.45 Bd and 170 Hz with Auto off; the signal is 75 Bd, 450 Hz, reversed.
    let h = harness(&hf::RTTY).set(&hf::rtty::AUTO, false);
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(RATE, 14.08e6, 14.08e6).recording())
        .unwrap();
    let cmd = rx.commands().id("auto_lock").unwrap();
    let args = rx.commands().def(cmd).unwrap().new_args();
    owner::invoke(rx.as_mut(), cmd, &args).unwrap();
    let mut m = hf::rtty::RttyModulator::new(RATE, hf::rtty::TEST_TEXT, 1900.0, 75.0, 450.0, true);
    let mut x = vec![Complex32::default(); (24.0 * RATE) as usize];
    m.fill(&mut x);
    x.iter_mut().for_each(|v| *v *= 0.5);
    add_noise(&mut x, noise_power(12.0, RATE), 4);
    feed_rx(rx.as_mut(), &x, 800);
    let res = rx.results();
    assert_eq!(field_num(res, "auto_state"), 2.0);
    assert_eq!(field_num(res, "symbol_rate_hz"), 75.0);
    assert!((field_num(res, "shift_hz") - 450.0).abs() <= 30.0);
    let reverse = res
        .decl()
        .field_id("reverse")
        .and_then(|id| res.fields().value(id));
    assert_eq!(reverse, Some(&sdrkit_core::Value::Bool(true)));
    assert!(
        field_num(res, "characters") > 100.0,
        "{}",
        field_text(res, "text")
    );
}

/// `seconds` of one PSK signal at 1300 Hz at the harness rate, at `snr_db` against the noise in 3 kHz.
fn psk_band(v: hf::psk::Variant, seconds: f64, snr_db: f64, seed: u64) -> Vec<Complex32> {
    let spec = hf::psk::PskSignalSpec::new(1300.0, v, "CQ CQ DE K0TST K0TST PSE K\n");
    let mut m = hf::psk::PskModulator::new(RATE, vec![spec]);
    let mut x = vec![Complex32::default(); (seconds * RATE) as usize];
    m.fill(&mut x);
    // The raised-cosine reversals have 3/4 of the peak power.
    add_noise(
        &mut x,
        0.75 / 10f64.powf(snr_db / 10.0) * RATE / 3000.0,
        seed,
    );
    x
}

/// The PSK stack set to BPSK31 with Auto on, configured for a recording at the harness rate.
fn psk_receiver() -> Box<dyn sdrkit_core::Receiver> {
    let mut rx = harness(&hf::PSK).receiver().unwrap();
    rx.configure(&InputInfo::new(RATE, 14.07e6, 14.07e6).recording())
        .unwrap();
    rx
}

fn flag_of(res: &sdrkit_core::Results, key: &str) -> bool {
    let v = res
        .decl()
        .field_id(key)
        .and_then(|id| res.fields().value(id));
    matches!(v, Some(sdrkit_core::Value::Bool(true)))
}

#[test]
fn psk_auto_finds_the_modulation() {
    use hf::psk::Variant;
    for v in [Variant::Bpsk63, Variant::Qpsk31, Variant::Bpsk125] {
        let mut rx = psk_receiver();
        feed_rx(rx.as_mut(), &psk_band(v, 14.0, 10.0, 7), 800);
        let res = rx.results();
        assert_eq!(field_text(res, "auto_modulation"), v.name(), "{v:?}");
        assert_eq!(field_text(res, "modulation"), v.name());
        assert!(flag_of(res, "auto_locked"));
    }
}

#[test]
fn psk_auto_finds_nothing_in_noise() {
    let mut rx = psk_receiver();
    let x = noise_only(30.0, 21);
    feed_rx(rx.as_mut(), &x, 800);
    let res = rx.results();
    assert_eq!(field_text(res, "auto_modulation"), "");
    assert!(!flag_of(res, "auto_locked"));
    assert_eq!(field_text(res, "modulation"), "BPSK31");
}

#[test]
fn psk_auto_lock_survives_a_burst_of_noise() {
    use hf::psk::Variant;
    let mut rx = psk_receiver();
    let mut x = psk_band(Variant::Bpsk31, 30.0, 10.0, 9);
    feed_rx(rx.as_mut(), &x[..(12.0 * RATE) as usize], 800);
    assert!(flag_of(rx.results(), "auto_locked"));
    // 2 s of noise 20 dB above the signal.
    let burst = &mut x[(12.0 * RATE) as usize..(14.0 * RATE) as usize];
    add_noise(burst, 0.75 * 100.0, 10);
    let mut seen_unlocked = false;
    for c in x[(12.0 * RATE) as usize..].chunks((RATE * 0.5) as usize) {
        feed_rx(rx.as_mut(), c, 800);
        let res = rx.results();
        seen_unlocked |= !flag_of(res, "auto_locked") || field_text(res, "modulation") != "BPSK31";
    }
    assert!(!seen_unlocked);
}

#[test]
fn a_hand_setting_turns_auto_off() {
    assert!(flag(&run(&harness(&hf::RTTY).seconds(1.0)), "auto"));
    assert!(flag(&run(&harness(&hf::PSK).seconds(1.0)), "auto"));
    let r = run(&harness(&hf::RTTY)
        .set(&hf::rtty::BAUD, Baud::B75)
        .seconds(1.0));
    assert!(!flag(&r, "auto"));
    let r = run(&harness(&hf::PSK)
        .set(&hf::psk::MODULATION_SETTING, hf::psk::Variant::Bpsk63)
        .seconds(1.0));
    assert!(!flag(&r, "auto"));
}
