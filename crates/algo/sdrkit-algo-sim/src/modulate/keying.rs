//! On-off keying with shaped edges, and Morse code keyed from text.

use crate::{Param, SimError};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::{Complex32, Generator};
use sdrkit_util_text::morse::{Key, Keyer};

/// Points in the raised-cosine edge table.
const EDGE_POINTS: usize = 256;

/// One pending edge: when it is centred (in samples) and which way it goes (+1 up, -1 down).
#[derive(Clone, Copy, Debug, Default)]
struct Edge {
    at: f64,
    dir: f32,
}

/// On-off keying: an envelope from timed key-down and key-up intervals, each edge a raised cosine of `rise` seconds centred on the
/// moment of the change, so a mark of length `T` measures `T` between its half-amplitude points whatever the rise time.
///
/// Edges add up (the envelope is the sum of shaped steps), so marks shorter than the rise time stay correct. Times are kept in
/// fractional samples, so intervals need not be whole samples. Sample `n` stands for the instant `n + 1/2`, so a mark from `a` to
/// `b` (in samples) covers samples `a..b` when both are whole: half-open, like the rectangle pulse. Used for CW, OOK devices (pulse-width and pulse-position codes),
/// beacons and ident keying.
///
/// ```
/// use sdrkit_algo_sim::modulate::Ook;
/// let mut k = Ook::new(1000.0, 0.0).unwrap(); // 1 kHz, hard edges
/// let mut env = Vec::new();
/// k.push(true, 0.003, &mut env).unwrap();
/// k.push(false, 0.002, &mut env).unwrap();
/// k.flush(&mut env);
/// assert_eq!(env, vec![1.0, 1.0, 1.0, 0.0, 0.0]);
/// ```
#[derive(Clone, Debug)]
pub struct Ook {
    rate: f64,
    /// Rise time in samples.
    rise: f64,
    /// The raised-cosine edge, tabulated from 0 to 1.
    table: Vec<f32>,
    /// Edges not yet finished, oldest first (a ring of fixed capacity).
    edges: Vec<Edge>,
    head: usize,
    len: usize,
    /// The level of all finished edges.
    base: f32,
    /// Whether the key is down at the end of what was pushed, and when that end is (samples).
    down: bool,
    end: f64,
    n: u64,
}

impl Ook {
    /// Keying for samples at `rate_hz` with edges `rise_s` long (0 for hard edges). Errors for a rate not positive or a rise time
    /// negative or above 1 s.
    pub fn new(rate_hz: f64, rise_s: f64) -> Result<Self, SimError> {
        Param::Rate.check_positive(rate_hz)?;
        Param::RiseTime.check_in(rise_s, 0.0, 1.0)?;
        let rise = rise_s * rate_hz;
        let table = (0..=EDGE_POINTS)
            .map(|i| sdrkit_algo_dsp::pulse::ramp(i as f64 / EDGE_POINTS as f64) as f32)
            .collect();
        let cap = (4.0 * rise).ceil() as usize + 64;
        Ok(Ook {
            rate: rate_hz,
            rise,
            table,
            edges: vec![Edge::default(); cap],
            head: 0,
            len: 0,
            base: 0.0,
            down: false,
            end: 0.0,
            n: 0,
        })
    }

    /// The sample rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    #[inline]
    fn step(&self, x: f64) -> f32 {
        if self.rise == 0.0 {
            return if x >= 0.0 { 1.0 } else { 0.0 };
        }
        let u = (x / self.rise + 0.5) * EDGE_POINTS as f64;
        if u <= 0.0 {
            0.0
        } else if u >= EDGE_POINTS as f64 {
            1.0
        } else {
            let i = u as usize;
            let f = (u - i as f64) as f32;
            self.table[i] + (self.table[i + 1] - self.table[i]) * f
        }
    }

