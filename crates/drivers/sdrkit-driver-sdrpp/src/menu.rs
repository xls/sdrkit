//! The menu of the server: the draw list read into controls, names from ids, the sample-rate list, the gain plan, and the
//! other controls as settings of the open radio.

use crate::proto::ActionValue;
use sdrkit_core::{ChoiceDecl, FieldDecl, SettingDecl, Value, ValueKind};
use std::borrow::Cow;

/// Element type bytes.
pub mod elem {
    /// A step: u8 code, u8 force-sync flag.
    pub const STEP: u8 = 0x00;
    /// u8.
    pub const BOOL: u8 = 0x01;
    /// i32.
    pub const INT: u8 = 0x02;
    /// f32.
    pub const FLOAT: u8 = 0x03;
    /// u16 length, bytes.
    pub const STRING: u8 = 0x04;
}

/// Step codes.
pub mod step {
    #![allow(missing_docs)]
    pub const FILL_WIDTH: u8 = 0x00;
    pub const SAME_LINE: u8 = 0x01;
    pub const BEGIN_DISABLED: u8 = 0x02;
    pub const END_DISABLED: u8 = 0x03;
    pub const COMBO: u8 = 0x80;
    pub const BUTTON: u8 = 0x81;
    pub const COLUMNS: u8 = 0x82;
    pub const NEXT_COLUMN: u8 = 0x83;
    pub const RADIO_BUTTON: u8 = 0x84;
    pub const BEGIN_GROUP: u8 = 0x85;
    pub const END_GROUP: u8 = 0x86;
    pub const LEFT_LABEL: u8 = 0x87;
    pub const SLIDER_INT: u8 = 0x88;
    pub const SLIDER_FLOAT_WITH_STEPS: u8 = 0x89;
    pub const INPUT_INT: u8 = 0x8a;
    pub const CHECKBOX: u8 = 0x8b;
    pub const SLIDER_FLOAT: u8 = 0x8c;
    pub const INPUT_TEXT: u8 = 0x8d;
    pub const TEXT: u8 = 0x8e;
    pub const TEXT_COLORED: u8 = 0x8f;
}

/// The id of the list the server itself draws: the source modules loaded.
pub const SERVER_SOURCE_ID: &str = "##sdrpp_server_src_sel";

/// One element of a draw list.
#[derive(Clone, Debug, PartialEq)]
enum Elem {
    Step(u8),
    Bool(bool),
    Int(i32),
    Float(f32),
    Str(String),
}

/// The draw list cannot be read: an unknown element type, or an element cut short.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unreadable;

fn elements(mut b: &[u8]) -> Result<Vec<Elem>, Unreadable> {
    let mut out = Vec::new();
    while let Some((&t, rest)) = b.split_first() {
        b = rest;
        let (e, n) = match t {
            elem::STEP => (Elem::Step(*b.first().ok_or(Unreadable)?), 2),
            elem::BOOL => (Elem::Bool(*b.first().ok_or(Unreadable)? != 0), 1),
            elem::INT => (
                Elem::Int(i32::from_le_bytes(
                    b.get(..4)
                        .ok_or(Unreadable)?
                        .try_into()
                        .expect("the range has the array's length"),
                )),
                4,
            ),
            elem::FLOAT => (
                Elem::Float(f32::from_le_bytes(
                    b.get(..4)
                        .ok_or(Unreadable)?
                        .try_into()
                        .expect("the range has the array's length"),
                )),
                4,
            ),
            elem::STRING => {
                let len = u16::from_le_bytes(
                    b.get(..2)
                        .ok_or(Unreadable)?
                        .try_into()
                        .expect("the range has the array's length"),
                ) as usize;
                let s = b.get(2..2 + len).ok_or(Unreadable)?;
                (Elem::Str(String::from_utf8_lossy(s).into_owned()), 2 + len)
            }
            _ => return Err(Unreadable),
        };
        if b.len() < n {
            return Err(Unreadable);
        }
        b = &b[n..];
        out.push(e);
    }
    Ok(out)
}

