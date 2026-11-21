//! Schedules: timed triggers. Each due moment produces a `schedule.due` event, which rules take like any other event.

use sdrkit_svc_events::split_list;
use serde::{Deserialize, Serialize};

use crate::error::{AutomationError, TextError};
use crate::rule::{civil, days, hhmm, Window};

/// The most due moments that one call produces, so that a jump of the clock cannot flood the rules.
pub(crate) const MAX_DUE: usize = 1000;

/// A schedule as the user set it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct ScheduleText {
    pub name: String,
    pub enabled: bool,
    pub every_s: f64,
    pub times: String,
    pub days: String,
    pub from: String,
    pub to: String,
}

/// A schedule, resolved.
#[derive(Clone, Debug)]
pub(crate) struct Schedule {
    pub text: ScheduleText,
    /// Minutes since midnight, sorted. Empty for an interval schedule.
    times: Vec<u16>,
    days: u8,
    window: Option<Window>,
}

impl Schedule {
    /// The schedule of `text`: every `every_s` seconds on whole multiples since 1970 if it is above 0, else at the `times`
    /// of day on the `days`.
    ///
    /// # Errors
    ///
    /// [`AutomationError::Text`] if a time or a day does not parse.
    pub fn resolve(text: ScheduleText) -> Result<Schedule, AutomationError> {
        let err = |arg| move |error: TextError| AutomationError::Text { arg, error };
        let mut times = split_list(&text.times)
            .map(|(at, t)| hhmm(t, at))
            .collect::<Result<Vec<u16>, _>>()
            .map_err(err("times"))?;
        times.sort_unstable();
        times.dedup();
        let window = Window::parse("", &text.from, &text.to)
            .map_err(|(arg, error)| AutomationError::Text { arg, error })?;
        Ok(Schedule {
            days: days(&text.days).map_err(err("days"))?,
            times,
            window,
            text,
        })
    }

    /// Adds the due moments in `(after, upto]`, Unix seconds, to `out`, with the host's offset from UTC, minutes.
    pub fn due(&self, after: f64, upto: f64, offset_min: i32, out: &mut Vec<f64>) {
        if !self.text.enabled || upto <= after {
            return;
        }
        let start = out.len();
        if self.text.every_s > 0.0 {
            let p = self.text.every_s.max(1.0);
            let mut k = (after / p).floor() + 1.0;
            while k * p <= upto && out.len() - start < MAX_DUE {
                out.push(k * p);
                k += 1.0;
            }
        } else if let (false, Some(first)) = (self.times.is_empty(), civil(after, offset_min)) {
            let off = f64::from(offset_min) * 60.0;
            let day0 = ((after + off) / 86_400.0).floor();
            let mut weekday = first.weekday.days_from_monday();
            let mut d = day0;
            while d * 86_400.0 - off <= upto && d <= day0 + 1000.0 && out.len() - start < MAX_DUE {
                if self.days == 0 || self.days & (1 << weekday) != 0 {
                    for m in &self.times {
                        let t = d * 86_400.0 + f64::from(*m) * 60.0 - off;
                        if t > after && t <= upto && out.len() - start < MAX_DUE {
                            out.push(t);
                        }
                    }
                }
                d += 1.0;
                weekday = (weekday + 1) % 7;
            }
        }
        if let Some(w) = &self.window {
            let mut i = start;
            while i < out.len() {
                if w.contains(out[i], offset_min) {
                    i += 1;
                } else {
                    out.remove(i);
                }
            }
        }
    }
}
