//! The engine makes the test signal of a stack once for an open and a start, and once more for each change of what the
//! signal is made from. A check of the signal hands the signal that it made to the driver.

mod common;

use common::{wait_for, METER_DESC};
use sdrkit_components::sim::NoiseKind;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Clock, Complex32, ConfigError, Facilities, GenConfig, Generator, MemStorage, Random,
    Setting, SettingError, SettingSet, SignalInfo, StackConfig, StackDescriptor, Value, Values,
};
use sdrkit_host_engine::{Engine, EngineError, Input, OwnerRef, RadioId, Setup};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// On, the signal states its band, so noise can be given as a signal to noise ratio.
const GEN_BAND: Setting<field::Bool> = Setting::new("gen_band", "States its band", "", true);
/// The level of the signal, which the generator reads.
const GEN_LEVEL: Setting<field::F64> = Setting::new("gen_level", "Level", "", 0.5);
/// On, the receiver of the stack refuses its settings.
const REFUSE: Setting<field::Bool> = Setting::new("refuse", "Refuse", "", false);

sdrkit_core::settings! {
    /// The settings of the meter, and those of the counted signal.
    enum CountSetting {
        Gain = common::GAIN,
        FollowHz = common::FOLLOW_HZ,
        Hop = common::HOP,
        Store = common::STORE,
        Band = GEN_BAND,
        Level = GEN_LEVEL,
        Refuse = REFUSE,
    }
}

/// The makes of the signal of each test's stack.
static MAKES: [AtomicUsize; 4] = [const { AtomicUsize::new(0) }; 4];
/// The drops of the signal of each test's stack.
static DROPS: [AtomicUsize; 4] = [const { AtomicUsize::new(0) }; 4];

/// A constant at [`GEN_LEVEL`] that states a band of 10 kHz if [`GEN_BAND`] is on.
struct Level {
    /// The stack of the signal: its drop is counted in [`DROPS`].
    n: usize,
    rate: f64,
    level: f32,
    band: bool,
}

impl Generator for Level {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::new(self.level, 0.0));
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        self.band.then(|| SignalInfo::new(10e3))
    }
}

impl Drop for Level {
    fn drop(&mut self) {
        DROPS[self.n].fetch_add(1, Ordering::SeqCst);
    }
}

/// The signal of stack `N`, counted in [`MAKES`].
fn make<const N: usize>(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    MAKES[N].fetch_add(1, Ordering::SeqCst);
    Ok(Box::new(Level {
        n: N,
        rate: cfg.rate_hz,
        level: cfg.settings.get(&GEN_LEVEL) as f32,
        band: cfg.settings.get(&GEN_BAND),
    }))
}

/// The meter, refused if [`REFUSE`] is on.
fn receiver(cfg: &StackConfig) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    if cfg.settings.get(&REFUSE) {
        return Err(ConfigError::RateTooLow {
            need: 1e9,
            got: 0.0,
        });
    }
    (METER_DESC.make_receiver)(cfg)
}

/// The meter with the counted signal `N`.
const fn counted<const N: usize>(id: &'static str) -> StackDescriptor {
    StackDescriptor {
        id,
        name: "Counted",
        submodes: &[],
        signal_types: &[],
        settings: CountSetting::schema,
        make_receiver: receiver,
        make_generator: Some(make::<N>),
        benchmarks: None,
        ..METER_DESC
    }
}

static COUNT0: StackDescriptor = counted::<0>("count0");
static COUNT1: StackDescriptor = counted::<1>("count1");
static COUNT2: StackDescriptor = counted::<2>("count2");
static COUNT3: StackDescriptor = counted::<3>("count3");