/// What kind of control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A combo list.
    List,
    /// A check box.
    CheckBox,
    /// One choice of a radio group.
    Radio,
    /// An integer slider.
    SliderInt,
    /// A float slider, stepped (step above 0) or not.
    SliderFloat,
    /// An integer input (no range).
    InputInt,
    /// A line of text.
    Text,
}

/// One control read from the menu.
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    /// Its kind.
    pub kind: Kind,
    /// Its full id (empty for text).
    pub id: String,
    /// Its label; for text, the text.
    pub label: String,
    /// Greyed out.
    pub disabled: bool,
    /// List entries.
    pub entries: Vec<String>,
    /// The list's selected index.
    pub selected: i32,
    /// Check box on, radio button chosen.
    pub on: bool,
    /// Slider or input value.
    pub value: f64,
    /// Slider minimum.
    pub min: f64,
    /// Slider maximum.
    pub max: f64,
    /// The step: 1 for integer sliders, the stepped float slider's step, 0 for a plain float slider; inputs max(step, 1).
    pub step: f64,
}

impl Control {
    fn new(kind: Kind, id: String, label: String, disabled: bool) -> Self {
        Control {
            kind,
            id,
            label,
            disabled,
            entries: Vec::new(),
            selected: 0,
            on: false,
            value: 0.0,
            min: 0.0,
            max: 0.0,
            step: 0.0,
        }
    }
    /// A slider of either kind.
    pub fn is_slider(&self) -> bool {
        matches!(self.kind, Kind::SliderInt | Kind::SliderFloat)
    }
    /// An integer control (its value goes out as INT).
    pub fn is_int(&self) -> bool {
        matches!(self.kind, Kind::SliderInt | Kind::InputInt)
    }
    /// The visible part of the id.
    pub fn visible(&self) -> &str {
        split_id(&self.id).0
    }
    /// The hidden part of the id.
    pub fn hidden(&self) -> &str {
        split_id(&self.id).1
    }
    /// The words of the id.
    pub fn words(&self) -> Vec<&str> {
        words(self.hidden())
    }
    /// The id words carry `w` (ignoring ASCII case).
    pub fn carries(&self, w: &str) -> bool {
        self.words().iter().any(|x| x.eq_ignore_ascii_case(w))
    }
    /// The tolerance below which a slider or input is not moved.
    pub fn tolerance(&self) -> f64 {
        if self.is_int() {
            0.5
        } else {
            (self.step / 2.0).max(1e-4)
        }
    }
    /// The selected entry's text, when the index is valid.
    pub fn selected_entry(&self) -> Option<&str> {
        usize::try_from(self.selected)
            .ok()
            .and_then(|i| self.entries.get(i))
            .map(|s| s.as_str())
    }
}

/// The server's menu, read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Menu {
    /// Controls and text lines, in menu order.
    pub controls: Vec<Control>,
}

fn str_at(a: &[Elem], i: usize) -> Option<&str> {
    match a.get(i) {
        Some(Elem::Str(s)) => Some(s),
        _ => None,
    }
}
fn int_at(a: &[Elem], i: usize) -> Option<i32> {
    match a.get(i) {
        Some(Elem::Int(v)) => Some(*v),
        _ => None,
    }
}
fn float_at(a: &[Elem], i: usize) -> Option<f32> {
    match a.get(i) {
        Some(Elem::Float(v)) => Some(*v),
        _ => None,
    }
}
fn bool_at(a: &[Elem], i: usize) -> Option<bool> {
    match a.get(i) {
        Some(Elem::Bool(v)) => Some(*v),
        _ => None,
    }
}

