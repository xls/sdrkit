//! Test hooks: what a stack supplies so a host (a console command, a test, a phone) can test it without knowing the
//! stack. A stack sets them as the [`VerifyHooks`] in the `verify` member of its [`crate::StackDescriptor`].

use crate::command::Command;
use crate::event::Event;
use crate::publish::{Declare, Field};
use crate::record::{FieldDecl, Kind};
use crate::results::Results;
use crate::settings::{Setting, SettingError, SettingKind, Values};
use crate::value::{ChoiceDecl, Value, ValueKind};
use crate::Hz;
use serde::Serialize;
use serde_json::Value as Json;
use std::path::Path;

/// How one check ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
    /// Could not be checked (described in the detail).
    NotChecked,
}

/// One check a hook made.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CheckLine {
    /// What was compared: for a vector, the key's path in `expected.json` (`"rds.ps"`). For a case, a short name.
    pub key: String,
    pub outcome: Outcome,
    /// What was found, and what was expected.
    pub detail: String,
}

impl CheckLine {
    /// A check that passed when `ok`, failed otherwise.
    pub fn new(key: impl Into<String>, ok: bool, detail: impl Into<String>) -> Self {
        CheckLine {
            key: key.into(),
            outcome: if ok { Outcome::Pass } else { Outcome::Fail },
            detail: detail.into(),
        }
    }
    /// A key the hook recognises but cannot check (a timing the recording does not contain, for example).
    pub fn not_checked(key: impl Into<String>, why: impl Into<String>) -> Self {
        CheckLine {
            key: key.into(),
            outcome: Outcome::NotChecked,
            detail: why.into(),
        }
    }
}

/// The options of a verification run. A stack's cases follow them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VerifyOptions {
    /// How long a test-signal run lasts, seconds (a case can need longer).
    pub seconds: f64,
    /// No impairments: no offset, no noise.
    pub clean: bool,
    /// The SNR of the added noise, dB in the stack's bandwidth.
    pub snr_db: f64,
}

/// One setting of a case or a vector: the key of a setting of the stack's schema and its value.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseSetting {
    /// The key of the setting.
    pub key: &'static str,
    /// The value.
    pub value: Value,
}

impl CaseSetting {
    /// Setting `s` at `v`.
    pub fn new<K: SettingKind>(s: &Setting<K>, v: K::In<'_>) -> Self {
        CaseSetting {
            key: s.field.key,
            value: value_of(&s.field, v),
        }
    }

    /// Sets each of `list` in `values`, in order.
    ///
    /// # Errors
    ///
    /// [`SettingError::Unknown`] if the schema has no setting of a key, or what [`Values::set`] returns. The settings before
    /// the refused one stay set.
    pub fn apply_all(list: &[CaseSetting], values: &mut Values) -> Result<(), SettingError> {
        for c in list {
            let id = values.schema().id(c.key)?;
            values.set(id, &c.value)?;
        }
        Ok(())
    }
}

/// A command that a case runs on the stack after `at_s` seconds of signal (select a service, for example).
#[derive(Clone, Debug, PartialEq)]
pub struct CaseCommand {
    /// Seconds of signal fed before the command runs.
    pub at_s: f64,
    /// The key of the command.
    pub key: &'static str,
    /// The arguments, by key.
    pub args: Vec<CaseSetting>,
}

impl CaseCommand {
    /// Command `c` after `at_s` seconds, with no arguments.
    pub fn new(at_s: f64, c: &Command) -> Self {
        CaseCommand {
            at_s,
            key: c.key,
            args: Vec::new(),
        }
    }
    /// The same command with argument `f` at `v`.
    pub fn arg<K: Kind>(mut self, f: &Field<K>, v: K::In<'_>) -> Self {
        self.args.push(CaseSetting {
            key: f.key,
            value: value_of(f, v),
        });
        self
    }
}

fn value_of<K: Kind>(_: &Field<K>, v: K::In<'_>) -> Value {
    let mut slot = Value::None;
    K::store(&mut slot, v);
    slot
}

