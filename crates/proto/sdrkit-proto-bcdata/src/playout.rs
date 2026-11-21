//! The audio playout between a broadcast audio decoder and the output: a queue, a release clock, concealment of lost frames
//! and a smooth return to real sound. The queue and the history have a fixed size, so the playout does not allocate after
//! [`Playout::new`].

use sdrkit_algo_dsp::Frame;
use std::collections::VecDeque;

sdrkit_core::choice_enum! {
    /// How lost frames are filled.
    pub enum Concealment {
        /// Loop the last 40 ms at full level, fade out over 120 ms, and join real sound with a 2 ms crossfade.
        Repeat = ("repeat", "Repeat the last 40 ms"),
        /// Loop one pitch period (the best correlation from 2.5 ms to 20 ms) at full level for 20 ms, fade out over 200 ms, and
        /// join real sound with a 20 ms crossfade.
        Pitch = ("pitch", "Repeat one pitch period"),
        /// Fade out over 10 ms.
        Mute = ("mute", "Fade out"),
    }
}

/// The playout parameters. [`Config::new`] sets a start margin of 250 ms and the concealment times of [`Concealment`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// The frame rate in Hz.
    pub rate: u32,
    /// How lost frames are filled.
    pub concealment: Concealment,
    /// Concealment at full level, in seconds.
    pub hold_s: f64,
    /// The fade to silence after the hold, in seconds.
    pub fade_s: f64,
    /// The crossfade from concealment back to real sound, in seconds.
    pub join_s: f64,
    /// The margin above the largest push that the queue holds before release starts, in seconds.
    pub margin_s: f64,
    /// The most the queue holds, in seconds. Older frames are dropped beyond it.
    pub max_queue_s: f64,
}

impl Config {
    /// The parameters for a rate and a concealment.
    pub fn new(rate: u32, concealment: Concealment) -> Self {
        let (hold_s, fade_s, join_s) = match concealment {
            Concealment::Repeat => (0.040, 0.120, 0.002),
            Concealment::Pitch => (0.020, 0.200, 0.020),
            Concealment::Mute => (0.0, 0.010, 0.010),
        };
        Config {
            rate,
            concealment,
            hold_s,
            fade_s,
            join_s,
            margin_s: 0.25,
            max_queue_s: 4.0,
        }
    }
}

/// The counters of a playout.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// Seconds of real sound released.
    pub real_s: f64,
    /// Seconds of concealment above silence released.
    pub concealed_s: f64,
    /// Seconds of silence released (faded concealment, or waiting for the start level).
    pub muted_s: f64,
    /// Concealments started.
    pub concealments: u64,
    /// Times the queue ran empty while playing.
    pub underruns: u64,
    /// Frames dropped because the queue was full.
    pub dropped: u64,
    /// The queue in seconds.
    pub queue_s: f64,
    /// The rate correction of [`Playout::pull_resampled`] in ppm.
    pub rate_ppm: f64,
}

/// The playout: a queue of frames, released by [`Playout::pull`] (a fixed number of frames, for a clock derived from the
/// signal) or by [`Playout::pull_resampled`] (for a consumer with its own clock: the rate changes by up to 2000 ppm, with
/// Catmull-Rom interpolation, to keep the queue at its start level).
///
/// Release starts when the queue holds the start level: the largest push plus the margin. If the queue runs empty, the
/// playout conceals, counts an underrun and waits for the start level again. Lost frames that arrive for a gap that
/// concealment already filled are not played twice.
///
/// ```
/// use sdrkit_proto_bcdata::playout::{Concealment, Config, Playout};
/// use sdrkit_algo_dsp::Frame;
///
/// let mut p = Playout::new(Config::new(48_000, Concealment::Pitch));
/// let mut out = Vec::new();
/// p.push_sound(&[Frame([0.1, 0.1]); 4800]);
/// p.push_lost(4800);
/// p.pull(9600, &mut out);
/// assert_eq!(out.len(), 9600);
/// ```
#[derive(Clone, Debug)]
pub struct Playout {
    cfg: Config,
    hold: usize,
    fade: usize,
    join: usize,
    queue: VecDeque<(Frame<2>, bool)>,
    max_queue: usize,
    hist: Vec<Frame<2>>,
    hist_at: usize,
    hist_len: usize,
    snap: Vec<Frame<2>>,
    snap_len: usize,
    seam: usize,
    start_level: usize,
    started: bool,
    concealing: bool,
    conceal_pos: usize,
    loop_pos: usize,
    pre: bool,
    pre_left: usize,
    snap_pre: usize,
    join_left: usize,
    owed: usize,
    stats: Stats,
    // Rate control.
    level: f64,
    ratio: f64,
    frac: f64,
    last4: [Frame<2>; 4],
}

