//! Rules and their actions: the text that the user sets, and the forms resolved against the event kinds that run on each
//! event.

use sdrkit_svc_events::{split_list, FamilySet, KindSet, Kinds, Observation, Template};
use sdrkit_util_geo::time::{split_f64, Civil};
use serde::{Deserialize, Serialize};

use crate::error::{AutomationError, TextError};
use crate::events::RecordWhat;
use crate::expr::Expr;

sdrkit_core::choice_enum! {
    /// What an action does.
    pub enum ActionKind {
        /// Asks the host to show a notification.
        Notify = ("notify", "Notify"),
        /// Writes a row in the log table.
        Log = ("log", "Log"),
        /// Asks the engine to tune.
        Tune = ("tune", "Tune"),
        /// Asks the engine to record the receiver of the trigger event.
        Record = ("record", "Record"),
        /// Asks the engine to save the rewind buffer.
        Rewind = ("rewind", "Save the rewind"),
        /// Asks the host to run a script in its sandbox.
        Script = ("script", "Run a script"),
        /// Asks the engine to send the trigger event to one output.
        Output = ("output", "Send to an output"),
    }
}

/// The local date and time of `unix_s`, `offset_min` minutes ahead of UTC. `None` if `unix_s` is out of range.
pub(crate) fn civil(unix_s: f64, offset_min: i32) -> Option<Civil> {
    let (secs, _) = split_f64(unix_s).ok()?;
    Civil::from_unix_local(secs, 0, offset_min).ok()
}

/// Minutes since midnight of `"HH:MM"`. `at` is the byte position of `text` in its argument.
pub(crate) fn hhmm(text: &str, at: usize) -> Result<u16, TextError> {
    let t = text.trim();
    let err = TextError::Time {
        at: at + text.len() - text.trim_start().len(),
    };
    let (h, m) = t.split_once(':').ok_or(err.clone())?;
    let (h, m): (u16, u16) = (
        h.parse().map_err(|_| err.clone())?,
        m.parse().map_err(|_| err.clone())?,
    );
    if h < 24 && m < 60 {
        Ok(h * 60 + m)
    } else {
        Err(err)
    }
}

/// The days of a list such as `"0, 5, 6"` (Monday 0 to Sunday 6) as bits. 0 for empty text (every day).
pub(crate) fn days(text: &str) -> Result<u8, TextError> {
    let mut mask = 0u8;
    for (at, d) in split_list(text) {
        match d.parse::<u8>() {
            Ok(n) if n < 7 => mask |= 1 << n,
            _ => return Err(TextError::Day { at }),
        }
    }
    Ok(mask)
}

/// Days and hours when something may run, in the host's local time. The hours can run over midnight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Window {
    /// Days as bits, Monday bit 0. 0 is every day.
    pub days: u8,
    /// Start, minutes since midnight.
    pub from: u16,
    /// End, minutes since midnight, exclusive. Equal to `from` is the whole day.
    pub to: u16,
}

impl Window {
    /// The window of the texts `days`, `from` and `to`. `None` if all three are empty.
    pub fn parse(
        days_text: &str,
        from: &str,
        to: &str,
    ) -> Result<Option<Window>, (&'static str, TextError)> {
        if days_text.trim().is_empty() && from.trim().is_empty() && to.trim().is_empty() {
            return Ok(None);
        }
        let time = |t: &str, key: &'static str| {
            if t.trim().is_empty() {
                Ok(0)
            } else {
                hhmm(t, 0).map_err(|e| (key, e))
            }
        };
        Ok(Some(Window {
            days: days(days_text).map_err(|e| ("days", e))?,
            from: time(from, "from")?,
            to: time(to, "to")?,
        }))
    }

    fn day_ok(&self, weekday: u8) -> bool {
        self.days == 0 || self.days & (1 << weekday) != 0
    }

    /// True if `unix_s` is inside the window, with the host's offset from UTC, minutes.
    pub fn contains(&self, unix_s: f64, offset_min: i32) -> bool {
        let Some(c) = civil(unix_s, offset_min) else {
            return false;
        };
        let now = u16::from(c.hour) * 60 + u16::from(c.minute);
        let today = c.weekday.days_from_monday();
        if self.from == self.to {
            return self.day_ok(today);
        }
        if self.from < self.to {
            return self.day_ok(today) && now >= self.from && now < self.to;
        }
        (now >= self.from && self.day_ok(today)) || (now < self.to && self.day_ok((today + 6) % 7))
    }
}