    /// The envelope at sample `n`, retiring finished edges.
    #[inline]
    fn value(&mut self, n: f64) -> f32 {
        let cap = self.edges.len();
        while self.len > 0 {
            let e = self.edges[self.head];
            if n - e.at >= self.rise / 2.0 {
                self.base += e.dir;
                self.head = (self.head + 1) % cap;
                self.len -= 1;
            } else {
                break;
            }
        }
        let mut v = self.base;
        for i in 0..self.len {
            let e = self.edges[(self.head + i) % cap];
            v += e.dir * self.step(n - e.at);
        }
        v.clamp(0.0, 1.0)
    }

    fn emit_until(&mut self, limit: f64, out: &mut Vec<f32>) {
        while (self.n as f64) + 0.5 < limit {
            let v = self.value(self.n as f64 + 0.5);
            out.push(v);
            self.n += 1;
        }
    }

    /// Keys down (`on`) or up for `seconds`, and appends every envelope sample this settles. Errors for a duration that is negative
    /// or not finite, or for more edges within one rise time than the keyer holds (four per sample of rise time, plus 64).
    pub fn push(&mut self, on: bool, seconds: f64, out: &mut Vec<f32>) -> Result<(), SimError> {
        if !(seconds >= 0.0 && seconds.is_finite()) {
            return Param::KeyInterval.at_least(0.0, seconds);
        }
        if on != self.down {
            let cap = self.edges.len();
            let dir = if on { 1.0 } else { -1.0 };
            // An edge at the same moment as the last one cancels it (a zero-length interval).
            if self.len > 0 && self.edges[(self.head + self.len - 1) % cap].at == self.end {
                self.len -= 1;
            } else {
                if self.len == cap {
                    return Param::KeyChanges.range(0.0, cap as f64, (cap + 1) as f64);
                }
                self.edges[(self.head + self.len) % cap] = Edge { at: self.end, dir };
                self.len += 1;
            }
            self.down = on;
        }
        self.end += seconds * self.rate;
        // A sample is settled once every edge that can reach it is known: edges come at `end` or later.
        self.emit_until(self.end - self.rise / 2.0, out);
        Ok(())
    }

    /// Appends the samples up to the end of what was pushed (and the last edge's settling, when the key ends down then up) and starts
    /// again at time 0, key up.
    pub fn flush(&mut self, out: &mut Vec<f32>) {
        let mut limit = self.end;
        if self.len > 0 {
            let cap = self.edges.len();
            let last = self.edges[(self.head + self.len - 1) % cap];
            limit = limit.max(last.at + self.rise / 2.0);
        }
        self.emit_until(limit, out);
        self.len = 0;
        self.head = 0;
        self.base = 0.0;
        self.down = false;
        self.end = 0.0;
        self.n = 0;
    }
}

/// Morse code keyed from text (ITU-R M.1677-1 timing, through `sdrkit_util_text::morse::Keyer`), as a keyed carrier at an offset: CW
/// beacons, NDB and VOR idents, station identification.
///
/// It is a [`Generator`]: once (ending after the text), or repeating with a pause between repeats.
///
/// ```
/// use sdrkit_algo_sim::modulate::Morse;
/// use sdrkit_core::Generator;
/// let mut m = Morse::new("CQ", 20.0, 8000.0, 700.0, 0.005).unwrap();
/// let mut out = vec![Default::default(); 80_000];
/// let n = m.generate(&mut out);
/// assert!(n > 0 && n < out.len());
/// ```
#[derive(Clone, Debug)]
pub struct Morse {
    keys: Vec<Key>,
    pause_s: Option<f64>,
    lead_s: f64,
    next: usize,
    ook: Ook,
    nco: Nco,
    env: Vec<f32>,
    read: usize,
    started: bool,
    ended: bool,
}

