//! CW keying quality meter (`cw_keying`): the timing and the shape of a keyed carrier. From the last marks and spaces: dit
//! and dah, speed by the PARIS word of 50 units, dah ratio, the gaps in dits sorted into element, character and word gaps
//! (ITU-R M.1677-1: 1, 3 and 7), weight, dit scatter, 10 to 90 % rise and fall times and chirp. It measures and copies no
//! text; it needs about 20 dB carrier to noise in 500 Hz. The levels, windows and rules are confirmed by no public document.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{index_percentile, tx_stack, Ctx, TxTest};
use crate::typed::{CwKeyingEventRow, CwKeyingFields, CW_KEYING_EVENTS};
use sdrkit_algo_dsp::carrierfind::{block_len, CarrierFinder, FinderConfig};
use sdrkit_algo_dsp::kmeans::{kmeans_1d, Start};
use sdrkit_algo_dsp::plateau::{crossing_down, crossing_down_back, crossing_up, plateau_level};
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "cw_keying";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 12e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 12e3;
/// The pass edge of the channel filter, Hz.
pub const PASS_HZ: f64 = 2100.0;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.2;
/// The envelope rate is this to twice this, Hz.
pub const ENVELOPE_HZ: f64 = 2000.0;
/// The pass edge of the envelope filter, Hz.
pub const ENVELOPE_PASS_HZ: f64 = 800.0;
/// The envelope samples dropped at the start, s.
pub const ENVELOPE_SKIP_S: f64 = 0.5;
/// The time constant of the smoothed envelope, s.
pub const SMOOTH_S: f64 = 0.004;
/// The decay of the held peak per second.
pub const PEAK_DECAY: f64 = 0.3;
/// The floor is the 20th percentile of the smoothed envelope over this time, s, taken every half second.
pub const FLOOR_WINDOW_S: f64 = 4.0;
/// A signal is there when the held peak is this many times the floor.
pub const SIGNAL_RATIO: f64 = 6.0;
/// The ring before a key-down, s.
pub const RING_S: f64 = 0.04;
/// The fall is collected for this long after a key-up, s.
pub const TAIL_S: f64 = 0.03;
/// The longest mark, s; the longest space kept, s.
pub const LONGEST_S: (f64, f64) = (4.0, 3.0);
/// The shortest mark, s.
pub const SHORTEST_S: f64 = 0.004;
/// The least rise or fall shown, s.
pub const EDGE_FLOOR_S: f64 = 0.0015;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the timing and shape of a keyed carrier from the last marks and spaces: dit, dah, speed \
(PARIS), dah ratio, gaps, weight, dit scatter, rise and fall times and chirp; copies no text. It needs about 20 dB carrier to \
noise in 500 Hz. The envelope filter makes a rise or fall under about 1.5 ms read 1.5 ms. The rules are confirmed by no \
public document.";

/// The carrier search half width.
pub const SEARCH_HZ: Setting<field::F64> =
    Setting::new("search_hz", "Carrier search half width", "Hz", 1500.0).range(100.0, 1500.0, 0.0);
/// The key-down share of the peak.
pub const ON_SHARE: Setting<field::F64> =
    Setting::new("on_share", "Key-down above (of the peak)", "", 0.5).range(0.1, 0.9, 0.0);
/// The key-up share of the mark level.
pub const OFF_SHARE: Setting<field::F64> =
    Setting::new("off_share", "Key-up under (of the peak)", "", 0.35).range(0.1, 0.9, 0.0);
/// The elements kept.
pub const ELEMENTS: Setting<field::I64> =
    Setting::new("elements", "Elements kept", "", 60).range(20.0, 200.0, 1.0);
/// The shortest mark for the chirp.
pub const MIN_CHIRP_MARK_S: Setting<field::F64> =
    Setting::new("min_chirp_mark_s", "Shortest mark for the chirp", "s", 0.05)
        .range(0.02, 0.5, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The search.
        SearchHz = SEARCH_HZ,
        /// The key-down level.
        OnShare = ON_SHARE,
        /// The key-up level.
        OffShare = OFF_SHARE,
        /// The elements kept.
        Elements = ELEMENTS,
        /// The chirp marks.
        MinChirpMarkS = MIN_CHIRP_MARK_S,
    }
}

