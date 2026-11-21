//! `nfc`: the CRCs and the Type A framing, the generator round trip through the standard impairments, a reader-only capture,
//! content that is never shown, and no frames from noise.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, Event, InputInfo, Output, Receiver, StackConfig, Value};
use sdrkit_dev_test::Harness;
use sdrkit_graph::StackGraph;
use sdrkit_stack_ism::nfc::{self, crc_a, crc_b, GenParams, Store, Tags, ETU, NFC};

fn rx(rate: f64) -> StackGraph {
    let cfg = StackConfig::new(sdrkit_core::Values::defaults(&(NFC.settings)()));
    let mut r = nfc::receiver(&cfg).unwrap();
    r.configure(&InputInfo::new(rate, 13.56e6, 13.56e6).recording())
        .unwrap();
    r
}

/// Feeds `x` and 0.3 s of bare field; returns what `nfc.tags` keeps, and the events.
fn feed(rate: f64, x: &[Complex32]) -> (Store, Vec<Event>) {
    let mut r = rx(rate);
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in x.chunks(65536) {
        r.feed(c, &mut out);
        events.append(&mut out.events);
    }
    r.feed(
        &vec![Complex32::new(nfc::GEN_FIELD, 0.0); (rate * 0.3) as usize],
        &mut out,
    );
    events.append(&mut out.events);
    let s = r
        .graph()
        .component_as::<Tags>("tags")
        .expect("the tag table")
        .store()
        .clone();
    (s, events)
}

fn names(s: &Store) -> Vec<String> {
    s.log.iter().map(|f| f.name.clone()).collect()
}

/// Acceptance test 1: CRC_A check 0xBF05, CRC_B check 0x906E (ISO/IEC 14443-3 annex B), and the CRC_A bytes of HLTA.
#[test]
fn crcs_and_parity() {
    assert_eq!(crc_a(b"123456789"), 0xBF05);
    assert_eq!(crc_b(b"123456789"), 0x906E);
    assert_eq!(crc_a(&[0x50, 0x00]).to_le_bytes(), [0x57, 0xCD]);
    let bits = nfc::a_bits(&[0x93, 0x20]);
    assert_eq!(nfc::a_bytes(&bits), (vec![0x93, 0x20], true, 18));
    let mut bad = bits.clone();
    bad[8] ^= 1;
    assert!(!nfc::a_bytes(&bad).1);
}

/// Acceptance test 3: the scripted session at 4 and 2 Msps, clean and at 30 dB carrier to noise.
#[test]
fn generator_round_trip() {
    for (rate, cn) in [
        (4e6, None),
        (2e6, None),
        (4e6, Some(30.0)),
        (2e6, Some(30.0)),
    ] {
        let mut h = Harness::new(&NFC).rate(rate).seconds(0.03);
        if let Some(cn) = cn {
            h = h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, cn);
        }
        let x = h.generate().unwrap();
        let (s, _) = feed(rate, &x);
        let n = names(&s);
        let tag = |uid: &str| {
            s.tags
                .get(uid)
                .cloned()
                .unwrap_or_else(|| panic!("{uid} at {rate} {cn:?}: {n:?}"))
        };
        let a = tag("04A1B2C3");
        assert_eq!(a.kind, "ISO 14443-A", "{rate} {cn:?} {n:?}");
        assert_eq!(a.atqa, Some(0x0004));
        assert_eq!(a.sak, Some(0x00));
        let v = tag("E004010203040506");
        assert_eq!(v.kind, "ISO 15693 (NXP Semiconductors)");
        assert_eq!(v.blocks, Some((28, 4)));
        let b = tag("11223344");
        assert_eq!(b.kind, "ISO 14443-B");
    }
}

/// The samples of Type A reader bits at `rate` (field 0.3, pauses of 2.5 µs to 2 %), from the shared modified Miller code.
fn miller_frame(bits: &[u8], rate: f64) -> Vec<Complex32> {
    let etu = ETU * rate;
    let mut chips = Vec::new();
    sdrkit_util_bits::line::modified_miller_encode(bits, &mut chips);
    let n = ((chips.len() / 2 + 1) as f64 * etu) as usize;
    let mut a = vec![0.3f32; n];
    for (h, &c) in chips.iter().enumerate() {
        if c == 0 {
            let s = (h as f64 * 0.5 * etu) as usize;
            for v in a.iter_mut().skip(s).take((2.5e-6 * rate) as usize) {
                *v = 0.006;
            }
        }
    }
    a.into_iter().map(|v| Complex32::new(v, 0.0)).collect()
}