/// One test-signal run: the stack's generator through the engine, then comparisons.
#[derive(Clone, Debug)]
pub struct VerifyCase {
    /// Its name in the report ("test signal", "test signal, mono" ...).
    pub name: String,
    pub submode: Option<String>,
    /// The stack's settings for the receiver and the generator (its `gen_*` settings), in order.
    pub settings: Vec<CaseSetting>,
    /// The commands that run during the case, in order of time.
    pub commands: Vec<CaseCommand>,
    /// Where the receiver is tuned (the station is placed there). `None` for the stack's default frequency.
    pub freq_hz: Option<Hz>,
    /// The station's carrier is this far from where the receiver is tuned, Hz.
    pub offset_hz: Hz,
    /// White noise at this SNR in the stack's bandwidth, dB. `None` for no noise.
    pub snr_db: Option<f64>,
    /// How long the signal runs, seconds.
    pub seconds: f64,
    /// The receiver must reach tracking or locked.
    pub expect_lock: bool,
    /// The receiver must make sound (`Some(true)`), must not (`Some(false)`), or either (`None`).
    pub expect_audio: Option<bool>,
    /// Comparisons of the results at the end of the run.
    pub check: Option<fn(&Results) -> Vec<CheckLine>>,
}

impl VerifyCase {
    /// A case at the stack's defaults, with the options' impairments (`offset_hz` and the options' SNR unless clean), that must
    /// lock. Change its fields to make other cases.
    pub fn new(name: impl Into<String>, o: &VerifyOptions, offset_hz: Hz) -> Self {
        VerifyCase {
            name: name.into(),
            submode: None,
            settings: Vec::new(),
            commands: Vec::new(),
            freq_hz: None,
            offset_hz: if o.clean { 0.0 } else { offset_hz },
            snr_db: (!o.clean).then_some(o.snr_db),
            seconds: o.seconds,
            expect_lock: true,
            expect_audio: None,
            check: None,
        }
    }
    /// The same case with setting `s` at `v`.
    pub fn with<K: SettingKind>(mut self, s: &Setting<K>, v: K::In<'_>) -> Self {
        self.settings.push(CaseSetting::new(s, v));
        self
    }
    /// The same case with command `c`.
    pub fn run(mut self, c: CaseCommand) -> Self {
        self.commands.push(c);
        self
    }
}

/// A recorded test vector after it went through the receiver: what [`VerifyHooks::check_vector`] compares.
#[derive(Clone, Copy, Debug)]
pub struct VectorRun<'a> {
    /// The recording's name (its file name without `.sigmf-meta`).
    pub name: &'a str,
    /// Its section of `expected.json`.
    pub section: &'a Json,
    /// The whole of `expected.json` (shared tolerances, notes).
    pub expected: &'a Json,
    /// The receiver's results at the end.
    pub results: &'a Results,
    /// The results after each piece of the recording fed (pieces of 0.1 s): (seconds of signal fed, results).
    pub reports: &'a [(f64, Results)],
    /// Every sound frame the receiver made, interleaved.
    pub audio: &'a [f32],
    pub audio_rate_hz: Hz,
    /// The samples per frame of `audio`.
    pub audio_channels: u16,
    /// Every event the receiver produced.
    pub events: &'a [Event],
    /// Seconds of signal in the recording.
    pub seconds: f64,
    /// The folder holding the recording and `expected.json` (for other files the expectation names).
    pub dir: &'a Path,
}

