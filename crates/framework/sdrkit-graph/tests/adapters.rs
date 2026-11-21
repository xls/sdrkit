//! The adapters that validate inserts for the rules of an input: the tuned centre, a centre out of view, a shift that changes
//! at a sample, the delay of each adapter, the analytic signal's band, the LLR scale, raw samples, codecs and item types.

use sdrkit_core::{Complex32, ConfigError, InputInfo, Output, Receiver, Values};
use sdrkit_graph::components::{Channel, Shift, ShiftChange};
use sdrkit_graph::*;
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};

/// What a sink saw: its configures, its input format and its samples.
#[derive(Default)]
struct Seen {
    configures: usize,
    format: Option<Format>,
    iq: Vec<Complex32>,
    llr: Vec<i8>,
}

/// A sink with input `port` that keeps what it sees.
struct Sink {
    port: Port,
    seen: Arc<Mutex<Seen>>,
}

impl Sink {
    fn new(port: Port) -> (Sink, Arc<Mutex<Seen>>) {
        let seen = Arc::new(Mutex::new(Seen::default()));
        (
            Sink {
                port,
                seen: Arc::clone(&seen),
            },
            seen,
        )
    }
}

impl Component for Sink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.sink", vec![self.port.clone()])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let mut s = self.seen.lock().unwrap();
        s.configures += 1;
        s.format = Some(inputs[0].clone());
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let mut s = self.seen.lock().unwrap();
        match io.input_format(0).kind {
            Kind::Iq => s.iq.extend_from_slice(io.iq(0)),
            Kind::Llr => s.llr.extend_from_slice(io.llrs(0)),
            _ => {}
        }
    }
}

/// Iq in; out a stream of `format` (its kind and fields), one item per input sample: soft bits of -3, or two bytes per
/// sample of `cs8` (I = 64, Q = -64), or eight per sample of `cs32` (half of full scale).
struct Source(Format);

impl Component for Source {
    fn info(&self) -> ComponentInfo {
        let mut out = Port::new("out", self.0.kind);
        out.format.sample_type = self.0.sample_type;
        out.format.bit_order = self.0.bit_order;
        ComponentInfo::processor("test.source", vec![Port::iq("in")], vec![out])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            rate: inputs[0].rate,
            ..self.0.clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.iq(0).len();
        match self.0.kind {
            Kind::Llr => io.llrs_out(0).extend(std::iter::repeat_n(-3, n)),
            Kind::Bytes => {
                let bytes: Vec<u8> = if self.0.sample_type == Some(SampleType::Cs32) {
                    let (i, q) = ((1i32 << 30).to_le_bytes(), (-(1i32 << 30)).to_le_bytes());
                    (0..n).flat_map(|_| i.into_iter().chain(q)).collect()
                } else {
                    (0..n).flat_map(|_| [64u8, (-64i8) as u8]).collect()
                };
                io.bytes_out(0).push(Packet::new(Arc::from(bytes), 0.0));
            }
            _ => {}
        }
    }
}

fn tone(rate: f64, freq: f64, n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| Complex32::from_polar(0.5, (TAU * freq * i as f64 / rate) as f32))
        .collect()
}

/// The mean frequency of `x` at `rate`, from the phase steps.
fn freq_of(x: &[Complex32], rate: f64) -> f64 {
    let s: Complex32 = x.windows(2).map(|w| w[1] * w[0].conj()).sum();
    f64::from(s.arg()) * rate / TAU
}

#[test]
fn a_tuned_channel_follows_a_retune_without_a_new_validate() {
    let (sink, seen) = Sink::new(Port::iq("in").tuned_channel(10e3));
    let mut g = GraphBuilder::new()
        .add("rx", sink)
        .input("rx")
        .build()
        .unwrap();
    let rate = 240e3;
    // Tuned 50 kHz above the centre: the channel is shifted and brought to 10 kHz.
    g.configure(&InputInfo::new(rate, 100e6, 100.05e6)).unwrap();
    let kinds: Vec<_> = g.report().adapters.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, ["dsp.shift_resample"]);
    let f = seen.lock().unwrap().format.clone().unwrap();
    assert_eq!((f.rate, f.centre_hz), (10e3, 100.05e6));
    let mut out = Output::default();
    g.process(&tone(rate, 50e3 + 1e3, 48_000), &mut out);
    // A retune to 30 kHz above the centre: the same shift moves, nothing is configured again.
    g.retuned(&InputInfo::new(rate, 100e6, 100.03e6));
    g.process(&tone(rate, 30e3 + 1e3, 48_000), &mut out);
    let s = seen.lock().unwrap();
    assert_eq!(s.configures, 1);
    let half = s.iq.len() / 2;
    let before = freq_of(&s.iq[half / 2..half], 10e3);
    let after = freq_of(&s.iq[half + half / 2..], 10e3);
    assert!((before - 1e3).abs() < 20.0, "{before}");
    assert!((after - 1e3).abs() < 20.0, "{after}");
}

