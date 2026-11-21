//! CDMA2000: the pilot PN, the Sync Channel coding, the capsule and the message, System Time, channel numbers, the search on
//! generated base stations and the stack end to end.

use crate::common::*;
use sdrkit_algo_fec::conv::{codes::CDMA2000_K9, Encoder, Ends, Viterbi};
use sdrkit_algo_modem::spreading::cdma2000::short_pn;
use sdrkit_algo_modem::spreading::walsh;
use sdrkit_core::InputInfo;
use sdrkit_stack_cell::cdma2000::gen::{BaseStation, CdmaGenerator, Pulse};
use sdrkit_stack_cell::cdma2000::search::{CdmaFound, CdmaSearcher, WORK_RATE};
use sdrkit_stack_cell::cdma2000::sync::{
    bro7, channel_mhz, frame_symbols, Capsule, Reassembly, SyncMessage,
};
use sdrkit_stack_cell::decoder::Capture;
use sdrkit_stack_cell::CDMA2000;
use sdrkit_util_bits::crc::catalog::CRC30_CDMA;
use serde_json::Value as J;

fn hex(bits: &[u8]) -> String {
    bits.chunks(4)
        .map(|q| format!("{:X}", q.iter().fold(0u8, |a, &b| a << 1 | b) << (4 - q.len())))
        .collect()
}

#[test]
fn pilot_pn_sequences() {
    for (q, head) in [(false, "A93A37990784695A"), (true, "9EBAD38A738D8E98")] {
        let s = short_pn(q);
        assert_eq!(s.len(), 32_768);
        assert_eq!(s.iter().filter(|&&b| b == 1).count(), 16_384);
        assert_eq!(s[0], 1);
        assert!(s[32_753..].iter().all(|&b| b == 0));
        assert_eq!(s[32_752], 1);
        let mut run = 0;
        let mut longest = 0;
        for &b in s.iter().chain(s.iter()) {
            run = if b == 0 { run + 1 } else { 0 };
            longest = longest.max(run);
        }
        assert_eq!(longest, 15);
        // The recurrence holds away from the inserted zero.
        let taps: &[usize] = if q { &[15, 12, 11, 10, 9, 5, 4, 3] } else { &[15, 10, 8, 7, 6, 2] };
        for n in 15..32_752 {
            assert_eq!(s[n], taps.iter().fold(0, |a, &d| a ^ s[n - d]), "{n}");
        }
        for lag in [1usize, 7, 64, 1000, 16_384] {
            let a: i64 = (0..32_768)
                .map(|n| (1 - 2 * i64::from(s[n])) * (1 - 2 * i64::from(s[(n + lag) % 32_768])))
                .sum();
            assert!(a.abs() <= 400, "{lag}: {a}");
        }
        assert_eq!(hex(&s[..64]), head);
    }
}

#[test]
fn walsh_64_is_orthogonal() {
    for a in 0..64 {
        for b in 0..64 {
            let s: i32 = walsh(a, 64).iter().zip(walsh(b, 64)).map(|(x, y)| i32::from(*x) * i32::from(y)).sum();
            assert_eq!(s, if a == b { 64 } else { 0 });
        }
    }
}

#[test]
fn encoder_impulse_response() {
    let mut e = Encoder::new(CDMA2000_K9);
    let mut out = Vec::new();
    e.encode(&[1, 0, 0, 0, 0, 0, 0, 0, 0], &mut out);
    let g0: u32 = out.iter().step_by(2).take(9).fold(0, |a, &b| a << 1 | u32::from(b));
    let g1: u32 = out.iter().skip(1).step_by(2).take(9).fold(0, |a, &b| a << 1 | u32::from(b));
    assert_eq!((g0, g1), (0o753, 0o561));
}

