//! The endpoint components: a stack's test signal drives a graph from inside into the host's streams, and a network sink sends
//! the samples.

use sdrkit_core::net::{DatagramOptions, FakeTransport, Transport};
use sdrkit_core::{
    owner, Complex32, ConfigError, Generator, InputInfo, Output, Receiver, Value, Values,
};
use sdrkit_graph::components::{
    convert_channels, HostData, HostIn, HostPort, HostStreams, MakeGenerator, NetSink,
    StackGenerator, TxSink,
};
use sdrkit_graph::{
    Component, ComponentInfo, Format, GraphBuilder, Io, Port, Registry, RunContext,
};
use std::sync::{Arc, Mutex};

/// A constant at 1 kHz, 50 ms long.
struct Short {
    left: usize,
}

impl Generator for Short {
    fn rate_hz(&self) -> f64 {
        1000.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n = out.len().min(self.left);
        out[..n].fill(Complex32::new(0.5, -0.25));
        self.left -= n;
        n
    }
}

fn generators() -> MakeGenerator {
    Arc::new(|stack: &str, _rate: f64| {
        (stack == "short").then(|| Box::new(Short { left: 50 }) as Box<dyn Generator>)
    })
}

#[derive(Default)]
struct Kept(Mutex<Vec<(HostPort, usize)>>);

impl HostStreams for Kept {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        let n = match data {
            HostData::Iq { samples, .. } => samples.len(),
            HostData::Audio { samples, .. } => samples.len(),
        };
        self.0.lock().unwrap().push((port, n));
    }
}

#[test]
fn the_registry_has_the_endpoint_kinds() {
    let r = Registry::standard();
    for k in ["host.out", "tx.sink", "gen.stack", "net.sink"] {
        assert!(r.names().any(|n| n == k), "{k}");
    }
}

