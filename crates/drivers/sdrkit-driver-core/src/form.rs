//! The standard form of the settings of a radio: the sections in their order, the settings that radios share with a fixed
//! key, label, unit, kind and section, the usual rates and filter widths, and a check of a schema against the form.
//!
//! The settings of every radio ([`RadioSetting`](crate::RadioSetting)) come first in a schema, in the order of their
//! sections. A driver declares the other standard settings below only when its radio has the part, with the range and
//! the step of its radio, and its own settings after them in the order of [`SECTIONS`].

use sdrkit_core::publish::field;
use sdrkit_core::settings::{Schema, SettingDecl, SettingKind, SettingSet};
use sdrkit_core::value::{Choice, ValueKind};
use sdrkit_core::{Hz, Setting};

/// The section of the gain settings.
pub const GAIN_SECTION: &str = "Gain";
/// The section of the rate and the analogue filter.
pub const RATE_SECTION: &str = "Rate and bandwidth";
/// The section of the frequency correction and the I/Q corrections.
pub const CORRECTIONS_SECTION: &str = "Corrections";
/// The section of the inputs, the outputs and the power on them.
pub const ANTENNA_SECTION: &str = "Antenna and power";
/// The section of the connection to the radio: protocol, buffers, format on the link.
pub const LINK_SECTION: &str = "Link";
/// The section of the settings of one driver.
pub const OTHER_SECTION: &str = "Other";

/// The sections in their order. A driver may add sections of its own after them.
pub const SECTIONS: [&str; 6] = [
    GAIN_SECTION,
    RATE_SECTION,
    CORRECTIONS_SECTION,
    ANTENNA_SECTION,
    LINK_SECTION,
    OTHER_SECTION,
];

/// The rates offered on a radio with a continuous range of rates, inside its range and what its link carries.
pub const USUAL_RATES_HZ: [Hz; 16] = [
    0.25e6, 0.5e6, 1e6, 1.024e6, 2e6, 2.048e6, 2.4e6, 2.56e6, 3.2e6, 4e6, 5e6, 8e6, 10e6, 12.5e6,
    16e6, 20e6,
];

/// The widths offered on a radio with a continuous range of filter widths, inside its range.
pub const USUAL_WIDTHS_HZ: [Hz; 14] = [
    0.2e6, 0.3e6, 0.6e6, 1e6, 1.536e6, 2e6, 2.5e6, 5e6, 6e6, 7e6, 8e6, 10e6, 16e6, 20e6,
];

/// A stage gain of -1 follows the gain: the driver shares [`GAIN`](crate::GAIN) between the stages.
pub const STAGE_FOLLOWS: i64 = -1;

/// The form of one standard setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Standard {
    /// The key.
    pub key: &'static str,
    /// The label.
    pub label: &'static str,
    /// The unit.
    pub unit: &'static str,
    /// The kind of the values.
    pub kind: ValueKind,
    /// The section.
    pub section: &'static str,
    /// A setting that can be changed lists the values that people use ([`SettingDecl::common`]).
    pub common: bool,
}

const fn std(
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    kind: ValueKind,
    section: &'static str,
) -> Standard {
    Standard {
        key,
        label,
        unit,
        kind,
        section,
        common: false,
    }
}

const fn listed(s: Standard) -> Standard {
    Standard { common: true, ..s }
}

/// The centre frequency. It has no section: a front end shows it as the tuning of the radio.
pub const CENTRE_FORM: Standard = std("centre_hz", "Centre frequency", "Hz", ValueKind::Hz, "");
/// The overall gain in dB.
pub const GAIN_FORM: Standard = std("gain", "Gain", "dB", ValueKind::F64, GAIN_SECTION);
/// Automatic gain, on or off.
pub const AGC_FORM: Standard = std("agc", "Automatic gain", "", ValueKind::Bool, GAIN_SECTION);
/// The sample rate.
pub const RATE_FORM: Standard = listed(std("rate", "Sample rate", "Hz", ValueKind::Hz, RATE_SECTION));
/// The width of the analogue filter.
pub const BANDWIDTH_FORM: Standard =
    listed(std("bandwidth", "Bandwidth", "Hz", ValueKind::Hz, RATE_SECTION));
/// The rate sent as it is.
pub const RATE_EXACT_FORM: Standard =
    std("rate_exact", "Exact rate", "", ValueKind::Bool, RATE_SECTION);
