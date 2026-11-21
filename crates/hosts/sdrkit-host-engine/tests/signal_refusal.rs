//! A test signal that refuses its settings fails the open of its radio, the start of a session and the change of a
//! setting with an error that names the setting, and the running signal keeps its settings.

mod common;

use common::METER_DESC;
use sdrkit_components::sim::NoiseKind;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Clock, Complex32, Facilities, MemStorage, Random, Setting, SettingError, SettingSet,
    SignalInfo, StackDescriptor, Value, Values,
};
use sdrkit_host_engine::{Engine, EngineError, Input, OwnerRef, RadioId, Setup};
use std::sync::Arc;

/// On, the test signal states its band and its power; off, it states nothing, so noise cannot be given as a signal to
/// noise ratio.
const GEN_BAND: Setting<field::Bool> = Setting::new("gen_band", "States its band", "", true);

sdrkit_core::settings! {
    /// The settings of the meter, and [`GEN_BAND`].
    enum BareSetting {
        Gain = common::GAIN,
        FollowHz = common::FOLLOW_HZ,
        Hop = common::HOP,
        Store = common::STORE,
        Band = GEN_BAND,
    }
}

/// Silence that states a band of 10 kHz and its power only if [`GEN_BAND`] is on.
struct Bare {
    rate: f64,
    band: bool,
}

impl sdrkit_core::Generator for Bare {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        self.band.then(|| SignalInfo::new(10e3))
    }
}

/// The meter with [`Bare`] as its test signal.
static BARE: StackDescriptor = StackDescriptor {
    id: "bare",
    name: "Bare",
    submodes: &[],
    signal_types: &[],
    settings: BareSetting::schema,
    make_generator: Some(|cfg| {
        Ok(Box::new(Bare {
            rate: cfg.rate_hz,
            band: cfg.settings.get(&GEN_BAND),
        }))
    }),
    benchmarks: None,
    ..METER_DESC
};

fn engine() -> Engine {
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(1.7e9),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![&BARE],
        families: Vec::new(),
        components: sdrkit_graph::Registry::standard(),
    })
    .unwrap()
}

/// `values` with `key` set to `value`.
fn with(mut values: Values, key: &str, value: Value) -> Values {
    let id = values.schema().id(key).unwrap();
    values.set(id, &value).unwrap();
    values
}

fn snr() -> Value {
    Value::Choice(NoiseKind::Snr.index())
}

/// The values of the test-signal radio of `bare` at 250 kHz around 100 MHz, with noise given as a signal to noise
/// ratio.
fn radio_values(e: &Engine) -> Values {
    let v = Values::defaults(&e.radio_schema("synth", "synth:bare").unwrap());
    let v = with(v, "rate", Value::Hz(250e3));
    let v = with(v, "centre_hz", Value::Hz(100e6));
    with(v, "gen_noise", snr())
}

fn open(e: &Engine) -> RadioId {
    e.open_radio("synth", "synth:bare", &radio_values(e))
        .unwrap()
}

/// Asserts that `r` is the refusal of setting `key` because the signal states no bandwidth.
fn assert_refused<T: std::fmt::Debug>(r: Result<T, EngineError>, key: &str) {
    let e = r.expect_err("the signal refuses noise as a ratio");
    let text = e.to_string();
    assert!(
        matches!(
            e,
            EngineError::Setting {
                error: SettingError::Config { .. },
                ..
            }
        ),
        "{e:?}"
    );
    assert!(text.contains(key) && text.contains("bandwidth"), "{text}");
}

/// The value of setting `key` of owner `r`.
fn value(e: &Engine, r: OwnerRef, key: &str) -> Value {
    e.owner(r, |o| {
        let v = o.values().unwrap();
        v.value(v.schema().id(key).unwrap()).unwrap().clone()
    })
    .unwrap()
}

#[test]
fn a_session_with_noise_as_a_ratio_fails_to_start_on_a_signal_that_states_no_bandwidth() {
    let e = engine();
    let radio = open(&e);
    let defaults = Values::defaults(&BareSetting::schema());
    let bare = with(defaults.clone(), GEN_BAND.key(), Value::Bool(false));
    assert_refused(
        e.start_stack("bare", None, &bare, Input::Radio(radio), Some(100e6)),
        "gen_noise",
    );
    e.start_stack("bare", None, &defaults, Input::Radio(radio), Some(100e6))
        .expect("the session starts on a signal that states its band");
}

#[test]
fn a_running_session_keeps_its_signal_when_the_signal_refuses_a_value() {
    let e = engine();
    let radio = open(&e);
    let defaults = Values::defaults(&BareSetting::schema());
    let s = e
        .start_stack("bare", None, &defaults, Input::Radio(radio), Some(100e6))
        .unwrap();
    let session = OwnerRef::Session(s);
    assert_refused(
        e.set(session, GEN_BAND.key(), Value::Bool(false)),
        GEN_BAND.key(),
    );
    assert_eq!(value(&e, session, GEN_BAND.key()), Value::Bool(true));
    // A floor needs nothing of the signal.
    e.set(
        OwnerRef::Radio(radio),
        "gen_noise",
        Value::Choice(NoiseKind::Floor.index()),
    )
    .unwrap();
    e.set(session, GEN_BAND.key(), Value::Bool(false)).unwrap();
    assert_eq!(value(&e, session, GEN_BAND.key()), Value::Bool(false));
}

#[test]
fn a_radio_refuses_noise_as_a_ratio_at_the_open_and_while_it_runs() {
    let e = engine();
    let bare = with(radio_values(&e), GEN_BAND.key(), Value::Bool(false));
    assert_refused(e.open_radio("synth", "synth:bare", &bare), "gen_noise");
    let radio = OwnerRef::Radio(open(&e));
    assert_refused(
        e.set(radio, GEN_BAND.key(), Value::Bool(false)),
        GEN_BAND.key(),
    );
    assert_eq!(value(&e, radio, GEN_BAND.key()), Value::Bool(true));
}
