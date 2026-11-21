//! A log of calls: transmissions with an identity (a talkgroup, a source, a slot) that start, continue and end by their
//! terminator, by a hang time without frames, or when another call takes the channel.

use crate::log::LogRing;
use crate::persist::Shift;
use crate::seen::Stamp;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Decides whether two records on the same channel belong to the same call.
///
/// A stack implements it on its own call record, usually by comparing the source, the target and the group flag.
pub trait SameCall {
    /// Returns true if `other` (the identity that just arrived) belongs to the call this record describes.
    fn same_call(&self, other: &Self) -> bool;
}

sdrkit_core::choice_enum! {
    /// Where a call is in its life. It is a choice, so a stack publishes it as a `Field<field::Choice<CallEnd>>`.
    #[derive(Default, Serialize, Deserialize)]
    pub enum CallEnd {
        /// On air.
        #[default]
        Active = ("active", "On air"),
        /// Its terminator came.
        Terminator = ("terminator", "Terminator"),
        /// No frame came for longer than the hang time ("signal lost").
        HangTime = ("hang_time", "Hang time"),
        /// A call with another identity started on its channel.
        Replaced = ("replaced", "Replaced"),
        /// It was ended from outside: a retune, or a restored state that held it on air.
        Closed = ("closed", "Closed"),
        /// A transmission that is over as soon as it starts: a message, a grant, a status.
        Single = ("single", "Single"),
    }
}

/// What a host hears about from [`CallLog`]: every call starts once and ends once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallEvent<K> {
    /// A call started.
    Started {
        /// Its id in the log.
        id: u64,
        /// Its channel.
        channel: K,
    },
    /// A call ended.
    Ended {
        /// Its id in the log.
        id: u64,
        /// Its channel.
        channel: K,
        /// How it ended.
        end: CallEnd,
    },
}

/// One call in the log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call<K, R> {
    /// Its id: it grows by one with every call and never repeats within the log's life, saved states included.
    pub id: u64,
    /// Where it is on air: a slot, a carrier, or both.
    pub channel: K,
    /// When its first identity arrived (signal time, and the wall time when known).
    pub start: Stamp,
    /// Signal time of its last frame or its terminator, seconds.
    pub last_s: f64,
    /// Frames counted with [`CallLog::frame`].
    pub frames: u64,
    /// It started from an identity sent inside the call (late entry), not from its header.
    pub late_entry: bool,
    /// Where it is in its life.
    pub end: CallEnd,
    /// The stack's record: identities, flags, counters, texts.
    pub record: R,
}

impl<K, R> Call<K, R> {
    /// The time from its start to its last frame, seconds.
    pub fn duration_s(&self) -> f64 {
        (self.last_s - self.start.t).max(0.0)
    }

    /// Returns true while the call is on air.
    pub fn is_active(&self) -> bool {
        self.end == CallEnd::Active
    }
}

impl<K, R: Shift> Shift for Call<K, R> {
    fn shift_times(&mut self, by_s: f64) {
        self.start.shift_times(by_s);
        self.last_s += by_s;
        self.record.shift_times(by_s);
    }
}

