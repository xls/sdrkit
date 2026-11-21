//! The verification cases of the stacks with a test signal at their default settings and frequency: the checks of each
//! case pass. A stack whose default frequency is 0 runs there unless its case names a frequency. (The case of HamDRM has
//! no test signal: its generator needs a format file.)

use sdrkit_core::verify::{Outcome, VerifyOptions};
use sdrkit_core::StackDescriptor;
use sdrkit_dev_test::Harness;

/// Each case of `d` at `rate`, clean, as a recording.
fn cases_pass(d: &'static StackDescriptor, rate: f64) {
    let o = VerifyOptions {
        seconds: 1.0,
        clean: true,
        snr_db: 30.0,
    };
    let hooks = d.verify.expect("verification hooks");
    for case in (hooks.cases)(&o) {
        let run = Harness::new(d)
            .with_settings(&case.settings)
            .rate(rate)
            .freq(case.freq_hz.unwrap_or(d.tuning.default_hz))
            .seconds(case.seconds)
            .run()
            .unwrap_or_else(|e| panic!("{} {}: {e:?}", d.id, case.name));
        let check = case.check.expect("a check");
        for line in check(&run.results) {
            assert_eq!(
                line.outcome,
                Outcome::Pass,
                "{} {}: {}",
                d.id,
                case.name,
                line.detail
            );
        }
    }
}

#[test]
fn every_case_passes_at_the_default_frequency() {
    assert_eq!(sdrkit_stack_broadcast::DRM.tuning.default_hz, 0.0);
    cases_pass(
        &sdrkit_stack_broadcast::DRM,
        sdrkit_stack_broadcast::drm::MIN_RATE,
    );
    cases_pass(
        &sdrkit_stack_broadcast::HDRADIO,
        sdrkit_stack_broadcast::hd::l1::FM_RATE,
    );
    cases_pass(
        &sdrkit_stack_broadcast::CDR,
        sdrkit_stack_broadcast::cdr::MIN_RATE,
    );
    cases_pass(
        &sdrkit_stack_broadcast::CQUAM,
        sdrkit_stack_broadcast::cquam::MIN_RATE,
    );
}
