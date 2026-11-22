//! The streams of a session: audio, media packets, pictures, taps, samples from the host, transmit samples, and the network
//! sink of a graph.

mod common;

use common::*;
use sdrkit_core::media::{Codec, MediaFormat, Packaging, Packet};
use sdrkit_core::net::{DatagramOptions, FakeTransport, Transport};
use sdrkit_core::{
    Clock, Complex32, ConfigError, Facilities, InputInfo, MemStorage, Output, Owner, Random,
    Receiver, Results, ResultsDecl, StackDescriptor, StreamKind, Value, Values,
};
use sdrkit_graph::{GraphPlan, Kind, PortRef, Registry, SampleType};
use sdrkit_host_engine::{
    Engine, EngineError, Input, OwnerRef, Samples, Setup, AUDIO_LOST, SAMPLES,
};
use std::sync::Arc;

const T: f64 = 10.0;

fn tone(n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| Complex32::from_polar(0.5, i as f32 * 0.3))
        .collect()
}

/// An engine with the FM stack, its components and `net` as its network.
fn fm_engine(net: &FakeTransport) -> Engine {
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(1.7e9),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(net.clone()),
        stacks: vec![&sdrkit_stack_fm::DESCRIPTOR],
        families: Vec::new(),
        components: sdrkit_stack_fm::graph::registry(),
    })
    .unwrap()
}

#[test]
fn audio_of_an_fm_session_reaches_the_host_and_what_it_leaves_is_counted_as_lost() {
    let e = fm_engine(&FakeTransport::new());
    let synth = e.find("driver.synth").unwrap();
    let r = e
        .open_radio("synth", "synth:fm", &defaults(&e, synth))
        .unwrap();
    let fm = e.find("stack.fm").unwrap();
    let s = e
        .start_stack("fm", None, &defaults(&e, fm), Input::Radio(r), None)
        .unwrap();
    let session = OwnerRef::Session(s);
    // The host takes nothing for a while: the oldest audio goes and is counted.
    let lost = wait_for(30.0, || {
        let n = number(&e, session, AUDIO_LOST.key);
        (n > 0.0).then_some(n)
    });
    assert!(
        lost.is_some(),
        "audio is lost when the host does not take it"
    );
    let mut buf = vec![0.0f32; 4800];
    let got = wait_for(T, || e.audio(s, &mut buf).unwrap()).expect("audio");
    assert!(got.frames > 0 && got.channels >= 1, "{got:?}");
    assert!(got.rate_hz >= 8e3, "{got:?}");
    let n = got.frames * usize::from(got.channels);
    assert!(buf[..n].iter().all(|x| x.is_finite()));
    // Once the host takes it, the audio keeps coming in whole frames.
    let mut frames = 0;
    wait_for(T, || {
        while let Some(a) = e.audio(s, &mut buf).unwrap() {
            assert_eq!(a.channels, got.channels);
            frames += a.frames;
        }
        (frames as f64 > got.rate_hz / 10.0).then_some(())
    })
    .expect("more audio");
}

/// A receiver that outputs one media packet per block and a picture every third block.
struct Media {
    results: Results,
    blocks: u64,
    sound: Arc<MediaFormat>,
    picture: Arc<MediaFormat>,
}

impl Receiver for Media {
    fn configure(&mut self, _: &InputInfo) -> Result<(), ConfigError> {
        Ok(())
    }
    fn feed(&mut self, _: &[Complex32], out: &mut Output) {
        self.blocks += 1;
        let t = self.blocks as f64 * 0.1;
        let bytes: Arc<[u8]> = vec![self.blocks as u8; 4].into();
        out.push_media(StreamKind::Aac, Packet::new(bytes, t), &self.sound);
        if self.blocks.is_multiple_of(3) {
            let pixels: Arc<[u8]> = vec![7u8; 16].into();
            out.push_media(StreamKind::Picture, Packet::new(pixels, t), &self.picture);
        }
    }
}

impl Owner for Media {
    fn schema(&self) -> &Arc<sdrkit_core::Schema> {
        static EMPTY: std::sync::OnceLock<Arc<sdrkit_core::Schema>> = std::sync::OnceLock::new();
        EMPTY.get_or_init(|| Arc::new(sdrkit_core::Schema::new(Vec::new()).unwrap()))
    }
    fn values(&self) -> Option<&Values> {
        None
    }
    fn apply(
        &mut self,
        id: sdrkit_core::SettingId,
        _: &Value,
    ) -> Result<(), sdrkit_core::SettingError> {
        Err(sdrkit_core::SettingError::NoSuchId { id })
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.decl()
    }
    fn results(&self) -> &Results {
        &self.results
    }
}