/// The calls of a receiver: the calls on air, one per channel, and a capped log of every call, oldest first.
///
/// A call starts when its first decodable identity arrives ([`CallLog::begin`]), counts its frames ([`CallLog::frame`]), and
/// ends by its terminator ([`CallLog::end`]), after the hang time without a frame ([`CallLog::expire`]), when another identity
/// starts on its channel, or with [`CallLog::close_all`] on a retune. Each start and end is given to an event callback. Use
/// [`crate::Activity`] instead when only the presence of a transmission on a channel matters.
///
/// ```
/// use sdrkit_util_objects::calls::{CallEnd, CallEvent, CallLog, SameCall};
/// use sdrkit_util_objects::Stamp;
///
/// #[derive(Clone, Debug, PartialEq)]
/// struct Voice { source: u32, talkgroup: u32 }
/// impl SameCall for Voice {
///     fn same_call(&self, o: &Self) -> bool { self.source == o.source && self.talkgroup == o.talkgroup }
/// }
///
/// let mut calls: CallLog<u8, Voice> = CallLog::new(2.0, 2, 300);
/// let mut events = Vec::new();
/// calls.begin(1, Stamp::at(10.0), false, Voice { source: 3120123, talkgroup: 91 }, |e| events.push(e));
/// calls.frame(1, 10.06);
/// calls.end(1, 10.5, |e| events.push(e));
/// let c = calls.newest_first().next().unwrap();
/// assert_eq!((c.end, c.frames, c.duration_s()), (CallEnd::Terminator, 1, 0.5));
/// assert_eq!(events[1], CallEvent::Ended { id: 1, channel: 1, end: CallEnd::Terminator });
/// ```
#[derive(Clone, Debug)]
pub struct CallLog<K, R> {
    log: LogRing<Call<K, R>>,
    /// The channels on air and the ids of their calls.
    active: Vec<(K, u64)>,
    channels: usize,
    hang_s: f64,
    next_id: u64,
}

impl<K: Copy + Eq, R> CallLog<K, R> {
    /// An empty log of at most `cap` calls (at least 1). A call ends after `hang_s` seconds without a frame (a value that is not
    /// finite or below 0 returns 0). Room is made for `channels` calls on air at once.
    pub fn new(hang_s: f64, channels: usize, cap: usize) -> Self {
        let channels = channels.max(1);
        CallLog {
            log: LogRing::new(cap),
            active: Vec::with_capacity(channels),
            channels,
            hang_s: if hang_s.is_finite() {
                hang_s.max(0.0)
            } else {
                0.0
            },
            next_id: 1,
        }
    }

    /// The hang time, seconds.
    pub fn hang_s(&self) -> f64 {
        self.hang_s
    }

    /// Sets the hang time, seconds. A value that is not finite or below 0 is refused, and the hang time does not change.
    pub fn set_hang_s(&mut self, hang_s: f64) -> bool {
        if hang_s.is_finite() && hang_s >= 0.0 {
            self.hang_s = hang_s;
            true
        } else {
            false
        }
    }

    /// The most calls the log keeps.
    pub fn cap(&self) -> usize {
        self.log.cap()
    }

    /// Sets the maximum number of calls kept (at least 1). If the log holds more, the oldest are removed, and tracking stops for a
    /// removed call that is on air.
    pub fn set_cap(&mut self, cap: usize) {
        self.log.set_cap(cap);
        self.forget_missing();
    }

    /// The identity of a call arrived on `channel` at `at` (a header, or an identity inside the call when `late_entry`).
    ///
    /// If the channel carries a call that [`SameCall::same_call`] says is this one, that call goes on: its last time moves to
    /// `at` and `record` is dropped. Otherwise the call there ends as [`CallEnd::Replaced`], and a new call starts with `record`.
    /// The replaced call keeps the time of its last frame. If the log is full, the oldest call leaves. If it was on air, it ends
    /// as [`CallEnd::Closed`] first. If calls are on air on every channel room was made for, the one silent longest ends as
    /// [`CallEnd::Closed`]. Events go to `events`. It returns the call, for the caller to fill in. A time that is not finite is
    /// taken as the channel's last time, or 0.
    pub fn begin(
        &mut self,
        channel: K,
        at: Stamp,
        late_entry: bool,
        record: R,
        mut events: impl FnMut(CallEvent<K>),
    ) -> &mut Call<K, R>
    where
        R: SameCall,
    {
        let at = if at.is_valid() {
            at
        } else {
            Stamp::at(self.call_on(channel).map_or(0.0, |c| c.last_s))
        };
        let same = self
            .call_on(channel)
            .is_some_and(|c| c.record.same_call(&record));
        if !same {
            self.finish(channel, f64::NAN, CallEnd::Replaced, &mut events);
            let id = self.next_id;
            self.next_id += 1;
            self.push(
                Call {
                    id,
                    channel,
                    start: at,
                    last_s: at.t,
                    frames: 0,
                    late_entry,
                    end: CallEnd::Active,
                    record,
                },
                &mut events,
            );
            if self.active.len() == self.channels {
                // More channels on air than room was made for: the one silent longest stops being tracked.
                if let Some(i) = self.longest_silent() {
                    let (ch, old) = self.active.swap_remove(i);
                    self.end_call(old, ch, f64::NAN, CallEnd::Closed, &mut events);
                }
            }
            self.active.push((channel, id));
            events(CallEvent::Started { id, channel });
        }
        let id = self.active_id(channel).unwrap_or(0);
        let call = self.find_mut(id).expect("a call on air is in the log");
        if same && at.t > call.last_s {
            call.last_s = at.t;
        }
        call
    }

