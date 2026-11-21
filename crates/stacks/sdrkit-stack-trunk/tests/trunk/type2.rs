//! Motorola Type II: the word code, the plan files, the messages of the worked examples, and the test scene through the
//! receiver over carriers, clocks, polarity, deviation, bad words, rates and noise.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_core::{
    Facilities, Generator, HostRequest, InputInfo, LockState, MemStorage, Receiver, Storage,
    StorePath, Value,
};
use sdrkit_dev_test::{ClockError, FrequencyOffset};
use sdrkit_stack_trunk::type2::code::{encode, Osw, WordDecoder};
use sdrkit_stack_trunk::type2::gen::{scene, Type2GenOptions, Type2Generator};
use sdrkit_stack_trunk::type2::msg::{Meaning, MessageLayer};
use sdrkit_stack_trunk::type2::plan::{ChannelPlan, Channels};
use sdrkit_stack_trunk::typed::{
    MOTOROLA_TYPE2_CALLS, MOTOROLA_TYPE2_MESSAGES, MOTOROLA_TYPE2_TALKGROUPS,
};
use sdrkit_stack_trunk::MOTOROLA_TYPE2;
use std::sync::Arc;

/// The control channel of the test plan, Hz.
const CC_HZ: f64 = 851.5125e6;

const PLANS: [(&str, usize); 7] = [
    ("type2_800_domestic.json", 5),
    ("type2_800_rebanded.json", 6),
    ("type2_800_splinter.json", 6),
    ("type2_800_international.json", 1),
    ("type2_800_international_splinter.json", 1),
    ("type2_900.json", 1),
    ("type2_obt_example.json", 1),
];

fn plan_text(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data/trunk")
        .join(name);
    std::fs::read_to_string(path).expect("the data file")
}

fn plan(name: &str) -> ChannelPlan {
    ChannelPlan::parse(&plan_text(name)).expect("a plan")
}

/// A storage that holds every plan file of the trunk family under `plans/`.
fn storage() -> Arc<MemStorage> {
    let s = MemStorage::new();
    for (name, _) in PLANS {
        s.write(
            &StorePath::new(&format!("plans/{name}")).unwrap(),
            plan_text(name).as_bytes(),
        )
        .unwrap();
    }
    Arc::new(s)
}

/// The receiver with the domestic plan and `settings`.
fn receiver(settings: &[(&str, &str)]) -> Box<dyn Receiver> {
    let mut all = vec![("plan", "plans/type2_800_domestic.json")];
    all.extend_from_slice(settings);
    let mut rx = make(&MOTOROLA_TYPE2, &all);
    rx.set_facilities(&Facilities {
        storage: storage(),
        ..Facilities::default()
    });
    rx
}

fn options(features: u8) -> Type2GenOptions {
    Type2GenOptions {
        channel_hz: 0.0,
        deviation_hz: 1200.0,
        invert: false,
        word_errors: 0.0,
        idle_inside: false,
        features,
        calls: scene(),
        seed: 1,
    }
}

/// `seconds` of the scene at `rate` with the control channel at the centre.
fn signal(rate: f64, seconds: f64, opt: Type2GenOptions) -> Vec<Complex32> {
    let mut g = Type2Generator::new(rate, opt);
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * seconds) as usize];
    g.generate(&mut x);
    x
}

/// Feeds `x` to `rx` with the control channel at the centre, tuned to the test plan's control channel.
fn run(rx: &mut dyn Receiver, x: &[Complex32], rate: f64) -> Fed {
    configure(rx, rate, CC_HZ, 0.0);
    let fed = feed(rx, x, 16384);
    let mut out = sdrkit_core::Output::default();
    rx.finish(&mut out);
    fed
}

fn talkgroups(rx: &dyn Receiver) -> Vec<std::collections::BTreeMap<String, String>> {
    rows(results(rx), &MOTOROLA_TYPE2_TALKGROUPS)
}

