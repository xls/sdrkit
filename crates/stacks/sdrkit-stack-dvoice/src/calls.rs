//! The lists that every digital voice stack keeps: the call log, messages, positions, the event log, the site, talkgroups and
//! grants, with their caps and the hang time, and their saved state.

use crate::items::{
    CallKind, CallRecord, ControlRecord, GrantRecord, MessageRecord, PositionRecord, SiteRecord,
};
use crate::typed::TalkgroupRow;
use sdrkit_core::interface::Time;
use sdrkit_core::{StateBlob, StateError};
use sdrkit_util_objects::calls::{Call, CallEvent, CallLog};
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{Format, LogLine, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};

/// The version of the saved state.
const STATE_VERSION: u32 = 3;

/// The channel of a transmission that has none (a single message, grant or control message).
pub const NO_CHANNEL: u32 = u32::MAX;

/// Calls the log follows on air at once (time slots and carriers of a wide capture).
const MAX_ACTIVE: usize = 64;

/// The hang time a new log starts with, seconds.
pub const HANG_S: f64 = 2.0;

/// The most positions kept.
pub const POSITION_CAP: usize = 512;
/// The most lines of the event log kept.
pub const LOG_CAP: usize = 300;
/// The most grants kept.
pub const GRANT_CAP: usize = 300;
/// The most talkgroups kept.
pub const TALKGROUP_CAP: usize = 500;

/// The counters of one talkgroup.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TalkgroupStat {
    /// The talkgroup.
    pub tg: u64,
    /// Grants to it.
    pub grants: u32,
    /// Voice calls to it.
    pub calls: u32,
    /// A call to it was encrypted.
    pub encrypted: bool,
}

impl Shift for TalkgroupStat {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for TalkgroupStat {
    type Row = TalkgroupRow;
    fn row(&self, seen: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&TalkgroupRow {
            talkgroup: self.tg as i64,
            grants: i64::from(self.grants),
            calls: i64::from(self.calls),
            encrypted: self.encrypted,
            last_heard: sdrkit_util_objects::publish::columns::last_heard(seen).map(Time),
        });
    }
}

/// The part of the lists that is saved.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lists {
    /// The call log, oldest first, with the calls on air.
    pub calls: CallLog<u32, CallRecord>,
    /// Messages, oldest first.
    pub messages: LogRing<MessageRecord>,
    /// Positions, oldest first.
    pub positions: LogRing<PositionRecord>,
    /// The event log, oldest first.
    pub log: LogRing<LogLine>,
    /// The site of the system, when the stack sends one.
    pub site: Option<SiteRecord>,
    /// Talkgroups heard.
    pub talkgroups: ObjectTable<u64, TalkgroupStat>,
    /// Grants, oldest first.
    pub grants: LogRing<GrantRecord>,
}

impl Shift for Lists {
    fn shift_times(&mut self, by_s: f64) {
        self.calls.shift_times(by_s);
        self.messages.shift_times(by_s);
        self.positions.shift_times(by_s);
        self.log.shift_times(by_s);
        self.site.shift_times(by_s);
        self.talkgroups.shift_times(by_s);
        self.grants.shift_times(by_s);
    }
}

/// The lists of one receiver.
#[derive(Clone, Debug)]
pub struct Tracker {
    /// The stack id. Saved state of another stack is refused.
    pub mode: &'static str,
    /// The lists.
    pub lists: Lists,
    /// Calls that started or ended and are not yet given out.
    pub call_events: Vec<CallEvent<u32>>,
    /// Lines of the event log not yet in the results.
    pub pending_log: Vec<String>,
    /// The results log must be emptied before the pending lines are written.
    pub log_cleared: bool,
    /// Something changed since the last report.
    pub dirty: bool,
}

