//! The player: which channel plays, the scan over a whole span, the scan that steps one tuner, and the priority watch.

use crate::items::{Blocks, Carriers, Chan, ChanBlock, PlayState, ScanState};
use crate::{AUDIO_RATE, BLOCK};
use sdrkit_graph::Timed;
use std::collections::BTreeMap;

/// What a stack's channel plan gives the player. The stack builds it from its region data and its settings.
pub trait ScanPlan: Send {
    /// The channels of the scan, in scan order.
    fn scan_list(&self) -> Vec<Chan>;
    /// The channel the user picked: every frequency it can be heard on. `None` leaves the choice to the player.
    fn picked(&self) -> Option<Vec<Chan>>;
    /// The priority channel, when one is watched.
    fn priority(&self) -> Option<Chan> {
        None
    }
    /// `c` is the priority channel.
    fn is_priority(&self, c: Chan) -> bool {
        self.priority().is_some_and(|p| p.key == c.key)
    }
    /// `c` is shown and never chosen to play by the automatic choice.
    fn muted(&self, _c: Chan) -> bool {
        false
    }
    /// The scan always steps the radio, whatever the capture holds.
    fn always_single_tuner(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Default)]
struct ChanState {
    hz: f64,
    open: bool,
    since: f64,
    last_open: f64,
    level: f64,
}

/// The player. Each 10 ms step it takes the blocks of every tracked channel, decides what plays and gives that channel's audio.
pub struct Player {
    /// The plan.
    pub plan: Box<dyn ScanPlan>,
    /// The scan is on.
    pub scan: bool,
    /// The time the single-tuner scan stays on a quiet channel, s.
    pub dwell: f64,
    /// A channel is held this long after it went quiet, s.
    pub hang: f64,
    /// The time the single-tuner scan waits for the radio to settle after a step, added to the dwell, s.
    pub settle: f64,
    /// The time the blocks of a channel come behind the carrier lists, s: the silence between carriers waits this long, so
    /// that the blocks of a receiver that starts with stored signal are not late.
    pub lag: f64,
    states: BTreeMap<i64, ChanState>,
    playing: Option<Chan>,
    interrupted: Option<Chan>,
    last_played: Option<Chan>,
    scan_state: ScanState,
    tune_list_pos: usize,
    tune_since: f64,
    last_probe: f64,
    tune: Option<Chan>,
    clock: f64,
    /// The channels the capture holds (first and last raster cell), when known.
    span: Option<(f64, f64)>,
    /// The player's own clock runs on through retunes (the graph's restarts at each): the clock at the last configure and the
    /// first block time after it.
    clock_base: f64,
    first_time: Option<f64>,
    /// The next 10 ms step (in the graph's time) whose audio has not gone out.
    next_block: Option<i64>,
    /// The channel the single-tuner scan held when it went to look at the priority channel.
    resume: Option<Chan>,
    /// The steps of one pass: the step number and the item and block it holds, `None` for a step of silence.
    steps: Vec<(i64, Option<(usize, usize)>)>,
}

impl Player {
    /// A player on `plan`: no scan, dwell 0.4 s, hang 2 s.
    pub fn new(plan: Box<dyn ScanPlan>) -> Self {
        Player {
            plan,
            scan: false,
            dwell: 0.4,
            hang: 2.0,
            settle: 0.0,
            lag: 0.0,
            states: BTreeMap::new(),
            playing: None,
            interrupted: None,
            last_played: None,
            scan_state: ScanState::Idle,
            tune_list_pos: 0,
            tune_since: 0.0,
            last_probe: 0.0,
            tune: None,
            clock: 0.0,
            span: None,
            clock_base: 0.0,
            first_time: None,
            next_block: None,
            resume: None,
            steps: Vec::new(),
        }
    }

    /// Starts on a new capture that holds the channels from `span.0` to `span.1`. The channel states go (they belong to the old
    /// capture). The scan keeps its place and its clock.
    pub fn configure(&mut self, span: Option<(f64, f64)>) {
        self.states.clear();
        self.playing = None;
        self.interrupted = None;
        self.clock_base = self.clock;
        self.first_time = None;
        self.next_block = None;
        self.span = span;
    }

    /// The player's clock: signal time across retunes, s.
    pub fn clock(&self) -> f64 {
        self.clock
    }

    /// What plays now.
    pub fn state(&self) -> PlayState {
        PlayState {
            playing: self.playing,
            scan_state: self.scan_state,
            tune_hz: self.tune.map(|c| c.hz),
        }
    }

    fn is_pri(&self, c: Chan) -> bool {
        self.plan.is_priority(c)
    }

    fn active(&self, c: Chan, now: f64) -> bool {
        self.states
            .get(&c.key)
            .is_some_and(|s| s.open || now - s.last_open < self.hang)
    }

