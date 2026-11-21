//! The clean audio of a tone signal: a list of steps, each with up to two tones at given levels for a given time.

use crate::{Param, TonesError};
use sdrkit_algo_dsp::nco::Nco;

/// One step of a tone sequence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The tones, Hz (0 for none).
    pub hz: [f64; 2],
    /// The amplitude of each tone (1.0 is full scale).
    pub amp: [f32; 2],
    /// How long, s.
    pub seconds: f64,
    /// A phase step added to each tone at the start of the step, cycles (0.5 is a reversal).
    pub phase_step: [f64; 2],
}

impl Step {
    /// One tone.
    pub fn one(hz: f64, amp: f32, seconds: f64) -> Step {
        Step {
            hz: [hz, 0.0],
            amp: [amp, 0.0],
            seconds,
            phase_step: [0.0; 2],
        }
    }

    /// Two tones at once.
    pub fn two(hz: [f64; 2], amp: [f32; 2], seconds: f64) -> Step {
        Step {
            hz,
            amp,
            seconds,
            phase_step: [0.0; 2],
        }
    }

    /// Silence.
    pub fn silence(seconds: f64) -> Step {
        Step::two([0.0; 2], [0.0; 2], seconds)
    }
}

/// A rendered step: its length in samples and where it starts.
#[derive(Clone, Copy, Debug)]
struct Laid {
    step: Step,
    start: u64,
    len: u64,
}

/// Audio of a list of [`Step`]s, with raised-cosine level changes. The phase of each tone runs on from step to step, and a
/// step can add a phase step (the CTCSS reverse burst). The samples do not depend on how the output is split across calls.
///
/// ```
/// use sdrkit_proto_tones::gen::{Step, ToneSequence};
/// // 0.1 s of 1000 Hz at amplitude 0.5, then 0.1 s of silence, at 8 kHz.
/// let mut s = ToneSequence::new(8000.0, &[Step::one(1000.0, 0.5, 0.1), Step::silence(0.1)], 0.0).unwrap();
/// assert_eq!(s.len(), 1600);
/// let mut out = vec![0f32; 1600];
/// assert_eq!(s.fill(&mut out), 1600);
/// assert!(out[..800].iter().any(|v| *v > 0.49) && out[800..].iter().all(|v| *v == 0.0));
/// ```
#[derive(Clone, Debug)]
pub struct ToneSequence {
    steps: Vec<Laid>,
    rate: f64,
    ramp: u64,
    total: u64,
    pos: u64,
    idx: usize,
    nco: [Nco; 2],
}

impl ToneSequence {
    /// The sequence at `rate` Hz. Each level change takes `ramp_s` seconds, as a raised-cosine edge centred on the change (0
    /// for hard keying). Errors for a tone outside `(0, rate / 2)` or a length that is not a finite positive number.
    pub fn new(rate: f64, steps: &[Step], ramp_s: f64) -> Result<Self, TonesError> {
        if !(rate.is_finite() && rate > 0.0) || !(ramp_s.is_finite() && ramp_s >= 0.0) {
            return Err(TonesError::BadParameter(Param::Sequence));
        }
        let mut laid = Vec::with_capacity(steps.len());
        let mut start = 0u64;
        for s in steps {
            if !(s.seconds.is_finite() && s.seconds >= 0.0)
                || s.hz
                    .iter()
                    .zip(&s.amp)
                    .any(|(&f, &a)| a != 0.0 && !(f > 0.0 && f < rate / 2.0) || !a.is_finite())
            {
                return Err(TonesError::BadParameter(Param::Step));
            }
            let len = (s.seconds * rate).round() as u64;
            laid.push(Laid {
                step: *s,
                start,
                len,
            });
            start += len;
        }
        let mut seq = ToneSequence {
            steps: laid,
            rate,
            ramp: (ramp_s * rate).round() as u64,
            total: start,
            pos: 0,
            idx: 0,
            nco: [Nco::new(rate, 0.0), Nco::new(rate, 0.0)],
        };
        seq.rewind();
        Ok(seq)
    }

    /// The length, samples.
    pub fn len(&self) -> u64 {
        self.total
    }

    /// Whether the sequence holds no samples.
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// The rate, Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Whether every sample has been given.
    pub fn done(&self) -> bool {
        self.pos >= self.total
    }

    /// Starts again from the first sample.
    pub fn rewind(&mut self) {
        self.pos = 0;
        self.idx = 0;
        for k in 0..2 {
            self.nco[k] = Nco::new(self.rate, 0.0);
        }
        self.enter(0);
    }

    fn enter(&mut self, i: usize) {
        if let Some(l) = self.steps.get(i) {
            for k in 0..2 {
                if l.step.hz[k] > 0.0 {
                    self.nco[k].set_freq(l.step.hz[k]);
                }
                if l.step.phase_step[k] != 0.0 {
                    let p = self.nco[k].phase_cycles() + l.step.phase_step[k];
                    self.nco[k].set_phase_cycles(p.rem_euclid(1.0));
                }
            }
        }
    }

    /// The amplitude of tone `k` at sample `n` of step `i` (with the ramps at the step's ends).
    fn amp(&self, i: usize, k: usize, n: u64) -> f32 {
        let l = &self.steps[i];
        let here = l.step.amp[k];
        if self.ramp == 0 {
            return here;
        }
        let half = self.ramp as f64 / 2.0;
        let blend = |other: f32, d: f64| -> f32 {
            // d: distance from the edge, samples (negative before it). Raised cosine from `other` to `here`.
            let t = ((d + half) / self.ramp as f64).clamp(0.0, 1.0);
            let g = (0.5 - 0.5 * (std::f64::consts::PI * t).cos()) as f32;
            other + (here - other) * g
        };
        let from_start = n as f64 + 0.5;
        let to_end = l.len as f64 - n as f64 - 0.5;
        if from_start < half {
            let prev = if i > 0 {
                self.steps[i - 1].step.amp[k]
            } else {
                0.0
            };
            let prev = if i > 0 && self.steps[i - 1].step.hz[k] == 0.0 {
                0.0
            } else {
                prev
            };
            blend(prev, from_start)
        } else if to_end < half {
            let next = self.steps.get(i + 1).map_or(0.0, |s| {
                if s.step.hz[k] == 0.0 {
                    0.0
                } else {
                    s.step.amp[k]
                }
            });
            blend(next, to_end)
        } else {
            here
        }
    }

    /// Writes the next samples into `out` (zeros past the end). Returns how many were inside the sequence.
    pub fn fill(&mut self, out: &mut [f32]) -> usize {
        let mut given = 0;
        for o in out.iter_mut() {
            while self.idx < self.steps.len()
                && self.pos >= self.steps[self.idx].start + self.steps[self.idx].len
            {
                self.idx += 1;
                self.enter(self.idx);
            }
            if self.idx >= self.steps.len() {
                *o = 0.0;
                continue;
            }
            let n = self.pos - self.steps[self.idx].start;
            let mut v = 0f32;
            for k in 0..2 {
                let z = self.nco[k].next_sample();
                if self.steps[self.idx].step.hz[k] > 0.0 {
                    v += self.amp(self.idx, k, n) * z.re;
                }
            }
            *o = v;
            self.pos += 1;
            given += 1;
        }
        given
    }
}
