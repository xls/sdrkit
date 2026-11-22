//! DCR (ARIB STD-T98 volume 3): the test frames of chapter 7 of the standard with the shared NXDN whitening and channel
//! coders and through the receiver at three rates, the plan data file, calls built from their fields through 14
//! impairments, the rows of messages and data, a noise sweep, signals that are not DCR, the scan of plan channels in one
//! capture, NXDN against DCR, and the benchmark.

use crate::common::values;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Complex32, InputInfo, Output, Owner, Receiver, Results, StackConfig};
use sdrkit_dev_test::Awgn;
use sdrkit_stack_dvoice::dcr::gen::{self, CallSpec, GenOptions, Transmission, TEST_FRAMES};
use sdrkit_stack_dvoice::dcr::{nearest, parse_plan, DcrRowKind};
use sdrkit_stack_dvoice::items::EncState;
use sdrkit_stack_dvoice::nxdn::{scrambler, ChannelCoder, FACCH1, SACCH};
use sdrkit_stack_dvoice::typed::{CallRow, DcrEventRow, DcrFields, CALLS_300, DCR_EVENTS};
use std::f64::consts::TAU;

/// The dibits of whitened symbol hex, two bits per symbol, first bit first.
fn dibits(hex: &str) -> Vec<(u8, u8)> {
    hex.chars()
        .map(|c| c.to_digit(16).unwrap() as u8)
        .flat_map(|n| [(n >> 3 & 1, n >> 2 & 1), (n >> 1 & 1, n & 1)])
        .collect()
}

/// The dewhitened bits of `hex` that starts at symbol `at` after the sync, first bit of each symbol first.
fn bits(hex: &str, at: usize) -> Vec<u8> {
    let pn = scrambler();
    dibits(hex)
        .iter()
        .enumerate()
        .flat_map(|(i, &(s, m))| [s ^ pn[at + i], m])
        .collect()
}

/// The RICH: the sign bit of each of its 8 dewhitened symbols, first sent in the most significant place.
fn rich(hex: &str) -> u8 {
    bits(hex, 0).chunks(2).fold(0, |a, b| a << 1 | b[0])
}

fn decode(coder: &mut ChannelCoder, b: &[u8]) -> (Vec<u8>, bool) {
    let soft: Vec<f32> = b.iter().map(|&x| if x == 1 { 1.0 } else { -1.0 }).collect();
    let (info, ok, _) = coder.decode(&soft);
    (info, ok)
}

fn number(b: &[u8]) -> u32 {
    b.iter().fold(0, |a, &x| a << 1 | u32::from(x))
}

#[test]
fn the_rich_of_the_test_frames() {
    assert_eq!(rich("5F7D"), 0x11);
    assert_eq!(rich("DDDD"), 0x8D);
    assert_eq!(rich("DF5D"), 0x95);
    assert_eq!(scrambler()[..8], [0, 0, 1, 0, 0, 1, 1, 1]);
}

#[test]
fn the_sacch_units_and_the_pich_of_the_test_frames() {
    let mut sacch = ChannelCoder::new(SACCH);
    // (voice SACCH, end SACCH, user code, call type)
    for (voice, end, uc, ct) in [
        ("C60DB46E960168D", "8E4DB4AD8F1D6FC", 1, 0),
        ("F70DF4DB9A4B280", "BF4DF41883572F1", 511, 0),
        ("FF0F705A9A1BBAB", "B74F70998307BDA", 511, 1),
    ] {
        for (hex, ty) in [(voice, 1), (end, 30)] {
            let (u, ok) = decode(&mut sacch, &bits(hex, 8));
            assert!(ok, "{hex}");
            assert_eq!((u[0], number(&u[1..3]), number(&u[3..8])), (1, 0, ty), "{hex}");
            assert_eq!((number(&u[8..10]), number(&u[10..19])), (ct, uc), "{hex}");
        }
    }
    let mut pich = ChannelCoder::new(FACCH1);
    let (p, ok) = decode(&mut pich, &bits("82022028832C2AA08399EAAEF82838802E2B", 38));
    assert!(ok);
    let digits: String = p[..36].chunks(4).map(|d| char::from_digit(number(d), 10).unwrap()).collect();
    assert_eq!(digits, "100000001");
}

/// The power of the default carrier amplitude 0.05.
const POWER: f64 = 0.05 * 0.05;

