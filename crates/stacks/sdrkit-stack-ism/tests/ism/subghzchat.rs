//! `subghzchat`: the packet and its CRC, the keys and AES-256-GCM, the generator round trip at two rates, the replay guard, an
//! inverted spectrum, sent lines read back, declared events with their frequency, strict settings and the retune request.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::owner::Owner;
use sdrkit_core::{
    Choice, Complex32, Generator, HostRequest, InputInfo, Output, Receiver, StackConfig, StateBlob,
    Value,
};
use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_dev_test::Harness;
use sdrkit_graph::StackGraph;
use sdrkit_stack_ism::family::{from_hex, hex};
use sdrkit_stack_ism::subghzchat::{self, Sender, SUBGHZCHAT, TUNE_OFFSET_HZ};
use sdrkit_stack_ism::typed::{
    SubghzchatLineRow, SubghzchatPacketRow, SUBGHZCHAT_CHAT, SUBGHZCHAT_LINE, SUBGHZCHAT_PACKETS,
    SUBGHZCHAT_STATE,
};

/// The chat frequency of the tests.
const CHAT_HZ: f64 = 433.92e6;

fn rx(rate: f64, password: Option<&str>) -> StackGraph {
    let cfg = match password {
        Some(p) => crate::util::stack_with(
            &SUBGHZCHAT,
            &[
                (
                    "encryption",
                    Value::Choice(subghzchat::Encryption::Password.index()),
                ),
                ("password", Value::Text(p.into())),
            ],
        ),
        None => StackConfig::new(sdrkit_core::Values::defaults(&(SUBGHZCHAT.settings)())),
    };
    let mut r = subghzchat::receiver(&cfg).unwrap();
    r.configure(&InputInfo::new(rate, CHAT_HZ + TUNE_OFFSET_HZ, CHAT_HZ).recording())
        .unwrap();
    r
}

/// Feeds `x` and ends the stream; returns the events.
fn feed(r: &mut StackGraph, x: &[Complex32]) -> Vec<sdrkit_core::Event> {
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in x.chunks(65536) {
        r.feed(c, &mut out);
        events.append(&mut out.events);
        out.clear();
    }
    r.finish(&mut out);
    events.append(&mut out.events);
    events
}

/// The chat rows, oldest first: (own, kind, text, packets).
fn chat(r: &StackGraph) -> Vec<(bool, String, String, i64)> {
    let mut rows: Vec<_> = SUBGHZCHAT_CHAT
        .rows::<SubghzchatLineRow>(r.results())
        .map(|(_, row)| {
            (
                row.own,
                subghzchat::LineKind::CHOICES[usize::from(row.kind.index())]
                    .key
                    .to_string(),
                row.text.to_owned(),
                row.packets,
            )
        })
        .collect();
    rows.reverse();
    rows
}

fn kinds(r: &StackGraph) -> Vec<String> {
    chat(r).into_iter().map(|c| c.1).collect()
}

/// The test signal at `rate`, through the standard impairments, as the radio sees it (the chat 300 kHz below the centre).
fn impaired(rate: f64, offset_hz: f64) -> Vec<Complex32> {
    Harness::new(&SUBGHZCHAT)
        .rate(rate)
        .seconds(8.5)
        .seed(2)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 15.0)
        .set(&GEN_OFFSET_HZ, offset_hz)
        .generate()
        .unwrap()
}