/// The checks of the scene with the domestic plan.
fn check_scene(rx: &dyn Receiver, what: &str, smartzone: bool) {
    let r = results(rx);
    assert!(
        num(r, "good_words") > 180.0,
        "{what}: {} good words",
        num(r, "good_words")
    );
    assert_eq!(text(r, "system"), "4A2C", "{what}");
    assert_eq!(num(r, "site_id"), 3.0, "{what}");
    assert_eq!(text(r, "system_type"), "Type II", "{what}");
    assert_eq!(
        text(r, "network"),
        if smartzone { "SmartZone" } else { "SmartNet" },
        "{what}"
    );
    let tgs = talkgroups(rx);
    assert!(tgs.len() >= 3, "{what}: {tgs:?}");
    for (tg, freq) in [("1230", 851.6125e6), ("4560", 851.6625e6)] {
        let row = tgs
            .iter()
            .find(|r| r["talkgroup"] == tg)
            .unwrap_or_else(|| panic!("{what}: {tg}"));
        assert_eq!(row["freq_hz"].parse::<f64>().unwrap(), freq, "{what}: {tg}");
    }
    let enc = tgs.iter().find(|r| r["talkgroup"] == "7770").expect("7770");
    assert_eq!(enc["encrypted"], "yes", "{what}");
}

#[test]
fn round_trip_repair_and_false_words() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    let mut word = || {
        let v = rng.next_u64();
        Osw::new(v as u16, v >> 16 & 1 == 1, (v >> 17) as u16 & 0x3FF)
    };
    let mut dec = WordDecoder::default();
    for _ in 0..2000 {
        let w = word();
        let d = dec.decode_bits(&encode(w)[8..]).unwrap();
        assert_eq!((d.osw, d.repaired), (w, 0));
    }
    // One wrong information bit: its place in the sent payload is pair bit 2k.
    let mut good = 0;
    for i in 0..400 {
        let w = word();
        let mut b = encode(w);
        let k = i % 36;
        let r = 2 * k;
        b[8 + (r % 4) * 19 + r / 4] ^= 1;
        if dec
            .decode_bits(&b[8..])
            .is_some_and(|d| d.osw == w && d.repaired == 1)
        {
            good += 1;
        }
    }
    assert!(good >= 390, "{good}");
    let mut rng2 = sdrkit_algo_sim::rng::Rng::new(11);
    let mut wrong = 0;
    for _ in 0..3000 {
        let w = word();
        let mut b = encode(w);
        for _ in 0..6 {
            let k = 8 + (rng2.next_u64() % 76) as usize;
            b[k] ^= 1;
        }
        if dec.decode_bits(&b[8..]).is_some_and(|d| d.osw != w) {
            wrong += 1;
        }
    }
    assert!(wrong < 12, "{wrong}");
}

#[test]
fn the_plan_files() {
    for (name, rows) in PLANS {
        assert_eq!(plan(name).rows.len(), rows, "{name}");
    }
    let f = |name: &str, n: u16| plan(name).rx(n).map(|(f, _)| f);
    assert_eq!(f("type2_800_domestic.json", 0x014), Some(851.5125e6));
    assert_eq!(f("type2_800_domestic.json", 0x2D0), Some(866.0e6));
    assert_eq!(f("type2_800_domestic.json", 0x300), None);
    assert_eq!(f("type2_800_rebanded.json", 0x1B8), Some(851.025e6));
    assert_eq!(f("type2_800_splinter.json", 0x258), Some(866.0125e6));
    assert_eq!(f("type2_800_international.json", 0x020), Some(851.8125e6));
    assert_eq!(
        f("type2_800_international_splinter.json", 0x020),
        Some(851.8e6)
    );
    assert_eq!(f("type2_900.json", 0x002), Some(935.0375e6));
    let obt = plan("type2_obt_example.json");
    assert_eq!(obt.rx(400).map(|(f, _)| f), Some(460.25e6));
    assert!(obt.is_tx(10));
    assert!(!plan("type2_800_domestic.json").is_tx(10));
}

