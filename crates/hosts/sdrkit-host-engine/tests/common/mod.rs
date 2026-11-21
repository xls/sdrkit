//! A test engine on memory storage, a fixed clock, a seeded random source and a fake network, with one small stack.

#![allow(dead_code)]

use sdrkit_core::net::FakeTransport;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, EventDef, Facilities, Field, HostRequest, InputInfo,
    LockProfile, MemStorage, Output, Owner, Published, Random, Receiver, Results, ResultsDecl,
    Setting, SettingError, SettingId, SettingSet, SignalType, StackDescriptor, SubMode, Tuning,
    Value, Values,
};
pub use sdrkit_driver_core::rtl_tcp::fake::{FakeOpts, FakeRtl};
use sdrkit_host_engine::{Engine, OwnerRef, Setup};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The mean power of what the meter is fed, times its gain.
pub const POWER: Field<field::F64> = Field::new("power", "Power", "");
/// The callsign of the station that the engine supplied the meter.
pub const CALL: Field<field::Text> = Field::new("call", "Station", "");
/// One per block the meter is fed.
pub static BLOCK: EventDef = EventDef::new("meter.block", "Block", &[&POWER]);
/// The gain of the meter.
pub const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "", 1.0).range(0.0, 10.0, 0.0);
/// The gain at which the meter stops on a fault, as a receiver with a mistake in it does.
pub const FAULT_GAIN: f64 = 10.0;
/// Above 0, the meter sends one request to follow this channel as AM voice.
pub const FOLLOW_HZ: Setting<field::Hz> =
    Setting::new("follow_hz", "Follow", "Hz", 0.0).range(0.0, 1e10, 0.0);
/// On, the meter sends one request to hop over two channels.
pub const HOP: Setting<field::Bool> = Setting::new("hop", "Hop", "", false);
pub const STORE: Setting<field::Bool> = Setting::new("store", "Store", "", false);
pub const SQUELCH: Field<field::Bool> = Field::new("squelch_open", "Squelch open", "");
/// The frequency where the meter's squelch opens.
pub const BUSY_HZ: f64 = 100.02e6;

sdrkit_core::settings! {
    /// The meter's settings.
    pub enum MeterSetting {
        Gain = GAIN,
        FollowHz = FOLLOW_HZ,
        Hop = HOP,
        Store = STORE,
    }
}

/// A small stack that reports the mean power of what it is fed and raises one event per block.
pub struct Meter {
    results: Published,
    values: Values,
    gain: f64,
    asked: bool,
    tuned_hz: f64,
}

impl Receiver for Meter {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        if input.rate_hz < 1000.0 {
            return Err(ConfigError::RateTooLow {
                need: 1000.0,
                got: input.rate_hz,
            });
        }
        self.tuned_hz = input.tuned_hz;
        Ok(())
    }
    fn retuned(&mut self, input: &InputInfo) {
        self.tuned_hz = input.tuned_hz;
    }
    fn set_station(&mut self, station: &sdrkit_core::Station) {
        self.results.set(&CALL, &station.callsign);
    }
    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        assert!(
            self.gain < FAULT_GAIN,
            "the meter has no gain of {}",
            self.gain
        );
        let p = f64::from(iq.iter().map(|s| s.norm_sqr()).sum::<f32>()) / iq.len().max(1) as f64;
        let p = p * self.gain;
        self.results.set(&POWER, p);
        let r = self.results.results_mut();
        r.update_series(sdrkit_core::SeriesId(0), |s| {
            s.fill(true, [[0.0, p as f32]])
        });
        r.update_raster(sdrkit_core::RasterId(0), |x| {
            if x.width > 0 {
                return false;
            }
            *x = sdrkit_core::Raster::new(
                2,
                1,
                sdrkit_core::Axis::new(100e6, 25e3, "Hz"),
                sdrkit_core::Axis::new(0.0, -0.5, "s"),
                "dB",
            );
            true
        });
        self.results
            .set(&SQUELCH, (self.tuned_hz - BUSY_HZ).abs() < 1.0);
        out.events.push(BLOCK.event(0.0, 0.0).set(&POWER, p));
        if !self.asked {
            self.asked = true;
            let follow = self.values.get(&FOLLOW_HZ);
            if follow > 0.0 {
                out.requests.push(HostRequest::Follow {
                    freq_hz: follow,
                    signal: Some(SignalType::AmVoice),
                    talkgroup: None,
                    bandwidth_hz: 10e3,
                    label: "Tower".into(),
                    max_s: 60.0,
                    // A key that the followed stack lacks is left out.
                    settings: vec![
                        ("gain".into(), sdrkit_core::Value::F64(2.5)),
                        ("absent".into(), sdrkit_core::Value::Bool(true)),
                    ],
                });
            }
            if self.values.get(&STORE) {
                for _ in 0..2 {
                    out.requests.push(HostRequest::StoreMemory {
                        name: "Twice".into(),
                        freq_hz: 145.5e6,
                        target: sdrkit_core::TuneTarget::Running,
                        bandwidth_hz: None,
                        settings: Vec::new(),
                        group: String::new(),
                    });
                }
            }
            if self.values.get(&HOP) {
                out.requests.push(HostRequest::Hop {
                    channels: vec![100e6, 101e6],
                    dwell_s: 1.0,
                    stop_on_lock: false,
                });
            }
        }
    }
}

