use sdrkit_algo_modem::timing::EdgeClock;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_proto_link::ax25::Frame;
use sdrkit_proto_link::modem::{afsk1200_audio, g3ruh9600_audio, Afsk1200, G3ruh9600};

fn frame(text: &str) -> Vec<u8> {
    let s = format!("N0CALL-9>APRS,WIDE1-1:{text}");
    let f = Frame::from_tnc2(&s).unwrap();
    let mut b = Vec::new();
    f.encode(&mut b).unwrap();
    b
}

fn afsk(rate: f64, audio: &[f32], piece: usize) -> Vec<Vec<u8>> {
    let mut rx = Afsk1200::new(rate).unwrap();
    let mut got = Vec::new();
    for c in audio.chunks(piece) {
        rx.process(c, |f| got.push(f.bytes.to_vec()));
    }
    got
}

fn g3ruh(rate: f64, audio: &[f32]) -> Vec<Vec<u8>> {
    let mut rx = G3ruh9600::new(rate).unwrap();
    let mut got = Vec::new();
    rx.process(audio, |f| got.push(f.bytes.to_vec()));
    got
}

fn noisy(audio: &mut [f32], snr_db: f64, seed: u64) {
    // The audio has peak 1, so mean power 0.5. The SNR is over the whole band.
    let mut n = Awgn::with_power(0.5 / 10f64.powf(snr_db / 10.0), seed).unwrap();
    n.process_real(audio);
}

#[test]
fn afsk_1200_decodes_at_several_rates_and_any_cut() {
    let a = frame("!4903.50N/07201.75W-rates");
    for rate in [48_000.0, 22_050.0, 12_000.0] {
        let mut audio = vec![0.0; 500];
        afsk1200_audio(&[&a], 20, rate, &mut audio).unwrap();
        for piece in [1, 37, 4096] {
            assert_eq!(
                afsk(rate, &audio, piece),
                std::slice::from_ref(&a),
                "{rate} {piece}"
            );
        }
    }
}

#[test]
fn afsk_1200_back_to_back_frames_noise_twist_and_clock_error() {
    let frames: Vec<Vec<u8>> = (0..3)
        .map(|i| frame(&format!(">status number {i}")))
        .collect();
    let refs: Vec<&[u8]> = frames.iter().map(|f| f.as_slice()).collect();
    let mut audio = vec![0.0; 500];
    afsk1200_audio(&refs, 30, 48_000.0, &mut audio).unwrap();
    let mut rx = Afsk1200::new(48_000.0).unwrap();
    let mut got = Vec::new();
    rx.process(&audio, |f| got.push(f.bytes.to_vec()));
    assert_eq!(got, frames);
    assert_eq!(rx.stats().frames, 3);
    assert!(
        rx.stats().duplicates > 0,
        "several slicers decode each frame"
    );
    // Noise: 10 dB over the whole 24 kHz band (about 20 dB in the 2.4 kHz of the tones).
    let mut n = audio.clone();
    noisy(&mut n, 10.0, 7);
    assert_eq!(afsk(48_000.0, &n, 1000), frames);
    // Twist: a one-pole low-pass at 800 Hz makes mark about 4 dB louder than space.
    let a = (-std::f32::consts::TAU * 800.0 / 48_000.0).exp();
    let mut y = 0.0;
    let tilted: Vec<f32> = audio
        .iter()
        .map(|&x| {
            y = a * y + (1.0 - a) * x;
            y
        })
        .collect();
    assert_eq!(afsk(48_000.0, &tilted, 1000), frames);
    // A transmitter clock 0.3 % fast.
    let mut fast = vec![0.0; 500];
    afsk1200_audio(&refs, 30, 48_000.0 / 1.003, &mut fast).unwrap();
    assert_eq!(afsk(48_000.0, &fast, 1000), frames);
    // Silence and noise give nothing.
    let mut quiet = vec![0.0f32; 48_000];
    noisy(&mut quiet, 0.0, 3);
    assert!(afsk(48_000.0, &quiet, 4096).is_empty());
}