/// Splits combo entries at NUL; a trailing NUL makes no empty last entry.
pub fn split_entries(s: &str) -> Vec<String> {
    let s = s.strip_suffix('\0').unwrap_or(s);
    if s.is_empty() {
        return Vec::new();
    }
    s.split('\0').map(str::to_string).collect()
}

impl Menu {
    /// Reads a draw list (the body of a GET_UI or UI_ACTION ACK after its 4-byte command).
    pub fn parse(body: &[u8]) -> Result<Menu, Unreadable> {
        let el = elements(body)?;
        if el.is_empty() {
            return Ok(Menu::default());
        }
        if !matches!(el[0], Elem::Step(_)) {
            return Err(Unreadable);
        }
        let mut controls = Vec::new();
        let mut disabled = 0u32;
        let mut left: Option<String> = None;
        let mut i = 0;
        while i < el.len() {
            let Elem::Step(code) = el[i] else {
                unreachable!("steps start each group")
            };
            let start = i + 1;
            let mut end = start;
            while end < el.len() && !matches!(el[end], Elem::Step(_)) {
                end += 1;
            }
            let a = &el[start..end];
            i = end;
            match code {
                step::FILL_WIDTH | step::SAME_LINE => continue,
                step::BEGIN_DISABLED => {
                    disabled += 1;
                    continue;
                }
                step::END_DISABLED => {
                    disabled = disabled.saturating_sub(1);
                    continue;
                }
                step::LEFT_LABEL => {
                    left = str_at(a, 0).map(str::to_string);
                    continue;
                }
                _ => {}
            }
            let label_for = |kind: Kind, id: &str| -> String {
                if let Some(l) = left.as_deref().filter(|l| !l.is_empty()) {
                    return l.to_string();
                }
                let vis = split_id(id).0;
                if !vis.is_empty() && kind != Kind::Radio {
                    return vis.to_string();
                }
                name_from_id(id)
            };
            let dis = disabled > 0;
            let c = match code {
                step::COMBO => (|| {
                    let id = str_at(a, 0)?;
                    let sel = int_at(a, 1)?;
                    let entries = str_at(a, 2)?;
                    int_at(a, 3)?;
                    let mut c = Control::new(Kind::List, id.into(), label_for(Kind::List, id), dis);
                    c.selected = sel;
                    c.entries = split_entries(entries);
                    Some(c)
                })(),
                step::RADIO_BUTTON => (|| {
                    let id = str_at(a, 0)?;
                    let on = bool_at(a, 1)?;
                    let mut c =
                        Control::new(Kind::Radio, id.into(), label_for(Kind::Radio, id), dis);
                    c.on = on;
                    Some(c)
                })(),
                step::CHECKBOX => (|| {
                    let id = str_at(a, 0)?;
                    let on = bool_at(a, 1)?;
                    let mut c = Control::new(
                        Kind::CheckBox,
                        id.into(),
                        label_for(Kind::CheckBox, id),
                        dis,
                    );
                    c.on = on;
                    Some(c)
                })(),
                step::SLIDER_INT => (|| {
                    let id = str_at(a, 0)?;
                    let (v, lo, hi) = (int_at(a, 1)?, int_at(a, 2)?, int_at(a, 3)?);
                    int_at(a, 4)?;
                    int_at(a, 5)?;
                    let mut c = Control::new(
                        Kind::SliderInt,
                        id.into(),
                        label_for(Kind::SliderInt, id),
                        dis,
                    );
                    (c.value, c.min, c.max, c.step) = (v as f64, lo as f64, hi as f64, 1.0);
                    Some(c)
                })(),
                step::SLIDER_FLOAT_WITH_STEPS => (|| {
                    let id = str_at(a, 0)?;
                    let (v, lo, hi, st) = (
                        float_at(a, 1)?,
                        float_at(a, 2)?,
                        float_at(a, 3)?,
                        float_at(a, 4)?,
                    );
                    int_at(a, 5)?;
                    let mut c = Control::new(
                        Kind::SliderFloat,
                        id.into(),
                        label_for(Kind::SliderFloat, id),
                        dis,
                    );
                    (c.value, c.min, c.max, c.step) = (v as f64, lo as f64, hi as f64, st as f64);
                    Some(c)
                })(),
                step::SLIDER_FLOAT => (|| {
                    let id = str_at(a, 0)?;
                    let (v, lo, hi) = (float_at(a, 1)?, float_at(a, 2)?, float_at(a, 3)?);
                    int_at(a, 4)?;
                    int_at(a, 5)?;
                    let mut c = Control::new(
                        Kind::SliderFloat,
                        id.into(),
                        label_for(Kind::SliderFloat, id),
                        dis,
                    );
                    (c.value, c.min, c.max, c.step) = (v as f64, lo as f64, hi as f64, 0.0);
                    Some(c)
                })(),
                step::INPUT_INT => (|| {
                    let id = str_at(a, 0)?;
                    let v = int_at(a, 1)?;
                    let st = int_at(a, 2)?;
                    int_at(a, 3)?;
                    int_at(a, 4)?;
                    let mut c = Control::new(
                        Kind::InputInt,
                        id.into(),
                        label_for(Kind::InputInt, id),
                        dis,
                    );
                    c.value = v as f64;
                    c.step = (st as f64).max(1.0);
                    Some(c)
                })(),
                step::TEXT | step::TEXT_COLORED => a
                    .iter()
                    .rev()
                    .find_map(|e| match e {
                        Elem::Str(s) if !s.is_empty() => Some(s.clone()),
                        _ => None,
                    })
                    .map(|t| Control::new(Kind::Text, String::new(), t, dis)),
                _ => None,
            };
            left = None;
            if let Some(c) = c {
                controls.push(c);
            }
        }
        Ok(Menu { controls })
    }

