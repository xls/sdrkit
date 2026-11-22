//! The protocol, table and generator cases, the descriptors, and the cut independence of the receiver.

use super::common::*;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Receiver, StackConfig, Value, Values};
use sdrkit_stack_wsjt::encode::{self, channel_symbols};
use sdrkit_stack_wsjt::generator::{stations, StationSource, WsjtGenerator};
use sdrkit_stack_wsjt::proto::codes;
use sdrkit_stack_wsjt::proto::{js8msg, k32, msg72, wsprmsg};
use sdrkit_stack_wsjt::runner::SlotResult;
use sdrkit_stack_wsjt::rx::slot::Decode;
use sdrkit_stack_wsjt::table::Display;
use sdrkit_stack_wsjt::{descriptors, Mode, Sub};
use sdrkit_util_objects::Tables;

#[test]
fn t_jt9_1_t_jt4_1_t_q65_2_t_fst_2_t_msk_1_frames() {
    let s = channel_symbols(&Sub::new(Mode::Jt9), "CQ K1ABC FN42").unwrap();
    assert_eq!(s.len(), 85);
    assert!(s.iter().all(|&t| t <= 8));
    assert_eq!(s.iter().filter(|&&t| t == 0).count(), 16);
    assert_ne!(
        s,
        channel_symbols(&Sub::new(Mode::Jt9), "K1ABC W9XYZ -15").unwrap()
    );
    let s = channel_symbols(&Sub::new(Mode::Jt4), "CQ K1ABC FN42").unwrap();
    assert_eq!(s.len(), 206);
    assert!(s.iter().all(|&t| t <= 3));
    assert!((Sub::new(Mode::Jt4).tx_s() - 47.088).abs() < 0.005);
    assert_eq!(channel_symbols(&Sub::new(Mode::Q65), "CQ K1ABC FN42").unwrap().len(), 85);
    let f = channel_symbols(&Sub::new(Mode::Fst4), "CQ K1ABC FN42").unwrap();
    assert_eq!(f.len(), 160);
    assert_eq!(f[..8], encode::FST4_SYNC[0]);
    let w = channel_symbols(&Sub::new(Mode::Fst4w), "K1ABC FN42 37").unwrap();
    assert_ne!(f, w);
    let m = channel_symbols(&Sub::new(Mode::Msk144), "CQ K1ABC FN42").unwrap();
    assert_eq!(m.len(), 144);
    assert_eq!(m[..8], encode::MSK_SYNC);
    assert_eq!(m[56..64], encode::MSK_SYNC);
    let mut cw = m[8..56].to_vec();
    cw.extend(&m[64..]);
    assert!(codes::msk144().is_codeword(&cw));
}

#[test]
fn t_jt9_1_fano_with_flipped_bits() {
    // Soft bits (positive means 0) at about 0 dB Es/N0 (BPSK in white noise): many hard decisions wrong.
    let dat = msg72::pack72("K1ABC W9XYZ -15").unwrap();
    let b = msg72::bits72(&dat).to_vec();
    let sent = k32::encode(&b);
    assert_eq!(sent.len(), 206);
    let mut r = rng(3);
    let sigma = (0.5f64).sqrt();
    let llr: Vec<f32> = sent
        .iter()
        .map(|&x| {
            let v = if x == 1 { -1.0 } else { 1.0 } + sigma * r.normal();
            (2.0 * v / (sigma * sigma)) as f32
        })
        .collect();
    let wrong = llr
        .iter()
        .zip(&sent)
        .filter(|(&l, &x)| (l < 0.0) != (x == 1))
        .count();
    assert!(wrong > 10, "{wrong}");
    let got = k32::decode(&llr, 72).unwrap();
    assert_eq!(got.bits, b);
}

