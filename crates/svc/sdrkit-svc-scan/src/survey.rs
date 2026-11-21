//! The survey of a wide range in two stages. Stage one steps the radio in blocks of its widest rate and looks, without the
//! stack, for keyed carriers in spectrum frames of the whole block ([`KeyedSurvey`] with the settings for weak carriers), and
//! lists the occupied channels of the block (signals of any modulation a few kHz wide), which are not visited. Stage two visits each candidate as a span block centred on it and lets the stack's own result
//! decide: a find locks the scan; a rejection is skipped in later passes for a time.

use crate::hits::{Hit, HitLog};
use crate::host::{EndReason, Progress, ScanMachine, ScanState, Step, Target};
use crate::observe::Observation;
use crate::span::{carrier_offset, FindConfig, Judge, Judged, Watch};
use sdrkit_algo_dsp::approx::power_of_db;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::keysurvey::{Candidate, KeyedSurvey, SurveyConfig};
use sdrkit_algo_modem::occupancy::{OccupancyConfig, Occupied};
use sdrkit_core::Results;
use sdrkit_svc_sweep::{SpectrumFrame, SpectrumNeed};
use std::collections::VecDeque;

/// The part of a block, from its middle, in which candidates are taken: the edges are in the slope of the radio's filter.
pub const USED_SHARE: f64 = 0.9;
/// Candidates this close to the middle of a block are left out: a radio has a carrier of its own there, Hz.
pub const CENTRE_GUARD_HZ: f64 = 2e3;
/// The time of one spectrum frame that the survey asks for: the bins are its inverse apart, s.
pub const FRAME_S: f64 = 0.012;
/// The most bins of a frame.
pub const MAX_BINS: usize = 262_144;
/// The most candidates visited after one block.
pub const MOST_VISITS: usize = 16;
/// The most candidate rows kept.
pub const MAX_CANDIDATES: usize = 1000;
/// The most occupied channels kept.
pub const MAX_OCCUPIED: usize = 1000;
/// A stage-one block without frames for this many look times goes on without candidates.
const LOOK_WAIT: f64 = 3.0;

/// Whether a keyed carrier of `keyed` (offsets from `centre`, Hz) lies within half of `width_hz` of `freq_hz`.
fn holds_keyed(keyed: &[Candidate], centre: f64, freq_hz: f64, width_hz: f64) -> bool {
    keyed
        .iter()
        .any(|k| (centre + k.offset_hz - freq_hz).abs() <= 0.5 * width_hz)
}

sdrkit_core::choice_enum! {
    /// Where a candidate of a survey is.
    pub enum CandidateState {
        /// It waits for its visit.
        Waiting = ("waiting", "Waiting"),
        /// It is visited now.
        Visiting = ("visiting", "Visiting"),
        /// The stack found a signal on it.
        Found = ("found", "Found"),
        /// The stack found nothing on it: later candidates near it are skipped until `until`.
        Rejected = ("rejected", "Rejected"),
    }
}

/// One candidate of stage one and what its visit gave.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateRow {
    /// Its sequence number, from 0.
    pub seq: u64,
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// How well its keying fits Morse timing, 0 to 1.
    pub score: f64,
    /// Its speed, words a minute.
    pub wpm: f64,
    /// Its level over the noise while keyed, dB.
    pub snr_db: f64,
    /// The pass in which it was seen.
    pub pass: u32,
    /// When it was seen, seconds since 1970.
    pub seen_s: f64,
    /// Where it is.
    pub state: CandidateState,
    /// For a rejected candidate: until when others near it are skipped, seconds since 1970.
    pub until_s: Option<f64>,
}

/// An occupied channel that stage one saw: power over the noise across a few kHz to 30 kHz, of any modulation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OccupiedChannel {
    /// Its number, from 0: it stays when the channel is seen again.
    pub id: u64,
    /// The centre of its power, Hz.
    pub freq_hz: f64,
    /// Its width, Hz.
    pub width_hz: f64,
    /// Its power over the noise in its width, dB.
    pub snr_db: f64,
    /// The pass in which it was seen last.
    pub pass: u32,
    /// The passes that saw it.
    pub seen: u32,
}