static MEDIA: StackDescriptor = StackDescriptor {
    id: "media",
    name: "Media",
    submodes: &[],
    signal_types: &[],
    make_generator: None,
    make_receiver: |_| {
        Ok(Box::new(Media {
            results: Results::new(Arc::new(ResultsDecl::new())),
            blocks: 0,
            sound: Arc::new(MediaFormat::new(Codec::Aac, Packaging::Adts)),
            picture: Arc::new(MediaFormat::new(Codec::Png, Packaging::Raw)),
        }))
    },
    ..METER_DESC
};

#[test]
fn media_packets_and_pictures_are_read_by_counter() {
    let e = Engine::new(Setup {
        facilities: Facilities::default(),
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![&MEDIA],
        families: Vec::new(),
        components: Registry::standard(),
    })
    .unwrap();
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack(
            "media",
            None,
            &Values::defaults(&(MEDIA.settings)()),
            Input::Feed(s_feed),
            None,
        )
        .unwrap();
    let mut bytes = Vec::new();
    let mut seen = Vec::new();
    let mut after = 0;
    wait_for(T, || {
        let _ = e.push(s_feed, Samples::Iq(&tone(480)));
        while let Some(p) = e.media(s, after, &mut bytes).unwrap() {
            assert_eq!(p.kind, StreamKind::Aac);
            assert_eq!(p.format.codec, Codec::Aac);
            assert_eq!(p.missed, 0);
            seen.push(bytes[0]);
            after = p.counter;
        }
        (seen.len() >= 6).then_some(())
    })
    .expect("media packets");
    // The packets come in order, each once.
    assert!(seen.windows(2).all(|w| w[1] == w[0] + 1), "{seen:?}");
    let p = e.picture(s, 0, &mut bytes).unwrap().expect("a picture");
    assert_eq!(p.kind, StreamKind::Picture);
    assert_eq!(bytes, vec![7u8; 16]);
    // A picture that its receiver sends without a number is a whole picture of its own.
    let tag = p
        .picture
        .as_ref()
        .expect("a picture states which picture it is");
    assert_eq!(tag.state, sdrkit_core::PictureState::Finished);
    assert!(tag.id >= 1 && tag.name.is_none());
    if let Some(next) = e.picture(s, p.counter, &mut bytes).unwrap() {
        assert_ne!(next.picture.unwrap().id, tag.id);
    }
    assert!(e.picture(s, p.counter, &mut bytes).unwrap().is_none() || p.counter > 0);
}

#[test]
fn a_tap_on_a_port_returns_its_samples_until_it_is_closed() {
    let e = engine();
    let mut plan = GraphPlan::new();
    let meter = plan
        .add(&Registry::standard(), "meter", "dsp.power_meter")
        .unwrap();
    plan.input(PortRef::new(meter, 0)).unwrap();
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e.start_graph(plan, Input::Feed(s_feed)).unwrap();
    let tap = e.open_tap(s, "input", 8).unwrap();
    let mut buf = vec![0.0f32; 512];
    let got = wait_for(T, || {
        let _ = e.push(s_feed, Samples::Iq(&tone(480)));
        e.read_tap(s, tap, &mut buf).unwrap()
    })
    .expect("tapped samples");
    assert_eq!(got.kind, Kind::Iq);
    assert_eq!(got.rate_hz, 48e3);
    assert!(got.values > 0 && got.values % 2 == 0, "{got:?}");
    assert!((buf[0].hypot(buf[1]) - 0.5).abs() < 1e-3, "{:?}", &buf[..2]);
    e.close_tap(s, tap).unwrap();
    assert!(matches!(
        e.read_tap(s, tap, &mut buf),
        Err(EngineError::NoSuchTap { .. })
    ));
    assert!(matches!(
        e.open_tap(s, "nowhere.out", 8),
        Err(EngineError::Graph(_))
    ));
}

#[test]
fn a_host_feed_takes_raw_samples_of_any_type() {
    let e = engine();
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    // 100 complex samples of two signed 16-bit values each.
    let raw: Vec<u8> = (0..200i16).flat_map(|x| (x * 100).to_le_bytes()).collect();
    let pushed = wait_for(T, || {
        e.push(
            s_feed,
            Samples::Raw {
                sample: SampleType::Cs16,
                bytes: &raw,
            },
        )
        .ok()
    });
    assert!(pushed.is_some());
    let fed = wait_for(T, || {
        let n = number(&e, OwnerRef::Session(s), SAMPLES.key);
        (n >= 100.0).then_some(n)
    });
    assert_eq!(fed, Some(100.0));
    assert!(matches!(
        e.push(
            s_feed,
            Samples::Raw {
                sample: SampleType::Cs16,
                bytes: &raw[..3],
            },
        ),
        Err(EngineError::PartSample { bytes: 3 })
    ));
}

