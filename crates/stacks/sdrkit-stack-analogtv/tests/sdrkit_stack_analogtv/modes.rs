//! The stacks as the framework runs them: the generator through the standard `gen_*` impairments into the receiver, the
//! declared results and events, and the refusal of bad settings.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::owner;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    LockState, ResultsDecl, SettingError, StackConfig, StackDescriptor, Value, Values,
};
use sdrkit_dev_test::Harness;
use sdrkit_stack_analogtv::settings::GEN_TELETEXT;
use sdrkit_stack_analogtv::{ATV, FPV, MAC};

fn declaration(id: &str) -> ResultsDecl {
    sdrkit_stack_analogtv::typed::shape(id).results_decl()
}

/// Runs a stack at 30 dB SNR and checks its results and events against its declaration.
fn check_stack(desc: &'static StackDescriptor, seconds: f64, extra: &[CaseSetting]) {
    let h = Harness::new(desc)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 30.0)
        .with_settings(extra)
        .seconds(seconds);
    let r = h.run().unwrap_or_else(|e| panic!("{}: {e}", desc.id));
    let d = declaration(desc.id);
    assert_eq!(
        r.results.status().lock_state,
        LockState::Locked,
        "{}",
        desc.id
    );
    for (f, v) in r.results.fields().iter() {
        if matches!(v, Value::None) {
            continue;
        }
        assert!(
            d.field_id(&f.key).is_some() || f.key == "bad_samples",
            "{}: {} is not declared",
            desc.id,
            f.key
        );
        if let Value::Hz(x) = v {
            assert!(
                *x != 0.0 || f.key == "offset_hz" || f.key == "sound_spacing",
                "{}: {} is sent as 0",
                desc.id,
                f.key
            );
        }
    }
    assert!(!r.events.is_empty(), "{}: no events", desc.id);
    for e in &r.events {
        assert!(
            d.events().iter().any(|k| k.kind == e.kind()),
            "{}: event {} is not declared",
            desc.id,
            e.kind()
        );
    }
}

#[test]
fn every_stack_publishes_what_it_declares() {
    std::thread::scope(|s| {
        s.spawn(|| check_stack(&ATV, 2.0, &[CaseSetting::new(&GEN_TELETEXT, true)]));
        s.spawn(|| check_stack(&MAC, 1.5, &[]));
        s.spawn(|| check_stack(&FPV, 2.0, &[]));
    });
}

#[test]
fn every_stack_refuses_bad_settings() {
    for desc in sdrkit_stack_analogtv::descriptors() {
        let schema = (desc.settings)();
        let mut rx = (desc.make_receiver)(&StackConfig::new(Values::defaults(&schema))).unwrap();
        assert!(
            matches!(
                schema.id("no_such_setting"),
                Err(SettingError::Unknown { .. })
            ),
            "{}",
            desc.id
        );
        if let Ok(id) = schema.id("saturation") {
            assert!(
                matches!(
                    owner::apply(rx.as_mut(), id, &Value::F64(9.0)),
                    Err(SettingError::OutOfRange { .. })
                ),
                "{}: saturation 9 is out of range",
                desc.id
            );
        }
    }
}

#[test]
fn the_generators_declare_their_signal() {
    for desc in sdrkit_stack_analogtv::descriptors() {
        let values = Values::defaults(&(desc.settings)());
        let g = (desc.make_generator.expect("a generator"))(
            &sdrkit_core::GenConfig::new(desc.tuning.rate_hz, values).with_seed(1),
        )
        .unwrap();
        let i = g.signal_info().expect("signal info");
        assert!(i.bandwidth_hz > 0.0 && i.power > 0.0, "{}", desc.id);
        for s in (desc.settings)().settings() {
            assert!(
                !sdrkit_components::sim::is_generator_key(&s.field.key),
                "{}: {} is a standard generator key",
                desc.id,
                s.field.key
            );
        }
    }
}

/// The `fpv` test signal at 5806 MHz with R5 as the one channel names R5, as the stack's verify case expects.
#[test]
fn the_fpv_test_signal_names_the_channel_set() {
    let r = Harness::new(&FPV)
        .set(&sdrkit_stack_analogtv::settings::CHANNELS, "R5=5806000000")
        .freq(5806e6)
        .seconds(2.0)
        .run()
        .expect("fpv runs");
    let channel = r
        .results
        .fields()
        .at(&sdrkit_stack_analogtv::typed::FpvFields::CHANNEL)
        .unwrap_or("")
        .to_owned();
    assert!(channel.contains("R5"), "{channel}");
}