#[test]
fn t_js8_10_protocol() {
    // The CRC catches a flipped bit; parity checks hold; tones and Costas arrays; symbol rates.
    let f = js8msg::Frame::Heartbeat {
        call: "LY2ABC".into(),
        grid: "KO24".into(),
        cq: false,
        choice: 0,
    };
    let bits = f.pack().unwrap();
    let mut info = js8msg::info87(&bits, 3);
    assert!(js8msg::check87(&info).is_some());
    info[10] ^= 1;
    assert!(js8msg::check87(&info).is_none());
    for sm in 0..3u8 {
        let t = encode::js8_tones(sm, &bits, 3);
        assert_eq!(t.len(), 79);
        let cs = encode::js8_costas(sm);
        assert_eq!(t[..7], cs[0]);
        assert_eq!(t[36..43], cs[1]);
        assert_eq!(t[72..], cs[2]);
    }
    assert_eq!(encode::js8_tones(2, &bits, 3)[..2], [0, 6]);
    assert_eq!(encode::js8_tones(2, &bits, 3)[36], 1);
    assert_eq!(encode::js8_tones(2, &bits, 3)[72], 2);
    for (sm, baud) in [(0u8, 6.25), (1, 10.0), (2, 20.0)] {
        assert!((Sub::new(Mode::Js8).with_sub(sm).spacing_hz() - baud).abs() < 1e-9);
    }
}

#[test]
fn t_msg_7_and_wspr_code() {
    // T-MSG-3: encode, interleave, flip 10 bits of +-3 soft values, deinterleave, Fano.
    for t in [
        "K1ABC FN42 37",
        "G4XYZ IO91 23",
        "PJ4/K1ABC 30",
        "DL1XYZ/P 10",
        "K1ABC/12 33",
        "<K1ABC> FN42AX 37",
    ] {
        let b50 = wsprmsg::pack50(t).unwrap();
        let sent = k32::encode(&b50);
        assert_eq!(sent.len(), 162);
        let mut llr: Vec<f32> = sent
            .iter()
            .map(|&x| if x == 1 { -3.0 } else { 3.0 })
            .collect();
        for i in 0..10 {
            llr[i * 15 + 2] = -llr[i * 15 + 2];
        }
        let got = k32::decode(&llr, 50).unwrap();
        assert_eq!(&got.bits[..], &b50[..]);
        let mut h = wsprmsg::WsprHashes::default();
        h.learn("K1ABC");
        assert_eq!(wsprmsg::unpack50(&got.bits, &h).unwrap().text, t);
    }
}

fn dec(i: usize) -> Decode {
    Decode {
        slot: 1_767_225_600.0 + 15.0 * i as f64,
        mode: Mode::Ft8,
        label: "FT8".into(),
        snr: -10,
        dt: 0.1,
        hz: 1000.0 + i as f64,
        text: format!("CQ K{}ABC FN42", i % 10),
        mirrored: false,
        cq: true,
        call: format!("K{}ABC", i % 10),
        grid: "FN42".into(),
        dbm: -1,
        pass: 0,
        dial: 14.074e6,
        deep: false,
        how: Default::default(),
        early: false,
        band: None,
    }
}

fn slot_of(decodes: Vec<Decode>) -> SlotResult {
    SlotResult {
        mode: Some(Mode::Ft8),
        utc: decodes.first().map_or(0.0, |d| d.slot),
        decodes,
        label: "FT8".into(),
        period: 15.0,
        ..Default::default()
    }
}

#[test]
fn t_tab_1_2_3_the_table() {
    let mut d = Display::new();
    for i in 0..1001 {
        d.slot(&slot_of(vec![dec(i)]), &mut Vec::new());
    }
    assert_eq!(d.heard.decodes.len(), 1000);
    assert_eq!(table(d.results(), "decodes").len(), 1000);
    assert_eq!(d.heard.decodes.iter().next().unwrap().hz, 1001.0);
    let saved = d.heard.save_state(1_767_241_000.0).unwrap();
    d.clear();
    assert!(d.heard.decodes.is_empty());
    assert!(table(d.results(), "decodes").is_empty());
    d.restore(&saved).unwrap();
    assert_eq!(d.heard.decodes.len(), 1000);
    assert_eq!(table(d.results(), "decodes").len(), 1000);
    assert!(d
        .restore(&sdrkit_core::StateBlob::new(
            "something.else",
            1,
            Vec::new()
        ))
        .is_err());
    assert_eq!(d.heard.decodes.len(), 1000);
}

