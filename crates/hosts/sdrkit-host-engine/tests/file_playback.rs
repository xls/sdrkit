//! A recording plays whole: as fast as the stacks take it or paced to real time, with a stack slower than real time
//! beside it, every sample reaches the stacks and the decoded frames are the same.

mod common;

use common::{defaults, number, wait_for, METER_DESC};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{
    Complex32, ConfigError, Facilities, GenConfig, InputInfo, MemStorage, Output, Owner,
    Published, Random, Receiver, Results, ResultsDecl, SettingError, SettingId, StackDescriptor, Storage,
    StorePath, Value, Values,
};
use sdrkit_host_engine::{Engine, Input, OwnerRef, SessionId, SessionState, Setup, STATE};
use std::sync::Arc;
use std::time::Duration;

/// The rate of the recording: the ring of the file radio and the queues of its sessions hold less than the file.
const RATE: f64 = 384e3;
/// The length of the recording.
const SECONDS: f64 = 12.0;
/// The centre of the recording.
const CENTRE: f64 = 446.1e6;
/// The time that the slow stack takes for each block, more than the block lasts at [`RATE`].
const NAP: Duration = Duration::from_millis(60);

/// A stack that is slower than real time: it sleeps on each block.
struct Slow {
    results: Published,
    values: Values,
}

impl Receiver for Slow {
    fn configure(&mut self, _: &InputInfo) -> Result<(), ConfigError> {
        Ok(())
    }
    fn feed(&mut self, _: &[Complex32], _: &mut Output) {
        std::thread::sleep(NAP);
    }
}

impl Owner for Slow {
    fn schema(&self) -> &Arc<sdrkit_core::Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<(), SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.results().decl()
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
}

static SLOW: StackDescriptor = StackDescriptor {
    id: "slow",
    name: "Slow",
    submodes: &[],
    make_receiver: |cfg| {
        Ok(Box::new(Slow {
            results: Published::from_decl(Arc::new(ResultsDecl::new())),
            values: cfg.settings.clone(),
        }))
    },
    benchmarks: None,
    ..METER_DESC
};

fn dmr() -> &'static StackDescriptor {
    sdrkit_host_catalog::all()
        .into_iter()
        .find(|d| d.id == "dmr")
        .unwrap()
}

/// Writes `SECONDS` of the test signal of the DMR stack as a SigMF recording, and returns its samples.
fn write_recording(storage: &MemStorage) -> u64 {
    let d = dmr();
    let make = d.make_generator.unwrap();
    let mut g = make(&GenConfig::new(RATE, Values::defaults(&(d.settings)())).with_seed(1)).unwrap();
    let total = (RATE * SECONDS) as usize;
    let mut iq = vec![Complex32::default(); total];
    assert_eq!(g.generate(&mut iq), total);
    let data: Vec<u8> = iq
        .iter()
        .flat_map(|c| c.re.to_le_bytes().into_iter().chain(c.im.to_le_bytes()))
        .collect();
    let meta = format!(
        r#"{{"global":{{"core:datatype":"cf32_le","core:sample_rate":{RATE}}},
            "captures":[{{"core:sample_start":0,"core:frequency":{CENTRE}}}]}}"#
    );
    storage
        .write(&StorePath::new("dmr.sigmf-meta").unwrap(), meta.as_bytes())
        .unwrap();
    storage
        .write(&StorePath::new("dmr.sigmf-data").unwrap(), &data)
        .unwrap();
    total as u64
}

fn engine(storage: MemStorage) -> Engine {
    let facilities = Facilities {
        storage: Arc::new(storage),
        clock: sdrkit_host_engine::system_clock(),
        random: Random::fixed(7),
        codecs: Arc::default(),
    };
    Engine::new(Setup {
        components: sdrkit_host_catalog::registry(&facilities),
        facilities,
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![dmr(), &SLOW],
        families: Vec::new(),
    })
    .unwrap()
}

fn stopped(e: &Engine, s: SessionId) -> bool {
    e.owner(OwnerRef::Session(s), |o| o.results().fields().get(&STATE))
        .ok()
        .flatten()
        == Some(SessionState::Stopped)
}

/// What a playback gave: the kinds of the DMR events in order, and the samples that the DMR stack took.
#[derive(Debug, PartialEq)]
struct Played {
    events: Vec<String>,
    samples: f64,
}

/// Plays the recording through the DMR stack with the slow stack beside it, as fast as they take it or in real time.
fn play(fast: bool) -> Played {
    let storage = MemStorage::new();
    let total = write_recording(&storage);
    let e = engine(storage);
    let kind = e.find("driver.file").unwrap();
    let mut v = defaults(&e, kind);
    v.set(v.schema().id("benchmark").unwrap(), &Value::Bool(fast))
        .unwrap();
    let radio = e.open_radio("file", "dmr.sigmf-meta", &v).unwrap();
    // The recording does not start before a stack reads it.
    std::thread::sleep(Duration::from_millis(300));
    let start = |id: &str| {
        let params = defaults(&e, e.find(&format!("stack.{id}")).unwrap());
        e.start_stack(id, None, &params, Input::Radio(radio), None)
            .unwrap()
    };
    let (rx, slow) = (start("dmr"), start("slow"));
    let mut events = Vec::new();
    let take = |events: &mut Vec<String>| {
        let mut got = Vec::new();
        e.take_events(OwnerRef::Session(rx), &mut got).unwrap();
        events.extend(
            got.iter()
                .map(|ev| ev.kind().to_owned())
                .filter(|k| k.starts_with("dvoice.")),
        );
    };
    // The sessions end on their own once the recording has played and their stacks have taken every block.
    let ended = wait_for(900.0, || {
        take(&mut events);
        (stopped(&e, rx) && stopped(&e, slow)).then_some(())
    });
    assert!(ended.is_some(), "the sessions end at the end of the recording");
    assert!(e.settle(Duration::from_secs(30)));
    take(&mut events);
    let count = |key: &str| [rx, slow].map(|s| number(&e, OwnerRef::Session(s), key));
    let [samples, slow_samples] = count("session.samples");
    assert_eq!(samples, total as f64, "every sample is played");
    // The slow stack started after the first block: it reads from there on, and paces the rest.
    assert!(slow_samples > 0.0 && slow_samples <= samples);
    assert_eq!(count("session.dropped"), [0.0; 2], "no sample is lost");
    let played = Played { events, samples };
    for s in [rx, slow] {
        e.close_session(s).unwrap();
    }
    e.close_radio(radio).unwrap();
    played
}

#[test]
fn a_recording_gives_the_same_frames_fast_and_in_real_time_with_a_slow_stack() {
    let fast = play(true);
    assert!(!fast.events.is_empty(), "the DMR stack decodes the recording");
    let real = play(false);
    assert_eq!(fast, real);
}