sdrkit_core::settings! {
    /// The settings of the `cw_keying` stack.
    pub enum CwKeyingSetting {
        /// The search.
        SearchHz = SEARCH_HZ,
        /// The key-down level.
        OnShare = ON_SHARE,
        /// The key-up level.
        OffShare = OFF_SHARE,
        /// The elements kept.
        Elements = ELEMENTS,
        /// The chirp marks.
        MinChirpMarkS = MIN_CHIRP_MARK_S,
        /// The text of the test signal.
        GenText = gen::GEN_TEXT,
        /// Its speed.
        GenWpm = gen::GEN_WPM,
        /// Its dah ratio.
        GenDahRatio = gen::GEN_DAH_RATIO,
        /// Its weight.
        GenWeight = gen::GEN_WEIGHT,
        /// Its character gap.
        GenCharGap = gen::GEN_CHAR_GAP,
        /// Its word gap.
        GenWordGap = gen::GEN_WORD_GAP,
        /// Its mark jitter.
        GenJitter = gen::GEN_JITTER,
        /// Its edges.
        GenRampS = gen::GEN_RAMP_S,
        /// Its chirp.
        GenChirpHz = gen::GEN_CHIRP_HZ,
        /// The chirp time constant.
        GenChirpTauS = gen::GEN_CHIRP_TAU_S,
        /// Its level swing.
        GenFadeDb = gen::GEN_FADE_DB,
    }
}

/// A kept mark.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Mark {
    ms: f64,
    rise: Option<f64>,
    fall: Option<f64>,
    chirp: Option<f64>,
}

/// A mark being collected.
#[derive(Clone, Debug, Default)]
struct Open {
    a: Vec<f64>,
    f: Vec<f64>,
    start: usize,
    end: Option<usize>,
    level: f64,
    t0: f64,
    ms: f64,
    tail: usize,
}

/// One evaluation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Eval {
    wpm: f64,
    dit: f64,
    dah: Option<f64>,
    ratio: Option<f64>,
    element: Option<f64>,
    char_gap: Option<(f64, i64)>,
    word_gap: Option<(f64, i64)>,
    weight: f64,
    scatter: f64,
    rise: Option<f64>,
    fall: Option<f64>,
    chirp: Option<(f64, i64)>,
}

/// The measurement of `cw_keying`.
#[derive(Default)]
pub struct CwKeying {
    front: Option<Front>,
    finder: Option<CarrierFinder>,
    env_front: Option<Front>,
    rate: f64,
    r2: f64,
    block: Vec<Complex32>,
    fc: Option<f64>,
    phase: f64,
    on_share: f64,
    off_share: f64,
    elements: usize,
    min_chirp: f64,
    t: f64,
    te: f64,
    alpha: f64,
    e: f64,
    h: f64,
    prev: Option<Complex32>,
    floor: Option<f64>,
    hist: VecDeque<f64>,
    next_floor: f64,
    ring: VecDeque<(f64, f64)>,
    down: Option<Open>,
    tail: Option<Open>,
    last_up: Option<f64>,
    marks: VecDeque<Mark>,
    spaces: VecDeque<f64>,
    new_marks: usize,
    last_mark_t: Option<f64>,
    eval: Option<Eval>,
    last_row: Option<(f64, f64)>,
    log: Log<CwKeyingEventRow>,
}

impl CwKeying {
    fn shape(&mut self, o: Open) {
        let r2 = self.r2;
        let end = o.end.unwrap_or(o.a.len());
        let mut m = Mark {
            ms: o.ms,
            ..Mark::default()
        };
        if let Some(l) = plateau_level(&o.a, o.start, end) {
            let third = o.start + (end - o.start) / 3;
            let (r10, r90) = (
                crossing_up(&o.a, 0.1 * l, 0, third),
                crossing_up(&o.a, 0.9 * l, 0, third),
            );
            m.rise = match (r10, r90) {
                (Some(a), Some(b)) if b > a => Some(EDGE_FLOOR_S.max((b - a) / r2)),
                _ => None,
            };
            let two = o.start + 2 * (end - o.start) / 3;
            m.fall = crossing_down_back(&o.a, 0.9 * l, two, end + 1).and_then(|f90| {
                let f10 = crossing_down(&o.a, 0.1 * l, f90.floor() as usize + 1, o.a.len())?;
                Some(EDGE_FLOOR_S.max((f10 - f90) / r2))
            });
            if o.ms / 1000.0 >= self.min_chirp {
                let at = |s: f64| o.start + (s * r2).round() as usize;
                let mean = |from: usize, to: usize, need: usize| {
                    let v: Vec<f64> = (from..to.min(o.a.len()))
                        .filter(|&i| o.a[i] > 0.7 * l)
                        .map(|i| o.f[i])
                        .collect();
                    (v.len() >= need).then(|| v.iter().sum::<f64>() / v.len() as f64)
                };
                let early = mean(at(0.005), at(0.025), 4);
                let lim = end
                    .saturating_sub((0.004 * r2).round() as usize)
                    .min(o.a.len().saturating_sub((0.003 * r2).round() as usize));
                let late = mean(o.start + (end - o.start) / 2, lim, 8);
                m.chirp = early.zip(late).map(|(a, b)| a - b);
            }
        }
        self.marks.push_back(m);
        while self.marks.len() > self.elements {
            self.marks.pop_front();
        }
        self.new_marks += 1;
        if self.marks.len() >= 10 && self.new_marks >= 5 {
            self.new_marks = 0;
            self.evaluate();
        }
    }