    /// A transmission that is over as soon as it starts (a message, a grant, a status) arrived on `channel` at `at`. It is logged
    /// with [`CallEnd::Single`] and does not touch the call on air there. Events go to `events` (a start, then an end). It returns
    /// the logged call.
    pub fn single(
        &mut self,
        channel: K,
        at: Stamp,
        record: R,
        mut events: impl FnMut(CallEvent<K>),
    ) -> &mut Call<K, R> {
        let id = self.next_id;
        self.next_id += 1;
        let at = if at.is_valid() { at } else { Stamp::at(0.0) };
        self.push(
            Call {
                id,
                channel,
                start: at,
                last_s: at.t,
                frames: 0,
                late_entry: false,
                end: CallEnd::Single,
                record,
            },
            &mut events,
        );
        events(CallEvent::Started { id, channel });
        events(CallEvent::Ended {
            id,
            channel,
            end: CallEnd::Single,
        });
        self.find_mut(id).expect("the call was just logged")
    }

    /// A frame of the call on `channel` arrived at signal time `at_s`. It returns the call, or `None` when no call is on air
    /// there. A time that is not finite, or before the last frame, is counted but does not move the last time.
    pub fn frame(&mut self, channel: K, at_s: f64) -> Option<&mut Call<K, R>> {
        let call = self.call_on_mut(channel)?;
        call.frames += 1;
        if at_s.is_finite() && at_s > call.last_s {
            call.last_s = at_s;
        }
        Some(call)
    }

    /// The terminator of the call on `channel` arrived at signal time `at_s`: the call ends as [`CallEnd::Terminator`]. It
    /// returns the call, or `None` when no call is on air there.
    pub fn end(
        &mut self,
        channel: K,
        at_s: f64,
        mut events: impl FnMut(CallEvent<K>),
    ) -> Option<&mut Call<K, R>> {
        let id = self.finish(channel, at_s, CallEnd::Terminator, &mut events)?;
        self.find_mut(id)
    }

    /// The call on `channel` ended without its terminator: the receiver lost its signal before the hang time ran out. The call
    /// ends as [`CallEnd::HangTime`] and keeps the time of its last frame. It returns the call, or `None` when no call is on air
    /// there.
    pub fn lost(
        &mut self,
        channel: K,
        mut events: impl FnMut(CallEvent<K>),
    ) -> Option<&mut Call<K, R>> {
        let id = self.finish(channel, f64::NAN, CallEnd::HangTime, &mut events)?;
        self.find_mut(id)
    }

    /// Ends as [`CallEnd::HangTime`] every call with no frame for longer than the hang time at signal time `now_s`. Their last
    /// time stays the time of their last frame. It returns how many ended.
    pub fn expire(&mut self, now_s: f64, mut events: impl FnMut(CallEvent<K>)) -> usize {
        if !now_s.is_finite() {
            return 0;
        }
        let mut n = 0;
        let mut i = 0;
        while i < self.active.len() {
            let (channel, id) = self.active[i];
            let quiet = self.find(id).is_none_or(|c| now_s - c.last_s > self.hang_s);
            if quiet {
                self.active.swap_remove(i);
                self.end_call(id, channel, f64::NAN, CallEnd::HangTime, &mut events);
                n += 1;
            } else {
                i += 1;
            }
        }
        n
    }

