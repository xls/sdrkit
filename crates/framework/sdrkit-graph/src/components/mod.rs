//! The standard components: thin wrappers around `sdrkit-algo-dsp` that add ports, formats, tags and settings.
//!
//! | Type name | Component | Ports | What it does |
//! |---|---|---|---|
//! | `dsp.dc_removal` | [`DcRemoval`] | iq -> iq | running mean of I and Q subtracted |
//! | `dsp.iq_correction` | [`IqCorrection`] | iq -> iq | blind gain and phase correction |
//! | `dsp.noise_blanker` | [`NoiseBlanker`] | iq -> iq | impulse blanker |
//! | `dsp.shift` | [`Shift`] | iq -> iq | frequency shift by an oscillator |
//! | `dsp.fir` | [`FirFilter`] | iq or real -> same | low-pass, high-pass or band-pass FIR |
//! | `dsp.decimator` | [`Decimate`] | iq, real or audio -> same | integer decimation with anti-alias filtering |
//! | `dsp.resampler` | [`Resample`] | iq, real or audio -> same | to a set rate |
//! | `dsp.channel` | [`Channel`] | iq -> iq | shift, filter and decimate one channel; width settable live |
//! | `dsp.agc` | [`AgcComponent`] | iq or real -> same | automatic gain control |
//! | `dsp.power_meter` | [`PowerMeterSink`] | iq or real -> (results) | level in dBFS |
//! | `dsp.spectrum` | [`Spectrum`] | iq or real -> spectrum | windowed FFT frames in dBFS or linear power |
//! | `dsp.fm_demod` | [`FmDemod`] | iq -> real | quadrature discriminator, normalised to the deviation |
//! | `dsp.am_demod` | [`AmDemod`] | iq -> real | envelope or synchronous (carrier tracking) detector |
//! | `dsp.ssb_demod` | [`SsbDemod`] | iq -> real | USB, LSB and CW with a BFO offset |
//! | `dsp.deemphasis` | [`DeemphasisComponent`] | real or audio -> same | FM de-emphasis with a time constant (µs, 0 for off) |
//! | `dsp.biquad` | [`BiquadComponent`] | real or audio -> same | one second-order section |
//! | `dsp.squelch` | [`Squelch`] | iq or real -> same | mutes below a level, or above a noise level |
//! | `dsp.audio_resampler` | [`AudioResampler`] | audio -> audio | to the audio rate |
//! | `dsp.real_to_audio` | [`RealToAudio`] | real -> audio | real to mono audio |
//! | `dsp.null_sink` | [`NullSink`] | any -> | discards (and counts) |
//! | `host.out` | [`HostOut`] | audio -> | to the host's audio stream of the graph |
//! | `host.in` | [`HostIn`] | IQ -> audio | the audio that the host gives to the session (a microphone), mono or stereo at 48 kHz |
//! | `tx.sink` | [`TxSink`] | iq -> | to the host's transmit stream |
//! | `gen.stack` | [`StackGenerator`] | -> iq | a stack's test signal |
//! | `net.sink` | [`NetSink`] | iq or audio -> | to a UDP receiver or a TCP server |
//! | `dsp.shift_resample` | [`ShiftResample`] | iq -> iq | shift and rate change in one pass |
//! | `dsp.downmix` | [`Downmix`] | audio -> mono audio | mean of the channels |
//! | `dsp.select_channels` | [`SelectChannels`] | audio -> audio | the first channels of each frame |
//! | `dsp.duplicate` | [`Duplicate`] | mono audio -> audio | the same sample on every channel |
//! | `dsp.analytic` | [`Analytic`] | real -> iq | the analytic signal (Hilbert transform) |
//! | `dsp.hard_decision` | [`HardDecision`] | llr -> hard bits | the sign of each soft bit |
//! | `dsp.llr_rescale` | [`LlrRescale`] | llr -> llr | soft bits to another scale |
//! | `dsp.to_iq` | [`ToIq`] | bytes -> iq | raw complex samples to `Iq` |
//! | `dsp.to_real` | [`ToReal`] | bytes -> real | raw samples to `Real` |
//! | `dsp.sample_guard` | [`SampleGuard`](crate::SampleGuard) | iq -> iq | fixes samples that are not finite or far beyond full scale |

