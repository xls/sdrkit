//! Copies, allocations and memory: no allocation once warmed up, bounded buffers, in-place work, capped item streams.

use sdrkit_core::publish::{field, keys, Field, Published};
use sdrkit_core::Owner;
use sdrkit_core::{
    Complex32, ConfigError, EventDef, InputInfo, Output, Receiver, Record, Results, Value,
};
use sdrkit_graph::components::*;
use sdrkit_graph::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::f64::consts::TAU;
use std::sync::Arc;

/// Whether the results declare `key` and hold a value for it.
fn has_field(r: &Results, key: &str) -> bool {
    r.decl()
        .field_id(key)
        .and_then(|id| r.fields().value(id))
        .is_some_and(|v| *v != Value::None)
}

/// Counts allocations made by the current thread.
struct Counting;
thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: forwards to the system allocator with the same layout.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `alloc` above with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: as for `alloc` and `dealloc`.
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

fn allocs() -> u64 {
    ALLOCS.with(|c| c.get())
}

fn info(rate: f64) -> InputInfo {
    InputInfo {
        rate_hz: rate,
        centre_hz: 100e6,
        tuned_hz: 100e6,
        live: true,
    }
}

fn am(n: usize, rate: f64) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            let t = i as f64 / rate;
            let e = 0.2 * (1.0 + 0.5 * (TAU * 1000.0 * t).cos());
            Complex32::from_polar(e as f32, (TAU * (30e3 * t).fract()) as f32)
        })
        .collect()
}

fn receiver() -> Graph {
    GraphBuilder::new()
        .add("dc", DcRemoval::default())
        .add("nb", NoiseBlanker::default())
        .add(
            "chan",
            Channel::new(10e3, 24e3).unwrap().with_offset(30e3).unwrap(),
        )
        .add("demod", AmDemod::default())
        .add("agc", AgcComponent::default())
        .add("deemph", DeemphasisComponent::new(0.0))
        .add("audio", AudioResampler::new(48e3).unwrap())
        .add("spec", Spectrum::new(SpectrumSize::S1024, 30.0).unwrap())
        .add("meter", PowerMeterSink::default())
        .add("sink", NullSink::default())
        .input("dc")
        .connect("dc", "nb")
        .connect("nb", "chan")
        .connect("chan", "demod")
        .connect("demod", "agc")
        .connect("agc", "deemph")
        .connect("deemph", "audio")
        .connect("nb", "spec")
        .connect("spec", "sink")
        .connect("chan", "meter")
        .output("audio")
        .status_from("demod")
        .build()
        .unwrap()
}

#[test]
fn nothing_is_allocated_once_warmed_up() {
    let mut g = receiver();
    g.configure(&info(240e3)).unwrap();
    let mut tap = g.tap("chan", 8).unwrap();
    let x = am(240_000, 240e3);
    let mut out = Output::default();
    let mut chunk = Buffer::default();
    let mut run = |g: &mut Graph, out: &mut Output| {
        for c in x.chunks(4800) {
            out.clear();
            g.feed(c, out);
            while tap.pop(&mut chunk) {}
        }
    };
    run(&mut g, &mut out); // warm up: every buffer grows to its size
    let before = allocs();
    run(&mut g, &mut out);
    let after = allocs();
    assert!(!out.audio().is_empty());
    // Results included: fields are rewritten in their own strings, and merged into the graph's in place.
    assert_eq!(
        after - before,
        0,
        "allocations in a second of signal after warm-up"
    );
    assert!(has_field(g.results(), "meter.level_dbfs"));

    // Without a component that publishes changing results, not one allocation.
    let mut g = GraphBuilder::new()
        .add("dc", DcRemoval::default())
        .add(
            "chan",
            Channel::new(10e3, 24e3).unwrap().with_offset(30e3).unwrap(),
        )
        .add("demod", AmDemod::default())
        .add("audio", AudioResampler::new(48e3).unwrap())
        .add("spec", Spectrum::new(SpectrumSize::S1024, 30.0).unwrap())
        .add("sink", NullSink::default())
        .input("dc")
        .connect("dc", "chan")
        .connect("chan", "demod")
        .connect("demod", "audio")
        .connect("dc", "spec")
        .connect("spec", "sink")
        .output("audio")
        .build()
        .unwrap();
    g.configure(&info(240e3)).unwrap();
    let mut tap = g.tap("spec", 8).unwrap();
    let mut chunk = Buffer::default();
    let mut feed = |g: &mut Graph| {
        for c in x.chunks(4800) {
            out.clear();
            g.feed(c, &mut out);
            while tap.pop(&mut chunk) {}
        }
    };
    feed(&mut g);
    let before = allocs();
    feed(&mut g);
    assert_eq!(allocs() - before, 0, "allocations after warm-up");
}

