//! A scene: several transmitters on one band, each at its own start time, frequency offset and level, summed into one stream.
//!
//! Sources are bursts known in advance (sent once or repeated with a period), streams from any [`Generator`] (from a start time,
//! for a duration or until it ends), and [`BurstSource`]s that make each burst when it is due (a sensor that reports every minute
//! with new readings). A scene is itself a [`Generator`]; put it through a [`crate::chain::Impaired`] with a noise floor
//! ([`crate::noise::NoiseLevel::FloorDbfs`]) to add the receiver's noise and impairments once, for all sources.
//!
//! Start times are rounded to the nearest sample. Each source's offset oscillator starts at phase 0 at its first sample and runs
//! only while the source sends.
//!
//! ```
//! use sdrkit_algo_sim::scene::Scene;
//! use sdrkit_core::{Complex32, Generator};
//! let mut s = Scene::new(1000.0).unwrap().with_duration(1.0).unwrap();
//! s.add_burst(0.1, 0.0, 0.0, vec![Complex32::new(1.0, 0.0); 50]).unwrap();
//! s.add_burst(0.12, 0.0, -6.0206, vec![Complex32::new(1.0, 0.0); 10]).unwrap();
//! let mut out = vec![Complex32::default(); 2000];
//! assert_eq!(s.generate(&mut out), 1000);
//! assert_eq!(out[99].re, 0.0);
//! assert_eq!(out[100].re, 1.0);
//! assert!((out[125].re - 1.5).abs() < 1e-4);
//! ```

use crate::front::gain_from_db;
use crate::{Param, SimError};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::{Complex32, Generator};

/// Samples per internal piece.
const PIECE: usize = 1024;

/// One burst a [`BurstSource`] makes.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    /// When it starts, seconds from the start of the scene.
    pub start_s: f64,
    /// Its frequency offset, Hz.
    pub offset_hz: f64,
    /// Its level, dB (gain applied to the samples).
    pub level_db: f64,
    /// The clean samples, at the scene's rate.
    pub samples: Vec<Complex32>,
}

/// Makes bursts as they become due, in order of start time.
pub trait BurstSource: Send {
    /// The next burst, or `None` when the source has finished. Bursts must come in order of start.
    fn next_burst(&mut self) -> Option<Burst>;
}

impl<F: FnMut() -> Option<Burst> + Send> BurstSource for F {
    fn next_burst(&mut self) -> Option<Burst> {
        self()
    }
}

enum Kind {
    Burst {
        samples: Vec<Complex32>,
        /// 0: once.
        period: u64,
    },
    Stream {
        gen: Box<dyn Generator>,
        stop: Option<u64>,
    },
}

struct Entry {
    start: u64,
    gain: f32,
    nco: Option<Nco>,
    kind: Kind,
    done: bool,
}

struct Lazy {
    src: Box<dyn BurstSource>,
    next: Option<Burst>,
    ended: bool,
}

/// Timed sources summed into one stream (module documentation).
pub struct Scene {
    rate: f64,
    entries: Vec<Entry>,
    lazy: Vec<Lazy>,
    pos: u64,
    end: Option<u64>,
    scratch: Vec<Complex32>,
}

impl Scene {
    /// An empty scene at `rate_hz` that never ends (until [`with_duration`](Self::with_duration)).
    pub fn new(rate_hz: f64) -> Result<Self, SimError> {
        if !(rate_hz > 0.0 && rate_hz.is_finite()) {
            return Param::Rate.positive(rate_hz);
        }
        Ok(Scene {
            rate: rate_hz,
            entries: Vec::new(),
            lazy: Vec::new(),
            pos: 0,
            end: None,
            scratch: vec![Complex32::default(); PIECE],
        })
    }

    /// Ends the scene after `seconds`.
    pub fn with_duration(mut self, seconds: f64) -> Result<Self, SimError> {
        if !(seconds >= 0.0 && seconds.is_finite()) {
            return Param::Duration.at_least(0.0, seconds);
        }
        self.end = Some((seconds * self.rate).round() as u64);
        Ok(self)
    }

    /// Where the scene is, seconds.
    pub fn position_s(&self) -> f64 {
        self.pos as f64 / self.rate
    }

