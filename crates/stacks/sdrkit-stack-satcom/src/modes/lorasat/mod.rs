//! LoRa satellites (`lora_sat`): LoRa frames from and to satellites with their Doppler shift and rate, and the Doppler curve of
//! each pass. The LoRa physical layer is `sdrkit-proto-lora-phy`. This stack adds the satellite presets and the carrier tracking.

pub mod display;
pub mod gen;
pub mod rx;

use display::LoraSatDisplay;
use rx::LoraSatRx;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, DeclareSetting, GenConfig, Generator, LockProfile, Receiver,
    ResultsDecl, Schema, StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, DecoderComponent, GraphBuilder, GraphError, Registry, StackGraph};
use sdrkit_util_geo::orbit::kepler::MU_EARTH_M3_S2;
use sdrkit_util_geo::{EARTH_MEAN_RADIUS_M, SPEED_OF_LIGHT_M_S};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "lora_sat";
/// The stack name.
pub const NAME: &str = "LoRa satellites";
/// The lowest sample rate, Hz.
pub const MIN_RATE: f64 = 1e6;
/// Seconds between reports.
pub const REPORT_S: f64 = 0.25;
/// The speed of light, m/s.
const C: f64 = SPEED_OF_LIGHT_M_S;

sdrkit_core::choice_enum! {
    /// A LoRa bandwidth.
    pub enum LoraBw {
        /// 41.7 kHz (125/3 kHz).
        B41k7 = ("41.7", "41.7 kHz"),
        /// 62.5 kHz.
        B62k5 = ("62.5", "62.5 kHz"),
        /// 125 kHz.
        B125k = ("125", "125 kHz"),
        /// 250 kHz.
        B250k = ("250", "250 kHz"),
        /// 500 kHz.
        B500k = ("500", "500 kHz"),
    }
}

impl LoraBw {
    /// The bandwidth, Hz.
    pub fn hz(self) -> f64 {
        match self {
            LoraBw::B41k7 => 125e3 / 3.0,
            LoraBw::B62k5 => 62.5e3,
            LoraBw::B125k => 125e3,
            LoraBw::B250k => 250e3,
            LoraBw::B500k => 500e3,
        }
    }
}

sdrkit_core::choice_enum! {
    /// A LoRa spreading factor.
    pub enum Sf {
        /// SF7.
        Sf7 = ("7", "7"),
        /// SF8.
        Sf8 = ("8", "8"),
        /// SF9.
        Sf9 = ("9", "9"),
        /// SF10.
        Sf10 = ("10", "10"),
        /// SF11.
        Sf11 = ("11", "11"),
        /// SF12.
        Sf12 = ("12", "12"),
    }
}

impl Sf {
    /// The spreading factor.
    pub fn value(self) -> u8 {
        self as u8 + 7
    }
}

/// A preset: a public description of a satellite LoRa link, each with a confidence.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// What it is.
    pub name: &'static str,
    /// The carrier, MHz.
    pub mhz: f64,
    /// The spreading factor, or 0 when it is searched.
    pub sf: u8,
    /// The bandwidth, kHz.
    pub bw_khz: f64,
    /// The coding rate denominator, or 0 when it comes from the header.
    pub cr: u8,
    /// The sync words, or none when 0x12 and 0x34 are both tried.
    pub syncs: &'static [u8],
    /// The preamble symbols, when the link uses a length other than the setting.
    pub preamble: Option<u16>,
    /// The frames come from ground devices, not from the satellite.
    pub uplink_only: bool,
    /// How sure the description is.
    pub confidence: &'static str,
    /// A note for people.
    pub note: &'static str,
}