/// The messages of `words` with `plan`.
fn messages(words: &[Option<Osw>], plan: &ChannelPlan) -> Vec<(Meaning, String)> {
    let mut l = MessageLayer::default();
    for w in words {
        match w {
            Some(w) => l.word(*w),
            None => l.lost(),
        }
    }
    let mut out = Vec::new();
    l.read(Channels(Some(plan)), true, |m| {
        out.push((m.meaning, m.text))
    });
    out.retain(|(m, _)| *m != Meaning::Idle);
    out
}

#[test]
fn the_worked_messages() {
    let dom = plan("type2_800_domestic.json");
    let w = |a: u16, g: bool, c: u16| Some(Osw::new(a, g, c));
    let grant = |m: &Meaning| match m {
        Meaning::Grant(g) => *g,
        other => panic!("{other:?}"),
    };
    let m = messages(&[w(12345, true, 0x308), w(0x1232, true, 0x014)], &dom);
    let g = grant(&m[0].0);
    assert_eq!(
        (g.target, g.source, g.channel, g.emergency, g.digital),
        (0x1230, 12345, 0x014, true, Some(false))
    );
    assert_eq!(dom.rx(g.channel).unwrap().0, 851.5125e6);
    let m = messages(&[w(555, true, 0x321), w(0x4568, true, 0x020)], &dom);
    let g = grant(&m[0].0);
    assert_eq!(
        (g.target, g.encrypted, g.digital),
        (0x4560, true, Some(true))
    );
    let g = grant(&messages(&[w(0x4560, true, 0x020)], &dom)[0].0);
    assert!(g.update && g.target == 0x4560);
    let g = grant(&messages(&[w(111, false, 0x308), w(222, false, 0x030)], &dom)[0].0);
    assert_eq!(
        (g.target, g.source, g.group, g.interconnect),
        (111, 222, false, false)
    );
    let g = grant(&messages(&[w(0, false, 0x308), w(222, false, 0x030)], &dom)[0].0);
    assert!(g.interconnect && g.source == 222);
    let m = messages(&[w(0x4A2C, true, 0x308), w(0x1F12, false, 0x014)], &dom);
    assert_eq!(
        m[0].0,
        Meaning::ControlChannel {
            channel: 0x014,
            system: 0x4A2C,
            adjacent: None
        }
    );
    let site = [
        w(0x4A2C, true, 0x308),
        w((2 << 10) | (3 << 7) | 0x1F, true, 0x320),
        w(0x6014, false, 0x30B),
    ];
    for words in [
        site.to_vec(),
        vec![site[0], site[1], w(0, false, 0x2F8), site[2]],
    ] {
        match &messages(&words, &dom)[0].0 {
            Meaning::Site(s) => assert_eq!(
                (s.system, s.site, s.band, s.features, s.channel, s.adjacent),
                (0x4A2C, 3, 3, 0x1F, 0x014, false)
            ),
            other => panic!("{other:?}"),
        }
    }
    let m = messages(&[w(0x1233, true, 0x308), w(0x0456, true, 0x340)], &dom);
    assert_eq!(
        m[0].0,
        Meaning::Patch {
            talkgroup: 0x1230,
            member: 0x4560,
            multiselect: false
        }
    );
    let m = messages(
        &[
            w((26 << 9) | (3 << 5) | 14, true, 0x308),
            w((1 << 13) | (17 << 8) | 45, true, 0x322),
        ],
        &dom,
    );
    assert!(m[0].1.contains("2026-03-14 17:45"), "{}", m[0].1);
    let m = messages(&[w((1 << 13) | 0x1000 | 5, false, 0x3C0)], &dom);
    assert_eq!(m[0].1, "system status: Type II");
    let obt = plan("type2_obt_example.json");
    let g = grant(&messages(&[w(777, true, 100), w(0x1230, true, 400)], &obt)[0].0);
    assert_eq!(
        (g.target, g.source, g.digital, g.channel),
        (0x1230, 777, Some(false), 400)
    );
    assert_eq!(obt.rx(400).unwrap().0, 460.25e6);
    let m = messages(&[w(12345, true, 0x308), None, w(0x1230, true, 0x014)], &dom);
    assert!(
        m.iter().all(|(m, _)| !matches!(m, Meaning::Grant(_))),
        "{m:?}"
    );
}