#[test]
fn g3ruh_9600_decodes_either_polarity_and_noise() {
    let frames: Vec<Vec<u8>> = (0..2).map(|i| frame(&format!(">nine six {i}"))).collect();
    let refs: Vec<&[u8]> = frames.iter().map(|f| f.as_slice()).collect();
    let mut audio = vec![0.0; 300];
    g3ruh9600_audio(&refs, 30, 48_000.0, &mut audio).unwrap();
    assert_eq!(g3ruh(48_000.0, &audio), frames);
    let inverted: Vec<f32> = audio.iter().map(|x| -x).collect();
    assert_eq!(g3ruh(48_000.0, &inverted), frames);
    let mut n = audio.clone();
    noisy(&mut n, 15.0, 11);
    assert_eq!(g3ruh(48_000.0, &n), frames);
    let mut audio = vec![0.0; 300];
    g3ruh9600_audio(&refs, 30, 96_000.0, &mut audio).unwrap();
    assert_eq!(g3ruh(96_000.0, &audio), frames);
}

#[test]
fn bad_parameters_are_refused() {
    assert!(Afsk1200::new(8_000.0).is_err());
    assert!(G3ruh9600::new(24_000.0).is_err());
    assert!(EdgeClock::new(1200.0, 2000.0, 0.5, 0.5).is_err());
    assert!(EdgeClock::new(1200.0, 48_000.0, 0.0, 0.5).is_err());
    assert!(afsk1200_audio(&[&[1, 2, 3]], 1, 48_000.0, &mut Vec::new()).is_err());
    assert!(g3ruh9600_audio(&[&[0u8; 401]], 1, 48_000.0, &mut Vec::new()).is_err());
}

#[test]
fn the_demodulators_do_not_allocate() {
    let a = frame("!4903.50N/07201.75W-alloc");
    let mut audio = vec![0.0; 500];
    afsk1200_audio(&[&a], 20, 48_000.0, &mut audio).unwrap();
    let mut rx = Afsk1200::new(48_000.0).unwrap();
    rx.process(&audio, |_| {});
    let mut n = 0;
    let allocs = crate::allocations(|| {
        for _ in 0..10 {
            for c in audio.chunks(audio.len() / 100 + 1) {
                rx.process(c, |_| n += 1);
            }
        }
    });
    assert_eq!(allocs, 0);
    assert!(n >= 10);
    let mut audio = vec![0.0; 500];
    g3ruh9600_audio(&[&a], 20, 48_000.0, &mut audio).unwrap();
    let mut rx = G3ruh9600::new(48_000.0).unwrap();
    rx.process(&audio, |_| {});
    let mut n = 0;
    let allocs = crate::allocations(|| {
        for _ in 0..100 {
            for c in audio.chunks(audio.len() / 10 + 1) {
                rx.process(c, |_| n += 1);
            }
        }
    });
    assert_eq!(allocs, 0);
    assert!(n >= 100);
}

/// AFSK 1200 audio at 48 kHz of a frame sent with the bytes `sent` and the FCS of `good`.
fn afsk_with_fcs_of(sent: &[u8], good: &[u8]) -> Vec<f32> {
    use sdrkit_proto_link::hdlc::{Line, Transmitter};
    use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
    let mut body = sent.to_vec();
    body.extend_from_slice(&CRC16_IBM_SDLC.checksum(good).to_le_bytes());
    let mut tx = Transmitter::new(Line::Nrzi);
    let mut bits = Vec::new();
    tx.encode_without_fcs(&[&body], 30, &mut bits);
    tx.flags(2, &mut bits);
    let mut m = sdrkit_algo_sim::modulate::Afsk::new(48_000.0, 1200.0, 1200.0, 2200.0).unwrap();
    let mut audio = vec![0.0; 500];
    m.push(&bits, &mut audio);
    m.flush(&mut audio);
    audio.extend(std::iter::repeat_n(0.0, 2000));
    audio
}

#[derive(Debug, Clone, PartialEq)]
struct Got {
    bytes: Vec<u8>,
    with_fcs: Vec<u8>,
    ok: bool,
    twist: Option<f32>,
}

