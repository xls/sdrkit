//! The receiver wrapper, decoders, the control port, host requests, profiling and saved state.

use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::{field, keys, Field, Published};
use sdrkit_core::{
    Complex32, ConfigError, HostRequest, InputInfo, Output, Receiver, Results, Setting,
    SettingError, SettingId, SettingSet, StateBlob, StateError, Station, Value, Values,
};
use sdrkit_graph::components::NullSink;
use sdrkit_graph::{
    apply_commands, Applied, Command, Component, ComponentInfo, Decoder, DecoderComponent, Format,
    GraphBuilder, GraphError, Helper, Io, Item, Kind, Outbox, Port, RunContext, StackGraph, Timed,
};
use std::sync::Arc;

/// A count of samples, as a typed item.
#[derive(Clone, Debug, PartialEq)]
struct Count(u64);
impl Item for Count {
    const KIND: &'static str = "test.count";
}

const STATION: Field<field::Text> = Field::new("station", "Station", "");
const GEN_SNR_DB: Setting<field::F64> =
    Setting::new("gen_snr_db", "SNR", "dB", 20.0).range(-10.0, 60.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the test stack.
    enum PeaksSetting {
        GenSnrDb = GEN_SNR_DB,
    }
}

const PEAKS_STATE: &str = "test.peaks";

/// Counts samples above 0.5 and emits the running count after each piece; reports frames_ok; keeps its count as state.
#[derive(Default)]
struct Peaks {
    above: u64,
    retunes: u32,
    helper: bool,
    station: String,
}

impl Decoder for Peaks {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "test.peaks"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<Count>("count")]
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if input.rate < 1000.0 {
            return Err(ConfigError::RateTooLow {
                need: 1000.0,
                got: input.rate,
            });
        }
        Ok(vec![Format::new(Kind::typed::<Count>(), 0.0)])
    }
    fn retune(&mut self, _: &Format) {
        self.retunes += 1;
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.above += x.iter().filter(|s| s.re > 0.5).count() as u64;
        if !x.is_empty() {
            out.items::<Count>(0).push(Timed {
                time: first as f64,
                value: Count(self.above),
            });
        }
    }
    fn results(&self) -> Published {
        Published::new(&[&keys::FRAMES_OK, &STATION])
    }
    fn report(&mut self, r: &mut Published) {
        r.set(&keys::FRAMES_OK, self.above as i64);
        r.set(&STATION, &self.station);
    }
    fn clear(&mut self) {
        self.above = 0;
    }
    fn helper(&self) -> Helper {
        if self.helper {
            Helper::WhenLive {
                batch: 1000,
                backlog: 100_000,
            }
        } else {
            Helper::Never
        }
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.station = ctx.station.callsign.clone();
    }
    fn save_state(&self) -> Option<StateBlob> {
        Some(StateBlob::new(
            PEAKS_STATE,
            1,
            self.above.to_le_bytes().to_vec(),
        ))
    }
    fn restore_state(&mut self, s: &StateBlob) -> Result<(), StateError> {
        let bytes = s.open(PEAKS_STATE, 1)?;
        let bytes: [u8; 8] = bytes.try_into().map_err(|_| StateError::Damaged)?;
        self.above = u64::from_le_bytes(bytes);
        Ok(())
    }
}

fn ones(n: usize) -> Vec<Complex32> {
    vec![Complex32::new(1.0, 0.0); n]
}

fn peaks_graph(helper: bool) -> sdrkit_graph::Graph {
    GraphBuilder::new()
        .add(
            "dec",
            DecoderComponent::new(Peaks {
                helper,
                ..Default::default()
            }),
        )
        .add("sink", NullSink::default())
        .input("dec")
        .connect("dec", "sink")
        .status_from("dec")
        .build()
        .unwrap()
}

/// Runs the stack command `key` with its default arguments, through the checked path.
fn run(rx: &mut StackGraph, key: &str) {
    let id = rx.commands().id(key).unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    owner::invoke(rx, id, &args).unwrap();
}