#[test]
fn memory_is_bounded_however_large_a_call_is() {
    let mut g = receiver();
    g.configure(&info(240e3)).unwrap();
    let x = am(240_000, 240e3);
    let mut out = Output::default();
    for c in x.chunks(4800) {
        g.feed(c, &mut out);
    }
    let small = g.memory_bytes();
    assert!(small > 0);
    // One call of ten seconds: the graph runs it in pieces of `max_chunk`, so its buffers stay the size that makes.
    let big = am(2_400_000, 240e3);
    out.clear();
    g.feed(&big, &mut out);
    let after = g.memory_bytes();
    let bound = g.max_chunk() * 8 * 3 + 1_000_000;
    assert!(
        after <= bound,
        "{after} bytes after a 2.4 M sample call (bound {bound})"
    );
    out.clear();
    g.feed(&big, &mut out);
    assert_eq!(g.memory_bytes(), after, "and it does not grow");
    assert!(after >= small);
}

#[test]
fn a_single_consumer_works_in_place_and_a_shared_stream_is_left_alone() {
    let x = am(10_000, 240e3);
    let mut g = GraphBuilder::new()
        .add("dc", DcRemoval::default())
        .add("sink", NullSink::default())
        .input("dc")
        .connect("dc", "sink")
        .build()
        .unwrap();
    g.configure(&info(240e3)).unwrap();
    g.feed(&x, &mut Output::default());
    // The graph's input is the caller's slice, never copied into the graph: its buffer holds only the index and the tags.
    assert_eq!(g.buffer(INPUT).unwrap().len(), 0);
    assert_eq!(g.buffer(INPUT).unwrap().end(), 10_000);
    assert_eq!(g.buffer("dc").unwrap().len(), 10_000);
    g.feed(&x, &mut Output::default());
    assert_eq!(g.buffer("dc").unwrap().first(), 10_000);

    // A single reader of a component's output works on it in place: dc's samples move into nb's output.
    let mut g = GraphBuilder::new()
        .add("dc", DcRemoval::default())
        .add("nb", NoiseBlanker::default())
        .add("sink", NullSink::default())
        .input("dc")
        .connect("dc", "nb")
        .connect("nb", "sink")
        .build()
        .unwrap();
    g.configure(&info(240e3)).unwrap();
    g.feed(&x, &mut Output::default());
    assert_eq!(g.buffer("dc").unwrap().len(), 0);
    assert_eq!(g.buffer("dc").unwrap().end(), 10_000);
    assert_eq!(g.buffer("nb").unwrap().len(), 10_000);

    // With a second reader of dc's output, nb copies.
    let mut g = GraphBuilder::new()
        .add("dc", DcRemoval::default())
        .add("nb", NoiseBlanker::default())
        .add("sink", NullSink::default())
        .add("other", NullSink::default())
        .input("dc")
        .connect("dc", "nb")
        .connect("dc", "other")
        .connect("nb", "sink")
        .build()
        .unwrap();
    g.configure(&info(240e3)).unwrap();
    g.feed(&x, &mut Output::default());
    assert_eq!(g.buffer("dc").unwrap().len(), 10_000);

    // Both give the same samples.
    let mut a = GraphBuilder::new()
        .add("dc", DcRemoval::default())
        .add("s", NullSink::default())
        .input("dc")
        .connect("dc", "s")
        .build()
        .unwrap();
    a.configure(&info(240e3)).unwrap();
    a.feed(&x, &mut Output::default());
    let (Data::Iq(p), Data::Iq(q)) = (&a.buffer("dc").unwrap().data, &g.buffer("dc").unwrap().data)
    else {
        panic!()
    };
    assert_eq!(p, q);
}

/// Floods its output with messages.
struct Chatty;
static CHATTY_LINE: EventDef = EventDef::new("chatty.line", "Line", &[]);

impl Component for Chatty {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "chatty",
            vec![Port::iq("in")],
            vec![Port::new("out", Kind::Messages)],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        io.messages_out(0)
            .extend((0..n).map(|i| CHATTY_LINE.event(i as f64, 0.0)));
    }
}