#[test]
fn the_channel_takes_a_rate_down_to_its_width_and_transition() {
    let mut c = Channel::new(3e3, 3e3).unwrap();
    let f = c.configure(&[Format::iq(4e3, 0.0, 0.0)]).unwrap();
    assert!(f[0].rate >= 3.75e3);
    assert!(c.configure(&[Format::iq(3.5e3, 0.0, 0.0)]).is_err());
}

/// Iq in, `ShiftChange` items out: one change to `freq_hz` at input sample `at`.
struct Finder {
    at: u64,
    freq_hz: f64,
}

impl Component for Finder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.finder",
            vec![Port::iq("in")],
            vec![Port::typed::<ShiftChange>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<ShiftChange>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let b = io.input(0);
        let (first, end) = (b.first(), b.first() + io.iq(0).len() as u64);
        if (first..end).contains(&self.at) {
            let change = ShiftChange {
                at: self.at,
                freq_hz: self.freq_hz,
            };
            io.items_out::<ShiftChange>(0).push(Timed {
                time: 0.0,
                value: change,
            });
        }
    }
}

fn shifted(piece: usize) -> (Vec<Complex32>, Vec<Tag>) {
    let mut g = GraphBuilder::new()
        .add(
            "finder",
            Finder {
                at: 1000,
                freq_hz: 1e3,
            },
        )
        .add("shift", Shift::new(0.0).unwrap())
        .input("finder")
        .input("shift.in")
        .connect("finder", "shift.control")
        .build()
        .unwrap();
    g.configure_input(Format::iq(48e3, 0.0, 0.0)).unwrap();
    let mut tap = g.tap("shift.out", 64).unwrap();
    let mut b = stream::Buffer::new(&Kind::Iq);
    let x = vec![Complex32::new(0.5, 0.0); 4000];
    let mut out = Output::default();
    let (mut y, mut tags) = (Vec::new(), Vec::new());
    for c in x.chunks(piece) {
        g.process(c, &mut out);
        while tap.pop(&mut b) {
            if let stream::Data::Iq(v) = &b.data {
                y.extend_from_slice(v);
            }
            tags.extend(b.tags().iter().cloned());
        }
    }
    (y, tags)
}

#[test]
fn a_shift_changes_at_the_exact_sample_of_a_control_item() {
    let (a, tags) = shifted(4000);
    let (b, _) = shifted(333);
    assert_eq!(a, b, "the cut of the input changes nothing");
    assert!(a[..1000]
        .iter()
        .all(|v| (v.im).abs() < 1e-6 && (v.re - 0.5).abs() < 1e-6));
    let f = freq_of(&a[1000..], 48e3);
    assert!((f - 1e3).abs() < 1.0, "{f}");
    // The change is announced at its sample.
    assert!(tags.iter().any(|t| t.at == 1000
        && matches!(t.kind, TagKind::Retuned { centre_hz, .. } if centre_hz == -1e3)));
}

/// Real in at the input rate, for the analytic signal.
struct RealSource;

impl Component for RealSource {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("test.real", vec![Port::iq("in")], vec![Port::real("out")])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].with_kind(Kind::Real)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.iq(0).len();
        io.real_out(0).extend(std::iter::repeat_n(0.0, n));
    }
}

fn analytic_report(want: Port) -> (ValidateReport, Format) {
    let (sink, seen) = Sink::new(want);
    let mut g = GraphBuilder::new()
        .add("src", RealSource)
        .add("rx", sink)
        .input("src")
        .connect("src", "rx")
        .build()
        .unwrap();
    let r = g.validate(Format::iq(48e3, 0.0, 0.0)).unwrap().clone();
    let f = seen.lock().unwrap().format.clone().unwrap();
    (r, f)
}