    fn choose(&mut self, now: f64) {
        let priority = self.plan.priority().is_some();
        let open_pri: Option<Chan> = self
            .states
            .iter()
            .map(|(k, s)| (Chan { key: *k, hz: s.hz }, s))
            .find(|(c, s)| s.open && self.is_pri(*c))
            .map(|(c, _)| c);
        // A picked channel.
        if !self.scan {
            if let Some(picked) = self.plan.picked() {
                let mut want = picked
                    .iter()
                    .copied()
                    .find(|c| self.states.contains_key(&c.key))
                    .or(picked.first().copied());
                if priority {
                    if let Some(p) = open_pri {
                        want = Some(p);
                    } else if let Some(c) = self.playing {
                        // Held through the hang of the priority channel, then back.
                        if self.is_pri(c) && self.active(c, now) {
                            want = Some(c);
                        }
                    }
                }
                self.playing = want;
                self.scan_state = ScanState::Idle;
                return;
            }
        }
        if priority {
            if let Some(p) = open_pri {
                if self.playing.map(|c| c.key) != Some(p.key) {
                    self.interrupted = self.playing.filter(|c| !self.is_pri(*c));
                }
                self.playing = Some(p);
                self.scan_state = ScanState::Priority;
                return;
            }
            // The scan holds the priority channel through its hang. Without the scan it is held like any channel (below).
            if let Some(c) = self.playing {
                if self.scan && self.is_pri(c) && self.active(c, now) {
                    self.scan_state = ScanState::Priority;
                    return;
                }
            }
            if let Some(i) = self.interrupted.take() {
                if self.active(i, now) {
                    self.playing = Some(i);
                    self.scan_state = if self.scan {
                        ScanState::Holding
                    } else {
                        ScanState::Idle
                    };
                    return;
                }
            }
        }
        if self.scan {
            // Whole span: hold until quiet for the hang time, then the first active channel after the one last played.
            if let Some(c) = self.playing {
                if self.active(c, now) {
                    self.scan_state = ScanState::Holding;
                    return;
                }
                self.last_played = Some(c);
            }
            let list = self.plan.scan_list();
            let start = self
                .last_played
                .and_then(|lp| list.iter().position(|c| c.key == lp.key))
                .map_or(0, |i| i + 1);
            self.playing = None;
            for k in 0..list.len() {
                let c = list[(start + k) % list.len()];
                if self.states.get(&c.key).is_some_and(|s| s.open) {
                    self.playing = Some(c);
                    break;
                }
            }
            self.scan_state = if self.playing.is_some() {
                ScanState::Holding
            } else {
                ScanState::Scanning
            };
            return;
        }
        // Nothing picked: the strongest open channel, sticky.
        let best = self
            .states
            .iter()
            .filter(|(k, s)| s.open && !self.plan.muted(Chan { key: **k, hz: s.hz }))
            .max_by(|a, b| a.1.level.total_cmp(&b.1.level))
            .map(|(k, s)| (Chan { key: *k, hz: s.hz }, s.level));
        match (self.playing, best) {
            (Some(cur), Some((b, bl))) => {
                let cs = self.states.get(&cur.key).cloned().unwrap_or_default();
                let closed_long = !cs.open && now - cs.last_open > 1.0;
                if b.key != cur.key && (bl > cs.level + 6.0 || closed_long) {
                    self.playing = Some(b);
                }
            }
            (None, Some((b, _))) => self.playing = Some(b),
            (Some(cur), None) => {
                let cs = self.states.get(&cur.key).cloned().unwrap_or_default();
                if now - cs.last_open > 1.0 {
                    self.playing = None;
                }
            }
            (None, None) => {}
        }
        self.scan_state = ScanState::Idle;
    }

    /// The single-tuner scan: the channel for the radio now. It tunes each listed channel for the dwell, stops on the first
    /// that is active and holds it until it has been quiet for the hang time. Every 2 s it visits the priority channel for
    /// 0.3 s (staying while that is active), then goes back to the channel it held or on with the list.
    fn single_tuner(&mut self, now: f64) -> Option<Chan> {
        let list = self.plan.scan_list();
        if list.is_empty() {
            return None;
        }
        let pri = self.plan.priority();
        let same = |a: Option<Chan>, b: Chan| a.is_some_and(|a| a.hz == b.hz);
        let cur = list[self.tune_list_pos % list.len()];
        let here = self.tune.unwrap_or(cur);
        let active_here = self.active(here, now);
        if self.scan_state == ScanState::PriorityProbe
            || (self.scan_state == ScanState::Priority && same(pri, here))
        {
            if active_here {
                self.scan_state = ScanState::Priority;
                return Some(here);
            }
            if self.scan_state == ScanState::Priority || now - self.tune_since >= 0.3 {
                self.tune_since = now;
                self.last_probe = now;
                return match self.resume.take() {
                    Some(c) => {
                        self.scan_state = ScanState::Holding;
                        // Held until quiet for the hang time, counted from now (its state was lost while away).
                        let s = self.states.entry(c.key).or_default();
                        s.hz = c.hz;
                        s.last_open = now;
                        Some(c)
                    }
                    None => {
                        self.scan_state = ScanState::Scanning;
                        Some(cur)
                    }
                };
            }
            return Some(here);
        }
        if let (Some(p), true) = (pri, now - self.last_probe >= 2.0) {
            if p.hz != here.hz {
                self.resume = active_here.then_some(here);
                self.scan_state = ScanState::PriorityProbe;
                self.tune_since = now;
                return Some(p);
            }
            self.last_probe = now;
        }
        if active_here {
            self.scan_state = if same(pri, here) {
                ScanState::Priority
            } else {
                ScanState::Holding
            };
            self.tune_since = now;
            return Some(here);
        }
        if now - self.tune_since >= self.dwell + self.settle {
            self.tune_list_pos = (self.tune_list_pos + 1) % list.len();
            self.tune_since = now;
        }
        self.scan_state = ScanState::Scanning;
        Some(list[self.tune_list_pos % list.len()])
    }