#[test]
fn an_item_stream_is_capped_and_says_so() {
    let mut g = GraphBuilder::new()
        .add("c", Chatty)
        .input("c")
        .output("c")
        .build()
        .unwrap();
    g.configure(&info(240e3)).unwrap();
    g.set_max_items(100);
    let mut out = Output::default();
    g.feed(&[Complex32::default(); 250], &mut out);
    assert_eq!(out.events.len(), 100);
    assert_eq!(g.dropped_items(), 150);
    assert_eq!(
        g.buffer("c").unwrap().tags()[0].kind,
        TagKind::Discontinuity { lost: 150 }
    );
}

/// A position marker carried by a typed tag.
#[derive(Clone, Copy)]
struct Mark(u64);
impl Item for Mark {
    const KIND: &'static str = "test.mark";
}

/// Passes its input on and marks the first sample of every call with a typed tag.
#[derive(Default)]
struct Marker(TagSource<Mark>, u64);

impl Component for Marker {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("marker", vec![Port::iq("in")], vec![Port::iq("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(inputs.to_vec())
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        io.iq_out(0).extend_from_slice(x);
        self.1 += 1;
        let tag = self.0.tag(Mark(self.1));
        io.output(0).tag_here(0, TagKind::Typed(tag));
    }
}

/// Passes its input on; the graph carries its tags.
#[derive(Default)]
struct Pass;

impl Component for Pass {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("pass", vec![Port::iq("in")], vec![Port::iq("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(inputs.to_vec())
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        io.iq_out(0).extend_from_slice(x);
    }
}

#[test]
fn typed_tags_allocate_nothing_once_warmed_up() {
    let mut g = GraphBuilder::new()
        .add("mark", Marker::default())
        .add("a", Pass)
        .add("b", Pass)
        .add("sink", NullSink::default())
        .input("mark")
        .connect("mark", "a")
        .connect("a", "b")
        .connect("b", "sink")
        .build()
        .unwrap();
    g.configure(&info(48e3)).unwrap();
    let x = am(48_000, 48e3);
    let mut out = Output::default();
    let mut run = |g: &mut Graph| {
        for c in x.chunks(480) {
            out.clear();
            g.feed(c, &mut out);
        }
    };
    run(&mut g);
    let before = allocs();
    run(&mut g);
    assert_eq!(allocs() - before, 0, "allocations after warm-up");
    let tags = g.buffer("b").unwrap().tags();
    assert!(tags.iter().any(
        |t| matches!(&t.kind, TagKind::Typed(m) if m.get::<Mark>().is_some_and(|m| m.0 == 200))
    ));
}

/// A decoder that measures the mean level of what it is fed and emits it as a number, for the allocation check of the wrapper.
#[derive(Default)]
struct Level(f64, u64);

const MEAN: Field<field::F64> = Field::new("mean", "Mean", "");
static MEAN_EVENT: EventDef = EventDef::new("test.mean", "Mean level", &[&MEAN]);

#[derive(Clone, Debug)]
struct Mean(f64);
impl Item for Mean {
    const KIND: &'static str = "test.mean";
    fn event_def() -> Option<&'static EventDef> {
        Some(&MEAN_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        r.set(&MEAN, self.0);
    }
}

impl Decoder for Level {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "test.level"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<Mean>("mean")]
    }
    fn configure(&mut self, _: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Mean>(), 0.0)])
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        for s in x {
            self.0 += s.norm_sqr() as f64;
            self.1 += 1;
        }
        out.items::<Mean>(0).push(Timed {
            time: first as f64,
            value: Mean(self.0 / self.1.max(1) as f64),
        });
    }
    fn results(&self) -> Published {
        Published::new(&[&keys::LEVEL_DBFS])
    }
    fn report(&mut self, r: &mut Published) {
        r.set(
            &keys::LEVEL_DBFS,
            10.0 * (self.0 / self.1.max(1) as f64).log10(),
        );
    }
}

#[test]
fn a_mode_graph_and_a_decoder_allocate_nothing_once_warmed_up() {
    let g = GraphBuilder::new()
        .add("chan", Channel::new(10e3, 24e3).unwrap())
        .add("dec", DecoderComponent::new(Level::default()))
        .add("sink", NullSink::default())
        .input("chan")
        .connect("chan", "dec")
        .connect("dec", "sink")
        .status_from("dec")
        .build()
        .unwrap();
    let mut rx = StackGraph::builder("level", g)
        .min_rate(48e3, "")
        .fixed_passes(4096)
        .build()
        .unwrap();
    rx.configure(&InputInfo::new(240e3, 100e6, 100e6).recording())
        .unwrap();
    let x = am(24_000, 240e3);
    let mut out = Output::default();
    for _ in 0..5 {
        out.clear();
        for p in x.chunks(3000) {
            rx.feed(p, &mut out);
        }
    }
    let before = allocs();
    for _ in 0..20 {
        out.clear();
        for p in x.chunks(3000) {
            rx.feed(p, &mut out);
        }
    }
    assert_eq!(allocs() - before, 0, "allocations in steady state");
    assert!(has_field(rx.results(), "level_dbfs"));
}