    /// The control with this id.
    pub fn get(&self, id: &str) -> Option<&Control> {
        self.controls
            .iter()
            .find(|c| c.id == id && c.kind != Kind::Text)
    }

    /// The server-source list.
    pub fn server_source(&self) -> Option<&Control> {
        self.controls
            .iter()
            .find(|c| c.kind == Kind::List && c.id == SERVER_SOURCE_ID)
    }

    /// The source module in use; None when no source module is loaded.
    pub fn source(&self) -> Option<&str> {
        self.server_source()?.selected_entry()
    }

    /// The text lines.
    pub fn texts(&self) -> Vec<&str> {
        self.controls
            .iter()
            .filter(|c| c.kind == Kind::Text)
            .map(|c| c.label.as_str())
            .collect()
    }

    /// The sample-rate list.
    pub fn rate_list(&self) -> Option<&Control> {
        let cands: Vec<&Control> = self
            .controls
            .iter()
            .filter(|c| {
                c.kind == Kind::List
                    && c.id != SERVER_SOURCE_ID
                    && !c.entries.is_empty()
                    && c.entries.iter().all(|e| rate_text(e) > 0.0)
            })
            .collect();
        let first = cands.iter().find(|c| {
            let h = c.hidden().to_ascii_lowercase();
            c.carries("sr")
                || h.contains("samplerate")
                || h.contains("sample_rate")
                || h.contains("_sr")
        });
        first
            .or_else(|| {
                cands.iter().find(|c| {
                    let h = c.hidden().to_ascii_lowercase();
                    !c.carries("bw") && !h.contains("bandwidth") && !h.contains("_bw")
                })
            })
            .copied()
    }

    /// The rates of the sample-rate list, in list order.
    pub fn rates(&self) -> Vec<f64> {
        self.rate_list()
            .map(|l| l.entries.iter().map(|e| rate_text(e)).collect())
            .unwrap_or_default()
    }