    fn check(
        &self,
        start_s: f64,
        offset_hz: f64,
        level_db: f64,
    ) -> Result<(u64, f32, Option<Nco>), SimError> {
        Param::Start.check_in(start_s, 0.0, f64::INFINITY)?;
        Param::Level.check_finite(level_db)?;
        if !(offset_hz.is_finite() && offset_hz.abs() <= self.rate / 2.0) {
            return Param::OffsetHz.range(-self.rate / 2.0, self.rate / 2.0, offset_hz);
        }
        let nco = (offset_hz != 0.0).then(|| Nco::new(self.rate, offset_hz));
        Ok((
            (start_s * self.rate).round() as u64,
            gain_from_db(level_db),
            nco,
        ))
    }

    /// Sends `samples` once from `start_s`, moved by `offset_hz` and scaled by `level_db`.
    pub fn add_burst(
        &mut self,
        start_s: f64,
        offset_hz: f64,
        level_db: f64,
        samples: Vec<Complex32>,
    ) -> Result<(), SimError> {
        let (start, gain, nco) = self.check(start_s, offset_hz, level_db)?;
        self.entries.push(Entry {
            start,
            gain,
            nco,
            kind: Kind::Burst { samples, period: 0 },
            done: false,
        });
        Ok(())
    }

    /// Sends `samples` from `start_s` and again every `period_s` (rounded to a whole number of samples, at least the burst's
    /// length).
    pub fn add_repeating(
        &mut self,
        start_s: f64,
        period_s: f64,
        offset_hz: f64,
        level_db: f64,
        samples: Vec<Complex32>,
    ) -> Result<(), SimError> {
        let (start, gain, nco) = self.check(start_s, offset_hz, level_db)?;
        let period = (period_s * self.rate).round();
        if !(period >= samples.len().max(1) as f64 && period.is_finite()) {
            return Param::Period.at_least(samples.len().max(1) as f64, period);
        }
        self.entries.push(Entry {
            start,
            gain,
            nco,
            kind: Kind::Burst {
                samples,
                period: period as u64,
            },
            done: false,
        });
        Ok(())
    }

    /// Plays `gen` from `start_s`, for `duration_s` (or until it ends), moved by `offset_hz` and scaled by `level_db`. Its rate must
    /// be the scene's.
    pub fn add_stream(
        &mut self,
        start_s: f64,
        duration_s: Option<f64>,
        offset_hz: f64,
        level_db: f64,
        gen: Box<dyn Generator>,
    ) -> Result<(), SimError> {
        let (start, gain, nco) = self.check(start_s, offset_hz, level_db)?;
        if (gen.rate_hz() - self.rate).abs() > 1e-9 * self.rate {
            return Param::StreamRate.conflict(Param::Rate);
        }
        let stop = match duration_s {
            Some(d) if d >= 0.0 && d.is_finite() => Some(start + (d * self.rate).round() as u64),
            Some(d) => return Param::Duration.at_least(0.0, d),
            None => None,
        };
        self.entries.push(Entry {
            start,
            gain,
            nco,
            kind: Kind::Stream { gen, stop },
            done: false,
        });
        Ok(())
    }

    /// Adds a source that makes its bursts when they are due.
    pub fn add_source(&mut self, src: Box<dyn BurstSource>) {
        self.lazy.push(Lazy {
            src,
            next: None,
            ended: false,
        });
    }

    /// Turns the lazy bursts due before `until` into entries.
    fn admit(&mut self, until: u64) {
        for l in &mut self.lazy {
            loop {
                if l.next.is_none() && !l.ended {
                    l.next = l.src.next_burst();
                    l.ended = l.next.is_none();
                }
                let due = match &l.next {
                    Some(b) => (b.start_s.max(0.0) * self.rate).round() as u64 <= until,
                    None => false,
                };
                if !due {
                    break;
                }
                let b = l.next.take().expect("checked above");
                let ok = b.start_s.is_finite() && b.level_db.is_finite() && b.offset_hz.is_finite();
                if !ok || b.offset_hz.abs() > self.rate / 2.0 {
                    continue;
                }
                self.entries.push(Entry {
                    start: (b.start_s.max(0.0) * self.rate).round() as u64,
                    gain: gain_from_db(b.level_db),
                    nco: (b.offset_hz != 0.0).then(|| Nco::new(self.rate, b.offset_hz)),
                    kind: Kind::Burst {
                        samples: b.samples,
                        period: 0,
                    },
                    done: false,
                });
            }
        }
    }