    fn evaluate(&mut self) {
        let d: Vec<f64> = self.marks.iter().map(|m| m.ms).collect();
        let n = d.len();
        let mut s = d.clone();
        s.sort_by(f64::total_cmp);
        let start = [s[n / 6], s[5 * n / 6]];
        let Some(k) = kmeans_1d(&d, 2, Start::Given(&start), 30) else {
            return;
        };
        let (c0, c1) = (k.centres[0], k.centres[1]);
        let two = c1 > 1.8 * c0 && c1 < 4.5 * c0;
        let (dits, dahs): (Vec<f64>, Vec<f64>) = if two {
            d.iter().partition(|&&v| k.nearest(v) == 0)
        } else {
            (d.clone(), Vec::new())
        };
        if dits.len() < 3 {
            self.eval = None;
            return;
        }
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let dit = mean(&dits);
        let sd = (dits.iter().map(|x| (x - dit).powi(2)).sum::<f64>() / dits.len() as f64).sqrt();
        let dah = (two && dahs.len() >= 2).then(|| mean(&dahs));
        let q: Vec<f64> = self.spaces.iter().map(|s| s * 1000.0 / dit).collect();
        let class = |lo: f64, hi: f64| -> Vec<f64> {
            q.iter().copied().filter(|&x| x >= lo && x < hi).collect()
        };
        let (el, ch, wo) = (class(0.0, 2.0), class(2.0, 5.0), class(5.0, f64::INFINITY));
        let element = (!el.is_empty()).then(|| mean(&el));
        let wpm = if el.len() >= 3 {
            1200.0 / ((dit + element.unwrap_or(1.0) * dit) / 2.0)
        } else {
            1200.0 / dit
        };
        let avg = |f: fn(&Mark) -> Option<f64>| {
            let v: Vec<f64> = self.marks.iter().filter_map(f).collect();
            (!v.is_empty()).then(|| (mean(&v), v.len() as i64))
        };
        self.eval = Some(Eval {
            wpm,
            dit,
            dah,
            ratio: dah.map(|a| a / dit),
            element,
            char_gap: (!ch.is_empty()).then(|| (mean(&ch), ch.len() as i64)),
            word_gap: (!wo.is_empty()).then(|| (mean(&wo), wo.len() as i64)),
            weight: element.map_or(0.0, |e| 1.0 / e),
            scatter: 100.0 * sd / dit,
            rise: avg(|m| m.rise).map(|v| v.0 * 1000.0),
            fall: avg(|m| m.fall).map(|v| v.0 * 1000.0),
            chirp: avg(|m| m.chirp),
        });
        let e = self.eval.unwrap_or_default();
        if self
            .last_row
            .is_none_or(|(t, w)| (e.wpm - w).abs() > 0.05 * w || self.te - t >= 30.0)
        {
            self.last_row = Some((self.te, e.wpm));
            self.log.push(CwKeyingEventRow {
                time_s: Seconds(self.te),
                wpm: e.wpm,
                dit_ms: e.dit,
                dah_ratio: e.ratio,
                element_gap: e.element,
                char_gap: e.char_gap.map(|g| g.0),
                word_gap: e.word_gap.map(|g| g.0),
                weight: e.element.map(|_| e.weight),
                dit_scatter_pct: e.scatter,
                rise_ms: e.rise,
                fall_ms: e.fall,
                chirp_hz: e.chirp.map(|c| c.0),
            });
        }
    }