impl VectorRun<'_> {
    /// Compares the expected value at `path` of the section with result field `f`, with the key's tolerance. The expected
    /// value is read as the kind of `f`. `None` if the section has no value at `path`.
    pub fn check_field(&self, path: &str, f: &dyn Declare) -> Option<CheckLine> {
        let decl = f.decl();
        let actual = self
            .results
            .decl()
            .field_id(f.key())
            .and_then(|id| self.results.fields().value(id))
            .filter(|v| !v.is_none());
        self.check(path, &decl, actual)
    }

    /// Compares the expected value at `path` of the section with `actual`, a value that the stack computed from its results.
    /// The expected value is read as the kind of `actual`. `None` if the section has no value at `path`.
    pub fn check_value(&self, path: &str, actual: &Value) -> Option<CheckLine> {
        let kind = actual.kind()?;
        let decl = FieldDecl::new(path.to_owned(), path.to_owned(), "", kind);
        self.check(path, &decl, Some(actual))
    }

    fn check(&self, path: &str, decl: &FieldDecl, actual: Option<&Value>) -> Option<CheckLine> {
        let exp = json_at(self.section, path)?;
        let tol = Tolerance::for_key(self.section, self.expected, path);
        Some(check_against(path, exp, decl, actual, tol.as_ref()))
    }
}

/// What a stack supplies so a host can test it.
#[derive(Clone, Copy, Debug)]
pub struct VerifyHooks {
    /// The test-signal runs (they replace the host's generic one). An empty list keeps the generic one.
    pub cases: fn(&VerifyOptions) -> Vec<VerifyCase>,
    /// The receiver's settings for a recorded vector, from the recording's name, its section of `expected.json` and the
    /// whole file. Without it, the section's `settings` that the stack recognises are used.
    pub vector_settings: Option<VectorSettings>,
    /// The stack's comparison of a recorded vector. The keys it returns are its own (a key that names an object covers
    /// everything under it). The host compares the other keys of the section by name.
    pub check_vector: Option<CheckVector>,
}

/// The receiver's settings for a recorded vector: (recording name, its section, the whole file) to settings.
pub type VectorSettings = fn(&str, &Json, &Json) -> Vec<CaseSetting>;

/// A stack's comparison of a recorded vector.
pub type CheckVector = fn(&VectorRun) -> Vec<CheckLine>;

/// A tolerance from `expected.json`: `[low, high]`, a number for plus or minus, `"at least N"` (or `"above N"`, `">= N"`),
/// `"at most N"` (`"below N"`, `"<= N"`), `"within N"` (`"+-N"`), `"exact"`. Other text is a note for people.
#[derive(Clone, Debug, PartialEq)]
pub enum Tolerance {
    Range(f64, f64),
    PlusMinus(f64),
    AtLeast(f64),
    AtMost(f64),
    Exact,
    /// A tolerance written for people, which a program cannot apply. The value is shown, not compared.
    ForPeople(String),
}

impl Tolerance {
    /// Reads one.
    pub fn parse(v: &Json) -> Tolerance {
        match v {
            Json::Array(a) if a.len() == 2 && a.iter().all(|x| x.is_number()) => {
                Tolerance::Range(a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0))
            }
            Json::Number(n) => Tolerance::PlusMinus(n.as_f64().unwrap_or(0.0).abs()),
            Json::String(s) => {
                let l = s.trim().to_ascii_lowercase();
                let after = |p: &str| {
                    l.strip_prefix(p)
                        .and_then(|r| json_number(&Json::String(r.trim_start().into())))
                };
                if l == "exact" || l.starts_with("exact ") {
                    Tolerance::Exact
                } else if let Some(n) = after("at least ")
                    .or_else(|| after("above "))
                    .or_else(|| after(">="))
                {
                    Tolerance::AtLeast(n)
                } else if let Some(n) = after("at most ")
                    .or_else(|| after("below "))
                    .or_else(|| after("<="))
                {
                    Tolerance::AtMost(n)
                } else if let Some(n) = after("within +-")
                    .or_else(|| after("within ±"))
                    .or_else(|| after("+-"))
                    .or_else(|| after("±"))
                    .or_else(|| after("within "))
                {
                    Tolerance::PlusMinus(n)
                } else {
                    Tolerance::ForPeople(s.clone())
                }
            }
            other => Tolerance::ForPeople(other.to_string()),
        }
    }

    /// The tolerance for the key at `path` (`"signal.pilot_pct"`): from the section's `tolerance`, else the file's, by the key's last
    /// name or its whole path, compared without `_`, `-`, spaces or case.
    pub fn for_key(section: &Json, expected: &Json, path: &str) -> Option<Tolerance> {
        let last = path.rsplit('.').next().unwrap_or(path);
        let (nl, np) = (norm(last), norm(path));
        [section.get("tolerance"), expected.get("tolerance")]
            .into_iter()
            .flatten()
            .filter_map(|t| t.as_object())
            .find_map(|o| {
                o.iter()
                    .find(|(k, _)| norm(k) == nl || norm(k) == np)
                    .map(|(_, v)| Tolerance::parse(v))
            })
    }

    /// As words for a report: `" in [7, 11]"`, `" at least 40"`; empty for none.
    pub fn describe(&self) -> String {
        match self {
            Tolerance::Range(a, b) => format!(" in [{}, {}]", plain(*a), plain(*b)),
            Tolerance::PlusMinus(d) => format!(" +- {}", plain(*d)),
            Tolerance::AtLeast(x) => format!(" at least {}", plain(*x)),
            Tolerance::AtMost(x) => format!(" at most {}", plain(*x)),
            Tolerance::Exact | Tolerance::ForPeople(_) => String::new(),
        }
    }
}

