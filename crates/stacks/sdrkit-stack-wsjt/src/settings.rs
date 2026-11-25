//! The settings of each stack of the family and of the test signal, and the sub-mode that a set of values selects.

use crate::components::{
    BAND_EDGES, BEACON_DBM, BEACON_EVERY, BEACON_HZ, BEACON_OFFSET, DEMOD, DIALS, EARLY_DECODE,
    ENABLE, HOP_BANDS, HOP_COORDINATED, HOP_CYCLE, HOP_ENABLED, HOP_HOURS, JS8_FAST, JS8_HZ,
    JS8_NORMAL, JS8_SPEED, JS8_TURBO, LISTEN, MULTIBAND, MULTIBAND_SELECT, QSO_AUTO, QSO_EVEN,
    QSO_HOLD, QSO_HZ, QSO_MODE, QSO_RETRIES, QSO_WATCHDOG, SPOTS, TIME_SOURCE, TIME_START, WIDE,
    WIDE_SPAN,
};
use crate::mode::{Mode, Sub};
use crate::runner::{DEEP, MIRRORED};
use sdrkit_core::publish::field;
use sdrkit_core::settings::DeclareSetting;
use sdrkit_core::{ChoiceDecl, FieldDecl, Schema, Setting, SettingDecl, Value, Values};
use std::borrow::Cow;
use std::sync::{Arc, OnceLock};

/// The test signal is upside down.
pub const GEN_MIRRORED: Setting<field::Bool> =
    Setting::new("gen_mirrored", "Test signal upside down", "", false)
        .section("Test signal")
        .advanced();
/// The test signal starts in step with the clock.
pub const GEN_LIVE: Setting<field::Bool> =
    Setting::new("gen_live", "Test signal in step with the clock", "", false)
        .section("Test signal")
        .advanced();
/// The test signal is one station sending this message. Empty: the stack's list of stations.
pub const GEN_SINGLE: Setting<field::Text> =
    Setting::new("gen_single", "Single station message", "", "")
        .section("Test signal")
        .advanced();
/// The audio frequency of the single station.
pub const GEN_HZ: Setting<field::Hz> =
    Setting::new("gen_hz", "Single station frequency", "Hz", 1500.0)
        .range(200.0, 3500.0, 1.0)
        .section("Test signal")
        .advanced();

/// The MSK144 test signal sends a short message as the 40-bit short frame.
pub const MSK144_SHORT: Setting<field::Bool> =
    Setting::new("gen_short", "Test signal short messages", "", false)
        .section("Test signal")
        .help(
            "Sends a single station message `<CALL1 CALL2> RPT`, whose report is -03, +00, +03, +06, +10, +13, +16, \
             R-03 to R+16, RRR or 73, as the 40-bit short frame.",
        )
        .advanced();

/// The labels of the sub-mode letters of JT65: the letter and its tone spacing.
const JT65_LETTERS: [&str; 3] = ["A (2.7 Hz)", "B (5.4 Hz)", "C (10.8 Hz)"];
/// The labels of the sub-mode letters of JT4: the letter and its tone spacing.
const JT4_LETTERS: [&str; 7] = [
    "A (4.4 Hz)",
    "B (8.75 Hz)",
    "C (17.5 Hz)",
    "D (35 Hz)",
    "E (70 Hz)",
    "F (140 Hz)",
    "G (280 Hz)",
];
/// The labels of the sub-mode letters of Q65.
const Q65_LETTERS: [&str; 5] = ["A", "B", "C", "D", "E"];

/// The declaration of `s` under the stack key `key`, in `section`.
fn renamed(s: &dyn DeclareSetting, key: String, section: &'static str) -> SettingDecl {
    let mut d = s.decl();
    d.field.key = Cow::Owned(key);
    if !section.is_empty() {
        d.section = Cow::Borrowed(section);
    }
    d
}

