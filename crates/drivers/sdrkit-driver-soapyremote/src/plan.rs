//! The rules of the driver that need no connection: addresses, the Remote driver text, the choice of rate and filter, the gain
//! plan and the texts of the log.

use crate::rpc::{ArgInfo, ArgType, Kwargs, Range, DRIVER};
use sdrkit_core::net::host_port;
use sdrkit_core::{ChoiceDecl, FieldDecl, Hz, SettingDecl, Value, ValueKind};
use sdrkit_driver_core::DriverError;
use std::borrow::Cow;

/// The SoapyRemote server's default port.
pub const DEFAULT_PORT: u16 = 55132;
/// The key every find and make carries, so a server does not ask its own SoapyRemote module.
pub const NO_DEEPER: &str = "soapy_remote_no_deeper";

// ---- Addresses ----

/// Cleans a typed address (spaces removed, a leading `tcp://` removed, a trailing `:port` taken as the port) and splits it.
///
/// # Errors
///
/// [`DriverError::Address`] for a text that is not an address, or port 0.
pub fn clean_address(text: &str) -> Result<(String, u16), DriverError> {
    let mut a: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if a.len() >= 6 && a[..6].eq_ignore_ascii_case("tcp://") {
        a = a[6..].to_string();
    }
    let a = a.trim_end_matches('/');
    match host_port(a, Some(DEFAULT_PORT)) {
        Ok((h, p)) if p != 0 => Ok((h.to_owned(), p)),
        _ => Err(DriverError::Address),
    }
}

/// The Remote driver text as device arguments: comma-separated items, each trimmed of spaces and tabs. An item without `=`
/// means `driver=<item>`. Empty items are skipped.
pub fn parse_remote(text: &str) -> Kwargs {
    let mut kw = Kwargs::new();
    for item in text.split(',') {
        let item = item.trim_matches(|c| c == ' ' || c == '\t');
        if item.is_empty() {
            continue;
        }
        match item.split_once('=') {
            Some((k, v)) => {
                let k = k.trim_matches(|c| c == ' ' || c == '\t');
                if !k.is_empty() {
                    kw.insert(
                        k.to_string(),
                        v.trim_matches(|c| c == ' ' || c == '\t').to_string(),
                    );
                }
            }
            None => {
                kw.insert(DRIVER.into(), item.to_string());
            }
        }
    }
    kw
}

/// `kw` plus the no-deeper key (empty value).
pub fn with_no_deeper(kw: &Kwargs) -> Kwargs {
    let mut k = kw.clone();
    k.insert(NO_DEEPER.into(), String::new());
    k
}

// ---- Rate and filter ----

/// How a value is forced onto a stepped range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Snap {
    /// To the nearest grid value.
    Nearest,
    /// Up to the next grid value.
    Up,
}

/// Forces `v` onto `r`: held to its ends, and onto its step grid if it has one. It never goes past the ends: a grid value
/// past the top returns the largest grid value.
pub fn force_onto(r: Range, v: f64, snap: Snap) -> f64 {
    let (lo, hi) = if r.min <= r.max {
        (r.min, r.max)
    } else {
        (r.max, r.min)
    };
    let c = v.clamp(lo, hi);
    if r.step <= 0.0 || hi <= lo {
        return c;
    }
    let top = ((hi - lo) / r.step + 1e-9).floor();
    let k = (c - lo) / r.step;
    let k = match snap {
        Snap::Nearest => k.round(),
        Snap::Up => (k - 1e-9).ceil(),
    };
    lo + k.clamp(0.0, top) * r.step
}

/// The result of the rate choice.
#[derive(Clone, Debug, PartialEq)]
pub struct RateChoice {
    /// The rate to send.
    pub rate: Hz,
    /// The chosen rate equals the requested rate.
    pub exact: bool,
    /// The note when not exact (shown only when the rate read back is below 0.97 x the requested rate).
    pub note: Option<String>,
}

fn m3(hz: Hz) -> String {
    format!("{:.3}", hz / 1e6)
}