impl Tracker {
    /// The lists of stack `stack`, with at most `call_cap` calls and `message_cap` messages.
    pub fn new(mode: &'static str, call_cap: usize, message_cap: usize) -> Tracker {
        Tracker {
            mode,
            lists: Lists {
                calls: CallLog::new(HANG_S, MAX_ACTIVE, call_cap),
                messages: LogRing::new(message_cap),
                positions: LogRing::new(POSITION_CAP),
                log: LogRing::new(LOG_CAP),
                site: None,
                talkgroups: ObjectTable::new(TALKGROUP_CAP),
                grants: LogRing::new(GRANT_CAP),
            },
            call_events: Vec::new(),
            pending_log: Vec::new(),
            log_cleared: false,
            dirty: false,
        }
    }

    /// The call log.
    pub fn calls(&self) -> &CallLog<u32, CallRecord> {
        &self.lists.calls
    }

    /// The call log, to change.
    pub fn calls_mut(&mut self) -> &mut CallLog<u32, CallRecord> {
        &mut self.lists.calls
    }

    /// Empties every list.
    pub fn clear(&mut self) {
        let l = &mut self.lists;
        l.calls.clear();
        l.messages.clear();
        l.positions.clear();
        l.log.clear();
        l.site = None;
        l.talkgroups.clear_all();
        l.grants.clear();
        self.pending_log.clear();
        self.log_cleared = true;
        self.dirty = true;
    }

    /// The record of the call on air on `key`, to change.
    pub fn active(&mut self, key: u32) -> Option<&mut CallRecord> {
        self.lists.calls.call_on_mut(key).map(|c| &mut c.record)
    }

    /// The call on air on `key`.
    pub fn active_ref(&self, key: u32) -> Option<&Call<u32, CallRecord>> {
        self.lists.calls.call_on(key)
    }

    /// Counts a call to a talkgroup at signal time `now`.
    fn count_talkgroup(&mut self, tg: u64, now: f64, f: impl FnOnce(&mut TalkgroupStat)) {
        if let Ok(u) = self
            .lists
            .talkgroups
            .upsert(&tg, Stamp::at(now), || TalkgroupStat {
                tg,
                ..Default::default()
            })
        {
            f(u.row);
        }
    }

    /// The identity of a transmission arrived on `key` at signal time `now` (`late_entry`: from inside the call). If the call
    /// on `key` has the same identity, it goes on. Otherwise that call ends as replaced and a new one starts. It returns the id
    /// of the call.
    pub fn start(&mut self, key: u32, now: f64, rec: CallRecord, late_entry: bool) -> u64 {
        let before = self.call_events.len();
        let events = &mut self.call_events;
        let c = self
            .lists
            .calls
            .begin(key, Stamp::at(now), late_entry, rec, |e| events.push(e));
        let id = c.id;
        let tg = c.record.group.then(|| c.record.dst.number()).flatten();
        let voice = c.record.kind.is_voice();
        let encrypted = c.record.encryption.is_encrypted();
        let started = self.call_events[before..]
            .iter()
            .any(|e| matches!(e, CallEvent::Started { id: s, .. } if *s == id));
        if let (true, Some(tg)) = (started, tg) {
            self.count_talkgroup(tg, now, |t| {
                if voice {
                    t.calls += 1;
                }
                t.encrypted |= encrypted;
            });
        }
        self.dirty = true;
        id
    }

    /// Logs a transmission that is over as soon as it starts (a message, a grant, a control message). It returns its id.
    pub fn single(&mut self, now: f64, rec: CallRecord) -> u64 {
        if let Some(tg) = (rec.group && rec.kind == CallKind::Grant)
            .then(|| rec.dst.number())
            .flatten()
        {
            self.count_talkgroup(tg, now, |t| t.grants += 1);
        }
        let events = &mut self.call_events;
        let id = self
            .lists
            .calls
            .single(NO_CHANNEL, Stamp::at(now), rec, |e| events.push(e))
            .id;
        self.dirty = true;
        id
    }

    /// A frame of the transmission on `key` arrived at signal time `now`.
    pub fn touch(&mut self, key: u32, now: f64) {
        self.lists.calls.frame(key, now);
        self.dirty = true;
    }

