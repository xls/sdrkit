//! PSK31 and its variants: the demo band, every variant, noise alone, close signals with a tuning error, and the recordings.

use crate::common::*;
use sdrkit_core::verify::CaseSetting;
use sdrkit_dev_test::Run;
use sdrkit_stack_hfkbd as hf;

// Columns of the signals table.
const OFFSET: &str = "offset_hz";
const CHARS: &str = "characters";
const TEXT: &str = "text";

/// Whether a signal within `tol` Hz of `f` has `want` in its text.
fn heard(r: &Run, f: f64, tol: f64, want: &str) -> bool {
    rows(r, "signals")
        .iter()
        .any(|s| (cell_num(s, OFFSET) - f).abs() < tol && cell_text(s, TEXT).contains(want))
}

#[test]
fn psk_demo_band() {
    // 24 dB in 2.5 kHz against the power of all four signals is about 21.6 dB against the strongest one alone.
    let r = run(&snr_2500(harness(&hf::PSK), 24.0).seconds(25.0));
    for (f, want) in [(700.0, "LY2ABC"), (1050.0, "K0TST"), (2200.0, "N0XYZ")] {
        assert!(
            heard(&r, f, 20.0, want),
            "{f} Hz: {:?}",
            rows(&r, "signals")
        );
    }
    assert_ne!(
        r.results.status().lock_state,
        sdrkit_core::LockState::Searching
    );
}

#[test]
fn psk_variants() {
    for m in [
        "BPSK63", "QPSK31", "QPSK63", "BPSK125", "BPSK250", "QPSK125",
    ] {
        let h = snr_2500(harness(&hf::PSK), 25.0)
            .set(
                &hf::psk::MODULATION_SETTING,
                hf::psk::Variant::from_key(m).unwrap(),
            )
            .set(&hf::decoder::GEN_MESSAGE, "CQ DE K0TST K0TST PSE K\n")
            .set(&hf::psk::GEN_FREQ_HZ, 1300.0)
            .seconds(15.0);
        let r = run(&h);
        assert!(
            heard(&r, 1300.0, 30.0, "DE K0TST K0TST PSE K"),
            "{m}: {:?}",
            rows(&r, "signals")
        );
        assert_eq!(text_field(&r, "modulation"), m);
    }
}

#[test]
fn psk_noise_only() {
    let r = feed(&harness(&hf::PSK), &noise_only(20.0, 11));
    let chars: f64 = rows(&r, "signals").iter().map(|s| cell_num(s, CHARS)).sum();
    assert!(chars < 3.0, "{chars}");
}

#[test]
fn psk_tuning_error_and_close_signals() {
    // A signal 37 Hz above where it was sent is found there with its text, beside another 70 Hz away.
    use hf::psk::{PskModulator, PskSignalSpec, Variant};
    let a = PskSignalSpec::new(1237.0, Variant::Bpsk31, "CQ DE K0TST K0TST PSE K\n");
    let mut b = PskSignalSpec::new(1307.0, Variant::Bpsk31, "TEST DE N0XYZ N0XYZ K\n");
    b.start_s = 0.4;
    let mut x = vec![sdrkit_core::Complex32::default(); 12000 * 20];
    sdrkit_core::Generator::generate(
        &mut hf::decoder::HfGenerator::new(
            RATE,
            Box::new(PskModulator::new(RATE, vec![a, b])),
            sdrkit_core::SignalInfo::new(100.0),
        ),
        &mut x,
    );
    let mut n = noise_only(20.0, 4);
    for (s, v) in x.iter_mut().zip(&mut n) {
        *s += *v * 0.1;
    }
    let r = feed(&harness(&hf::PSK).freq(0.0), &x);
    for (f, want) in [(1237.0, "DE K0TST K0TST"), (1307.0, "DE N0XYZ N0XYZ")] {
        assert!(heard(&r, f, 20.0, want), "{f}: {:?}", rows(&r, "signals"));
    }
}

fn psk_check(name: &str, settings: &[CaseSetting], only: Option<usize>) {
    let e = expected(name);
    for piece in [800, 4093] {
        let h = harness(&hf::PSK).piece(piece).with_settings(settings);
        let r = recording(&h, name);
        let sigs = rows(&r, "signals");
        for (i, want) in e["expect"]["signals"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            if only.is_some_and(|o| o != i) {
                continue;
            }
            let ok = sigs.iter().any(|s| {
                in_range(cell_num(s, OFFSET), &want["offset_hz"])
                    && cell_text(s, TEXT).contains(want["contains"].as_str().unwrap())
            });
            assert!(ok, "{name}: {want} in {sigs:?}");
            if let Some(calls) = want["callsigns"].as_array() {
                for c in calls {
                    let c = c.as_str().unwrap();
                    assert!(
                        rows(&r, "stations")
                            .iter()
                            .any(|s| cell_text(s, "callsign") == c),
                        "{c}"
                    );
                }
            }
        }
    }
}

#[test]
fn psk31_recording() {
    psk_check(
        "hfkbd_psk31_12k",
        &[
            CaseSetting::new(&hf::psk::MODULATION_SETTING, hf::psk::Variant::Bpsk31),
            CaseSetting::new(&hf::psk::THRESHOLD, 5.0),
        ],
        Some(0),
    );
}

