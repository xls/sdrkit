//! The frequency that a session listens to and the centre of its input differ by the tune offset of its stack, whether the
//! host gave a frequency or not, and the test signal of the stack has its station on the frequency that the session
//! listens to.

mod common;

use common::*;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Clock, Complex32, ConfigError, Facilities, Field, InputInfo, LockState, MemStorage, Output,
    Owner, Published, Random, Receiver, Results, ResultsDecl, SettingError, SettingId,
    StackDescriptor, Tuning, Value, Values,
};
use sdrkit_host_engine::{Engine, Input, OwnerRef, RadioId, SessionId, Setup};
use std::sync::Arc;

const T: f64 = 20.0;
/// The tune offset of the probe stacks.
const OFFSET_HZ: f64 = 50e3;
/// The sample rate of the probe stacks.
const RATE_HZ: f64 = 250e3;

/// The part of the power of the last block that is a tone on the frequency that the probe listens to: 1 for a tone
/// there, 0 for a tone elsewhere.
const ON_STATION: Field<field::F64> = Field::new("on_station", "On station", "");
/// The frequency that the probe was told it listens to.
const LISTENS: Field<field::Hz> = Field::new("listens_hz", "Listens", "Hz");
/// The centre of the input that the probe was told.
const INPUT_CENTRE: Field<field::Hz> = Field::new("input_centre_hz", "Centre", "Hz");
/// The sample rate of the input that the probe was told.
const INPUT_RATE: Field<field::Hz> = Field::new("input_rate_hz", "Rate", "Hz");

/// A receiver that measures how much of its input is a tone on the frequency that it listens to.
struct Probe {
    results: Published,
    values: Values,
    info: Option<InputInfo>,
}

impl Probe {
    fn told(&mut self, input: &InputInfo) {
        self.info = Some(*input);
        self.results.set(&LISTENS, input.tuned_hz);
        self.results.set(&INPUT_CENTRE, input.centre_hz);
        self.results.set(&INPUT_RATE, input.rate_hz);
    }
}

impl Receiver for Probe {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        self.told(input);
        Ok(())
    }
    fn retuned(&mut self, input: &InputInfo) {
        self.told(input);
    }
    fn feed(&mut self, iq: &[Complex32], _out: &mut Output) {
        let Some(i) = self.info else {
            return;
        };
        let step = std::f64::consts::TAU * (i.tuned_hz - i.centre_hz) / i.rate_hz;
        let (mut re, mut im, mut power) = (0.0f64, 0.0f64, 0.0f64);
        for (n, s) in iq.iter().enumerate() {
            let (sin, cos) = (step * n as f64).sin_cos();
            let (a, b) = (f64::from(s.re), f64::from(s.im));
            re += a * cos + b * sin;
            im += b * cos - a * sin;
            power += a * a + b * b;
        }
        if power > 0.0 {
            let on = (re * re + im * im) / (iq.len() as f64 * power);
            self.results.set(&ON_STATION, on);
        }
    }
}

impl Owner for Probe {
    fn schema(&self) -> &Arc<sdrkit_core::Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.values.set(id, value)
    }
    fn results_decl(&self) -> &Arc<sdrkit_core::ResultsDecl> {
        self.results.results().decl()
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
}

/// A tone at `offset_hz` from the centre of the samples.
struct Tone {
    rate: f64,
    offset_hz: f64,
    phase: f64,
    /// True if the generator states that its station is at the centre of the samples.
    states_centre: bool,
}

impl sdrkit_core::Generator for Tone {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let step = std::f64::consts::TAU * self.offset_hz / self.rate;
        for s in out.iter_mut() {
            *s = Complex32::from_polar(0.5, self.phase as f32);
            self.phase = (self.phase + step) % std::f64::consts::TAU;
        }
        out.len()
    }
    fn station_offset_hz(&self, tune_offset_hz: f64) -> f64 {
        if self.states_centre {
            0.0
        } else {
            -tune_offset_hz
        }
    }
}

/// A probe stack with no default frequency and a tune offset of [`OFFSET_HZ`].
const PROBE_DESC: StackDescriptor = StackDescriptor {
    tuning: Tuning {
        tune_offset_hz: OFFSET_HZ,
        ..Tuning::anywhere(0.0, RATE_HZ, 1e3, 10e3)
    },
    submodes: &[],
    signal_types: &[],
    make_receiver: |cfg| {
        Ok(Box::new(Probe {
            results: Published::from_decl(Arc::new(ResultsDecl::new().with_fields(&[
                &ON_STATION,
                &LISTENS,
                &INPUT_CENTRE,
                &INPUT_RATE,
            ]))),
            values: cfg.settings.clone(),
            info: None,
        }))
    },
    benchmarks: None,
    ..METER_DESC
};

