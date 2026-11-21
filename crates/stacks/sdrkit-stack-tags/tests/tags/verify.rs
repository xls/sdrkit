//! The verify case of each stack, run in process: the test signal with the noise of the case into the receiver, then the
//! checks of the case on the results.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::verify::{Outcome, VerifyOptions};
use sdrkit_dev_test::Harness;

#[test]
fn each_verify_case_passes_in_process() {
    for clean in [true, false] {
        let o = VerifyOptions {
            seconds: 1.0,
            clean,
            snr_db: 20.0,
        };
        for desc in sdrkit_stack_tags::descriptors() {
            let hooks = desc.verify.expect("a verify case");
            for case in (hooks.cases)(&o) {
                let mut h = Harness::new(desc)
                    .with_settings(&case.settings)
                    .seconds(case.seconds);
                if let Some(snr) = case.snr_db {
                    h = h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, snr);
                }
                let r = h.run().expect("the run");
                for line in (case.check.expect("checks"))(&r.results) {
                    assert_eq!(
                        line.outcome,
                        Outcome::Pass,
                        "{} {}: {} {}",
                        desc.id,
                        case.name,
                        line.key,
                        line.detail
                    );
                }
            }
        }
    }
}