#[test]
fn t_tab_2_clear_through_the_receiver_keeps_the_time_source() {
    let sub = Sub::new(Mode::Ft8);
    let st: Vec<St> = stations(Mode::Ft8)
        .iter()
        .take(3)
        .map(|s| (s.0, s.1, -6.0, s.3, false))
        .collect();
    let iq = iq_of(&audio_of(sub, &st, 16.0, true, 77));
    let mut rx = receiver(Some(Mode::Ft8), &[], 8000.0, 0.0);
    let ev = feed(&mut rx, &iq, 800);
    assert_eq!(texts(&ev).len(), 3);
    let saved = rx.save_state().unwrap();
    let clear = rx.commands().id("clear").unwrap();
    let args = rx.commands().def(clear).unwrap().new_args();
    owner::invoke(&mut rx, clear, &args).unwrap();
    let mut out = Output::default();
    rx.feed(&[], &mut out);
    assert!(table(rx.results(), "decodes").is_empty());
    assert_eq!(choice_key(rx.results(), "time_source"), "recording_start");
    rx.restore_state(&saved).unwrap();
    assert_eq!(table(rx.results(), "decodes").len(), 3);
}

#[test]
fn t_gen_1_every_station_list_through_its_decoder() {
    for m in [
        Mode::Ft8,
        Mode::Ft4,
        Mode::Ft2,
        Mode::Wspr,
        Mode::Jt65,
        Mode::Jt9,
        Mode::Jt4,
        Mode::Fst4,
        Mode::Fst4w,
        Mode::Msk144,
    ] {
        let sub = Sub::new(m);
        let mut s = StationSource::new(sub, 0.3, false).unwrap();
        let mut a = s.audio((sub.period * 12000.0) as usize);
        // 12 kHz to 8 kHz.
        let mut r = sdrkit_algo_dsp::resample::Resampler::<f32>::new(12000.0, 8000.0).unwrap();
        let mut a8 = Vec::new();
        r.process(&a, &mut a8);
        a.clear();
        let n8 = noise(a8.len(), 0.01, 5);
        for (x, n) in a8.iter_mut().zip(n8) {
            *x += n;
        }
        let d = decode_with(
            sub,
            &a8,
            sdrkit_stack_wsjt::runner::DecodeState::default(),
        );
        for st in stations(m) {
            assert!(
                d.iter().any(|x| x.text == st.0),
                "{m:?}: {} missing in {:?}",
                st.0,
                d.iter().map(|x| &x.text).collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn t_gen_2_refusals() {
    for t in ["K1ABC/ABCD W9XYZ", "THIS IS TWENTY CHARS"] {
        assert!(encode::audio(
            &encode::Transmission {
                sub: Sub::new(Mode::Ft8),
                hz: 1000.0,
                amp: 0.5,
                mirrored: false
            },
            t,
            48000.0,
        )
        .is_err());
        assert!(WsjtGenerator::single(
            Sub::new(Mode::Ft8),
            48000.0,
            t,
            1500.0,
        )
        .is_err());
    }
    assert!(!encode::offered_for_transmit(Mode::Q65));
    assert!(encode::offered_for_transmit(Mode::Ft8));
    assert!(encode::offered_for_transmit(Mode::Js8));
    assert!(!encode::offered_for_transmit(Mode::Fst4));
}

#[test]
fn descriptors_make_receivers_and_generators() {
    let d = descriptors();
    assert_eq!(d.len(), 17);
    let ids: Vec<&str> = d.iter().map(|x| x.id).collect();
    for id in [
        "wsjt", "ft8", "ft4", "ft2", "wspr", "jt65", "jt9", "jt4", "q65", "fst4", "fst4w",
        "msk144", "js8", "fsk441", "jt6m", "iscat", "pi4",
    ] {
        assert!(ids.contains(&id), "{id}");
    }
    for x in d {
        assert_eq!(x.group, "Weak-signal digital");
        assert_eq!(x.tuning.tune_offset_hz, 20e3);
        let settings = Values::defaults(&(x.settings)());
        let mut rx = (x.make_receiver)(&StackConfig::new(settings.clone())).unwrap();
        assert!(rx.configure(&InputInfo::new(4000.0, 14e6, 14e6)).is_err());
        rx.configure(&InputInfo::new(48000.0, 14e6, 14e6 - 20e3))
            .unwrap();
        let mut out = Output::default();
        rx.feed(&vec![Complex32::default(); 4800], &mut out);
        let mut g =
            (x.make_generator.unwrap())(&GenConfig::new(48000.0, settings).with_seed(1)).unwrap();
        let mut buf = vec![Complex32::default(); 480];
        assert_eq!(g.generate(&mut buf), 480);
    }
}

#[test]
fn the_combined_mode_decodes_ft8_and_ft4_at_once() {
    let a = audio_of(
        Sub::new(Mode::Ft8),
        &[("CQ K1ABC FN42", 800.0, -8.0, 0.0, false)],
        16.0,
        true,
        88,
    );
    let b = audio_of(
        Sub::new(Mode::Ft4),
        &[("CQ DL1XYZ JO62", 2000.0, -8.0, 0.0, false)],
        16.0,
        false,
        89,
    );
    let mix: Vec<f32> = a.iter().zip(&b).map(|(x, y)| x + y).collect();
    let mut rx = receiver(None, &[], 8000.0, 0.0);
    let got = texts(&feed(&mut rx, &iq_of(&mix), 800));
    assert!(
        got.contains(&"CQ K1ABC FN42".to_string()) && got.contains(&"CQ DL1XYZ JO62".to_string()),
        "{got:?}"
    );
    assert!(rx.results().status().summary.contains("FT8"));
}

#[test]
fn contact_lines_of_the_map() {
    let mut a = dec(0);
    a.text = "K1ABC W9XYZ -12".into();
    let mut b = dec(1);
    b.text = "W9XYZ K1ABC R-05".into();
    let c = dec(2);
    let lines = sdrkit_stack_wsjt::table::contact_lines([&a, &b, &c], 1_767_225_700.0, 10.0);
    assert_eq!(lines, vec![("K1ABC".to_string(), "W9XYZ".to_string())]);
}

#[test]
fn clear_empties_the_hash_tables_of_a_decoder() {
    let mut st = sdrkit_stack_wsjt::runner::DecodeState::default();
    st.calls.learn("PJ4/K1ABC");
    st.wspr.learn("K1ABC");
    assert!(!st.calls.is_empty());
    st.clear();
    assert!(
        st.calls.is_empty()
            && st
                .wspr
                .get(sdrkit_stack_wsjt::proto::wsprmsg::hash15("K1ABC"))
                .is_none()
    );
}

#[test]
fn a_verification_case_asks_noise_only_of_a_signal_that_states_its_bandwidth() {
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 3.0,
        clean: false,
        snr_db: 30.0,
    };
    for d in sdrkit_stack_wsjt::descriptors() {
        let g = (d.make_generator.expect("a generator"))(
            &GenConfig::new(48_000.0, Values::defaults(&(d.settings)())).with_seed(1),
        )
        .unwrap();
        for c in (d.verify.expect("the hooks").cases)(&o) {
            if c.snr_db.is_some() {
                assert!(
                    g.signal_info().is_some(),
                    "{}: {} adds noise to a signal without a bandwidth",
                    d.id,
                    c.name
                );
            }
        }
    }
}

#[test]
fn a_recorded_vector_runs_the_combined_receiver_with_only_its_own_mode() {
    let hooks = sdrkit_stack_wsjt::WSJT
        .verify
        .expect("the hooks of the combined mode");
    let section = serde_json::json!({"receiver": {"mode": "fst4", "period_s": 60}});
    let s =
        (hooks.vector_settings.expect("vector settings"))("wsjt_fst4_60_8k", &section, &section);
    let get = |k: &str| s.iter().find(|c| c.key == k).map(|c| c.value.clone());
    assert_eq!(get("enable_fst4"), Some(Value::Bool(true)));
    for other in [
        "enable_ft8",
        "enable_jt65",
        "enable_jt4",
        "enable_fst4w",
        "js8_normal",
    ] {
        assert_eq!(get(other), Some(Value::Bool(false)), "{other}");
    }
}