/// The sub-mode setting `<mode>_submode` of `mode` with these letter labels.
fn letters(mode: Mode, labels: &[&'static str]) -> SettingDecl {
    let choices: Vec<ChoiceDecl> = mode
        .submodes()
        .iter()
        .zip(labels)
        .map(|(k, l)| ChoiceDecl::new(k, l))
        .collect();
    SettingDecl::new(
        FieldDecl::with_choices(
            format!("{}_submode", mode.id()),
            format!("{} sub-mode", mode.name()),
            choices,
        ),
        Value::Choice(0),
    )
    .with_section("Modes")
}

/// The period setting `<mode>_period` of `mode`, with its default and its range. The cutter refuses a period that the mode
/// does not have.
fn period(mode: Mode, default: f64) -> SettingDecl {
    let p = mode.periods();
    let (lo, hi) = (
        p.iter().copied().fold(f64::INFINITY, f64::min),
        p.iter().copied().fold(0.0, f64::max),
    );
    let mut d = crate::components::PERIOD.decl();
    d.field.key = Cow::Owned(format!("{}_period", mode.id()));
    d.field.label = Cow::Owned(format!("{} T/R period", mode.name()));
    d.default = Value::Duration(default);
    d.range = Some(sdrkit_core::settings::Range {
        min: lo,
        max: hi,
        step: 0.0,
    });
    d.help = Cow::Owned(format!(
        "One of {} s.",
        p.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    d.with_section("Modes")
}

/// The declarations of the settings of `mode` (`None`: the combined stack).
fn decls_for(mode: Option<Mode>) -> Vec<SettingDecl> {
    let all = mode.is_none();
    let has = |m: Mode| all || mode == Some(m);
    let mut v = Vec::new();
    if all {
        for m in Mode::ALL.into_iter().filter(|&m| m != Mode::Js8) {
            let mut d = renamed(&ENABLE, format!("enable_{}", m.id()), "Modes");
            d.field.label = Cow::Owned(format!(
                "Decode {} ({} s slots)",
                m.name(),
                m.default_period()
            ));
            v.push(d);
        }
    }
    if has(Mode::Jt65) {
        v.push(letters(Mode::Jt65, &JT65_LETTERS));
    }
    if has(Mode::Jt4) {
        v.push(letters(Mode::Jt4, &JT4_LETTERS));
    }
    if has(Mode::Q65) {
        v.push(letters(Mode::Q65, &Q65_LETTERS));
        v.push(period(Mode::Q65, 60.0));
    }
    if has(Mode::Fst4) {
        v.push(period(Mode::Fst4, 60.0));
    }
    if has(Mode::Fst4w) {
        v.push(period(Mode::Fst4w, 120.0));
    }
    if has(Mode::Msk144) {
        v.push(period(Mode::Msk144, 15.0));
    }
    if has(Mode::Js8) {
        v.push(renamed(&JS8_NORMAL, "js8_normal".into(), "JS8"));
        v.push(renamed(&JS8_FAST, "js8_fast".into(), "JS8"));
        v.push(renamed(&JS8_TURBO, "js8_turbo".into(), "JS8"));
    }
    if all || mode.is_some_and(Mode::searches_mirrored) {
        v.push(MIRRORED.decl());
    }
    v.push(DEEP.decl());
    if early_too(mode) {
        v.push(EARLY_DECODE.decl());
    }
    v.push(DEMOD.decl());
    v.push(LISTEN.decl());
    v.push(renamed(&TIME_SOURCE, "time_source".into(), "Time"));
    v.push(renamed(&TIME_START, "time_start".into(), "Time"));
    v.push(renamed(&BAND_EDGES, "band_edges".into(), "Bands"));
    if wide_too(mode) {
        v.push(renamed(&DIALS, "dials".into(), "Bands"));
        v.push(renamed(&WIDE, "wide".into(), "Wide"));
        v.push(renamed(&WIDE_SPAN, "wide_span".into(), "Wide"));
        v.push(renamed(&MULTIBAND, "multiband".into(), "Wide"));
        v.push(renamed(
            &MULTIBAND_SELECT,
            "multiband_select".into(),
            "Wide",
        ));
    }
    if hop_too(mode) {
        let hop: [&dyn DeclareSetting; 5] = [
            &HOP_ENABLED,
            &HOP_COORDINATED,
            &HOP_HOURS,
            &HOP_BANDS,
            &HOP_CYCLE,
        ];
        for s in hop {
            v.push(renamed(s, format!("hop.{}", s.key()), "Band hopping"));
        }
    }
    v.push(SPOTS.decl());
    // The transmit side. My call and locator are the station's. The sequencer of a stack of one mode sends that mode.
    if qso_too(mode) {
        if all {
            v.push(renamed(&QSO_MODE, "qso_mode".into(), "Transmit"));
        }
        let qso: [&dyn DeclareSetting; 6] = [
            &QSO_EVEN,
            &QSO_AUTO,
            &QSO_HOLD,
            &QSO_HZ,
            &QSO_RETRIES,
            &QSO_WATCHDOG,
        ];
        for s in qso {
            v.push(renamed(s, format!("qso_{}", s.key()), "Transmit"));
        }
    }
    if has(Mode::Wspr) {
        let beacon: [&dyn DeclareSetting; 4] =
            [&BEACON_DBM, &BEACON_EVERY, &BEACON_OFFSET, &BEACON_HZ];
        for s in beacon {
            v.push(renamed(s, format!("beacon_{}", s.key()), "Transmit"));
        }
    }
    if has(Mode::Js8) {
        let js8: [&dyn DeclareSetting; 2] = [&JS8_SPEED, &JS8_HZ];
        for s in js8 {
            v.push(renamed(s, format!("js8_{}", s.key()), "Transmit"));
        }
    }
    let test: [&dyn DeclareSetting; 4] = [&GEN_MIRRORED, &GEN_LIVE, &GEN_SINGLE, &GEN_HZ];
    v.extend(test.iter().map(|s| s.decl()));
    if has(Mode::Msk144) {
        v.push(MSK144_SHORT.decl());
    }
    v
}

/// Whether the stack of `mode` (`None`: the combined stack) calls CQ and answers through the QSO sequencer: FT8, FT4,
/// FT2, JT65, JT9 and JT4.
pub fn qso_too(mode: Option<Mode>) -> bool {
    mode.is_none_or(|m| crate::components::QsoMode::of_mode(m).is_some())
}

/// Whether the stack of `mode` (`None`: the combined stack) transmits: the QSO sequencer, the WSPR beacon or the JS8
/// sender.
pub fn transmits_too(mode: Option<Mode>) -> bool {
    mode.is_none_or(crate::encode::offered_for_transmit)
}

/// Whether the stack of `mode` (`None`: the combined stack) also decodes the whole span and the multi-band monitor.
pub fn wide_too(mode: Option<Mode>) -> bool {
    mode.is_none() || mode.is_some_and(|m| bands_modes().contains(&m))
}

/// Whether the stack of `mode` hops bands (FT8, FT4, FT2 and WSPR; the combined stack does not).
pub fn hop_too(mode: Option<Mode>) -> bool {
    mode.is_some_and(|m| bands_modes().contains(&m))
}

/// Whether the stack of `mode` (`None`: the combined stack) has early decode (FT8 and FT4).
pub fn early_too(mode: Option<Mode>) -> bool {
    mode.is_none_or(|m| matches!(m, Mode::Ft8 | Mode::Ft4))
}

/// The modes of whole-span decoding.
fn bands_modes() -> &'static [Mode] {
    &crate::bands::MONITOR_MODES
}

/// The settings of one stack of the family, or of the combined stack (`None`). The station (my call and locator) comes from
/// the host's station, and the impairments of the test signal from the framework's `gen_*` settings.
pub fn schema_for(mode: Option<Mode>) -> Arc<Schema> {
    static SCHEMAS: [OnceLock<Arc<Schema>>; 13] = [const { OnceLock::new() }; 13];
    Arc::clone(SCHEMAS[slot_of(mode)].get_or_init(|| Arc::new(Schema::of(settings_of(mode)))))
}

/// The settings of the stack of `mode` (`None`: the combined stack) as the description lists them, made once.
pub fn settings_of(mode: Option<Mode>) -> &'static [&'static dyn DeclareSetting] {
    static LISTS: [OnceLock<Vec<&'static dyn DeclareSetting>>; 13] =
        [const { OnceLock::new() }; 13];
    LISTS[slot_of(mode)].get_or_init(|| {
        decls_for(mode)
            .into_iter()
            .map(|decl| {
                let key: &'static str = match &decl.field.key {
                    Cow::Borrowed(k) => k,
                    Cow::Owned(k) => Box::leak(k.clone().into_boxed_str()),
                };
                let s: &'static dyn DeclareSetting = Box::leak(Box::new(Declared { key, decl }));
                s
            })
            .collect()
    })
}

/// The place of the stack of `mode` in the lists of [`schema_for`] and [`settings_of`].
fn slot_of(mode: Option<Mode>) -> usize {
    mode.map_or(0, |m| usize::from(sdrkit_core::Choice::index(m)) + 1)
}

/// A setting whose key, label or section the stack sets: the declaration of a component's setting under the key of
/// the stack.
struct Declared {
    key: &'static str,
    decl: SettingDecl,
}

impl DeclareSetting for Declared {
    fn key(&self) -> &'static str {
        self.key
    }
    fn decl(&self) -> SettingDecl {
        self.decl.clone()
    }
}