mod adapters;
mod audio;
mod demod;
mod endpoints;
mod filters;
mod frontend;
mod level;
mod spectrum;

pub use adapters::{
    convert_channels, iq_from_bytes, AdapterInput, Analytic, AnalyticSetting, ChannelCountSetting, Downmix,
    Duplicate, HardDecision, LlrRescale, LlrRescaleSetting, SelectChannels, ShiftResample,
    ShiftResampleSetting, ToIq, ToReal,
};
pub use audio::{AudioResampler, NullSink, RealToAudio, RealToAudioSetting};
pub use demod::{AmDemod, AmDemodFields, AmDetector, FmDemod, Sideband, SsbDemod};
pub use endpoints::{
    EndpointError, HostData, HostIn, HostOut, HostPort, HostStreams, MakeGenerator, NetProtocol,
    NetSample, NetSink, NetSinkFields, NetSinkSetting, StackGenerator, StackGeneratorFields,
    StackGeneratorSetting, TxSink, HOST_IN_RATE,
};
pub use filters::{
    BiquadComponent, BiquadShape, Channel, ChannelFilter, Decimate, DeemphasisComponent,
    DeemphasisSetting, FirFilter, FirResponse, Resample, ResampleSetting, Shift, ShiftChange,
    ShiftSetting,
};
pub use frontend::{
    DcRemoval, DcRemovalFields, IqCorrection, IqCorrectionFields, NoiseBlanker, NoiseBlankerFields,
};
pub use level::{
    AgcComponent, PowerMeterFields, PowerMeterSink, Squelch, SquelchFields, SquelchMode,
};
pub(crate) use spectrum::FPS_DEFAULT;
pub use spectrum::{
    FrameTiming, Spectrum, SpectrumScale, SpectrumSetting, SpectrumSize, SpectrumWindow,
};

use crate::desc::Registry;
use sdrkit_core::interface::{Family, StackShape};
use std::sync::OnceLock;

/// The shared interface of the standard components: the result set of each component that publishes results. A
/// family whose stacks list one of them names this interface with `Family::uses`, and lists the set under the name
/// of the component in its graph: `.component("dc", |c| c.fields::<DcRemovalFields>())`.
pub fn interface() -> &'static Family {
    static SHARED: OnceLock<Family> = OnceLock::new();
    SHARED.get_or_init(|| {
        Family::shared("graph", 1, 0).stack(
            StackShape::new("graph")
                .component("dc_removal", |c| c.fields::<DcRemovalFields>())
                .component("iq_correction", |c| c.fields::<IqCorrectionFields>())
                .component("noise_blanker", |c| c.fields::<NoiseBlankerFields>())
                .component("power_meter", |c| c.fields::<PowerMeterFields>())
                .component("squelch", |c| c.fields::<SquelchFields>())
                .component("am", |c| c.fields::<AmDemodFields>())
                .component("generator", |c| c.fields::<StackGeneratorFields>())
                .component("net_sink", |c| c.fields::<NetSinkFields>()),
        )
    })
}