/// The settings of a survey.
#[derive(Clone, Debug, PartialEq)]
pub struct SurveyScanConfig {
    /// The low edge of the range, Hz.
    pub start_hz: f64,
    /// The high edge of the range, Hz.
    pub stop_hz: f64,
    /// The widest rate of the radio, Hz: stage one takes blocks of [`USED_SHARE`] of it.
    pub rate_hz: f64,
    /// How long stage one looks at a block, s.
    pub look_s: f64,
    /// How long a rejected candidate keeps others near it from a visit, s.
    pub reject_hold_s: f64,
    /// The test of a visit and the lock.
    pub find: FindConfig,
}

impl SurveyScanConfig {
    /// The block width of stage one, Hz.
    pub fn block_hz(&self) -> f64 {
        self.rate_hz * USED_SHARE
    }

    /// The spectrum frames that stage one needs: back to back, about [`FRAME_S`] long, at the widest rate.
    pub fn need(&self) -> SpectrumNeed {
        let want = (self.rate_hz * FRAME_S).max(256.0);
        let bins = (want.log2().round().exp2() as usize).clamp(256, MAX_BINS);
        SpectrumNeed {
            bins,
            window: Window::BlackmanHarris4,
            rate_hz: self.rate_hz,
            average: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Stage one: frames are taken from `from` on until the look is over or `deadline` passes.
    Look { from: f64, deadline: f64 },
    /// Stage two: the candidate `seq` is visited.
    Visit { seq: u64, judge: Judge },
}

/// A two-stage survey.
pub struct SurveyScanner {
    cfg: SurveyScanConfig,
    need: SpectrumNeed,
    blocks: Vec<f64>,
    index: usize,
    pass: u32,
    steps: u64,
    found: usize,
    phase: Option<Phase>,
    detector: KeyedSurvey,
    look_frames: u64,
    found_now: Vec<Candidate>,
    occupied_now: Vec<Occupied>,
    occupied: VecDeque<OccupiedChannel>,
    next_occupied: u64,
    queue: VecDeque<u64>,
    rows: VecDeque<CandidateRow>,
    next_seq: u64,
    revision: u64,
    unseen: u64,
    watch: Watch,
    hits: HitLog,
    open: Option<u64>,
    start_s: f64,
    now_s: f64,
    paused: bool,
    end: EndReason,
    value: Option<f64>,
}

impl std::fmt::Debug for SurveyScanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurveyScanner")
            .field("blocks", &self.blocks.len())
            .field("index", &self.index)
            .field("pass", &self.pass)
            .field("phase", &self.phase)
            .finish()
    }
}

impl SurveyScanner {
    /// A survey with `cfg`. Errors when the frames of the widest rate do not make a detector.
    pub fn new(cfg: SurveyScanConfig) -> Result<SurveyScanner, sdrkit_algo_modem::ModemError> {
        let need = cfg.need();
        let frame_s = need.bins as f64 / cfg.rate_hz;
        let detector = KeyedSurvey::new(SurveyConfig {
            occupancy: Some(OccupancyConfig::default()),
            ..SurveyConfig::paired(
                need.bins,
                cfg.rate_hz / need.bins as f64,
                frame_s,
                cfg.look_s,
            )
        })?;
        let look_frames = (cfg.look_s / frame_s).ceil().max(1.0) as u64;
        Ok(SurveyScanner {
            blocks: crate::span::blocks(cfg.start_hz, cfg.stop_hz, cfg.block_hz()),
            need,
            index: 0,
            pass: 0,
            steps: 0,
            found: 0,
            phase: None,
            detector,
            look_frames,
            found_now: Vec::new(),
            occupied_now: Vec::new(),
            occupied: VecDeque::new(),
            next_occupied: 0,
            queue: VecDeque::new(),
            rows: VecDeque::new(),
            next_seq: 0,
            revision: 0,
            unseen: 0,
            watch: Watch::new(&cfg.find.watch),
            hits: HitLog::new(cfg.find.hit_cap),
            open: None,
            start_s: 0.0,
            now_s: 0.0,
            paused: false,
            end: EndReason::None,
            value: None,
            cfg,
        })
    }