    /// Adds `src[..]` (the source's samples for `out`'s span) times the gain and offset into `out`, through the scratch buffer.
    fn add(
        scratch: &mut [Complex32],
        nco: &mut Option<Nco>,
        gain: f32,
        src: &[Complex32],
        out: &mut [Complex32],
    ) {
        for (s, o) in src.chunks(PIECE).zip(out.chunks_mut(PIECE)) {
            let buf = &mut scratch[..s.len()];
            for (b, v) in buf.iter_mut().zip(s) {
                *b = *v * gain;
            }
            if let Some(n) = nco {
                n.mix_in_place(buf);
            }
            for (o, b) in o.iter_mut().zip(buf.iter()) {
                *o += *b;
            }
        }
    }

    fn fill(&mut self, out: &mut [Complex32]) {
        out.iter_mut().for_each(|v| *v = Complex32::default());
        let (a, b) = (self.pos, self.pos + out.len() as u64);
        self.admit(b);
        let scratch = &mut self.scratch;
        for e in &mut self.entries {
            if e.done || e.start >= b {
                continue;
            }
            match &mut e.kind {
                Kind::Burst { samples, period } => {
                    let len = samples.len() as u64;
                    match a.saturating_sub(e.start).checked_div(*period) {
                        None => {
                            // Sent once.
                            let (s, t) = (e.start.max(a), (e.start + len).min(b));
                            if s < t {
                                let src = &samples[(s - e.start) as usize..(t - e.start) as usize];
                                Self::add(
                                    scratch,
                                    &mut e.nco,
                                    e.gain,
                                    src,
                                    &mut out[(s - a) as usize..(t - a) as usize],
                                );
                            }
                            if e.start + len <= b {
                                e.done = true;
                            }
                        }
                        Some(first) => {
                            // Every repeat that overlaps [a, b).
                            let mut r = first;
                            loop {
                                let rs = e.start + r * *period;
                                if rs >= b {
                                    break;
                                }
                                let (s, t) = (rs.max(a), (rs + len).min(b));
                                if s < t {
                                    let src = &samples[(s - rs) as usize..(t - rs) as usize];
                                    Self::add(
                                        scratch,
                                        &mut e.nco,
                                        e.gain,
                                        src,
                                        &mut out[(s - a) as usize..(t - a) as usize],
                                    );
                                }
                                r += 1;
                            }
                        }
                    }
                }
                Kind::Stream { gen, stop } => {
                    let s = e.start.max(a);
                    let t = stop.map_or(b, |st| st.min(b));
                    let mut p = s;
                    while p < t {
                        let k = ((t - p) as usize).min(PIECE);
                        let got = gen.generate(&mut scratch[..k]);
                        let o = &mut out[(p - a) as usize..(p - a) as usize + got];
                        let buf = &mut scratch[..got];
                        for v in buf.iter_mut() {
                            *v *= e.gain;
                        }
                        if let Some(n) = &mut e.nco {
                            n.mix_in_place(buf);
                        }
                        for (o, v) in o.iter_mut().zip(buf.iter()) {
                            *o += *v;
                        }
                        p += got as u64;
                        if got < k {
                            e.done = true;
                            break;
                        }
                    }
                    if stop.is_some_and(|st| st <= b) {
                        e.done = true;
                    }
                }
            }
        }
        self.entries.retain(|e| !e.done);
        self.pos = b;
    }
}