/// Adds every standard component to `r`, under the names in the table above, and the standard adapters
/// ([`Catalog::standard`](crate::Catalog::standard)).
pub fn register(r: &mut Registry) {
    r.add("dsp.dc_removal", || Box::new(DcRemoval::default()));
    r.add("dsp.iq_correction", || Box::new(IqCorrection::default()));
    r.add("dsp.noise_blanker", || Box::new(NoiseBlanker::default()));
    r.add("dsp.shift", || Box::new(Shift::default()));
    r.add("dsp.fir", || Box::new(FirFilter::default()));
    r.add("dsp.decimator", || Box::new(Decimate::default()));
    r.add("dsp.resampler", || Box::new(Resample::default()));
    r.add("dsp.channel", || Box::new(Channel::default()));
    r.add("dsp.agc", || Box::new(AgcComponent::default()));
    r.add("dsp.power_meter", || Box::new(PowerMeterSink::default()));
    r.add("dsp.spectrum", || Box::new(Spectrum::default()));
    r.add("dsp.fm_demod", || Box::new(FmDemod::default()));
    r.add("dsp.am_demod", || Box::new(AmDemod::default()));
    r.add("dsp.ssb_demod", || Box::new(SsbDemod::default()));
    r.add(
        "dsp.deemphasis",
        || Box::new(DeemphasisComponent::default()),
    );
    r.add("dsp.biquad", || Box::new(BiquadComponent::default()));
    r.add("dsp.squelch", || Box::new(Squelch::default()));
    r.add(
        "dsp.audio_resampler",
        || Box::new(AudioResampler::default()),
    );
    r.add("dsp.real_to_audio", || Box::new(RealToAudio::default()));
    r.add("dsp.null_sink", || Box::new(NullSink::default()));
    r.add("host.out", || Box::new(HostOut::default()));
    r.add("host.in", || Box::new(HostIn::default()));
    r.add("tx.sink", || Box::new(TxSink::default()));
    r.add("gen.stack", || Box::new(StackGenerator::default()));
    r.add("net.sink", || Box::new(NetSink::default()));
    r.add("dsp.shift_resample", || Box::new(ShiftResample::default()));
    r.add("dsp.downmix", || Box::new(Downmix));
    r.add(
        "dsp.select_channels",
        || Box::new(SelectChannels::default()),
    );
    r.add("dsp.duplicate", || Box::new(Duplicate::default()));
    r.add("dsp.analytic", || Box::new(Analytic::default()));
    r.add("dsp.hard_decision", || Box::new(HardDecision));
    r.add("dsp.llr_rescale", || Box::new(LlrRescale::default()));
    r.add("dsp.to_iq", || Box::new(ToIq::default()));
    r.add("dsp.to_real", || Box::new(ToReal::default()));
    r.add("dsp.sample_guard", || {
        Box::new(crate::SampleGuard::default())
    });
    for a in crate::adapt::Catalog::standard().adapters() {
        r.add_adapter(a.clone());
    }
}

pub(crate) use crate::settings::Every;

/// A filter or resampler of sdrkit-algo-dsp refused its parameters.
pub(crate) fn dsp(e: sdrkit_algo_dsp::DspError) -> sdrkit_core::ConfigError {
    sdrkit_core::ConfigError::Stage(sdrkit_core::StageError::new(e))
}

/// A DSP object for each sample type a generic component may see.
pub(crate) enum PerKind<C, R> {
    None,
    Iq(C),
    Real(R),
    Audio(Box<dyn AudioEngine>),
}

/// A DSP object that turns samples of type `T` into more samples of that type.
pub(crate) trait Engine<T> {
    /// Appends the output for `x` to `out`.
    fn run(&mut self, x: &[T], out: &mut impl sdrkit_algo_dsp::Append<T>);
}

impl<T: sdrkit_algo_dsp::Sample> Engine<T> for sdrkit_algo_dsp::decim::Decimator<T> {
    fn run(&mut self, x: &[T], out: &mut impl sdrkit_algo_dsp::Append<T>) {
        self.process(x, out);
    }
}

impl<T: sdrkit_algo_dsp::Sample> Engine<T> for sdrkit_algo_dsp::resample::Resampler<T> {
    fn run(&mut self, x: &[T], out: &mut impl sdrkit_algo_dsp::Append<T>) {
        self.process(x, out);
    }
}

/// A DSP object on interleaved audio of a fixed channel count.
pub(crate) trait AudioEngine: Send {
    /// Appends the output for the interleaved samples `x` to `out`.
    fn run(&mut self, x: &[f32], out: &mut Vec<f32>);
}

/// One object on mono audio, with no copy.
pub(crate) struct Mono<E>(pub(crate) E);

impl<E: Engine<f32> + Send> AudioEngine for Mono<E> {
    fn run(&mut self, x: &[f32], out: &mut Vec<f32>) {
        self.0.run(x, out);
    }
}

/// One object on frames of `N` channels. The frames out go straight into the interleaved output.
pub(crate) struct Framed<const N: usize, E> {
    pub(crate) engine: E,
}