#[test]
fn the_scene_with_the_plan_and_following() {
    let rate = 480e3;
    let x = signal(rate, 6.0, options(0x1F));
    let mut rx = receiver(&[("follow", "true")]);
    let fed = run(rx.as_mut(), &x, rate);
    check_scene(rx.as_ref(), "480 ksps", true);
    let follows: Vec<f64> = fed
        .requests
        .iter()
        .filter_map(|r| match r {
            HostRequest::Follow { freq_hz, .. } => Some(*freq_hz),
            _ => None,
        })
        .collect();
    assert_eq!(follows, [851.6125e6, 851.6625e6], "{:?}", fed.requests);
    assert!(fed
        .requests
        .iter()
        .any(|r| matches!(r, HostRequest::Release { freq_hz } if *freq_hz == 851.6125e6)));
    let skipped: Vec<_> = fed
        .events
        .iter()
        .filter(|e| e.kind() == "trunk.follow" && field(e, "target") == "7770")
        .collect();
    assert!(skipped
        .iter()
        .all(|e| field(e, "action") == "skipped" && field(e, "reason") == "encrypted"));
    assert!(!skipped.is_empty());
    let calls = rows(results(rx.as_ref()), &MOTOROLA_TYPE2_CALLS);
    assert!(calls
        .iter()
        .any(|c| c["talkgroup"] == "7770" && c["encrypted"] == "yes"));
    assert!(!rows(results(rx.as_ref()), &MOTOROLA_TYPE2_MESSAGES).is_empty());
    check_events(
        &kinds(&sdrkit_stack_trunk::type2::declaration()),
        &fed.events,
        true,
    );
    assert!(text(results(rx.as_ref()), "note").contains("Not confirmed by a public document"));
}

/// A case: its name, the rate, the change of the generator options, the carrier offset (Hz) and the clock error (ppm).
type Case = (
    &'static str,
    f64,
    Box<dyn Fn(&mut Type2GenOptions)>,
    f64,
    f64,
);

#[test]
fn the_scene_under_impairments() {
    let base = 480e3;
    let cases: Vec<Case> = vec![
        ("+900 Hz", base, Box::new(|_| {}), 900.0, 0.0),
        ("-1500 Hz", base, Box::new(|_| {}), -1500.0, 0.0),
        ("inverted", base, Box::new(|o| o.invert = true), 0.0, 0.0),
        ("+150 ppm", base, Box::new(|_| {}), 0.0, 150.0),
        ("-300 ppm", base, Box::new(|_| {}), 0.0, -300.0),
        (
            "deviation 700 Hz",
            base,
            Box::new(|o| o.deviation_hz = 700.0),
            0.0,
            0.0,
        ),
        (
            "5 % bad words",
            base,
            Box::new(|o| o.word_errors = 0.05),
            0.0,
            0.0,
        ),
        (
            "an idle inside",
            base,
            Box::new(|o| o.idle_inside = true),
            0.0,
            0.0,
        ),
        ("1.92 Msps", 1.92e6, Box::new(|_| {}), 0.0, 0.0),
        ("600 ksps", 600e3, Box::new(|_| {}), 0.0, 0.0),
    ];
    for (what, rate, change, offset, ppm) in cases {
        let mut opt = options(0x1F);
        change(&mut opt);
        let x = signal(rate, 6.0, opt);
        let mut y = Vec::new();
        ClockError::new(rate, ppm).process(&x, &mut y);
        FrequencyOffset::new(rate, offset).process(&mut y);
        let mut rx = receiver(&[]);
        run(rx.as_mut(), &y, rate);
        check_scene(rx.as_ref(), what, true);
        if what == "inverted" {
            assert_eq!(text(results(rx.as_ref()), "inverted"), "yes");
        }
    }
}

