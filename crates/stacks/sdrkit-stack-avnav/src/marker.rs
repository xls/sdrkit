//! ILS marker beacons on 75 MHz: the tone detector that names the marker (outer, middle, inner) from its tone, measures the
//! keying and says whether it fits the standard, and the test signal.
//! Standard: ICAO Annex 10 Volume I, 3.1.7 (carrier 3.1.7.2, tones and depth 3.1.7.4, keying 3.1.7.5).
//! Not confirmed by a public document (the standard gives only the rates): the lengths of the dot (0.08 s) and the dash
//! (0.30 s) of the test signal, and the element limits of the detector (outer elements at least 0.15 s, inner at most 0.14 s,
//! middle elements shorter than 0.6 s, mixed keying when the sorted element lengths step up by a ratio of 1.8).

use crate::typed::{MarkerEvent, MARKER_EVENT};
use sdrkit_algo_dsp::goertzel::ToneBank;
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::keys;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, EventDef, Generator, Record,
    SignalInfo, Values,
};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, DecoderCommand, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use std::f64::consts::{PI, TAU};
use std::fmt::Write as _;

/// The marker carrier, Hz (3.1.7.2.1).
pub const CARRIER_HZ: f64 = 75e6;
/// The tones of the outer, middle and inner marker, Hz (3.1.7.4.1).
pub const TONES_HZ: [f64; 3] = [400.0, 1300.0, 3000.0];
/// The nominal modulation depth (3.1.7.4.2).
pub const NOMINAL_DEPTH: f64 = 0.95;
/// The length of a measuring period, s.
pub const PERIOD_S: f64 = 0.01;
/// The width of the test signal that a signal to noise ratio is measured in, Hz.
pub const SIGNAL_WIDTH_HZ: f64 = 12e3;

/// A keyed period: the strongest tone has at least this depth.
const KEYED_DEPTH: f64 = 0.30;
/// A keyed period: the strongest tone has at least this many times the depth of each other tone.
const KEYED_RATIO: f64 = 3.0;
/// A passage ends this long after its last keyed period, s.
const PASSAGE_END_S: f64 = 1.5;
/// A passage of fewer elements is dropped.
const MIN_ELEMENTS: usize = 3;
/// The elements a passage keeps for its analysis: the last ones.
const RING: usize = 64;
/// Keying is mixed (dots and dashes) when the sorted element lengths step up by at least this ratio.
const MIXED_RATIO: f64 = 1.8;
/// An element at least this long is a dash when the keying is not mixed, s.
const DASH_S: f64 = 0.18;
/// The keying rates of the standard and their tolerance of plus or minus 15 % (3.1.7.5.1), a second.
const OUTER_RATE: (f64, f64) = (1.7, 2.3);
const INNER_RATE: (f64, f64) = (5.1, 6.9);
/// The dot-dash pairs a minute of a middle marker that fit: the rates of 3.1.7.5.1 b with their tolerance, read either as a
/// dash slot of 1/2 s and a dot slot of 1/6 s (90 a minute) or as one pair for each dash (120 a minute).
const MIDDLE_PAIRS: (f64, f64) = (75.0, 140.0);
/// Element limits of the detector, s.
const OUTER_SHORTEST_S: f64 = 0.15;
const INNER_SHORTEST_S: f64 = 0.14;
const MIDDLE_LONGEST_S: f64 = 0.6;
/// The live indication: a tone is on above the first depth and off below the second.
const LIVE_ON: f64 = 0.35;
const LIVE_OFF: f64 = 0.20;
/// The live indication holds the last this many on-lengths of each tone.
const LIVE_LENGTHS: usize = 8;
/// The live indication: a held depth falls by this factor each 25 ms.
const LIVE_DECAY_25MS: f64 = 0.97;