/// A row of the DCR list.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    time_s: f64,
    kind: DcrRowKind,
    identity: String,
    text: String,
    channel: Option<String>,
}

/// The rows of `r`, oldest first.
fn rows(r: &Results) -> Vec<Row> {
    let mut v: Vec<Row> = DCR_EVENTS
        .rows::<DcrEventRow>(r)
        .map(|(_, w)| Row {
            time_s: w.time_s,
            kind: w.kind,
            identity: w.identity.to_string(),
            text: w.text.to_string(),
            channel: w.channel.map(str::to_string),
        })
        .collect();
    v.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
    v
}

fn of_kind(rows: &[Row], kind: DcrRowKind) -> Vec<&Row> {
    rows.iter().filter(|r| r.kind == kind).collect()
}

/// The counters of `r`.
fn counts(r: &Results) -> (i64, i64, i64, i64) {
    let f = DcrFields::read(r.fields()).expect("the result set of DCR");
    (f.sacch_ok, f.dcr_frames, f.voice_frames, f.scan_channels)
}

/// A DCR receiver with `plan` fed `x` at `rate`, centred on `centre_hz` and tuned to `tuned_hz`, then 2 s of nothing.
fn receive(x: &[Complex32], rate: f64, centre_hz: f64, tuned_hz: f64, plan: &str) -> Box<dyn Receiver> {
    let v = values("dcr", &[("plan", plan)]);
    let mut rx = (sdrkit_stack_dvoice::DCR.make_receiver)(&StackConfig::new(v)).expect("the receiver builds");
    rx.configure(&InputInfo::new(rate, centre_hz, tuned_hz).recording())
        .expect("the rate is taken");
    let piece = (rate / 100.0) as usize;
    let mut out = Output::default();
    let zeros = vec![Complex32::new(0.0, 0.0); (2.0 * rate) as usize];
    for c in x.chunks(piece).chain(zeros.chunks(piece)) {
        rx.feed(c, &mut out);
        out.clear();
    }
    rx
}

/// A receiver without a plan fed `x` on the tuned frequency.
fn receive_one(x: &[Complex32], rate: f64) -> Box<dyn Receiver> {
    receive(x, rate, 351.2875e6, 351.2875e6, "")
}

/// `x` with white noise of `snr_db` in 6.25 kHz against the carrier power `power`.
fn noisy(mut x: Vec<Complex32>, rate: f64, power: f64, snr_db: f64, seed: u64) -> Vec<Complex32> {
    Awgn::new(power, snr_db, 6_250.0, rate, seed).process(&mut x);
    x
}

/// The user codes of the calls in the Call rows, in order.
fn call_codes(rows: &[Row]) -> Vec<u16> {
    of_kind(rows, DcrRowKind::Call)
        .iter()
        .map(|r| {
            let uc = r.identity.rsplit("UC ").next().unwrap_or("");
            uc.parse().expect("a user code")
        })
        .collect()
}

#[test]
fn the_test_frames_through_the_receiver_at_three_rates() {
    for rate in [48e3, 250e3, 1e6] {
        for (i, t) in TEST_FRAMES.iter().enumerate() {
            let x = gen::render(&GenOptions {
                rate_hz: rate,
                content: vec![Transmission::TestFrames(*t)],
                seed: i as u64 + 1,
                ..GenOptions::default()
            });
            let rx = receive_one(&x, rate);
            let r = Owner::results(rx.as_ref());
            let rows = rows(r);
            let at = format!("{} at {rate}", t.name);
            let calls = of_kind(&rows, DcrRowKind::Call);
            assert_eq!(calls.len(), 1, "{at}: {rows:#?}");
            assert_eq!(calls[0].identity, format!("UC {}", t.user_code), "{at}");
            let ct = if t.privacy { "privacy scrambled" } else { "normal" };
            assert!(calls[0].text.contains(ct), "{at}: {}", calls[0].text);
            assert!(calls[0].text.ends_with("call sign 100000001"), "{at}: {}", calls[0].text);
            let signs = of_kind(&rows, DcrRowKind::CallSign);
            assert_eq!(signs.len(), 1, "{at}");
            assert_eq!(signs[0].text, "call sign 100000001", "{at}");
            let ends = of_kind(&rows, DcrRowKind::CallEnd);
            assert_eq!(ends.len(), 1, "{at}");
            assert!(ends[0].text.ends_with("end-of-communication message"), "{at}: {}", ends[0].text);
            let (_, _, voice, _) = counts(r);
            assert!((88..=100).contains(&voice), "{at}: {voice} voice frames");
            let call: Vec<CallRow> = CALLS_300.rows::<CallRow>(r).map(|(_, c)| c).collect();
            assert_eq!(call.len(), 1, "{at}");
            assert_eq!(call[0].dst, format!("UC {}", t.user_code));
            assert_eq!(call[0].src, "100000001");
            assert!(call[0].end_seen, "{at}");
            let enc = if t.privacy { EncState::Encrypted } else { EncState::Clear };
            assert_eq!(call[0].encryption, enc, "{at}");
        }
    }
}