    /// Ends every call on air as [`CallEnd::Closed`] (a retune).
    pub fn close_all(&mut self, mut events: impl FnMut(CallEvent<K>)) {
        while let Some((channel, id)) = self.active.pop() {
            self.end_call(id, channel, f64::NAN, CallEnd::Closed, &mut events);
        }
    }

    /// The call on air on `channel`.
    pub fn call_on(&self, channel: K) -> Option<&Call<K, R>> {
        self.find(self.active_id(channel)?)
    }

    /// The call on air on `channel`, to change.
    pub fn call_on_mut(&mut self, channel: K) -> Option<&mut Call<K, R>> {
        let id = self.active_id(channel)?;
        self.find_mut(id)
    }

    /// The call with id `id`, while it is in the log.
    pub fn get(&self, id: u64) -> Option<&Call<K, R>> {
        self.find(id)
    }

    /// The call with id `id`, to change, while it is in the log.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Call<K, R>> {
        self.find_mut(id)
    }

    /// The calls on air, in no set order.
    pub fn active(&self) -> impl Iterator<Item = &Call<K, R>> {
        self.active.iter().filter_map(|&(_, id)| self.find(id))
    }

    /// The calls in the log, oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Call<K, R>> + ExactSizeIterator {
        self.log.iter()
    }

    /// The calls in the log, newest first.
    pub fn newest_first(&self) -> impl Iterator<Item = &Call<K, R>> {
        self.log.newest_first()
    }

    /// The log, for publishing with [`crate::publish::log_rows`] and for saved state.
    pub fn log(&self) -> &LogRing<Call<K, R>> {
        &self.log
    }

    /// Calls in the log.
    pub fn len(&self) -> usize {
        self.log.len()
    }

    /// Returns true if the log holds no call.
    pub fn is_empty(&self) -> bool {
        self.log.is_empty()
    }

    /// Forgets every call, on air or not (the table action `clear`). Ids go on growing.
    pub fn clear(&mut self) {
        self.log.clear();
        self.active.clear();
    }

    /// Takes the calls of `saved` (a restored state) in place of what this log holds. It keeps this log's cap, hang time and
    /// room for channels. Calls the saved state held on air end as [`CallEnd::Closed`], and ids go on from the larger of both.
    pub fn adopt(&mut self, saved: CallLog<K, R>) {
        self.active.clear();
        self.log.adopt(saved.log);
        self.next_id = self.next_id.max(saved.next_id);
        close_restored(&mut self.log, &mut self.next_id);
    }

    fn active_id(&self, channel: K) -> Option<u64> {
        self.active
            .iter()
            .find(|(c, _)| *c == channel)
            .map(|&(_, id)| id)
    }

    fn longest_silent(&self) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for (i, &(_, id)) in self.active.iter().enumerate() {
            let last = self.find(id).map_or(f64::NEG_INFINITY, |c| c.last_s);
            if best.is_none_or(|(_, b)| last < b) {
                best = Some((i, last));
            }
        }
        best.map(|(i, _)| i)
    }

    fn find(&self, id: u64) -> Option<&Call<K, R>> {
        self.log.newest_first().find(|c| c.id == id)
    }

    fn find_mut(&mut self, id: u64) -> Option<&mut Call<K, R>> {
        self.log.find_recent_mut(usize::MAX, |c| c.id == id)
    }

    /// Adds a call. If the oldest leaves while on air, it ends as [`CallEnd::Closed`] and stops being tracked.
    fn push(&mut self, call: Call<K, R>, events: &mut impl FnMut(CallEvent<K>)) {
        if let Some(gone) = self.log.push(call) {
            if gone.is_active() {
                self.active.retain(|&(_, id)| id != gone.id);
                events(CallEvent::Ended {
                    id: gone.id,
                    channel: gone.channel,
                    end: CallEnd::Closed,
                });
            }
        }
    }

    /// Drops the channels whose call is not in the log.
    fn forget_missing(&mut self) {
        let mut i = 0;
        while i < self.active.len() {
            if self.find(self.active[i].1).is_some() {
                i += 1;
            } else {
                self.active.swap_remove(i);
            }
        }
    }

    /// Ends the call on air on `channel`, if any, and returns its id.
    fn finish(
        &mut self,
        channel: K,
        at_s: f64,
        end: CallEnd,
        events: &mut impl FnMut(CallEvent<K>),
    ) -> Option<u64> {
        let i = self.active.iter().position(|(c, _)| *c == channel)?;
        let (_, id) = self.active.swap_remove(i);
        self.end_call(id, channel, at_s, end, events);
        Some(id)
    }

    /// Marks the call `id` ended. A finite `at_s` later than its last time moves the last time.
    fn end_call(
        &mut self,
        id: u64,
        channel: K,
        at_s: f64,
        end: CallEnd,
        events: &mut impl FnMut(CallEvent<K>),
    ) {
        if let Some(c) = self.find_mut(id) {
            c.end = end;
            if at_s.is_finite() && at_s > c.last_s {
                c.last_s = at_s;
            }
        }
        events(CallEvent::Ended { id, channel, end });
    }
}