    /// Whether the scan must step the radio (single tuner): some channel of the scan list lies outside the capture. A capture that
    /// holds them all is scanned as a whole span, without stepping.
    pub fn single_span(&self) -> bool {
        if self.plan.always_single_tuner() {
            return true;
        }
        let Some((lo, hi)) = self.span else {
            return false;
        };
        self.plan
            .scan_list()
            .iter()
            .any(|c| c.hz < lo - 1.0 || c.hz > hi + 1.0)
    }

    /// Takes the blocks and the carrier lists of one pass and appends the audio of the channel that plays to `audio`: 10 ms for
    /// every step, silence when nothing plays. While no carrier is tracked (so no block comes) the steps follow the finder's
    /// estimates, so the scan keeps time and the audio stays steady. Returns whether what plays, the scan state or the radio
    /// frequency changed.
    pub fn process(
        &mut self,
        items: &[Timed<Blocks>],
        carriers: &[Timed<Carriers>],
        audio: &mut Vec<f32>,
    ) -> bool {
        let mut steps = std::mem::take(&mut self.steps);
        steps.clear();
        for (i, b) in items.iter().enumerate() {
            for (j, cb) in b.value.0.iter().enumerate() {
                steps.push((
                    (cb.time * AUDIO_RATE / BLOCK as f64).round() as i64,
                    Some((i, j)),
                ));
            }
        }
        for c in carriers {
            if c.value.0.is_empty() {
                // A margin of 50 ms for the delay of the channel receivers.
                let last = ((c.time - 0.05 - self.lag) * AUDIO_RATE / BLOCK as f64).floor() as i64;
                let from = self.next_block.unwrap_or(last);
                steps.extend((from..=last).map(|k| (k, None)));
            }
        }
        // A stable sort keeps the blocks of one step in the order they came.
        steps.sort_by_key(|s| s.0);
        let key_of = |c: Option<Chan>| c.map(|c| c.key);
        let before = (key_of(self.playing), self.scan_state, key_of(self.tune));
        let mut changed = false;
        let mut start = 0;
        while start < steps.len() {
            let k = steps[start].0;
            let end = start + steps[start..].iter().take_while(|s| s.0 == k).count();
            let group = &steps[start..end];
            start = end;
            let block = |s: &(i64, Option<(usize, usize)>)| -> Option<&ChanBlock> {
                s.1.map(|(i, j)| &items[i].value.0[j])
            };
            let t = k as f64 * BLOCK as f64 / AUDIO_RATE;
            let t0 = *self.first_time.get_or_insert(t);
            let now = self.clock_base + (t - t0);
            self.clock = now;
            for b in group.iter().filter_map(block) {
                let s = self.states.entry(b.key).or_default();
                s.hz = b.freq_hz;
                if b.open && !s.open {
                    s.since = now;
                }
                s.open = b.open;
                if b.open {
                    s.last_open = now;
                    s.level = b.level_dbfs;
                }
            }
            if self.scan && self.single_span() {
                self.tune = self.single_tuner(now);
                // The radio holds one channel at a time: it plays while it is active.
                self.playing = self
                    .tune
                    .filter(|c| self.states.get(&c.key).is_some_and(|s| s.open));
            } else {
                self.tune = None;
                self.choose(now);
            }
            changed |= (key_of(self.playing), self.scan_state, key_of(self.tune)) != before;
            // A step already played (a block late behind the silence) only updates the states.
            if self.next_block.is_some_and(|n| k < n) {
                continue;
            }
            self.next_block = Some(k + 1);
            let play = self
                .playing
                .and_then(|c| group.iter().filter_map(block).find(|b| b.key == c.key));
            match play {
                Some(b) => audio.extend_from_slice(&b.audio),
                None => audio.resize(audio.len() + BLOCK, 0.0),
            }
        }
        self.steps = steps;
        changed
    }
}
