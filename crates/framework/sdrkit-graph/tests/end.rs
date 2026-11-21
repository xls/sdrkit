//! The end of the stream, the retune hook, pictures to the host, and decoders without outputs.

use sdrkit_core::publish::{field, keys, Field, Published};
use sdrkit_core::Owner;
use sdrkit_core::{Complex32, ConfigError, InputInfo, Output, Receiver, Results, StreamKind};
use sdrkit_graph::media::{Codec, Picture, PixelFormat, PixelLayout};
use sdrkit_graph::{
    Component, ComponentInfo, Decoder, DecoderComponent, Format, GraphBuilder, Helper, Io, Item,
    Kind, Outbox, Port, StackGraph, Timed,
};
use std::sync::{Arc, Mutex};

/// A frame of 100 samples, as a typed item.
#[derive(Clone, Debug, PartialEq)]
struct Frame(u64);
impl Item for Frame {
    const KIND: &'static str = "test.frame";
}

/// Cuts the samples into frames of 100. At the end of the stream a frame of at least 50 samples still counts.
#[derive(Default)]
struct Framer {
    held: usize,
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
            vec![Port::typed::<Frame>("frames")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        (self.held, self.n) = (0, 0);
        Ok(vec![Format::new(Kind::typed::<Frame>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        self.held += io.samples::<Complex32>(0).len();
        while self.held >= 100 {
            self.held -= 100;
            self.n += 1;
            io.items_out::<Frame>(0).push(Timed {
                time: self.n as f64,
                value: Frame(self.n),
            });
        }
    }
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        if self.held >= 50 {
            self.held = 0;
            self.n += 1;
            io.items_out::<Frame>(0).push(Timed {
                time: self.n as f64,
                value: Frame(self.n),
            });
        }
    }
}

/// Counts frames; reports the count; runs on a helper thread when live. It has no outputs.
#[derive(Default)]
struct FrameCount {
    frames: u64,
    finished: bool,
    /// Seconds between reports; 0 for a report on every call that brings frames.
    interval: f64,
}
const FINISHED: Field<field::Bool> = Field::new("finished", "Finished", "");

impl Decoder for FrameCount {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    type Input = Timed<Frame>;
    fn name(&self) -> &'static str {
        "test.frame_count"
    }
    fn outputs(&self) -> Vec<Port> {
        Vec::new()
    }
    fn configure(&mut self, _: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(Vec::new())
    }
    fn process(&mut self, x: &[Timed<Frame>], _: u64, _: &mut Outbox) {
        self.frames += x.len() as u64;
    }
    fn finish(&mut self, _: &mut Outbox) {
        self.finished = true;
    }
    fn results(&self) -> Published {
        Published::new(&[&keys::FRAMES_OK, &FINISHED])
    }
    fn report(&mut self, r: &mut Published) {
        r.set(&keys::FRAMES_OK, self.frames as i64);
        r.set(&FINISHED, self.finished);
    }
    fn report_interval_s(&self) -> f64 {
        self.interval
    }
    fn helper(&self) -> Helper {
        Helper::WhenLive {
            batch: 4,
            backlog: 1000,
        }
    }
}

fn frames_graph(interval: f64) -> StackGraph {
    let g = GraphBuilder::new()
        .add("framer", Framer::default())
        .add(
            "count",
            DecoderComponent::new(FrameCount {
                interval,
                ..Default::default()
            }),
        )
        .input("framer")
        .connect("framer", "count")
        .status_from("count")
        .build()
        .unwrap();
    StackGraph::builder("frames", g).build().unwrap()
}

fn frames(r: &Results) -> Option<i64> {
    r.fields().get(&keys::FRAMES_OK)
}

fn finished(r: &Results) -> Option<bool> {
    r.fields().get(&FINISHED)
}

#[test]
fn finish_flushes_every_component_and_merges_the_results() {
    for live in [false, true] {
        let mut rx = frames_graph(1e9);
        let mut input = InputInfo::new(1000.0, 0.0, 0.0);
        if !live {
            input = input.recording();
        }
        rx.configure(&input).unwrap();
        let mut out = Output::default();
        // 10 whole frames and 70 samples of an eleventh, in pieces too short to merge the results on their own.
        let x = vec![Complex32::new(1.0, 0.0); 1070];
        for p in x.chunks(5) {
            rx.feed(p, &mut out);
        }
        assert_ne!(frames(rx.results()), Some(11), "live {live}");
        rx.finish(&mut out);
        assert_eq!(frames(rx.results()), Some(11), "live {live}");
        assert_eq!(finished(rx.results()), Some(true), "live {live}");
    }
}

#[test]
fn finish_runs_the_samples_held_by_fixed_passes() {
    let g = GraphBuilder::new()
        .add("framer", Framer::default())
        .add("count", DecoderComponent::new(FrameCount::default()))
        .input("framer")
        .connect("framer", "count")
        .status_from("count")
        .build()
        .unwrap();
    let mut rx = StackGraph::builder("frames", g)
        .fixed_passes(300)
        .build()
        .unwrap();
    rx.configure(&InputInfo::new(1000.0, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::new(1.0, 0.0); 760], &mut out);
    rx.finish(&mut out);
    // 600 samples in two passes, then 160 in a short last pass: 7 whole frames and one of 60.
    assert_eq!(frames(rx.results()), Some(8));
}

#[test]
fn an_empty_feed_merges_the_results() {
    let mut rx = frames_graph(0.0);
    rx.configure(&InputInfo::new(1000.0, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    // One sample a call: the results merge every 17 samples on their own, so the frame at sample 100 is not shown yet.
    for _ in 0..100 {
        rx.feed(&[Complex32::new(1.0, 0.0)], &mut out);
    }
    assert_ne!(frames(rx.results()), Some(1));
    rx.feed(&[], &mut out);
    assert_eq!(frames(rx.results()), Some(1));
    assert_eq!(finished(rx.results()), Some(false));
}

/// Remembers the frequencies the graph sends it.
struct Tuned(Arc<Mutex<Vec<(f64, f64)>>>);
impl Component for Tuned {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.tuned", vec![Port::iq("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(Vec::new())
    }
    fn process(&mut self, _: &mut Io) {}
    fn retuned(&mut self, centre_hz: f64, tuned_hz: f64) {
        self.0.lock().unwrap().push((centre_hz, tuned_hz));
    }
}

#[test]
fn every_component_hears_the_frequency_at_configure_and_on_retune() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let g = GraphBuilder::new()
        .add("t", Tuned(seen.clone()))
        .input("t")
        .build()
        .unwrap();
    let mut rx = StackGraph::builder("tuned", g).build().unwrap();
    rx.configure(&InputInfo::new(1e6, 868e6, 868.3e6)).unwrap();
    rx.retuned(&InputInfo::new(1e6, 869e6, 869.5e6));
    assert_eq!(*seen.lock().unwrap(), [(868e6, 868.3e6), (869e6, 869.5e6)]);
}

/// Sends one grey picture of 4 x 2 pixels on each pass.
struct Painter;
impl Component for Painter {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.painter",
            vec![Port::iq("in")],
            vec![Port::typed::<Picture>("picture")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Picture>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        io.items_out::<Picture>(0).push(Timed {
            time: 0.5,
            value: Picture {
                codec: Codec::Pixels(PixelFormat::Grey8),
                layout: Some(PixelLayout::packed(PixelFormat::Grey8, 4, 2)),
                name: None,
                data: Arc::from(&[7u8; 8][..]),
            },
        });
    }
}

#[test]
fn a_picture_output_reaches_the_host_with_its_pixel_layout() {
    let g = GraphBuilder::new()
        .add("paint", Painter)
        .input("paint")
        .output("paint")
        .build()
        .unwrap();
    let mut rx = StackGraph::builder("paint", g).build().unwrap();
    rx.configure(&InputInfo::new(1000.0, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    rx.feed(&[Complex32::default(); 10], &mut out);
    assert_eq!(out.media.len(), 1);
    let m = &out.media[0];
    assert_eq!(m.kind, StreamKind::Picture);
    assert_eq!(m.packet.bytes(), &[7u8; 8]);
    assert_eq!(m.format.codec, Codec::Pixels(PixelFormat::Grey8));
    let l = m.format.pixels.as_ref().unwrap();
    assert_eq!((l.width, l.height), (4, 2));
    assert!(out.events.is_empty());
}