/// A rule as the user set it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct RuleText {
    pub name: String,
    pub enabled: bool,
    pub kinds: String,
    pub families: String,
    pub urgent_only: bool,
    pub condition: String,
    pub days: String,
    pub from: String,
    pub to: String,
    pub fence: String,
    pub inside: bool,
    pub min_km: f64,
    pub max_km: f64,
    pub cooldown_s: f64,
    pub per_identity: bool,
    pub max_per_hour: i64,
}

/// A rule resolved against the event kinds.
#[derive(Clone, Debug)]
pub(crate) struct Rule {
    pub text: RuleText,
    pub kinds: Option<KindSet>,
    pub families: FamilySet,
    pub condition: Expr,
    pub window: Option<Window>,
}

fn text_err(arg: &'static str) -> impl Fn(TextError) -> AutomationError {
    move |error| AutomationError::Text { arg, error }
}

impl Rule {
    /// The rule of `text`.
    ///
    /// # Errors
    ///
    /// [`AutomationError::Text`] with the argument and the position of the first name or value that does not resolve.
    pub fn resolve(text: RuleText, k: &Kinds) -> Result<Rule, AutomationError> {
        let kinds = k
            .kind_list(&text.kinds, 0)
            .map_err(|e| text_err("kinds")(e.into()))?;
        let families =
            Kinds::family_list(&text.families, 0).map_err(|e| text_err("families")(e.into()))?;
        let condition = Expr::parse(&text.condition, k, kinds.as_ref())
            .map_err(|e| text_err("condition")(e.into()))?;
        let window = Window::parse(&text.days, &text.from, &text.to)
            .map_err(|(arg, error)| AutomationError::Text { arg, error })?;
        Ok(Rule {
            text,
            kinds,
            families,
            condition,
            window,
        })
    }

    /// True if the trigger (kinds, families, urgency) takes `o`.
    pub fn triggers(&self, o: &Observation<'_>) -> bool {
        self.text.enabled
            && self.kinds.as_ref().is_none_or(|k| k.contains(o.kind))
            && self.families.admits(o.family)
            && (!self.text.urgent_only || o.is_urgent())
    }

    /// True if the window and the distance limits let `o` through.
    pub fn admits(&self, o: &Observation<'_>, offset_min: i32) -> bool {
        if self
            .window
            .is_some_and(|w| !w.contains(o.unix_time, offset_min))
        {
            return false;
        }
        let (min, max) = (self.text.min_km, self.text.max_km);
        if min > 0.0 || max > 0.0 {
            let Some(d) = o.distance_km else { return false };
            if (min > 0.0 && d < min) || (max > 0.0 && d > max) {
                return false;
            }
        }
        true
    }
}

/// An action as the user set it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ActionText {
    pub rule: String,
    pub action: String,
    pub title: String,
    pub text: String,
    pub urgent: bool,
    pub freq: String,
    pub stack: String,
    pub what: String,
    pub seconds: f64,
    pub max_bytes: i64,
    pub script: String,
    pub args: String,
    pub output: String,
}

/// An action resolved against the kinds of its rule.
#[derive(Clone, Debug)]
pub(crate) struct Action {
    pub text: ActionText,
    pub kind: ActionKind,
    pub what: RecordWhat,
    pub title: Template,
    pub body: Template,
    pub freq: Template,
    pub args: Template,
}

impl Action {
    /// The action of `text`, for a rule that reads the kinds `within`.
    ///
    /// # Errors
    ///
    /// [`AutomationError::Text`] if a template names a field that the kinds do not declare.
    pub fn resolve(
        text: ActionText,
        kind: ActionKind,
        what: RecordWhat,
        k: &Kinds,
        within: Option<&KindSet>,
    ) -> Result<Action, AutomationError> {
        let t = |src: &str, arg: &'static str| {
            Template::parse(src, 0, k, within).map_err(|e| text_err(arg)(e.into()))
        };
        Ok(Action {
            title: t(&text.title, "title")?,
            body: t(&text.text, "text")?,
            freq: t(&text.freq, "freq")?,
            args: t(&text.args, "args")?,
            kind,
            what,
            text,
        })
    }
}
