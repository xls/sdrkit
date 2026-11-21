//! `cp16`: recognition against a font, the receiver on clean and impaired signals on both sidebands, noise, and the transmitter
//! through the receiver.

use sdrkit_core::interface::Column;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackConfig, Value, Values};
use sdrkit_proto_glyph::cell::{Cell, Cells};
use sdrkit_stack_hfkbd::cp16::{transmission, Recognizer, TxParams};
use sdrkit_stack_hfkbd::typed::Cp16Fields;
use sdrkit_stack_hfkbd::CP16;

const TEXT: &str = "CQ CP16 DE BA0TST 73";

/// A stand-in for a font: an HZK16 file of a pseudo-random glyph for every GB 2312 character.
fn random_hzk16() -> Vec<u8> {
    let mut f = vec![0u8; 87 * 94 * 32];
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    for b in f.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = (x >> 24) as u8 & (x >> 40) as u8;
    }
    f
}

#[test]
fn every_37th_glyph_is_recognised_as_itself() {
    let cells = Cells::with_font(random_hzk16()).unwrap();
    let rec = Recognizer::new(&cells);
    assert!(rec.len() > 7000, "{}", rec.len());
    let (mut n, mut ok) = (0, 0);
    for (i, (_, _, c)) in sdrkit_proto_glyph::gb2312::chars().enumerate() {
        if i % 37 != 0 || c == '\u{3000}' {
            continue;
        }
        let g: Cell = cells.cell(c);
        let mut d = [[0f32; 16]; 16];
        for (r, row) in d.iter_mut().enumerate() {
            for (k, v) in row.iter_mut().enumerate() {
                *v = f32::from(u8::from(g.dot(r, k)));
            }
        }
        n += 1;
        if rec.recognise(&d).map(|m| m.c) == Some(c) {
            ok += 1;
        }
    }
    assert_eq!(ok, n, "{ok} of {n}");
}