sdrkit_core::choice_enum! {
    /// A marker beacon, named by its tone.
    pub enum MarkerKind {
        /// No marker tone.
        None = ("none", "None"),
        /// 400 Hz.
        Outer = ("outer", "Outer"),
        /// 1300 Hz.
        Middle = ("middle", "Middle"),
        /// 3000 Hz.
        Inner = ("inner", "Inner"),
    }
}

impl MarkerKind {
    /// The marker of tone `i` of [`TONES_HZ`].
    pub fn of_tone(i: usize) -> MarkerKind {
        match i {
            0 => MarkerKind::Outer,
            1 => MarkerKind::Middle,
            2 => MarkerKind::Inner,
            _ => MarkerKind::None,
        }
    }
    /// The nominal tone, Hz.
    pub fn tone_hz(self) -> Option<f64> {
        match self {
            MarkerKind::None => None,
            MarkerKind::Outer => Some(TONES_HZ[0]),
            MarkerKind::Middle => Some(TONES_HZ[1]),
            MarkerKind::Inner => Some(TONES_HZ[2]),
        }
    }
    /// The name of the beacon ("Outer marker").
    pub fn name(self) -> &'static str {
        match self {
            MarkerKind::None => "No marker",
            MarkerKind::Outer => "Outer marker",
            MarkerKind::Middle => "Middle marker",
            MarkerKind::Inner => "Inner marker",
        }
    }
    /// The live pattern of the standard's keying.
    fn pattern(self) -> MarkerPattern {
        match self {
            MarkerKind::None => MarkerPattern::NotYet,
            MarkerKind::Outer => MarkerPattern::Dashes,
            MarkerKind::Middle => MarkerPattern::DotDash,
            MarkerKind::Inner => MarkerPattern::Dots,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The keying pattern that the live indication hears.
    pub enum MarkerPattern {
        /// Fewer than four elements heard.
        NotYet = ("not_yet", "Not yet"),
        /// Dashes only.
        Dashes = ("dashes", "Dashes"),
        /// Dots and dashes.
        DotDash = ("dot_dash", "Dot-dash"),
        /// Dots only.
        Dots = ("dots", "Dots"),
        /// Elements that are neither dots nor dashes.
        Irregular = ("irregular", "No pattern"),
    }
}

sdrkit_core::choice_enum! {
    /// What a finished passage is.
    pub enum PassageKind {
        /// A marker tone keyed as the standard says.
        Marker = ("marker", "Marker"),
        /// A marker tone whose keying does not fit the standard.
        ToneOnly = ("tone_only", "Tone only"),
    }
}

sdrkit_core::choice_enum! {
    /// The marker of the test signal.
    pub enum GenMarker {
        /// 400 Hz, two dashes a second.
        Outer = ("outer", "Outer"),
        /// 1300 Hz, alternate dots and dashes.
        Middle = ("middle", "Middle"),
        /// 3000 Hz, six dots a second.
        Inner = ("inner", "Inner"),
    }
}

impl GenMarker {
    /// The marker kind.
    pub fn kind(self) -> MarkerKind {
        match self {
            GenMarker::Outer => MarkerKind::Outer,
            GenMarker::Middle => MarkerKind::Middle,
            GenMarker::Inner => MarkerKind::Inner,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How the test signal keys its tone.
    pub enum GenMarkerKeying {
        /// The keying of the marker.
        Standard = ("standard", "As the standard"),
        /// The tone without keying.
        Steady = ("steady", "Steady tone"),
    }
}

/// What the detector measured of a passage (or of the passage so far).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerSummary {
    /// The marker of the tone.
    pub marker: MarkerKind,
    /// The tone, measured from the phase advance between keyed periods, Hz.
    pub tone_hz: f64,
    /// The modulation depth of the tone, 0..1, over the keyed periods inside the elements.
    pub depth: f64,
    /// Elements in the passage.
    pub elements: u32,
    /// Dots and dashes among the elements analysed (the last 64).
    pub dots: u32,
    /// See `dots`.
    pub dashes: u32,
    /// Dots and dashes: the sorted element lengths step up by a ratio of 1.8 or more.
    pub mixed: bool,
    /// Elements a second, from the first onset to the last.
    pub per_s: f64,
    /// Dot-dash pairs a minute, from the first dash onset to the last: mixed keying with two dashes or more.
    pub pairs_per_min: Option<f64>,
    /// The mean dot length, s.
    pub dot_s: Option<f64>,
    /// The mean dash length, s.
    pub dash_s: Option<f64>,
    /// The shortest and the longest element, s, leaving out the outer tenth.
    pub shortest_s: f64,
    /// See `shortest_s`.
    pub longest_s: f64,
    /// From the start of the first keyed period to the end of the last, s.
    pub duration_s: f64,
    /// The end time of the last keyed period, s.
    pub end_s: f64,
    /// The keying fits the standard.
    pub fits: bool,
}

impl MarkerSummary {
    /// The keying as text: "2.0 dashes per second (0.30 s long)", "6.0 dots per second (0.08 s long)" or "90 dot-dash pairs per
    /// minute (dot 0.08 s, dash 0.30 s)".
    pub fn write_keying(&self, out: &mut String) {
        // Writing into a String cannot fail.
        let _ = match (self.pairs_per_min, self.dot_s, self.dash_s) {
            (Some(p), Some(dot), Some(dash)) => write!(
                out,
                "{p:.0} dot-dash pairs per minute (dot {dot:.2} s, dash {dash:.2} s)"
            ),
            (_, None, Some(dash)) => write!(
                out,
                "{:.1} dashes per second ({dash:.2} s long)",
                self.per_s
            ),
            (_, Some(dot), None) => {
                write!(out, "{:.1} dots per second ({dot:.2} s long)", self.per_s)
            }
            _ => write!(out, "{:.1} elements per second", self.per_s),
        };
    }
    /// The text of a passage row: "400 Hz, 2.0 dashes per second (0.30 s long), depth 95 %, 7.8 s", with "; the keying does not
    /// fit the standard" when it does not.
    pub fn write_row(&self, out: &mut String) {
        let _ = write!(out, "{:.0} Hz, ", self.tone_hz);
        self.write_keying(out);
        let _ = write!(
            out,
            ", depth {:.0} %, {:.1} s",
            self.depth * 100.0,
            self.duration_s
        );
        if !self.fits {
            out.push_str("; the keying does not fit the standard");
        }
    }
    /// What the passage is.
    pub fn kind(&self) -> PassageKind {
        if self.fits {
            PassageKind::Marker
        } else {
            PassageKind::ToneOnly
        }
    }
}

/// The detector's state, sent each 0.1 s.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerState {
    /// The passage under way, once it has three elements.
    pub current: Option<MarkerSummary>,
    /// The tone with the greatest held depth, `None` below 0.2.
    pub live: MarkerKind,
    /// That tone keys the last period (depth over 0.3).
    pub keyed_now: bool,
    /// Its held depth, 0..1.
    pub held_depth: f64,
    /// The pattern of its last on-lengths.
    pub pattern: MarkerPattern,
}

impl Item for MarkerState {
    const KIND: &'static str = "ils.marker_state";
}

/// A finished passage. It leaves the graph as the event `ils.marker`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerPassage(pub MarkerSummary);

impl Item for MarkerPassage {
    const KIND: &'static str = "ils.marker";
    fn event_def() -> Option<&'static EventDef> {
        Some(MARKER_EVENT.def())
    }
    fn write_fields(&self, r: &mut Record) {
        let s = &self.0;
        let mut text = String::new();
        s.write_row(&mut text);
        MarkerEvent {
            source: s.marker.name(),
            text: &text,
            family: keys::Family::Other,
            kind: s.kind(),
            marker: s.marker,
            tone_hz: Hertz(s.tone_hz),
            depth_pct: s.depth * 100.0,
            duration_s: s.duration_s,
        }
        .write(r);
    }
}