    fn envelope(&mut self, z: Complex32, c: &mut Ctx) {
        let r2 = self.r2;
        self.te += 1.0 / r2;
        let a = f64::from(z.norm());
        let f = self.prev.map_or(0.0, |p| {
            let d = z * p.conj();
            f64::from(d.im).atan2(f64::from(d.re)) * r2 / TAU
        });
        self.prev = Some(z);
        self.e += self.alpha * (a - self.e);
        self.h = self.e.max(self.h * (1.0 - PEAK_DECAY / r2));
        self.hist.push_back(self.e);
        while self.hist.len() as f64 > FLOOR_WINDOW_S * r2 {
            self.hist.pop_front();
        }
        if self.te >= self.next_floor {
            self.next_floor = self.te + 0.5;
            if self.hist.len() > 100 {
                let mut v: Vec<f64> = self.hist.iter().copied().collect();
                self.floor = index_percentile(&mut v, 0.2);
            }
        }
        let signal = self.floor.is_some_and(|fl| self.h > SIGNAL_RATIO * fl);
        if let Some(t) = self.tail.as_mut() {
            t.a.push(a);
            t.f.push(f);
            t.tail += 1;
            if t.tail as f64 >= TAIL_S * r2 {
                let t = self.tail.take().unwrap_or_default();
                self.shape(t);
            }
        }
        match self.down.as_mut() {
            None => {
                if signal && self.e > self.on_share * self.h {
                    if let Some(t) = self.tail.take() {
                        self.shape(t);
                    }
                    if let Some(up) = self.last_up {
                        let s = self.te - up;
                        if (0.0..=LONGEST_S.1).contains(&s) {
                            self.spaces.push_back(s);
                            while self.spaces.len() > self.elements {
                                self.spaces.pop_front();
                            }
                        }
                    }
                    let (ra, rf): (Vec<f64>, Vec<f64>) = self.ring.iter().copied().unzip();
                    let start = ra.len().saturating_sub(1);
                    self.down = Some(Open {
                        a: ra,
                        f: rf,
                        start,
                        level: self.h,
                        t0: self.te,
                        ..Open::default()
                    });
                }
            }
            Some(o) => {
                o.a.push(a);
                o.f.push(f);
                if self.e < self.off_share * o.level || self.te - o.t0 > LONGEST_S.0 {
                    let mut o = self.down.take().unwrap_or_default();
                    let dur = self.te - o.t0;
                    self.last_up = Some(self.te);
                    if dur >= SHORTEST_S {
                        o.ms = dur * 1000.0;
                        o.end = Some(o.a.len());
                        self.tail = Some(o);
                        self.last_mark_t = Some(self.te);
                        c.tally.good(self.te);
                    }
                }
            }
        }
        self.ring.push_back((a, f));
        while self.ring.len() as f64 > RING_S * r2 {
            self.ring.pop_front();
        }
    }
}