#[test]
fn a_mode_graph_checks_the_rate_routes_settings_and_guards_its_input() {
    let mut rx = StackGraph::builder("peaks", peaks_graph(false))
        .with_schema(PeaksSetting::schema())
        .min_rate(2000.0, "Peaks needs 2 kHz")
        .command("forget", sdrkit_graph::CommandTarget::of("dec", "clear"))
        .build()
        .unwrap();
    assert!(matches!(
        rx.configure(&InputInfo::new(1500.0, 0.0, 0.0)),
        Err(ConfigError::RateTooLow { need: 2000.0, .. })
    ));
    assert_eq!(rx.results().status().summary, "Peaks needs 2 kHz");
    rx.configure(&InputInfo::new(8000.0, 1e6, 1e6).recording())
        .unwrap();
    rx.set_station(&Station {
        callsign: "VK2ABC".into(),
        ..Station::default()
    });
    let mut out = Output::default();
    let mut x = ones(4000);
    x[7] = Complex32::new(f32::NAN, 0.0);
    x[9] = Complex32::new(1e12, 0.0);
    rx.feed(&x, &mut out);
    rx.refresh_results();
    // The NaN became 0 (not counted); the huge one was brought back to the limit (still above 0.5).
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(3999));
    assert_eq!(rx.results().fields().get(&keys::BAD_SAMPLES), Some(2));
    assert_eq!(rx.results().fields().get(&STATION), Some("VK2ABC"));
    // The generator's settings are accepted and left alone; routed keys reach the graph; a setting with no target and an
    // unknown key are refused.
    assert!(owner::apply(&mut rx, PeaksSetting::GenSnrDb.id(), &Value::F64(3.0)).is_ok());
    run(&mut rx, "forget");
    assert!(rx.schema().id("colour").is_err());
    rx.refresh_results();
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(0));
}

#[test]
fn clear_reaches_every_component_that_has_it_and_state_round_trips() {
    let g = GraphBuilder::new()
        .add("dec", DecoderComponent::new(Peaks::default()))
        .add("dec2", DecoderComponent::new(Peaks::default()))
        .add("sink", NullSink::default())
        .add("sink2", NullSink::default())
        .input("dec")
        .input("dec2")
        .connect("dec", "sink")
        .connect("dec2", "sink2")
        .status_from("dec")
        .build()
        .unwrap();
    let mut rx = StackGraph::builder("peaks", g)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .build()
        .unwrap();
    assert_eq!(rx.commands().len(), 1);
    rx.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&ones(500), &mut out);
    let saved = rx.save_state().expect("a state");
    run(&mut rx, "clear");
    let dec2 = rx.graph().component_results("dec2").unwrap();
    assert_eq!(dec2.fields().get(&keys::FRAMES_OK), Some(0));
    rx.refresh_results();
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(0));
    rx.restore_state(&saved).unwrap();
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(500));
    rx.feed(&ones(1000), &mut out);
    rx.refresh_results();
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(1500));
    // A state of another stack is refused.
    let mut other = StackGraph::builder("other", peaks_graph(false))
        .build()
        .unwrap();
    assert!(matches!(
        other.restore_state(&saved),
        Err(StateError::WrongOwner { .. })
    ));
}

/// Publishes the lengths of the passes it sees.
struct PassSizes(Published, Vec<usize>);
const SIZES: Field<field::Text> = Field::new("sizes", "Pass sizes", "");
impl Default for PassSizes {
    fn default() -> Self {
        PassSizes(Published::new(&[&SIZES]), Vec::new())
    }
}
impl Component for PassSizes {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.pass_sizes", vec![Port::iq("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        if n > 0 {
            self.1.push(n);
            self.0.set(&SIZES, &format!("{:?}", self.1));
        }
    }
    fn results(&self) -> Option<&Results> {
        Some(self.0.results())
    }
}

#[test]
fn fixed_passes_cut_the_input_the_same_whatever_the_feed() {
    let run = |piece: usize| {
        let g = GraphBuilder::new()
            .add("p", PassSizes::default())
            .input("p")
            .status_from("p")
            .build()
            .unwrap();
        let mut rx = StackGraph::builder("passes", g)
            .fixed_passes(256)
            .build()
            .unwrap();
        rx.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
            .unwrap();
        let x = ones(2048 + 100);
        let mut out = Output::default();
        for p in x.chunks(piece) {
            rx.feed(p, &mut out);
        }
        rx.refresh_results();
        rx.results().fields().get(&SIZES).unwrap().to_string()
    };
    let want = format!("{:?}", vec![256; 8]);
    for piece in [1, 100, 256, 1000, 4096] {
        assert_eq!(run(piece), want, "pieces of {piece}");
    }
}