/// One element: the end time of its first keyed period and its length in periods.
#[derive(Clone, Copy, Debug, Default)]
struct Element {
    onset: f64,
    periods: u32,
}

/// A passage under way: a run of keyed periods of one tone with gaps shorter than 1.5 s.
#[derive(Clone, Debug)]
struct Passage {
    tone: usize,
    first: f64,
    last: f64,
    ring: [Element; RING],
    head: usize,
    held: usize,
    total: u32,
    run: u32,
    run_onset: f64,
    run_sum: f64,
    run_first: f64,
    run_last: f64,
    prev: Option<Complex32>,
    inner_depth: (f64, u32),
    edge_depth: (f64, u32),
    offset_hz: (f64, u32),
}

impl Passage {
    fn new(tone: usize, t: f64) -> Self {
        Passage {
            tone,
            first: t,
            last: t,
            ring: [Element::default(); RING],
            head: 0,
            held: 0,
            total: 0,
            run: 0,
            run_onset: t,
            run_sum: 0.0,
            run_first: 0.0,
            run_last: 0.0,
            prev: None,
            inner_depth: (0.0, 0),
            edge_depth: (0.0, 0),
            offset_hz: (0.0, 0),
        }
    }

    /// A keyed period ending at `t` with the tone's phasor `p` and depth `d`. `advance` is the phase a period turns at the
    /// nominal tone, rad.
    fn keyed(&mut self, t: f64, p: Complex32, d: f64, advance: f64, period_s: f64) {
        if self.run == 0 {
            self.run_onset = t;
            self.run_sum = 0.0;
            self.run_first = d;
        } else if let Some(q) = self.prev {
            let turn = (p * q.conj()).arg() as f64 - advance;
            let turn = (turn + PI).rem_euclid(TAU) - PI;
            self.offset_hz.0 += turn / (TAU * period_s);
            self.offset_hz.1 += 1;
        }
        self.run += 1;
        self.run_sum += d;
        self.run_last = d;
        self.prev = Some(p);
        self.last = t;
    }

