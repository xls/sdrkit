//! Validate: formats resolved in run order, adapters inserted from the catalog, derivations shared, and demand.

use sdrkit_core::{ConfigError, Output, Values};
use sdrkit_graph::components::{SpectrumSize, SpectrumWindow};
use sdrkit_graph::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Iq in, one output of `kind` with `channels` channels at the input rate. It writes one zero item per input sample.
struct Source {
    kind: Kind,
    channels: u16,
}

impl Component for Source {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "source",
            vec![Port::iq("in")],
            vec![Port::new("out", self.kind)],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            channels: self.channels,
            ..inputs[0].with_kind(self.kind)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.iq(0).len();
        match self.kind {
            Kind::Audio => {
                let c = usize::from(self.channels);
                io.audio_out(0)
                    .samples
                    .extend(std::iter::repeat_n(0.0, n * c));
            }
            Kind::Llr => io.llrs_out(0).extend(std::iter::repeat_n(-3, n)),
            Kind::HardBits => io.hard_bits_out(0).extend(std::iter::repeat_n(1, n)),
            Kind::Real => io.real_out(0).extend(std::iter::repeat_n(0.0, n)),
            _ => io.iq_out(0).extend_from_slice(&[]),
        }
    }
}

/// What a probe saw: its calls and the address of what its input held at each call.
#[derive(Default)]
struct Seen {
    calls: AtomicUsize,
    addresses: Mutex<Vec<usize>>,
}

/// A sink with input `port` that counts its calls and notes the address of its input's items.
struct Probe {
    port: Port,
    seen: Arc<Seen>,
}

impl Probe {
    fn new(port: Port) -> (Probe, Arc<Seen>) {
        let seen = Arc::new(Seen::default());
        (
            Probe {
                port,
                seen: Arc::clone(&seen),
            },
            seen,
        )
    }
}

impl Component for Probe {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("probe", vec![self.port.clone()])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        self.seen.calls.fetch_add(1, Ordering::Relaxed);
        let address = match io.input_format(0).kind {
            Kind::Iq => io.iq(0).as_ptr() as usize,
            Kind::Real => io.real(0).as_ptr() as usize,
            Kind::Audio => io.audio(0).samples.as_ptr() as usize,
            Kind::Spectrum => std::ptr::from_ref(io.spectrum(0)) as usize,
            _ => 0,
        };
        self.seen.addresses.lock().unwrap().push(address);
    }
}

/// An `Iq` processor that counts its calls.
struct Counter(Arc<AtomicUsize>);

impl Component for Counter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "counter",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        self.0.fetch_add(1, Ordering::Relaxed);
        let x = io.iq(0);
        io.iq_out(0).extend_from_slice(x);
    }
}

fn input() -> Format {
    Format::iq(48e3, 100e6, 100e6)
}

/// The graph `source -> probe`, validated, and its adapter kinds.
fn adapters(source: Source, port: Port) -> Result<Vec<&'static str>, GraphError> {
    let (probe, _) = Probe::new(port);
    let mut g = GraphBuilder::new()
        .add("src", source)
        .add("probe", probe)
        .input("src")
        .connect("src", "probe")
        .build()?;
    let r = g.validate(input())?;
    Ok(r.adapters.iter().map(|a| a.kind).collect())
}

fn connect_error(e: GraphError) -> ConnectError {
    match e {
        GraphError::Connect { error, .. } => *error,
        e => panic!("{e}"),
    }
}

#[test]
fn stereo_into_mono_downmixes_and_mono_into_stereo_duplicates() {
    let stereo = || Source {
        kind: Kind::Audio,
        channels: 2,
    };
    let mono_in = || Port::audio("in").channels(ChannelRule::Exact(1));
    assert_eq!(
        adapters(stereo(), mono_in().lossy()).unwrap(),
        ["dsp.downmix"]
    );
    // A downmix is declared-lossy: an input that does not take one refuses stereo.
    assert!(matches!(
        connect_error(adapters(stereo(), mono_in()).unwrap_err()),
        ConnectError::ChannelMismatch { channels: 2, .. }
    ));
    let mono = Source {
        kind: Kind::Audio,
        channels: 1,
    };
    let stereo_in = Port::audio("in").channels(ChannelRule::Exact(2));
    assert_eq!(adapters(mono, stereo_in).unwrap(), ["dsp.duplicate"]);
}