#[test]
fn a_decoder_on_a_helper_thread_returns_the_same_count_live() {
    let mut rx = StackGraph::builder("peaks", peaks_graph(true))
        .build()
        .unwrap();
    rx.configure(&InputInfo::new(8000.0, 0.0, 0.0)).unwrap(); // live
    let mut out = Output::default();
    for _ in 0..10 {
        rx.feed(&ones(500), &mut out);
    }
    // Five batches of 1000 went to helpers, one at a time; wait for the last to come back.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        rx.feed(&[], &mut out);
        rx.refresh_results();
        if rx.results().fields().get(&keys::FRAMES_OK) == Some(5000) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{:?} {:?}",
            rx.results().fields(),
            rx.results().status()
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// A sender of commands and host requests: on each tick it commands the controlled component to change its gain and requests a
/// retune from the host.
struct Supervisor;
impl Component for Supervisor {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.supervisor",
            vec![Port::iq("in")],
            vec![
                Port::command("control"),
                Port::typed::<HostRequest>("requests"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<Command>(), 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        if io.tick() {
            let t = io.input(0).end() as f64;
            io.items_out::<Command>(0).push(Timed {
                time: t,
                value: Command::new(GainSetting::Gain.id(), Value::F64(2.0)),
            });
            io.items_out::<HostRequest>(1).push(Timed {
                time: t,
                value: HostRequest::retune(145.8e6, "test"),
            });
        }
    }
}

const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "", 1.0).range(0.0, 10.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the gain component.
    enum GainSetting {
        Gain = GAIN,
    }
}

/// Multiplies by a gain set through its control port.
struct Gain(f32);
impl Component for Gain {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::values_of([(GainSetting::Gain, Value::F64(f64::from(self.0)))])
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.gain",
            vec![Port::iq("in"), Port::command("control")],
            vec![Port::iq("out")],
        )
        .with_settings(GainSetting::schema())
    }
    fn configure(&mut self, i: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![i[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let _ = apply_commands(self, io, 1);
        let g = self.0;
        let x = io.iq(0);
        io.iq_out(0).extend(x.iter().map(|s| s * g));
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match GainSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            GainSetting::Gain => self.0 = GAIN.read(id, v)? as f32,
        }
        Ok(Applied::Live)
    }
}

#[test]
fn commands_flow_on_a_feedback_edge_and_requests_reach_the_host() {
    let mut g = GraphBuilder::new()
        .add("gain", Gain(1.0))
        .add("sup", Supervisor)
        .add("sink", NullSink::default())
        .connect("input", "gain.in")
        .connect("gain", "sup")
        .feedback("sup.control", "gain.control")
        .connect("gain", "sink")
        .output("sup.requests")
        .clock(0.01)
        .build()
        .unwrap();
    g.configure(&InputInfo::new(1000.0, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    g.feed(&ones(10), &mut out); // one clock period: the supervisor sends at its end
    assert_eq!(out.requests, [HostRequest::retune(145.8e6, "test")]);
    assert_eq!(g.buffer("gain.out").unwrap().data.len(), 10);
    out.clear();
    g.feed(&ones(5), &mut out);
    // The command acts from the next pass on.
    match &g.buffer("gain.out").unwrap().data {
        sdrkit_graph::stream::Data::Iq(v) => assert!(v.iter().all(|s| s.re == 2.0)),
        d => panic!("{d:?}"),
    }
    let mut p = Vec::new();
    sdrkit_core::Staged::component_loads(&g, &mut p);
    assert_eq!(p.len(), 3);
    assert!(p.iter().all(|c| c.calls >= 2), "{p:?}");
}

#[test]
fn settings_are_applied_strictly_at_construction() {
    let schema = PeaksSetting::schema();
    let v = Values::defaults(&schema);
    let rx = StackGraph::builder("peaks", peaks_graph(false))
        .with_schema(Arc::clone(&schema))
        .with_settings(&v)
        .build()
        .unwrap();
    assert_eq!(rx.values(), Some(&v));

    // A value that the stack refuses fails the build.
    const WIDE: Setting<field::F64> =
        Setting::new("gen_snr_db", "SNR", "dB", 20.0).range(-100.0, 100.0, 0.0);
    sdrkit_core::settings! {
        enum WideSetting {
            Snr = WIDE,
        }
    }
    let mut given = Values::defaults(&WideSetting::schema());
    given.set(WideSetting::Snr.id(), &Value::F64(90.0)).unwrap();
    let e = StackGraph::builder("peaks", peaks_graph(false))
        .with_schema(schema)
        .with_settings(&given)
        .build()
        .unwrap_err();
    assert!(
        matches!(&e, GraphError::Setting { key, error: SettingError::OutOfRange { .. } } if key == "gen_snr_db"),
        "{e}"
    );
}

#[test]
fn a_retune_runs_the_retune_command() {
    let mut rx = StackGraph::builder("peaks", peaks_graph(false))
        .on_retune(sdrkit_graph::CommandTarget::of("dec", "clear"))
        .unwrap()
        .build()
        .unwrap();
    assert!(matches!(
        StackGraph::builder("peaks", peaks_graph(false))
            .on_retune(sdrkit_graph::CommandTarget::of("dec", "nothing")),
        Err(GraphError::UnresolvedCommand { .. })
    ));
    rx.configure(&InputInfo::new(8000.0, 1e6, 1e6).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&ones(1000), &mut out);
    rx.refresh_results();
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(1000));
    rx.retuned(&InputInfo::new(8000.0, 2e6, 2e6).recording());
    rx.refresh_results();
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(0));
}

