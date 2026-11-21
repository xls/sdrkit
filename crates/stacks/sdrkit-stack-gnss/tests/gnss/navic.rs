//! NavIC: two satellites at 42 dB-Hz with opposite Doppler, tracked and decoded within 34 s, and nothing on noise alone.

use sdrkit_core::{Complex32, Generator, InputInfo, LockState, Output, StackConfig, Values};
use sdrkit_stack_gnss::navic::{self, code, gen};

fn run(
    sats: Vec<gen::GenSat>,
    seconds: f64,
    cn0_dbhz: Option<f64>,
) -> Box<dyn sdrkit_core::Receiver> {
    let rate = navic::RATE_HZ;
    let mut g = gen::NavicGenerator::new(rate, sats, 1);
    let noise = cn0_dbhz
        .map(|c| sdrkit_algo_sim::noise::Awgn::with_power(rate / 10f64.powf(c / 10.0), 5).unwrap());
    let mut noise = noise;
    let v = Values::defaults(&(navic::DESCRIPTOR.settings)());
    let mut rx = (navic::DESCRIPTOR.make_receiver)(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, code::L5_HZ, code::L5_HZ).recording())
        .unwrap();
    let mut x = vec![Complex32::default(); 65_536];
    let mut out = Output::default();
    for _ in 0..(seconds * rate) as usize / x.len() {
        g.generate(&mut x);
        if let Some(n) = noise.as_mut() {
            n.process(&mut x);
        }
        rx.feed(&x, &mut out);
        out.clear();
    }
    rx
}

fn rows(r: &sdrkit_core::Results) -> Vec<(i64, i64, i64)> {
    let decl = r.decl();
    let Some(t) = decl.table_id("satellites").and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first()
        .map(|(_, row)| {
            let get = |k: &str| match row.decl().id(k).and_then(|i| row.value(i)) {
                Some(sdrkit_core::Value::I64(v)) => *v,
                _ => -1,
            };
            (get("prn"), get("subframes_ok"), get("towc"))
        })
        .collect()
}

#[test]
fn two_satellites_are_tracked_and_decoded() {
    let sats = vec![
        gen::GenSat {
            prn: 2,
            doppler_hz: 2800.0,
            code_start: 37.5,
            towc: gen::TOWC0,
            amplitude: 1.0,
        },
        gen::GenSat {
            prn: 5,
            doppler_hz: -1400.0,
            code_start: 75.0,
            towc: gen::TOWC0 + 1,
            amplitude: 1.0,
        },
    ];
    let rx = run(sats, 34.0, Some(42.0));
    let r = rx.results();
    let mut got = rows(r);
    got.sort();
    assert_eq!(
        got.iter().map(|g| g.0).collect::<Vec<_>>(),
        [2, 5],
        "{:?}",
        r.log_lines()
    );
    for (prn, ok, towc) in &got {
        assert!(*ok >= 1, "PRN {prn}: {ok} subframes; {:?}", r.log_lines());
        assert!(*towc >= gen::TOWC0 as i64, "PRN {prn}: TOWC {towc}");
    }
    assert_eq!(r.status().lock_state, LockState::Locked);
    let note = r
        .decl()
        .field_id("note")
        .and_then(|id| r.fields().value(id))
        .map(|v| format!("{v:?}"))
        .unwrap_or_default();
    assert!(note.contains("not confirmed by a public document"));
}

#[test]
fn noise_alone_gives_nothing() {
    let rx = run(Vec::new(), 4.0, Some(42.0));
    assert!(rows(rx.results()).is_empty());
    assert_ne!(rx.results().status().lock_state, LockState::Locked);
}

/// The data file holds the dial of the stack's section of the specification: the L5 carrier of the ICD.
#[test]
fn the_dial_file_has_its_row() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/gnss/navic_dials.csv");
    let text = std::fs::read_to_string(path).expect("the data file");
    let rows: Vec<&str> = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .collect();
    assert_eq!(rows.len(), 1);
    let hz: f64 = rows[0].split(',').nth(1).unwrap().parse().unwrap();
    assert_eq!(hz, code::L5_HZ);
}

/// The generator of the descriptor at its rate with the default settings: the satellites are tracked within 8 s.
#[test]
fn the_default_test_signal_is_tracked() {
    let rate = navic::RATE_HZ;
    let v = Values::defaults(&(navic::DESCRIPTOR.settings)());
    let mut g =
        (navic::DESCRIPTOR.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, v.clone()))
            .unwrap();
    let mut rx = (navic::DESCRIPTOR.make_receiver)(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, code::L5_HZ, code::L5_HZ))
        .unwrap();
    let mut x = vec![Complex32::default(); 16_384];
    let mut out = Output::default();
    for _ in 0..(8.0 * rate) as usize / x.len() {
        g.generate(&mut x);
        rx.feed(&x, &mut out);
        out.clear();
    }
    let got = rows(rx.results());
    assert!(got.len() >= 3, "{got:?} {:?}", rx.results().log_lines());
    assert_eq!(rx.results().status().lock_state, LockState::Tracking);
}

/// `clear` empties the satellite table.
#[test]
fn clear_empties_the_table() {
    let sats = vec![gen::GenSat {
        prn: 9,
        doppler_hz: 1000.0,
        code_start: 10.0,
        towc: gen::TOWC0,
        amplitude: 1.0,
    }];
    let mut rx = run(sats, 3.0, Some(45.0));
    assert!(!rows(rx.results()).is_empty());
    let commands = rx.commands();
    let id = commands.id("clear").unwrap();
    let args = commands.defs()[id.index()].new_args();
    sdrkit_core::owner::invoke(rx.as_mut(), id, &args).unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); 4096], &mut out);
    let left: Vec<_> = rows(rx.results()).into_iter().filter(|r| r.1 > 0).collect();
    assert!(left.is_empty(), "{left:?}");
}
