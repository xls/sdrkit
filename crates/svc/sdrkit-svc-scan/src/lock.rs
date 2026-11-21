//! The lock test of a channel scan: it judges one channel by the lock profile that the stack declares in its descriptor.

use crate::observe::Observation;
use sdrkit_core::record::FieldId;
use sdrkit_core::{
    LockCondition, LockProfile, LockState, NameSource, OwnerSetting, Results, ResultsDecl, Value,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A channel of a channel scan (a TV channel, a DAB channel, an FM step, an AIS frequency).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Channel {
    /// The name of the channel in its plan (`"21"`, `"12C"`). Empty for none.
    pub name: String,
    /// The centre frequency, Hz.
    pub freq_hz: f64,
    /// The stack id.
    pub stack: String,
    /// The settings to apply before the test (the width of a TV channel).
    pub settings: Vec<OwnerSetting>,
    /// The channel bandwidth, Hz.
    pub bandwidth_hz: Option<f64>,
}

impl Channel {
    /// A channel named `name` at `freq_hz` in `stack`.
    pub fn new(name: &str, freq_hz: f64, stack: &str) -> Channel {
        Channel {
            name: name.into(),
            freq_hz,
            stack: stack.into(),
            ..Channel::default()
        }
    }

    /// Channels on a raster: `start_hz`, `start_hz + step_hz`, and so on up to `stop_hz`. Empty if the step is not above 0
    /// or the stop is below the start.
    pub fn raster(start_hz: f64, stop_hz: f64, step_hz: f64, stack: &str) -> Vec<Channel> {
        if !(step_hz > 0.0 && stop_hz >= start_hz) {
            return Vec::new();
        }
        let n = ((stop_hz - start_hz) / step_hz + 1e-6).floor() as usize + 1;
        (0..n)
            .map(|i| Channel::new("", start_hz + i as f64 * step_hz, stack))
            .collect()
    }
}

sdrkit_core::choice_enum! {
    /// Why a channel scan found nothing on a channel.
    #[derive(Serialize, Deserialize)]
    pub enum EmptyReason {
        /// There was no signal before the quiet time ended.
        NoSignal = ("no_signal", "No signal"),
        /// There was a signal but no lock before the timeout.
        NoLock = ("no_lock", "No lock"),
    }
}

/// What a lock test found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    /// The station, ensemble or multiplex name. Empty if none was read.
    pub name: String,
    /// The best SNR, dB.
    pub snr_db: Option<f32>,
    /// The level, dBFS.
    pub level_dbfs: Option<f32>,
    /// The captured result fields of the profile, by key, typed.
    pub values: Vec<(&'static str, Value)>,
}

/// The answer of a lock test.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Keep listening.
    Pending,
    /// A station.
    Locked(Found),
    /// Nothing usable here.
    Empty(EmptyReason),
}

/// The test of one channel. It is notified when a channel starts, then queried at every tick until it decides.
pub trait LockTest: Send {
    /// The time to wait after the retune before judging, seconds.
    fn settle_s(&self) -> f64;
    /// A new channel is tested.
    fn begin(&mut self, _channel: &Channel) {}
    /// Judges the channel `elapsed_s` seconds after settling.
    fn judge(&mut self, elapsed_s: f64, obs: &Observation, results: Option<&Results>) -> Verdict;
}

/// The rank of a lock state: off, searching, tracking, locked.
pub(crate) fn lock_state_rank(l: LockState) -> u8 {
    match l {
        LockState::Off => 0,
        LockState::Searching => 1,
        LockState::Tracking => 2,
        LockState::Locked => 3,
    }
}

/// The number at the start of `s` (3 for `"3 stations"`).
fn lead_number(s: &str) -> Option<f64> {
    let t = s.trim();
    let end = t
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.')))
        .unwrap_or(t.len());
    t[..end].parse().ok()
}

/// The number that a value produces: a number of any kind, or the number at the start of a text.
pub(crate) fn number(v: &Value) -> Option<f64> {
    match v {
        Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => Some(*x),
        Value::I64(x) => Some(*x as f64),
        Value::Text(t) => lead_number(t),
        _ => None,
    }
}

/// The field ids of a profile in one results declaration: the field keys of the conditions in pre-order, of the names and of
/// the captures.
#[derive(Clone, Debug, Default)]
struct Resolved {
    decl: Option<Arc<ResultsDecl>>,
    conditions: Vec<Option<FieldId>>,
    names: Vec<Option<FieldId>>,
    captures: Vec<Option<FieldId>>,
}