    /// The gain plan.
    pub fn gain_plan(&self) -> Option<GainPlan<'_>> {
        let sliders: Vec<&Control> = self
            .controls
            .iter()
            .filter(|c| c.is_slider() && c.min != c.max)
            .collect();
        let is_stage = |c: &Control| {
            ["lna", "vga", "mix", "mixer", "if"]
                .iter()
                .any(|w| c.carries(w))
        };
        let mut best: Option<(&Control, u8)> = None;
        for c in sliders.iter().copied().filter(|c| !is_stage(c)) {
            if !(c.hidden().to_ascii_lowercase().contains("gain")
                || c.label.to_ascii_lowercase().contains("gain"))
            {
                continue;
            }
            let score = if c.carries("lin") {
                4
            } else if c.carries("sens") {
                3
            } else {
                2
            };
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((c, score));
            }
        }
        let sliders: Vec<&Control> = match best {
            Some((c, _)) => vec![c],
            None => sliders.into_iter().filter(|c| is_stage(c)).collect(),
        };
        if sliders.is_empty() {
            return None;
        }
        let (lo, hi) = if sliders.len() == 1 {
            let c = sliders[0];
            (c.min.min(c.max), c.min.max(c.max))
        } else {
            (0.0, sliders.iter().map(|c| (c.max - c.min).abs()).sum())
        };
        Some(GainPlan { sliders, lo, hi })
    }
}

/// The sliders the driver moves for the gain, and the range it offers.
#[derive(Clone, Debug, PartialEq)]
pub struct GainPlan<'a> {
    /// In menu order.
    pub sliders: Vec<&'a Control>,
    /// The low end of the range.
    pub lo: f64,
    /// The high end of the range.
    pub hi: f64,
}

impl GainPlan<'_> {
    /// At least one slider is not disabled.
    pub fn drivable(&self) -> bool {
        self.sliders.iter().any(|c| !c.disabled)
    }
    /// The step of the range offered: 1 unless it is one float slider.
    pub fn step(&self) -> f64 {
        match self.sliders.as_slice() {
            [c] if c.kind == Kind::SliderFloat => {
                if c.step > 0.0 {
                    c.step
                } else {
                    0.1
                }
            }
            _ => 1.0,
        }
    }
}