fn clean(rate: f64, offset_hz: f64, repeat: bool) -> Vec<Complex32> {
    let mut g = subghzchat::generator(rate, 2, offset_hz, repeat).unwrap();
    let mut x = vec![Complex32::default(); (rate * 16.0) as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    x
}

#[test]
fn the_packet_crc_is_crc16_cms() {
    assert_eq!(
        sdrkit_util_bits::crc::catalog::CRC16_CMS.checksum(b"123456789"),
        0xAEE7
    );
    let exp = read_expected("ism").unwrap();
    let pre = exp["byte_vectors"]["flipper_chat_packet"]["octets_prefix_hex"]
        .as_str()
        .unwrap();
    let p = subghzchat::packet(b"hi", 4);
    assert!(hex(&p).starts_with(&format!("{pre}6869")), "{}", hex(&p));
    let c = sdrkit_util_bits::crc::catalog::CRC16_CMS.checksum(&p[6..9]);
    assert_eq!(&p[9..], &c.to_be_bytes());
}

#[test]
fn the_keys_come_from_sha256_and_hex() {
    // FIPS 180-4 example: SHA-256 of "abc".
    assert_eq!(
        hex(&subghzchat::key_from_password("abc")),
        "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
    );
    let k = "00:11:22:33:44:55:66:77-8899aabbccddeeff 00112233445566778899AABBCCDDEEFF";
    assert_eq!(subghzchat::key_from_hex(k).unwrap()[..2], [0x00, 0x11]);
    assert!(subghzchat::key_from_hex(&"0".repeat(63)).is_none());
    assert!(subghzchat::key_from_hex(&"0".repeat(65)).is_none());
}

/// AES-256-GCM: test case 16 of the GCM specification (AES-256, 96-bit IV, with additional data), and a message that only the
/// right key opens.
#[test]
fn the_chat_cipher_is_the_shared_aes_gcm() {
    let key: [u8; 32] =
        from_hex("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308")
            .unwrap()
            .try_into()
            .unwrap();
    let iv: [u8; 12] = from_hex("cafebabefacedbaddecaf888")
        .unwrap()
        .try_into()
        .unwrap();
    let aad = from_hex("feedfacedeadbeeffeedfacedeadbeefabaddad2").unwrap();
    let mut buf = from_hex(
        "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39",
    )
    .unwrap();
    let tag = subghzchat::cipher(&key)
        .encrypt_in_place(&iv, &aad, &mut buf)
        .unwrap();
    assert_eq!(
        hex(&buf),
        "522DC1F099567D07F47F37A32A84427D643A8CDCBFE5C0C97598A2BD2555D1AA8CB08E48590DBB3DA7B08B1056828838C5F61E6393BA7A0ABCC9F662"
    );
    assert_eq!(hex(&tag), "76FC6ECE0F4E1768CDDF8853BB2D551B");

    let gcm = subghzchat::cipher(&subghzchat::key_from_password("x"));
    let mut s = Sender {
        run_id: 7,
        counter: 1,
    };
    let m = subghzchat::encrypt(&gcm, &mut s, [9; 12], b"a line\r\n").unwrap();
    let (run, ctr, plain) = subghzchat::decrypt(&gcm, &m).unwrap();
    assert_eq!((run, ctr, plain.as_slice()), (7, 1, &b"a line\r\n"[..]));
    let other = subghzchat::cipher(&subghzchat::key_from_password("y"));
    assert!(subghzchat::decrypt(&other, &m).is_none());
    let mut bad = m.clone();
    bad[30] ^= 1;
    assert!(subghzchat::decrypt(&gcm, &bad).is_none());
}

#[test]
fn round_trip_with_and_without_the_password() {
    let want: Vec<String> = subghzchat::conversation()
        .iter()
        .map(|c| c.0.to_string())
        .collect();
    for (rate, off) in [(2e6, 10e3), (250e3, -10e3)] {
        let x = impaired(rate, off);
        let mut r = rx(rate, Some(subghzchat::TEST_PASSWORD));
        feed(&mut r, &x);
        let lines = chat(&r);
        let texts: Vec<String> = lines.iter().map(|l| l.2.clone()).collect();
        assert_eq!(texts, want, "at {rate}");
        assert_eq!(lines[3].1, "decrypted");
        assert_eq!(lines[4].1, "decrypted");
        assert!(lines[4].3 > 1);
        let mut r = rx(rate, None);
        feed(&mut r, &x);
        assert_eq!(
            kinds(&r),
            ["plain", "plain", "plain", "encrypted", "encrypted", "plain"],
            "at {rate}"
        );
    }
}

#[test]
fn a_replay_is_dropped_and_clear_keeps_the_guard() {
    let x = clean(2e6, -TUNE_OFFSET_HZ, true);
    let mut r = rx(2e6, Some(subghzchat::TEST_PASSWORD));
    feed(&mut r, &x);
    let k = kinds(&r);
    assert!(
        k.windows(2)
            .any(|w| w[0] == "decrypted" && w[1] == "repeat"),
        "{k:?}"
    );
    crate::util::clear(&mut r);
    let x = clean(2e6, -TUNE_OFFSET_HZ, false);
    feed(&mut r, &x);
    let k = kinds(&r);
    assert!(
        k.iter().any(|k| k == "repeat") && !k.iter().any(|k| k == "decrypted"),
        "{k:?}"
    );
}

#[test]
fn an_inverted_spectrum_still_reads() {
    // I and Q exchanged: the chat appears 300 kHz above the centre, mirrored.
    let mut x = clean(2e6, TUNE_OFFSET_HZ, false);
    for v in x.iter_mut() {
        *v = Complex32::new(v.im, v.re);
    }
    let mut r = rx(2e6, Some(subghzchat::TEST_PASSWORD));
    feed(&mut r, &x);
    assert_eq!(chat(&r).len(), 6, "{:?}", chat(&r));
    let rows: Vec<_> = SUBGHZCHAT_PACKETS
        .rows::<SubghzchatPacketRow>(r.results())
        .collect();
    assert!(!rows.is_empty());
    for (_, row) in rows {
        assert!(row.inverted);
    }
}

#[test]
fn sent_samples_read_back() {
    let gcm = subghzchat::cipher(&subghzchat::key_from_password("x"));
    let mut s = Sender {
        run_id: 7,
        counter: 1,
    };
    for enc in [false, true] {
        let text = b"sdrkit: a line sent\r\n";
        let msg = if enc {
            subghzchat::encrypt(&gcm, &mut s, [9; 12], text).unwrap()
        } else {
            text.to_vec()
        };
        let mut x = vec![Complex32::default(); 100_000];
        x.extend(subghzchat::tx_samples(&msg, 1e6, -TUNE_OFFSET_HZ, 8).unwrap());
        let mut r = rx(1e6, Some("x"));
        feed(&mut r, &x);
        let c = chat(&r);
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].2, "sdrkit: a line sent");
    }
    assert!(subghzchat::tx_samples(&[0u8; 1025], 1e6, 0.0, 8).is_none());
}

