//! MIL-STD-188-110 serial tone: 2400 Bd 8-PSK on 1800 Hz, 75 to 4800 bit/s, the rate and interleaver read from the preamble.
//!
//! Standard: MIL-STD-188-110C with Change 1, section 5.3.2.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [ms110: serial-tone front, preamble search, DFE, deinterleaver, Viterbi] -> text, D_PDUs, events
//! ```

pub mod rx;
pub mod signal;

use crate::receiver::GEN_SECTION;
use crate::serial::gen::SymbolGen;
use crate::serial::{psk8, TEST_TEXT};
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Schema, Setting,
    SettingSet, StackConfig, StackDescriptor, Tuning,
};
use signal::Rate;
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The data rate of the test signal.
    pub enum GenBps {
        /// 75 bit/s.
        B75 = ("75", "75 bit/s"),
        /// 150 bit/s.
        B150 = ("150", "150 bit/s"),
        /// 300 bit/s.
        B300 = ("300", "300 bit/s"),
        /// 600 bit/s.
        B600 = ("600", "600 bit/s"),
        /// 1200 bit/s.
        B1200 = ("1200", "1200 bit/s"),
        /// 2400 bit/s.
        B2400 = ("2400", "2400 bit/s"),
        /// 4800 bit/s.
        B4800 = ("4800", "4800 bit/s"),
    }
}

impl GenBps {
    /// The rate.
    pub fn rate(self) -> Rate {
        match self {
            GenBps::B75 => Rate::B75,
            GenBps::B150 => Rate::B150,
            GenBps::B300 => Rate::B300,
            GenBps::B600 => Rate::B600,
            GenBps::B1200 => Rate::B1200,
            GenBps::B2400 => Rate::B2400,
            GenBps::B4800 => Rate::B4800,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The interleaver of the test signal.
    pub enum GenInterleaver {
        /// 0.6 s.
        Short = ("short", "Short (0.6 s)"),
        /// 4.8 s.
        Long = ("long", "Long (4.8 s)"),
    }
}

/// The data rate of the test signal.
pub const GEN_BPS: Setting<field::Choice<GenBps>> =
    Setting::choice("gen_bps", "Data rate", GenBps::B600)
        .section(GEN_SECTION)
        .advanced();
/// The interleaver of the test signal (4800 bit/s has none).
pub const GEN_INTERLEAVER: Setting<field::Choice<GenInterleaver>> =
    Setting::choice("gen_interleaver", "Interleaver", GenInterleaver::Short)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the MIL-STD-188-110 stack: its test signal.
    pub enum Ms110Setting {
        /// Data rate of the test signal.
        GenBps = GEN_BPS,
        /// Interleaver of the test signal.
        GenInterleaver = GEN_INTERLEAVER,
    }
}

/// The silence between two transmissions of the test signal, symbols (0.5 s).
pub const GEN_GAP: usize = 1200;

/// The tribits of one test transmission of `text` at `rate`.
pub fn test_tribits(rate: Rate, long: bool, text: &[u8]) -> Vec<u8> {
    let long = long && rate != Rate::B4800;
    signal::transmission(rate, long, &signal::message_bits(rate, long, text))
}

/// The test signal at `cfg.rate_hz`: the test text, its end-of-message pattern and flush, then 0.5 s of silence, repeated.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let rate = cfg.settings.get(&GEN_BPS).rate();
    let long = cfg.settings.get(&GEN_INTERLEAVER) == GenInterleaver::Long;
    let mut sym: Vec<_> = test_tribits(rate, long, TEST_TEXT.as_bytes())
        .into_iter()
        .map(psk8)
        .collect();
    sym.extend(std::iter::repeat_n(
        sdrkit_core::Complex32::default(),
        GEN_GAP,
    ));
    Ok(Box::new(SymbolGen::serial(cfg.rate_hz, sym)))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    Ms110Setting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("ms110"),
        "MIL-STD-188-110",
        rx::Ms110Decoder::new(),
        cfg,
    )
}

/// The dial of the test signal. It is not the frequency of a station: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 6.5e6;

/// The tuning of the serial-tone and naval HF stacks of this crate, with the test-signal dial `default_hz`: any frequency
/// (the data files give the usual dials and ranges).
pub const fn hf_tuning(default_hz: f64) -> Tuning {
    Tuning {
        tune_offset_hz: crate::serial::TUNE_OFFSET_HZ,
        radio_filter_hz: Some(1.75e6),
        ..Tuning::anywhere(default_hz, 2e6, 250e3, 4e3)
    }
}

/// MIL-STD-188-110 serial tone.
pub static MS110: StackDescriptor = StackDescriptor {
    id: "ms110",
    name: "MIL-STD-188-110 serial tone",
    group: "HF utility",
    blurb: "2400 Bd 8-PSK on 1800 Hz, 75 to 4800 bit/s: the rate and interleaver from the preamble, an equaliser, the K = 7 code; text and STANAG 5066 D_PDUs.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        tune_offset_hz: crate::serial::TUNE_OFFSET_HZ,
        radio_filter_hz: Some(1.75e6),
        ..Tuning::anywhere(TEST_SIGNAL_HZ, 2e6, 250e3, 4e3)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::MS110),
    benchmarks: Some(crate::benchmarks),
};