impl Owner for Meter {
    fn schema(&self) -> &Arc<sdrkit_core::Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match MeterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MeterSetting::Gain => self.gain = GAIN.read(id, value)?,
            MeterSetting::FollowHz | MeterSetting::Hop | MeterSetting::Store => {}
        }
        self.values.set(id, value)
    }
    fn results_decl(&self) -> &Arc<sdrkit_core::ResultsDecl> {
        self.results.results().decl()
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
}

/// The meter stack.
pub static METER: StackDescriptor = METER_DESC;

/// The meter stack, to build others from.
pub const METER_DESC: StackDescriptor = StackDescriptor {
    id: "meter",
    name: "Meter",
    group: "Tools",
    blurb: "Shows the power of the signal.",
    tuning: Tuning::anywhere(100e6, 48e3, 1e3, 10e3),
    submodes: &[SubMode {
        id: "slow",
        name: "Slow",
        default_hz: 100e6,
    }],
    settings: MeterSetting::schema,
    make_receiver: |cfg| {
        Ok(Box::new(Meter {
            results: Published::from_decl(Arc::new(
                ResultsDecl::new()
                    .with_fields(&[&POWER, &CALL, &SQUELCH])
                    .series("levels", "Levels", "", "")
                    .raster("grid", "Grid", "Hz", "s", "dB")
                    .event(&BLOCK),
            )),
            values: cfg.settings.clone(),
            gain: cfg.settings.get(&GAIN),
            asked: false,
            tuned_hz: 0.0,
        }))
    },
    make_generator: None,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Carrier],
    auto_gain: AutoGain::Hold,
    verify: None,
    benchmarks: Some(|| {
        vec![
            sdrkit_core::Benchmark {
                name: "meter.power",
                run: |seconds| {
                    let iq = vec![Complex32::new(0.5, 0.5); 4096];
                    sdrkit_core::bench::measure(seconds, iq.len(), 4096.0 / 48e3, || {
                        std::hint::black_box(iq.iter().map(|s| s.norm_sqr()).sum::<f32>());
                    })
                },
            },
            // A benchmark of a graph of two components: the engine shows its load.
            sdrkit_core::Benchmark {
                name: "meter.graph",
                run: |seconds| {
                    use sdrkit_graph::components::{DcRemoval, NullSink};
                    let mut g = sdrkit_graph::GraphBuilder::new()
                        .add("dc", DcRemoval::default())
                        .add("sink", NullSink::default())
                        .input("dc")
                        .connect("dc", "sink")
                        .build()
                        .unwrap();
                    sdrkit_core::Receiver::configure(
                        &mut g,
                        &sdrkit_core::InputInfo::new(48e3, 0.0, 0.0).recording(),
                    )
                    .unwrap();
                    let iq = vec![Complex32::new(0.5, 0.5); 4096];
                    let mut out = sdrkit_core::Output::default();
                    sdrkit_core::bench::measure(seconds, iq.len(), 4096.0 / 48e3, || {
                        out.clear();
                        sdrkit_core::Receiver::feed(&mut g, &iq, &mut out);
                    })
                },
            },
        ]
    }),
};

/// The offset of the tone of the tone stack's test signal from the centre.
pub const TONE_OFFSET_HZ: f64 = 50e3;

/// A tone at [`TONE_OFFSET_HZ`] over a little noise.
struct ToneGen {
    rate: f64,
    phase: f64,
    seed: u32,
}