/// Lower case without `_`, `-` or spaces: `snr_db`, `snrDb` and `SNR dB` are the same name.
pub fn norm(k: &str) -> String {
    k.chars()
        .filter(|c| !matches!(c, '_' | '-' | ' '))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// A number as short text (no trailing zeros).
pub fn plain(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// The expected value `exp` of `expected.json` read as a value of field `decl`: a number, or text that starts with a number
/// (`"9.0 %"`) for a number field, and hexadecimal text for a whole number (`"4A21"`). `true`, `false`, "yes", "on", "1"
/// and their opposites for a yes/no field. Text for a text field (a number as its short text). The key or the label of a
/// choice. A list of flag keys or labels for a flag set. `{"lat", "lon"}` for a position. `None` if `exp` does not read
/// as that kind.
pub fn expected_value(exp: &Json, decl: &FieldDecl) -> Option<Value> {
    let number = || json_number(exp);
    Some(match decl.kind {
        ValueKind::Bool => Value::Bool(match exp {
            Json::Bool(b) => *b,
            Json::Number(n) => n.as_f64()? != 0.0,
            Json::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                "true" | "yes" | "on" | "1" => true,
                "false" | "no" | "off" | "0" => false,
                _ => return None,
            },
            _ => return None,
        }),
        ValueKind::I64 => Value::I64(match exp {
            Json::String(s) => i64::from_str_radix(s.trim(), 16).ok()?,
            _ => number()? as i64,
        }),
        ValueKind::F64 => Value::F64(number()?),
        ValueKind::Hz => Value::Hz(number()?),
        ValueKind::Time => Value::Time(number()?),
        ValueKind::Duration => Value::Duration(number()?),
        ValueKind::Text => Value::Text(match exp {
            Json::String(s) => s.clone(),
            Json::Number(n) => plain(n.as_f64()?),
            Json::Bool(b) => b.to_string(),
            _ => return None,
        }),
        ValueKind::Choice => {
            let s = exp.as_str()?.trim();
            let i = decl
                .choices
                .iter()
                .position(|c| c.key == s || c.label == s)?;
            Value::Choice(u16::try_from(i).ok()?)
        }
        ValueKind::Flags => {
            let mut mask = 0;
            for e in exp.as_array()? {
                let s = e.as_str()?.trim();
                mask |= 1 << decl.flags.iter().position(|f| f.key == s || f.label == s)?;
            }
            Value::Flags(mask)
        }
        ValueKind::Position => Value::Position {
            lat: exp.get("lat")?.as_f64()?,
            lon: exp.get("lon")?.as_f64()?,
        },
        ValueKind::Bytes | ValueKind::Any => return None,
    })
}

/// Whether an actual value meets an expected one of the same kind. Without a tolerance, whole numbers must be equal and
/// other numbers within 5 %. Text is compared as it is and with spaces trimmed. Values of two kinds never match.
pub fn matches(exp: &Value, act: &Value, tol: Option<&Tolerance>) -> bool {
    match (exp, act) {
        (Value::Text(e), Value::Text(a)) => e == a || e.trim() == a.trim(),
        (Value::I64(e), Value::I64(a)) => match tol {
            None | Some(Tolerance::ForPeople(_)) => e == a,
            Some(t) => numbers_match(*e as f64, *a as f64, Some(t)),
        },
        (Value::F64(e), Value::F64(a))
        | (Value::Hz(e), Value::Hz(a))
        | (Value::Time(e), Value::Time(a))
        | (Value::Duration(e), Value::Duration(a)) => numbers_match(*e, *a, tol),
        _ => exp == act,
    }
}

fn numbers_match(e: f64, a: f64, tol: Option<&Tolerance>) -> bool {
    match tol {
        Some(Tolerance::Range(lo, hi)) => a >= *lo && a <= *hi,
        Some(Tolerance::PlusMinus(d)) => (a - e).abs() <= *d,
        Some(Tolerance::AtLeast(x)) => a >= *x,
        Some(Tolerance::AtMost(x)) => a <= *x,
        Some(Tolerance::Exact) => a == e,
        _ if e == e.trunc() && a == a.trunc() => a == e,
        _ => (a - e).abs() <= (0.05 * e.abs()).max(1e-9),
    }
}

/// A value as short text for a report, a choice by its label.
pub fn short(v: &Value, choices: &[ChoiceDecl]) -> String {
    let s = match v {
        Value::Text(t) => format!("{t:?}"),
        Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => {
            plain(if x.abs() >= 100.0 {
                (x * 10.0).round() / 10.0
            } else {
                (x * 1000.0).round() / 1000.0
            })
        }
        other => other.display(choices).to_string(),
    };
    if s.chars().count() > 40 {
        format!("{}...", s.chars().take(37).collect::<String>())
    } else {
        s
    }
}

/// Compares the expected value `exp` (read as a value of field `decl`) with `actual` (`None`: the receiver has no value)
/// with `tol`. A tolerance in words makes it "not checked".
pub fn check_against(
    path: &str,
    exp: &Json,
    decl: &FieldDecl,
    actual: Option<&Value>,
    tol: Option<&Tolerance>,
) -> CheckLine {
    if let Some(Tolerance::ForPeople(w)) = tol {
        return CheckLine::not_checked(path, format!("tolerance in words: {w}"));
    }
    let Some(e) = expected_value(exp, decl) else {
        return CheckLine::not_checked(path, format!("{exp} is not a value of this field"));
    };
    let c = &decl.choices;
    let Some(act) = actual else {
        return CheckLine::new(path, false, format!("no value, expected {}", short(&e, c)));
    };
    let t = tol.map(Tolerance::describe).unwrap_or_default();
    if matches(&e, act, tol) {
        CheckLine::new(
            path,
            true,
            format!("{}, expected {}{t}", short(act, c), short(&e, c)),
        )
    } else {
        CheckLine::new(
            path,
            false,
            format!("got {}, expected {}{t}", short(act, c), short(&e, c)),
        )
    }
}

/// The amplitude of a tone at `hz` in `x` (one channel at `rate`): a single-bin DFT, peak amplitude (a full-scale sine is 1.0).
pub fn tone_amplitude(x: &[f32], rate: f64, hz: f64) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let w = 2.0 * std::f64::consts::PI * hz / rate;
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for (n, &v) in x.iter().enumerate() {
        let p = w * n as f64;
        re += v as f64 * p.cos();
        im -= v as f64 * p.sin();
    }
    2.0 * (re * re + im * im).sqrt() / x.len() as f64
}

