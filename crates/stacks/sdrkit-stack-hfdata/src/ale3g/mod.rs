//! 3G-ALE (third-generation HF link automation): the BW0 link-setup bursts with their LE PDUs, and the traffic bursts BW1 (TM,
//! HDL_ACK, HDL_EOM), BW2 (HDL packets), BW3 (LDL packets) and BW4 (LDL acknowledgements); the datagrams rebuilt.
//!
//! Standard: MIL-STD-188-141B Appendix C.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [ale3g: matched filter, burst detectors, RAKE Walsh despreading, Viterbi decoders, PDUs] -> PDU log, datagrams
//! ```

pub mod pdu;
pub mod rx;
pub mod signal;

use crate::receiver::GEN_SECTION;
use crate::serial::gen::SymbolGen;
use crate::serial::TEST_TEXT;
use pdu::{le_call, le_handshake, le_notification, link_id, tm};
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver, Schema, Setting,
    SettingSet, StackConfig, StackDescriptor,
};
use std::sync::Arc;

/// The feedback taps of the BW2 PN generator: a mask of the register bits whose parity enters bit 15.
pub const BW2_TAPS: Setting<field::I64> = Setting::new(
    "bw2_taps",
    "BW2 PN taps",
    "",
    signal::BW2_TAPS_DRAWN as i64,
)
.range(1.0, 65535.0, 1.0)
.help(
    "The register bits (as a 16-bit mask) whose parity is the new bit of the BW2 PN generator. The default is Figure C-12 as \
drawn (bits 15, 14, 5 and 0); no recording has confirmed it.",
)
.advanced();

sdrkit_core::choice_enum! {
    /// What the test signal sends.
    pub enum GenSet {
        /// A call, its handshake, a notification, traffic setup, an LDL packet, its acknowledgement and the end of traffic.
        Exchange = ("exchange", "A full exchange"),
        /// The three link-setup PDUs only.
        Bw0 = ("bw0", "Link setup (BW0) only"),
    }
}

/// What the test signal sends.
pub const GEN_SET: Setting<field::Choice<GenSet>> =
    Setting::choice("gen_set", "Bursts", GenSet::Exchange)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the 3G-ALE receiver.
    pub enum Ale3gRxSetting {
        /// BW2 PN taps.
        Bw2Taps = BW2_TAPS,
    }
}

sdrkit_core::settings! {
    /// The settings of the 3G-ALE stack.
    pub enum Ale3gSetting {
        /// BW2 PN taps.
        Bw2Taps = BW2_TAPS,
        /// Bursts of the test signal.
        GenSet = GEN_SET,
    }
}

/// The silence between bursts of the test signal, symbols (0.2 s).
pub const GEN_GAP: usize = 480;

/// The addresses of the test signal: 547 (member 17 of group 3) and 163 (member 5 of group 3).
pub const TEST_CALLER: u32 = 547;
/// See [`TEST_CALLER`].
pub const TEST_RESPONDER: u32 = 163;

/// The bursts of the test signal, each followed by 0.2 s of silence.
pub fn test_symbols(set: GenSet) -> Vec<Complex32> {
    let gap = || std::iter::repeat_n(Complex32::default(), GEN_GAP);
    let mut v: Vec<Complex32> = gap().collect();
    let bw0s = [
        le_call(5, 2, 17, 3),
        le_handshake(u32::from(link_id(TEST_CALLER, TEST_RESPONDER)), 2, 12),
        le_notification(0, 5, 3),
    ];
    for b in &bw0s {
        v.extend(signal::bw0(b));
        v.extend(gap());
    }
    if set == GenSet::Bw0 {
        return v;
    }
    v.extend(signal::bw1(&tm(2, TEST_RESPONDER, TEST_CALLER, 0, 7)));
    v.extend(gap());
    v.extend(signal::bw1(&tm(3, TEST_CALLER, TEST_RESPONDER, 1, 7)));
    v.extend(gap());
    let mut data: Vec<u8> = TEST_TEXT.bytes().collect();
    data.resize(64, b' ');
    let pkt = pdu::ldl_packet(&data, 64, pdu::sequence(true, true, 64));
    v.extend(signal::bw3(&pkt, 64, 0).expect("64 bytes have an interleaver"));
    v.extend(gap());
    v.extend(signal::bw4(3));
    v.extend(gap());
    v.extend(signal::bw1(&tm(3, TEST_CALLER, TEST_RESPONDER, 2, 0)));
    v.extend(gap());
    v
}

/// The test signal at `cfg.rate_hz`, repeated.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let set = cfg.settings.get(&GEN_SET);
    Ok(Box::new(SymbolGen::serial(cfg.rate_hz, test_symbols(set))))
}

/// The settings of the stack.
pub fn schema() -> Arc<Schema> {
    Ale3gSetting::schema()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("ale3g"),
        "3G-ALE",
        rx::Ale3gDecoder::new(),
        cfg,
    )
}

/// 3G-ALE.
pub static ALE3G: StackDescriptor = StackDescriptor {
    id: "ale3g",
    name: "3G-ALE",
    group: "HF utility",
    blurb: "Third-generation ALE bursts: link setup (BW0), traffic management, HDL and LDL packets and acknowledgements (BW1 to BW4), with the datagrams rebuilt.",
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
    verify: Some(&crate::verify::ALE3G),
    benchmarks: Some(crate::benchmarks),
};