/// The presets.
pub const PRESETS: [Preset; 10] = [
    Preset {
        name: "FOSSA Systems pico-satellites",
        mhz: 436.8,
        sf: 11,
        bw_khz: 125.0,
        cr: 8,
        syncs: &[0x12],
        preamble: None,
        uplink_only: false,
        confidence: "uncertain",
        note: "public description",
    },
    Preset {
        name: "FOSSASAT-2 family (second frequency)",
        mhz: 436.2,
        sf: 11,
        bw_khz: 125.0,
        cr: 8,
        syncs: &[0x12],
        preamble: None,
        uplink_only: false,
        confidence: "uncertain",
        note: "public description",
    },
    Preset {
        name: "Tianqi constellation downlink",
        mhz: 400.45,
        sf: 0,
        bw_khz: 500.0,
        cr: 0,
        syncs: &[],
        preamble: None,
        uplink_only: false,
        confidence: "uncertain",
        note: "SF and sync searched",
    },
    Preset {
        name: "Lacuna Space uplink, EU868 868.1",
        mhz: 868.1,
        sf: 0,
        bw_khz: 125.0,
        cr: 5,
        syncs: &[0x34],
        preamble: None,
        uplink_only: true,
        confidence: "plausible",
        note: "you hear the ground devices",
    },
    Preset {
        name: "Lacuna Space uplink, EU868 868.3",
        mhz: 868.3,
        sf: 0,
        bw_khz: 125.0,
        cr: 5,
        syncs: &[0x34],
        preamble: None,
        uplink_only: true,
        confidence: "plausible",
        note: "you hear the ground devices",
    },
    Preset {
        name: "Lacuna Space uplink, EU868 868.5",
        mhz: 868.5,
        sf: 0,
        bw_khz: 125.0,
        cr: 5,
        syncs: &[0x34],
        preamble: None,
        uplink_only: true,
        confidence: "plausible",
        note: "you hear the ground devices",
    },
    Preset {
        name: "Space LoRaWAN uplinks, US915 sub-band 2",
        mhz: 904.6,
        sf: 0,
        bw_khz: 125.0,
        cr: 5,
        syncs: &[0x34],
        preamble: None,
        uplink_only: true,
        confidence: "uncertain",
        note: "you hear the ground devices",
    },
    Preset {
        name: "FOSSASAT on 401.703 MHz",
        mhz: 401.703,
        sf: 11,
        bw_khz: 125.0,
        cr: 8,
        syncs: &[],
        preamble: None,
        uplink_only: false,
        confidence: "plausible",
        note: "sync searched",
    },
    Preset {
        name: "137.055 MHz LoRa telemetry",
        mhz: 137.055,
        sf: 8,
        bw_khz: 125.0 / 3.0,
        cr: 5,
        syncs: &[0x12],
        preamble: Some(15),
        uplink_only: false,
        confidence: "plausible",
        note: "15-symbol preamble, CRC on",
    },
    Preset {
        name: "Generic 70 cm LoRa cubesat",
        mhz: 437.4,
        sf: 10,
        bw_khz: 125.0,
        cr: 0,
        syncs: &[0x12],
        preamble: None,
        uplink_only: false,
        confidence: "uncertain",
        note: "CR from the header",
    },
];

/// The preset within 15 kHz of a frequency.
pub fn nearest(hz: f64) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| (p.mhz * 1e6 - hz).abs() <= 15e3)
}

/// The largest Doppler shift (at the horizon) and rate (overhead, an upper bound) for a 600 km circular orbit at `f` Hz.
pub fn doppler_limits(f: f64) -> (f64, f64) {
    let h = 600e3;
    let re = EARTH_MEAN_RADIUS_M;
    let r = re + h;
    let v = (MU_EARTH_M3_S2 / r).sqrt();
    let shift = sdrkit_util_geo::orbit::max_doppler_hz(f, h);
    let rate = f / C * v * v / h * re / r;
    (shift, rate)
}

/// The straight-line pass model: shift(t) = f v / c (tca - t) / sqrt((t - tca)^2 + (d / v)^2).
pub fn pass_shift(f: f64, t: f64, tca: f64) -> f64 {
    let v = 7560.0;
    let d = 600e3;
    f * v / C * (tca - t) / ((t - tca).powi(2) + (d / v).powi(2)).sqrt()
}

/// The rate of [`pass_shift`], Hz/s.
pub fn pass_rate(f: f64, t: f64, tca: f64) -> f64 {
    let v = 7560.0;
    let d = 600e3;
    let a = d / v;
    let x = t - tca;
    -f * v / C * a * a / (x * x + a * a).powf(1.5)
}