impl sdrkit_core::Generator for ToneGen {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let step = std::f64::consts::TAU * TONE_OFFSET_HZ / self.rate;
        for s in out.iter_mut() {
            self.seed = self
                .seed
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            let n = (f32::from((self.seed >> 16) as u16) / 65536.0 - 0.5) * 0.02;
            *s = Complex32::from_polar(0.5, self.phase as f32) + Complex32::new(n, -n);
            self.phase = (self.phase + step) % std::f64::consts::TAU;
        }
        out.len()
    }
}

/// The meter with a test signal: a tone beside the centre.
pub static TONE: StackDescriptor = StackDescriptor {
    id: "tone",
    name: "Tone",
    submodes: &[],
    signal_types: &[],
    make_generator: Some(|cfg| {
        Ok(Box::new(ToneGen {
            rate: cfg.rate_hz,
            phase: 0.0,
            seed: 1,
        }))
    }),
    benchmarks: None,
    ..METER_DESC
};

/// The meter as a stack for AM voice, with no variants.
pub static VOICE: StackDescriptor = StackDescriptor {
    id: "voice",
    name: "Voice",
    submodes: &[],
    signal_types: &[SignalType::AmVoice],
    ..METER_DESC
};

/// An engine with the meter and voice stacks, the standard components and `net` as its
/// network.
pub fn engine_on(net: &FakeTransport) -> Engine {
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(1.7e9),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(net.clone()),
        stacks: vec![&METER, &VOICE, &TONE],
        families: Vec::new(),
        components: sdrkit_graph::Registry::standard(),
    })
    .unwrap()
}

/// An engine on the system clock, with `storage`, for services whose steps follow the time.
pub fn live_engine(storage: Arc<dyn sdrkit_core::Storage>) -> Engine {
    Engine::new(Setup {
        facilities: Facilities {
            storage,
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![&METER, &VOICE, &TONE],
        families: Vec::new(),
        components: sdrkit_graph::Registry::standard(),
    })
    .unwrap()
}

/// Opens the test-signal radio that plays the tone stack's signal at `rate_hz`, centred on 100 MHz.
pub fn tone_radio(e: &Engine, rate_hz: f64) -> sdrkit_host_engine::RadioId {
    let kind = e.find("driver.synth").unwrap();
    let mut v = defaults(e, kind);
    for (key, value) in [
        ("rate", Value::Hz(rate_hz)),
        ("centre_hz", Value::Hz(100e6)),
    ] {
        if let Ok(id) = v.schema().id(key) {
            v.set(id, &value).unwrap();
        }
    }
    e.open_radio("synth", "synth:tone", &v).unwrap()
}

/// An engine with no network.
pub fn engine() -> Engine {
    engine_on(&FakeTransport::new())
}

/// The defaults of the owner `r`.
pub fn defaults(e: &Engine, r: OwnerRef) -> Values {
    e.owner(r, |o| Values::defaults(o.schema())).unwrap()
}

/// Waits up to `secs` for `f` to give something.
pub fn wait_for<T>(secs: f64, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let t = Instant::now();
    loop {
        if let Some(v) = f() {
            return Some(v);
        }
        if t.elapsed().as_secs_f64() > secs {
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The sample rate the fake rtl_tcp server sends at, whatever rate is requested.
pub const RTL_RATE: f64 = 2.048e6;

/// A fake rtl_tcp server on a fake network of its own, which sends at [`RTL_RATE`].
pub fn fake_rtl() -> FakeRtl {
    FakeRtl::start(FakeOpts {
        own_rate: Some(RTL_RATE),
        ..FakeOpts::default()
    })
}

/// The value of field `key` of the results of owner `r`.
pub fn field(e: &Engine, r: OwnerRef, key: &str) -> Option<Value> {
    e.owner(r, |o| {
        let res = o.results();
        let id = res.decl().field_id(key)?;
        res.fields().value(id).cloned()
    })
    .ok()
    .flatten()
}

/// The number in field `key` of owner `r`, 0 when it has none.
pub fn number(e: &Engine, r: OwnerRef, key: &str) -> f64 {
    match field(e, r, key) {
        Some(Value::I64(n)) => n as f64,
        Some(Value::F64(x) | Value::Hz(x) | Value::Duration(x)) => x,
        _ => 0.0,
    }
}
