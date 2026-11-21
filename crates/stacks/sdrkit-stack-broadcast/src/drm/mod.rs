//! DRM: DRM30 (robustness modes A to D) and DRM+ (mode E): the receiver, its settings, and the clean test signal.
//!
//! Standard: ETSI ES 201 980.

pub mod audio;
pub mod data;
pub mod fac;
pub mod interleave;
pub mod mlc;
pub mod params;
pub mod results;
pub mod rx;
pub mod tx;

use params::{Qam, Robustness};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, GenConfig, Generator, Receiver, Setting, SignalInfo, StackConfig,
};
use sdrkit_graph::{Catalog, GraphError, StackGraph};
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::seq::PRBS9_DISPERSAL;
use std::collections::VecDeque;
use tx::{AudioKind, DataConfig, DrmTransmitter, TxConfig};

/// The stack id.
pub const ID: &str = "drm";
/// The minimum input rate (DRM30). DRM+ needs [`MIN_RATE_E`].
pub const MIN_RATE: f64 = 48e3;
/// The minimum input rate for DRM+.
pub const MIN_RATE_E: f64 = 100e3;

/// Adds the energy dispersal PRBS of clause 7.2.2 to `bits` (one bit per byte), from its first bit.
pub fn disperse(bits: &mut [u8]) {
    Additive::new(PRBS9_DISPERSAL).apply_bits(bits);
}