    /// Ends the transmission on `key`: by its terminator (`end_seen`) at signal time `now`, or because the signal went.
    pub fn close(&mut self, key: u32, now: f64, end_seen: bool) {
        let events = &mut self.call_events;
        let c = if end_seen {
            self.lists.calls.end(key, now, |e| events.push(e))
        } else {
            self.lists.calls.lost(key, |e| events.push(e))
        };
        let Some(c) = c else {
            return;
        };
        let r = &c.record;
        let tg = r.group.then(|| r.dst.number()).flatten();
        let encrypted = r.encryption.is_encrypted();
        if let Some(t) = tg.and_then(|tg| self.lists.talkgroups.get_mut(&tg)) {
            t.encrypted |= encrypted;
        }
        self.dirty = true;
    }

    /// Moves the end of call `id`, which is off air, to signal time `now`: a repeated terminator.
    pub fn extend_end(&mut self, id: u64, now: f64) {
        if let Some(c) = self.lists.calls.get_mut(id) {
            if now > c.last_s {
                c.last_s = now;
                self.dirty = true;
            }
        }
    }

    /// Ends every transmission silent for longer than the hang time at signal time `now`. It returns how many ended.
    pub fn expire(&mut self, now: f64) -> usize {
        let events = &mut self.call_events;
        let n = self.lists.calls.expire(now, |e| events.push(e));
        if n > 0 {
            self.dirty = true;
        }
        n
    }

    /// Ends everything on air (a retune).
    pub fn close_all(&mut self) {
        let events = &mut self.call_events;
        self.lists.calls.close_all(|e| events.push(e));
        self.dirty = true;
    }

