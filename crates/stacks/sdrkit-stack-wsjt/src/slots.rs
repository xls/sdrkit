//! The slot clock and the slot cutter: audio sample to UTC, the time sources, slot search, and the cutting of each mode's
//! decode windows.

use crate::mode::Sub;
use crate::rx::slot::Slot;

sdrkit_core::choice_enum! {
    /// Where the slot clock comes from.
    pub enum TimeSource {
        /// The system clock (a live radio).
        SystemClock = ("system_clock", "System clock"),
        /// A recording whose start is known.
        RecordingStart = ("recording_start", "Recording start"),
        /// The slots are being searched for (start unknown).
        Searching = ("searching", "Searching the slots"),
        /// The slots were found from the signals.
        FoundFromSignals = ("found_from_signals", "Slots found from the signals"),
    }
}

/// The mapping of audio samples to UTC at one moment (stream kind `wsjt.time`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClockMap {
    /// An audio sample index.
    pub at: u64,
    /// Its UTC, seconds since 1970.
    pub utc: f64,
    /// The time source.
    pub source: TimeSource,
    /// Increments on every re-anchoring: cutters start again from the next slot.
    pub epoch: u32,
    /// The end (one past the last sample) of the audio this map came with.
    pub end: u64,
}

impl ClockMap {
    /// The UTC of sample `i` at `rate`.
    pub fn utc_of(&self, i: u64, rate: f64) -> f64 {
        self.utc + (i as f64 - self.at as f64) / rate
    }
    /// The sample index of a UTC (it can be fractional or before 0).
    pub fn index_of(&self, utc: f64, rate: f64) -> f64 {
        self.at as f64 + (utc - self.utc) * rate
    }
}

sdrkit_core::choice_enum! {
    /// The time source that the user chooses.
    pub enum TimeChoice {
        /// The system clock.
        System = ("system", "System clock"),
        /// A given start.
        Start = ("start", "Recording start"),
        /// Slot search.
        Search = ("search", "Slot search"),
    }
}

/// A source of the wall-clock time, seconds since 1970 (replaceable in tests).
pub type WallClock = sdrkit_core::Clock;

/// The system's wall clock.
pub fn system_clock() -> WallClock {
    sdrkit_core::Clock::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    })
}

/// The slot clock: anchors the first audio sample to UTC by the chosen time source, then counts samples. With the system
/// clock on a live input it also follows the wall clock.
pub struct SlotClock {
    choice: TimeChoice,
    start_utc: f64,
    wall: WallClock,
    rate: f64,
    map: Option<ClockMap>,
    err_smooth: f64,
    epoch: u32,
    changed: bool,
    follow: bool,
}

impl SlotClock {
    /// A clock for audio at `rate`. It follows the wall clock (see [`SlotClock::set_follow`]).
    pub fn new(choice: TimeChoice, start_utc: f64, rate: f64, wall: WallClock) -> SlotClock {
        SlotClock {
            choice,
            start_utc,
            wall,
            rate,
            map: None,
            err_smooth: 0.0,
            epoch: 0,
            changed: true,
            follow: true,
        }
    }
    /// Whether the system clock source follows the wall clock after the anchor: on for a live radio, whose samples arrive
    /// as the air sends them. Off, the time of a sample is the anchor plus its sample count, whatever the pace of the input.
    pub fn set_follow(&mut self, on: bool) {
        self.follow = on;
    }
    /// Starts again (a reset or retune): the next sample is anchored afresh.
    pub fn reset(&mut self) {
        self.map = None;
        self.err_smooth = 0.0;
        self.changed = true;
    }
    /// Changes the time source (applies at the next reset).
    pub fn set_choice(&mut self, choice: TimeChoice, start_utc: f64) {
        self.choice = choice;
        self.start_utc = start_utc;
    }
    /// The time source in use.
    pub fn source(&self) -> TimeSource {
        self.map.map(|m| m.source).unwrap_or(match self.choice {
            TimeChoice::System => TimeSource::SystemClock,
            TimeChoice::Start => TimeSource::RecordingStart,
            TimeChoice::Search => TimeSource::Searching,
        })
    }
    /// The current map.
    pub fn map(&self) -> Option<ClockMap> {
        self.map
    }
    /// Takes whether the map changed since the last call.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Advances over a buffer whose first sample is `first` and which holds `n` samples.
    pub fn advance(&mut self, first: u64, n: usize) {
        if self.map.is_none() {
            let (utc, source) = match self.choice {
                TimeChoice::System => (self.wall.now(), TimeSource::SystemClock),
                TimeChoice::Start => (self.start_utc, TimeSource::RecordingStart),
                // An arbitrary grid until the slots are found: the recording start, or 0.
                TimeChoice::Search => (self.start_utc, TimeSource::Searching),
            };
            self.epoch += 1;
            self.map = Some(ClockMap {
                at: first,
                utc,
                source,
                epoch: self.epoch,
                end: first + n as u64,
            });
            self.changed = true;
            return;
        }
        if let Some(m) = self.map.as_mut() {
            m.end = first + n as u64;
        }
        if self.choice != TimeChoice::System || !self.follow {
            return;
        }
        // Follow the system clock: the error at the end of the buffer, smoothed; a jump re-anchors at once.
        let Some(m) = self.map else { return };
        let end = first + n as u64;
        let now = self.wall.now();
        let err = now - m.utc_of(end, self.rate);
        if err.abs() > 2.0 {
            self.reanchor(end, now);
            return;
        }
        self.err_smooth += 0.05 * (err - self.err_smooth);
        if self.err_smooth.abs() > 0.2 {
            self.reanchor(end, now);
        }
    }