fn condition_keys(c: &LockCondition, out: &mut Vec<Option<&'static str>>) {
    match *c {
        LockCondition::LockState { .. } | LockCondition::Snr { .. } => out.push(None),
        LockCondition::FieldAbove { key, .. } | LockCondition::FieldPresent { key } => {
            out.push(Some(key));
        }
        LockCondition::Any(of) | LockCondition::All(of) => {
            out.push(None);
            for c in of {
                condition_keys(c, out);
            }
        }
    }
}

impl Resolved {
    /// Finds the field ids again if `decl` is not the declaration they were found in.
    fn update(&mut self, p: &LockProfile, decl: &Arc<ResultsDecl>) {
        if self.decl.as_ref().is_some_and(|d| Arc::ptr_eq(d, decl)) {
            return;
        }
        let id = |k: &str| decl.field_id(k);
        let mut keys = Vec::new();
        condition_keys(&p.lock, &mut keys);
        self.conditions = keys.into_iter().map(|k| k.and_then(id)).collect();
        self.names = p
            .name_from
            .iter()
            .map(|s| match s {
                NameSource::Field(k) => id(k),
                NameSource::Summary => None,
            })
            .collect();
        self.captures = p.capture.iter().map(|k| id(k)).collect();
        self.decl = Some(Arc::clone(decl));
    }
}

/// Evaluates `c` and every condition under it, in pre-order, with `ids[*at..]` as their field ids.
fn holds(
    c: &LockCondition,
    obs: &Observation,
    r: Option<&Results>,
    ids: &[Option<FieldId>],
    at: &mut usize,
) -> bool {
    let id = ids.get(*at).copied().flatten();
    *at += 1;
    let field = || r.zip(id).and_then(|(r, id)| r.fields().value(id));
    match *c {
        LockCondition::LockState { at_least } => {
            lock_state_rank(obs.lock_state) >= lock_state_rank(at_least)
        }
        LockCondition::Snr { min_db } => obs.snr().is_some_and(|s| s >= min_db),
        LockCondition::FieldAbove { min, .. } => field().and_then(number).is_some_and(|v| v > min),
        LockCondition::FieldPresent { .. } => field().is_some_and(|v| match v {
            Value::None => false,
            Value::Text(t) => !t.trim().is_empty(),
            _ => true,
        }),
        LockCondition::Any(of) => of
            .iter()
            .fold(false, |any, c| holds(c, obs, r, ids, at) | any),
        LockCondition::All(of) => of
            .iter()
            .fold(true, |all, c| holds(c, obs, r, ids, at) & all),
    }
}

/// A [`LockTest`] that follows a [`LockProfile`]: settle, leave early when there is no signal at all, wait for the lock until
/// the timeout, then collect the name and the captured fields for a while.
#[derive(Clone, Debug)]
pub struct ProfileLockTest {
    profile: LockProfile,
    resolved: Resolved,
    locked_at: Option<f64>,
    best_snr: Option<f32>,
}

impl ProfileLockTest {
    /// A test that follows `profile`.
    pub fn new(profile: LockProfile) -> ProfileLockTest {
        ProfileLockTest {
            profile,
            resolved: Resolved::default(),
            locked_at: None,
            best_snr: None,
        }
    }

    fn name<'r>(&self, r: &'r Results) -> &'r str {
        for (s, id) in self.profile.name_from.iter().zip(&self.resolved.names) {
            let text = match (s, id) {
                (NameSource::Summary, _) => r.status().summary.trim(),
                (NameSource::Field(_), Some(id)) => match r.fields().value(*id) {
                    Some(Value::Text(t)) => t.trim(),
                    _ => "",
                },
                (NameSource::Field(_), None) => "",
            };
            if !text.is_empty() {
                return text;
            }
        }
        ""
    }

    fn found(&self, obs: &Observation, r: Option<&Results>) -> Found {
        let mut f = Found {
            snr_db: self.best_snr,
            level_dbfs: obs.level_dbfs,
            ..Found::default()
        };
        if let Some(r) = r {
            f.name = self.name(r).to_owned();
            for (key, id) in self.profile.capture.iter().zip(&self.resolved.captures) {
                if let Some(v) = id
                    .and_then(|id| r.fields().value(id))
                    .filter(|v| !v.is_none())
                {
                    f.values.push((key, v.clone()));
                }
            }
        }
        f
    }
}