#[test]
fn a_live_minimum_rate_refuses_a_radio_but_a_recording_at_a_lower_rate_decodes() {
    // One 250 kHz LoRa channel needs 250 ksps from any input, and the host requests 1 Msps from a live radio.
    let mut rx = StackGraph::builder("peaks", peaks_graph(false))
        .min_rate(250e3, "LoRa needs a sample rate of at least 250 ksps")
        .min_rate_live(1e6, "a live radio must give at least 1 Msps")
        .build()
        .unwrap();
    let live = InputInfo::new(600e3, 868e6, 868e6);
    assert_eq!(
        rx.configure(&live),
        Err(ConfigError::RateTooLow {
            need: 1e6,
            got: 600e3
        })
    );
    assert_eq!(
        rx.results().status().summary,
        "a live radio must give at least 1 Msps"
    );
    assert!(!rx.ready());
    rx.configure(&live.recording()).unwrap();
    assert!(rx.ready());
    let mut out = Output::default();
    rx.feed(&ones(60_000), &mut out); // 0.1 s, one report interval of the decoder
    rx.refresh_results();
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(60000));
    // Below the minimum for every input, the base message is the reason, also for a live radio.
    let low = InputInfo::new(200e3, 868e6, 868e6);
    assert!(rx.configure(&low.recording()).is_err());
    assert_eq!(
        rx.results().status().summary,
        "LoRa needs a sample rate of at least 250 ksps"
    );
    assert!(rx.configure(&low).is_err());
    assert_eq!(
        rx.results().status().summary,
        "LoRa needs a sample rate of at least 250 ksps"
    );
    rx.configure(&InputInfo::new(1e6, 868e6, 868e6)).unwrap();
    // A live minimum below the base minimum changes nothing.
    rx.set_min_rate_live(100e3);
    assert!(rx.configure(&InputInfo::new(200e3, 868e6, 868e6)).is_err());
}

/// A frame of an input without a rate, made by [`Framer`].
#[derive(Clone, Debug, PartialEq)]
struct Frame(u64);
impl Item for Frame {
    const KIND: &'static str = "test.frame";
}

/// Makes one frame for every `every` input samples, at the frame's signal time. Its output has no rate.
struct Framer {
    every: u64,
    rate: f64,
    n: u64,
}
impl Component for Framer {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.framer",
            vec![Port::iq("in")],
            vec![Port::typed::<Frame>("out")],
        )
    }
    fn configure(&mut self, i: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = i[0].rate;
        Ok(vec![Format::new(Kind::typed::<Frame>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        for k in 0..io.input_len(0) as u64 {
            let at = first + k;
            if at.is_multiple_of(self.every) {
                self.n += 1;
                io.items_out::<Frame>(0).push(Timed {
                    time: at as f64 / self.rate,
                    value: Frame(self.n),
                });
            }
        }
    }
}

/// Counts frames and its own reports. With `sleep_ms` it is slow, so a helper thread falls behind a live radio.
#[derive(Default)]
struct FrameCount {
    frames: u64,
    reports: u64,
    sleep_ms: u64,
}
impl Decoder for FrameCount {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    type Input = Timed<Frame>;
    fn name(&self) -> &'static str {
        "test.frame_count"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<Frame>("out")]
    }
    fn configure(&mut self, _: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Frame>(), 0.0)])
    }
    fn process(&mut self, x: &[Timed<Frame>], _: u64, _: &mut Outbox) {
        if self.sleep_ms > 0 && !x.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(self.sleep_ms));
        }
        self.frames += x.len() as u64;
    }
    fn results(&self) -> Published {
        Published::new(&[&keys::FRAMES_OK])
    }
    fn report(&mut self, r: &mut Published) {
        self.reports += 1;
        r.set(&keys::FRAMES_OK, self.frames as i64);
    }
    fn report_interval_s(&self) -> f64 {
        1.0
    }
    fn helper(&self) -> Helper {
        if self.sleep_ms > 0 {
            Helper::WhenLive {
                batch: 1,
                backlog: 2,
            }
        } else {
            Helper::Never
        }
    }
}