/// The rows of the plan data file: (label, frequency in Hz, band, calling flag).
fn plan_rows() -> Vec<(String, f64, String, bool)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans/dcr.csv");
    let text = std::fs::read_to_string(&path).expect("the plan data file");
    let plan = sdrkit_data_plans::io::from_csv(&text, "", "").expect("the plan import takes the file");
    assert_eq!(plan.id, "dcr");
    let mut header: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let cells: Vec<&str> = line.split(',').collect();
        if header.is_empty() {
            header = cells;
            continue;
        }
        let at = |k: &str| cells[header.iter().position(|h| *h == k).expect("a column")];
        out.push((at("name").to_string(), at("centre_hz").parse().expect("a frequency"), at("band").to_string(), at("calling") == "1"));
    }
    assert_eq!(plan.channels.len(), out.len());
    out
}

/// The plan data file as the text of the setting `plan`.
fn plan_setting() -> String {
    plan_rows().iter().map(|(l, hz, _, _)| format!("{l}={hz}")).collect::<Vec<_>>().join(";")
}

#[test]
fn the_plan_data_file() {
    let rows = plan_rows();
    assert_eq!(rows.len(), 220);
    let b351: Vec<f64> = rows.iter().filter(|r| r.2.starts_with("351")).map(|r| r.1).collect();
    assert_eq!(b351.len(), 97);
    for f in &b351 {
        let steps = (f - 351.031_25e6) / 6_250.0;
        assert!((steps - steps.round()).abs() < 1e-6 && steps >= 0.0, "{f}");
        assert!(*f <= 351.631_25e6, "{f}");
    }
    let mut all: Vec<i64> = rows.iter().map(|r| r.1 as i64).collect();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 220, "a frequency repeats");
    let hz = |label: &str| rows.iter().find(|r| r.0 == label).map(|r| r.1).expect(label);
    for (label, mhz) in [
        ("351 MHz ch 1", 351.2),
        ("351 MHz ch 70", 351.631_25),
        ("351 MHz ch 71", 351.031_25),
        ("351 MHz ch 82", 351.1),
        ("351 MHz S1", 351.168_75),
        ("351 MHz S5", 351.193_75),
        ("351 MHz S6", 351.106_25),
        ("351 MHz S15", 351.162_5),
        ("467 MHz ch 1", 467.0),
        ("467 MHz ch 65", 467.4),
        ("467 MHz ch 66", 465.096_875),
        ("467 MHz ch 75", 465.153_125),
        ("154 MHz ch 1", 154.443_75),
        ("154 MHz ch 28 (data)", 154.612_5),
        ("relay D1 input", 465.034_375),
        ("relay D10 input", 465.090_625),
        ("relay D1 output", 468.796_875),
        ("relay D10 output", 468.853_125),
    ] {
        assert!((hz(label) - mhz * 1e6).abs() < 1.0, "{label}");
    }
    let plan = parse_plan(&plan_setting()).expect("the setting takes the plan");
    let i = nearest(&plan, 351.2878e6).expect("on the plan");
    assert_eq!(plan[i].label, "351 MHz ch 15");
    assert!(rows.iter().any(|r| r.0 == "351 MHz ch 15" && r.3));
    assert_eq!(rows.iter().filter(|r| r.3).count(), 1);
    assert_eq!(nearest(&plan, 100e6), None);
}

/// One case of the sweep of 8 calls: rate, channel from the centre, carrier error, clock error, reversed polarity, deviation
/// scale and the carrier to noise ratio in 6.25 kHz.
struct Case {
    rate: f64,
    channel_hz: f64,
    carrier_hz: f64,
    clock_ppm: f64,
    inverted: bool,
    scale: f64,
    snr_db: Option<f64>,
}

