//! The same input cut into pieces of 1, 7, 4096, 65 536 samples and a ragged mixture produces the same channel table,
//! states, search work and report numbers (the receiver works in fixed pieces of 16 384 samples).

use sdrkit_core::{Complex32, InputInfo, Output, StackConfig, Value, Values};
use sdrkit_stack_gnss::core::{CoreConfig, GnssCore, GnssReport};
use sdrkit_stack_gnss::gen::{GenOptions, GnssGenerator};
use sdrkit_stack_gnss::system::System;

/// The report and frame counts after `x`, fed in pieces of `cut(k)` samples, as text (a NaN equals itself there).
fn run_core(x: &[Complex32], fs: f64, cut: &dyn Fn(usize) -> usize) -> (GnssReport, String) {
    let mut rx = GnssCore::new(CoreConfig {
        systems: System::Gps.bit() | System::Sbas.bit() | System::Qzss.bit(),
        week_ref: 2400,
        ..Default::default()
    });
    rx.configure(fs, 1575.42e6, false).unwrap();
    let mut i = 0;
    let mut k = 0;
    while i < x.len() {
        let n = cut(k).min(x.len() - i);
        rx.feed(&x[i..i + n]);
        i += n;
        k += 1;
    }
    let mut d = rx.last_report().clone();
    d.acq_plot = None;
    let text = format!("{:?}", (&d, rx.frame_counts()));
    (d, text)
}

#[test]
fn cuts_do_not_matter() {
    let fs = 2.5e6;
    let mut g = GnssGenerator::new(GenOptions {
        rate_hz: fs,
        max_sats: Some(6),
        ..Default::default()
    });
    let mut sky = crate::Sky::new(&g);
    let mut x = vec![Complex32::new(0.0, 0.0); (5.0 * fs) as usize];
    sky.render(&mut g, &mut x);
    let a = run_core(&x, fs, &|_| 65_536);
    assert_eq!(a.0.state, 1, "nothing locked in 5 s");
    for (name, cut) in [
        ("1", Box::new(|_| 1usize) as Box<dyn Fn(usize) -> usize>),
        ("7", Box::new(|_| 7usize)),
        ("4096", Box::new(|_| 4096usize)),
        (
            "ragged",
            Box::new(|k: usize| [1usize, 333, 16_384, 70_001, 5, 4097][k % 6]),
        ),
    ] {
        let b = run_core(&x, fs, &cut);
        assert_eq!(a.1, b.1, "cut {name}");
    }
    // The stack's graph: block sizes of 4096 and 65 536 give the same satellite table.
    let table = |n: usize| {
        let desc = &sdrkit_stack_gnss::GNSS;
        let schema = (desc.settings)();
        let mut v = Values::defaults(&schema);
        v.set(schema.id("known_week").unwrap(), &Value::I64(2400))
            .unwrap();
        let mut rx = (desc.make_receiver)(&StackConfig::new(v)).unwrap();
        rx.configure(&InputInfo::new(fs, 1575.42e6, 1575.42e6).recording())
            .unwrap();
        let mut out = Output::default();
        for c in x.chunks(n) {
            rx.feed(c, &mut out);
            out.clear();
        }
        let r = rx.results();
        serde_json::to_string(&r.table(r.decl().table_id("satellites").unwrap())).unwrap()
    };
    assert_eq!(table(4096), table(65_536));
}