fn frame_graph(dec: FrameCount, clock_s: Option<f64>) -> sdrkit_graph::Graph {
    let b = GraphBuilder::new()
        .add(
            "framer",
            Framer {
                every: 800,
                rate: 0.0,
                n: 0,
            },
        )
        .add("count", DecoderComponent::new(dec))
        .input("framer")
        .connect("framer", "count");
    match clock_s {
        Some(c) => b.clock(c),
        None => b,
    }
    .status_from("count")
    .build()
    .unwrap()
}

fn reports(g: &sdrkit_graph::Graph) -> u64 {
    g.component_as::<DecoderComponent<FrameCount>>("count")
        .and_then(|c| c.decoder())
        .map_or(0, |d| d.reports)
}

#[test]
fn a_decoder_without_an_input_rate_reports_once_a_second_of_signal_not_on_every_call() {
    // 10 s of signal at 8 kHz, one frame every 0.1 s, fed in 100 calls of one frame each. The interval is 1 s, so the frame
    // times 0, 1, 2 ... 9 s report: 10 reports, where every call reported before (100).
    let mut g = frame_graph(FrameCount::default(), None);
    g.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    for _ in 0..100 {
        g.feed(&ones(800), &mut out);
    }
    assert_eq!(reports(&g), 10);
    g.refresh_results();
    assert_eq!(g.results().fields().get(&keys::FRAMES_OK), Some(91));

    // The same signal in calls of 0.05 s (half of them hold no frame) produces the same reports.
    let mut g = frame_graph(FrameCount::default(), None);
    g.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
        .unwrap();
    for _ in 0..200 {
        g.feed(&ones(400), &mut out);
    }
    assert_eq!(reports(&g), 10);

    // With a graph clock of 0.5 s, the ticks decide: the first frame, then 20 periods in 10 s.
    let mut g = frame_graph(FrameCount::default(), Some(0.5));
    g.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
        .unwrap();
    for _ in 0..100 {
        g.feed(&ones(800), &mut out);
    }
    assert_eq!(reports(&g), 21);
}

