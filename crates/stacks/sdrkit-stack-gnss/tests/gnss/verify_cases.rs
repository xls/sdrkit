//! The verify cases of both stacks, run in process: the stack's test signal with the noise of its sky, into the stack's
//! receiver, then the checks of each case on the results.

use sdrkit_core::verify::{CaseSetting, Outcome, VerifyOptions};
use sdrkit_core::{Complex32, InputInfo, Output, StackConfig, Values};
use sdrkit_stack_gnss::gen::GnssGenerator;

/// The settings of a case: the defaults of `desc`, with the case's settings applied.
fn values(desc: &sdrkit_core::StackDescriptor, case: &[CaseSetting]) -> Values {
    let schema = (desc.settings)();
    let mut v = Values::defaults(&schema);
    CaseSetting::apply_all(case, &mut v).unwrap();
    v
}

#[test]
fn each_verify_case_passes_in_process() {
    let o = VerifyOptions {
        seconds: 1.0,
        clean: true,
        snr_db: 30.0,
    };
    for desc in sdrkit_stack_gnss::descriptors() {
        let Some(hooks) = desc.verify else {
            continue;
        };
        // NavIC has a generator of its own and its own test.
        if desc.id == sdrkit_stack_gnss::navic::ID {
            continue;
        }
        for case in (hooks.cases)(&o) {
            let v = values(desc, &case.settings);
            let rate = desc.tuning.rate_hz;
            let sbas = desc.id == sdrkit_stack_gnss::SBAS.id;
            let mut g = GnssGenerator::new(sdrkit_stack_gnss::settings::gen_options(
                &v, rate, 1, sbas, None,
            ));
            let mut sky = crate::Sky::new(&g);
            let mut rx = (desc.make_receiver)(&StackConfig::new(v)).unwrap();
            let f = desc.tuning.default_hz;
            rx.configure(&InputInfo::new(rate, f, f).recording())
                .unwrap();
            let mut x = vec![Complex32::new(0.0, 0.0); 65_536];
            let mut out = Output::default();
            for _ in 0..(case.seconds * rate) as usize / x.len() {
                sky.render(&mut g, &mut x);
                rx.feed(&x, &mut out);
                out.clear();
            }
            rx.finish(&mut out);
            for line in (case.check.unwrap())(rx.results()) {
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