fn engine() -> Engine {
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(1.7e9),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![&COUNT0, &COUNT1, &COUNT2, &COUNT3],
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

/// Opens the test radio of stack `id` at 250 kHz around 100 MHz, with noise as a signal to noise ratio.
fn open(e: &Engine, id: &str) -> RadioId {
    let address = format!("synth:{id}");
    let v = Values::defaults(&e.radio_schema("synth", &address).unwrap());
    let v = with(v, "rate", Value::Hz(250e3));
    let v = with(v, "centre_hz", Value::Hz(100e6));
    let v = with(v, "gen_noise", Value::Choice(NoiseKind::Snr.index()));
    e.open_radio("synth", &address, &v).unwrap()
}

/// The makes of signal `n` once the driver and the engine have taken every change: waits for at least `least`, then
/// for some ticks of the engine.
fn makes(n: usize, least: usize) -> usize {
    wait_for(5.0, || {
        (MAKES[n].load(Ordering::SeqCst) >= least).then_some(())
    });
    std::thread::sleep(Duration::from_millis(500));
    MAKES[n].load(Ordering::SeqCst)
}

#[test]
fn an_open_and_a_start_make_the_signal_once() {
    let e = engine();
    let radio = open(&e, "count0");
    let defaults = Values::defaults(&CountSetting::schema());
    e.start_stack("count0", None, &defaults, Input::Radio(radio), Some(100e6))
        .unwrap();
    assert_eq!(makes(0, 1), 1);
}

#[test]
fn a_start_with_other_signal_settings_makes_the_signal_once_more() {
    let e = engine();
    let radio = open(&e, "count1");
    assert_eq!(makes(1, 1), 1);
    let louder = with(
        Values::defaults(&CountSetting::schema()),
        GEN_LEVEL.key(),
        Value::F64(0.9),
    );
    e.start_stack("count1", None, &louder, Input::Radio(radio), Some(100e6))
        .unwrap();
    assert_eq!(makes(1, 2), 2, "the start hands its signal to the driver");
}

/// A start that the signal refuses makes the signal once, to find the refusal. A start that the receiver refuses after
/// the check made the signal drops that signal: the driver plays on what it played, and makes nothing.
#[test]
fn a_failed_start_makes_the_signal_once_and_drops_it() {
    let e = engine();
    let radio = open(&e, "count2");
    assert_eq!(makes(2, 1), 1);
    let defaults = Values::defaults(&CountSetting::schema());
    let bare = with(defaults.clone(), GEN_BAND.key(), Value::Bool(false));
    let r = e.start_stack("count2", None, &bare, Input::Radio(radio), Some(100e6));
    assert!(
        matches!(
            r,
            Err(EngineError::Setting {
                error: SettingError::Config { .. },
                ..
            })
        ),
        "{r:?}"
    );
    assert_eq!(makes(2, 2), 2, "a refused start makes the signal once");
    let refused = with(defaults.clone(), REFUSE.key(), Value::Bool(true));
    let r = e.start_stack("count2", None, &refused, Input::Radio(radio), Some(100e6));
    assert!(
        matches!(r, Err(EngineError::Config(ConfigError::RateTooLow { .. }))),
        "{r:?}"
    );
    assert_eq!(
        makes(2, 3),
        3,
        "a start that fails after the check makes the signal once"
    );
    let live = MAKES[2].load(Ordering::SeqCst) - DROPS[2].load(Ordering::SeqCst);
    assert_eq!(live, 1, "the signal of the failed start is dropped");
    // The radio plays on the signal of the open: a start with its settings makes nothing.
    e.start_stack("count2", None, &defaults, Input::Radio(radio), Some(100e6))
        .unwrap();
    assert_eq!(makes(2, 3), 3);
}

#[test]
fn a_change_of_a_signal_setting_makes_the_signal_once() {
    let e = engine();
    let radio = open(&e, "count3");
    let defaults = Values::defaults(&CountSetting::schema());
    let s = e
        .start_stack("count3", None, &defaults, Input::Radio(radio), Some(100e6))
        .unwrap();
    assert_eq!(makes(3, 1), 1);
    e.set(OwnerRef::Session(s), GEN_LEVEL.key(), Value::F64(0.2))
        .unwrap();
    assert_eq!(makes(3, 2), 2, "a session setting of the signal");
    e.set(
        OwnerRef::Radio(radio),
        "gen_noise",
        Value::Choice(NoiseKind::Floor.index()),
    )
    .unwrap();
    assert_eq!(makes(3, 3), 3, "a radio setting of the signal");
    // A setting that the signal does not read makes nothing.
    e.set(OwnerRef::Radio(radio), "quantize", Value::Bool(false))
        .unwrap();
    assert_eq!(makes(3, 3), 3);
}