#[test]
fn a_decoder_on_a_helper_thread_publishes_the_items_it_dropped() {
    // A live radio does not wait: a decoder that needs 30 ms per frame, fed 40 frames at once with room for 2 waiting, drops
    // frames. Every frame fed is decoded, dropped or still waiting.
    let mut g = frame_graph(
        FrameCount {
            sleep_ms: 30,
            ..Default::default()
        },
        None,
    );
    g.configure(&InputInfo::new(8000.0, 0.0, 0.0)).unwrap(); // live
    let mut out = Output::default();
    for _ in 0..40 {
        g.feed(&ones(800), &mut out);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        g.feed(&[], &mut out);
        let c = g
            .component_as::<DecoderComponent<FrameCount>>("count")
            .unwrap();
        if !c.busy() && c.decoder().is_some() {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let c = g
        .component_as::<DecoderComponent<FrameCount>>("count")
        .unwrap();
    let (decoded, dropped) = (c.decoder().unwrap().frames, c.dropped());
    assert!(dropped > 0, "nothing was dropped");
    assert!(decoded + dropped <= 40 && 40 - decoded - dropped <= 2);
    g.refresh_results();
    assert_eq!(
        g.results().fields().get(&keys::DROPPED),
        Some(dropped as i64)
    );

    // In line (a recording) nothing is dropped and the key is not published.
    let mut g = frame_graph(
        FrameCount {
            sleep_ms: 1,
            ..Default::default()
        },
        None,
    );
    g.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
        .unwrap();
    for _ in 0..5 {
        g.feed(&ones(800), &mut out);
    }
    g.refresh_results();
    assert_eq!(g.results().fields().get(&keys::DROPPED), None);
}

const FACTOR: Setting<field::F64> = Setting::new("factor", "Factor", "", 1.0).range(1.0, 2.0, 1.0);

sdrkit_core::settings! {
    /// The settings of the halver.
    enum HalverSetting {
        Factor = FACTOR,
    }
}

/// Halves or keeps the rate (setting `factor`, 1 or 2): a change of factor changes the output rate. A new factor waits for the
/// next configure, so the outputs of a pass keep the format they were configured with.
struct Halver {
    factor: usize,
    next: usize,
    phase: usize,
}
impl Component for Halver {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::values_of([(HalverSetting::Factor, Value::F64(self.next as f64))])
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.halver",
            vec![Port::iq("in"), Port::command("control")],
            vec![Port::iq("out")],
        )
        .with_settings(HalverSetting::schema())
    }
    fn configure(&mut self, i: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.phase = 0;
        self.factor = self.next;
        Ok(vec![Format {
            rate: i[0].rate / self.factor as f64,
            ..i[0].clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let _ = sdrkit_graph::apply_commands_reformat(self, io, 1);
        let x = io.iq(0);
        let f = self.factor;
        let mut phase = self.phase;
        let out = io.iq_out(0);
        for s in x {
            if phase == 0 {
                out.push(*s);
            }
            phase = (phase + 1) % f;
        }
        self.phase = phase;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match HalverSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            HalverSetting::Factor => {
                let f = FACTOR.read(id, v)? as usize;
                let changed = f != self.next;
                self.next = f;
                Ok(if changed {
                    Applied::Reformat
                } else {
                    Applied::Live
                })
            }
        }
    }
}

/// Measures the band of its input and, at the first tick, sends `factor` 2 to the halver (an automatic channel width).
struct Chooser {
    sent: bool,
    rate_seen: Vec<f64>,
}
impl Component for Chooser {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.chooser",
            vec![Port::iq("in")],
            vec![Port::command("control")],
        )
    }
    fn configure(&mut self, i: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate_seen.push(i[0].rate);
        Ok(vec![Format::new(Kind::typed::<Command>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if io.tick() && !self.sent {
            self.sent = true;
            io.items_out::<Command>(0).push(Timed {
                time: 0.0,
                value: Command::new(HalverSetting::Factor.id(), Value::F64(2.0)),
            });
        }
    }
}

#[test]
fn a_command_that_changes_a_rate_reconfigures_the_graph_from_the_next_pass() {
    // 8 kHz in; the chooser requests half the rate at the end of the first 0.1 s period. The halver gets the command at the
    // start of the second pass, and the graph configures it and the chooser again at the end of that pass: 4 kHz from the
    // third pass on, so 800 + 800 + 400 + 400 samples leave the halver in four passes of 800.
    let mut g = GraphBuilder::new()
        .add(
            "halver",
            Halver {
                factor: 1,
                next: 1,
                phase: 0,
            },
        )
        .add(
            "chooser",
            Chooser {
                sent: false,
                rate_seen: vec![],
            },
        )
        .add("sink", sdrkit_graph::components::NullSink::default())
        .input("halver.in")
        .connect("halver.out", "chooser.in")
        .connect("halver.out", "sink")
        .feedback("chooser.control", "halver.control")
        .clock(0.1)
        .build()
        .unwrap();
    g.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    let mut lens = vec![];
    for _ in 0..4 {
        g.feed(&ones(800), &mut out);
        lens.push(g.buffer("halver.out").unwrap().len());
    }
    assert_eq!(lens, [800, 800, 400, 400]);
    assert_eq!(g.format("halver.out").unwrap().rate, 4000.0);
    let c = g.component_as::<Chooser>("chooser");
    // The chooser does not open itself to the graph's owner.
    assert!(c.is_none());
    assert!(g.component("chooser").is_some());
}

/// Changes its own output rate by itself after `after` samples, through `Io::request_reformat` (a natural rate found while
/// running). With `refuse`, it refuses the new configuration.
struct SelfRate {
    after: u64,
    seen: u64,
    factor: f64,
    refuse: bool,
}
impl Component for SelfRate {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.self_rate",
            vec![Port::iq("in")],
            vec![Port::iq("out")],
        )
    }
    fn configure(&mut self, i: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if self.refuse && self.factor > 1.0 {
            return Err(ConfigError::BandwidthNotSupported { got: 2000.0 });
        }
        Ok(vec![Format {
            rate: i[0].rate / self.factor,
            ..i[0].clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        io.iq_out(0).extend_from_slice(x);
        self.seen += x.len() as u64;
        if self.seen >= self.after && self.factor == 1.0 {
            self.factor = 4.0;
            io.request_reformat();
        }
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[test]
fn a_component_that_finds_a_new_natural_rate_reconfigures_what_follows() {
    let build = |refuse| {
        GraphBuilder::new()
            .add(
                "front",
                SelfRate {
                    after: 1000,
                    seen: 0,
                    factor: 1.0,
                    refuse,
                },
            )
            .add(
                "chooser",
                Chooser {
                    sent: true,
                    rate_seen: vec![],
                },
            )
            .add("sink", sdrkit_graph::components::NullSink::default())
            .input("front")
            .connect("front", "chooser")
            .connect("chooser.control", "sink")
            .build()
            .unwrap()
    };
    let mut g = build(false);
    g.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    g.feed(&ones(600), &mut out);
    assert_eq!(g.format("front.out").unwrap().rate, 8000.0);
    g.feed(&ones(600), &mut out); // the decision falls in this pass
    assert_eq!(g.format("front.out").unwrap().rate, 2000.0);
    // The later component was configured again with the new rate.
    let seen = &g.component_as::<SelfRate>("front").unwrap().seen;
    assert_eq!(*seen, 1200);
    g.feed(&ones(600), &mut out);
    assert!(g.results().status().summary.is_empty());

    // A component that refuses its new configuration stops the graph, and the status says why.
    let mut g = build(true);
    g.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
        .unwrap();
    g.feed(&ones(1200), &mut out);
    assert_eq!(g.results().status().lock_state, sdrkit_core::LockState::Off);
    assert!(
        g.results().status().summary.contains("no channel"),
        "{}",
        g.results().status().summary
    );
}

#[test]
fn the_owner_of_a_graph_reaches_a_running_component_between_passes() {
    // Through the StackGraph, and through the receiver as the host holds it.
    let mut rx = StackGraph::builder("peaks", peaks_graph(false))
        .build()
        .unwrap();
    rx.configure(&InputInfo::new(8000.0, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&ones(100), &mut out);
    let dec = rx
        .graph_mut()
        .component_as_mut::<DecoderComponent<Peaks>>("dec")
        .and_then(|c| c.decoder_mut())
        .unwrap();
    assert_eq!(dec.above, 100);
    dec.above = 1000;
    rx.feed(&ones(100), &mut out);
    let boxed: &mut dyn Receiver = &mut rx;
    let g = boxed
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<sdrkit_graph::Graph>())
        .unwrap();
    let c = g.component_as::<DecoderComponent<Peaks>>("dec").unwrap();
    assert_eq!(c.decoder().unwrap().above, 1100);
    // The wrong type, an unknown id and a closed component give nothing.
    assert!(g
        .component_as::<DecoderComponent<FrameCount>>("dec")
        .is_none());
    assert!(g.component_as::<NullSink>("nothing").is_none());
    assert!(g.component_as::<NullSink>("sink").is_none());
    assert!(g.component_mut("sink").is_some());
}

/// Notes which codec plug-ins the run context offers.
#[derive(Default)]
struct CodecUser {
    has_ambe2: bool,
}
impl Component for CodecUser {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.codec_user", vec![Port::iq("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, _: &mut Io) {}
    fn attach(&mut self, ctx: &RunContext) {
        self.has_ambe2 = ctx.codecs.has(sdrkit_core::FourCc::new("amb2"));
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

#[test]
fn the_hosts_codec_plug_ins_reach_every_component_and_stay_across_a_new_station() {
    use sdrkit_core::codec::{AudioCodec, CodecError, CodecSet, Pcm};
    struct Silence;
    impl AudioCodec for Silence {
        fn decode(&mut self, _: &[u8], _: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
            pcm.samples.extend([0; 160]);
            Ok(())
        }
    }
    let g = GraphBuilder::new()
        .add("user", CodecUser::default())
        .input("user")
        .build()
        .unwrap();
    let mut set = CodecSet::default();
    set.register(
        sdrkit_core::FourCc::new("amb2"),
        Some("codewords"),
        Arc::new(|_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Silence) as Box<dyn AudioCodec>)
        }),
    );
    let mut rx = StackGraph::builder("dvoice", g).build().unwrap();
    let has = |rx: &StackGraph| {
        rx.graph()
            .component_as::<CodecUser>("user")
            .unwrap()
            .has_ambe2
    };
    assert!(!has(&rx));
    rx.set_codecs(set);
    assert!(has(&rx));
    rx.set_station(&Station::default());
    rx.configure(&InputInfo::new(48e3, 0.0, 0.0)).unwrap();
    assert!(has(&rx));
    assert!(rx
        .graph()
        .context()
        .codecs
        .find(sdrkit_core::FourCc::new("amb2"), "codewords")
        .is_some());
    rx.set_codecs(CodecSet::default());
    assert!(!has(&rx));
}

/// A decoder with a command of its own (`mark`), which reads the host's clock, random source and storage when attached, and
/// declares a cap of 8 items on its output.
#[derive(Default)]
struct Marks {
    marks: Vec<i64>,
    attached: Option<(f64, u64, Vec<u8>)>,
}

const AT: Setting<field::I64> = Setting::new("at", "At", "", 0).range(0.0, 100.0, 1.0);
const MARK: sdrkit_core::Command = sdrkit_core::Command {
    args: &[&AT],
    ..sdrkit_core::Command::new("mark", "Mark")
};

impl Decoder for Marks {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "test.marks"
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<Count>("count")]
    }
    fn output_cap(&self, output: usize) -> Option<usize> {
        (output == 0).then_some(8)
    }
    fn configure(&mut self, _: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Count>(), 0.0)])
    }
    fn process(&mut self, _: &[Complex32], first: u64, out: &mut Outbox) {
        out.items::<Count>(0).push(Timed {
            time: first as f64,
            value: Count(self.marks.len() as u64),
        });
    }
    fn commands(&self) -> &'static [&'static sdrkit_core::Command] {
        &[&MARK]
    }
    fn invoke(
        &mut self,
        id: sdrkit_core::CommandId,
        args: &Values,
    ) -> Result<Option<sdrkit_core::Record>, sdrkit_core::CommandError> {
        match id.0 {
            0 => {
                self.marks.push(args.get(&AT));
                Ok(None)
            }
            _ => Err(sdrkit_core::CommandError::NoSuchId { id }),
        }
    }
    fn clear(&mut self) {
        self.marks.clear();
    }
    fn attach(&mut self, ctx: &RunContext) {
        let path = sdrkit_core::StorePath::new("tables/codes.bin").unwrap();
        let bytes = ctx.storage.read(&path).unwrap_or_default();
        self.attached = Some((ctx.clock.now(), ctx.random.u64(), bytes));
    }
}