#[test]
fn site_trunking_is_not_smartzone() {
    let rate = 240e3;
    let x = signal(rate, 6.0, options(0x0F));
    let mut rx = receiver(&[]);
    run(rx.as_mut(), &x, rate);
    check_scene(rx.as_ref(), "features 0F", false);
}

#[test]
fn noise() {
    let rate = 240e3;
    let mut lines = Vec::new();
    for sigma in [0.02, 0.05, 0.1, 0.16] {
        let mut x = signal(rate, 6.0, options(0x1F));
        add_noise(&mut x, sigma, 5);
        let mut rx = receiver(&[]);
        run(rx.as_mut(), &x, rate);
        let good = num(results(rx.as_ref()), "good_words");
        lines.push(format!("noise sigma {sigma}: {good} good words"));
        if sigma <= 0.05 {
            assert!(good > 150.0, "{lines:?}");
        }
    }
    println!("{}", lines.join("\n"));
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 6.0) as usize];
    add_noise(&mut x, 0.05, 6);
    let mut rx = receiver(&[]);
    let fed = run(rx.as_mut(), &x, rate);
    let r = results(rx.as_ref());
    assert_eq!(num(r, "good_words"), 0.0);
    assert_ne!(r.status().lock_state, LockState::Locked);
    assert!(fed.events.is_empty());
}

#[test]
fn without_a_plan_grants_have_no_frequency() {
    let rate = 240e3;
    let x = signal(rate, 6.0, options(0x1F));
    let mut rx = make(&MOTOROLA_TYPE2, &[("follow", "true")]);
    let fed = run(rx.as_mut(), &x, rate);
    let r = results(rx.as_ref());
    assert!(
        r.status().summary.contains("no channel plan"),
        "{}",
        r.status().summary
    );
    assert!(fed.requests.is_empty());
    assert!(talkgroups(rx.as_ref())
        .iter()
        .all(|t| !t.contains_key("freq_hz") || t["freq_hz"].is_empty()));
}

#[test]
fn the_test_signal_locks_with_the_default_settings() {
    let d = &MOTOROLA_TYPE2;
    let rate = d.tuning.rate_hz;
    let v = sdrkit_dev_test::Harness::new(d).values().unwrap();
    let mut g = (d.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, v)).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 3.0) as usize];
    g.generate(&mut x);
    let mut rx = make(d, &[]);
    configure(rx.as_mut(), rate, d.tuning.default_hz + 100e3, -100e3);
    feed(rx.as_mut(), &x, 16384);
    let r = results(rx.as_ref());
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert!(num(r, "messages") > 10.0);
}

#[test]
fn retune_keeps_clear_empties_and_the_state_round_trips() {
    let rate = 240e3;
    let x = signal(rate, 3.0, options(0x1F));
    let mut rx = receiver(&[]);
    run(rx.as_mut(), &x, rate);
    let before = rows(results(rx.as_ref()), &MOTOROLA_TYPE2_MESSAGES).len();
    assert!(before > 0);
    let saved = rx.save_state().expect("a saved state");
    rx.retuned(&InputInfo::new(rate, CC_HZ + 25e3, CC_HZ + 25e3).recording());
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 48_000], 16384);
    assert_eq!(
        rows(results(rx.as_ref()), &MOTOROLA_TYPE2_MESSAGES).len(),
        before
    );
    invoke(rx.as_mut(), "clear");
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 48_000], 16384);
    assert!(rows(results(rx.as_ref()), &MOTOROLA_TYPE2_MESSAGES).is_empty());
    let mut back = receiver(&[]);
    configure(back.as_mut(), rate, CC_HZ, 0.0);
    back.restore_state(&saved).expect("the state is taken");
    feed(
        back.as_mut(),
        &vec![Complex32::new(0.0, 0.0); 48_000],
        16384,
    );
    assert_eq!(
        rows(results(back.as_ref()), &MOTOROLA_TYPE2_MESSAGES).len(),
        before
    );
    assert!(rx.configure(&InputInfo::new(24e3, CC_HZ, CC_HZ)).is_err());
    let _ = Value::Bool(true);
}