/// The settings of the receiver, the display and the test signal, in that order.
/// Every setting of the stack, in the order of the schema.
pub(crate) const SETTINGS: &[&dyn DeclareSetting] = &[
    &rx::FOLLOW_DIAL,
    &rx::AUTO_SF,
    &rx::SF,
    &rx::AUTO_BW,
    &rx::BW_KHZ,
    &rx::SYNC,
    &rx::SYNC_OTHER,
    &rx::PREAMBLE,
    &rx::DOPPLER_MAX_HZ,
    &rx::TRACK,
    &rx::OFFSET_HZ,
    &display::PASS_SATELLITE,
    &gen::GEN_BAND,
    &gen::GEN_SF,
    &gen::GEN_BW_KHZ,
    &gen::GEN_PASS_DOPPLER,
    &gen::GEN_START_S,
    &gen::GEN_SYNC,
];

/// Every setting of the stack. Noise and the radio's errors of the test signal come from the standard `gen_*` settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(Schema::of(SETTINGS)))
        .clone()
}

/// The receiver: the LoRa receiver, then the display, in one graph.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut b = GraphBuilder::new()
        .add("receiver", DecoderComponent::new(LoraSatRx::default()))
        .add("display", DecoderComponent::new(LoraSatDisplay::default()))
        .input("receiver.in")
        .connect("receiver.out", "display.in")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S);
    for s in sdrkit_graph::Decoder::settings(&LoraSatRx::default()).settings() {
        b = b.alias(&s.field.key, &format!("receiver.{}", s.field.key));
    }
    let g = b
        .alias("pass_satellite", "display.pass_satellite")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            MIN_RATE,
            "LoRa satellites needs a sample rate of at least 1000 ksps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz + DESCRIPTOR.tuning.tune_offset_hz)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::LoraSatGenerator::from_config(cfg)))
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// Adds the components of the stack.
pub fn register(r: &mut Registry) {
    r.add("satcom.lora_sat_receiver", || {
        Box::new(DecoderComponent::new(LoraSatRx::default()))
    });
    r.add("satcom.lora_sat_display", || {
        Box::new(DecoderComponent::new(LoraSatDisplay::default()))
    });
}

/// The verification of the stack: the test signal's beacons decode with a good CRC.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, a pass at 436.8 MHz", o, 0.0);
    c.seconds = c.seconds.max(14.0);
    c.expect_audio = Some(false);
    c.check = Some(check_frames);
    vec![c]
}

fn check_frames(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let good = crate::field(r, "frames_ok");
    vec![CheckLine::new(
        "frames_ok",
        good >= 1.0,
        format!("{good} frames with a good CRC"),
    )]
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "LoRa frames from and to satellites, with the Doppler shift and Doppler rate of each pass. Receive only.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 137e6,
        max_hz: 1000e6,
        default_hz: 436.8e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 0.25e6,
        tune_offset_hz: 300e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Lora],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l1_presets_and_l2_l3_limits() {
        assert!(PRESETS.len() >= 5);
        assert!(PRESETS
            .iter()
            .all(|p| p.confidence != "documented" && p.bw_khz > 0.0 && !p.name.is_empty()));
        assert_eq!(nearest(436.8e6).unwrap().name, PRESETS[0].name);
        assert_eq!(nearest(436.81e6).unwrap().name, PRESETS[0].name);
        assert!(nearest(150e6).is_none());
        let (s, r) = doppler_limits(436e6);
        assert!(
            (9.5e3..11e3).contains(&s) && (100.0..160.0).contains(&r),
            "{s} {r}"
        );
        let (s, r) = doppler_limits(868e6);
        assert!(
            (19e3..22e3).contains(&s) && (200.0..320.0).contains(&r),
            "{s} {r}"
        );
        let f = 436.8e6;
        assert!(pass_shift(f, 300.0, 300.0).abs() < 1e-6);
        let want = -f * 7560.0f64.powi(2) / (C * 600e3);
        assert!((pass_rate(f, 300.0, 300.0) - want).abs() < 1e-6 * want.abs());
        assert!(pass_shift(f, 0.0, 300.0) > 9e3 && pass_shift(f, 600.0, 300.0) < -9e3);
        assert!((pass_shift(f, 0.0, 300.0) + pass_shift(f, 600.0, 300.0)).abs() < 1e-6);
        assert!((LoraBw::B41k7.hz() - 125e3 / 3.0).abs() < 1.0);
        assert_eq!(LoraBw::B62k5.hz(), 62.5e3);
        assert_eq!(Sf::Sf12.value(), 12);
    }
}