impl LockTest for ProfileLockTest {
    fn settle_s(&self) -> f64 {
        self.profile.settle_s
    }

    fn begin(&mut self, _channel: &Channel) {
        self.locked_at = None;
        self.best_snr = None;
    }

    fn judge(&mut self, elapsed_s: f64, obs: &Observation, r: Option<&Results>) -> Verdict {
        if let Some(r) = r {
            self.resolved.update(&self.profile, r.decl());
        }
        if let Some(s) = obs.snr() {
            self.best_snr = Some(self.best_snr.map_or(s, |b| b.max(s)));
        }
        let p = &self.profile;
        if self.locked_at.is_none() {
            if holds(&p.lock, obs, r, &self.resolved.conditions, &mut 0) {
                self.locked_at = Some(elapsed_s);
            } else {
                let signal = self.best_snr.is_some_and(|s| s >= p.signal_snr_db)
                    || lock_state_rank(obs.lock_state) >= lock_state_rank(LockState::Tracking);
                if p.quiet_after_s.is_some_and(|q| elapsed_s >= q) && !signal {
                    return Verdict::Empty(EmptyReason::NoSignal);
                }
                if elapsed_s >= p.timeout_s {
                    return Verdict::Empty(EmptyReason::NoLock);
                }
                return Verdict::Pending;
            }
        }
        let since = elapsed_s - self.locked_at.unwrap_or(elapsed_s);
        // With a name to read, the lock is reported as soon as the name is there. Else after the whole collect time.
        let named = !p.name_from.is_empty() && r.is_some_and(|r| !self.name(r).is_empty());
        if named || since >= p.collect_s {
            return Verdict::Locked(self.found(obs, r));
        }
        Verdict::Pending
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sdrkit_core::publish::{field, Field, Published};
    use sdrkit_core::Status;

    const STATIONS: Field<field::Text> = Field::new("stations", "Stations", "");
    const VESSELS: Field<field::I64> = Field::new("vessels", "Vessels", "");
    const EMPTY: Field<field::Text> = Field::new("empty", "Empty", "");
    const STATION: Field<field::Text> = Field::new("station", "Station", "");
    const LABEL: Field<field::Text> = Field::new("ensemble_label", "Ensemble", "");
    const SNR: Field<field::F64> = Field::new("snr_db", "SNR", "dB");

    /// The profile of DAB, as the stack declares it.
    pub(crate) const DAB: LockProfile = LockProfile {
        settle_s: 0.2,
        quiet_after_s: Some(1.5),
        signal_snr_db: 4.0,
        timeout_s: 5.0,
        lock: LockCondition::LockState {
            at_least: LockState::Locked,
        },
        collect_s: 3.0,
        name_from: &[NameSource::Field("ensemble_label")],
        capture: &["snr_db"],
    };
    /// The profile of AIS, as the stack declares it.
    pub(crate) const AIS: LockProfile = LockProfile {
        settle_s: 0.1,
        quiet_after_s: None,
        signal_snr_db: 6.0,
        timeout_s: 20.0,
        lock: LockCondition::Any(&[
            LockCondition::LockState {
                at_least: LockState::Locked,
            },
            LockCondition::FieldAbove {
                key: "stations",
                min: 0.0,
            },
        ]),
        collect_s: 0.0,
        name_from: &[],
        capture: &["stations"],
    };

    fn results() -> Published {
        Published::new(&[&STATIONS, &VESSELS, &EMPTY, &STATION, &LABEL, &SNR])
    }

    fn obs(lock_state: LockState, snr: f32) -> Observation {
        Observation {
            lock_state,
            snr_db: Some(snr),
            ..Observation::default()
        }
    }

    fn check(c: &LockCondition, o: &Observation, r: Option<&Results>) -> bool {
        let mut keys = Vec::new();
        condition_keys(c, &mut keys);
        let ids: Vec<_> = keys
            .into_iter()
            .map(|k| k.and_then(|k| r.and_then(|r| r.decl().field_id(k))))
            .collect();
        holds(c, o, r, &ids, &mut 0)
    }

    #[test]
    fn conditions() {
        let mut p = results();
        p.set(&STATIONS, "3");
        p.set(&EMPTY, " ");
        let r = Some(p.results());
        let o = obs(LockState::Tracking, 10.0);
        let lock_state = |l| LockCondition::LockState { at_least: l };
        assert!(check(&lock_state(LockState::Searching), &o, None));
        assert!(!check(&lock_state(LockState::Locked), &o, None));
        let above = |key, min| LockCondition::FieldAbove { key, min };
        assert!(check(&above("stations", 0.0), &o, r));
        p.set(&VESSELS, 2);
        let r = Some(p.results());
        assert!(check(&above("vessels", 1.5), &o, r));
        assert!(!check(&LockCondition::FieldPresent { key: "empty" }, &o, r));
        assert!(check(
            &LockCondition::Any(&[
                LockCondition::Snr { min_db: 20.0 },
                LockCondition::Snr { min_db: 5.0 }
            ]),
            &o,
            None
        ));
    }

    #[test]
    fn names_come_from_a_field_or_the_summary() {
        let mut p = results();
        p.set(&STATION, " RADIO 1 ");
        p.results_mut()
            .set_status(&Status::new(LockState::Locked).with_summary("Locked, 12 services"));
        let mut t = ProfileLockTest::new(LockProfile {
            name_from: &[NameSource::Field("station")],
            ..LockProfile::DEFAULT
        });
        t.resolved.update(&t.profile.clone(), p.results().decl());
        assert_eq!(t.name(p.results()), "RADIO 1");
        let mut t = ProfileLockTest::new(LockProfile {
            name_from: &[NameSource::Field("nope"), NameSource::Summary],
            ..LockProfile::DEFAULT
        });
        t.resolved.update(&t.profile.clone(), p.results().decl());
        assert_eq!(t.name(p.results()), "Locked, 12 services");
    }

    #[test]
    fn quiet_channel_exits_early_and_timeout_ends_a_weak_one() {
        let mut t = ProfileLockTest::new(DAB);
        t.begin(&Channel::new("5A", 174.928e6, "dab"));
        assert_eq!(
            t.judge(0.5, &obs(LockState::Searching, 0.0), None),
            Verdict::Pending
        );
        assert_eq!(
            t.judge(1.6, &obs(LockState::Searching, 0.0), None),
            Verdict::Empty(EmptyReason::NoSignal)
        );
        t.begin(&Channel::new("5B", 176.64e6, "dab"));
        assert_eq!(
            t.judge(1.6, &obs(LockState::Searching, 8.0), None),
            Verdict::Pending
        );
        assert_eq!(
            t.judge(5.0, &obs(LockState::Searching, 8.0), None),
            Verdict::Empty(EmptyReason::NoLock)
        );
    }

    #[test]
    fn waits_for_the_name_after_a_lock() {
        let mut t = ProfileLockTest::new(DAB);
        t.begin(&Channel::new("12C", 227.36e6, "dab"));
        let mut p = results();
        assert_eq!(
            t.judge(1.0, &obs(LockState::Locked, 15.0), Some(p.results())),
            Verdict::Pending
        );
        p.set(&LABEL, "Example Mux");
        p.set(&SNR, 15.5);
        match t.judge(1.5, &obs(LockState::Locked, 15.0), Some(p.results())) {
            Verdict::Locked(f) => {
                assert_eq!(f.name, "Example Mux");
                assert_eq!(f.values, vec![("snr_db", Value::F64(15.5))]);
                assert_eq!(f.snr_db, Some(15.0));
            }
            v => panic!("{v:?}"),
        }
        // Without a name, the lock is reported when the collect time is over.
        t.begin(&Channel::new("12D", 229.072e6, "dab"));
        let p = results();
        assert_eq!(
            t.judge(1.0, &obs(LockState::Locked, 15.0), Some(p.results())),
            Verdict::Pending
        );
        assert!(matches!(
            t.judge(4.0, &obs(LockState::Locked, 15.0), Some(p.results())),
            Verdict::Locked(f) if f.name.is_empty()
        ));
    }

    #[test]
    fn ais_locks_on_ships() {
        let mut t = ProfileLockTest::new(AIS);
        t.begin(&Channel::new("AIS 1", 161.975e6, "ais"));
        let mut p = results();
        p.set(&STATIONS, "0");
        assert_eq!(
            t.judge(5.0, &obs(LockState::Searching, 0.0), Some(p.results())),
            Verdict::Pending
        );
        p.set(&STATIONS, "2");
        match t.judge(6.0, &obs(LockState::Searching, 0.0), Some(p.results())) {
            Verdict::Locked(f) => assert_eq!(f.values, vec![("stations", Value::Text("2".into()))]),
            v => panic!("{v:?}"),
        }
    }
}