impl Morse {
    /// `text` at `wpm` words a minute, for samples at `rate_hz`, carrier at `offset_hz`, edges of `rise_s`. Characters Morse has
    /// no code for are skipped. Errors for a speed outside 1 to 200 wpm, an empty keying, or a bad rate or rise time.
    pub fn new(
        text: &str,
        wpm: f64,
        rate_hz: f64,
        offset_hz: f64,
        rise_s: f64,
    ) -> Result<Self, SimError> {
        let keyer = Keyer::new(wpm).map_err(|_| SimError::OutOfRange {
            param: Param::Wpm,
            min: 1.0,
            max: 200.0,
            got: wpm,
        })?;
        Self::with_keyer(text, keyer, rate_hz, offset_hz, rise_s)
    }

    /// The same with a keyer of the caller's (Farnsworth spacing, for example).
    pub fn with_keyer(
        text: &str,
        keyer: Keyer,
        rate_hz: f64,
        offset_hz: f64,
        rise_s: f64,
    ) -> Result<Self, SimError> {
        let keys: Vec<Key> = keyer.key(text).collect();
        if keys.is_empty() {
            return Param::Keys.degenerate();
        }
        Self::from_keys(keys, rate_hz, offset_hz, rise_s)
    }

    /// Morse from key intervals the caller made: hand keying with jitter, an uneven dash ratio, or a keying a test needs. The
    /// first interval is the start of the sending. Errors for no intervals, an interval that is not a positive number of seconds,
    /// or a bad rate, carrier or rise time.
    ///
    /// ```
    /// use sdrkit_algo_sim::modulate::Morse;
    /// use sdrkit_util_text::morse::Key;
    /// let keys = [Key { on: true, seconds: 0.07 }, Key { on: false, seconds: 0.05 }, Key { on: true, seconds: 0.2 }];
    /// let m = Morse::from_keys(keys, 8000.0, 700.0, 0.005).unwrap();
    /// assert_eq!(m.keys().len(), 3);
    /// ```
    pub fn from_keys(
        keys: impl IntoIterator<Item = Key>,
        rate_hz: f64,
        offset_hz: f64,
        rise_s: f64,
    ) -> Result<Self, SimError> {
        let keys: Vec<Key> = keys.into_iter().collect();
        if keys.is_empty() {
            return Param::Keys.length(1, 0);
        }
        if keys
            .iter()
            .any(|k| !(k.seconds.is_finite() && k.seconds > 0.0))
        {
            return Param::KeyInterval.not_allowed(
                keys.iter()
                    .map(|k| k.seconds)
                    .find(|s| !(s.is_finite() && *s > 0.0))
                    .unwrap_or(0.0),
            );
        }
        if !(offset_hz.is_finite() && offset_hz.abs() < rate_hz / 2.0) {
            return Param::OffsetHz.range(-rate_hz / 2.0, rate_hz / 2.0, offset_hz);
        }
        let ook = Ook::new(rate_hz, rise_s)?;
        Ok(Morse {
            keys,
            pause_s: None,
            lead_s: 0.0,
            next: 0,
            ook,
            nco: Nco::new(rate_hz, offset_hz),
            env: Vec::with_capacity(1 << 12),
            read: 0,
            started: false,
            ended: false,
        })
    }

    /// Repeats the text forever with `pause_s` of silence between the end of one sending and the start of the next.
    pub fn repeating(mut self, pause_s: f64) -> Result<Self, SimError> {
        if !(pause_s >= 0.0 && pause_s.is_finite()) {
            return Param::Pause.at_least(0.0, pause_s);
        }
        self.pause_s = Some(pause_s);
        Ok(self)
    }

    /// Starts with `lead_s` of silence.
    pub fn with_lead(mut self, lead_s: f64) -> Result<Self, SimError> {
        if !(lead_s >= 0.0 && lead_s.is_finite()) {
            return Param::Lead.at_least(0.0, lead_s);
        }
        self.lead_s = lead_s;
        Ok(self)
    }