    /// Ends the element under way, if any.
    fn close(&mut self) {
        if self.run == 0 {
            return;
        }
        self.ring[self.head] = Element {
            onset: self.run_onset,
            periods: self.run,
        };
        self.head = (self.head + 1) % RING;
        self.held = (self.held + 1).min(RING);
        self.total += 1;
        // The first and last period of an element hold part of the tone: the depth is read from the periods between.
        if self.run >= 3 {
            self.inner_depth.0 += self.run_sum - self.run_first - self.run_last;
            self.inner_depth.1 += self.run - 2;
        } else {
            self.edge_depth.0 += self.run_sum;
            self.edge_depth.1 += self.run;
        }
        self.run = 0;
        self.prev = None;
    }

    /// The analysis of the closed elements, or `None` with fewer than three.
    fn summary(&self, period_s: f64) -> Option<MarkerSummary> {
        if self.held < MIN_ELEMENTS {
            return None;
        }
        let n = self.held;
        let elements = || (0..n).map(|i| self.ring[(self.head + RING - n + i) % RING]);
        let mut sorted = [0u32; RING];
        for (s, e) in sorted.iter_mut().zip(elements()) {
            *s = e.periods.max(1);
        }
        let sorted = &mut sorted[..n];
        sorted.sort_unstable();
        // The shortest and the longest leave out the outer tenth, so one element cut by the start of the input or by a fade
        // does not decide.
        let trim = n / 10;
        let shortest = sorted[trim];
        let longest = sorted[n - 1 - trim];
        // Mixed keying: the greatest step between neighbours of the sorted lengths, with a fifth of the elements (one at the
        // least) on each side, is a ratio of 1.8 or more. The longer side are the dashes.
        let least = (n / 5).max(1);
        let (ratio, split) = (least - 1..n - least)
            .map(|i| (f64::from(sorted[i + 1]) / f64::from(sorted[i]), sorted[i + 1]))
            .fold((1.0, u32::MAX), |a, b| if b.0 > a.0 { b } else { a });
        let mixed = ratio >= MIXED_RATIO;
        let mean = elements().map(|e| f64::from(e.periods)).sum::<f64>() / n as f64;
        let is_dash = |e: &Element| {
            if mixed {
                e.periods >= split
            } else {
                mean * period_s >= DASH_S
            }
        };
        let (mut dots, mut dashes, mut dot_len, mut dash_len) = (0u32, 0u32, 0u64, 0u64);
        let (mut first_dash, mut last_dash) = (None, 0.0);
        for e in elements() {
            if is_dash(&e) {
                dashes += 1;
                dash_len += u64::from(e.periods);
                first_dash.get_or_insert(e.onset);
                last_dash = e.onset;
            } else {
                dots += 1;
                dot_len += u64::from(e.periods);
            }
        }
        let first_onset = elements().next().map_or(0.0, |e| e.onset);
        let last_onset = elements().next_back().map_or(0.0, |e| e.onset);
        let span = last_onset - first_onset;
        let per_s = if span > 0.0 {
            (n - 1) as f64 / span
        } else {
            0.0
        };
        let pairs_per_min = match first_dash {
            Some(f) if mixed && dashes >= 2 && last_dash > f => {
                Some(f64::from(dashes - 1) * 60.0 / (last_dash - f))
            }
            _ => None,
        };
        let dot_s = (dots > 0).then(|| dot_len as f64 / f64::from(dots) * period_s);
        let dash_s = (dashes > 0).then(|| dash_len as f64 / f64::from(dashes) * period_s);
        let shortest_s = f64::from(shortest) * period_s;
        let longest_s = f64::from(longest) * period_s;
        let marker = MarkerKind::of_tone(self.tone);
        let within = |x: f64, (lo, hi): (f64, f64)| (lo..=hi).contains(&x);
        let fits = match marker {
            MarkerKind::Outer => {
                !mixed && within(per_s, OUTER_RATE) && shortest_s >= OUTER_SHORTEST_S
            }
            MarkerKind::Inner => {
                !mixed && within(per_s, INNER_RATE) && shortest_s <= INNER_SHORTEST_S
            }
            MarkerKind::Middle => {
                mixed
                    && dots.abs_diff(dashes) <= 1
                    && pairs_per_min.is_some_and(|p| within(p, MIDDLE_PAIRS))
                    && longest_s < MIDDLE_LONGEST_S
            }
            MarkerKind::None => false,
        };
        let depth = if self.inner_depth.1 > 0 {
            self.inner_depth.0 / f64::from(self.inner_depth.1)
        } else {
            self.edge_depth.0 / f64::from(self.edge_depth.1.max(1))
        };
        let offset = if self.offset_hz.1 > 0 {
            self.offset_hz.0 / f64::from(self.offset_hz.1)
        } else {
            0.0
        };
        Some(MarkerSummary {
            marker,
            tone_hz: TONES_HZ[self.tone] + offset,
            depth,
            elements: self.total,
            dots,
            dashes,
            mixed,
            per_s,
            pairs_per_min,
            dot_s,
            dash_s,
            shortest_s,
            longest_s,
            duration_s: self.last - self.first + period_s,
            end_s: self.last,
            fits,
        })
    }
}