/// Channel `ch` (0 left, 1 right) of interleaved audio of `channels` channels.
pub fn channel(audio: &[f32], channels: u16, ch: usize) -> Vec<f32> {
    audio
        .iter()
        .skip(ch)
        .step_by(usize::from(channels.max(1)))
        .copied()
        .collect()
}

/// A value at a path of a JSON value: `"detail.station"`, `"fields.pi"`; array elements by index (`"points.0.lat"`).
pub fn json_at<'a>(v: &'a Json, path: &str) -> Option<&'a Json> {
    path.split('.').try_fold(v, |v, k| match v {
        Json::Object(o) => o.get(k),
        Json::Array(a) => k.parse::<usize>().ok().and_then(|i| a.get(i)),
        _ => None,
    })
}

/// A number from a JSON value: a number, a bool (1 or 0), or text starting with a number ("9.0 %").
pub fn json_number(v: &Json) -> Option<f64> {
    match v {
        Json::Number(n) => n.as_f64(),
        Json::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Json::String(s) => {
            let head: String = s
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
                .collect();
            (1..=head.len()).rev().find_map(|k| head[..k].parse().ok())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::{field, Published};
    use crate::settings::SettingSet;
    use serde_json::json;

    crate::choice_enum! {
        enum Mode {
            Mono = ("mono", "Mono"),
            Stereo = ("stereo", "Stereo"),
        }
    }

    #[test]
    fn paths_and_numbers() {
        let v = json!({"detail": {"station": "ABC", "list": [1, {"x": 2.5}]}, "fields": {"pilot": "9.0 %"}});
        assert_eq!(json_at(&v, "detail.station"), Some(&json!("ABC")));
        assert_eq!(json_at(&v, "detail.list.1.x"), Some(&json!(2.5)));
        assert_eq!(json_at(&v, "detail.nope"), None);
        assert_eq!(json_number(&v["fields"]["pilot"]), Some(9.0));
        assert_eq!(json_number(&json!(true)), Some(1.0));
    }

    #[test]
    fn tolerances_parse_and_apply() {
        assert_eq!(
            Tolerance::parse(&json!([7, 11])),
            Tolerance::Range(7.0, 11.0)
        );
        assert_eq!(Tolerance::parse(&json!(1.5)), Tolerance::PlusMinus(1.5));
        assert_eq!(
            Tolerance::parse(&json!("at least 38 (11.4 groups/s are sent)")),
            Tolerance::AtLeast(38.0)
        );
        assert_eq!(
            Tolerance::parse(&json!("below 1e-4")),
            Tolerance::AtMost(1e-4)
        );
        assert_eq!(
            Tolerance::parse(&json!("within +-50 (the tone)")),
            Tolerance::PlusMinus(50.0)
        );
        assert_eq!(Tolerance::parse(&json!("exact (88.5)")), Tolerance::Exact);
        assert!(matches!(
            Tolerance::parse(&json!("never open")),
            Tolerance::ForPeople(_)
        ));
        let e = json!({"tolerance": {"pilot_pct": [7, 11]}});
        assert_eq!(
            Tolerance::for_key(&json!({}), &e, "signal.pilot_pct"),
            Some(Tolerance::Range(7.0, 11.0))
        );
        assert!(matches(
            &Value::F64(8.99),
            &Value::F64(9.0),
            Some(&Tolerance::Range(7.0, 11.0))
        ));
        assert!(!matches(&Value::Bool(true), &Value::Bool(false), None));
        assert!(!matches(&Value::I64(3), &Value::F64(3.0), None));
        let tone: Vec<f32> = (0..4800)
            .map(|n| (0.5 * (2.0 * std::f64::consts::PI * 1000.0 * n as f64 / 48e3).sin()) as f32)
            .collect();
        assert!((tone_amplitude(&tone, 48e3, 1000.0) - 0.5).abs() < 1e-3);
        assert_eq!(channel(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3, 1), [2.0, 5.0]);
    }

    #[test]
    fn expected_values_read_as_the_kind_of_the_field() {
        let pi = FieldDecl::new("pi", "PI", "", ValueKind::I64);
        assert_eq!(
            expected_value(&json!("4A21"), &pi),
            Some(Value::I64(0x4A21))
        );
        let pct = FieldDecl::new("pilot", "Pilot", "%", ValueKind::F64);
        assert_eq!(expected_value(&json!("9.0 %"), &pct), Some(Value::F64(9.0)));
        let on = FieldDecl::new("on", "On", "", ValueKind::Bool);
        assert_eq!(expected_value(&json!("yes"), &on), Some(Value::Bool(true)));
        let mode = Field::<field::Choice<Mode>>::new("mode", "Mode", "").decl();
        assert_eq!(
            expected_value(&json!("stereo"), &mode),
            Some(Value::Choice(1))
        );
        assert_eq!(
            expected_value(&json!("Mono"), &mode),
            Some(Value::Choice(0))
        );
        assert_eq!(expected_value(&json!("quad"), &mode), None);
    }

    #[test]
    fn a_vector_checks_fields_and_computed_values() {
        const STATION: Field<field::Text> = Field::new("station", "Station", "");
        const MODE: Field<field::Choice<Mode>> = Field::new("mode", "Mode", "");
        const SNR: Field<field::F64> = Field::new("snr_db", "SNR", "dB");
        let mut p = Published::new(&[&STATION, &MODE, &SNR]);
        p.set(&STATION, "SDRKIT ");
        p.set(&MODE, Mode::Stereo);
        let section = json!({"rds": {"ps": "SDRKIT", "mode": "stereo", "snr": 30, "count": 9},
                             "tolerance": {"snr": "at least 20"}});
        let file = json!({});
        let v = VectorRun {
            name: "x",
            section: &section,
            expected: &file,
            results: p.results(),
            reports: &[],
            audio: &[],
            audio_rate_hz: 48e3,
            audio_channels: 2,
            events: &[],
            seconds: 1.0,
            dir: Path::new("."),
        };
        assert_eq!(
            v.check_field("rds.ps", &STATION).unwrap().outcome,
            Outcome::Pass
        );
        assert_eq!(
            v.check_field("rds.mode", &MODE).unwrap().outcome,
            Outcome::Pass
        );
        let snr = v.check_field("rds.snr", &SNR).unwrap();
        assert_eq!(snr.outcome, Outcome::Fail);
        assert!(snr.detail.starts_with("no value"));
        assert_eq!(
            v.check_value("rds.count", &Value::I64(9)).unwrap().outcome,
            Outcome::Pass
        );
        assert!(v.check_field("rds.none", &STATION).is_none());
    }

    #[test]
    fn a_case_follows_the_options() {
        const GEN_PI: Setting<field::Text> = Setting::new("gen_pi", "PI", "", "");
        const SERVICE: Field<field::I64> = Field::new("service", "Service", "");
        static SELECT: Command = Command::new("select", "Select");
        let o = VerifyOptions {
            seconds: 3.0,
            clean: false,
            snr_db: 30.0,
        };
        let c = VerifyCase::new("x", &o, 500.0)
            .with(&GEN_PI, "4A21")
            .run(CaseCommand::new(1.0, &SELECT).arg(&SERVICE, 7));
        assert_eq!((c.offset_hz, c.snr_db, c.seconds), (500.0, Some(30.0), 3.0));
        assert_eq!(c.settings[0].value, Value::Text("4A21".into()));
        assert_eq!(c.commands[0].args[0].value, Value::I64(7));
        crate::settings! {
            enum GenSetting {
                Pi = GEN_PI,
            }
        }
        let mut values = Values::defaults(&GenSetting::schema());
        CaseSetting::apply_all(&c.settings, &mut values).unwrap();
        assert_eq!(values.get(&GEN_PI), "4A21");
        let unknown = [CaseSetting {
            key: "nope",
            value: Value::Bool(true),
        }];
        assert!(matches!(
            CaseSetting::apply_all(&unknown, &mut values),
            Err(SettingError::Unknown { .. })
        ));
        let c = VerifyCase::new("x", &VerifyOptions { clean: true, ..o }, 500.0);
        assert_eq!((c.offset_hz, c.snr_db), (0.0, None));
        assert_eq!(CheckLine::new("k", false, "").outcome, Outcome::Fail);
    }
}