/// Acceptance test 4: a capture beside a reader only.
#[test]
fn reader_only_capture() {
    let rate = 2e6;
    let with = |mut v: Vec<u8>| {
        let c = crc_a(&v);
        v.extend(c.to_le_bytes());
        v
    };
    let short = |v: u8| (0..7).map(|i| (v >> i) & 1).collect::<Vec<u8>>();
    let mut enc = nfc::a_bits(&[0x73, 0x44, 0x2F, 0xCC, 0x38, 0x8D, 0x28, 0x73]);
    for i in (8..enc.len()).step_by(18) {
        enc[i] ^= 1;
    }
    let frames: Vec<Vec<u8>> = vec![
        short(0x52),
        short(0x26),
        nfc::a_bits(&[0x93, 0x20]),
        nfc::a_bits(&with(vec![0x93, 0x70, 0x8E, 0x63, 0x3D, 0x3B, 0xEB])),
        nfc::a_bits(&with(vec![0x60, 0x04])),
        enc,
        nfc::a_bits(&with(vec![0x50, 0x00])),
    ];
    let mut x: Vec<Complex32> = vec![Complex32::new(0.3, 0.0); 4000];
    for f in frames {
        x.extend(miller_frame(&f, rate));
        x.extend(vec![Complex32::new(0.3, 0.0); 1000]);
    }
    let (s, events) = feed(rate, &x);
    let n = names(&s);
    assert!(s.tags.get("8E633D3B").is_some(), "{n:?}");
    assert!(
        n.iter()
            .any(|x| x == "MIFARE Classic authentication request: key A, block 4"),
        "{n:?}"
    );
    assert!(
        n.iter()
            .any(|x| x == "encrypted (MIFARE Classic Crypto1), not decoded"),
        "{n:?}"
    );
    assert_eq!(s.failures, 0);
    assert_eq!(s.tag_frames, 0);
    // The event of the tag carries the carrier frequency, not 0.
    assert!(events.iter().all(|e| e.freq_hz == 13.56e6), "{events:?}");
}

/// Acceptance test 5: a Type 2 READ answer (16 bytes of memory with an NDEF record) is logged by kind and length only.
#[test]
fn memory_content_is_never_shown() {
    let mut v: Vec<u8> = vec![
        0x03, 0x0C, 0xD1, 0x01, 0x08, 0x54, 0x02, 0x65, 0x6E, 0x68, 0x65, 0x6C, 0x6C, 0x6F, 0xFE,
        0x00,
    ];
    let c = crc_a(&v);
    v.extend(c.to_le_bytes());
    let rate = 4e6;
    let w = nfc::frame_waveform(false, 'A', &v, false, rate);
    let mut x: Vec<Complex32> = vec![Complex32::new(nfc::GEN_FIELD, 0.0); 4000];
    nfc::keyed_samples(&w, 4000, rate, nfc::GEN_FIELD, &mut x);
    let mut r = rx(rate);
    let mut out = Output::default();
    r.feed(&x, &mut out);
    r.feed(
        &vec![Complex32::new(nfc::GEN_FIELD, 0.0); 1_200_000],
        &mut out,
    );
    let s = r.graph().component_as::<Tags>("tags").unwrap().store();
    let lines: Vec<_> = s.log.iter().collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].name, "frame (tag), 18 bytes");
    assert!(lines[0].hex.is_empty());
    let all = serde_json::to_string(r.results()).unwrap();
    assert!(!all.contains("68656C6C6F"));
    assert!(!all.to_lowercase().contains("68656c6c6f"));
}

/// Acceptance test 6: noise alone and a bare carrier give no frames.
#[test]
fn noise_and_bare_carrier_give_no_frames() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
    let x: Vec<Complex32> = (0..2_000_000)
        .map(|_| Complex32::new(0.3, 0.0) + rng.complex_normal(2.0 * 0.003f64.powi(2)))
        .collect();
    let (s, _) = feed(2e6, &x);
    assert_eq!(s.count, 0, "{:?}", names(&s));
    let x: Vec<Complex32> = (0..2_000_000)
        .map(|_| rng.complex_normal(2.0 * 0.01f64.powi(2)))
        .collect();
    let (s, _) = feed(2e6, &x);
    assert_eq!(s.count, 0);
}

/// Acceptance test 2 at the bit level: short frames and multi-byte frames survive the Type A framing, parity errors are flagged,
/// and the shared modified Miller code follows ISO/IEC 14443-2 (X for 1, Y after a 1, Z otherwise).
#[test]
fn type_a_bit_framing_and_miller_sequences() {
    for v in [0x26u8, 0x52] {
        let bits: Vec<u8> = (0..7).map(|i| (v >> i) & 1).collect();
        assert_eq!(nfc::a_bytes(&bits), (vec![v], true, 7));
    }
    let frame = [0x93u8, 0x70, 0x8E, 0x63, 0x3D, 0x3B, 0xEB];
    let bits = nfc::a_bits(&frame);
    assert_eq!(bits.len(), 63);
    assert_eq!(nfc::a_bytes(&bits), (frame.to_vec(), true, 63));
    for k in 0..frame.len() {
        let mut b = bits.clone();
        b[9 * k + 8] ^= 1;
        let (v, ok, _) = nfc::a_bytes(&b);
        assert_eq!(v, frame.to_vec());
        assert!(!ok, "parity of byte {k}");
    }
    let pauses = |bits: &[u8]| {
        let mut c = Vec::new();
        sdrkit_util_bits::line::modified_miller_encode(bits, &mut c);
        c.iter()
            .enumerate()
            .filter(|(_, &x)| x == 0)
            .map(|(i, _)| i)
            .collect::<Vec<_>>()
    };
    // Bits 1, 0, 0, 1: start Z, X, Y, Z, X; the end's logic 0 after a 1 is Y.
    assert_eq!(pauses(&[1, 0, 0, 1]), vec![0, 3, 6, 9]);
    // Bits 0, 1, 0: start Z, Z, X, Y; the end's logic 0 after a 0 is Z.
    assert_eq!(pauses(&[0, 1, 0]), vec![0, 2, 5, 8]);
}