/// The live indication of one tone: its held depth and its last on-lengths.
#[derive(Clone, Copy, Debug, Default)]
struct LiveTone {
    held: f64,
    depth: f64,
    on: bool,
    start: f64,
    lengths: [f64; LIVE_LENGTHS],
    head: usize,
    count: usize,
}

impl LiveTone {
    fn period(&mut self, t: f64, d: f64, decay: f64, period_s: f64) {
        self.held = (self.held * decay).max(d);
        self.depth = d;
        if !self.on && d > LIVE_ON {
            self.on = true;
            self.start = t - period_s;
        } else if self.on && d < LIVE_OFF {
            self.on = false;
            self.lengths[self.head] = t - period_s - self.start;
            self.head = (self.head + 1) % LIVE_LENGTHS;
            self.count = (self.count + 1).min(LIVE_LENGTHS);
        }
    }

    /// The pattern of the on-lengths: below 0.18 s a dot, 0.25 to 0.6 s a dash.
    fn pattern(&self) -> MarkerPattern {
        if self.count < 4 {
            return MarkerPattern::NotYet;
        }
        let held = &self.lengths[..self.count];
        let dots = held.iter().filter(|&&l| l < DASH_S).count();
        let dashes = held.iter().filter(|&&l| (0.25..=0.6).contains(&l)).count();
        match (dots, dashes) {
            _ if dots + dashes + 1 < self.count => MarkerPattern::Irregular,
            (d, h) if d > 0 && h > 0 => MarkerPattern::DotDash,
            (_, h) if h > 0 => MarkerPattern::Dashes,
            _ => MarkerPattern::Dots,
        }
    }
}

