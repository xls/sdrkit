//! Pulse trains: units, despike and sliver folding.

use crate::common::{pairs_us, train};
use sdrkit_proto_ookdev::{Modulation, Pulse, Train};

#[test]
fn units_and_last_space() {
    let t = train(Modulation::Ook, 250e3, &[(400.0, 800.0), (400.0, 1e4)]);
    assert_eq!(t.pulses[0], Pulse::new(100.0, 200.0));
    assert_eq!(t.us(100.0), 400.0);
    assert_eq!(t.samples(400.0), 100.0);
    assert_eq!(t.space_at(0), 200.0);
    assert_eq!(t.space_at(1), f32::INFINITY);
    let mut f = t.clone();
    f.modulation = Modulation::Fsk;
    assert_eq!(f.space_at(1), 2500.0);
    assert_eq!(t.len_samples(), 100.0 + 200.0 + 100.0 + 2500.0);
}

#[test]
fn despike_merges_inner_runs() {
    let mut t = train(
        Modulation::Ook,
        1e6,
        &[(100.0, 5.0), (100.0, 100.0), (100.0, 1e6)],
    );
    t.despike(10.0);
    assert_eq!(pairs_us(&t), vec![(205.0, 100.0), (100.0, 1e6)]);
    // A short space inside and a short mark inside, merged in one pass.
    let mut t = train(
        Modulation::Ook,
        1e6,
        &[(100.0, 100.0), (3.0, 100.0), (100.0, 2.0), (100.0, 50.0)],
    );
    t.despike(10.0);
    assert_eq!(pairs_us(&t), vec![(100.0, 203.0), (202.0, 50.0)]);
    // The first mark and the last space are kept even when short.
    let mut t = train(Modulation::Ook, 1e6, &[(3.0, 100.0), (100.0, 4.0)]);
    t.despike(10.0);
    assert_eq!(pairs_us(&t), vec![(3.0, 100.0), (100.0, 4.0)]);
}

#[test]
fn fold_slivers_learns_the_limit() {
    // 500 µs runs with a 4 µs sliver in a space and a 3 µs mark at the start.
    let mut t = train(
        Modulation::Ook,
        1e6,
        &[
            (3.0, 500.0),
            (500.0, 500.0),
            (500.0, 248.0),
            (4.0, 248.0),
            (500.0, 500.0),
            (500.0, 9000.0),
        ],
    );
    let mut scratch = Vec::new();
    t.fold_slivers(2.5, &mut scratch);
    assert_eq!(
        pairs_us(&t),
        vec![
            (500.0, 500.0),
            (500.0, 500.0),
            (500.0, 500.0),
            (500.0, 9000.0)
        ]
    );
    // Short trains are left alone.
    let mut s = train(Modulation::Ook, 1e6, &[(3.0, 500.0), (500.0, 10.0)]);
    let before = s.clone();
    s.fold_slivers(2.5, &mut scratch);
    assert_eq!(s, before);
}

#[test]
fn validity() {
    let mut t = Train::from_us(Modulation::Ook, 1e6, &[(1.0, 1.0)]);
    assert!(t.is_valid());
    t.pulses[0].space = f32::INFINITY;
    assert!(!t.is_valid());
    assert!(!Train::new(Modulation::Ook, 0.0).is_valid());
}