#[test]
fn a_decoder_returns_its_own_commands_after_clear() {
    let mut c = DecoderComponent::new(Marks::default());
    let keys: Vec<_> = (0..c.commands().len())
        .map(|i| {
            c.commands()
                .def(sdrkit_core::CommandId(i as u16))
                .unwrap()
                .key
                .to_string()
        })
        .collect();
    assert_eq!(keys, ["clear", "mark"]);
    let mark = c.commands().id("mark").unwrap();
    let mut given = c.commands().def(mark).unwrap().new_args();
    given
        .set_value(sdrkit_core::FieldId(0), &Value::I64(7))
        .unwrap();
    let args = c.commands().check(mark, &given).unwrap();
    assert_eq!(c.invoke(mark, &args), Ok(None));
    assert_eq!(c.decoder().unwrap().marks, [7]);
    let clear = c.commands().id("clear").unwrap();
    let none = c
        .commands()
        .check(clear, &c.commands().def(clear).unwrap().new_args())
        .unwrap();
    c.invoke(clear, &none).unwrap();
    assert!(c.decoder().unwrap().marks.is_empty());
    assert_eq!(
        c.invoke(sdrkit_core::CommandId(2), &none),
        Err(sdrkit_core::CommandError::NoSuchId {
            id: sdrkit_core::CommandId(2)
        })
    );
}

#[test]
fn a_decoder_reads_the_hosts_facilities_when_attached() {
    let storage = Arc::new(sdrkit_core::MemStorage::new());
    let path = sdrkit_core::StorePath::new("tables/codes.bin").unwrap();
    sdrkit_core::Storage::write(&*storage, &path, &[1, 2, 3]).unwrap();
    let mut g = GraphBuilder::new()
        .add("dec", DecoderComponent::new(Marks::default()))
        .add("sink", NullSink::default())
        .input("dec")
        .connect("dec", "sink")
        .build()
        .unwrap();
    let random = sdrkit_core::Random::fixed(9);
    let expected = sdrkit_core::Random::fixed(9).u64();
    g.set_context(RunContext {
        storage,
        clock: sdrkit_core::Clock::fixed(1_700_000_000.0),
        random,
        ..RunContext::default()
    });
    let c = g.component_as::<DecoderComponent<Marks>>("dec").unwrap();
    let (now, r, bytes) = c.decoder().unwrap().attached.clone().unwrap();
    assert_eq!((now, r, bytes), (1_700_000_000.0, expected, vec![1, 2, 3]));
}