fn heard_afsk(audio: &[f32], piece: usize) -> (Vec<Got>, u64) {
    let mut rx = Afsk1200::new(48_000.0).unwrap();
    let mut got = Vec::new();
    let mut take = |h: sdrkit_proto_link::modem::Heard<'_>| {
        got.push(Got {
            bytes: h.bytes.to_vec(),
            with_fcs: h.with_fcs.to_vec(),
            ok: h.fcs_ok,
            twist: h.tones.map(|t| t.twist_db()),
        })
    };
    for c in audio.chunks(piece) {
        rx.process_frames(c, &mut take);
    }
    rx.flush_frames(&mut take);
    let bad = rx.bad_frames();
    (got, bad)
}

#[test]
fn a_failed_transmission_returns_one_bad_frame_that_repair_restores() {
    use sdrkit_proto_link::hdlc::{repair_ax25, Repair};
    let good = frame("!4903.50N/07201.75W-bad fcs");
    let mut wrong = good.clone();
    wrong[20] ^= 0x08;
    let audio = afsk_with_fcs_of(&wrong, &good);
    // Twelve slicers fail the same transmission: one bad frame, marked, with the FCS it carried.
    for piece in [1000, audio.len()] {
        let (got, bad) = heard_afsk(&audio, piece);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(!got[0].ok);
        assert_eq!(got[0].bytes, wrong);
        assert_eq!(bad, 1);
        let mut b = got[0].with_fcs.clone();
        assert_eq!(
            repair_ax25(&mut b, 1),
            Some(Repair {
                bits: 1,
                at: 20 * 8 + 3
            })
        );
        assert_eq!(&b[..good.len()], good);
    }
    // A good transmission returns its frame once and no bad frame, though some slicers fail on it.
    let mut ok = vec![0.0; 500];
    afsk1200_audio(&[&good], 30, 48_000.0, &mut ok).unwrap();
    let mut n = ok.clone();
    noisy(&mut n, 10.0, 7);
    let (got, bad) = heard_afsk(&n, 1000);
    assert_eq!(
        got.iter()
            .map(|g| (g.bytes.clone(), g.ok))
            .collect::<Vec<_>>(),
        [(good.clone(), true)]
    );
    assert_eq!(bad, 0);
    // `process` returns only the good frame.
    assert!(afsk(48_000.0, &audio, 1000).is_empty());
}

#[test]
fn the_tone_levels_give_the_twist_of_a_known_filter() {
    // A linear filter H scales a steady tone of frequency f by |H(f)|, so after it the twist is
    // 20 log10(|H(1200)| / |H(2200)|) dB. At 48 kHz, with w = 2 pi f / 48000:
    // - the first difference x[n] - x[n-1] (a pre-emphasis) has |H| = 2 sin(w / 2): -5.25 dB.
    // - the moving sum of 16 samples has |H| = |sin(8 w) / sin(w / 2)|: +7.39 dB.
    // - no filter: 0 dB.
    let f = frame("!4903.50N/07201.75W-the tones of this frame");
    let mut clean = vec![0.0; 500];
    afsk1200_audio(&[&f], 30, 48_000.0, &mut clean).unwrap();
    clean.extend(std::iter::repeat_n(0.0, 2000));
    let w = |hz: f64| std::f64::consts::TAU * hz / 48_000.0;
    let diff_db = 20.0 * ((w(1200.0) / 2.0).sin() / (w(2200.0) / 2.0).sin()).log10();
    let sum = |hz: f64| ((8.0 * w(hz)).sin() / (w(hz) / 2.0).sin()).abs();
    let sum_db = 20.0 * (sum(1200.0) / sum(2200.0)).log10();
    assert!((diff_db + 5.25).abs() < 0.01 && (sum_db - 7.39).abs() < 0.01);
    let first_difference: Vec<f32> = (0..clean.len())
        .map(|n| clean[n] - if n > 0 { clean[n - 1] } else { 0.0 })
        .collect();
    let moving_sum: Vec<f32> = (0..clean.len())
        .map(|n| clean[n.saturating_sub(15)..=n].iter().sum())
        .collect();
    for (name, audio, want) in [
        ("none", &clean, 0.0),
        ("first difference", &first_difference, diff_db),
        ("moving sum", &moving_sum, sum_db),
    ] {
        let (got, _) = heard_afsk(audio, 4096);
        assert_eq!(got.len(), 1, "{name}");
        let tw = got[0].twist.unwrap() as f64;
        assert!(
            (tw - want).abs() < 0.1,
            "{name}: twist {tw:.2} dB, want {want:.2}"
        );
    }
}