const HIST_S: f64 = 0.045;

/// `a` faded out and `b` faded in by `w` (0 to 1).
fn mix(a: Frame<2>, b: Frame<2>, w: f32) -> Frame<2> {
    a * (1.0 - w) + b * w
}

fn raised_cos(i: usize, n: usize) -> f32 {
    0.5 + 0.5 * (std::f32::consts::PI * i as f32 / n.max(1) as f32).cos()
}

impl Playout {
    /// A playout with these parameters.
    pub fn new(cfg: Config) -> Self {
        let r = cfg.rate.max(1) as f64;
        let n = |s: f64| (s.max(0.0) * r).round() as usize;
        let hist_len = n(HIST_S).max(64);
        let max_queue = n(cfg.max_queue_s).max(1);
        Playout {
            hold: n(cfg.hold_s),
            fade: n(cfg.fade_s).max(1),
            join: n(cfg.join_s),
            queue: VecDeque::with_capacity(max_queue),
            max_queue,
            hist: vec![Frame::default(); hist_len],
            hist_at: 0,
            hist_len: 0,
            snap: vec![Frame::default(); hist_len],
            snap_len: 0,
            seam: n(0.002).max(1),
            start_level: n(cfg.margin_s),
            started: false,
            concealing: false,
            conceal_pos: 0,
            loop_pos: 0,
            pre: false,
            pre_left: 0,
            snap_pre: 0,
            join_left: 0,
            owed: 0,
            stats: Stats::default(),
            level: 0.0,
            ratio: 1.0,
            frac: 0.0,
            last4: [Frame::default(); 4],
            cfg,
        }
    }

