//! STANAG 4539 high-rate serial tone (the waveform of MIL-STD-188-110C Appendix C): 3200 to 12800 bit/s in QPSK, 8-PSK and 16-,
//! 32- and 64-QAM; a 287-symbol preamble, frames of 256 symbols with 31-symbol mini-probes, the 103-symbol block every 72 frames.
//!
//! Standard: MIL-STD-188-110C with Change 1, Appendix C.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [s4539: matched filter, block search, probe channel estimates, MMSE DFE, deinterleaver, tail-biting Viterbi] -> text, D_PDUs, events
//! ```

pub mod rx;
pub mod signal;

use crate::receiver::GEN_SECTION;
use crate::serial::gen::SymbolGen;
use crate::serial::TEST_TEXT;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Schema, Setting,
    SettingSet, StackConfig, StackDescriptor,
};
use signal::Rate;
use std::sync::Arc;

/// The multipath equaliser: on, an MMSE decision-feedback equaliser from the probes; off, one tap.
pub const EQUALISER: Setting<field::Bool> =
    Setting::new("equaliser", "Multipath equaliser", "", true)
        .help("Off: one channel tap between the probes, for a channel without multipath.");

sdrkit_core::choice_enum! {
    /// The data rate of the test signal.
    pub enum GenBps {
        /// 3200 bit/s.
        B3200 = ("3200", "3200 bit/s (QPSK)"),
        /// 4800 bit/s.
        B4800 = ("4800", "4800 bit/s (8-PSK)"),
        /// 6400 bit/s.
        B6400 = ("6400", "6400 bit/s (16-QAM)"),
        /// 8000 bit/s.
        B8000 = ("8000", "8000 bit/s (32-QAM)"),
        /// 9600 bit/s.
        B9600 = ("9600", "9600 bit/s (64-QAM)"),
        /// 12800 bit/s.
        B12800 = ("12800", "12800 bit/s (64-QAM, uncoded)"),
    }
}

sdrkit_core::choice_enum! {
    /// The interleaver of the test signal, in frames.
    pub enum GenFrames {
        /// 1 frame.
        F1 = ("1", "Ultra short (1 frame)"),
        /// 3 frames.
        F3 = ("3", "Very short (3 frames)"),
        /// 9 frames.
        F9 = ("9", "Short (9 frames)"),
        /// 18 frames.
        F18 = ("18", "Medium (18 frames)"),
        /// 36 frames.
        F36 = ("36", "Long (36 frames)"),
        /// 72 frames.
        F72 = ("72", "Very long (72 frames)"),
    }
}

/// The data rate of the test signal.
pub const GEN_BPS: Setting<field::Choice<GenBps>> =
    Setting::choice("gen_bps", "Data rate", GenBps::B4800)
        .section(GEN_SECTION)
        .advanced();
/// The interleaver of the test signal.
pub const GEN_FRAMES: Setting<field::Choice<GenFrames>> =
    Setting::choice("gen_frames", "Interleaver", GenFrames::F3)
        .help("12800 bit/s always has one frame.")
        .section(GEN_SECTION)
        .advanced();
/// AGC blocks before the sync.
pub const GEN_AGC_BLOCKS: Setting<field::I64> = Setting::new("gen_agc_blocks", "AGC blocks", "", 0)
    .range(0.0, 7.0, 1.0)
    .section(GEN_SECTION)
    .advanced();
/// The end-of-message pattern after the text.
pub const GEN_EOM: Setting<field::Bool> = Setting::new("gen_eom", "End of message", "", true)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the STANAG 4539 receiver.
    pub enum S4539RxSetting {
        /// Multipath equaliser.
        Equaliser = EQUALISER,
    }
}

sdrkit_core::settings! {
    /// The settings of the STANAG 4539 stack.
    pub enum S4539Setting {
        /// Multipath equaliser.
        Equaliser = EQUALISER,
        /// Data rate of the test signal.
        GenBps = GEN_BPS,
        /// Interleaver of the test signal.
        GenFrames = GEN_FRAMES,
        /// AGC blocks of the test signal.
        GenAgcBlocks = GEN_AGC_BLOCKS,
        /// End of message of the test signal.
        GenEom = GEN_EOM,
    }
}

/// The silence between two transmissions of the test signal, symbols (0.5 s).
pub const GEN_GAP: usize = 1200;

/// The symbols of one transmission of `bytes`.
pub fn test_symbols(
    rate: Rate,
    frames: usize,
    agc: usize,
    bytes: &[u8],
    eom: bool,
) -> Vec<sdrkit_core::Complex32> {
    let frames = if rate == Rate::B12800 { 1 } else { frames };
    signal::transmission(
        rate,
        frames,
        agc,
        &signal::message_bits(rate, frames, bytes, eom),
    )
}

/// The test signal at `cfg.rate_hz`: one transmission of the test text, then 0.5 s of silence, repeated.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let rate = Rate::ALL[usize::from(sdrkit_core::Choice::index(s.get(&GEN_BPS)))];
    let frames = signal::FRAMES[usize::from(sdrkit_core::Choice::index(s.get(&GEN_FRAMES)))];
    let agc = s.get(&GEN_AGC_BLOCKS).clamp(0, 7) as usize;
    let mut sym = test_symbols(rate, frames, agc, TEST_TEXT.as_bytes(), s.get(&GEN_EOM));
    sym.extend(std::iter::repeat_n(
        sdrkit_core::Complex32::default(),
        GEN_GAP,
    ));
    Ok(Box::new(SymbolGen::serial(cfg.rate_hz, sym)))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    S4539Setting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("s4539"),
        "STANAG 4539",
        rx::S4539Decoder::new(),
        cfg,
    )
}

/// STANAG 4539.
pub static S4539: StackDescriptor = StackDescriptor {
    id: "s4539",
    name: "STANAG 4539 high-rate serial tone",
    group: "HF utility",
    blurb: "3200 to 12800 bit/s on 1800 Hz (QPSK to 64-QAM): rate and interleaver from the preamble, probe-based MMSE equaliser, tail-biting code; text and STANAG 5066 D_PDUs.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..crate::ms110::hf_tuning(crate::ms110::TEST_SIGNAL_HZ)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::S4539),
    benchmarks: Some(crate::benchmarks),
};