#[test]
fn a_test_signal_drives_a_graph_into_the_transmit_stream() {
    let kept = Arc::new(Kept::default());
    let mut g = GraphBuilder::new()
        .add("gen", StackGenerator::default())
        .add("tx", TxSink::default())
        .connect("gen.out", "tx")
        .build()
        .unwrap();
    g.set_context(RunContext {
        streams: Some(kept.clone()),
        generators: Some(generators()),
        ..RunContext::default()
    });
    let stack = sdrkit_core::Owner::schema(&g).id("gen.stack").unwrap();
    owner::apply(&mut g, stack, &Value::Text("short".into())).unwrap();
    g.configure(&InputInfo::new(0.0, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    for _ in 0..4 {
        g.feed(&[], &mut out);
    }
    let got = kept.0.lock().unwrap().clone();
    // 20 ms a pass at 1 kHz: 20, 20 and the last 10 of 50.
    assert_eq!(
        got,
        vec![
            (HostPort::Transmit, 20),
            (HostPort::Transmit, 20),
            (HostPort::Transmit, 10)
        ]
    );
}

#[test]
fn a_stack_without_a_test_signal_is_refused_at_configure() {
    let mut g = GraphBuilder::new()
        .add("gen", StackGenerator::default())
        .add("tx", TxSink::default())
        .connect("gen.out", "tx")
        .build()
        .unwrap();
    g.set_context(RunContext {
        generators: Some(generators()),
        ..RunContext::default()
    });
    assert!(g.configure(&InputInfo::new(1000.0, 0.0, 0.0)).is_err());
}

#[test]
fn a_network_sink_sends_the_samples_as_udp_datagrams() {
    let net = FakeTransport::new();
    let mut listen = net
        .bind(&DatagramOptions::at("127.0.0.1:7355".parse().unwrap()))
        .unwrap();
    let mut g = GraphBuilder::new()
        .add("net", NetSink::default())
        .input("net")
        .build()
        .unwrap();
    g.set_context(RunContext {
        transport: Some(Arc::new(net.clone()) as Arc<dyn Transport>),
        ..RunContext::default()
    });
    g.configure(&InputInfo::new(48e3, 0.0, 0.0)).unwrap();
    let iq = vec![Complex32::new(1.0, -1.0); 100];
    g.feed(&iq, &mut Output::default());
    let mut buf = [0u8; 2048];
    let (n, _) = listen.recv_from(&mut buf).unwrap();
    assert_eq!(n, 800, "100 samples of two 32-bit floats");
    assert_eq!(&buf[..4], &1.0f32.to_le_bytes());
    assert_eq!(&buf[4..8], &(-1.0f32).to_le_bytes());
}

/// A host with mono audio for the graph, given by [`HostStreams::read`] alone.
struct MonoHost(Mutex<Vec<f32>>);

impl HostStreams for MonoHost {
    fn write(&self, _: HostPort, _: HostData<'_>) {}
    fn read(&self, port: HostPort, out: &mut [f32]) -> usize {
        assert_eq!(port, HostPort::Microphone);
        let mut left = self.0.lock().unwrap();
        let n = left.len().min(out.len());
        out[..n].copy_from_slice(&left[..n]);
        left.drain(..n);
        n
    }
}

/// Keeps the audio of its input and its channel count.
struct Capture(Arc<Mutex<(u16, Vec<f32>)>>);

impl Component for Capture {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.capture", vec![Port::audio("in")])
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.0.lock().unwrap().0 = inputs[0].channels;
        Ok(Vec::new())
    }
    fn process(&mut self, io: &mut Io) {
        self.0.lock().unwrap().1.extend_from_slice(&io.audio(0).samples);
    }
}

/// The channel count and the audio that a capture behind `mic` gets from a host with the mono audio `given`.
fn host_audio_through(mic: HostIn, given: &[f32]) -> (u16, Vec<f32>) {
    let got = Arc::new(Mutex::new((0, Vec::new())));
    let mut g = GraphBuilder::new()
        .add("mic", mic)
        .add("cap", Capture(got.clone()))
        .input("mic.in")
        .connect("mic.out", "cap.in")
        .build()
        .unwrap();
    g.set_context(RunContext {
        streams: Some(Arc::new(MonoHost(Mutex::new(given.to_vec())))),
        ..RunContext::default()
    });
    g.configure(&InputInfo::new(48e3, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    g.feed(&[Complex32::default(); 480], &mut out);
    let got = got.lock().unwrap().clone();
    got
}

#[test]
fn a_mono_host_audio_input_puts_out_the_audio_of_the_host() {
    let given: Vec<f32> = (0..7000).map(|i| i as f32).collect();
    assert_eq!(host_audio_through(HostIn::default(), &given), (1, given));
}

#[test]
fn a_stereo_host_audio_input_puts_mono_audio_of_the_host_in_both_channels() {
    let given: Vec<f32> = (0..7000).map(|i| i as f32).collect();
    let (channels, got) = host_audio_through(HostIn::stereo(), &given);
    assert_eq!(channels, 2);
    assert_eq!(got.len(), 2 * given.len());
    assert!(got.chunks(2).zip(&given).all(|(f, &s)| f == [s, s]));
}

#[test]
fn channels_convert_by_the_mean_the_copy_and_the_first_channels() {
    let mut out = Vec::new();
    convert_channels(&[1.0, 3.0, -2.0, 4.0, 9.0], 2, 1, &mut out);
    assert_eq!(out, [2.0, 1.0]);
    out.clear();
    convert_channels(&[1.0, 2.0], 1, 2, &mut out);
    assert_eq!(out, [1.0, 1.0, 2.0, 2.0]);
    out.clear();
    convert_channels(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3, 2, &mut out);
    assert_eq!(out, [1.0, 2.0, 4.0, 5.0]);
    out.clear();
    convert_channels(&[1.0, 2.0, 3.0], 2, 2, &mut out);
    assert_eq!(out, [1.0, 2.0]);
}