#[test]
fn viterbi_bit_error_ratio() {
    let mut rng = sdrkit_dev_test::Rng::new(5);
    for (ebn0, limit) in [(4.0, 1e-3), (3.0, 5e-3)] {
        let mut errors = 0;
        for _ in 0..20 {
            let bits: Vec<u8> = (0..400).map(|_| (rng.next_u64() & 1) as u8).collect();
            let mut code = Vec::new();
            Encoder::new(CDMA2000_K9).encode(&bits, &mut code);
            // Rate 1/2: Es/N0 = Eb/N0 - 3 dB; BPSK with unit symbols.
            let sigma = (1.0 / (2.0 * 0.5 * 10f64.powf(ebn0 / 10.0))).sqrt();
            let llr: Vec<i8> = code
                .iter()
                .map(|&c| {
                    let y = 1.0 - 2.0 * f64::from(c) + sigma * rng.gauss();
                    (y * 2.0 / (sigma * sigma) * 4.0).round().clamp(-127.0, 127.0) as i8
                })
                .collect();
            let mut v = Viterbi::new(CDMA2000_K9, 400).unwrap();
            let mut out = vec![0u8; 400];
            v.decode_only(&llr, Ends::Start(0), &mut out).unwrap();
            errors += out.iter().zip(&bits).filter(|(a, b)| a != b).count();
        }
        let ber = errors as f64 / 8000.0;
        assert!(ber < limit, "{ebn0} dB: {ber}");
    }
}

#[test]
fn interleavers_are_permutations() {
    let bro = |m: u32, i: usize| (i as u32).reverse_bits() as usize >> (32 - m);
    assert_eq!(bro(3, 6), 3);
    let mut a: Vec<usize> = (0..128).map(bro7).collect();
    a.sort_unstable();
    assert_eq!(a, (0..128).collect::<Vec<_>>());
    let mut b: Vec<usize> = (0..384).map(|i| 64 * (i % 6) + bro(6, i / 6)).collect();
    b.sort_unstable();
    assert_eq!(b, (0..384).collect::<Vec<_>>());
}

#[test]
fn crc_30() {
    let crc = &CRC30_CDMA;
    assert_eq!(crc.checksum(b"123456789"), 0x04C3_4ABF);
    let zero = vec![0u8; 100];
    let c0 = crc.bits(&zero);
    assert_ne!(c0, 0);
    for k in 0..100 {
        let mut d = zero.clone();
        d[k] = 1;
        assert_ne!(crc.bits(&d), c0, "{k}");
    }
}

fn test_message() -> SyncMessage {
    SyncMessage {
        p_rev: 6,
        min_p_rev: 3,
        sid: 4139,
        nid: 65_535,
        pilot_pn: 168,
        lc_state: 0x2A5_A5A5_A5A5,
        sys_time: 17_000_000_000,
        lp_sec: 18,
        ltm_off: -10,
        daylt: true,
        prat: 1,
        cdma_freq: 283,
        ext_cdma_freq: 384,
    }
}

#[test]
fn message_capsule_and_frames() {
    let m = test_message();
    assert_eq!(m.pdu().len(), 186);
    let cap = m.capsule();
    assert_eq!(cap.len(), 224);
    assert_eq!(hex(&cap), "1C0106032057FFFEA8A9696969694FD51DA8004B6A46CC002CC376FA");
    let frames = m.frames();
    assert_eq!(frames.len(), 9);
    assert_eq!(hex(&frames[0]), "8E008301");
    assert_eq!(hex(&frames[1]), "4815FFFF");
    assert_eq!(hex(&frames[7]), "7A000000");
    assert_eq!(hex(&frames[8]), "00000000");
    let mut r = Reassembly::default();
    let got: Vec<Capsule> = frames.iter().filter_map(|f| r.push(f)).collect();
    assert_eq!(got.len(), 1);
    let Capsule::Good(pdu) = &got[0] else { panic!("{got:?}") };
    assert_eq!(SyncMessage::parse(pdu), Some(m.clone()));
    // Frame 0 coded from state 0, then repeated and interleaved.
    let mut code = Vec::new();
    Encoder::new(CDMA2000_K9).encode(&frames[0], &mut code);
    assert_eq!(hex(&code), "EFB88C5CEF6F8BA4");
    assert_eq!(hex(&frame_symbols(&code)), "F9FECDD4C6E70AD4F9FECDD4C6E70AD4");
    // A frame with a bit flipped fails its CRC.
    let mut bad = frames.clone();
    bad[3][5] ^= 1;
    let mut r = Reassembly::default();
    let got: Vec<Capsule> = bad.iter().filter_map(|f| r.push(f)).collect();
    assert_eq!(got, vec![Capsule::Bad]);
}