/// A slider's target for `gain` within the plan's range.
pub fn gain_target(c: &Control, gain: f64, lo: f64, hi: f64) -> f64 {
    let frac = if hi > lo {
        ((gain - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let t = c.min + frac * (c.max - c.min);
    if c.kind == Kind::SliderInt {
        t.round()
    } else if c.step > 0.0 {
        c.min + ((t - c.min) / c.step).round() * c.step
    } else {
        t
    }
}

/// Reads a rate text: a number then an optional unit; 0 when it is not a rate.
pub fn rate_text(s: &str) -> f64 {
    let s = s.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return 0.0;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let k = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > k {
            i = j;
        }
    }
    let Ok(v) = s[..i].parse::<f64>() else {
        return 0.0;
    };
    if !v.is_finite() || v <= 0.0 {
        return 0.0;
    }
    let unit: String = s[i..]
        .chars()
        .filter(|c| *c != ' ')
        .collect::<String>()
        .to_ascii_lowercase();
    let is_unit = |u: &str| matches!(u, "" | "hz" | "sps" | "s/s");
    let mult = if is_unit(&unit) {
        1.0
    } else {
        let (p, rest) = unit.split_at(unit.chars().next().map_or(0, char::len_utf8));
        match (p, is_unit(rest)) {
            ("k", true) => 1e3,
            ("m", true) => 1e6,
            ("g", true) => 1e9,
            _ => return 0.0,
        }
    };
    v * mult
}

/// Splits an id at its first `##` into the visible and the hidden part.
pub fn split_id(id: &str) -> (&str, &str) {
    match id.find("##") {
        Some(i) => (&id[..i], &id[i + 2..]),
        None => (id, ""),
    }
}

/// The words of a hidden part.
pub fn words(hidden: &str) -> Vec<&str> {
    let mut w: Vec<&str> = hidden.split('_').filter(|s| !s.is_empty()).collect();
    if w.last()
        .is_some_and(|l| l.contains(' ') || l.bytes().any(|b| b.is_ascii_uppercase()))
    {
        w.pop();
    }
    if w.len() > 1 {
        w.remove(0);
    }
    w
}

fn word_name(w: &str) -> Option<&'static str> {
    Some(match w.to_ascii_lowercase().as_str() {
        "dev" => "Device",
        "sr" => "Sample rate",
        "bw" => "Bandwidth",
        "ant" => "Antenna",
        "gm" => "Gain mode",
        "clk" => "Clock",
        "ch" | "chan" => "Channel",
        "refr" => "Refresh",
        "ds" => "Direct sampling",
        "agc" => "AGC",
        "port" => "Port",
        "lna" => "LNA",
        "vga" => "VGA",
        "mix" => "Mixer",
        "ref" => "Reference",
        "att" | "atten" | "attn" => "Attenuator",
        "ppm" => "PPM",
        "biast" => "Bias tee",
        "ofs" => "Offset tuning",
        "rtl" => "RTL",
        "lin" => "Linear",
        "sens" => "Sensitive",
        _ => return None,
    })
}

/// A name made from an id's words.
pub fn name_from_id(id: &str) -> String {
    if id == SERVER_SOURCE_ID {
        return "Server source".into();
    }
    let hidden = split_id(id).1;
    let parts: Vec<String> = words(hidden)
        .into_iter()
        .filter(|w| !w.eq_ignore_ascii_case("sel") && !w.eq_ignore_ascii_case("select"))
        .map(|w| word_name(w).map_or_else(|| w.to_ascii_lowercase(), str::to_string))
        .collect();
    let joined = parts.join(" ");
    if joined.is_empty() {
        return hidden.to_string();
    }
    let mut c = joined.chars();
    let first = c.next().expect("not empty");
    first.to_uppercase().chain(c).collect()
}

/// `v` with at most 6 significant digits.
fn six_digits(v: f64) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    format!("{v:.5e}").parse().unwrap_or(v)
}

/// How a run-time setting reaches a control of the server's menu.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    /// A list: the value is the index of the entry.
    List(String),
    /// A check box.
    CheckBox(String),
    /// A group of radio buttons: the value picks the button with this id.
    Radio(Vec<String>),
    /// A slider or an input, with the kind of value it takes.
    Number(String, Kind),
}

impl Target {
    /// The id of the control that `v` sets, and the value of the action. `None` for a value of another kind.
    pub(crate) fn action(&self, v: &Value) -> Option<(&str, ActionValue)> {
        Some(match (self, v) {
            (Target::List(id), Value::Choice(i)) => (id, ActionValue::Int(i32::from(*i))),
            (Target::CheckBox(id), Value::Bool(b)) => (id, ActionValue::Bool(*b)),
            (Target::Radio(ids), Value::Choice(i)) => {
                (ids.get(usize::from(*i))?, ActionValue::Bool(true))
            }
            (Target::Number(id, Kind::SliderFloat), Value::F64(x)) => {
                (id, ActionValue::Float(*x as f32))
            }
            (Target::Number(id, Kind::SliderInt | Kind::InputInt), Value::I64(n)) => {
                (id, ActionValue::Int(i32::try_from(*n).ok()?))
            }
            _ => return None,
        })
    }
}

/// A control of the server's menu as a setting of the open radio.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ServerSetting {
    /// Where a value goes.
    pub target: Target,
    /// The declaration, with the value on the server as the default.
    pub decl: SettingDecl,
}