#[test]
fn every_adapter_reports_its_delay_and_the_input_reads_the_sum() {
    let (r, f) = analytic_report(Port::iq("in").rate(RateRule::Exact(12e3)));
    let kinds: Vec<_> = r.adapters.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, ["dsp.resampler", "dsp.analytic"]);
    assert!(r
        .adapters
        .iter()
        .all(|a| a.group_delay_s > 0.0 && a.first_valid > 0));
    let sum: f64 = r.adapters.iter().map(|a| a.group_delay_s).sum();
    assert!((f.delay_s - sum).abs() < 1e-12, "{} {sum}", f.delay_s);
}

#[test]
fn the_analytic_signal_keeps_the_band_that_the_input_asks_for() {
    let (low, _) = analytic_report(Port::iq("in").lowest(200.0));
    let (high, _) = analytic_report(Port::iq("in").lowest(2000.0));
    // A lower edge needs a longer Hilbert filter.
    let delay = |r: &ValidateReport| {
        r.adapters
            .iter()
            .find(|a| a.kind == "dsp.analytic")
            .unwrap()
            .group_delay_s
    };
    assert!(delay(&low) > 4.0 * delay(&high));
}

fn adapters_between(from: Format, want: Port) -> (Vec<&'static str>, Arc<Mutex<Seen>>) {
    let (sink, seen) = Sink::new(want);
    let mut g = GraphBuilder::new()
        .add("src", Source(from))
        .add("rx", sink)
        .input("src")
        .connect("src", "rx")
        .build()
        .unwrap();
    g.configure_input(Format::iq(48e3, 0.0, 0.0)).unwrap();
    g.process(&[Complex32::default(); 100], &mut Output::default());
    let kinds = g.report().adapters.iter().map(|a| a.kind).collect();
    (kinds, seen)
}

#[test]
fn soft_bits_of_another_scale_are_rescaled() {
    let (kinds, seen) =
        adapters_between(Format::llr(0.0, Some(4.0)), Port::llr("in").llr_scale(8.0));
    assert_eq!(kinds, ["dsp.llr_rescale"]);
    let s = seen.lock().unwrap();
    assert_eq!(s.format.as_ref().unwrap().llr_scale, Some(8.0));
    assert!(s.llr.iter().all(|&v| v == -6));
}

#[test]
fn raw_samples_in_bytes_become_iq() {
    for t in [SampleType::Cs8, SampleType::Cs32] {
        let raw = Format {
            sample_type: Some(t),
            ..Format::new(Kind::Bytes, 0.0)
        };
        let (kinds, seen) = adapters_between(raw, Port::iq("in"));
        assert_eq!(kinds, ["dsp.to_iq"]);
        let s = seen.lock().unwrap();
        assert_eq!(s.iq.len(), 100);
        assert!(
            s.iq.iter().all(|v| *v == Complex32::new(0.5, -0.5)),
            "{t:?}"
        );
    }
}

#[test]
fn a_centre_out_of_view_reads_silence_or_fails() {
    let rate = 48e3;
    let (sink, seen) = Sink::new(Port::iq("in").centre_may_leave(100.1e6));
    let mut g = GraphBuilder::new()
        .add("rx", sink)
        .input("rx")
        .build()
        .unwrap();
    g.configure(&InputInfo::new(rate, 100e6, 100e6)).unwrap();
    g.process(&tone(rate, 1e3, 4800), &mut Output::default());
    let s = seen.lock().unwrap();
    assert_eq!(s.iq.len(), 4800);
    assert!(s.iq.iter().all(|v| *v == Complex32::default()));
    drop(s);
    let (sink, _) = Sink::new(Port::iq("in").centre(100.1e6));
    let mut g = GraphBuilder::new()
        .add("rx", sink)
        .input("rx")
        .build()
        .unwrap();
    let e = g.validate(Format::iq(rate, 100e6, 100e6)).unwrap_err();
    assert!(
        matches!(&e, GraphError::Connect { error, .. } if matches!(**error, ConnectError::CentreMismatch { .. })),
        "{e}"
    );
}

/// A typed item of the test.
#[derive(Clone, Debug)]
struct Word(u32);
impl Item for Word {
    const KIND: &'static str = "test.word";
}

/// Another typed item of the test.
#[derive(Clone, Debug)]
struct Letter;
impl Item for Letter {
    const KIND: &'static str = "test.letter";
}