/// The value of setting `key` of `v`, if its schema has it.
pub fn value<'a>(v: &'a Values, key: &str) -> Option<&'a Value> {
    v.schema().id(key).ok().and_then(|id| v.value(id))
}

/// The sub-mode that a set of values selects for `mode` (letter, period), with the host's sub-mode id (`jt65b`, `q65-30c`,
/// `js8fast`) when one is given. The host's id wins.
pub fn sub_of(mode: Mode, v: &Values, submode: Option<&str>) -> Sub {
    let letter = |k: &str| match value(v, &format!("{k}_submode")) {
        Some(Value::Choice(i)) => *i as u8,
        _ => 0,
    };
    let period = |d: f64| match value(v, &format!("{}_period", mode.id())) {
        Some(Value::Duration(p)) => *p,
        _ => d,
    };
    let mut s = Sub::new(mode);
    match mode {
        Mode::Jt65 => s = s.with_sub(letter("jt65")),
        Mode::Jt4 => s = s.with_sub(letter("jt4")),
        Mode::Q65 => s = s.with_sub(letter("q65")).with_period(period(60.0)),
        Mode::Fst4 => s = s.with_period(period(60.0)),
        Mode::Fst4w => s = s.with_period(period(120.0)),
        Mode::Msk144 => s = s.with_period(period(15.0)),
        _ => {}
    }
    if let Some(id) = submode {
        let id = id.to_ascii_lowercase();
        if let Some(rest) = id.strip_prefix(mode.id()) {
            let rest = rest.trim_start_matches(['-', '_']);
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(p) = digits.parse::<f64>() {
                s = s.with_period(p);
            }
            let tail = &rest[digits.len()..];
            if let Some(i) = mode
                .submodes()
                .iter()
                .position(|l| l.eq_ignore_ascii_case(tail))
            {
                s = s.with_sub(i as u8);
            }
        }
    }
    s
}

/// `v` with the sub-mode `sub` written into the sub-mode and period settings of its mode, where the schema has them.
pub fn with_sub(mut v: Values, sub: Sub) -> Values {
    let m = sub.mode;
    let mut put = |key: String, value: Value| {
        if let Ok(id) = v.schema().id(&key) {
            // A value the schema refuses keeps the setting as it was: the cutter would refuse it too.
            let _ = v.set(id, &value);
        }
    };
    if !m.submodes().is_empty() && m != Mode::Js8 {
        put(
            format!("{}_submode", m.id()),
            Value::Choice(u16::from(sub.sub)),
        );
    }
    if m.periods().len() > 1 && m != Mode::Js8 {
        put(format!("{}_period", m.id()), Value::Duration(sub.period));
    }
    v
}