/// Makes the DRM receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The DRM receiver: its graph in a [`StackGraph`]. A retune resets the tracker and the decoder.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), rx::graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE, "DRM needs a sample rate of at least 48 ksps")
        .command(
            "service.select",
            sdrkit_graph::CommandTarget::of("audio", "service.select"),
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("ofdm", "clear"))?
        .on_retune(sdrkit_graph::CommandTarget::of("decoder", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

sdrkit_core::choice_enum! {
    /// The occupancy of the test signal.
    #[derive(Default)]
    pub enum OccupancyChoice {
        /// The usual occupancy of the mode.
        #[default]
        Usual = ("usual", "Usual"),
        /// 4.5 kHz.
        O0 = ("0", "4.5 kHz"),
        /// 5 kHz.
        O1 = ("1", "5 kHz"),
        /// 9 kHz.
        O2 = ("2", "9 kHz"),
        /// 10 kHz.
        O3 = ("3", "10 kHz"),
        /// 18 kHz.
        O4 = ("4", "18 kHz"),
        /// 20 kHz.
        O5 = ("5", "20 kHz"),
    }
}

sdrkit_core::choice_enum! {
    /// The MSC constellation of the test signal.
    #[derive(Default)]
    pub enum MscQamChoice {
        /// 64-QAM (16-QAM in mode E).
        #[default]
        High = ("high", "64-QAM (E: 16-QAM)"),
        /// 16-QAM (4-QAM in mode E).
        Low = ("low", "16-QAM (E: 4-QAM)"),
    }
}

sdrkit_core::choice_enum! {
    /// The protection level of part B of the test signal.
    #[derive(Default)]
    pub enum ProtectionChoice {
        /// The usual level for the mode and constellation.
        #[default]
        Usual = ("usual", "Usual"),
        /// Level 0.
        P0 = ("0", "0"),
        /// Level 1.
        P1 = ("1", "1"),
        /// Level 2.
        P2 = ("2", "2"),
        /// Level 3.
        P3 = ("3", "3"),
    }
}

sdrkit_core::choice_enum! {
    /// The interleaver of the test signal.
    #[derive(Default)]
    pub enum InterleaverChoice {
        /// The long interleaver.
        #[default]
        Long = ("long", "Long"),
        /// The short interleaver.
        Short = ("short", "Short"),
    }
}

sdrkit_core::choice_enum! {
    /// The audio of the test signal.
    #[derive(Default)]
    pub enum AudioChoice {
        /// A melody.
        #[default]
        Melody = ("melody", "Melody"),
        /// A 1 kHz tone.
        Tone = ("tone", "1 kHz tone"),
        /// Silence.
        Silence = ("silence", "Silence"),
        /// A noise-like signal.
        Noise = ("noise", "Noise-like"),
        /// Bytes that are not AAC.
        NotAac = ("not_aac", "Not AAC"),
    }
}

sdrkit_core::choice_enum! {
    /// The data service of the test signal, by its DAB user application type (TS 101 756 table 16).
    #[derive(Default)]
    pub enum DataChoice {
        /// None.
        #[default]
        None = ("none", "None"),
        /// MOT slide show.
        Slides = ("2", "MOT slide show"),
        /// MOT broadcast website.
        Website = ("3", "MOT website"),
        /// TPEG.
        Tpeg = ("4", "TPEG"),
        /// EPG.
        Epg = ("7", "EPG"),
        /// Journaline.
        Journaline = ("1098", "Journaline"),
    }
}

impl DataChoice {
    /// The user application type, `None` for no data service.
    pub fn app_type(self) -> Option<u16> {
        match self {
            DataChoice::None => None,
            DataChoice::Slides => Some(2),
            DataChoice::Website => Some(3),
            DataChoice::Tpeg => Some(4),
            DataChoice::Epg => Some(7),
            DataChoice::Journaline => Some(1098),
        }
    }
}

/// Test signal: the robustness mode.
pub const GEN_ROBUSTNESS: Setting<field::Choice<Robustness>> =
    Setting::choice("gen_robustness", "Robustness mode", Robustness::B)
        .section("Test signal")
        .advanced();
/// Test signal: the occupancy.
pub const GEN_OCCUPANCY: Setting<field::Choice<OccupancyChoice>> =
    Setting::choice("gen_occupancy", "Occupancy", OccupancyChoice::Usual)
        .section("Test signal")
        .advanced();
/// Test signal: the MSC constellation.
pub const GEN_MSC_QAM: Setting<field::Choice<MscQamChoice>> =
    Setting::choice("gen_msc_qam", "MSC", MscQamChoice::High)
        .section("Test signal")
        .advanced();
/// Test signal: the protection level of part B.
pub const GEN_PROTECTION_B: Setting<field::Choice<ProtectionChoice>> =
    Setting::choice("gen_protection_b", "Protection", ProtectionChoice::Usual)
        .section("Test signal")
        .advanced();
/// Test signal: the interleaver.
pub const GEN_INTERLEAVER: Setting<field::Choice<InterleaverChoice>> =
    Setting::choice("gen_interleaver", "Interleaver", InterleaverChoice::Long)
        .section("Test signal")
        .advanced();
/// Test signal: the audio.
pub const GEN_AUDIO: Setting<field::Choice<AudioChoice>> =
    Setting::choice("gen_audio", "Audio", AudioChoice::Melody)
        .section("Test signal")
        .advanced();
/// Test signal: a text message.
pub const GEN_TEXT_MESSAGE: Setting<field::Bool> =
    Setting::new("gen_text_message", "Text message", "", true)
        .section("Test signal")
        .advanced();
/// Test signal: the data service.
pub const GEN_DATA: Setting<field::Choice<DataChoice>> =
    Setting::choice("gen_data", "Data service", DataChoice::None)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the DRM stack: the test signal's.
    pub enum DrmSetting {
        /// Test signal: the robustness mode.
        GenRobustness = GEN_ROBUSTNESS,
        /// Test signal: the occupancy.
        GenOccupancy = GEN_OCCUPANCY,
        /// Test signal: the MSC constellation.
        GenMscQam = GEN_MSC_QAM,
        /// Test signal: the protection level of part B.
        GenProtectionB = GEN_PROTECTION_B,
        /// Test signal: the interleaver.
        GenInterleaver = GEN_INTERLEAVER,
        /// Test signal: the audio.
        GenAudio = GEN_AUDIO,
        /// Test signal: a text message.
        GenTextMessage = GEN_TEXT_MESSAGE,
        /// Test signal: the data service.
        GenData = GEN_DATA,
    }
}

/// The DRM test signal: the transmitter's clean signal at its OFDM rate, resampled to the requested rate. Noise, offsets,
/// clock errors and the channels of Annex B come from the framework's `gen_*` settings.
pub struct DrmGenerator {
    tx: DrmTransmitter,
    out_rate: f64,
    bandwidth: f64,
    rs: Option<Resampler<Complex32>>,
    pending: VecDeque<Complex32>,
    frame: Vec<Complex32>,
    out: Vec<Complex32>,
}

impl DrmGenerator {
    /// A generator at `out_rate`; `None` for a configuration the standard does not allow.
    pub fn new(cfg: TxConfig, out_rate: f64) -> Option<DrmGenerator> {
        let mode = cfg.mode;
        let bandwidth = params::occupancy_khz(mode, cfg.occupancy) * 1e3;
        let tx = DrmTransmitter::new(cfg)?;
        let rate = mode.rate();
        let rs = if (out_rate - rate).abs() > 1e-9 {
            Some(Resampler::with_quality(rate, out_rate, 0.45, 80.0).ok()?)
        } else {
            None
        };
        Some(DrmGenerator {
            tx,
            out_rate,
            bandwidth,
            rs,
            pending: VecDeque::new(),
            frame: Vec::new(),
            out: Vec::new(),
        })
    }
    /// The transmitter configuration of the stack's `gen_*` settings.
    pub fn config_of(cfg: &GenConfig) -> TxConfig {
        let s = &cfg.settings;
        let mode = s.get(&GEN_ROBUSTNESS);
        let mut t = TxConfig::usual(mode);
        t.seed = cfg.seed;
        let occ = s.get(&GEN_OCCUPANCY);
        if occ != OccupancyChoice::Usual {
            t.occupancy = sdrkit_core::Choice::index(occ) as u8 - 1;
        }
        if s.get(&GEN_MSC_QAM) == MscQamChoice::Low {
            t.qam = if mode == Robustness::E {
                Qam::Q4
            } else {
                Qam::Q16
            };
        }
        let prot = s.get(&GEN_PROTECTION_B);
        t.protection = if prot == ProtectionChoice::Usual {
            tx::usual_protection(mode, t.qam)
        } else {
            sdrkit_core::Choice::index(prot) as u8 - 1
        };
        t.short_interleaver = s.get(&GEN_INTERLEAVER) == InterleaverChoice::Short;
        t.audio = match s.get(&GEN_AUDIO) {
            AudioChoice::Tone => AudioKind::Tone,
            AudioChoice::Silence => AudioKind::Silence,
            AudioChoice::Noise => AudioKind::Noise,
            AudioChoice::NotAac => AudioKind::NotAac,
            AudioChoice::Melody => AudioKind::Melody,
        };
        t.text = s.get(&GEN_TEXT_MESSAGE);
        t.data = s.get(&GEN_DATA).app_type().map(|app_type| DataConfig {
            app_type,
            ..Default::default()
        });
        t
    }
    /// The transmitter.
    pub fn transmitter(&self) -> &DrmTransmitter {
        &self.tx
    }
}

impl Generator for DrmGenerator {
    fn rate_hz(&self) -> f64 {
        self.out_rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        while self.pending.len() < out.len() {
            self.frame.clear();
            self.tx.next_frame(&mut self.frame);
            match &mut self.rs {
                Some(r) => {
                    self.out.clear();
                    r.process(&self.frame, &mut self.out);
                    self.pending.extend(self.out.iter());
                }
                None => self.pending.extend(self.frame.iter()),
            }
        }
        for o in out.iter_mut() {
            *o = self.pending.pop_front().unwrap_or_default();
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The mean power is 1 over the whole frame (pilots and guard included), and the SNR is stated in the nominal channel
        // width (ES 201 980 annex A).
        Some(SignalInfo::new(self.bandwidth))
    }
}

/// Makes the generator.
///
/// # Errors
///
/// [`ConfigError::Stage`] if the standard does not allow the configuration.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    match DrmGenerator::new(DrmGenerator::config_of(cfg), cfg.rate_hz) {
        Some(g) => Ok(Box::new(g)),
        None => Err(crate::silence::NoSignal {
            rate_hz: cfg.rate_hz,
        }
        .refused()),
    }
}