/// Chooses the rate to send for `asked` from the radio's rate table. An empty table sends the requested rate.
pub fn choose_rate(table: &[Range], asked: Hz) -> RateChoice {
    let exact = RateChoice {
        rate: asked,
        exact: true,
        note: None,
    };
    if table.is_empty() {
        return exact;
    }
    if table
        .iter()
        .any(|r| (force_onto(*r, asked, Snap::Nearest) - asked).abs() <= 1.0)
    {
        return exact;
    }
    let cands: Vec<f64> = table
        .iter()
        .map(|r| force_onto(*r, asked, Snap::Up))
        .collect();
    let up = cands
        .iter()
        .copied()
        .filter(|c| *c >= asked - 1.0)
        .min_by(f64::total_cmp);
    let c = up.unwrap_or_else(|| cands.iter().copied().fold(f64::MIN, f64::max));
    let note = if c >= asked - 1.0 {
        format!(
            "this radio does not offer {} Msps; {} Msps is used",
            m3(asked),
            m3(c)
        )
    } else {
        format!(
            "this radio only offers up to {} Msps, {} Msps were requested",
            m3(c),
            m3(asked)
        )
    };
    RateChoice {
        rate: c,
        exact: false,
        note: Some(note),
    }
}

/// The transmit rate: the smallest offered value at or above the requested rate, else the fastest. An empty table sends it as is.
pub fn choose_rate_up(table: &[Range], asked: Hz) -> Hz {
    choose_rate(table, asked).rate
}

/// The baseband filter to send: each range produces `wanted` forced onto it, and the nearest wins. `None` without ranges.
pub fn choose_bandwidth(table: &[Range], wanted: Hz) -> Option<Hz> {
    table
        .iter()
        .map(|r| force_onto(*r, wanted, Snap::Nearest))
        .min_by(|a, b| (a - wanted).abs().total_cmp(&(b - wanted).abs()))
}

/// The lowest minimum and highest maximum of `table`.
pub fn span(table: &[Range]) -> Option<(f64, f64)> {
    if table.is_empty() {
        return None;
    }
    let lo = table
        .iter()
        .map(|r| r.min.min(r.max))
        .fold(f64::MAX, f64::min);
    let hi = table
        .iter()
        .map(|r| r.max.max(r.min))
        .fold(f64::MIN, f64::max);
    Some((lo, hi))
}

/// The single values of `table`: the ranges from a value to itself, in the order of the table.
pub fn points(table: &[Range]) -> Vec<f64> {
    table
        .iter()
        .filter(|r| r.min == r.max)
        .map(|r| r.min)
        .collect()
}

/// The rate list of the capabilities: only continuous ranges and single rates. Stepped ranges count for min and max only.
pub fn rate_limits(table: &[Range]) -> sdrkit_driver_core::RateLimits {
    let Some((lo, hi)) = span(table) else {
        return Default::default();
    };
    sdrkit_driver_core::RateLimits {
        min_hz: lo,
        max_hz: hi,
        ranges: table
            .iter()
            .filter(|r| r.step <= 0.0 || r.min == r.max)
            .map(|r| (r.min.min(r.max), r.max.max(r.min)))
            .collect(),
        steady_hz: None,
    }
}

/// The note for a frequency outside the radio's ranges.
pub fn frequency_note(table: &[Range], f: Hz) -> Option<String> {
    let (lo, hi) = span(table)?;
    (f < lo || f > hi).then(|| {
        format!(
            "this radio is specified for {} to {} MHz: the driver can fail to reach {} MHz, or the signal can be weak or absent",
            sdrkit_driver_core::fmt_m(lo),
            sdrkit_driver_core::fmt_m(hi),
            sdrkit_driver_core::fmt_m(f)
        )
    })
}

/// The frequency sent when the radio cannot correct its own clock: `centre / (1 + ppm x 1e-6)`.
pub fn corrected(centre: Hz, ppm: f64) -> Hz {
    centre / (1.0 + ppm * 1e-6)
}

// ---- Gain ----

/// One gain element with its range.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// The element's name.
    pub name: String,
    /// Its range.
    pub range: Range,
}