/// Iq in, one `Word` per call out.
struct Words;

impl Component for Words {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.words",
            vec![Port::iq("in")],
            vec![Port::typed::<Word>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Word>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        io.items_out::<Word>(0).push(Timed {
            time: 0.0,
            value: Word(7),
        });
    }
}

/// `Word` to `Letter`, an item adapter of the test.
#[derive(Default)]
struct Spell;

impl Component for Spell {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.spell",
            vec![Port::typed::<Word>("in")],
            vec![Port::typed::<Letter>("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Letter>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.items::<Word>(0).iter().filter(|w| w.value.0 > 0).count();
        io.items_out::<Letter>(0).extend((0..n).map(|_| Timed {
            time: 0.0,
            value: Letter,
        }));
    }
}

/// Iq in, MP2 bytes out, one packet per call.
struct Mp2Out;

impl Component for Mp2Out {
    fn info(&self) -> ComponentInfo {
        let spec = media::MediaSpec::new(Codec::Mp2, Packaging::Raw);
        ComponentInfo::processor(
            "test.mp2",
            vec![Port::iq("in")],
            vec![Port::media("out", &[spec]).rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            media: Some(MediaFormat::new(Codec::Mp2, Packaging::Raw)),
            ..Format::new(Kind::Bytes, 0.0)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        io.bytes_out(0)
            .push(Packet::new(Arc::from(&[1u8][..]), 0.0));
    }
}

/// MP2 bytes to AAC bytes, a codec adapter of the test.
#[derive(Default)]
struct Transcode;

impl Component for Transcode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.transcode",
            vec![Port::bytes("in")],
            vec![Port::bytes("out").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            media: Some(MediaFormat::new(Codec::Aac, Packaging::Adts)),
            ..Format::new(Kind::Bytes, 0.0)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.bytes(0).len();
        io.bytes_out(0).extend(std::iter::repeat_n(
            Packet::new(Arc::from(&[2u8][..]), 0.0),
            n,
        ));
    }
}

/// The standard catalog with the test's item and codec adapters, as a host registry holds them.
fn test_catalog() -> Arc<Catalog> {
    let mut c = Catalog::standard();
    c.add(Adapter {
        kind: "test.spell",
        from: Kind::typed::<Word>(),
        to: Kind::typed::<Letter>(),
        class: AdapterClass::Lossless,
        cost: 1,
        settings: |_, _| Some(no_values()),
        make: || Box::new(Spell),
    });
    c.add(Adapter {
        kind: "test.transcode",
        from: Kind::Bytes,
        to: Kind::Bytes,
        class: AdapterClass::Lossless,
        cost: 1,
        settings: |f, want| {
            let mp2 = f.media.as_ref().is_some_and(|m| m.codec == Codec::Mp2);
            let aac = want.media.iter().any(|s| s.codec == Codec::Aac);
            (mp2 && aac).then(no_values)
        },
        make: || Box::new(Transcode),
    });
    Arc::new(c)
}

#[test]
fn an_item_adapter_of_the_catalog_joins_two_item_types() {
    let letters = Port::typed::<Letter>("in");
    // Without the adapter the item types do not join.
    let e = GraphBuilder::new()
        .add("words", Words)
        .add("rx", Sink::new(letters.clone()).0)
        .input("words")
        .connect("words", "rx")
        .build()
        .unwrap_err();
    assert!(
        matches!(&e, GraphError::Connect { error, .. } if matches!(**error, ConnectError::ItemTypeMismatch { .. })),
        "{e}"
    );
    // The catalog of the host's registry supplies the item adapter of the graph built in code.
    let mut g = GraphBuilder::new()
        .catalog(&test_catalog())
        .add("words", Words)
        .add("rx", Sink::new(letters).0)
        .input("words")
        .connect("words", "rx")
        .build()
        .unwrap();
    let r = g.validate(Format::iq(48e3, 0.0, 0.0)).unwrap();
    let kinds: Vec<_> = r.adapters.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, ["test.spell"]);
}

#[test]
fn a_codec_that_the_input_does_not_take_is_converted() {
    let aac = media::MediaSpec::new(Codec::Aac, Packaging::Adts);
    let (sink, _) = Sink::new(Port::media("in", &[aac]));
    let mut g = GraphBuilder::new()
        .catalog(&test_catalog())
        .add("enc", Mp2Out)
        .add("rx", sink)
        .input("enc")
        .connect("enc", "rx")
        .build()
        .unwrap();
    let r = g.validate(Format::iq(48e3, 0.0, 0.0)).unwrap();
    let kinds: Vec<_> = r.adapters.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, ["test.transcode"]);
}