#[test]
fn a_packet_pool_reuses_buffers_once_dropped_and_then_allocates_nothing() {
    let mut p = PacketPool::default();
    let a = p.make(10, 0.0, |b| b.fill(1));
    assert_eq!(a.bytes(), &[1; 10]);
    let held = a.data.clone();
    drop(a);
    // Still held downstream: a second buffer.
    let b = p.make(10, 0.0, |b| b.fill(2));
    assert!(!std::sync::Arc::ptr_eq(&held, &b.data));
    drop(held);
    drop(b);
    let before = allocs();
    for i in 0..1000 {
        let c = p.make(5 + i % 50, i as f64, |b| b.fill(3));
        assert_eq!(c.bytes()[0], 3);
    }
    assert_eq!(allocs() - before, 0);
}

/// One access unit per 1000 input samples as a `MediaPacket`, in one shared format, and one frame of an input without a rate.
struct Units {
    rate: f64,
    format: Arc<MediaFormat>,
    unit: Arc<[u8]>,
}
impl Component for Units {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.units",
            vec![
                Port::iq("in"),
                Port::command("control").optional("no commands"),
            ],
            vec![Port::typed::<MediaPacket>("media")],
        )
    }
    fn configure(&mut self, i: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = i[0].rate;
        Ok(vec![Format::new(Kind::typed::<MediaPacket>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        // The control path that can change a rate, with nothing to do.
        let _ = apply_commands_reformat(self, io, 1);
        let b = io.input(0);
        for at in (b.first().div_ceil(1000) * 1000..b.end()).step_by(1000) {
            let t = at as f64 / self.rate;
            let p = Packet::new(self.unit.clone(), t).stamped(Some(at as i64), None, true);
            io.items_out::<MediaPacket>(0).push(Timed {
                time: t,
                value: MediaPacket::new(p, self.format.clone()),
            });
        }
    }
}

/// Counts the media packets it is fed (an input without a rate) and reports the count once a second of signal.
#[derive(Default)]
struct UnitCount(u64);
impl Decoder for UnitCount {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    type Input = Timed<MediaPacket>;
    fn name(&self) -> &'static str {
        "test.unit_count"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<MediaPacket>("out")]
    }
    fn configure(&mut self, _: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<MediaPacket>(), 0.0)])
    }
    fn process(&mut self, x: &[Timed<MediaPacket>], _: u64, _: &mut Outbox) {
        self.0 += x.len() as u64;
    }
    fn results(&self) -> Published {
        Published::new(&[&keys::FRAMES_OK])
    }
    fn report(&mut self, r: &mut Published) {
        r.set(&keys::FRAMES_OK, self.0 as i64);
    }
    fn report_interval_s(&self) -> f64 {
        1.0
    }
}

#[test]
fn media_packets_a_rateless_decoder_and_declared_results_allocate_nothing_once_warmed_up() {
    let decl = sdrkit_core::ResultsDecl::new().with_fields(&[&keys::FRAMES_OK]);
    let mut g = GraphBuilder::new()
        .add(
            "units",
            Units {
                rate: 0.0,
                format: Arc::new(MediaFormat::new(Codec::Aac, Packaging::Adts)),
                unit: Arc::from(&[0xffu8, 0xf1, 0x50, 0x80][..]),
            },
        )
        .add("count", DecoderComponent::new(UnitCount::default()))
        .input("units.in")
        .connect("units.media", "count")
        .output("units.media")
        .declare(&decl)
        .build()
        .unwrap();
    g.configure(&info(48e3)).unwrap();
    let x = vec![Complex32::default(); 4800];
    let mut out = Output::default();
    let run = |g: &mut Graph, out: &mut Output| {
        for _ in 0..100 {
            out.clear();
            g.feed(&x, out);
            g.refresh_results();
        }
    };
    run(&mut g, &mut out); // warm up
    let before = allocs();
    run(&mut g, &mut out); // 10 s of signal, 480 packets to the host, 10 reports
    assert_eq!(allocs() - before, 0, "allocations after warm-up");
    assert_eq!(out.media.len(), 4); // the last 0.1 s holds the units at 19.912, 19.937, 19.958 and 19.979 s
                                    // The declared key of a component that is not the status one keeps its name.
                                    // The last report is at the unit of 19 s: the units up to the end of that call, 0 to 916000 samples.
    assert_eq!(g.results().fields().get(&keys::FRAMES_OK), Some(917));
}