/// A graph that plays the tone stack's test signal into the transmit sink.
fn transmit_plan(r: &Registry) -> GraphPlan {
    let mut plan = GraphPlan::new();
    let g = plan.add(r, "gen", "gen.stack").unwrap();
    let tx = plan.add(r, "tx", "tx.sink").unwrap();
    plan.connect(PortRef::new(g, 0), PortRef::new(tx, 0))
        .unwrap();
    plan.set("gen", "stack", &Value::Text("tone".into()))
        .unwrap();
    plan
}

#[test]
fn a_transmit_session_driven_by_a_test_signal_is_drained_by_the_host() {
    let e = engine();
    let s = e
        .start_graph(transmit_plan(&Registry::standard()), Input::Inside)
        .unwrap();
    let mut buf = vec![Complex32::default(); 4096];
    let mut total = 0;
    let first = wait_for(T, || {
        let r = e.transmit(s, &mut buf).unwrap()?;
        total += r.samples;
        Some(r)
    })
    .expect("transmit samples");
    // The tone stack plays at its own rate, 48 kHz.
    assert_eq!(first.rate_hz, 48e3);
    assert!(buf[..first.samples]
        .iter()
        .all(|c| (c.norm() - 0.5).abs() < 0.05));
    // The session paces the signal: about one second of it in one second.
    let t = std::time::Instant::now();
    while t.elapsed().as_secs_f64() < 1.0 {
        if let Some(r) = e.transmit(s, &mut buf).unwrap() {
            total += r.samples;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!((30e3..120e3).contains(&(total as f64)), "{total}");
}

#[test]
fn a_network_sink_of_a_graph_sends_the_samples_it_takes() {
    let net = FakeTransport::new();
    let mut listen = net
        .bind(&DatagramOptions::at("127.0.0.1:7355".parse().unwrap()))
        .unwrap();
    let e = engine_on(&net);
    let r = Registry::standard();
    let mut plan = GraphPlan::new();
    let sink = plan.add(&r, "net", "net.sink").unwrap();
    plan.input(PortRef::new(sink, 0)).unwrap();
    plan.set("net", "sample", &Value::Choice(1)).unwrap();
    let s_feed = e.open_feed(48e3, 100e6);
    e.start_graph(plan, Input::Feed(s_feed)).unwrap();
    let mut got = [0u8; 2048];
    let n = wait_for(T, || {
        let _ = e.push(s_feed, Samples::Iq(&tone(100)));
        listen
            .try_recv_from(&mut got)
            .ok()
            .flatten()
            .map(|(n, _)| n)
    })
    .expect("a datagram");
    assert_eq!(n, 400, "100 samples of two 16-bit values");
    let i = i16::from_le_bytes([got[0], got[1]]);
    assert_eq!(i, (0.5f32 * 32767.0).round() as i16);
}

#[test]
fn the_engine_validates_a_plan_with_its_registry_and_describes_a_stack_graph() {
    let e = fm_engine(&FakeTransport::new());
    let mut plan = GraphPlan::new();
    let chan = plan.add(e.registry(), "chan", "dsp.channel").unwrap();
    let tx = plan.add(e.registry(), "tx", "tx.sink").unwrap();
    plan.input(PortRef::new(chan, 0)).unwrap();
    plan.connect(PortRef::new(chan, 0), PortRef::new(tx, 0))
        .unwrap();
    let report = e.validate_graph(&plan, 48e3, 100e6).unwrap();
    assert!(report.ports.iter().any(|p| p.0 == "chan.out"));
    // The plan stays and can be validated again.
    assert!(e.validate_graph(&plan, 48e3, 100e6).is_ok());
    let d = e.stack_graph("fm").unwrap();
    assert!(!d.components.is_empty());
    assert!(d.edges.iter().any(|(from, _)| from == "input"));
    assert!(e.stack_graph("nope").is_err());
}

/// The mean frequency of `x` at `rate` Hz, from the phase step between samples.
fn mean_freq(x: &[Complex32], rate: f64) -> f64 {
    let s: Complex32 = x.windows(2).map(|w| w[1] * w[0].conj()).sum();
    f64::from(s.arg()) * rate / std::f64::consts::TAU
}

/// The frequency of the test signal of a tone radio with the `gen_*` settings `set`.
fn tone_freq(set: &[(&str, Value)]) -> f64 {
    let e = engine();
    let kind = e.find("driver.synth").unwrap();
    let mut v = defaults(&e, kind);
    for (key, value) in [("rate", Value::Hz(1e6)), ("quantize", Value::Bool(false))]
        .iter()
        .chain(set)
    {
        let id = v.schema().id(key).unwrap();
        v.set(id, value).unwrap();
    }
    let r = e.open_radio("synth", "synth:tone", &v).unwrap();
    let reg = Registry::standard();
    let mut plan = GraphPlan::new();
    let tx = plan.add(&reg, "tx", "tx.sink").unwrap();
    plan.input(PortRef::new(tx, 0)).unwrap();
    let s = e.start_graph(plan, Input::Radio(r)).unwrap();
    let mut buf = vec![Complex32::default(); 4096];
    let got = wait_for(T, || e.transmit(s, &mut buf).unwrap()).expect("samples");
    mean_freq(&buf[..got.samples], got.rate_hz)
}

#[test]
fn the_test_signal_driver_returns_its_gen_settings_to_the_generator() {
    let plain = tone_freq(&[]);
    assert!((plain - TONE_OFFSET_HZ).abs() < 500.0, "{plain}");
    let moved = tone_freq(&[("gen_offset_hz", Value::F64(25e3))]);
    assert!((moved - TONE_OFFSET_HZ - 25e3).abs() < 500.0, "{moved}");
}

/// A change of frequency by the host. From the order until the receiver has taken the frequency, the session shows no
/// result of the receiver, and it keeps its own fields. After that it shows what the receiver holds: the meter keeps
/// its power over the change, so the power is in view again.
#[test]
fn a_retune_by_the_host_hides_the_results_until_the_receiver_has_taken_it() {
    let e = engine();
    let v = defaults(&e, e.find("stack.meter").unwrap());
    let s_feed = e.open_feed(48e3, 100e6);
    let s = e
        .start_stack("meter", None, &v, Input::Feed(s_feed), None)
        .unwrap();
    let session = OwnerRef::Session(s);
    // The power and the number of changes that the receiver has taken, read from one state of the results.
    let seen = |o: &dyn sdrkit_core::Owner| {
        let res = o.results();
        let value = |key: &str| {
            let id = res.decl().field_id(key)?;
            res.fields().value(id).filter(|v| !v.is_none()).cloned()
        };
        (
            value("power"),
            value("session.retunes"),
            value("session.samples"),
        )
    };
    let before = wait_for(T, || {
        let _ = e.push(s_feed, Samples::Iq(&tone(480)));
        e.owner(session, |o| seen(o)).ok().filter(|s| s.0.is_some())
    })
    .expect("a power at the first frequency");
    assert_eq!(before.1, Some(Value::I64(0)));
    // The receiver has taken every sample that was pushed, so its power does not change again.
    let mut taken = -1.0;
    wait_for(T, || {
        std::thread::sleep(std::time::Duration::from_millis(60));
        let now = number(&e, session, "session.samples");
        (std::mem::replace(&mut taken, now) == now).then_some(())
    })
    .expect("the samples are taken");
    let power = field(&e, session, "power");

    // The order and the first look at the results are one step for the session, so the receiver cannot be in between.
    let at_once = e
        .owner(session, |o| {
            o.apply(sdrkit_core::SettingId(0), &Value::Hz(100.01e6))
                .unwrap();
            seen(o)
        })
        .unwrap();
    assert_eq!(
        at_once.0, None,
        "the power of the frequency before is in view"
    );
    assert_eq!(at_once.1, Some(Value::I64(0)));
    assert!(at_once.2.is_some(), "the fields of the session stay");

    // The power is in view only when the receiver has taken the change.
    let after = wait_for(T, || {
        let now = e.owner(session, |o| seen(o)).unwrap();
        assert!(now.0.is_none() || now.1 == Some(Value::I64(1)), "{now:?}");
        now.0.is_some().then_some(now)
    })
    .expect("the results of the receiver come back");
    assert_eq!(after.0, power);

    // The same frequency again is no change, and a change by a service leaves the results in view.
    let again = e
        .owner(session, |o| {
            o.apply(sdrkit_core::SettingId(0), &Value::Hz(100.01e6))
                .unwrap();
            seen(o)
        })
        .unwrap();
    assert_eq!(again.0, power);
}