#[test]
fn iq_at_the_wrong_centre_and_rate_gets_one_fused_shift_and_rate_change() {
    let (probe, seen) = Probe::new(Port::iq("in").centre(100.01e6).rate(RateRule::Exact(24e3)));
    let mut g = GraphBuilder::new()
        .add("probe", probe)
        .input("probe")
        .build()
        .unwrap();
    let r = g.validate(input()).unwrap().clone();
    assert_eq!(r.adapters.len(), 1);
    assert_eq!(r.adapters[0].kind, "dsp.shift_resample");
    assert_eq!(r.adapters[0].source, "input");
    let (_, f) = r
        .ports
        .iter()
        .find(|(name, _)| *name == r.adapters[0].output)
        .unwrap();
    assert_eq!((f.rate, f.centre_hz), (24e3, 100.01e6));
    g.configure_input(input()).unwrap();
    g.process(&[Default::default(); 4800], &mut Output::default());
    assert_eq!(seen.calls.load(Ordering::Relaxed), 1);
}

#[test]
fn llr_into_hard_bits_decides_and_hard_bits_into_llr_fails() {
    let llr = Source {
        kind: Kind::Llr,
        channels: 1,
    };
    assert_eq!(
        adapters(llr, Port::hard_bits("in").lossy()).unwrap(),
        ["dsp.hard_decision"]
    );
    let bits = Source {
        kind: Kind::HardBits,
        channels: 1,
    };
    assert!(matches!(
        connect_error(adapters(bits, Port::llr("in")).unwrap_err()),
        ConnectError::NoConversion {
            produces: Kind::HardBits,
            ..
        }
    ));
}

#[test]
fn iq_into_a_real_input_is_refused_because_it_changes_the_meaning() {
    let (probe, _) = Probe::new(Port::real("in"));
    let mut plan = GraphPlan::new();
    let p = plan.add_component("probe", Box::new(probe)).unwrap();
    assert!(matches!(
        plan.input(PortRef::new(p, 0)),
        Err(ConnectError::NoConversion {
            produces: Kind::Iq,
            ..
        })
    ));
}

#[test]
fn a_cycle_and_an_unconnected_input_fail_before_a_sample_flows() {
    let e = GraphBuilder::new()
        .add("a", Counter(Arc::default()))
        .add("b", Counter(Arc::default()))
        .connect("a", "b")
        .connect("b", "a")
        .build()
        .unwrap_err();
    assert!(matches!(e, GraphError::Cycle(_)), "{e}");
    let e = GraphBuilder::new()
        .add("a", Counter(Arc::default()))
        .build()
        .unwrap_err();
    assert_eq!(e, GraphError::Unconnected("a.in".into()));
}

#[test]
fn the_report_lists_every_adapter_and_the_format_of_every_port() {
    let (probe, _) = Probe::new(
        Port::audio("in")
            .channels(ChannelRule::Exact(1))
            .rate(RateRule::Exact(8e3))
            .lossy(),
    );
    let mut g = GraphBuilder::new()
        .add(
            "src",
            Source {
                kind: Kind::Audio,
                channels: 2,
            },
        )
        .add("probe", probe)
        .input("src")
        .connect("src", "probe")
        .build()
        .unwrap();
    let r = g.validate(input()).unwrap();
    // The downmix comes first: it halves what the rate change must touch.
    let kinds: Vec<_> = r.adapters.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, ["dsp.downmix", "dsp.resampler"]);
    assert_eq!(r.adapters[0].source, "src.out");
    assert_eq!(r.adapters[1].source, r.adapters[0].output);
    assert_eq!(r.adapters[1].consumers, ["probe.in"]);
    let names: Vec<&str> = r.ports.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names[..2], ["input", "src.out"]);
    assert_eq!(r.ports.len(), 4);
    let last = &r.ports[3].1;
    assert_eq!((last.kind, last.channels, last.rate), (Kind::Audio, 1, 8e3));
}

fn spectrum_input(size: SpectrumSize) -> Port {
    Port::spectrum(
        "in",
        SpectrumSpec {
            size: sdrkit_graph::SizeRule::Bins(size),
            window: SpectrumWindow::Hann,
            average: 1,
            frames: sdrkit_graph::FrameRule::PerSecond(10.0),
            scale: sdrkit_graph::components::SpectrumScale::Db,
        },
    )
}