/// The marker detector over measuring periods: keyed periods, passages and their analysis, and the live indication.
#[derive(Clone, Debug)]
pub struct MarkerDetector {
    period_s: f64,
    advance: [f64; 3],
    decay: f64,
    passage: Option<Passage>,
    live: [LiveTone; 3],
}

impl MarkerDetector {
    /// For periods of `n` samples at `rate`.
    pub fn new(rate: f64, n: usize) -> Self {
        let period_s = n as f64 / rate;
        MarkerDetector {
            period_s,
            advance: TONES_HZ.map(|f| (TAU * f * period_s + PI).rem_euclid(TAU) - PI),
            decay: LIVE_DECAY_25MS.powf(period_s / 0.025),
            passage: None,
            live: [LiveTone::default(); 3],
        }
    }

    /// Forgets every passage and the live indication.
    pub fn reset(&mut self) {
        self.passage = None;
        self.live = [LiveTone::default(); 3];
    }

    /// One period ending at `t` s with the amplitude phasor of each tone of [`TONES_HZ`] (the envelope relative to the carrier,
    /// so a magnitude is a depth). Returns the passage that this period ended, if it had three elements or more.
    pub fn period(&mut self, t: f64, amps: &[Complex32]) -> Option<MarkerSummary> {
        let depth: [f64; 3] = [0, 1, 2].map(|i| f64::from(amps[i].norm()));
        for (l, &d) in self.live.iter_mut().zip(&depth) {
            l.period(t, d, self.decay, self.period_s);
        }
        let k = (0..3).max_by(|&a, &b| depth[a].total_cmp(&depth[b])).unwrap_or(0);
        let keyed = depth[k] >= KEYED_DEPTH
            && (0..3).all(|j| j == k || depth[k] >= KEYED_RATIO * depth[j]);
        let mut ended = None;
        if keyed {
            if self.passage.as_ref().is_some_and(|p| p.tone != k) {
                ended = self.finish();
            }
            let p = self.passage.get_or_insert_with(|| Passage::new(k, t));
            p.keyed(t, amps[k], depth[k], self.advance[k], self.period_s);
        } else if let Some(p) = &mut self.passage {
            p.close();
            if t - p.last >= PASSAGE_END_S - 1e-9 {
                ended = self.finish();
            }
        }
        ended
    }

    /// Ends the passage under way. Returns its analysis when it had three elements or more.
    pub fn finish(&mut self) -> Option<MarkerSummary> {
        let mut p = self.passage.take()?;
        p.close();
        p.summary(self.period_s)
    }