/// A decoder that counts the spectrum frames of its signal beside the samples, on a helper thread when requested.
#[derive(Default)]
struct Frames(Arc<Mutex<(usize, usize)>>, bool);

impl Decoder for Frames {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "test.frames"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn extra_input(&self) -> Option<Port> {
        Some(Port::spectrum(
            "spectrum",
            SpectrumSpec {
                size: SizeRule::Bins(components::SpectrumSize::S1024),
                window: components::SpectrumWindow::Hann,
                average: 1,
                frames: FrameRule::BackToBack,
                scale: components::SpectrumScale::Power,
            },
        ))
    }
    fn extra(&mut self, format: &Format, input: &stream::Buffer) {
        assert_eq!(format.frame_len, 1024);
        if let stream::Data::Spectrum(f) = &input.data {
            self.0.lock().unwrap().1 += f.len();
        }
    }
    fn configure(&mut self, _: &Format) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, x: &[Complex32], _: u64, _: &mut Outbox) {
        if self.1 {
            // Slow enough that the graph calls again while the helper holds the decoder.
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        self.0.lock().unwrap().0 += x.len();
    }
    fn helper(&self) -> Helper {
        if self.1 {
            Helper::WhenLive {
                batch: 4096,
                backlog: 1 << 20,
            }
        } else {
            Helper::Never
        }
    }
}

#[test]
fn a_decoder_reads_the_shared_spectrum_of_its_signal_beside_it() {
    let seen = Arc::new(Mutex::new((0, 0)));
    let mut g = GraphBuilder::new()
        .add(
            "dec",
            DecoderComponent::new(Frames(Arc::clone(&seen), false)),
        )
        .input("dec.in")
        .input("dec.spectrum")
        .build()
        .unwrap();
    g.configure_input(Format::iq(48e3, 0.0, 0.0)).unwrap();
    assert_eq!(g.report().adapters[0].kind, "dsp.spectrum");
    g.process(&vec![Complex32::default(); 10_240], &mut Output::default());
    assert_eq!(*seen.lock().unwrap(), (10_240, 10));
}

#[test]
fn a_decoder_on_a_helper_thread_gets_the_spectrum_frames_of_every_call() {
    let seen = Arc::new(Mutex::new((0, 0)));
    let mut g = GraphBuilder::new()
        .add(
            "dec",
            DecoderComponent::new(Frames(Arc::clone(&seen), true)),
        )
        .input("dec.in")
        .input("dec.spectrum")
        .build()
        .unwrap();
    g.set_context(RunContext {
        live: true,
        ..RunContext::default()
    });
    g.configure_input(Format::iq(48e3, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    for _ in 0..40 {
        g.process(&vec![Complex32::default(); 1024], &mut out);
    }
    g.finish(&mut out);
    assert_eq!(*seen.lock().unwrap(), (40 * 1024, 40));
}

/// Adapters of a type that is not the catalog of this crate.
#[derive(Debug)]
struct Foreign;

impl sdrkit_core::AdapterCatalog for Foreign {
    fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

#[test]
fn a_stack_takes_the_hosts_adapter_catalog_from_its_configuration() {
    let schema = Arc::new(sdrkit_core::Schema::default());
    let plain = sdrkit_core::StackConfig::new(Values::defaults(&schema));
    assert!(Catalog::of(&plain).is_ok());
    let mut own = Catalog::standard();
    let n = own.adapters().len();
    own.add(own.adapters()[0].clone());
    let cfg = plain.clone().with_adapters(Arc::new(own));
    assert_eq!(Catalog::of(&cfg).unwrap().adapters().len(), n + 1);
    let cfg = plain.with_adapters(Arc::new(Foreign));
    assert!(matches!(
        Catalog::of(&cfg),
        Err(GraphError::ForeignAdapters)
    ));
    assert!(matches!(
        sdrkit_core::ConfigError::from(GraphError::ForeignAdapters),
        sdrkit_core::ConfigError::Stage(_)
    ));
}