/// The chat events carry the chat frequency.
#[test]
fn every_event_is_declared_and_carries_the_chat_frequency() {
    let decl = sdrkit_stack_ism::typed::shape("subghzchat").results_decl();
    let x = clean(2e6, -TUNE_OFFSET_HZ, false);
    let mut r = rx(2e6, None);
    let events = feed(&mut r, &x);
    assert!(events.iter().filter(|e| e.is(&SUBGHZCHAT_LINE)).count() >= 6);
    assert!(events.iter().any(|e| e.is(&SUBGHZCHAT_STATE)));
    assert!(crate::util::declared(&decl, &events));
    for e in &events {
        assert!(
            (e.freq_hz - CHAT_HZ).abs() < 20e3,
            "{} at {} Hz",
            e.kind(),
            e.freq_hz
        );
    }
}

/// A value of the wrong kind, out of the choices or malformed is refused.
#[test]
fn a_bad_setting_value_is_refused() {
    let mut r = rx(2e6, None);
    assert!(crate::util::set(&mut r, "encryption", Value::Text("rot13".into())).is_err());
    assert!(crate::util::set(&mut r, "encryption", Value::Choice(9)).is_err());
    assert!(crate::util::set(&mut r, "frequency", Value::Choice(9)).is_err());
    assert!(crate::util::set(&mut r, "key", Value::Text("12 34".into())).is_err());
    assert!(crate::util::set(&mut r, "key", Value::F64(5.0)).is_err());
    assert!(crate::util::set(&mut r, "key", Value::Text("".into())).is_ok());
    assert!(crate::util::set(&mut r, "key", Value::Text("ab".repeat(32))).is_ok());
}

#[test]
fn a_new_chat_frequency_asks_the_host_to_retune() {
    let mut r = rx(2e6, None);
    let mut out = Output::default();
    r.feed(&vec![Complex32::default(); 100_000], &mut out);
    assert!(out.requests.is_empty());
    crate::util::set(
        &mut r,
        "frequency",
        Value::Choice(subghzchat::ChatFrequency::F868.index()),
    )
    .unwrap();
    out.clear();
    r.feed(&vec![Complex32::default(); 600_000], &mut out);
    assert_eq!(out.requests.len(), 1);
    match &out.requests[0] {
        HostRequest::Retune { tuned_hz, .. } => assert_eq!(*tuned_hz, 868.35e6),
        q => panic!("{q:?}"),
    }
}

#[test]
fn the_state_comes_back_and_another_mode_is_refused() {
    let x = clean(2e6, -TUNE_OFFSET_HZ, false);
    let mut r = rx(2e6, None);
    feed(&mut r, &x);
    let state = r.save_state().expect("a state");
    let mut back = rx(2e6, None);
    back.restore_state(&state).unwrap();
    back.feed(&vec![Complex32::default(); 100_000], &mut Output::default());
    assert_eq!(chat(&back), chat(&r));
    assert!(back
        .restore_state(&StateBlob::new("subghzchat", 1, b"{}".to_vec()))
        .is_err());
}

/// A change of frequency keeps the chat, the packet log and the counts, and drops the SNR and the level of the present
/// signal.
#[test]
fn a_change_of_frequency_keeps_the_chat_and_drops_the_snr() {
    let x = clean(2e6, -TUNE_OFFSET_HZ, false);
    let mut r = rx(2e6, None);
    feed(&mut r, &x);
    let lines = chat(&r).len();
    let messages = crate::util::number(r.results(), "messages");
    assert!(lines >= 1 && messages > 0.0);
    assert!(crate::util::has(r.results(), "snr_db"));
    r.forget();
    assert_eq!(chat(&r).len(), lines);
    assert_eq!(crate::util::number(r.results(), "messages"), messages);
    assert!(!crate::util::has(r.results(), "snr_db"));
    assert!(!crate::util::has(r.results(), "level_dbfs"));
}