    /// The centres of the stage-one blocks, Hz.
    pub fn blocks(&self) -> &[f64] {
        &self.blocks
    }

    /// The finds.
    pub fn hits(&self) -> &HitLog {
        &self.hits
    }

    /// The candidates, oldest first.
    pub fn candidates(&self) -> impl Iterator<Item = &CandidateRow> {
        self.rows.iter()
    }

    /// The occupied channels that stage one saw, in the order of their first sight: one row for each channel, with the
    /// values of the look that saw it last. They are not candidates of a visit.
    pub fn occupied(&self) -> impl Iterator<Item = &OccupiedChannel> {
        self.occupied.iter()
    }

    /// A count that grows with every change of the candidates.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The candidates that wait for a visit.
    pub fn waiting(&self) -> usize {
        self.queue.len()
    }

    /// The stage-one blocks that ended without a spectrum frame.
    pub fn unseen(&self) -> u64 {
        self.unseen
    }

    /// The last watched value read on a visit.
    pub fn watched(&self) -> Option<f64> {
        self.value
    }

    /// The time left, s, if the scan has a total time.
    pub fn time_left(&self, now: f64) -> Option<f64> {
        (self.cfg.find.total_s > 0.0).then(|| (self.start_s + self.cfg.find.total_s - now).max(0.0))
    }

    /// The spectrum frames that the survey needs while it runs.
    pub fn need(&self) -> Option<SpectrumNeed> {
        (self.end == EndReason::None).then_some(self.need)
    }

    /// Takes a spectrum frame of the block tuned now. A frame of another centre, rate or size is left.
    pub fn frame(&mut self, f: &SpectrumFrame<'_>) {
        let Some(Phase::Look { from, .. }) = self.phase else {
            return;
        };
        let Some(&centre) = self.blocks.get(self.index) else {
            return;
        };
        let same = (f.centre_hz - centre).abs() < 1.0
            && (f.span_hz - self.cfg.rate_hz).abs() < 1.0
            && f.bins_db.len() == self.need.bins;
        if !same || self.paused || self.now_s < from {
            return;
        }
        if self.detector.frames() < self.look_frames {
            // The size was checked above.
            let _ = self.detector.push_with(f.bins_db, power_of_db);
        }
    }

    /// Attaches `text` to the find `seq`, or to the newest find. False if there is no such find.
    pub fn set_text(&mut self, seq: Option<u64>, text: &str) -> bool {
        let seq = seq.or_else(|| self.hits.pushed().checked_sub(1));
        match seq.and_then(|s| self.hits.get_mut(s)) {
            Some(h) => {
                h.text = text.to_owned();
                true
            }
            None => false,
        }
    }

    /// Ends the scan.
    pub fn stop(&mut self, now: f64) -> Step {
        self.close(now);
        self.end = EndReason::Stopped;
        self.phase = None;
        Step::Done
    }

    /// Leaves the block or the candidate now. `None` once the scan has ended.
    pub fn skip(&mut self, now: f64) -> Option<Step> {
        if self.end != EndReason::None {
            return None;
        }
        self.paused = false;
        Some(self.leave(now))
    }

    /// Rejects the candidate visited now and goes on. `None` when no candidate is visited.
    pub fn reject(&mut self, now: f64) -> Option<Step> {
        let Some(Phase::Visit { seq, .. }) = self.phase else {
            return None;
        };
        self.mark(seq, CandidateState::Rejected, now);
        self.paused = false;
        Some(self.after_visit(now))
    }