/// The bias tee.
pub const BIAS_TEE_FORM: Standard =
    std("bias_tee", "Bias tee", "", ValueKind::Bool, ANTENNA_SECTION);
/// The kind of automatic gain, for a radio with more than one.
pub const AGC_MODE_FORM: Standard = std("agc_mode", "AGC mode", "", ValueKind::Choice, GAIN_SECTION);
/// The gain of the LNA stage.
pub const LNA_GAIN_FORM: Standard = std("lna_gain", "LNA gain", "dB", ValueKind::I64, GAIN_SECTION);
/// The gain of the mixer stage.
pub const MIXER_GAIN_FORM: Standard =
    std("mixer_gain", "Mixer gain", "dB", ValueKind::I64, GAIN_SECTION);
/// The gain of the VGA (baseband) stage.
pub const VGA_GAIN_FORM: Standard = std("vga_gain", "VGA gain", "dB", ValueKind::I64, GAIN_SECTION);
/// The state of an LNA set in states, not in dB.
pub const LNA_STATE_FORM: Standard = std("lna_state", "LNA state", "", ValueKind::I64, GAIN_SECTION);
/// The gain reduction of the IF stage.
pub const IF_REDUCTION_FORM: Standard =
    std("if_reduction", "IF gain reduction", "dB", ValueKind::I64, GAIN_SECTION);
/// The RF amplifier, on or off.
pub const RF_AMP_FORM: Standard = std("amp", "RF amplifier", "", ValueKind::Bool, GAIN_SECTION);
/// The frequency correction.
pub const PPM_FORM: Standard =
    std("ppm", "Frequency correction", "ppm", ValueKind::F64, CORRECTIONS_SECTION);
/// I and Q swapped.
pub const SWAP_IQ_FORM: Standard =
    std("swap_iq", "Swap I and Q", "", ValueKind::Bool, CORRECTIONS_SECTION);
/// The antenna input or port.
pub const ANTENNA_FORM: Standard =
    std("antenna", "Antenna", "", ValueKind::Choice, ANTENNA_SECTION);
/// Direct sampling of HF.
pub const DIRECT_SAMPLING_FORM: Standard =
    std("ds", "Direct sampling", "", ValueKind::Choice, ANTENNA_SECTION);
/// The source of the reference clock.
pub const CLOCK_SOURCE_FORM: Standard =
    std("clocksrc", "Clock source", "", ValueKind::Choice, ANTENNA_SECTION);
/// The clock on the output socket.
pub const CLOCK_OUT_FORM: Standard =
    std("clock_out", "Clock output", "", ValueKind::Bool, ANTENNA_SECTION);
/// The protocol on the link.
pub const PROTOCOL_FORM: Standard = std("protocol", "Protocol", "", ValueKind::Choice, LINK_SECTION);
/// The sample format on the link.
pub const FORMAT_FORM: Standard = std("format", "Sample format", "", ValueKind::Choice, LINK_SECTION);
/// The sample buffers in flight.
pub const BUFFERS_FORM: Standard = std("buffers", "Buffers", "", ValueKind::I64, LINK_SECTION);
/// The samples per transfer.
pub const BUFFER_SIZE_FORM: Standard =
    std("buffer_size", "Buffer size", "", ValueKind::Choice, LINK_SECTION);
/// The rate that a server keeps whatever it is asked.
pub const SERVER_RATE_FORM: Standard =
    std("server_rate", "Rate of the server", "Hz", ValueKind::Hz, LINK_SECTION);

/// Every standard setting.
pub const STANDARD: [Standard; 25] = [
    CENTRE_FORM,
    GAIN_FORM,
    AGC_FORM,
    RATE_FORM,
    BANDWIDTH_FORM,
    RATE_EXACT_FORM,
    BIAS_TEE_FORM,
    AGC_MODE_FORM,
    LNA_GAIN_FORM,
    MIXER_GAIN_FORM,
    VGA_GAIN_FORM,
    LNA_STATE_FORM,
    IF_REDUCTION_FORM,
    RF_AMP_FORM,
    PPM_FORM,
    SWAP_IQ_FORM,
    ANTENNA_FORM,
    DIRECT_SAMPLING_FORM,
    CLOCK_SOURCE_FORM,
    CLOCK_OUT_FORM,
    PROTOCOL_FORM,
    FORMAT_FORM,
    BUFFERS_FORM,
    BUFFER_SIZE_FORM,
    SERVER_RATE_FORM,
];