/// Every element name, in upper case, is at least 3 characters long and ends in `GR` (an SDRplay): more is less gain.
pub fn is_gain_reduction(elements: &[Element]) -> bool {
    !elements.is_empty()
        && elements.iter().all(|e| {
            let u = e.name.to_uppercase();
            u.len() >= 3 && u.ends_with("GR")
        })
}

fn find<'a>(elements: &'a [Element], name: &str) -> Option<&'a Element> {
    elements.iter().find(|e| e.name.eq_ignore_ascii_case(name))
}

/// The RF stage of a gain-reduction radio with both RFGR and IFGR: (RFGR, IFGR).
pub fn rf_stage(elements: &[Element]) -> Option<(&Element, &Element)> {
    if !is_gain_reduction(elements) {
        return None;
    }
    Some((find(elements, "RFGR")?, find(elements, "IFGR")?))
}

/// The range of the gain setting: the overall range, or, for a gain-reduction radio whose overall span is below 10 dB, its minimum to
/// its minimum plus the sum of the element spans.
pub fn overall_range(overall: Range, elements: &[Element]) -> Range {
    if (overall.max - overall.min) < 10.0 && is_gain_reduction(elements) {
        let sum: f64 = elements
            .iter()
            .map(|e| (e.range.max - e.range.min).max(0.0))
            .sum();
        Range::new(overall.min, overall.min + sum, overall.step)
    } else {
        overall
    }
}

/// What a gain is sent as.
#[derive(Clone, Debug, PartialEq)]
pub enum GainPlan {
    /// The overall gain (call 703) with this value.
    Overall(f64),
    /// The gain of each (element, value) in order (call 704).
    Elements(Vec<(String, f64)>),
}

/// The gain plan for the gain `g`. `rfgr` is the RF stage state (`None`: the gain sets it).
pub fn gain_plan(overall: Range, elements: &[Element], g: f64, rfgr: Option<i32>) -> GainPlan {
    let r = overall_range(overall, elements);
    let (lo, hi) = (r.min.min(r.max), r.max.max(r.min));
    let g = g.clamp(lo, hi);
    if let Some((rf, ifg)) = rf_stage(elements) {
        let (ifl, ifh) = (ifg.range.min, ifg.range.max);
        let (rfl, rfh) = (rf.range.min, rf.range.max);
        let (rfv, ifv) = match rfgr {
            None => {
                let t = if hi > lo {
                    ((g - lo) / (hi - lo)).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let ifv = (ifh - t * (ifh - ifl)).clamp(ifl, ifh).round();
                let rfv = (rfh - t * (rfh - rfl)).round().clamp(rfl, rfh);
                (rfv, ifv)
            }
            Some(n) => {
                let rfv = (n as f64).clamp(rfl, rfh);
                let ifv = (ifh - (g - lo)).clamp(ifl, ifh);
                (rfv, ifv)
            }
        };
        return GainPlan::Elements(vec![(rf.name.clone(), rfv), (ifg.name.clone(), ifv)]);
    }
    if is_gain_reduction(elements) {
        return GainPlan::Overall(hi - (g - lo));
    }
    GainPlan::Overall(g)
}

// ---- What the radio says ----

/// The radio's settings keys that drive a bias tee.
pub fn is_bias_key(key: &str) -> bool {
    matches!(
        key.to_lowercase().as_str(),
        "biastee" | "bias_tee" | "biast_ctrl"
    )
}

/// A bool setting value as the radio writes it.
pub fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Words in a make error that mean another program holds the radio.
pub fn is_busy(text: &str) -> bool {
    let t = text.to_lowercase();
    [
        "busy",
        "in use",
        "claim",
        "resource temporarily",
        "already open",
        "access denied",
    ]
    .iter()
    .any(|w| t.contains(w))
}

// ---- Status and sizes ----

fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The status line: empty while the stream is clean.
pub fn status_text(overflows: u64, lost: u64, reconnects: u64) -> String {
    let mut v = Vec::new();
    if overflows > 0 {
        v.push(format!(
            "{} at the server (it or its network is too slow)",
            plural(overflows, "overflow", "overflows")
        ));
    }
    if lost > 0 {
        v.push(format!(
            "{} lost on the network",
            plural(lost, "datagram", "datagrams")
        ));
    }
    if reconnects > 0 {
        v.push(plural(reconnects, "reconnect", "reconnects"));
    }
    v.join(", ")
}

/// The window in packets from the receive buffer: at least 1.
pub fn window_packets(granted: usize, mtu: usize) -> u32 {
    ((granted / mtu.max(1)) as u32).max(1)
}

/// Elements per full data datagram of one channel.
pub fn elements_per_datagram(mtu: usize, bytes_per_element: usize) -> usize {
    mtu.saturating_sub(72) / bytes_per_element.max(1)
}

/// The RPC version as text (0x00000400 as "4.0").
pub fn show_version(v: u32) -> String {
    format!("{}.{}", v >> 8, v & 0xff)
}

/// SoapySDR stream error names.
pub fn error_name(code: i32) -> String {
    match code {
        -1 => "TIMEOUT".into(),
        -2 => "STREAM_ERROR".into(),
        -3 => "CORRUPTION".into(),
        -4 => "OVERFLOW".into(),
        -5 => "NOT_SUPPORTED".into(),
        -6 => "TIME_ERROR".into(),
        -7 => "UNDERFLOW".into(),
        c => format!("error {c}"),
    }
}

// ---- Settings that the radio reveals at open ----

/// How a run-time setting of an open radio reaches the radio.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Control {
    /// A setting of the radio's SoapySDR driver (call 1400), written as text.
    Module {
        /// The key of the setting in the driver.
        key: String,
        /// How a value is written.
        form: ModuleForm,
    },
    /// One gain element (call 704), in dB.
    Stage {
        /// The name of the element.
        element: String,
        /// The gain of the element when the radio opened. At this value the element follows the gain setting.
        at_open: f64,
    },
}