    /// The state: the passage under way and the live indication.
    pub fn state(&self) -> MarkerState {
        let k = (0..3)
            .max_by(|&a, &b| self.live[a].held.total_cmp(&self.live[b].held))
            .unwrap_or(0);
        let l = &self.live[k];
        let live = if l.held >= LIVE_OFF {
            MarkerKind::of_tone(k)
        } else {
            MarkerKind::None
        };
        MarkerState {
            current: self.passage.as_ref().and_then(|p| p.summary(self.period_s)),
            live,
            keyed_now: live != MarkerKind::None && l.depth > KEYED_DEPTH,
            held_depth: l.held,
            pattern: if live == MarkerKind::None {
                MarkerPattern::NotYet
            } else {
                l.pattern()
            },
        }
    }
}

impl MarkerState {
    /// The live pattern is the one of the standard for the live tone.
    pub fn pattern_fits(&self) -> bool {
        self.live != MarkerKind::None && self.pattern == self.live.pattern()
    }
}

/// The marker tones (`avnav.marker`, real -> [`MarkerState`] `state` + [`MarkerPassage`] `passage`): the depths of 400, 1300
/// and 3000 Hz in the envelope relative to the carrier, over periods of 10 ms, through the [`MarkerDetector`]. Its command
/// `clear` forgets every passage.
pub struct MarkerTones {
    rate: f64,
    bank: Option<ToneBank>,
    detector: MarkerDetector,
    since_state: u32,
    states: Vec<Timed<MarkerState>>,
    passages: Vec<Timed<MarkerPassage>>,
}

impl Default for MarkerTones {
    fn default() -> Self {
        MarkerTones {
            rate: 0.0,
            bank: None,
            detector: MarkerDetector::new(1.0 / PERIOD_S, 1),
            since_state: 0,
            states: Vec::new(),
            passages: Vec::new(),
        }
    }
}

/// The state goes out once in this many periods.
const STATE_EVERY: u32 = 10;