const fn number<K: SettingKind>(
    f: Standard,
    default: K::Default,
) -> Setting<K> {
    Setting::new(f.key, f.label, f.unit, default).section(f.section)
}

const fn choice<E: Choice + Send + Sync>(f: Standard, default: E) -> Setting<field::Choice<E>> {
    Setting::choice(f.key, f.label, default).section(f.section)
}

/// The frequency correction in ppm, positive when the clock of the radio runs fast. A driver gives the range and the
/// step of its radio.
pub const PPM: Setting<field::F64> = number::<field::F64>(PPM_FORM, 0.0)
    .range(-200.0, 200.0, 0.1)
    .help("The clock error in ppm, positive if the clock runs fast (signals show up a little low).");
/// I and Q swapped, for a radio that delivers them the other way round.
pub const SWAP_IQ: Setting<field::Bool> = number::<field::Bool>(SWAP_IQ_FORM, false)
    .help("Swaps I and Q: a spectrum that shows mirrored shows the right way round.");
/// The clock of the radio on its output socket.
pub const CLOCK_OUT: Setting<field::Bool> = number::<field::Bool>(CLOCK_OUT_FORM, false);
/// The rate that a server keeps whatever it is asked. The driver finds and sets it; 0: none found.
pub const SERVER_RATE: Setting<field::Hz> = number::<field::Hz>(SERVER_RATE_FORM, 0.0)
    .range(0.0, 40e6, 1.0)
    .advanced()
    .help("The rate the server sends whatever it is asked. The driver finds it. 0 means none was found.");

/// The gain of the LNA stage in dB, from -1 ([`STAGE_FOLLOWS`]) to `max` in steps of 1. `default` is the value before
/// one is set.
pub const fn lna_gain(default: i64, max: f64) -> Setting<field::I64> {
    number::<field::I64>(LNA_GAIN_FORM, default).range(-1.0, max, 1.0)
}
/// The gain of the mixer stage in dB, from -1 ([`STAGE_FOLLOWS`]) to `max`.
pub const fn mixer_gain(default: i64, max: f64) -> Setting<field::I64> {
    number::<field::I64>(MIXER_GAIN_FORM, default).range(-1.0, max, 1.0)
}
/// The gain of the VGA stage in dB, from -1 ([`STAGE_FOLLOWS`]) to `max`.
pub const fn vga_gain(default: i64, max: f64) -> Setting<field::I64> {
    number::<field::I64>(VGA_GAIN_FORM, default).range(-1.0, max, 1.0)
}
/// The state of an LNA set in states, from -1 ([`STAGE_FOLLOWS`]) to `max`.
pub const fn lna_state(default: i64, max: f64) -> Setting<field::I64> {
    number::<field::I64>(LNA_STATE_FORM, default).range(-1.0, max, 1.0)
}
/// The gain reduction of the IF stage in dB, from -1 ([`STAGE_FOLLOWS`]) to `max`.
pub const fn if_reduction(default: i64, max: f64) -> Setting<field::I64> {
    number::<field::I64>(IF_REDUCTION_FORM, default).range(-1.0, max, 1.0)
}
/// The RF amplifier, on (`default` true) or off before one is set.
pub const fn rf_amp(default: bool) -> Setting<field::Bool> {
    number::<field::Bool>(RF_AMP_FORM, default)
}
/// The kind of automatic gain, from the modes `E` of the radio.
pub const fn agc_mode<E: Choice + Send + Sync>(default: E) -> Setting<field::Choice<E>> {
    choice(AGC_MODE_FORM, default)
}
/// The antenna input, from the inputs `E` of the radio.
pub const fn antenna<E: Choice + Send + Sync>(default: E) -> Setting<field::Choice<E>> {
    choice(ANTENNA_FORM, default)
}
/// Direct sampling of HF, from the inputs `E` of the radio.
pub const fn direct_sampling<E: Choice + Send + Sync>(default: E) -> Setting<field::Choice<E>> {
    choice(DIRECT_SAMPLING_FORM, default)
}
/// The source of the reference clock, from the sources `E` of the radio.
pub const fn clock_source<E: Choice + Send + Sync>(default: E) -> Setting<field::Choice<E>> {
    choice(CLOCK_SOURCE_FORM, default)
}
/// The protocol on the link, from the protocols `E` of the driver.
pub const fn protocol<E: Choice + Send + Sync>(default: E) -> Setting<field::Choice<E>> {
    choice(PROTOCOL_FORM, default)
}
/// The sample format on the link, from the formats `E` of the driver.
pub const fn format<E: Choice + Send + Sync>(default: E) -> Setting<field::Choice<E>> {
    choice(FORMAT_FORM, default)
}
/// The samples per transfer, from the sizes `E` of the driver.
pub const fn buffer_size<E: Choice + Send + Sync>(default: E) -> Setting<field::Choice<E>> {
    choice(BUFFER_SIZE_FORM, default)
}
/// The sample buffers in flight, from `min` to `max`.
pub const fn buffers(default: i64, min: f64, max: f64) -> Setting<field::I64> {
    number::<field::I64>(BUFFERS_FORM, default).range(min, max, 1.0)
}