    /// The key intervals of one sending.
    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    /// Fills `env` with more envelope (at least one sample unless the sending has ended).
    fn refill(&mut self) {
        self.env.clear();
        self.read = 0;
        if !self.started {
            self.started = true;
            if self.lead_s > 0.0 {
                let _ = self.ook.push(false, self.lead_s, &mut self.env);
            }
        }
        while self.env.len() < 1024 && !self.ended {
            if self.next < self.keys.len() {
                let k = self.keys[self.next];
                self.next += 1;
                // `from_keys` checked every interval: a refusal cannot happen here.
                let _ = self.ook.push(k.on, k.seconds, &mut self.env);
            } else if let Some(p) = self.pause_s {
                let _ = self.ook.push(false, p, &mut self.env);
                self.next = 0;
            } else {
                self.ook.flush(&mut self.env);
                self.ended = true;
            }
        }
    }
}

impl Generator for Morse {
    fn rate_hz(&self) -> f64 {
        self.ook.rate()
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut done = 0;
        while done < out.len() {
            if self.read == self.env.len() {
                if self.ended {
                    break;
                }
                self.refill();
                continue;
            }
            let k = (self.env.len() - self.read).min(out.len() - done);
            self.nco.mix_real(
                &self.env[self.read..self.read + k],
                &mut out[done..done + k],
            );
            self.read += k;
            done += k;
        }
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs of the envelope above half amplitude: (on, length in samples).
    fn runs(env: &[f32]) -> Vec<(bool, usize)> {
        let mut r: Vec<(bool, usize)> = Vec::new();
        for &v in env {
            let on = v >= 0.5;
            match r.last_mut() {
                Some((o, n)) if *o == on => *n += 1,
                _ => r.push((on, 1)),
            }
        }
        r
    }

    #[test]
    fn edges_are_centred_so_marks_keep_their_length() {
        // 10 kHz, 4 ms edges: a 30 ms mark measures 30 ms at half amplitude; the edge passes 0.5 at the change.
        let mut k = Ook::new(10_000.0, 0.004).unwrap();
        let mut env = Vec::new();
        k.push(false, 0.010, &mut env).unwrap();
        k.push(true, 0.030, &mut env).unwrap();
        k.push(false, 0.020, &mut env).unwrap();
        k.flush(&mut env);
        assert_eq!(runs(&env), vec![(false, 100), (true, 300), (false, 200)]);
        assert!((env[100] - 0.5).abs() < 0.03 && env[79] == 0.0 && env[120] == 1.0);
        // Raised cosine: sample 90 is the instant 90.5, (90.5 - 100) / 40 + 1/2 of the way through the rise.
        let x = (90.5 - 100.0) / 40.0 + 0.5;
        let want = (1.0 - (std::f64::consts::PI * x).cos()) / 2.0;
        assert!((env[90] as f64 - want).abs() < 1e-4);
        // A mark shorter than the rise time still rises and falls symmetrically, below full height.
        let mut s = Ook::new(10_000.0, 0.004).unwrap();
        let mut e2 = Vec::new();
        s.push(false, 0.01, &mut e2).unwrap();
        s.push(true, 0.002, &mut e2).unwrap();
        s.push(false, 0.01, &mut e2).unwrap();
        s.flush(&mut e2);
        let peak = e2.iter().cloned().fold(0.0f32, f32::max);
        assert!(peak > 0.4 && peak < 0.8, "{peak}");
        assert!(Ook::new(1.0, -1.0).is_err());
        assert!(k.push(true, f64::NAN, &mut env).is_err());
    }

    #[test]
    fn paris_takes_fifty_dots_with_the_m1677_proportions() {
        // M.1677-1: "PARIS " is 50 dots; at 12 wpm a dot is 100 ms. With the trailing word space in a repeat, one cycle is 5 s.
        // The marks are .--. .- .-. .. ... with 1-dot gaps inside letters and 3-dot gaps between them.
        let rate = 1000.0;
        let mut m = Morse::new("PARIS", 12.0, rate, 0.0, 0.004)
            .unwrap()
            .repeating(0.7)
            .unwrap();
        let mut out = vec![Complex32::default(); 10_000];
        assert_eq!(m.generate(&mut out), 10_000);
        let env: Vec<f32> = out.iter().map(|v| v.re).collect();
        let r = runs(&env);
        let dots: Vec<(bool, usize)> = r
            .iter()
            .map(|&(o, n)| (o, (n as f64 / 100.0).round() as usize))
            .collect();
        let marks: Vec<usize> = dots.iter().filter(|d| d.0).map(|d| d.1).take(14).collect();
        assert_eq!(marks, vec![1, 3, 3, 1, 1, 3, 1, 3, 1, 1, 1, 1, 1, 1]);
        let gaps: Vec<usize> = dots
            .iter()
            .skip(1)
            .filter(|d| !d.0)
            .map(|d| d.1)
            .take(14)
            .collect();
        assert_eq!(gaps, vec![1, 1, 1, 3, 1, 3, 1, 1, 3, 1, 3, 1, 1, 7]);
        // Every edge lands on a whole number of dots (to a sample).
        for &(_, n) in r.iter().skip(1).take(26) {
            assert!(n % 100 <= 1 || n % 100 >= 99, "{n}");
        }
        // The repeat period: 43 dots of PARIS, then the 0.7 s pause = 7 dots: a sending starts after every space of 7 dots.
        let mut t = 0;
        let mut starts = Vec::new();
        let mut prev_off = usize::MAX;
        for &(o, n) in &r {
            if o && prev_off >= 650 {
                starts.push(t);
            }
            if !o {
                prev_off = n;
            }
            t += n;
        }
        assert_eq!(starts[0], 0);
        assert_eq!(starts[1], 5000, "{starts:?}");
    }

    #[test]
    fn a_single_sending_ends() {
        let mut m = Morse::new("E", 20.0, 8000.0, 1000.0, 0.0)
            .unwrap()
            .with_lead(0.01)
            .unwrap();
        let mut out = vec![Complex32::default(); 4000];
        let n = m.generate(&mut out);
        // 10 ms lead, one 60 ms dot: 560 samples.
        assert_eq!(n, 560);
        assert_eq!(m.generate(&mut out), 0);
        assert!(out[..80].iter().all(|v| v.norm() == 0.0));
        assert!(out[80..560].iter().all(|v| (v.norm() - 1.0).abs() < 1e-5));
        assert!(Morse::new("~~", 20.0, 8000.0, 0.0, 0.0).is_err());
        assert!(Morse::new("E", 0.0, 8000.0, 0.0, 0.0).is_err());
    }

    #[test]
    fn hand_keying_from_intervals() {
        // Hand keying: each interval 0 to 20 % longer, in a fixed pattern.
        let keys: Vec<Key> = Keyer::new(15.0)
            .unwrap()
            .key("TEST")
            .enumerate()
            .map(|(i, k)| Key {
                on: k.on,
                seconds: k.seconds * (1.0 + 0.05 * (i % 5) as f64),
            })
            .collect();
        let rate = 8000.0;
        let mut m = Morse::from_keys(keys.clone(), rate, 0.0, 0.002).unwrap();
        let mut out = vec![Complex32::default(); 80_000];
        let n = m.generate(&mut out);
        let env: Vec<f32> = out[..n].iter().map(|z| z.norm()).collect();
        let got = runs(&env);
        for (g, k) in got.iter().zip(&keys) {
            assert_eq!(g.0, k.on);
            assert!((g.1 as f64 - k.seconds * rate).abs() <= 2.0, "{got:?}");
        }
        assert!(Morse::from_keys([], rate, 0.0, 0.002).is_err());
        let bad = [Key {
            on: true,
            seconds: f64::NAN,
        }];
        assert!(Morse::from_keys(bad, rate, 0.0, 0.002).is_err());
    }
}