impl TxTest for CwKeying {
    const ID: &'static str = ID;
    const NAME: &'static str = "cw_keying.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let front = Front::new(rate_hz, WORK_HZ, PASS_HZ, SKIP_S);
        self.rate = front.rate();
        let cfg = FinderConfig::new(v.get(&SEARCH_HZ), PASS_HZ, 30, 60.0).every(2);
        self.finder = CarrierFinder::new(block_len(self.rate, 0.7), self.rate, cfg).ok();
        self.front = Some(front);
        let env = Front::new(self.rate, ENVELOPE_HZ, ENVELOPE_PASS_HZ, ENVELOPE_SKIP_S);
        self.r2 = env.rate();
        self.env_front = Some(env);
        self.alpha = 1.0 - (-1.0 / (self.r2 * SMOOTH_S)).exp();
        self.on_share = v.get(&ON_SHARE);
        self.off_share = v.get(&OFF_SHARE);
        self.elements = v.get(&ELEMENTS).max(10) as usize;
        self.min_chirp = v.get(&MIN_CHIRP_MARK_S);
        self.block.clear();
        self.fc = None;
        self.phase = 0.0;
        self.t = self.t.max(SKIP_S);
        self.e = 0.0;
        self.h = 0.0;
        self.prev = None;
        self.floor = None;
        self.hist.clear();
        self.next_floor = self.te;
        self.ring.clear();
        self.down = None;
        self.tail = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let (Some(mut front), Some(mut env)) = (self.front.take(), self.env_front.take()) else {
            return;
        };
        let mut mixed = Vec::new();
        for &z in front.process(x) {
            self.t += 1.0 / self.rate;
            self.block.push(z);
            let n = self.finder.as_ref().map_or(usize::MAX, CarrierFinder::len);
            if self.block.len() >= n {
                if let Some(look) = self
                    .finder
                    .as_mut()
                    .and_then(|f| f.look(&self.block))
                    .filter(|l| l.present)
                {
                    self.fc = Some(
                        self.fc
                            .map_or(look.offset_hz, |fc| 0.7 * fc + 0.3 * look.offset_hz),
                    );
                }
                self.block.clear();
            }
            if let Some(fc) = self.fc {
                self.phase = (self.phase - TAU * fc / self.rate).rem_euclid(TAU);
                mixed.push(z * Complex32::from_polar(1.0, self.phase as f32));
            }
        }
        let zs: Vec<Complex32> = env.process(&mixed).to_vec();
        for z in zs {
            self.envelope(z, c);
        }
        self.front = Some(front);
        self.env_front = Some(env);
    }
    fn clear(&mut self) {
        self.marks.clear();
        self.spaces.clear();
        self.new_marks = 0;
        self.last_up = None;
        self.last_mark_t = None;
        self.eval = None;
        self.last_row = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&CW_KEYING_EVENTS, p);
        let recent = self.last_mark_t.is_some_and(|t| self.te - t <= 8.0);
        let e = self.eval.filter(|_| recent);
        p.put(&CwKeyingFields {
            wpm: e.map(|e| e.wpm),
            dit_ms: e.map(|e| e.dit),
            dah_ms: e.and_then(|e| e.dah),
            dah_ratio: e.and_then(|e| e.ratio),
            element_gap: e.and_then(|e| e.element),
            char_gap: e.and_then(|e| e.char_gap.map(|g| g.0)),
            word_gap: e.and_then(|e| e.word_gap.map(|g| g.0)),
            char_gaps: e.map(|e| e.char_gap.map_or(0, |g| g.1)),
            word_gaps: e.map(|e| e.word_gap.map_or(0, |g| g.1)),
            weight: e.map(|e| e.weight),
            dit_scatter_pct: e.map(|e| e.scatter),
            rise_ms: e.and_then(|e| e.rise),
            fall_ms: e.and_then(|e| e.fall),
            chirp_hz: e.and_then(|e| e.chirp.map(|c| c.0)),
            chirp_marks: e.and_then(|e| e.chirp.map(|c| c.1)),
            marks: self.marks.len() as i64,
            spaces: self.spaces.len() as i64,
            carrier_offset_hz: self.fc.filter(|_| recent).map(Hertz),
        });
        match e {
            Some(e) => format!(
                "{:.1} wpm, dit {:.1} ms, dah ratio {}, weight {:.2}",
                e.wpm,
                e.dit,
                e.ratio.map_or("none".to_string(), |r| format!("{r:.2}")),
                e.weight
            ),
            None if self.fc.is_some() => format!("carrier found, {} marks", self.marks.len()),
            None => "searching for a carrier".to_string(),
        }
    }
}

const KEYS: [&str; 5] = [
    "search_hz",
    "on_share",
    "off_share",
    "elements",
    "min_chirp_mark_s",
];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("text at 20 wpm", o, 0.0);
    // The text ends at about 20 s, and the readings hold for 8 s after the last mark.
    c.seconds = o.seconds.max(24.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let w = crate::instrument::number(r, CwKeyingFields::WPM.key);
        vec![CheckLine::new(
            CwKeyingFields::WPM.key,
            // At 20 dB in 500 Hz, the least carrier to noise of the stack, the speed holds to 1 wpm.
            (w - 20.0).abs() <= 1.0,
            format!("{w:.2} wpm, 20 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: CwKeying,
    settings: CwKeyingSetting,
    keys: KEYS,
    name: "CW keying quality meter",
    blurb: "Speed, dit and dah, dah ratio, gaps, weight, dit scatter, rise and fall times and chirp of a keyed carrier.",
    tuning: (0.1e6, 500e6, 48e3, 4e3, true),
    rate_text: "The CW keying meter needs at least 12 ksps",
    generator: |cfg| gen::CwGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