const CLEAN: Case = Case {
    rate: 250e3,
    channel_hz: 0.0,
    carrier_hz: 0.0,
    clock_ppm: 0.0,
    inverted: false,
    scale: 1.0,
    snr_db: None,
};

/// Matches the calls found against the calls sent, in order: returns the calls found and the wrong calls. A call that comes
/// again after a loss of the signal (the same user code as the call matched last) is neither.
fn score(found: &[u16], sent: &[u16]) -> (usize, usize) {
    let (mut next, mut ok, mut wrong) = (0, 0, 0);
    for uc in found {
        if next > 0 && sent[next - 1] == *uc {
            continue;
        }
        match sent[next..].iter().position(|s| s == uc) {
            Some(k) => {
                next += k + 1;
                ok += 1;
            }
            None => wrong += 1,
        }
    }
    (ok, wrong)
}

fn sent_signs(content: &[Transmission]) -> Vec<String> {
    content
        .iter()
        .filter_map(|t| match t {
            Transmission::Call(c) => Some(c.call_sign.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn eight_calls_through_fourteen_impairments() {
    let content = gen::sweep(6, 3);
    let signs = sent_signs(&content);
    let cases = [
        ("clean 250 ksps", CLEAN),
        ("clean 48 ksps", Case { rate: 48e3, ..CLEAN }),
        ("clean 1 Msps", Case { rate: 1e6, ..CLEAN }),
        ("carrier +2000 Hz", Case { carrier_hz: 2000.0, ..CLEAN }),
        ("carrier -2000 Hz", Case { carrier_hz: -2000.0, ..CLEAN }),
        ("clock +150 ppm", Case { clock_ppm: 150.0, ..CLEAN }),
        ("clock -150 ppm", Case { clock_ppm: -150.0, ..CLEAN }),
        ("reversed", Case { inverted: true, ..CLEAN }),
        ("deviation x 0.85", Case { scale: 0.85, ..CLEAN }),
        ("deviation x 1.15", Case { scale: 1.15, ..CLEAN }),
        ("40 kHz off, +1500 Hz, 80 ppm", Case { channel_hz: 40e3, carrier_hz: 1500.0, clock_ppm: 80.0, ..CLEAN }),
        ("60 kHz below, reversed, -800 Hz, -60 ppm", Case { channel_hz: -60e3, carrier_hz: -800.0, clock_ppm: -60.0, inverted: true, ..CLEAN }),
        ("24 dB, +1000 Hz", Case { carrier_hz: 1000.0, snr_db: Some(24.0), ..CLEAN }),
        ("20 dB, 100 ppm, reversed", Case { clock_ppm: 100.0, inverted: true, snr_db: Some(20.0), ..CLEAN }),
    ];
    for (name, c) in cases {
        let x = gen::render(&GenOptions {
            rate_hz: c.rate,
            channel_hz: c.channel_hz,
            carrier_hz: c.carrier_hz,
            clock_ppm: c.clock_ppm,
            inverted: c.inverted,
            deviation_scale: c.scale,
            content: content.clone(),
            seed: 7,
            ..GenOptions::default()
        });
        let x = match c.snr_db {
            Some(s) => noisy(x, c.rate, POWER, s, 11),
            None => x,
        };
        let tuned = 351.2875e6;
        let rx = receive(&x, c.rate, tuned - c.channel_hz, tuned, "");
        let r = Owner::results(rx.as_ref());
        let rows = rows(r);
        let found = call_codes(&rows);
        let (ok, wrong) = score(&found, &gen::SWEEP_USER_CODES);
        let got_signs: Vec<&str> = of_kind(&rows, DcrRowKind::CallSign).iter().map(|r| r.identity.as_str()).collect();
        assert!(got_signs.iter().all(|s| signs.iter().any(|t| t == s)), "{name}: {got_signs:?}");
        assert_eq!(wrong, 0, "{name}: {found:?}");
        if c.snr_db.is_none() {
            assert_eq!(found, gen::SWEEP_USER_CODES, "{name}");
            assert_eq!(got_signs.len(), 8, "{name}");
        } else {
            assert!(ok >= 7, "{name}: {found:?}");
            assert!(got_signs.len() >= 6, "{name}: {got_signs:?}");
        }
        let scrambled = of_kind(&rows, DcrRowKind::Call)
            .iter()
            .any(|r| r.identity == "UC 511" && r.text.contains("privacy scrambled"));
        assert!(scrambled || c.snr_db.is_some(), "{name}");
        let calls = CALLS_300.rows::<CallRow>(r).count();
        assert_eq!(calls, found.len(), "{name}");
        println!("{name}: {ok} of 8 calls, {} call signs", got_signs.len());
    }
}

#[test]
fn messages_data_and_a_call_sign_give_the_exact_rows() {
    use sdrkit_stack_dvoice::dcr::link::hex_of_bits;
    for rate in [250e3, 48e3] {
        let x = gen::render(&GenOptions {
            rate_hz: rate,
            carrier_hz: 900.0,
            content: gen::messages(),
            ..GenOptions::default()
        });
        let rx = receive_one(&x, rate);
        let rows = rows(Owner::results(rx.as_ref()));
        let expect = [
            (DcrRowKind::ManufacturerMessage, "UC 7".to_string(), "format 1, manufacturer 5, user code 7, 0 extra bits".to_string()),
            (DcrRowKind::ManufacturerMessage, "UC 7".into(), format!("format 4, manufacturer 5, user code 7, 18 extra bits, {}", hex_of_bits(&gen::bits(18, 2)))),
            (DcrRowKind::ManufacturerMessage, "UC 7".into(), format!("format 8, manufacturer 5, user code 7, 54 extra bits, {}", hex_of_bits(&gen::bits(54, 3)))),
            (DcrRowKind::Data, "TCH1".into(), hex_of_bits(&gen::bits(80, 4))),
            (DcrRowKind::Data, "TCH2".into(), hex_of_bits(&gen::bits(80, 5))),
            (DcrRowKind::CallSign, "234567890".into(), "call sign 234567890".into()),
            (DcrRowKind::Idle, "UC 7".into(), "idle, user code 7".into()),
        ];
        for (kind, identity, text) in &expect {
            let n = rows.iter().filter(|r| r.kind == *kind && r.identity == *identity && r.text == *text).count();
            assert_eq!(n, 1, "{rate}: {kind:?} {identity} {text}: {rows:#?}");
        }
        assert_eq!(rows.len(), expect.len(), "{rate}: {rows:#?}");
        assert_eq!(hex_of_bits(&gen::bits(80, 4)).len(), 20);
    }
}

#[test]
fn a_noise_sweep_of_sixteen_calls() {
    let rate = 250e3;
    let mut content = gen::sweep(6, 3);
    content.extend(gen::sweep(6, 4));
    let sent: Vec<u16> = [gen::SWEEP_USER_CODES, gen::SWEEP_USER_CODES].concat();
    let clean = gen::render(&GenOptions {
        rate_hz: rate,
        carrier_hz: 500.0,
        content,
        seed: 5,
        ..GenOptions::default()
    });
    for snr in [30.0, 26.0, 22.0, 18.0, 14.0, 10.0, 8.0, 6.0, 4.0, 2.0] {
        let x = noisy(clean.clone(), rate, POWER, snr, 13);
        let rx = receive_one(&x, rate);
        let found = call_codes(&rows(Owner::results(rx.as_ref())));
        let (ok, wrong) = score(&found, &sent);
        println!("{snr} dB: {ok} of 16 calls, {wrong} wrong");
        assert!(wrong <= 1, "{snr} dB: {found:?}");
        if snr >= 18.0 {
            assert!(ok >= 15 && wrong == 0, "{snr} dB: {found:?}");
        }
    }
}

/// 150 s at 48 ksps of noise, with an unmodulated carrier or with a carrier frequency modulated by a 1 kHz tone with a
/// deviation of 2.5 kHz.
fn not_dcr(rate: f64, seconds: f64, carrier_hz: Option<f64>, tone: bool, seed: u64) -> Vec<Complex32> {
    let n = (seconds * rate) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    if let Some(f0) = carrier_hz {
        let mut ph = 0.0f64;
        for (i, z) in x.iter_mut().enumerate() {
            let t = i as f64 / rate;
            let f = f0 + if tone { 2_500.0 * (TAU * 1_000.0 * t).sin() } else { 0.0 };
            ph = (ph + TAU * f / rate).rem_euclid(TAU);
            *z = Complex32::from_polar(0.05, ph as f32);
        }
    }
    noisy(x, rate, POWER, 20.0, seed)
}

#[test]
fn noise_a_carrier_and_a_tone_give_no_row() {
    let rate = 48e3;
    for (name, carrier, tone) in [("noise", None, false), ("carrier", Some(1_000.0), false), ("tone", Some(0.0), true)] {
        let x = not_dcr(rate, 150.0, carrier, tone, 21);
        let rx = receive_one(&x, rate);
        let r = Owner::results(rx.as_ref());
        let (sacch_ok, frames, _, _) = counts(r);
        assert!(rows(r).is_empty(), "{name}: {:?}", rows(r));
        assert!(sacch_ok <= 1 + frames / 32, "{name}: {sacch_ok} good SACCH of {frames} frames");
        println!("{name}: {sacch_ok} good SACCH of {frames} frames");
    }
}

/// Four calls of user codes `first` to `first + 3`.
fn four_calls(first: u16) -> Vec<Transmission> {
    (0..4)
        .map(|i| {
            Transmission::Call(CallSpec {
                call_type: 0,
                user_code: first + i,
                manufacturer: 3,
                call_sign: format!("2000000{:02}", first + i),
                frames: 6,
            })
        })
        .collect()
}

#[test]
fn a_scan_of_three_plan_channels_in_one_capture() {
    let rate = 500e3;
    let centre = 351.3e6;
    // (label, frequency, amplitude, carrier error, first user code, gap)
    let chans = [
        ("351 MHz ch 15", 351.2875e6, 0.05, -300.0, 11, 0.6),
        ("351 MHz ch 40", 351.443_75e6, 0.03, 0.0, 21, 0.7),
        ("351 MHz S3", 351.181_25e6, 0.02, 300.0, 31, 0.5),
    ];
    let mut x: Vec<Complex32> = Vec::new();
    for (i, (_, hz, a, err, uc, gap)) in chans.iter().enumerate() {
        let y = gen::render(&GenOptions {
            rate_hz: rate,
            channel_hz: hz - centre,
            carrier_hz: *err,
            amplitude: *a,
            content: four_calls(*uc),
            gap_s: *gap,
            lead_s: 0.3 + 0.2 * i as f64,
            seed: 30 + i as u64,
            ..GenOptions::default()
        });
        if y.len() > x.len() {
            x.resize(y.len(), Complex32::new(0.0, 0.0));
        }
        for (a, b) in x.iter_mut().zip(&y) {
            *a += *b;
        }
    }
    let x = noisy(x, rate, POWER, 30.0, 17);
    let plan = plan_setting();
    let rx = receive(&x, rate, centre, centre, &plan);
    let r = Owner::results(rx.as_ref());
    let rows = rows(r);
    let f = DcrFields::read(r.fields()).unwrap();
    assert!(f.scan_channels >= 3, "{}", f.scan_channels);
    for (label, _, _, _, uc, _) in chans {
        let codes: Vec<u16> = of_kind(&rows, DcrRowKind::Call)
            .iter()
            .filter(|r| r.channel.as_deref() == Some(label))
            .map(|r| {
                assert!(r.identity.starts_with(label), "{}", r.identity);
                r.identity.rsplit("UC ").next().unwrap().parse().unwrap()
            })
            .collect();
        assert_eq!(codes, [uc, uc + 1, uc + 2, uc + 3], "{label}: {rows:#?}");
    }
    assert_eq!(of_kind(&rows, DcrRowKind::Call).len(), 12, "{rows:#?}");
    let calls: Vec<CallRow> = CALLS_300.rows::<CallRow>(r).map(|(_, c)| c).collect();
    assert_eq!(calls.len(), 12);
    // A retune to the same centre: the scan time runs on, and the same capture gives 12 more calls, each ended by its
    // end message.
    let mut rx = rx;
    rx.retuned(&InputInfo::new(rate, centre, centre).recording());
    let mut out = Output::default();
    let zeros = vec![Complex32::new(0.0, 0.0); (2.0 * rate) as usize];
    for c in x.chunks(5000).chain(zeros.chunks(5000)) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let again = self::rows(Owner::results(rx.as_ref()));
    assert_eq!(of_kind(&again, DcrRowKind::Call).len(), 24, "{again:#?}");
    let ends = of_kind(&again, DcrRowKind::CallEnd);
    assert_eq!(ends.len(), 24, "{ends:#?}");
    assert!(ends.iter().all(|r| r.text.ends_with("end-of-communication message")), "{ends:#?}");
    // The same capture tuned to 450 MHz has no plan channel in its span: one channel at the tuned frequency.
    let rx = receive(&x, rate, 450e6, 450e6, &plan);
    let f = DcrFields::read(Owner::results(rx.as_ref()).fields()).unwrap();
    assert_eq!((f.scan_channels, f.scan_decoders), (0, 0));
    assert_eq!(f.channel, Some("not on the plan"));
}

#[test]
fn a_scan_does_not_start_on_noise_a_carrier_or_a_tone() {
    let rate = 500e3;
    let centre = 351.3e6;
    let plan = plan_setting();
    for (name, carrier, tone) in [
        ("noise", None, false),
        ("carrier", Some(351.443_75e6 - centre), false),
        ("tone", Some(351.443_75e6 - centre), true),
    ] {
        let x = not_dcr(rate, 40.0, carrier, tone, 23);
        let rx = receive(&x, rate, centre, centre, &plan);
        let r = Owner::results(rx.as_ref());
        let (sacch_ok, _, _, scan) = counts(r);
        assert!(scan > 0, "{name}");
        assert!(rows(r).is_empty(), "{name}: {:?}", rows(r));
        assert_eq!(sacch_ok, 0, "{name}");
        if carrier.is_none() {
            let f = DcrFields::read(r.fields()).unwrap();
            assert_eq!(f.scan_decoders, 0, "{name}");
        }
    }
}

#[test]
fn nxdn48_into_dcr_and_dcr_into_nxdn() {
    // The NXDN test signal at 48 kHz holds an NXDN48 group call, a status and NXDN96 transmissions. The NXDN stack takes
    // 96 ksps or more.
    let nxdn = sdrkit_stack_dvoice::nxdn::render();
    let rx = receive_one(&nxdn, 48e3);
    let rows = rows(Owner::results(rx.as_ref()));
    assert!(of_kind(&rows, DcrRowKind::Call).is_empty(), "{rows:#?}");
    let x = gen::render(&GenOptions {
        rate_hz: 96e3,
        content: TEST_FRAMES.iter().map(|t| Transmission::TestFrames(*t)).collect(),
        ..GenOptions::default()
    });
    let mut rx = crate::common::receiver("nxdn", &[]);
    let _ = crate::common::run(rx.as_mut(), 96e3, &x, 960);
    let r = Owner::results(rx.as_ref());
    assert_eq!(CALLS_300.rows::<CallRow>(r).count(), 0);
}

#[test]
fn the_receiver_refuses_a_rate_below_48_ksps() {
    let mut rx = crate::common::receiver("dcr", &[]);
    assert!(rx.configure(&InputInfo::new(47e3, 351.2875e6, 351.2875e6)).is_err());
    let summary = &Owner::results(rx.as_ref()).status().summary;
    assert!(summary.contains("DCR needs a sample rate of at least 48 ksps"), "{summary}");
    assert!(rx.configure(&InputInfo::new(48e3, 351.2875e6, 351.2875e6)).is_ok());
}

#[test]
fn the_dcr_benchmark_runs_faster_than_real_time() {
    for b in sdrkit_stack_dvoice::verify::dcr_benchmarks() {
        let r = (b.run)(0.5);
        println!("{}: {:.2} % of one core in real time", b.name, 100.0 * r.cpu_fraction_of_realtime);
        assert!(r.cpu_fraction_of_realtime > 0.0 && r.cpu_fraction_of_realtime < 1.0, "{}", b.name);
    }
}


#[test]
fn the_verify_case_locks_and_lists_a_call() {
    use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
    let d = &sdrkit_stack_dvoice::DCR;
    let o = sdrkit_core::verify::VerifyOptions { seconds: 12.0, clean: false, snr_db: 30.0 };
    let case = &(d.verify.expect("the hooks").cases)(&o)[0];
    let run = sdrkit_dev_test::Harness::new(d)
        .rate(d.tuning.rate_hz)
        .freq(crate::common::start_hz(d))
        .seconds(case.seconds)
        .set(&GEN_OFFSET_HZ, case.offset_hz)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, case.snr_db.unwrap_or(30.0))
        .run()
        .expect("the case runs");
    for line in (case.check.expect("a check"))(&run.results) {
        assert_eq!(line.outcome, sdrkit_core::verify::Outcome::Pass, "{}: {}", line.key, line.detail);
    }
    let rows = rows(&run.results);
    assert!(of_kind(&rows, DcrRowKind::Call).len() >= 3, "{rows:#?}");
}