/// The weak signal of the recording is stated as 6 dB in 31.25 Hz against its carrier. Text has about 0.75 of the carrier
/// power, so Eb/N0 is 4.75 dB (the recording reads 4.4 to 5.3 dB over its text). Coherent detection with differential decoding
/// then has a symbol error rate of Q(sqrt(2 Eb/N0)) = 0.73 %; the line of 21 characters is 178 bits, whole with a chance of
/// 27 %, and the 25 s hold two whole lines: 47 %. On a steady signal the receiver is within 0.3 dB of that detection (the test
/// of the channel in `psk.rs`). On the generator's signal with the pauses of the recording it holds a line in 3 of 48 runs at
/// this level and in 24 of 48 at Eb/N0 = 6 dB, where theory has 47 % and 88 %. Theory does not explain the rest: lines are
/// lost at their start, while the signal is found (5 dB threshold) and the clock, the frequency and the carrier loop settle
/// within the 32 reversals before the text.
#[test]
#[ignore = "one whole line of the weak signal at Eb/N0 = 4.75 dB: the receiver holds a line in 3 of 48 generated runs (24 of 48 at 6 dB), where coherent detection has 47 % (88 %) in theory. Open defect: lines are lost at their start, in the search for the signal and the settling of clock, frequency and carrier loop"]
fn psk31_recording_weak_signal() {
    psk_check(
        "hfkbd_psk31_12k",
        &[
            CaseSetting::new(&hf::psk::MODULATION_SETTING, hf::psk::Variant::Bpsk31),
            CaseSetting::new(&hf::psk::THRESHOLD, 5.0),
        ],
        Some(1),
    );
}

/// The synthetic QPSK31 recording turns the carrier the other way for each code symbol than the designer's published phase
/// table (it was made with the rule (2 - symbol) mod 4 quarter turns): it is the signal as heard on the lower sideband.
/// Mirrored about its 1200 Hz carrier, which turns every step back, the receiver reads its text.
#[test]
fn qpsk31_recording_mirrored() {
    let name = "hfkbd_qpsk31_12k";
    let Some(rec) = sdrkit_dev_test::read_recording(&format!("hfkbd/{name}")) else {
        return;
    };
    let x: Vec<sdrkit_core::Complex32> = rec
        .samples
        .iter()
        .enumerate()
        .map(|(n, z)| {
            let p = std::f64::consts::TAU * 2400.0 * n as f64 / rec.rate_hz;
            z.conj() * sdrkit_core::Complex32::new(p.cos() as f32, p.sin() as f32)
        })
        .collect();
    let r = feed(
        &harness(&hf::PSK)
            .freq(0.0)
            .set(&hf::psk::MODULATION_SETTING, hf::psk::Variant::Qpsk31),
        &x,
    );
    let want = &expected(name)["expect"]["signals"][0];
    let sigs = rows(&r, "signals");
    assert!(
        sigs.iter()
            .any(|s| in_range(cell_num(s, OFFSET), &want["offset_hz"])
                && cell_text(s, TEXT).contains(want["contains"].as_str().unwrap())),
        "{want} in {sigs:?}"
    );
}

/// The lines that a receiver reads whole from `bursts` bursts of one signal of modulation `v`: at default settings (Auto on)
/// for BPSK31, else with the modulation set. A
/// burst is 1 s of idle, [`BURST_LINE`], 1 s of idle and 2 s without a carrier; the first starts after 3 s of noise. The
/// noise is at `snr_db` in 2.5 kHz and the samples go in pieces of `piece`.
fn burst_lines(v: hf::psk::Variant, snr_db: f64, seed: u64, piece: usize, bursts: usize) -> usize {
    use hf::psk::{PskModulator, PskSignalSpec};
    let mut s = PskSignalSpec::new(1000.0, v, BURST_LINE);
    s.start_s = 3.0;
    let mut bits = 0;
    sdrkit_util_text::varicode::psk::encode(BURST_LINE.as_bytes(), &mut |_| bits += 1).unwrap();
    let period = 2.0 + 2.0 + bits as f64 / v.baud();
    // The last burst ends 2 s before the end: its pause is not needed.
    let seconds = 3.0 + period * bursts as f64 - 1.0;
    let mut x = vec![sdrkit_core::Complex32::default(); (RATE * seconds) as usize];
    sdrkit_core::Generator::generate(
        &mut hf::decoder::HfGenerator::new(
            RATE,
            Box::new(PskModulator::new(RATE, vec![s])),
            sdrkit_core::SignalInfo::new(62.5),
        ),
        &mut x,
    );
    // The signal has a mean power of 0.75: the noise of `noise_only` is that of 0 dB against a power of 1.
    let gain = (0.75f32 / 10f32.powf(snr_db as f32 / 10.0)).sqrt();
    for (s, v) in x.iter_mut().zip(noise_only(seconds, seed)) {
        *s += v * gain;
    }
    let r = feed(
        &harness(&hf::PSK)
            .freq(0.0)
            .piece(piece)
            .set(&hf::psk::MODULATION_SETTING, v),
        &x,
    );
    let text: String = rows(&r, "signals")
        .iter()
        .filter(|s| (cell_num(s, OFFSET) - 1000.0).abs() < 20.0)
        .map(|s| cell_text(s, TEXT))
        .collect();
    text.matches(BURST_LINE.trim_end()).count()
}

const BURST_LINE: &str = "CQ CQ DE K0TST K0TST PSE K\n";

/// Each burst is read from its first character: the first one, which starts in noise before the receiver has found the
/// signal, and those that follow a pause. Auto keeps BPSK31 over the first seconds of the first burst.
#[test]
fn psk_reads_a_burst_from_its_first_character() {
    use hf::psk::Variant::{Bpsk31, Bpsk63};
    for (v, snr, seed, piece) in [
        (Bpsk31, 20.0, 4, 800),
        (Bpsk31, 20.0, 5, 4093),
        (Bpsk31, 5.0, 1, 4093),
        (Bpsk31, -3.0, 4, 800),
        (Bpsk63, 15.0, 1, 800),
    ] {
        assert_eq!(
            burst_lines(v, snr, seed, piece, 4),
            4,
            "{} at {snr} dB, seed {seed}",
            v.name()
        );
    }
}