    fn reanchor(&mut self, at: u64, utc: f64) {
        self.epoch += 1;
        self.err_smooth = 0.0;
        self.map = Some(ClockMap {
            at,
            utc,
            source: TimeSource::SystemClock,
            epoch: self.epoch,
            end: at,
        });
        self.changed = true;
    }

    /// Slot search found the slots: move the clock by `shift_s` (added to every UTC) and mark the source.
    pub fn found(&mut self, shift_s: f64) {
        if let Some(m) = self.map.as_mut() {
            m.utc += shift_s;
            m.source = TimeSource::FoundFromSignals;
            self.epoch += 1;
            m.epoch = self.epoch;
            self.changed = true;
        }
    }
}

/// Cuts one mode's slots out of the audio.
pub struct Cutter {
    sub: Sub,
    /// The next sub-mode / period, applied from the next slot cut.
    pending: Option<Sub>,
    enabled: bool,
    rate: f64,
    buf: Vec<f32>,
    buf_first: u64,
    /// The first sample index seen since the last reset.
    audio_first: Option<u64>,
    next: Option<i64>,
    epoch: u32,
    dial_hz: f64,
    /// Early cuts are made (FT8 and FT4).
    early: bool,
    /// The slot index of the last early cut.
    early_done: Option<i64>,
}