/// How a value of a driver setting is written to the radio.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ModuleForm {
    /// `true` or `false`.
    Bool,
    /// One of these options, by its index in the choice list.
    Options(Vec<String>),
    /// A whole number.
    Int,
    /// A number.
    Float,
}

/// A setting that the open radio adds to the schema: how it reaches the radio, and its declaration.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunTime {
    /// How a value reaches the radio.
    pub control: Control,
    /// The declaration, with the value of the radio at open as the default.
    pub decl: SettingDecl,
}

impl RunTime {
    /// The text written to the radio for `v`. `None` for a stage or a value of another kind.
    pub fn wire(&self, v: &Value) -> Option<String> {
        let Control::Module { form, .. } = &self.control else {
            return None;
        };
        Some(match (form, v) {
            (ModuleForm::Bool, Value::Bool(b)) => b.to_string(),
            (ModuleForm::Options(o), Value::Choice(i)) => o.get(usize::from(*i))?.clone(),
            (ModuleForm::Int, Value::I64(n)) => n.to_string(),
            (ModuleForm::Float, Value::F64(x)) => x.to_string(),
            _ => return None,
        })
    }
}

/// The driver setting that `info` (call 1402) describes, with the radio's value `radio_value` (call 1401). `None` for an
/// empty key, the bias-tee key, and free text.
pub(crate) fn module_setting(info: &ArgInfo, radio_value: Option<&str>) -> Option<RunTime> {
    if info.key.is_empty() || is_bias_key(&info.key) {
        return None;
    }
    let value = radio_value
        .filter(|v| !v.is_empty())
        .unwrap_or(&info.value)
        .trim();
    let key = format!("soapy.{}", info.key);
    let label = if info.name.is_empty() {
        info.key.clone()
    } else {
        info.name.clone()
    };
    let kind = info.kind.unwrap_or(ArgType::String);
    let (decl, form) = if kind == ArgType::Bool {
        let field = FieldDecl::new(key, label, "", ValueKind::Bool);
        (
            SettingDecl::new(field, Value::Bool(truthy(value))),
            ModuleForm::Bool,
        )
    } else if !info.options.is_empty() {
        let names = if info.option_names.len() == info.options.len() {
            &info.option_names
        } else {
            &info.options
        };
        let choices: Vec<ChoiceDecl> = info
            .options
            .iter()
            .zip(names)
            .map(|(k, n)| ChoiceDecl {
                key: Cow::Owned(k.clone()),
                label: Cow::Owned(n.clone()),
            })
            .collect();
        let at = info.options.iter().position(|o| o == value).unwrap_or(0);
        let field = FieldDecl::with_choices(key, label, choices);
        (
            SettingDecl::new(field, Value::Choice(at as u16)),
            ModuleForm::Options(info.options.clone()),
        )
    } else if matches!(kind, ArgType::Int | ArgType::Float) && info.range.max > info.range.min {
        let r = info.range;
        let x = value.parse::<f64>().unwrap_or(r.min).clamp(r.min, r.max);
        let (vk, default, form, step) = if kind == ArgType::Int {
            let step = if r.step > 0.0 { r.step } else { 1.0 };
            (
                ValueKind::I64,
                Value::I64(x.round() as i64),
                ModuleForm::Int,
                step,
            )
        } else {
            (
                ValueKind::F64,
                Value::F64(x),
                ModuleForm::Float,
                r.step.max(0.0),
            )
        };
        let field = FieldDecl::new(key, label, info.units.clone(), vk);
        (
            SettingDecl::new(field, default).with_range(r.min, r.max, step),
            form,
        )
    } else {
        return None;
    };
    let help = if info.description.is_empty() {
        format!("A setting of the radio's SoapySDR driver ({}).", info.key)
    } else {
        format!(
            "{} (SoapySDR driver setting {}).",
            info.description.trim_end_matches('.'),
            info.key
        )
    };
    Some(RunTime {
        control: Control::Module {
            key: info.key.clone(),
            form,
        },
        decl: decl.with_section("Radio").with_help(help),
    })
}