/// The controls of `menu` as settings of the open radio, in menu order: every control but text, the sample-rate list and the
/// gain sliders, which [`RATE`](sdrkit_driver_core::RATE) and [`GAIN`](sdrkit_driver_core::GAIN) drive. The radio buttons
/// of one group make one setting.
pub(crate) fn server_settings(menu: &Menu) -> Vec<ServerSetting> {
    let rate = menu.rate_list().map(|c| c.id.as_str());
    let plan: Vec<&str> = menu
        .gain_plan()
        .map(|p| p.sliders.iter().map(|c| c.id.as_str()).collect())
        .unwrap_or_default();
    let mut out: Vec<ServerSetting> = Vec::new();
    for c in &menu.controls {
        if c.kind == Kind::Text || rate == Some(c.id.as_str()) || plan.contains(&c.id.as_str()) {
            continue;
        }
        let key = if c.kind == Kind::Radio {
            format!("server.##{}", c.hidden())
        } else {
            format!("server.{}", c.id)
        };
        if out.iter().any(|s| s.decl.field.key == key) {
            continue;
        }
        let label = if c.kind == Kind::Radio {
            name_from_id(&c.id)
        } else {
            c.label.clone()
        };
        let named = |entries: Vec<String>| -> Vec<ChoiceDecl> {
            entries
                .into_iter()
                .map(|e| ChoiceDecl {
                    key: Cow::Owned(e.clone()),
                    label: Cow::Owned(e),
                })
                .collect()
        };
        let (target, decl) = match c.kind {
            Kind::List if !c.entries.is_empty() => {
                let at = c.selected.clamp(0, c.entries.len() as i32 - 1) as u16;
                let field = FieldDecl::with_choices(key, label, named(c.entries.clone()));
                (
                    Target::List(c.id.clone()),
                    SettingDecl::new(field, Value::Choice(at)),
                )
            }
            Kind::CheckBox => (
                Target::CheckBox(c.id.clone()),
                SettingDecl::new(
                    FieldDecl::new(key, label, "", ValueKind::Bool),
                    Value::Bool(c.on),
                ),
            ),
            Kind::Radio => {
                let group: Vec<&Control> = menu
                    .controls
                    .iter()
                    .filter(|o| o.kind == Kind::Radio && o.hidden() == c.hidden())
                    .collect();
                let at = group.iter().position(|g| g.on).unwrap_or(0) as u16;
                let names = group.iter().map(|g| g.visible().to_string()).collect();
                let field = FieldDecl::with_choices(key, label, named(names));
                (
                    Target::Radio(group.iter().map(|g| g.id.clone()).collect()),
                    SettingDecl::new(field, Value::Choice(at)),
                )
            }
            Kind::SliderFloat => {
                let (lo, hi) = (c.min.min(c.max), c.min.max(c.max));
                let field = FieldDecl::new(key, label, "", ValueKind::F64);
                (
                    Target::Number(c.id.clone(), c.kind),
                    SettingDecl::new(field, Value::F64(six_digits(c.value.clamp(lo, hi))))
                        .with_range(
                            lo,
                            hi,
                            if c.step > 0.0 {
                                c.step
                            } else {
                                (hi - lo) / 100.0
                            },
                        ),
                )
            }
            Kind::SliderInt | Kind::InputInt => {
                let (lo, hi) = if c.kind == Kind::SliderInt {
                    (c.min.min(c.max), c.min.max(c.max))
                } else {
                    (f64::from(i32::MIN), f64::from(i32::MAX))
                };
                let field = FieldDecl::new(key, label, "", ValueKind::I64);
                (
                    Target::Number(c.id.clone(), c.kind),
                    SettingDecl::new(field, Value::I64(c.value.clamp(lo, hi).round() as i64))
                        .with_range(lo, hi, c.step.max(1.0)),
                )
            }
            Kind::List | Kind::Text => continue,
        };
        out.push(ServerSetting {
            target,
            decl: decl.with_section("Server").with_help(
                "A control of the SDR++ server. SDR++ keeps the value after this client disconnects.",
            ),
        });
    }
    out
}

#[cfg(test)]
mod tests;