#[test]
fn equal_spectra_are_made_once_and_read_from_one_buffer() {
    let (a, seen_a) = Probe::new(spectrum_input(SpectrumSize::S1024));
    let (b, seen_b) = Probe::new(spectrum_input(SpectrumSize::S1024));
    let (c, _) = Probe::new(spectrum_input(SpectrumSize::S4096));
    let mut g = GraphBuilder::new()
        .add("a", a)
        .add("b", b)
        .add("c", c)
        .input("a")
        .input("b")
        .input("c")
        .build()
        .unwrap();
    let r = g.validate(input()).unwrap();
    let spectra: Vec<_> = r
        .adapters
        .iter()
        .filter(|x| x.kind == "dsp.spectrum")
        .collect();
    // Equal parameters share. Another size is another derivation.
    assert_eq!(spectra.len(), 2);
    assert_eq!(spectra[0].consumers, ["a.in", "b.in"]);
    assert_eq!(spectra[1].consumers, ["c.in"]);
    g.configure_input(input()).unwrap();
    g.process(&[Default::default(); 4800], &mut Output::default());
    let a = seen_a.addresses.lock().unwrap().clone();
    let b = seen_b.addresses.lock().unwrap().clone();
    assert_eq!(a, b);
}

#[test]
fn a_branch_behind_a_closed_tap_does_not_run() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut g = GraphBuilder::new()
        .add("count", Counter(Arc::clone(&calls)))
        .input("count")
        .build()
        .unwrap();
    g.configure_input(input()).unwrap();
    let mut out = Output::default();
    let x = [Default::default(); 480];
    g.process(&x, &mut out);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let tap = g.tap("count.out", 4).unwrap();
    g.process(&x, &mut out);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    drop(tap);
    g.process(&x, &mut out);
    g.process(&x, &mut out);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn a_fan_out_to_three_consumers_copies_nothing() {
    let probes: Vec<_> = (0..3).map(|_| Probe::new(Port::real("in"))).collect();
    let mut b = GraphBuilder::new()
        .add(
            "src",
            Source {
                kind: Kind::Real,
                channels: 1,
            },
        )
        .input("src");
    let mut seen = Vec::new();
    for (k, (p, s)) in probes.into_iter().enumerate() {
        let id = format!("p{k}");
        b = b.add(&id, p).connect("src", &id);
        seen.push(s);
    }
    let mut g = b.build().unwrap();
    g.configure_input(input()).unwrap();
    g.process(&[Default::default(); 480], &mut Output::default());
    let held = g.buffer("src.out").unwrap();
    let Data::Real(v) = &held.data else {
        panic!("real samples")
    };
    let at = v.as_ptr() as usize;
    for s in &seen {
        assert_eq!(*s.addresses.lock().unwrap(), [at]);
    }
}

#[test]
fn a_setting_given_after_add_sets_the_declared_output_rate() {
    use sdrkit_graph::components::Decimate;
    let rule = Decimate::new(2).unwrap().info().outputs[0].output.rate;
    assert!(matches!(rule, RateFrom::DividedBySetting { input: 0, .. }));
    let iq = Format::iq(240e3, 0.0, 0.0);
    let mut built = GraphBuilder::new()
        .add("dec", Decimate::new(2).unwrap())
        .input("dec")
        .set("dec.factor", sdrkit_core::Value::I64(4))
        .build()
        .unwrap();
    built.validate(iq.clone()).unwrap();
    assert_eq!(built.format("dec.out").unwrap().rate, 60e3);

    let json = r#"{
      "components": [{ "id": "dec", "type": "dsp.decimator", "settings": { "factor": 5 } }],
      "edges": [["input", "dec.in"]]
    }"#;
    let mut described = Graph::from_json(json, &Registry::standard()).unwrap();
    described.validate(iq).unwrap();
    assert_eq!(described.format("dec.out").unwrap().rate, 48e3);
}

/// Iq in and out, with an output rate rule that reads a setting it does not declare.
struct UndeclaredRate;

impl Component for UndeclaredRate {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "undeclared_rate",
            vec![Port::iq("in")],
            vec![Port::iq("out").rate_from(RateFrom::Setting(sdrkit_core::SettingId(0)))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(inputs.to_vec())
    }
    fn process(&mut self, _: &mut Io) {}
}