/// Ends the calls a restored log held on air, and moves `next_id` past every id in it.
fn close_restored<K, R>(log: &mut LogRing<Call<K, R>>, next_id: &mut u64) {
    for c in log.iter_mut() {
        *next_id = (*next_id).max(c.id.saturating_add(1));
        if c.is_active() {
            c.end = CallEnd::Closed;
        }
    }
}

impl<K, R: Shift> Shift for CallLog<K, R> {
    fn shift_times(&mut self, by_s: f64) {
        self.log.shift_times(by_s);
    }
}

#[derive(Serialize)]
struct LogOut<'a, K, R> {
    hang_s: f64,
    channels: usize,
    next_id: u64,
    calls: &'a LogRing<Call<K, R>>,
}

#[derive(Deserialize)]
#[serde(bound(deserialize = "K: DeserializeOwned, R: DeserializeOwned"))]
struct LogIn<K, R> {
    hang_s: f64,
    channels: usize,
    next_id: u64,
    calls: LogRing<Call<K, R>>,
}

/// Written as `{"hang_s", "channels", "next_id", "calls": {"cap", "entries": [...]}}`, oldest call first.
impl<K: Serialize, R: Serialize> Serialize for CallLog<K, R> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        LogOut {
            hang_s: self.hang_s,
            channels: self.channels,
            next_id: self.next_id,
            calls: &self.log,
        }
        .serialize(s)
    }
}

/// Reads what [`Serialize`] wrote. A call saved on air ends as [`CallEnd::Closed`], since nothing continues it. A call with a
/// time that is not finite is refused.
impl<'de, K: DeserializeOwned, R: DeserializeOwned> Deserialize<'de> for CallLog<K, R> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let i = LogIn::<K, R>::deserialize(d)?;
        if i.calls
            .iter()
            .any(|c| !c.start.is_valid() || !c.last_s.is_finite())
        {
            return Err(D::Error::custom("a call with a time that is not finite"));
        }
        let channels = i.channels.max(1);
        let mut log = i.calls;
        let mut next_id = i.next_id.max(1);
        close_restored(&mut log, &mut next_id);
        Ok(CallLog {
            log,
            active: Vec::with_capacity(channels),
            channels,
            hang_s: if i.hang_s.is_finite() {
                i.hang_s.max(0.0)
            } else {
                0.0
            },
            next_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl SameCall for u32 {
        fn same_call(&self, o: &Self) -> bool {
            self == o
        }
    }

    #[test]
    fn a_lost_call_ends_at_its_last_frame() {
        let mut log: CallLog<u8, u32> = CallLog::new(2.0, 2, 8);
        let mut events = Vec::new();
        log.begin(1, Stamp::at(10.0), false, 7, |e| events.push(e));
        log.frame(1, 10.4);
        let c = log.lost(1, |e| events.push(e)).unwrap();
        assert_eq!((c.end, c.last_s), (CallEnd::HangTime, 10.4));
        assert!(log.lost(1, |e| events.push(e)).is_none());
        assert_eq!(
            events[1],
            CallEvent::Ended {
                id: 1,
                channel: 1,
                end: CallEnd::HangTime
            }
        );
    }
}
