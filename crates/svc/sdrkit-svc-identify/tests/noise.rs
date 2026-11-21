//! The identifier on plain noise: it reports no signal, at any radio frequency.

use sdrkit_core::Complex32;
use sdrkit_dev_test::Awgn;
use sdrkit_svc_identify::{Identifier, ModClass, Sureness, Verdict};

#[test]
fn noise_is_unknown_also_with_a_frequency() {
    let rate = 1e6;
    let mut x = vec![Complex32::default(); 500_000];
    Awgn::with_sigma(0.01, 3).process(&mut x);
    let ident = Identifier::new();
    for rf in [98e6, 446e6, 1090e6] {
        let id = ident.identify(&x, rate, rf, None);
        assert_eq!(id.verdict, Verdict::Unknown, "{}", id.text());
        assert_eq!(id.sureness, Sureness::None);
    }
}

#[test]
fn every_signal_leaves_out_bumps_of_the_noise() {
    // Plain noise may hold regions a little over the floor; none of them is reported as a signal of its own.
    let rate = 1e6;
    let mut x = vec![Complex32::default(); 500_000];
    Awgn::with_sigma(0.01, 4).process(&mut x);
    let all = Identifier::new().identify_all(&x, rate, 145e6);
    assert!(
        all.iter().all(|id| id.features.class != ModClass::Noise),
        "{:?}",
        all.iter().map(|i| i.text()).collect::<Vec<_>>()
    );
}