#[test]
fn a_rule_that_reads_an_undeclared_setting_fails_when_added() {
    let e = GraphBuilder::new()
        .add("u", UndeclaredRate)
        .input("u")
        .output("u.out")
        .build()
        .unwrap_err();
    assert!(matches!(
        e,
        GraphError::Role {
            problem: PortProblem::RuleSetting { .. },
            ..
        }
    ));
}

/// The format that validate resolved for the input of probe `id`.
fn probe_format(r: &ValidateReport, id: &str) -> Format {
    let a = r
        .adapters
        .iter()
        .find(|a| a.consumers.iter().any(|c| c.starts_with(id)))
        .expect("an adapter feeds the probe");
    r.ports
        .iter()
        .find(|(n, _)| *n == a.output)
        .map(|(_, f)| f.clone())
        .expect("the adapter output has a format")
}

#[test]
fn a_real_stream_gets_its_spectrum_in_linear_power() {
    let spec = |scale| SpectrumSpec {
        size: SizeRule::Bins(SpectrumSize::S1024),
        window: SpectrumWindow::Hann,
        average: 1,
        frames: FrameRule::PerSecond(10.0),
        scale,
    };
    let (db, _) = Probe::new(Port::spectrum("in", spec(components::SpectrumScale::Db)));
    let (lin, seen) = Probe::new(Port::spectrum("in", spec(components::SpectrumScale::Power)));
    let mut g = GraphBuilder::new()
        .add(
            "src",
            Source {
                kind: Kind::Real,
                channels: 1,
            },
        )
        .add("db", db)
        .add("lin", lin)
        .input("src")
        .connect("src", "db")
        .connect("src", "lin")
        .build()
        .unwrap();
    let r = g.validate(input()).unwrap().clone();
    // dB and linear power are two derivations of the real stream.
    let spectra: Vec<_> = r
        .adapters
        .iter()
        .filter(|a| a.kind == "dsp.spectrum")
        .collect();
    assert_eq!(spectra.len(), 2);
    assert!(spectra.iter().all(|a| a.source == "src.out"));
    let f = probe_format(&r, "lin");
    assert_eq!(f.scale, Some(components::SpectrumScale::Power));
    // A real stream has a one-sided spectrum: half the bins, from 0 Hz to half the rate.
    assert!(f.one_sided);
    assert_eq!(f.frame_len, 512);
    assert_eq!(f.bandwidth_hz, input().rate / 2.0);
    g.configure_input(input()).unwrap();
    g.process(&[Default::default(); 9600], &mut Output::default());
    assert!(seen.calls.load(Ordering::Relaxed) > 0);
}

#[test]
fn a_spectrum_of_about_n_seconds_back_to_back_follows_the_rate() {
    // Bins of about 2 Hz at 250 ksps: frames of about half a second, one after the other.
    let want = SpectrumSpec {
        size: SizeRule::Seconds(0.5),
        window: SpectrumWindow::Hann,
        average: 1,
        frames: FrameRule::BackToBack,
        scale: components::SpectrumScale::Power,
    };
    let (p, _) = Probe::new(Port::spectrum("in", want));
    let mut g = GraphBuilder::new().add("p", p).input("p").build().unwrap();
    let r = g.validate(Format::iq(250e3, 0.0, 0.0)).unwrap().clone();
    let f = probe_format(&r, "p");
    assert_eq!(f.frame_len, 131_072);
    assert!((f.rate - 250e3 / 131_072.0).abs() < 1e-9);
}

#[test]
fn the_sweep_spectrum_maps_one_to_one() {
    // A sweep requests frames of `bins` bins (a power of two from 256 to 65536), Blackman-Harris, back to back, not averaged,
    // in dB.
    for bins in [256usize, 4096, 65_536] {
        let size = SpectrumSize::of(bins).expect("every sweep size is a spectrum size");
        assert_eq!(size.bins(), bins);
        let want = SpectrumSpec {
            size: SizeRule::Bins(size),
            window: SpectrumWindow::BlackmanHarris,
            average: 1,
            frames: FrameRule::BackToBack,
            scale: components::SpectrumScale::Db,
        };
        let (p, _) = Probe::new(Port::spectrum("in", want));
        let mut g = GraphBuilder::new().add("p", p).input("p").build().unwrap();
        let r = g.validate(Format::iq(2.4e6, 0.0, 0.0)).unwrap().clone();
        let f = probe_format(&r, "p");
        assert_eq!(f.frame_len, bins);
        assert!((f.rate - 2.4e6 / bins as f64).abs() < 1e-9);
    }
}