#[test]
fn reporting_bad_frames_does_not_allocate() {
    let good = frame("!4903.50N/07201.75W-alloc bad");
    let mut wrong = good.clone();
    wrong[17] ^= 1;
    let mut audio = afsk_with_fcs_of(&wrong, &good);
    afsk1200_audio(&[&good], 20, 48_000.0, &mut audio).unwrap();
    let mut rx = Afsk1200::new(48_000.0).unwrap();
    let mut g = G3ruh9600::new(48_000.0).unwrap();
    rx.process_frames(&audio, |_| {});
    g.process_frames(&audio, |_| {});
    let (mut good_n, mut bad_n) = (0, 0);
    let allocs = crate::allocations(|| {
        for _ in 0..5 {
            for c in audio.chunks(997) {
                rx.process_frames(c, |h| if h.fcs_ok { good_n += 1 } else { bad_n += 1 });
                g.process_frames(c, |_| {});
            }
        }
    });
    assert_eq!(allocs, 0);
    assert!(good_n >= 5 && bad_n >= 5, "{good_n} {bad_n}");
}

#[test]
fn g3ruh_returns_one_bad_frame_that_repair_restores() {
    use sdrkit_algo_dsp::pulse::Shape;
    use sdrkit_proto_link::hdlc::{repair_ax25, Line, Repair, Transmitter};
    use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
    let good = frame(">nine six bad fcs");
    let mut wrong = good.clone();
    wrong[18] ^= 0x40;
    let mut body = wrong.clone();
    body.extend_from_slice(&CRC16_IBM_SDLC.checksum(&good).to_le_bytes());
    // The baseband as g3ruh9600_audio makes it: scrambled NRZI levels with a raised-cosine pulse (roll-off 0.5).
    let mut tx = Transmitter::new(Line::G3ruh);
    let mut bits = Vec::new();
    tx.encode_without_fcs(&[&body], 30, &mut bits);
    tx.flags(4, &mut bits);
    let mut m = sdrkit_algo_sim::modulate::Linear::new(Shape::Rc { beta: 0.5 }, 8.0, 5.0).unwrap();
    let symbols: Vec<sdrkit_core::Complex32> = bits
        .iter()
        .map(|&b| sdrkit_core::Complex32::new(if b == 1 { 1.0 } else { -1.0 }, 0.0))
        .collect();
    let mut iq = Vec::new();
    m.push(&symbols, &mut iq);
    m.flush(&mut iq);
    let mut audio: Vec<f32> = vec![0.0; 300];
    audio.extend(iq.iter().map(|z| z.re));
    audio.extend(std::iter::repeat_n(0.0, 500));
    let mut rx = G3ruh9600::new(48_000.0).unwrap();
    let mut got = Vec::new();
    let mut take = |h: sdrkit_proto_link::modem::Heard<'_>| {
        got.push((h.bytes.to_vec(), h.with_fcs.to_vec(), h.fcs_ok, h.tones))
    };
    for c in audio.chunks(777) {
        rx.process_frames(c, &mut take);
    }
    rx.flush_frames(&mut take);
    assert_eq!(got.len(), 1, "{got:?}");
    let (bytes, with_fcs, ok, tones) = got.pop().unwrap();
    assert!(!ok && tones.is_none());
    assert_eq!(bytes, wrong);
    assert_eq!(with_fcs, body);
    assert_eq!(rx.bad_frames(), 1);
    let mut b = with_fcs;
    assert_eq!(
        repair_ax25(&mut b, 1),
        Some(Repair {
            bits: 1,
            at: 18 * 8 + 6
        })
    );
    assert_eq!(&b[..good.len()], good);
}