    /// Holds the block or the candidate that is tuned.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Goes on after a pause or a find that stays.
    pub fn resume(&mut self, now: f64) -> Option<Step> {
        let locked = matches!(self.phase, Some(Phase::Visit { judge, .. }) if judge.locked());
        let held = self.paused || locked;
        self.paused = false;
        if self.end != EndReason::None || !held {
            return None;
        }
        if locked {
            return Some(self.after_visit(now));
        }
        None
    }

    fn time_over(&self, now: f64) -> bool {
        self.cfg.find.total_s > 0.0 && now - self.start_s >= self.cfg.find.total_s
    }

    fn close(&mut self, now: f64) {
        if let Some(seq) = self.open.take() {
            let off = self.cfg.find.clock_offset_s;
            if let Some(h) = self.hits.get_mut(seq) {
                h.end_s = now + off;
            }
        }
    }

    fn row_mut(&mut self, seq: u64) -> Option<&mut CandidateRow> {
        let first = self.rows.front()?.seq;
        let i = usize::try_from(seq.checked_sub(first)?).ok()?;
        self.rows.get_mut(i)
    }

    fn mark(&mut self, seq: u64, state: CandidateState, now: f64) {
        let until = (state == CandidateState::Rejected)
            .then_some(now + self.cfg.reject_hold_s + self.cfg.find.clock_offset_s);
        if let Some(r) = self.row_mut(seq) {
            r.state = state;
            r.until_s = until;
        }
        self.revision += 1;
    }

    /// Leaves what is tuned: a block goes on to its candidates or the next block, a visit to the next candidate.
    fn leave(&mut self, now: f64) -> Step {
        match self.phase {
            Some(Phase::Look { .. }) => self.end_look(now),
            Some(Phase::Visit { seq, judge }) => {
                if !judge.locked() {
                    self.mark(seq, CandidateState::Rejected, now);
                }
                self.after_visit(now)
            }
            None => self.look(now),
        }
    }

    /// The candidates of the block just looked at go into the queue; then the first visit, or the next block.
    fn end_look(&mut self, now: f64) -> Step {
        if self.detector.frames() == 0 {
            self.unseen += 1;
        }
        self.detector.candidates(&mut self.found_now);
        let centre = self.blocks[self.index];
        let half = self.cfg.block_hz() / 2.0;
        let skip_hz = 3.0 * self.cfg.rate_hz / self.need.bins as f64;
        let wall = now + self.cfg.find.clock_offset_s;
        let mut taken = 0;
        let found_now = std::mem::take(&mut self.found_now);
        for c in &found_now {
            if taken >= MOST_VISITS {
                break;
            }
            let off = c.offset_hz;
            if off.abs() > half || off.abs() < CENTRE_GUARD_HZ {
                continue;
            }
            let f = centre + off;
            let near = |r: &CandidateRow| (r.freq_hz - f).abs() <= skip_hz;
            let held = self.rows.iter().any(|r| {
                near(r)
                    && match r.state {
                        CandidateState::Rejected => r.until_s.is_some_and(|u| u > wall),
                        CandidateState::Waiting | CandidateState::Visiting => true,
                        CandidateState::Found => false,
                    }
            });
            if held {
                continue;
            }
            let seq = self.next_seq;
            self.next_seq += 1;
            self.rows.push_back(CandidateRow {
                seq,
                freq_hz: f,
                score: f64::from(c.score),
                wpm: f64::from(c.wpm),
                snr_db: f64::from(c.snr_db),
                pass: self.pass,
                seen_s: wall,
                state: CandidateState::Waiting,
                until_s: None,
            });
            while self.rows.len() > MAX_CANDIDATES {
                self.rows.pop_front();
            }
            self.queue.push_back(seq);
            taken += 1;
        }
        self.found_now = found_now;
        self.list_occupied(centre, half);
        self.revision += 1;
        self.after_visit(now)
    }