    /// The parameters.
    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Changes the concealment (the other parameters take that concealment's values).
    pub fn set_concealment(&mut self, c: Concealment) {
        let mut cfg = Config::new(self.cfg.rate, c);
        cfg.margin_s = self.cfg.margin_s;
        cfg.max_queue_s = self.cfg.max_queue_s;
        let r = cfg.rate.max(1) as f64;
        self.hold = (cfg.hold_s * r).round() as usize;
        self.fade = ((cfg.fade_s * r).round() as usize).max(1);
        self.join = (cfg.join_s * r).round() as usize;
        self.cfg = cfg;
    }

    /// The counters.
    pub fn stats(&self) -> Stats {
        let mut s = self.stats;
        s.queue_s = self.queue.len() as f64 / self.cfg.rate as f64;
        s.rate_ppm = (self.ratio - 1.0) * 1e6;
        s
    }

    /// Sets the counters to zero.
    pub fn clear_counts(&mut self) {
        self.stats = Stats::default();
    }

    /// Frames waiting in the queue.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Starts again (a new service or a retune): queue and history dropped, counters kept.
    pub fn reset(&mut self) {
        self.queue.clear();
        self.hist_len = 0;
        self.started = false;
        self.concealing = false;
        self.pre = false;
        self.join_left = 0;
        self.owed = 0;
        self.level = 0.0;
        self.ratio = 1.0;
        self.frac = 0.0;
        self.last4 = [Frame::default(); 4];
    }

    fn make_room(&mut self, n: usize) {
        let over = (self.queue.len() + n).saturating_sub(self.max_queue);
        if over > 0 {
            let k = over.min(self.queue.len());
            self.queue.drain(..k);
            self.stats.dropped += k as u64;
        }
    }

    fn note_burst(&mut self, n: usize) {
        let margin = (self.cfg.margin_s * self.cfg.rate as f64) as usize;
        self.start_level = self.start_level.max((n + margin).min(self.max_queue / 2));
    }

    /// Queues decoded sound.
    pub fn push_sound(&mut self, frames: &[Frame<2>]) {
        // Real sound ends an outage: losses reported later are new losses.
        self.owed = 0;
        self.note_burst(frames.len());
        let frames = &frames[frames.len().saturating_sub(self.max_queue)..];
        self.make_room(frames.len());
        self.queue.extend(frames.iter().map(|&f| (f, true)));
    }

    /// Queues `n` lost frames. Frames that concealment already filled during an underrun are not queued again.
    pub fn push_lost(&mut self, n: usize) {
        let covered = n.min(self.owed);
        self.owed -= covered;
        let n = (n - covered).min(self.max_queue);
        if n == 0 {
            return;
        }
        self.note_burst(n);
        self.make_room(n);
        self.queue
            .extend(std::iter::repeat_n((Frame::default(), false), n));
    }

    fn remember(&mut self, s: Frame<2>) {
        let cap = self.hist.len();
        self.hist[self.hist_at] = s;
        self.hist_at = (self.hist_at + 1) % cap;
        self.hist_len = (self.hist_len + 1).min(cap);
    }

    /// The `k`-th most recent history frame (0 is the newest).
    fn hist_back(&self, k: usize) -> Frame<2> {
        let cap = self.hist.len();
        self.hist[(self.hist_at + cap - 1 - k) % cap]
    }

    /// Takes the loop for a new concealment, and the `seam` frames before it. The loop ends with the `ahead`-th frame of the
    /// queue (`ahead` = 0: the newest frame of the history).
    fn take_loop(&mut self, ahead: usize) {
        self.loop_pos = 0;
        let r = self.cfg.rate as usize;
        let avail = self.hist_len + ahead;
        let l = match self.cfg.concealment {
            Concealment::Repeat => self.hold.max(1),
            Concealment::Pitch => self.pitch_period(r / 400, r / 50),
            Concealment::Mute => 0,
        }
        .min(avail)
        .min(self.snap.len());
        let s = self.seam.min(avail - l).min(self.snap.len() - l).min(l / 2);
        let total = l + s;
        for i in 0..total {
            // Frame `k` back from the end of the loop.
            let k = total - 1 - i;
            self.snap[i] = if k < ahead {
                self.queue[ahead - 1 - k].0
            } else {
                self.hist_back(k - ahead)
            };
        }
        self.snap_len = l;
        self.snap_pre = s;
    }

    /// Starts concealment at a lost frame (or an empty queue).
    fn start_concealment(&mut self) {
        if !self.pre {
            self.take_loop(0);
        }
        self.stats.concealments += 1;
        self.pre = false;
        self.concealing = true;
        self.conceal_pos = 0;
    }

    /// The period from `lo` to `hi` frames whose repetition best continues the history (normalised correlation over 8.3 ms).
    fn pitch_period(&self, lo: usize, hi: usize) -> usize {
        let w = self.cfg.rate as usize / 120;
        if self.hist_len < hi + w || lo == 0 {
            return hi.min(self.hist_len).max(1);
        }
        let mut best = (f32::MIN, hi);
        for p in lo..=hi {
            let (mut c, mut e) = (0.0f32, 1e-12f32);
            for i in 0..w {
                let a = self.hist_back(i);
                let b = self.hist_back(i + p);
                c += a.0[0] * b.0[0] + a.0[1] * b.0[1];
                e += b.0[0] * b.0[0] + b.0[1] * b.0[1];
            }
            let score = c / e.sqrt();
            if score > best.0 {
                best = (score, p);
            }
        }
        best.1
    }

    /// The next frame of the loop, at full level. The last `s` frames of each pass fade into the `s` frames that came before
    /// the loop start, so the jump back to the start is continuous.
    fn loop_frame(&mut self) -> Frame<2> {
        let (l, s) = (self.snap_len, self.snap_pre);
        let j = self.loop_pos;
        self.loop_pos += 1;
        if l == 0 {
            return Frame::default();
        }
        let p = j % l;
        let v = self.snap[s + p];
        if s > 0 && p >= l - s {
            let q = p - (l - s);
            return mix(v, self.snap[q], (q + 1) as f32 / (s + 1) as f32);
        }
        v
    }

    /// The next concealment frame, and whether it is above silence.
    fn conceal_frame(&mut self) -> (Frame<2>, bool) {
        let i = self.conceal_pos;
        self.conceal_pos += 1;
        let gain = if i < self.hold {
            1.0
        } else if i < self.hold + self.fade {
            raised_cos(i - self.hold, self.fade)
        } else {
            0.0
        };
        let v = self.loop_frame();
        if gain == 0.0 || self.snap_len == 0 {
            return (Frame::default(), false);
        }
        (v * gain, true)
    }

    /// A real frame `pre_left` frames before a loss, faded into the frames before the loop start.
    fn pre_frame(&mut self, f: Frame<2>) -> Frame<2> {
        let d = self.pre_left;
        self.pre_left -= 1;
        let s = self.snap_pre;
        if d > s {
            return f;
        }
        let k = s - d;
        mix(f, self.snap[k], (k + 1) as f32 / (s + 1) as f32)
    }

    /// The next released frame.
    fn next_frame(&mut self) -> Frame<2> {
        let dt = 1.0 / self.cfg.rate as f64;
        if !self.started {
            if self.queue.len() >= self.start_level && !self.queue.is_empty() {
                self.started = true;
            } else {
                // Waiting: the concealment of an underrun goes on, else silence.
                let (c, audible) = if self.concealing {
                    self.conceal_frame()
                } else {
                    (Frame::default(), false)
                };
                if audible {
                    self.stats.concealed_s += dt;
                } else {
                    self.stats.muted_s += dt;
                }
                if self.concealing {
                    self.owed += 1;
                }
                return c;
            }
        }
        match self.queue.pop_front() {
            Some((f, true)) => {
                if self.concealing {
                    self.concealing = false;
                    self.join_left = self.join;
                }
                let x = self.seam;
                // A loss starts after the next `x − 1` real frames: take the loop now, and over the last frames fade the real
                // sound into the frames just before the loop start. The loss then starts at the loop start, continuously.
                if !self.pre
                    && self.join_left == 0
                    && self.cfg.concealment != Concealment::Mute
                    && self.queue.get(x - 1).is_some_and(|e| !e.1)
                    && (x < 2 || self.queue.get(x - 2).is_some_and(|e| e.1))
                {
                    self.remember(f);
                    self.take_loop(x - 1);
                    self.pre = true;
                    self.pre_left = x;
                    self.stats.real_s += dt;
                    return self.pre_frame(f);
                }
                let out = if self.join_left > 0 {
                    let a = 1.0 - self.join_left as f32 / (self.join + 1) as f32;
                    let (c, _) = self.conceal_frame();
                    self.join_left -= 1;
                    mix(c, f, a)
                } else if self.pre && self.pre_left > 0 {
                    self.pre_frame(f)
                } else {
                    f
                };
                self.remember(f);
                self.stats.real_s += dt;
                out
            }
            other => {
                if other.is_none() {
                    self.started = false;
                    self.stats.underruns += 1;
                    self.owed += 1;
                }
                if !self.concealing {
                    self.start_concealment();
                }
                let (c, audible) = self.conceal_frame();
                if audible {
                    self.stats.concealed_s += dt;
                } else {
                    self.stats.muted_s += dt;
                }
                c
            }
        }
    }

    /// Releases `n` frames, appended to `out`.
    pub fn pull(&mut self, n: usize, out: &mut Vec<Frame<2>>) {
        out.reserve(n);
        for _ in 0..n {
            let f = self.next_frame();
            out.push(f);
        }
    }

    /// Fills `out` for a consumer with its own clock, changing the rate to keep the queue at the start level.
    pub fn pull_resampled(&mut self, out: &mut [Frame<2>]) {
        if self.started {
            self.level = if self.level == 0.0 {
                self.queue.len() as f64
            } else {
                0.95 * self.level + 0.05 * self.queue.len() as f64
            };
            let target = self.start_level.max(1) as f64;
            self.ratio = (1.0 + 0.01 * (self.level - target) / target).clamp(0.998, 1.002);
        }
        for o in out.iter_mut() {
            while self.frac >= 1.0 {
                self.frac -= 1.0;
                self.last4.rotate_left(1);
                self.last4[3] = self.next_frame();
            }
            let t = self.frac as f32;
            let [p0, p1, p2, p3] = self.last4;
            let cr = |p0: f32, p1: f32, p2: f32, p3: f32| {
                let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
                let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
                let c = -0.5 * p0 + 0.5 * p2;
                ((a * t + b) * t + c) * t + p1
            };
            *o = Frame(std::array::from_fn(|c| {
                cr(p0.0[c], p1.0[c], p2.0[c], p3.0[c])
            }));
            self.frac += self.ratio;
        }
    }
}