#[test]
fn a_reader_frame_with_z_after_x_is_refused() {
    // REQA with its fourth pause moved to the start of the next bit time: X then Z, which the code does not allow.
    let rate = 4e6;
    let etu = ETU * rate;
    let bits: Vec<u8> = (0..7).map(|i| (0x26u8 >> i) & 1).collect();
    let mut chips = Vec::new();
    sdrkit_util_bits::line::modified_miller_encode(&bits, &mut chips);
    let x_at = chips
        .iter()
        .enumerate()
        .position(|(i, &c)| c == 0 && i % 2 == 1)
        .unwrap();
    chips[x_at + 1] = 0;
    let n = ((chips.len() / 2 + 1) as f64 * etu) as usize;
    let mut a = vec![1.0f32; 8000 + n];
    for (h, &c) in chips.iter().enumerate() {
        if c == 0 {
            let s = 8000 + (h as f64 * 0.5 * etu) as usize;
            for v in a.iter_mut().skip(s).take((2.5e-6 * rate) as usize) {
                *v = 0.02;
            }
        }
    }
    let x: Vec<Complex32> = a.into_iter().map(|v| Complex32::new(v, 0.0)).collect();
    let (s, _) = feed(rate, &x);
    assert!(!names(&s).iter().any(|n| n == "REQA"), "{:?}", names(&s));
}

#[test]
fn every_event_is_declared_and_has_its_frequency() {
    let decl = sdrkit_stack_ism::typed::shape("nfc").results_decl();
    let run = Harness::new(&NFC).seconds(0.1).run().unwrap();
    assert!(!run.events.is_empty());
    assert!(crate::util::declared(&decl, &run.events));
    for e in &run.events {
        assert!(e.freq_hz > 13e6, "{} at {} Hz", e.kind(), e.freq_hz);
    }
}

/// A setting the stack does not have is refused.
#[test]
fn an_unknown_setting_is_refused() {
    let cfg = StackConfig::new(sdrkit_core::Values::defaults(&(NFC.settings)()));
    let mut r = nfc::receiver(&cfg).unwrap();
    assert!(crate::util::set(&mut r, "cn", Value::F64(10.0)).is_err());
}

#[test]
fn a_reader_only_test_signal_identifies_no_tag_answers() {
    let rate = 4e6;
    let mut g = nfc::generator(rate, GenParams { reader_only: true }).unwrap();
    let mut x = vec![Complex32::default(); (rate * 0.03) as usize];
    sdrkit_core::Generator::generate(&mut g, &mut x);
    let (s, _) = feed(rate, &x);
    assert_eq!(s.tag_frames, 0);
    assert!(s.tags.get("04A1B2C3").is_some(), "{:?}", names(&s));
}

/// The receiver benchmark (`ism.nfc_receiver_4m`) runs the whole chain.
#[test]
fn the_receiver_benchmark_runs() {
    let b = nfc::benchmarks();
    assert_eq!(b[0].name, "ism.nfc_receiver_4m");
    assert!((b[0].run)(0.01).items_per_second > 0.0);
}

/// A change of frequency keeps the tags, the frame log and the counts, and drops the levels of the present field.
#[test]
fn a_change_of_frequency_keeps_the_tags_and_drops_the_levels() {
    let h = Harness::new(&NFC).rate(4e6).seconds(0.1);
    let mut r = rx(4e6);
    crate::util::feed_all(&mut r, &h.generate().unwrap());
    let tags = crate::util::table(r.results(), "tags").len();
    let frames = crate::util::number(r.results(), "frames");
    assert!(tags >= 1 && frames > 0.0);
    assert!(crate::util::has(r.results(), "field_dbfs"));
    r.forget();
    assert_eq!(crate::util::table(r.results(), "tags").len(), tags);
    assert_eq!(crate::util::number(r.results(), "frames"), frames);
    assert!(!crate::util::has(r.results(), "field_dbfs"));
    assert!(!crate::util::has(r.results(), "subcarrier_dbfs"));
}