/// The rank of `section` in [`SECTIONS`]; a section of the driver ranks after them.
fn rank(section: &str) -> usize {
    SECTIONS
        .iter()
        .position(|s| *s == section)
        .unwrap_or(SECTIONS.len())
}

/// What in `schema`, the schema of a radio, does not follow the form, one line each:
///
/// - a standard setting with another label, unit, kind or section, a setting with the label of a standard setting
///   under another key, and a standard setting that can be changed without its common values where the form needs
///   them;
/// - a number setting that can be changed without a range and a step above 0;
/// - a setting without a section (the centre frequency aside), and sections out of the order of [`SECTIONS`], within
///   the settings of every radio and within the settings that follow them.
///
/// The check ends at the first transmit setting ([`TxSetting`](crate::TxSetting)): the transmit settings keep their own
/// form.
pub fn check(schema: &Schema) -> Vec<String> {
    let mut out = Vec::new();
    let first = crate::RadioSetting::SETTINGS.len();
    let tx: Vec<&str> = crate::TxSetting::SETTINGS.iter().map(|s| s.key()).collect();
    let mut last = [0usize; 2];
    for (i, d) in schema.settings().iter().enumerate() {
        let key = d.field.key.as_ref();
        // The transmit settings follow the receive settings and keep their own form.
        if tx.contains(&key) {
            break;
        }
        check_one(d, &mut out);
        if key == CENTRE_FORM.key {
            continue;
        }
        let standard = STANDARD.iter().any(|s| s.key == key);
        if d.section.is_empty() && !standard {
            out.push(format!("{key}: no section"));
        }
        if d.section.is_empty() {
            continue;
        }
        let part = usize::from(i >= first);
        let r = rank(&d.section);
        if r < last[part] {
            out.push(format!("{key}: section {} after {}", d.section, section_name(last[part])));
        }
        last[part] = last[part].max(r);
    }
    out
}

fn section_name(rank: usize) -> &'static str {
    SECTIONS.get(rank).copied().unwrap_or("a section of the driver")
}

fn check_one(d: &SettingDecl, out: &mut Vec<String>) {
    let key = d.field.key.as_ref();
    let by_key = STANDARD.iter().find(|s| s.key == key);
    if let Some(s) = STANDARD.iter().find(|s| s.label == d.field.label && s.key != key) {
        out.push(format!("{key}: has the label of {}", s.key));
    }
    if let Some(s) = by_key {
        if d.field.label != s.label {
            out.push(format!("{key}: label {:?}, not {:?}", d.field.label, s.label));
        }
        if d.field.unit != s.unit {
            out.push(format!("{key}: unit {:?}, not {:?}", d.field.unit, s.unit));
        }
        if d.field.kind != s.kind {
            out.push(format!("{key}: kind {:?}, not {:?}", d.field.kind, s.kind));
        }
        if d.section != s.section {
            out.push(format!("{key}: section {:?}, not {:?}", d.section, s.section));
        }
        if s.common && !d.fixed && d.common.is_empty() {
            out.push(format!("{key}: no common values"));
        }
    }
    let number = matches!(d.field.kind, ValueKind::F64 | ValueKind::I64 | ValueKind::Hz);
    if number && !d.fixed && d.range.is_none_or(|r| r.step <= 0.0) {
        out.push(format!("{key}: no range with a step"));
    }
}