#[test]
fn speech_erasures_and_guarded_codec_calls_allocate_nothing_once_warmed_up() {
    use sdrkit_core::codec::{AudioCodec, CodecError, CodecSet, Pcm};
    use sdrkit_core::FourCc;
    use sdrkit_graph::codec::GuardedCodec;
    struct Silence;
    impl AudioCodec for Silence {
        fn decode(&mut self, _: &[u8], _: bool, pcm: &mut Pcm) -> Result<(), CodecError> {
            pcm.samples.extend([0i16; 160]);
            Ok(())
        }
    }
    let ambe2 = FourCc::new("amb2");
    let mut set = CodecSet::default();
    set.register(
        ambe2,
        None,
        Arc::new(|_: &sdrkit_core::media::MediaFormat| {
            Some(Box::new(Silence) as Box<dyn AudioCodec>)
        }),
    );
    let mut codec = GuardedCodec::start(
        set.find(ambe2, "codewords").unwrap(),
        &sdrkit_core::media::MediaFormat::new(
            sdrkit_core::media::Codec::Ambe2,
            sdrkit_core::media::Packaging::Raw,
        ),
    );
    let words = Packet::new(Arc::from(&[0u8; 12][..]), 0.0);
    let frame = SpeechPacket::frame(ambe2, sdrkit_graph::SpeechPackaging::Codewords, 20, words);
    let mut run = |n: i64| {
        for k in 0..n {
            let gap = SpeechPacket::erasure(
                ambe2,
                sdrkit_graph::SpeechPackaging::Codewords,
                20,
                k * 160,
                k as f64 * 0.02,
            );
            assert_eq!(codec.decode_packet(&gap).unwrap().samples.len(), 160);
            assert_eq!(codec.decode_packet(&frame).unwrap().samples.len(), 160);
        }
    };
    run(100);
    let before = allocs();
    run(1000);
    assert_eq!(
        allocs() - before,
        0,
        "allocations in 1000 frames and 1000 erasures"
    );
}

/// A decoder that returns one more item on each call than on the call before, up to `cap` when it declares that cap.
#[derive(Default)]
struct Growing(usize, Option<usize>);

impl Decoder for Growing {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "test.growing"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<Mean>("mean")]
    }
    fn configure(&mut self, _: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Mean>(), 0.0)])
    }
    fn output_cap(&self, _: usize) -> Option<usize> {
        self.1
    }
    fn process(&mut self, _: &[Complex32], first: u64, out: &mut Outbox) {
        self.0 = (self.0 + 1).min(self.1.unwrap_or(usize::MAX));
        let v = out.items::<Mean>(0);
        v.extend((0..self.0).map(|_| Timed {
            time: first as f64,
            value: Mean(0.0),
        }));
    }
}

#[test]
fn a_decoder_whose_output_grows_allocates_nothing_after_its_first_items() {
    let mut g = GraphBuilder::new()
        .add("dec", DecoderComponent::new(Growing(0, None)))
        .add("sink", NullSink::default())
        .input("dec")
        .connect("dec", "sink")
        .build()
        .unwrap();
    g.configure(&info(48e3)).unwrap();
    let x = vec![Complex32::default(); 100];
    let mut out = Output::default();
    for _ in 0..4 {
        g.process(&x, &mut out);
    }
    let before = allocs();
    for _ in 0..1000 {
        g.process(&x, &mut out);
    }
    assert_eq!(allocs() - before, 0, "allocations while the output grows");
}

#[test]
fn a_decoder_with_a_declared_cap_keeps_that_much_room_and_allocates_nothing() {
    let run = |cap: Option<usize>| {
        let mut g = GraphBuilder::new()
            .add("dec", DecoderComponent::new(Growing(0, cap)))
            .add("sink", NullSink::default())
            .input("dec")
            .connect("dec", "sink")
            .build()
            .unwrap();
        g.configure(&info(48e3)).unwrap();
        let x = vec![Complex32::default(); 100];
        let mut out = Output::default();
        for _ in 0..40 {
            g.process(&x, &mut out);
        }
        let before = allocs();
        for _ in 0..1000 {
            g.process(&x, &mut out);
        }
        assert_eq!(allocs() - before, 0, "allocations with the cap {cap:?}");
        g.memory_bytes()
    };
    let small = run(Some(16));
    let large = run(None);
    assert!(
        small + 1000 * std::mem::size_of::<Timed<Mean>>() < large,
        "{small} {large}"
    );
}
