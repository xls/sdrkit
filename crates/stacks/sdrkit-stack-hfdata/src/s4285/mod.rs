//! STANAG 4285: 2400 Bd frames of 256 symbols (80 sync, 176 scrambled) on 1800 Hz, 75 to 3600 bit/s. The coded rates are found
//! by their start-of-message pattern; an uncoded rate is a setting.
//!
//! Standard: NATO STANAG 4285 (not public).
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [s4285: serial-tone front, frame sync, DFE, a deinterleaver and Viterbi decoder a profile] -> text, D_PDUs, events
//! ```

pub mod rx;
pub mod signal;

use crate::receiver::GEN_SECTION;
use crate::serial::gen::SymbolGen;
use crate::serial::{bits_lsb, psk8, TEST_TEXT};
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Schema, Setting,
    SettingSet, StackConfig, StackDescriptor,
};
use signal::{Profile, ScramblerStart, PROFILES};
use std::sync::Arc;

sdrkit_core::choice_enum! {
    /// The profile the receiver reads.
    pub enum ProfileChoice {
        /// Every coded profile; the one whose start-of-message pattern is found.
        Auto = ("auto", "Automatic (coded, by the start of message)"),
        /// Profile 0.
        P0 = ("2400_short", "2400 bit/s coded, short"),
        /// Profile 1.
        P1 = ("1200_short", "1200 bit/s coded, short"),
        /// Profile 2.
        P2 = ("600_short", "600 bit/s coded, short"),
        /// Profile 3.
        P3 = ("300_short", "300 bit/s coded, short"),
        /// Profile 4.
        P4 = ("150_short", "150 bit/s coded, short"),
        /// Profile 5.
        P5 = ("75_short", "75 bit/s coded, short"),
        /// Profile 6.
        P6 = ("3600_uncoded", "3600 bit/s uncoded"),
        /// Profile 7.
        P7 = ("2400_uncoded", "2400 bit/s uncoded"),
        /// Profile 8.
        P8 = ("1200_uncoded", "1200 bit/s uncoded"),
        /// Profile 10.
        P10 = ("2400_long", "2400 bit/s coded, long"),
        /// Profile 11.
        P11 = ("1200_long", "1200 bit/s coded, long"),
        /// Profile 12.
        P12 = ("600_long", "600 bit/s coded, long"),
        /// Profile 13.
        P13 = ("300_long", "300 bit/s coded, long"),
        /// Profile 14.
        P14 = ("150_long", "150 bit/s coded, long"),
        /// Profile 15.
        P15 = ("75_long", "75 bit/s coded, long"),
    }
}

impl ProfileChoice {
    /// The profile; `None` for automatic.
    pub fn profile(self) -> Option<Profile> {
        let i = usize::from(sdrkit_core::Choice::index(self));
        (i > 0).then(|| PROFILES[i - 1])
    }
}

sdrkit_core::choice_enum! {
    /// The start state of the scrambler at each frame.
    pub enum StartChoice {
        /// All nine stages 1.
        AllOnes = ("all_ones", "All ones"),
        /// The value 1.
        One = ("one", "The value 1"),
    }
}

/// The profile the receiver reads.
pub const PROFILE: Setting<field::Choice<ProfileChoice>> =
    Setting::choice("profile", "Rate and interleaver", ProfileChoice::Auto).help(
        "Automatic tries every coded profile and reads the one whose start-of-message pattern is found. An uncoded \
profile has no such pattern, so it must be set.",
    );
/// The scrambler's start state (no public document settles it).
pub const SCRAMBLER_START: Setting<field::Choice<StartChoice>> = Setting::choice(
    "scrambler_start",
    "Scrambler start",
    StartChoice::AllOnes,
)
.help(
    "The state of the 9-stage scrambler at the start of each frame. No public document settles it.",
)
.advanced();
/// The profile of the test signal.
pub const GEN_PROFILE: Setting<field::Choice<ProfileChoice>> =
    Setting::choice("gen_profile", "Profile", ProfileChoice::P2)
        .help("Automatic sends 600 bit/s coded, short.")
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the STANAG 4285 receiver.
    pub enum S4285RxSetting {
        /// Rate and interleaver.
        Profile = PROFILE,
        /// Scrambler start.
        ScramblerStart = SCRAMBLER_START,
    }
}

sdrkit_core::settings! {
    /// The settings of the STANAG 4285 stack.
    pub enum S4285Setting {
        /// Rate and interleaver.
        Profile = PROFILE,
        /// Scrambler start.
        ScramblerStart = SCRAMBLER_START,
        /// Profile of the test signal.
        GenProfile = GEN_PROFILE,
    }
}

/// The silence between two transmissions of the test signal, symbols (1.5 s: longer than the frames a receiver waits before
/// it lets a transmission go).
pub const GEN_GAP: usize = 3600;

/// The tribits of one test transmission of `bytes` with profile `p`.
pub fn test_tribits(p: &Profile, bytes: &[u8], start: ScramblerStart) -> Vec<u8> {
    let bits = if p.coded {
        signal::coded_bits(p, &signal::message_bits(p, bytes))
    } else {
        bits_lsb(bytes)
    };
    signal::frames(p, &bits, start)
}

/// The profile of the test signal when its setting is automatic, and its default: 600 bit/s coded, short.
pub const GEN_DEFAULT: Profile = PROFILES[2];

/// The length in seconds of one cycle of the test signal with profile `p`: the transmission of the test text and the
/// silence after it.
pub fn test_cycle_s(p: &Profile) -> f64 {
    let symbols = test_tribits(p, TEST_TEXT.as_bytes(), ScramblerStart::AllOnes).len() + GEN_GAP;
    symbols as f64 / crate::serial::BAUD
}

/// The test signal at `cfg.rate_hz`: one transmission of the test text, then 1.5 s of silence, repeated.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = cfg
        .settings
        .get(&GEN_PROFILE)
        .profile()
        .unwrap_or(GEN_DEFAULT);
    let start = match cfg.settings.get(&SCRAMBLER_START) {
        StartChoice::AllOnes => ScramblerStart::AllOnes,
        StartChoice::One => ScramblerStart::One,
    };
    let mut sym: Vec<_> = test_tribits(&p, TEST_TEXT.as_bytes(), start)
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
    S4285Setting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("s4285"),
        "STANAG 4285",
        rx::S4285Decoder::new(),
        cfg,
    )
}

/// STANAG 4285.
pub static S4285: StackDescriptor = StackDescriptor {
    id: "s4285",
    name: "STANAG 4285",
    group: "HF utility",
    blurb: "2400 Bd frames of 256 symbols on 1800 Hz, 75 to 3600 bit/s: coded rates found by their start of message; text and STANAG 5066 D_PDUs.",
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
    verify: Some(&crate::verify::S4285),
    benchmarks: Some(crate::benchmarks),
};