/// The receiver's results after `x` (complex at `rate`, the dial at 0 Hz), with these settings.
fn receive(x: &[Complex32], rate: f64, extra: &[(&str, Value)]) -> Box<dyn Receiver> {
    let schema = (CP16.settings)();
    let mut s = Values::defaults(&schema);
    for (k, v) in extra {
        s.set(schema.id(k).unwrap(), v).unwrap();
    }
    let mut rx = (CP16.make_receiver)(&StackConfig::new(s)).unwrap();
    rx.configure(&InputInfo::new(rate, 7e6, 7e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(4800) {
        rx.feed(c, &mut out);
        out.clear();
    }
    rx.finish(&mut out);
    rx
}

/// `TEXT` sent twice with `p` at 24 kHz, moved by `offset` Hz, the clock `ppm` fast, in noise of `snr` dB in 2500 Hz.
fn signal(p: &TxParams, offset: f64, ppm: f64, snr: f64, seed: u64) -> Vec<Complex32> {
    let rate = 24_000.0;
    let one = transmission(TEXT, p, &Cells::new(), rate * (1.0 + ppm * 1e-6), seed).unwrap();
    let mut x = one.clone();
    x.extend(std::iter::repeat_n(
        Complex32::default(),
        (2.0 * rate) as usize,
    ));
    x.extend_from_slice(&one);
    x.extend(std::iter::repeat_n(
        Complex32::default(),
        (3.0 * rate) as usize,
    ));
    for (i, s) in x.iter_mut().enumerate() {
        let ph = std::f64::consts::TAU * offset * i as f64 / rate;
        *s *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
    }
    // The SNR is against the mean power of the samples that are not zero (the key-down power).
    let on: Vec<f64> = one
        .iter()
        .map(|c| f64::from(c.norm_sqr()))
        .filter(|&p| p > 1e-12)
        .collect();
    let power = on.iter().sum::<f64>() / on.len() as f64;
    let level = sdrkit_algo_sim::noise::NoiseLevel::SnrDb {
        snr_db: snr,
        bandwidth_hz: 2500.0,
    };
    sdrkit_algo_sim::noise::Awgn::complex(level, power, rate, seed)
        .unwrap()
        .process(&mut x);
    x
}

fn text(rx: &dyn Receiver) -> String {
    rx.results()
        .fields()
        .get(&sdrkit_core::publish::keys::TEXT)
        .unwrap_or("")
        .to_uppercase()
}

fn num(rx: &dyn Receiver, c: &Column<sdrkit_core::publish::field::F64>) -> f64 {
    rx.results().fields().at(c).unwrap_or(f64::NAN)
}

/// Checks a reception: the text read twice, the base, the spacing and the character time.
fn check(p: &TxParams, offset: f64, ppm: f64, snr: f64) {
    let x = signal(p, offset, ppm, snr, 3);
    let rx = receive(&x, 24_000.0, &[]);
    let t = text(rx.as_ref());
    assert!(t.matches(TEXT).count() >= 1, "{p:?}: {t:?}");
    let base = num(rx.as_ref(), &Cp16Fields::BASE_HZ);
    let want_base = p.base_hz + if p.lower { -offset } else { offset };
    assert!((base - want_base).abs() <= 2.0, "{p:?}: base {base}");
    let d = num(rx.as_ref(), &Cp16Fields::SPACING_HZ);
    assert!(
        (d - p.spacing_hz * (1.0 + ppm * 1e-6)).abs() <= 0.15,
        "{p:?}: spacing {d}"
    );
    let c = num(rx.as_ref(), &Cp16Fields::CHAR_S);
    assert!(
        (c - p.char_s).abs() <= 0.03 * p.char_s,
        "{p:?}: character time {c}"
    );
}

#[test]
fn a_clean_signal_on_the_upper_sideband() {
    let p = TxParams {
        char_s: 2.0,
        spacing_hz: 15.0,
        base_hz: 1000.0,
        lower: false,
    };
    check(&p, 0.0, 0.0, 30.0);
}

#[test]
fn impaired_signals_on_both_sidebands() {
    let p = TxParams {
        char_s: 3.0,
        spacing_hz: 11.0,
        base_hz: 1000.0,
        lower: false,
    };
    check(&p, -73.0, -120.0, 30.0);
    let p = TxParams {
        char_s: 2.0,
        spacing_hz: 15.0,
        base_hz: 1000.0,
        lower: false,
    };
    check(&p, 0.0, 0.0, -3.0);
}

/// Rows shorter than about 1.3 over the spacing blur their edges: the row clock then fails. These cases are printed.
#[test]
fn short_rows_are_printed() {
    let p = TxParams {
        char_s: 1.0,
        spacing_hz: 20.0,
        base_hz: 1000.0,
        lower: true,
    };
    let x = signal(&p, 40.0, 80.0, 30.0, 4);
    let rx = receive(&x, 24_000.0, &[]);
    eprintln!(
        "CP16 1 s, 20 Hz, lower: {:?} {:?}",
        text(rx.as_ref()),
        rx.results().status().summary
    );
    let p = TxParams {
        char_s: 1.5,
        spacing_hz: 12.0,
        base_hz: 1200.0,
        lower: false,
    };
    let x = transmission(TEXT, &p, &Cells::new(), 24_000.0, 9).unwrap();
    let peak = x.iter().map(|c| c.norm()).fold(0.0f32, f32::max);
    assert!(peak < 1.0, "{peak}");
    let mut y = x.clone();
    y.extend(std::iter::repeat_n(Complex32::default(), 24_000 * 2));
    y.extend_from_slice(&x);
    let rx = receive(&y, 24_000.0, &[]);
    eprintln!(
        "CP16 transmitter 1.5 s, 12 Hz: {:?} {:?}",
        text(rx.as_ref()),
        rx.results().status().summary
    );
}

#[test]
fn noise_locks_nothing() {
    let mut x = vec![Complex32::default(); 20 * 24_000];
    sdrkit_algo_sim::noise::Awgn::with_power(1.0, 5)
        .unwrap()
        .process(&mut x);
    let rx = receive(&x, 24_000.0, &[]);
    let f = rx.results().fields();
    assert_ne!(f.at(&Cp16Fields::STATE), Some(2));
    assert_eq!(f.at(&Cp16Fields::SURE).unwrap_or(0), 0);
}

#[test]
fn the_transmitter_through_the_receiver() {
    let p = TxParams {
        char_s: 2.0,
        spacing_hz: 15.0,
        base_hz: 1200.0,
        lower: true,
    };
    let x = transmission(TEXT, &p, &Cells::new(), 24_000.0, 9).unwrap();
    let peak = x.iter().map(|c| c.norm()).fold(0.0f32, f32::max);
    assert!(peak < 1.0, "{peak}");
    let mut y = x.clone();
    y.extend(std::iter::repeat_n(Complex32::default(), 24_000 * 2));
    y.extend_from_slice(&x);
    y.extend(std::iter::repeat_n(Complex32::default(), 24_000 * 3));
    let rx = receive(&y, 24_000.0, &[]);
    assert!(text(rx.as_ref()).contains(TEXT), "{:?}", text(rx.as_ref()));
}

#[test]
fn the_stack_transmits() {
    let rx = (CP16.make_receiver)(&StackConfig::new(Values::defaults(&(CP16.settings)()))).unwrap();
    assert!(rx.transmits());
}