impl<const N: usize, E: Engine<sdrkit_algo_dsp::Frame<N>> + Send> AudioEngine for Framed<N, E> {
    fn run(&mut self, x: &[f32], out: &mut Vec<f32>) {
        let mut w = sdrkit_algo_dsp::FrameWriter::<N>(out);
        self.engine.run(sdrkit_algo_dsp::Frame::view(x), &mut w);
    }
}

/// The most audio channels a frame engine takes.
pub(crate) const MAX_CHANNELS: u16 = 8;

/// One [`AudioEngine`] for `$channels` channels: `$make` builds the object with the type name `$t` bound to the sample type
/// (`f32` for mono, `Frame<N>` otherwise) and returns `Result<_, DspError>`. More than [`MAX_CHANNELS`] channels is a
/// `DspError`.
macro_rules! audio_engine {
    ($channels:expr, $t:ident => $make:expr) => {{
        use sdrkit_algo_dsp::{DspError, Frame, Param};
        use $crate::components::{AudioEngine, Framed, Mono};
        fn framed<const N: usize, E>(engine: E) -> Box<dyn AudioEngine>
        where
            Framed<N, E>: AudioEngine + 'static,
        {
            Box::new(Framed::<N, E> { engine })
        }
        let r: Result<Box<dyn AudioEngine>, DspError> = match $channels {
            0 | 1 => {
                type $t = f32;
                ($make).map(|e| Box::new(Mono(e)) as Box<dyn AudioEngine>)
            }
            2 => {
                type $t = Frame<2>;
                ($make).map(framed::<2, _>)
            }
            3 => {
                type $t = Frame<3>;
                ($make).map(framed::<3, _>)
            }
            4 => {
                type $t = Frame<4>;
                ($make).map(framed::<4, _>)
            }
            5 => {
                type $t = Frame<5>;
                ($make).map(framed::<5, _>)
            }
            6 => {
                type $t = Frame<6>;
                ($make).map(framed::<6, _>)
            }
            7 => {
                type $t = Frame<7>;
                ($make).map(framed::<7, _>)
            }
            8 => {
                type $t = Frame<8>;
                ($make).map(framed::<8, _>)
            }
            n => Err(DspError::OutOfRange {
                param: Param::Channels,
                min: 1.0,
                max: f64::from($crate::components::MAX_CHANNELS),
                got: f64::from(n),
            }),
        };
        r
    }};
}
pub(crate) use audio_engine;

#[cfg(test)]
pub(crate) mod testing {
    //! Runs a single component without a graph.
    use crate::component::Component;
    use crate::stream::{Buffer, Format};
    use crate::Io;

    /// Configures `c` for `inputs`, then runs `pieces` (each a list of input buffers' contents set by `fill`) and returns the output
    /// buffers after each call.
    pub struct Harness {
        pub ins: Vec<Buffer>,
        pub outs: Vec<Buffer>,
        pub formats: Vec<Format>,
        ids_in: Vec<usize>,
        ids_out: Vec<usize>,
    }

    impl Harness {
        pub fn new(c: &mut dyn Component, inputs: Vec<Format>) -> Harness {
            let outs = c.configure(&inputs).expect("configure");
            let n = inputs.len();
            let ids_in = (0..n).collect();
            let ids_out = (n..n + outs.len()).collect();
            let ins = inputs.iter().map(|f| Buffer::new(&f.kind)).collect();
            let ob = outs.iter().map(|f| Buffer::new(&f.kind)).collect();
            let mut formats = inputs;
            formats.extend(outs);
            Harness {
                ins,
                outs: ob,
                formats,
                ids_in,
                ids_out,
            }
        }
        /// One call: `fill` writes the inputs; returns nothing, read `outs` after.
        pub fn run(&mut self, c: &mut dyn Component, fill: impl FnOnce(&mut [Buffer])) {
            for b in self.ins.iter_mut().chain(self.outs.iter_mut()) {
                b.advance();
            }
            fill(&mut self.ins);
            let mut io = Io::standalone(
                &self.ins,
                &self.formats,
                &self.ids_in,
                &self.ids_out,
                &mut self.outs,
            );
            c.process(&mut io);
        }
    }
}