impl Generator for Scene {
    fn rate_hz(&self) -> f64 {
        self.rate
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n = match self.end {
            Some(e) => (e.saturating_sub(self.pos) as usize).min(out.len()),
            None => out.len(),
        };
        self.fill(&mut out[..n]);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tone {
        rate: f64,
        n: u64,
        left: u64,
    }

    impl Generator for Tone {
        fn rate_hz(&self) -> f64 {
            self.rate
        }
        fn generate(&mut self, out: &mut [Complex32]) -> usize {
            let k = (out.len() as u64).min(self.left) as usize;
            for o in &mut out[..k] {
                *o = Complex32::new(1.0, 0.0);
                self.n += 1;
            }
            self.left -= k as u64;
            k
        }
    }

    fn bursts_scene() -> Scene {
        let mut s = Scene::new(10_000.0).unwrap().with_duration(2.0).unwrap();
        s.add_burst(0.5, 1000.0, 0.0, vec![Complex32::new(1.0, 0.0); 300])
            .unwrap();
        s.add_repeating(0.0, 0.3, -2000.0, -6.0, vec![Complex32::new(0.0, 1.0); 100])
            .unwrap();
        s.add_stream(
            1.0,
            Some(0.25),
            500.0,
            0.0,
            Box::new(Tone {
                rate: 10_000.0,
                n: 0,
                left: u64::MAX,
            }),
        )
        .unwrap();
        let mut k = 0;
        s.add_source(Box::new(move || {
            k += 1;
            (k <= 3).then(|| Burst {
                start_s: 0.05 + 0.5 * k as f64,
                offset_hz: 0.0,
                level_db: 6.0206,
                samples: vec![Complex32::new(0.5, 0.0); 10],
            })
        }));
        s
    }

    #[test]
    fn sources_land_at_their_times_offsets_and_levels() {
        let mut s = bursts_scene();
        let mut out = vec![Complex32::default(); 30_000];
        assert_eq!(s.generate(&mut out), 20_000);
        let rate = 10_000.0;
        // The once burst: 300 samples from 5000 at +1 kHz, unit amplitude.
        let freq = |a: usize, b: usize| {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for n in a..b - 1 {
                let p = out[n + 1] * out[n].conj();
                re += p.re as f64;
                im += p.im as f64;
            }
            im.atan2(re) * rate / std::f64::consts::TAU
        };
        assert!((freq(5010, 5290) - 1000.0).abs() < 0.5);
        assert!((out[5100].norm() - 1.0).abs() < 1e-5);
        assert_eq!(out[5300], Complex32::default());
        // The repeating burst: every 3000 samples, 100 long, -6 dB, at -2 kHz.
        for r in [0usize, 1, 2, 3, 5] {
            // (Repeat 4, at 12000, lies on the stream.)
            let st = r * 3000;
            assert!((out[st + 50].norm() - 0.501).abs() < 1e-3, "repeat {r}");
            assert_eq!(out[st + 100], Complex32::default());
            assert!((freq(st + 2, st + 98) + 2000.0).abs() < 0.5);
        }
        // The stream: 2500 samples from 10000 at +500 Hz.
        assert!((freq(10_600, 11_900) - 500.0).abs() < 0.5);
        assert_eq!(out[12_500], Complex32::default());
        assert!((out[12_499].norm() - 1.0).abs() < 1e-5);
        // The lazy bursts: 10 samples at amplitude 1 (0.5 and 6 dB) at 0.55 s and 1.55 s (the one at 1.05 s lies on the stream).
        for st in [5500usize, 15_500] {
            assert!(
                (out[st + 5] - Complex32::new(1.0, 0.0)).norm() < 1e-4,
                "lazy burst at {st}"
            );
            assert_eq!(out[st - 1], Complex32::default());
        }
        assert_eq!(out[15_510], Complex32::default());
    }

    #[test]
    fn any_cut_returns_the_same_scene() {
        let mut a = bursts_scene();
        let mut b = bursts_scene();
        let mut x = vec![Complex32::default(); 20_000];
        a.generate(&mut x);
        let mut y = vec![Complex32::default(); 20_000];
        for c in y.chunks_mut(777) {
            b.generate(c);
        }
        for (i, (p, q)) in x.iter().zip(&y).enumerate() {
            assert!((p - q).norm() < 1e-6, "sample {i}");
        }
        let mut s = Scene::new(1000.0).unwrap();
        assert!(s.add_burst(-1.0, 0.0, 0.0, vec![]).is_err());
        assert!(s.add_burst(0.0, 600.0, 0.0, vec![]).is_err());
        assert!(s
            .add_repeating(0.0, 0.001, 0.0, 0.0, vec![Complex32::default(); 5])
            .is_err());
    }
}