impl Component for MarkerTones {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.marker",
            vec![Port::real("in")],
            vec![
                Port::typed::<MarkerState>("state").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<MarkerPassage>("passage").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        // The 3000 Hz tone needs a rate above 6 kHz with room for the filter.
        if f.rate < 8000.0 {
            return Err(ConfigError::RateTooLow {
                need: 8000.0,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        let n = (PERIOD_S * f.rate).round() as usize;
        self.bank = Some(ToneBank::with_rate(f.rate, &TONES_HZ, n)?);
        self.detector = MarkerDetector::new(f.rate, n);
        self.since_state = 0;
        self.states = Vec::with_capacity(64);
        self.passages = Vec::with_capacity(8);
        Ok(vec![
            Format {
                rate: 0.0,
                ..f.with_kind(Kind::typed::<MarkerState>())
            },
            Format {
                rate: 0.0,
                ..f.with_kind(Kind::typed::<MarkerPassage>())
            },
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let x = io.real(0);
        let Some(mut bank) = self.bank.take() else {
            return;
        };
        self.states.clear();
        self.passages.clear();
        bank.process(x, |end, amps| {
            let time = (first + end as u64) as f64 / self.rate;
            if let Some(s) = self.detector.period(time, amps) {
                self.passages.push(Timed {
                    time,
                    value: MarkerPassage(s),
                });
            }
            self.since_state += 1;
            if self.since_state >= STATE_EVERY {
                self.since_state = 0;
                self.states.push(Timed {
                    time,
                    value: self.detector.state(),
                });
            }
        });
        self.bank = Some(bank);
        io.items_out::<MarkerState>(0).append(&mut self.states);
        io.items_out::<MarkerPassage>(1).append(&mut self.passages);
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        DecoderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DecoderCommand::Clear => {
                self.detector.reset();
                if let Some(b) = &mut self.bank {
                    b.reset();
                }
            }
        }
        Ok(None)
    }
}

/// One element of a keying pattern: on for `on` s in a slot of `slot` s.
#[derive(Clone, Copy, Debug)]
struct Slot {
    on: f64,
    slot: f64,
}

/// The dash of the test signal: 0.30 s in the slot of a keying of 2 a second (the length is the specification's).
const DASH: Slot = Slot {
    on: 0.30,
    slot: 0.5,
};
/// The dot of the test signal: 0.08 s in the slot of a keying of 6 a second (the length is the specification's).
const DOT: Slot = Slot {
    on: 0.08,
    slot: 1.0 / 6.0,
};
/// The rise and the fall of the keying, s.
const EDGE_S: f64 = 0.001;

/// The marker test signal: a carrier at 0 Hz that is never interrupted, amplitude modulated by the marker tone, keyed as
/// 3.1.7.5.1 says (outer: dashes at 2 a second; middle: alternate dashes at 2 a second and dots at 6 a second; inner: dots at 6 a
/// second), at unit mean power.
pub struct MarkerGenerator {
    rate: f64,
    tone_hz: f64,
    depth: f64,
    slots: &'static [Slot],
    period: f64,
    start: f64,
    stop: f64,
    amplitude: f64,
    n: u64,
    phase0: f64,
}

impl MarkerGenerator {
    /// `marker` keyed by `keying`, its tone at `tone_hz` (0: the marker's own) and `depth` (0..1), keyed from `start` s to `stop`
    /// s (0: without end).
    pub fn new(
        rate: f64,
        marker: GenMarker,
        keying: GenMarkerKeying,
        tone_hz: f64,
        depth: f64,
        start: f64,
        stop: f64,
        seed: u64,
    ) -> Self {
        let slots: &'static [Slot] = match (keying, marker) {
            (GenMarkerKeying::Steady, _) => &[],
            (_, GenMarker::Outer) => &[DASH],
            (_, GenMarker::Middle) => &[DASH, DOT],
            (_, GenMarker::Inner) => &[DOT],
        };
        let period: f64 = slots.iter().map(|s| s.slot).sum();
        let duty = if slots.is_empty() {
            1.0
        } else {
            slots.iter().map(|s| s.on).sum::<f64>() / period
        };
        let tone = marker.kind().tone_hz().unwrap_or(TONES_HZ[0]);
        MarkerGenerator {
            rate,
            tone_hz: if tone_hz > 0.0 { tone_hz } else { tone },
            depth,
            slots,
            period,
            start,
            stop,
            // Unit mean power over the keying: the carrier and the tone for the share of the time it is on.
            amplitude: 1.0 / (1.0 + duty * depth * depth / 2.0).sqrt(),
            n: 0,
            phase0: sdrkit_algo_sim::rng::Rng::new(seed).uniform() * TAU,
        }
    }

    /// The keying at `t` s, 0..1, with raised-cosine edges.
    fn key(&self, t: f64) -> f64 {
        if t < self.start || (self.stop > 0.0 && t >= self.stop) {
            return 0.0;
        }
        if self.slots.is_empty() {
            return 1.0;
        }
        let mut at = (t - self.start) % self.period;
        for s in self.slots {
            if at < s.slot {
                if at >= s.on {
                    return 0.0;
                }
                let edge = at.min(s.on - at) / EDGE_S;
                return if edge >= 1.0 {
                    1.0
                } else {
                    0.5 - 0.5 * (PI * edge).cos()
                };
            }
            at -= s.slot;
        }
        0.0
    }
}

impl Generator for MarkerGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(SIGNAL_WIDTH_HZ))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let (s0, c0) = self.phase0.sin_cos();
        for o in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            let e = self.amplitude
                * (1.0 + self.depth * self.key(t) * (TAU * ((self.tone_hz * t) % 1.0)).cos());
            *o = Complex32::new((e * c0) as f32, (e * s0) as f32);
            self.n += 1;
        }
        out.len()
    }
}