/// The share of the words of seconds 1.5 to 4.5 read (the first 1.5 s is the follow-up's settling time) at `snr_db` in 12.5 kHz,
/// with the carrier `offset_hz` from the channel centre and a sample clock `ppm` off.
fn word_share(snr_db: f64, offset_hz: f64, ppm: f64, seed: u64) -> f64 {
    let rate = 96e3;
    let opt = Type2GenOptions {
        calls: Vec::new(),
        ..options(0x1F)
    };
    let x = signal(rate, 4.5, opt);
    let mut y = Vec::new();
    ClockError::new(rate, ppm).process(&x, &mut y);
    FrequencyOffset::new(rate, offset_hz).process(&mut y);
    add_noise_snr(&mut y, rate, snr_db, 12.5e3, seed);
    let split = (1.5 * rate) as usize;
    let mut rx = receiver(&[]);
    configure(rx.as_mut(), rate, CC_HZ, 0.0);
    feed(rx.as_mut(), &y[..split], 16384);
    let first = num(results(rx.as_ref()), "good_words");
    feed(rx.as_mut(), &y[split..], 16384);
    let words = 3.0 * 3600.0 / sdrkit_stack_trunk::type2::code::WORD_BITS as f64;
    (num(results(rx.as_ref()), "good_words") - first) / words
}

#[test]
fn ninety_percent_of_words_in_noise_with_a_carrier_error_and_a_clock_error() {
    // Shares of the words over 8 noise seeds. Asked: 90 % at 3.8 dB (AWGN and a 50 ppm clock) and at 4.5 dB with the carrier
    // +3.1 kHz off. The last line holds the margin: 85 % at 3.0 dB.
    let share = |snr, offset, ppm| {
        (1..=8)
            .map(|s| word_share(snr, offset, ppm, s))
            .sum::<f64>()
            / 8.0
    };
    let mut lines = Vec::new();
    for (what, offset, ppm, snr, want) in [
        ("+3.1 kHz", 3100.0, 0.0, 4.5, 0.9),
        ("awgn", 0.0, 0.0, 3.8, 0.9),
        ("awgn", 0.0, 0.0, 3.0, 0.85),
        ("50 ppm", 0.0, 50.0, 3.8, 0.9),
    ] {
        let got = share(snr, offset, ppm);
        lines.push(format!("{what} {snr} dB: {got:.3}"));
        assert!(got >= want, "{lines:?}");
    }
    eprintln!("{}", lines.join("\n"));
}

#[test]
fn noise_alone_has_no_word() {
    // 60 s of noise at two levels through the soft decoder: no word, no lock.
    let rate = 96e3;
    for (sigma, seed) in [(0.5, 21), (0.02, 22)] {
        let mut x = vec![Complex32::new(0.0, 0.0); (rate * 60.0) as usize];
        add_noise(&mut x, sigma, seed);
        let mut rx = receiver(&[]);
        configure(rx.as_mut(), rate, CC_HZ, 0.0);
        let fed = feed(rx.as_mut(), &x, 16384);
        let r = results(rx.as_ref());
        assert_eq!(num(r, "good_words"), 0.0, "sigma {sigma}");
        assert_ne!(r.status().lock_state, LockState::Locked);
        assert!(fed.events.is_empty());
    }
}