    /// The occupied channels of the block just looked at, around `centre` and within `half` of it, go into the list. A
    /// channel within half its width (or half the width of a row, if that is more) of a row is that channel seen again:
    /// the row takes its values and counts the pass, and other rows that it overlaps go. A channel or a row that holds a
    /// keyed carrier of the same look within half its width is that carrier and is left out or goes.
    fn list_occupied(&mut self, centre: f64, half: f64) {
        self.detector.occupied(&mut self.occupied_now);
        let pass = self.pass;
        let keyed = &self.found_now;
        self.occupied
            .retain(|r| !holds_keyed(keyed, centre, r.freq_hz, r.width_hz));
        for c in &self.occupied_now {
            if c.offset_hz.abs() > half || c.offset_hz.abs() < CENTRE_GUARD_HZ {
                continue;
            }
            let f = centre + c.offset_hz;
            if holds_keyed(keyed, centre, f, c.width_hz) {
                continue;
            }
            let same =
                |r: &OccupiedChannel| (r.freq_hz - f).abs() <= 0.5 * r.width_hz.max(c.width_hz);
            let (id, seen) = match self.occupied.iter().find(|r| same(r)) {
                Some(r) => (r.id, r.seen + u32::from(r.pass != pass)),
                None => {
                    self.next_occupied += 1;
                    (self.next_occupied - 1, 1)
                }
            };
            let row = OccupiedChannel {
                id,
                freq_hz: f,
                width_hz: c.width_hz,
                snr_db: f64::from(c.snr_db),
                pass,
                seen,
            };
            match self.occupied.iter().position(|r| r.id == id) {
                Some(i) => self.occupied[i] = row,
                None => {
                    if self.occupied.len() >= MAX_OCCUPIED {
                        self.occupied.pop_front();
                    }
                    self.occupied.push_back(row);
                }
            }
            self.occupied.retain(|r| r.id == id || !same(r));
        }
    }

    /// The next waiting candidate, or the next block.
    fn after_visit(&mut self, now: f64) -> Step {
        self.close(now);
        if self.time_over(now) {
            self.end = EndReason::TimeOver;
            self.phase = None;
            return Step::Done;
        }
        while let Some(seq) = self.queue.pop_front() {
            if self.row_mut(seq).is_some() {
                self.mark(seq, CandidateState::Visiting, now);
                self.steps += 1;
                self.value = None;
                self.phase = Some(Phase::Visit {
                    seq,
                    judge: Judge::tuned(now, &self.cfg.find),
                });
                return Step::Tune;
            }
        }
        if matches!(
            self.phase,
            Some(Phase::Look { .. }) | Some(Phase::Visit { .. })
        ) {
            self.index += 1;
            if self.index >= self.blocks.len() {
                self.index = 0;
                self.pass += 1;
            }
        }
        self.look(now)
    }

    /// Tunes the block `index` for stage one.
    fn look(&mut self, now: f64) -> Step {
        self.steps += 1;
        self.detector.reset();
        let from = now + self.cfg.find.settle_s;
        let frame_s = self.need.bins as f64 / self.cfg.rate_hz;
        let wait = (self.cfg.look_s + frame_s) * LOOK_WAIT;
        self.phase = Some(Phase::Look {
            from,
            deadline: from + wait,
        });
        Step::Tune
    }
}

impl ScanMachine for SurveyScanner {
    fn start(&mut self, now_s: f64) -> Step {
        self.start_s = now_s;
        self.now_s = now_s;
        if self.blocks.is_empty() {
            self.end = EndReason::NothingToScan;
            return Step::Done;
        }
        self.look(now_s)
    }