#[test]
fn system_time_as_text() {
    let mut m = SyncMessage::default();
    assert_eq!(m.utc_text(), "1980-01-06 00:00:00.00 UTC");
    m.sys_time = 12;
    assert_eq!(m.utc_text(), "1980-01-06 00:00:00.96 UTC");
    let t = test_message();
    assert_eq!(t.utc_text(), "2023-02-09 17:46:22.00 UTC");
    assert_eq!(t.local_text(), "2023-02-09 12:46:22.00");
}

#[test]
fn channel_numbers() {
    let near = |a: Option<f64>, b: f64| a.is_some_and(|a| (a - b).abs() < 1e-6);
    assert!(near(channel_mhz(0, 283), 878.490));
    assert!(near(channel_mhz(1, 25), 1931.250));
    assert!(near(channel_mhz(0, 1013), 869.700));
    assert_eq!(channel_mhz(0, 900), None);
}

/// The stations of a receiver case: the message of the format test with SID 100 + k and SYS_TIME 1000 + 37 k.
fn stations(pns: &[(u16, f32)], offset: f64) -> Vec<BaseStation> {
    pns.iter()
        .enumerate()
        .map(|(k, &(pn, amp))| BaseStation {
            msg: SyncMessage {
                pilot_pn: pn,
                sid: 100 + k as u16,
                sys_time: 1000 + 37 * k as u64,
                ..test_message()
            },
            offset_hz: offset + 40.0 * k as f64,
            amp,
        })
        .collect()
}

fn receive(st: &[BaseStation], ppm: f64, snr_db: f64, start: i64, pulse: Pulse, seconds: f64) -> Vec<CdmaFound> {
    let gen = CdmaGenerator::new(WORK_RATE, st.to_vec(), start, pulse, 9);
    let mut keys = snr(snr_db);
    keys.push(("gen_clock_ppm", J::from(ppm)));
    let x = impaired(gen, &keys, seconds);
    let cap = Capture::new(x, WORK_RATE, 0.0, seconds);
    CdmaSearcher::default().search(&cap, 5000.0, 6, 2.4)
}

fn check(pns: &[(u16, f32)], offset: f64, ppm: f64, snr_db: f64, start: i64, pulse: Pulse) -> Vec<CdmaFound> {
    let st = stations(pns, offset);
    let found = receive(&st, ppm, snr_db, start, pulse, 0.7);
    let sum: f64 = pns.iter().map(|p| f64::from(p.1).powi(2)).sum();
    for f in found.iter().filter_map(|f| f.msg.as_ref()) {
        assert!(pns.iter().any(|p| p.0 == f.pilot_pn), "unknown PN {}: {found:?}", f.pilot_pn);
    }
    for s in &st {
        let f = found
            .iter()
            .find(|f| f.msg.as_ref().is_some_and(|m| m.pilot_pn == s.msg.pilot_pn))
            .unwrap_or_else(|| panic!("PN {} not decoded: {found:?}", s.msg.pilot_pn));
        let m = f.msg.as_ref().unwrap();
        assert_eq!((m.sid, m.cdma_freq, m.lc_state), (s.msg.sid, s.msg.cdma_freq, s.msg.lc_state));
        assert!((f.offset_hz - s.offset_hz).abs() < 60.0, "{f:?}");
        let want = -7.0 + 20.0 * f64::from(s.amp).log10() - 10.0 * sum.log10()
            - 10.0 * (1.0 + 0.5 * 10f64.powf(-snr_db / 10.0)).log10();
        assert!((f.ec_io_db - want).abs() < 2.0, "Ec/Io {} against {want}: {f:?}", f.ec_io_db);
    }
    found
}

#[test]
fn case_1_one_station() {
    check(&[(168, 1.0)], 0.0, 0.0, 10.0, 0, Pulse::Fir);
}

#[test]
fn case_1_with_the_root_raised_cosine() {
    check(&[(168, 1.0)], 0.0, 0.0, 10.0, 0, Pulse::Rrc);
}

#[test]
fn case_2_offset_at_3_db() {
    check(&[(12, 1.0)], 3100.0, 0.0, 3.0, 12_345, Pulse::Fir);
}

#[test]
fn case_3_offset_and_clock() {
    check(&[(300, 1.0)], -4400.0, 12.0, 0.0, 7_777_777, Pulse::Fir);
}