/// The probe whose test signal has its station the tune offset below the centre, as a radio that the engine tuned gives it.
static BESIDE: StackDescriptor = StackDescriptor {
    id: "beside",
    name: "Beside",
    make_generator: Some(|cfg| {
        Ok(Box::new(Tone {
            rate: cfg.rate_hz,
            offset_hz: -OFFSET_HZ,
            phase: 0.0,
            states_centre: false,
        }))
    }),
    ..PROBE_DESC
};

/// The probe whose test signal has its station at the centre of the samples, and says so.
static CENTRED: StackDescriptor = StackDescriptor {
    id: "centred",
    name: "Centred",
    make_generator: Some(|cfg| {
        Ok(Box::new(Tone {
            rate: cfg.rate_hz,
            offset_hz: 0.0,
            phase: 0.0,
            states_centre: true,
        }))
    }),
    ..PROBE_DESC
};

/// An engine with `stacks` and `components`.
fn engine_of(stacks: Vec<&'static StackDescriptor>, components: sdrkit_graph::Registry) -> Engine {
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(1.7e9),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(FakeTransport::new()),
        stacks,
        families: Vec::new(),
        components,
    })
    .unwrap()
}

/// Starts `stack` with its default settings on its own test signal, tuned to `freq_hz` if one is given.
fn on_own_signal(
    e: &Engine,
    stack: &StackDescriptor,
    freq_hz: Option<f64>,
) -> (RadioId, SessionId) {
    let synth = e.find("driver.synth").unwrap();
    let address = format!("synth:{}", stack.id);
    let radio = e
        .open_radio("synth", &address, &defaults(e, synth))
        .unwrap();
    let values = Values::defaults(&(stack.settings)());
    let id = e
        .start_stack(stack.id, None, &values, Input::Radio(radio), freq_hz)
        .unwrap();
    (radio, id)
}

/// Runs probe stack `stack` on its own test signal and checks that the session listens the tune offset below the centre of
/// its input, on `freq_hz` if one is given, at the rate of the stack, and that the tone of the test signal is there.
fn the_probe_hears_its_station(stack: &'static StackDescriptor, freq_hz: Option<f64>) {
    assert_eq!(stack.tuning.default_hz, 0.0);
    let e = engine_of(vec![&BESIDE, &CENTRED], sdrkit_graph::Registry::standard());
    let (_, id) = on_own_signal(&e, stack, freq_hz);
    let s = OwnerRef::Session(id);
    let heard = wait_for(T, || (number(&e, s, ON_STATION.key) > 0.9).then_some(()));
    let (listens, centre) = (number(&e, s, LISTENS.key), number(&e, s, INPUT_CENTRE.key));
    assert!(
        heard.is_some(),
        "{}: {} of the power is on the station; listens to {listens}, centre {centre}",
        stack.id,
        number(&e, s, ON_STATION.key),
    );
    assert!(centre > 0.0);
    assert_eq!(listens, centre - OFFSET_HZ, "{}", stack.id);
    assert_eq!(number(&e, s, INPUT_RATE.key), RATE_HZ, "{}", stack.id);
    if let Some(hz) = freq_hz {
        assert_eq!(listens, hz, "{}", stack.id);
    }
}

#[test]
fn a_session_with_no_frequency_listens_the_tune_offset_below_the_centre_of_its_input() {
    the_probe_hears_its_station(&BESIDE, None);
}

#[test]
fn a_session_with_a_frequency_has_its_radio_the_tune_offset_above_it() {
    the_probe_hears_its_station(&BESIDE, Some(433.5e6));
}

#[test]
fn a_test_signal_made_at_the_centre_is_moved_onto_the_station() {
    the_probe_hears_its_station(&CENTRED, None);
    the_probe_hears_its_station(&CENTRED, Some(433.5e6));
}

/// Runs `fanet` on its own test signal and returns true if it locks.
fn fanet_locks(freq_hz: Option<f64>) -> bool {
    let mut components = sdrkit_graph::Registry::standard();
    sdrkit_stack_lora::register(&mut components);
    let e = engine_of(sdrkit_stack_lora::descriptors(), components);
    let d = &sdrkit_stack_lora::FANET;
    assert_ne!(d.tuning.tune_offset_hz, 0.0);
    let (_, id) = on_own_signal(&e, d, freq_hz);
    wait_for(3.0 * T, || {
        let lock = e
            .owner(OwnerRef::Session(id), |o| o.results().status().lock_state)
            .unwrap();
        matches!(lock, LockState::Tracking | LockState::Locked).then_some(())
    })
    .is_some()
}

#[test]
fn a_stack_with_a_tune_offset_locks_on_its_test_signal_with_no_frequency() {
    // A frequency of 0 is no frequency, whatever the default of the stack.
    assert!(fanet_locks(Some(0.0)));
}

#[test]
fn a_stack_with_a_tune_offset_locks_on_its_test_signal_with_a_frequency() {
    assert!(fanet_locks(Some(868.2e6)));
}