    fn tick(&mut self, now_s: f64, _obs: &Observation, results: Option<&Results>) -> Option<Step> {
        self.now_s = now_s;
        let phase = self.phase?;
        if self.paused {
            return None;
        }
        match phase {
            Phase::Look { deadline, .. } => {
                if self.time_over(now_s) {
                    return Some(self.after_visit(now_s));
                }
                if self.detector.frames() >= self.look_frames || now_s >= deadline {
                    return Some(self.end_look(now_s));
                }
                None
            }
            Phase::Visit { seq, mut judge } => {
                let value = results.map(|r| self.watch.read(r));
                if value.is_some() {
                    self.value = value;
                }
                let out = judge.tick(now_s, value, &self.cfg.find);
                self.phase = Some(Phase::Visit { seq, judge });
                match out {
                    Judged::Wait if self.time_over(now_s) && !judge.locked() => {
                        Some(self.leave(now_s))
                    }
                    Judged::Wait => None,
                    Judged::Found(_) => {
                        self.found += 1;
                        self.mark(seq, CandidateState::Found, now_s);
                        let off = self.cfg.find.clock_offset_s;
                        let freq_hz =
                            self.row_mut(seq).map_or(0.0, |r| r.freq_hz) + carrier_offset(results);
                        self.open = Some(self.hits.pushed());
                        self.hits.push(Hit {
                            start_s: now_s + off,
                            end_s: now_s + off,
                            freq_hz,
                            stack: self.cfg.find.stack.clone(),
                            name: String::new(),
                            tone: String::new(),
                            peak_snr_db: None,
                            priority: false,
                            memory_id: None,
                            index: self.index,
                            pass: self.pass,
                            text: String::new(),
                        });
                        None
                    }
                    Judged::Next { .. } => Some(self.leave(now_s)),
                }
            }
        }
    }

    fn target(&self) -> Option<Target<'_>> {
        match self.phase? {
            Phase::Look { .. } => Some(Target {
                freq_hz: *self.blocks.get(self.index)?,
                stack: &self.cfg.find.stack,
                submode: "",
                name: "",
                settings: &[],
                bandwidth_hz: None,
                rate_hz: Some(self.cfg.rate_hz),
            }),
            Phase::Visit { seq, .. } => {
                let first = self.rows.front()?.seq;
                let row = self
                    .rows
                    .get(usize::try_from(seq.checked_sub(first)?).ok()?)?;
                Some(Target {
                    freq_hz: row.freq_hz,
                    stack: &self.cfg.find.stack,
                    submode: "",
                    name: "",
                    settings: &[],
                    bandwidth_hz: None,
                    rate_hz: None,
                })
            }
        }
    }

    fn audio_wanted(&self) -> bool {
        self.paused || matches!(self.phase, Some(Phase::Visit { judge, .. }) if judge.locked())
    }

    fn finished(&self) -> bool {
        self.end != EndReason::None
    }

    fn progress(&self) -> Progress {
        let state = match (self.end, self.phase) {
            (EndReason::None, _) if self.paused => ScanState::Paused,
            (EndReason::None, Some(Phase::Look { from, .. })) if self.now_s < from => {
                ScanState::Settle
            }
            (EndReason::None, Some(Phase::Look { .. })) => ScanState::Look,
            (EndReason::None, Some(Phase::Visit { judge, .. })) => match judge.state() {
                ScanState::Listen => ScanState::Test,
                s => s,
            },
            (EndReason::None, None) => ScanState::Settle,
            _ => ScanState::Done,
        };
        Progress {
            state,
            index: self.index,
            count: self.blocks.len(),
            at_priority: false,
            pass: self.pass,
            steps: self.steps,
            found: self.found,
            end: self.end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_channel_that_holds_a_keyed_carrier_of_the_look_is_that_carrier() {
        let keyed = |offset_hz| Candidate {
            offset_hz,
            bin: 0,
            score: 0.9,
            wpm: 20.0,
            snr_db: 30.0,
            marks: 20,
            duty: 0.4,
        };
        let centre = 300e6;
        let found = [keyed(-40e3), keyed(12e3)];
        assert!(holds_keyed(&found, centre, centre + 14.4e3, 5e3));
        assert!(holds_keyed(&found, centre, centre - 40e3, 2e3));
        assert!(!holds_keyed(&found, centre, centre + 14.6e3, 5e3));
        assert!(!holds_keyed(&[], centre, centre + 12e3, 5e3));
    }
}