#[test]
fn case_4_two_stations_and_derived_offsets() {
    let found = check(&[(5, 1.0), (260, 0.6)], 1200.0, -8.0, 6.0, 99_999, Pulse::Fir);
    // Each pilot's relative offset agrees with its PN against the other.
    let pn5 = found.iter().find(|f| f.msg.as_ref().is_some_and(|m| m.pilot_pn == 5)).unwrap();
    let pn260 = found.iter().find(|f| f.msg.as_ref().is_some_and(|m| m.pilot_pn == 260)).unwrap();
    assert_eq!((i64::from(pn260.relative) - i64::from(pn5.relative)).rem_euclid(512), 255);
}

#[test]
fn case_5_three_stations_two_adjacent() {
    check(&[(40, 1.0), (41, 0.7), (400, 0.5)], -700.0, 4.0, 8.0, 3333, Pulse::Fir);
}

#[test]
fn case_6_minus_3_db() {
    check(&[(499, 1.0)], 250.0, 0.0, -3.0, 54_321, Pulse::Fir);
}

#[test]
fn case_7_noise_gives_no_message() {
    let x = sdrkit_dev_test::Awgn::with_sigma(0.1, 5).noise(WORK_RATE as usize);
    let cap = Capture::new(x, WORK_RATE, 0.0, 1.0);
    let found = CdmaSearcher::default().search(&cap, 5000.0, 6, 2.4);
    assert!(found.iter().all(|f| f.msg.is_none()), "{found:?}");
}

#[test]
fn derived_offset_of_a_pilot_without_a_message() {
    // PN 5 at -14 dB against PN 260: its pilot is found, its Sync Channel is too weak to decode, and its PN offset follows from
    // PN 260 and the relative offset.
    let st = stations(&[(260, 1.0), (5, 0.2)], 0.0);
    let found = receive(&st, 0.0, 20.0, 99_999, Pulse::Fir, 0.7);
    assert!(found.iter().any(|f| f.msg.as_ref().is_some_and(|m| m.pilot_pn == 260)), "{found:?}");
    let weak = found.iter().find(|f| f.msg.is_none() && f.derived_pn == Some(5));
    assert!(weak.is_some(), "{found:?}");
    for f in found.iter().filter(|f| f.msg.is_none()) {
        assert_eq!(f.derived_pn, Some(5), "{found:?}");
    }
}

#[test]
fn case_8_the_stack_end_to_end() {
    let mut st = stations(&[(168, 1.0)], 700.0);
    st[0].msg.sid = 4139;
    let x = impaired(CdmaGenerator::new(WORK_RATE, st, 4321, Pulse::Fir, 2), &snr(6.0), 1.0);
    let mut rx = stack_graph(&CDMA2000, &[("capture_s", J::from(0.98))]);
    let input = InputInfo::new(WORK_RATE, 878.49e6, 878.49e6).recording();
    let s = run_graph(&mut rx, input, &x, &[8192]);
    let cs = cells(&s);
    assert_eq!(cs.len(), 1, "{cs:?}");
    assert_eq!(cs[0]["msg"]["sid"], 4139);
    assert_eq!(cs[0]["msg"]["pilot_pn"], 168);
}

#[test]
fn region_data_names_the_band_classes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans/cell3g.json");
    let text = std::fs::read_to_string(&path).expect("the region data file");
    let mut plans = sdrkit_data_plans::Plans::new();
    sdrkit_data_plans::api::import_as(&mut plans, &text, sdrkit_data_plans::api::PlanFormat::Json, "", "")
        .expect("the registry takes the file");
    sdrkit_data_plans::data::parse("cell3g.json", &text).expect("a plan file");
    // The band class of a radio frequency is region data: 878.49 MHz lies in the band of band class 0, and its channel 283 is
    // the standard's channel arithmetic.
    let j: J = serde_json::from_str(&text).unwrap();
    let group = j["groups"].as_array().unwrap().iter().find(|g| g["id"] == "cell_cdma2000").unwrap();
    let band = group["bands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["start_hz"].as_f64().unwrap() <= 878.49e6 && 878.49e6 <= b["end_hz"].as_f64().unwrap())
        .unwrap();
    assert!(band["note"].as_str().unwrap().contains("band_class=0"));
    let centre = band["channels"][0]["centre_hz"].as_f64().unwrap();
    assert!((channel_mhz(0, 283).unwrap() * 1e6 - centre).abs() < 1.0);
}