impl Cutter {
    /// A cutter for `sub` at `rate`.
    pub fn new(sub: Sub, rate: f64) -> Cutter {
        Cutter {
            sub,
            pending: None,
            enabled: true,
            rate,
            buf: Vec::new(),
            buf_first: 0,
            audio_first: None,
            next: None,
            epoch: 0,
            dial_hz: 0.0,
            early: false,
            early_done: None,
        }
    }
    /// Turns early cuts on or off. Only FT8 and FT4 have them; for the other stacks the setting is kept but makes no cut.
    pub fn set_early(&mut self, on: bool) {
        self.early = on;
    }
    /// The time of the early cut after the slot start, seconds: t0 + 0.85 of the transmission + 0.05 s; `None` for a stack
    /// without early cuts.
    pub fn early_at(sub: &Sub) -> Option<f64> {
        matches!(sub.mode, crate::mode::Mode::Ft8 | crate::mode::Mode::Ft4)
            .then(|| sub.t0() + 0.85 * sub.tx_s() + 0.05)
    }
    /// The mode in use.
    pub fn sub(&self) -> Sub {
        self.sub
    }
    /// Changes the sub-mode or period (from the next slot).
    pub fn set_sub(&mut self, sub: Sub) {
        self.pending = Some(sub);
    }
    /// Enables or disables cutting (the slot counter still advances).
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }
    /// Whether enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    /// The dial frequency stamped on slots.
    pub fn set_dial(&mut self, hz: f64) {
        self.dial_hz = hz;
    }
    /// Drops everything (a retune or reset): starts with the next slot.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.audio_first = None;
        self.next = None;
    }
    /// Audio kept, seconds.
    pub fn kept_s(&self) -> f64 {
        self.buf.len() as f64 / self.rate
    }

    fn step(&self, map: &ClockMap) -> f64 {
        if map.source == TimeSource::Searching {
            self.sub.period / 2.0
        } else {
            self.sub.period
        }
    }

    fn window(&self, map: &ClockMap) -> (f64, f64) {
        if map.source == TimeSource::Searching {
            let p = self.sub.period;
            let g = 2.0 * self.sub.symbol_s();
            (
                self.sub.t0() - 0.05 * p - g,
                self.sub.t0() + 0.55 * p + self.sub.tx_s() + g,
            )
        } else {
            self.sub.window()
        }
    }

    /// Takes a buffer of audio (first sample `first`) with the clock `map`; appends the slots whose windows are complete to `out`.
    /// `flush`: the end of a recording (slots whose audio reached t0 + half the transmission are cut too). `early_ok`: an early
    /// cut may be made now (the decoder is idle). With early cuts on, the slot being received is cut once, with the audio so far,
    /// when its audio reaches [`Cutter::early_at`] and `early_ok` holds; that slot has `early` set. No early cut is made while
    /// the slots are searched for.
    pub fn push(
        &mut self,
        audio: &[f32],
        first: u64,
        map: &ClockMap,
        flush: bool,
        early_ok: bool,
        out: &mut Vec<Slot>,
    ) {
        if map.epoch != self.epoch {
            // A new anchoring: start again from the next slot (keep the audio).
            self.epoch = map.epoch;
            self.next = None;
        }
        if self.audio_first.is_none() {
            self.audio_first = Some(first);
            self.buf_first = first;
            self.buf.clear();
        }
        // Append (a gap restarts the buffer).
        if first != self.buf_first + self.buf.len() as u64 {
            self.buf.clear();
            self.buf_first = first;
            self.audio_first = Some(first);
            self.next = None;
        }
        self.buf
            .extend(audio.iter().map(|&x| if x.is_finite() { x } else { 0.0 }));
        let end = self.buf_first + self.buf.len() as u64;
        let rate = self.rate;
        let audio_first = *self.audio_first.get_or_insert(first);
        let start_utc = map.utc_of(audio_first, rate);
        loop {
            if let Some(p) = self.pending.take() {
                self.sub = p;
                self.next = None;
            }
            let step = self.step(map);
            let (w0, w1) = self.window(map);
            let k = match self.next {
                Some(k) => k,
                None => {
                    // The first slot whose transmission starts at most 1.0 s after the audio began.
                    let mut k = ((start_utc - self.sub.t0() - 1.0) / step).ceil() as i64;
                    if map.source == TimeSource::Searching {
                        k = ((start_utc - w0) / step).ceil() as i64;
                    }
                    self.next = Some(k);
                    k
                }
            };
            let slot_utc = k as f64 * step;
            let i0 = map.index_of(slot_utc + w0, rate).floor();
            let i1 = map.index_of(slot_utc + w1, rate).ceil();
            let i_half = map.index_of(slot_utc + self.sub.t0() + self.sub.tx_s() / 2.0, rate);
            let have = end as f64;
            let complete = have >= i1 || (flush && have >= i_half);
            if !complete {
                let searching = map.source == TimeSource::Searching;
                if let Some(t) = Self::early_at(&self.sub)
                    .filter(|_| self.early && early_ok && self.enabled && !searching && !flush)
                {
                    let i_early = map.index_of(slot_utc + t, rate);
                    if self.early_done != Some(k) && have >= i_early {
                        self.early_done = Some(k);
                        let a = (i0.max(self.buf_first as f64) as u64).max(self.buf_first);
                        if end > a {
                            let s = (a - self.buf_first) as usize;
                            out.push(Slot {
                                sub: self.sub,
                                utc: slot_utc,
                                audio: std::sync::Arc::new(self.buf[s..].to_vec()),
                                rate,
                                t_first: map.utc_of(a, rate) - slot_utc,
                                search: false,
                                dial_hz: self.dial_hz,
                                index: k,
                                early: true,
                                seq: 0,
                            });
                        }
                    }
                }
                break;
            }
            self.next = Some(k + 1);
            if self.enabled {
                let a = (i0.max(self.buf_first as f64) as u64).max(self.buf_first);
                let b = (i1 as u64).min(end);
                if b > a {
                    let s = (a - self.buf_first) as usize;
                    let e = (b - self.buf_first) as usize;
                    out.push(Slot {
                        sub: self.sub,
                        utc: slot_utc,
                        audio: std::sync::Arc::new(self.buf[s..e].to_vec()),
                        rate,
                        t_first: map.utc_of(a, rate) - slot_utc,
                        search: map.source == TimeSource::Searching,
                        dial_hz: self.dial_hz,
                        index: k,
                        early: false,
                        seq: 0,
                    });
                }
            }
            if flush && have < i1 {
                break;
            }
        }
        // Keep the audio from the earliest window start still needed.
        let step = self.step(map);
        let (w0, _) = self.window(map);
        if let Some(k) = self.next {
            let keep_from = map.index_of(k as f64 * step + w0, rate).floor().max(0.0) as u64;
            let cap = (self.rate * (self.sub.period + 60.0).max(300.0)) as usize;
            let mut drop = keep_from.saturating_sub(self.buf_first) as usize;
            if self.buf.len() > cap {
                drop = drop.max(self.buf.len() - cap);
            }
            let drop = drop.min(self.buf.len());
            if drop > 0 {
                self.buf.drain(..drop);
                self.buf_first += drop as u64;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::Mode;

    fn fixed(t: f64) -> WallClock {
        sdrkit_core::Clock::fixed(t)
    }

    #[test]
    fn a_late_start_skips_the_slot() {
        // Audio starting 7 s into an FT8 slot: the first slot cut is the next one.
        let mut clock = SlotClock::new(TimeChoice::Start, 1_767_225_607.0, 8000.0, fixed(0.0));
        let mut cut = Cutter::new(Sub::new(Mode::Ft8), 8000.0);
        let mut out = Vec::new();
        let chunk = vec![0f32; 800];
        for i in 0..400u64 {
            clock.advance(i * 800, 800);
            cut.push(
                &chunk,
                i * 800,
                &clock.map().unwrap(),
                false,
                false,
                &mut out,
            );
        }
        assert!(!out.is_empty());
        assert_eq!(out[0].utc, 1_767_225_615.0);
        assert!((out[1].utc - out[0].utc - 15.0).abs() < 1e-9);
    }

    #[test]
    fn slots_are_the_same_for_any_cut() {
        let run = |piece: usize| {
            let mut clock = SlotClock::new(TimeChoice::Start, 0.0, 8000.0, fixed(0.0));
            let mut cut = Cutter::new(Sub::new(Mode::Ft4), 8000.0);
            let audio: Vec<f32> = (0..8000 * 30).map(|i| (i % 97) as f32).collect();
            let mut out = Vec::new();
            let mut i = 0;
            while i < audio.len() {
                let n = piece.min(audio.len() - i);
                clock.advance(i as u64, n);
                cut.push(
                    &audio[i..i + n],
                    i as u64,
                    &clock.map().unwrap(),
                    false,
                    false,
                    &mut out,
                );
                i += n;
            }
            out.iter()
                .map(|s| (s.utc, s.audio.len(), s.t_first))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(800), run(1234));
        assert_eq!(run(800).len(), 3);
    }

    #[test]
    fn one_early_cut_per_slot_when_allowed() {
        let run = |early: bool, early_ok: bool, choice: TimeChoice| {
            let mut clock = SlotClock::new(choice, 0.0, 8000.0, fixed(0.0));
            let mut cut = Cutter::new(Sub::new(Mode::Ft8), 8000.0);
            cut.set_early(early);
            let mut out = Vec::new();
            let chunk = vec![0f32; 800];
            for i in 0..200u64 {
                clock.advance(i * 800, 800);
                cut.push(
                    &chunk,
                    i * 800,
                    &clock.map().unwrap(),
                    false,
                    early_ok,
                    &mut out,
                );
            }
            out.iter().map(|s| (s.utc, s.early)).collect::<Vec<_>>()
        };
        // 20 s from a slot start: the early cut of slot 0 at 11.3 s, slot 0 at 15.96 s; slot 1 is not due.
        assert_eq!(
            run(true, true, TimeChoice::Start),
            vec![(0.0, true), (0.0, false)]
        );
        assert_eq!(run(true, false, TimeChoice::Start), vec![(0.0, false)]);
        assert_eq!(run(false, true, TimeChoice::Start), vec![(0.0, false)]);
        assert!(run(true, true, TimeChoice::Search).iter().all(|s| !s.1));
        assert_eq!(Cutter::early_at(&Sub::new(Mode::Ft2)), None);
        let t = Cutter::early_at(&Sub::new(Mode::Ft8)).unwrap();
        assert!((t - 11.294).abs() < 1e-9, "{t}");
    }

    #[test]
    fn the_slot_queue_is_idle_when_the_newest_slot_is_decoded() {
        let q = crate::runner::SlotQueue::new();
        assert!(q.idle());
        let a = q.next();
        let b = q.next();
        assert!(!q.idle());
        q.done(a);
        assert!(!q.idle());
        // A slot dropped from the backlog: the newer one decoded makes the queue idle.
        q.done(b);
        assert!(q.idle());
        q.done(0);
        assert!(q.idle());
    }

    #[test]
    fn the_system_clock_reanchors_on_a_jump() {
        let t = std::sync::Arc::new(std::sync::Mutex::new(100.0f64));
        let t2 = t.clone();
        let mut clock = SlotClock::new(
            TimeChoice::System,
            0.0,
            8000.0,
            sdrkit_core::Clock::new(move || *t2.lock().unwrap()),
        );
        clock.advance(0, 800);
        let e0 = clock.map().unwrap().epoch;
        *t.lock().unwrap() = 100.2;
        clock.advance(800, 800);
        assert_eq!(clock.map().unwrap().epoch, e0);
        *t.lock().unwrap() = 110.0;
        clock.advance(1600, 800);
        assert_ne!(clock.map().unwrap().epoch, e0);
    }
}