/// The gain elements that are settings of their own: none for a radio with one element or an element without a range, and
/// not the RF stage of a gain-reduction radio (it has [`RF_STATE`](crate::RF_STATE)).
pub(crate) fn stages(elements: &[Element]) -> impl Iterator<Item = &Element> {
    let any = elements.len() >= 2 && elements.iter().all(|e| e.range.max > e.range.min);
    let rf = rf_stage(elements).is_some();
    elements.iter().filter(move |e| {
        any && !(rf && (e.name.eq_ignore_ascii_case("IFGR") || e.name.eq_ignore_ascii_case("RFGR")))
    })
}

/// The setting of gain element `e`, whose gain was `at_open` dB when the radio opened.
pub(crate) fn stage_setting(e: &Element, at_open: Option<f64>) -> RunTime {
    let r = e.range;
    let at_open = at_open.unwrap_or(r.min).clamp(r.min, r.max);
    let field = FieldDecl::new(
        format!("gain.{}", e.name),
        stage_label(&e.name),
        "dB",
        ValueKind::F64,
    );
    RunTime {
        control: Control::Stage {
            element: e.name.clone(),
            at_open,
        },
        decl: SettingDecl::new(field, Value::F64(at_open))
            .with_range(r.min, r.max, r.step.max(0.0))
            .with_section("Gain stages")
            .with_help(format!(
                "The gain of the element {} of the radio. A value other than the one at open overrides what the gain setting \
                 gives this element.",
                e.name
            )),
    }
}

/// The label of a gain element.
pub(crate) fn stage_label(element: &str) -> String {
    let named = match element.to_uppercase().as_str() {
        "LNA" => "Low-noise amplifier",
        "VGA" => "Variable-gain amplifier",
        "TIA" => "Transimpedance amplifier",
        "PGA" => "Programmable-gain amplifier",
        "AMP" => "RF amplifier",
        "RF" => "RF gain",
        "IF" => "IF gain",
        "BB" => "Baseband gain",
        "MIX" | "MIXER" => "Mixer gain",
        "ATT" => "Attenuator",
        "IFGR" => "IF gain reduction",
        "RFGR" => "RF gain reduction (LNA state)",
        "LNAG" => "LNA gain",
        "TUNER" => "Tuner gain",
        "PAD" => "Output pad",
        "IAMP" => "Intermediate amplifier",
        _ => return format!("Gain stage {element}"),
    };
    format!("{named} ({element})")
}