    /// The keys of the calls on air.
    pub fn active_keys(&self) -> impl Iterator<Item = u32> + '_ {
        self.lists.calls.active().map(|c| c.channel)
    }

    /// Adds a message.
    pub fn message(&mut self, m: MessageRecord) {
        self.lists.messages.push(m);
        self.dirty = true;
    }

    /// Adds a position. The same position from the same source, repeated within a minute, stays one entry whose time moves.
    pub fn position(&mut self, p: PositionRecord) {
        if let Some(q) = self.lists.positions.iter_mut().rev().take(8).find(|q| {
            q.src == p.src
                && q.source == p.source
                && (q.lat - p.lat).abs() < 1e-6
                && (q.lon - p.lon).abs() < 1e-6
                && p.time - q.time < 60.0
        }) {
            q.time = p.time;
            q.alt_m = p.alt_m.or(q.alt_m);
            q.speed_kmh = p.speed_kmh.or(q.speed_kmh);
            q.course_deg = p.course_deg.or(q.course_deg);
            self.dirty = true;
            return;
        }
        self.lists.positions.push(p);
        self.dirty = true;
    }

    /// Adds a line to the event log at signal time `now`.
    pub fn event(&mut self, now: f64, line: String) {
        self.pending_log.push(line.clone());
        self.lists.log.push(LogLine {
            at: Stamp::at(now),
            text: line,
        });
        self.dirty = true;
    }

    /// Adds the text of a control message to the event log.
    pub fn control(&mut self, c: &ControlRecord) {
        self.event(c.time, c.text.clone());
    }

    /// Sets the site of the system.
    pub fn site(&mut self, s: SiteRecord) {
        self.lists.site = Some(s);
        self.dirty = true;
    }

    /// Adds a grant.
    pub fn grant(&mut self, g: GrantRecord) {
        self.lists.grants.push(g);
        self.dirty = true;
    }

    /// The lists as saved state, at signal time `clock_s`.
    ///
    /// # Errors
    ///
    /// [`sdrkit_util_objects::Error`] if the lists cannot be written.
    pub fn save(&self, clock_s: f64) -> Result<StateBlob, sdrkit_util_objects::Error> {
        Format::new(self.mode, STATE_VERSION).save(clock_s, &self.lists)
    }

    /// Takes the lists of saved state `state` in place of these, with every time moved to signal time `now_s`. The caps and
    /// the hang time stay. Calls that the state held on air end as closed.
    ///
    /// # Errors
    ///
    /// [`StateError::WrongOwner`] for the state of another stack, [`StateError::WrongVersion`] for another version, and
    /// [`StateError::Damaged`] if it does not read.
    pub fn load(&mut self, state: &StateBlob, now_s: f64) -> Result<(), StateError> {
        let saved: Lists = Format::new(self.mode, STATE_VERSION).restore(state, now_s)?;
        let l = &mut self.lists;
        l.calls.adopt(saved.calls);
        l.messages.adopt(saved.messages);
        l.positions.adopt(saved.positions);
        l.log.adopt(saved.log);
        l.site = saved.site;
        l.talkgroups.adopt(saved.talkgroups);
        l.grants.adopt(saved.grants);
        self.pending_log.clear();
        self.pending_log
            .extend(self.lists.log.iter().map(|l| l.text.clone()));
        self.log_cleared = true;
        self.dirty = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Ident;
    use sdrkit_util_objects::calls::CallEnd;

    fn voice(src: u64) -> CallRecord {
        CallRecord {
            kind: CallKind::GroupVoice,
            src: Ident::Number(src),
            dst: Ident::Number(91),
            group: true,
            ..Default::default()
        }
    }

    #[test]
    fn calls_open_update_and_close_by_terminator_or_hang_time() {
        let mut t = Tracker::new("dmr", 3, 16);
        t.start(1, 1.0, voice(5), false);
        t.touch(1, 1.5);
        t.close(1, 1.6, true);
        let c = t.calls().iter().next().unwrap();
        assert_eq!(c.end, CallEnd::Terminator);
        assert!((c.duration_s() - 0.6).abs() < 1e-9);
        t.start(2, 2.0, voice(6), false);
        assert_eq!(t.expire(3.0), 0);
        assert_eq!(t.expire(4.1), 1);
        assert_eq!(t.calls().iter().nth(1).unwrap().end, CallEnd::HangTime);
        for i in 0..5 {
            t.single(
                5.0 + i as f64,
                CallRecord {
                    kind: CallKind::Data,
                    ..Default::default()
                },
            );
        }
        assert_eq!(t.calls().len(), 3);
        assert_eq!(t.lists.talkgroups.get(&91).unwrap().calls, 2);
    }

    #[test]
    fn the_same_identity_goes_on_and_another_one_replaces_the_call() {
        let mut t = Tracker::new("dmr", 8, 16);
        let a = t.start(1, 1.0, voice(5), false);
        assert_eq!(t.start(1, 1.2, voice(5), true), a);
        let b = t.start(1, 1.4, voice(6), false);
        assert_ne!(a, b);
        assert_eq!(t.calls().get(a).unwrap().end, CallEnd::Replaced);
        t.close(1, 1.5, false);
        assert_eq!(t.calls().get(b).unwrap().end, CallEnd::HangTime);
        assert_eq!(t.call_events.len(), 4);
    }

    #[test]
    fn a_saved_state_loads_back_and_another_stack_is_refused() {
        let mut t = Tracker::new("nxdn", 300, 100);
        t.single(
            1.0,
            CallRecord {
                kind: CallKind::Status,
                ..Default::default()
            },
        );
        t.event(1.0, "hello".into());
        let blob = t.save(2.0).unwrap();
        let mut u = Tracker::new("nxdn", 300, 100);
        u.load(&blob, 2.0).unwrap();
        assert_eq!(u.calls().len(), 1);
        assert_eq!(u.lists.log.iter().next().unwrap().text, "hello");
        assert!(matches!(
            Tracker::new("dmr", 1, 1).load(&blob, 2.0),
            Err(StateError::WrongOwner { .. })
        ));
        assert_eq!(u.calls().len(), 1);
    }
}
