//! The APT line format and the line decoder: envelope values at the word rate in, picture lines out.
//!
//! Standard: NOAA KLM User's Guide, section 4.2 (APT), Figures 4.2.3-2 and 4.2.3-3, Table 4.2.3-2.

/// Words a second.
pub const WORD_RATE: f64 = 4160.0;
/// Words of a line.
pub const LINE_WORDS: usize = 2080;
/// Words of a sync field.
pub const SYNC_WORDS: usize = 39;
/// The first word of picture A.
pub const PICTURE_A: usize = 86;
/// The first word of picture B.
pub const PICTURE_B: usize = 1126;
/// Words of a picture.
pub const PICTURE_WORDS: usize = 909;
/// The first word of sync B.
pub const SYNC_B: usize = 1040;
/// The subcarrier, Hz.
pub const SUBCARRIER_HZ: f64 = 2400.0;
/// The peak deviation of the carrier, Hz.
pub const DEVIATION_HZ: f64 = 17_000.0;
/// The low level of a sync pulse, as the 8-bit input of the converter on board.
pub const SYNC_LOW: u8 = 11;
/// The high level of a sync pulse.
pub const SYNC_HIGH: u8 = 244;
/// The least subcarrier amplitude over the greatest, for a modulation depth of 87 %.
pub const LEAST_AMPLITUDE: f32 = (1.0 - 0.87) / (1.0 + 0.87);

/// The best correlation that locks.
const LOCK_AT: f32 = 0.45;
/// The least correlation of a tracked line.
const TRACK_AT: f32 = 0.35;
/// Words searched to each side of the expected line start. The line period is followed, so a clock error does not add up.
const REACH: usize = 2;
/// The lock ends when the running mean of the sync correlation (0 for a line without sync) falls below this.
const HOLD_AT: f32 = 0.3;
/// Envelope values for each word at the input of the decoder.
pub const OVERSAMPLE: usize = 4;
/// Values of a line.
const LINE: usize = LINE_WORDS * OVERSAMPLE;
/// More misses in a row than this end the lock.
const MAX_MISSES: u32 = 12;

/// The levels of sync A, one for each word: 4 low, 7 cycles of 2 high and 2 low, 7 low.
pub const fn sync_a() -> [bool; SYNC_WORDS] {
    let mut p = [false; SYNC_WORDS];
    let mut k = 0;
    while k < 7 {
        p[4 + 4 * k] = true;
        p[5 + 4 * k] = true;
        k += 1;
    }
    p
}

/// The levels of sync B: 4 low, 7 cycles of 3 high and 2 low.
pub const fn sync_b() -> [bool; SYNC_WORDS] {
    let mut p = [false; SYNC_WORDS];
    let mut k = 0;
    while k < 7 {
        p[4 + 5 * k] = true;
        p[5 + 5 * k] = true;
        p[6 + 5 * k] = true;
        k += 1;
    }
    p
}

/// The 8-bit level of word `word` of line `line` of the test signal: the sync fields, the two pictures from `pixel(channel,
/// column, line)` (10 bits, the top 8 are sent), and mid grey elsewhere.
pub fn test_word(word: usize, line: u32, pixel: impl Fn(u16, u32, u32) -> u16) -> u8 {
    let sync = |on: bool| if on { SYNC_HIGH } else { SYNC_LOW };
    match word {
        w if w < SYNC_WORDS => sync(sync_a()[w]),
        w if (PICTURE_A..PICTURE_A + PICTURE_WORDS).contains(&w) => {
            (pixel(0, (w - PICTURE_A) as u32, line) >> 2) as u8
        }
        w if (SYNC_B..SYNC_B + SYNC_WORDS).contains(&w) => sync(sync_b()[w - SYNC_B]),
        w if (PICTURE_B..PICTURE_B + PICTURE_WORDS).contains(&w) => {
            (pixel(1, (w - PICTURE_B) as u32, line) >> 2) as u8
        }
        _ => 128,
    }
}

/// The subcarrier amplitude of an 8-bit level: 1 for 255, [`LEAST_AMPLITUDE`] for 0.
pub fn amplitude(level: u8) -> f32 {
    LEAST_AMPLITUDE + (1.0 - LEAST_AMPLITUDE) * f32::from(level) / 255.0
}

/// A decoded line.
#[derive(Clone, Debug, PartialEq)]
pub struct AptLine {
    /// The 2080 words, 8 bits each.
    pub words: Vec<u8>,
    /// The correlation of its sync A with the pattern, -1 to 1.
    pub quality: f32,
    /// The sync was not found within the window. The line was taken at the expected place.
    pub missed: bool,
}

/// The size of the 1040 Hz burst of sync A in 39 word values: the amplitude of its fundamental. It does not depend on where
/// in a word the values were taken.
pub fn burst_amplitude(words: &[f32]) -> f32 {
    let (mut re, mut im) = (0f32, 0f32);
    for (k, &v) in words.iter().enumerate().take(32).skip(4) {
        match k % 4 {
            0 => re += v,
            1 => im += v,
            2 => re -= v,
            _ => im -= v,
        }
    }
    (re * re + im * im).sqrt() / 14.0
}

/// The line decoder. It finds sync A by correlation, follows the line start within a window, reads the words between the
/// values it holds, and scales the envelope to 8 bits with the sync levels.
#[derive(Clone, Debug)]
pub struct AptDecoder {
    pattern: [f32; SYNC_WORDS],
    vals: Vec<f32>,
    locked: bool,
    misses: u32,
    held: f32,
    lines: u64,
    /// The line period, in values.
    period: f64,
    /// Where the next line is expected to start, in values from the start of the buffer.
    expect: f64,
    black: f32,
    white: f32,
    burst_share: f32,
    /// Lines locked so far.
    pub locks: u32,
    /// Locks lost so far.
    pub losses: u32,
}

/// The normalised correlation of the 39 word values that start at value `at` with the pattern (both without their mean), -1
/// to 1.
fn correlate(v: &[f32], at: usize, pattern: &[f32; SYNC_WORDS]) -> f32 {
    let word = |k: usize| v[at + OVERSAMPLE * k];
    let mean = (0..SYNC_WORDS).map(word).sum::<f32>() / SYNC_WORDS as f32;
    let (mut dot, mut energy) = (0f32, 0f32);
    for (k, p) in pattern.iter().enumerate() {
        let d = word(k) - mean;
        dot += d * p;
        energy += d * d;
    }
    let pe: f32 = pattern.iter().map(|p| p * p).sum();
    if energy <= 1e-20 {
        0.0
    } else {
        dot / (energy * pe).sqrt()
    }
}

impl AptDecoder {
    /// A decoder. `burst_share` is the [`burst_amplitude`] that the envelope detector before the decoder shows for a sync A of
    /// height 1 (0.6366 for a detector that passes only the fundamental of the pulses).
    pub fn new(burst_share: f32) -> Self {
        let levels = sync_a();
        let mean = 14.0 / SYNC_WORDS as f32;
        let mut pattern = [0f32; SYNC_WORDS];
        for (p, &on) in pattern.iter_mut().zip(&levels) {
            *p = f32::from(u8::from(on)) - mean;
        }
        AptDecoder {
            pattern,
            vals: Vec::with_capacity(3 * LINE),
            locked: false,
            misses: 0,
            held: 0.0,
            lines: 0,
            period: LINE as f64,
            expect: 0.0,
            black: 0.0,
            white: 1.0,
            burst_share: burst_share.clamp(0.05, 1.0),
            locks: 0,
            losses: 0,
        }
    }

    /// The line lock holds.
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// The line clock of the signal against the clock of the values, ppm.
    pub fn clock_ppm(&self) -> f64 {
        (LINE as f64 / self.period - 1.0) * 1e6
    }

    /// Ends the lock and forgets the values held. The counters stay.
    pub fn reset(&mut self) {
        self.vals.clear();
        self.locked = false;
        self.misses = 0;
        self.lines = 0;
        self.period = LINE as f64;
    }

    /// The place of the correlation peak near value `at`, from the peak and its two neighbours.
    fn peak(&self, at: usize, best: f32) -> f64 {
        if at == 0 {
            return 0.0;
        }
        let a = correlate(&self.vals, at - 1, &self.pattern);
        let c = correlate(&self.vals, at + 1, &self.pattern);
        let den = a - 2.0 * best + c;
        let d = if den.abs() > 1e-9 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        at as f64 + f64::from(d)
    }

    /// The line that starts at value `start` (the centre of its first word), scaled with the sync levels.
    fn take(&mut self, start: f64, quality: f32, missed: bool) -> AptLine {
        let step = self.period / LINE_WORDS as f64;
        let raw: Vec<f32> = (0..LINE_WORDS)
            .map(|k| {
                let t = (start + k as f64 * step).max(0.0);
                let i = (t as usize).min(self.vals.len() - 2);
                let f = (t - i as f64) as f32;
                self.vals[i] + f * (self.vals[i + 1] - self.vals[i])
            })
            .collect();
        if !missed {
            // Black from the flat low run at the end of the sync, white from the size of the burst.
            let black = raw[33..38].iter().sum::<f32>() / 5.0;
            let white = black + burst_amplitude(&raw) / self.burst_share;
            let w = if self.lines < 4 { 0.5 } else { 0.1 };
            if self.lines == 0 {
                (self.black, self.white) = (black, white);
            } else {
                self.black += w * (black - self.black);
                self.white += w * (white - self.white);
            }
        }
        let span = (self.white - self.black).max(1e-9);
        let (lo, hi) = (f32::from(SYNC_LOW), f32::from(SYNC_HIGH));
        let words = raw
            .iter()
            .map(|x| {
                (lo + (x - self.black) / span * (hi - lo))
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
            .collect();
        self.lines += 1;
        AptLine {
            words,
            quality,
            missed,
        }
    }

    /// Drops the values before the line after the one that started at `start`, less the search reach and one word.
    fn next_line(&mut self, start: f64, reach: usize) {
        let next = start + self.period;
        let keep = (next as usize).saturating_sub(reach + OVERSAMPLE);
        self.vals.drain(..keep.min(self.vals.len()));
        self.expect = next - keep as f64;
    }

    /// The best correlation among the values `from..=to`, and its place.
    fn best(&self, from: usize, to: usize) -> (f32, usize) {
        let (mut best, mut at) = (-1f32, from);
        for s in from..=to {
            let c = correlate(&self.vals, s, &self.pattern);
            if c > best {
                (best, at) = (c, s);
            }
        }
        (best, at)
    }

    /// Takes envelope values at [`OVERSAMPLE`] times the word rate and appends the lines they complete to `out`.
    pub fn push(&mut self, values: &[f32], out: &mut Vec<AptLine>) {
        self.vals.extend_from_slice(values);
        let reach = REACH * OVERSAMPLE;
        let sync = SYNC_WORDS * OVERSAMPLE;
        loop {
            if !self.locked {
                // Three lines and two sync lengths: every start position of one line has two whole lines behind it.
                if self.vals.len() < 3 * LINE + 2 * sync + 2 * OVERSAMPLE {
                    return;
                }
                let (best, at) = self.best(0, LINE + sync - 1);
                // A lock needs the sync of the next two lines too: noise passes the first test now and then.
                let (next, _) = self.best(at + LINE - OVERSAMPLE, at + LINE + OVERSAMPLE);
                let (third, _) = self.best(at + 2 * LINE - OVERSAMPLE, at + 2 * LINE + OVERSAMPLE);
                if best < LOCK_AT || next < LOCK_AT || third < LOCK_AT {
                    self.vals.drain(..LINE);
                    continue;
                }
                self.locked = true;
                self.locks += 1;
                self.misses = 0;
                self.held = best;
                self.period = LINE as f64;
                let start = self.peak(at, best);
                let line = self.take(start, best, false);
                out.push(line);
                self.next_line(start, reach);
            } else {
                let expected = self.expect.round() as usize;
                if self.vals.len() < expected + reach + LINE + 2 * OVERSAMPLE {
                    return;
                }
                let (best, at) = self.best(expected - reach, expected + reach);
                let missed = best < TRACK_AT;
                let start = if missed {
                    self.misses += 1;
                    self.expect
                } else {
                    self.misses = 0;
                    let start = self.peak(at, best);
                    // The line period follows the distance of the line starts.
                    let measured = self.period + (start - self.expect);
                    self.period += 0.2 * (measured - self.period);
                    self.period = self.period.clamp(0.999 * LINE as f64, 1.001 * LINE as f64);
                    start
                };
                self.held += 0.25 * (if missed { 0.0 } else { best } - self.held);
                if self.misses > MAX_MISSES || self.held < HOLD_AT {
                    self.locked = false;
                    self.losses += 1;
                    self.lines = 0;
                    continue;
                }
                let line = self.take(start, best, missed);
                out.push(line);
                self.next_line(start, reach);
            }
        }
    }
}

/// The first word of the steady middle of space A, whose noise the line filter measures.
pub const SPACE_A_QUIET: usize = 47;
/// Words of the steady middle of space A.
pub const SPACE_A_QUIET_WORDS: usize = 31;
/// The weight of a new line in the averaged noise.
const LINE_NOISE_WEIGHT: f32 = 0.1;

/// The line filter of pictures A and B. Per line: the variance and the robust variance (from the median absolute deviation)
/// of the 31 middle words of space A, each averaged over lines (the first line sets them, then a weight of 0.1); in each
/// picture, click removal at 4 robust standard deviations from the median of 5, then the line Wiener filter over 7 words with
/// the plain variance. Sync fields and spaces are left as they are. Nothing is allocated after the first line.
#[derive(Clone, Debug, Default)]
pub struct LineFilter {
    noise: Option<(f32, f32)>,
    scratch: Vec<f64>,
    a: Vec<f32>,
    b: Vec<f32>,
}

impl LineFilter {
    /// A filter with no noise measured.
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets the measured noise.
    pub fn reset(&mut self) {
        self.noise = None;
    }

    /// The averaged noise: the variance and the robust variance of one word, in levels squared.
    pub fn noise(&self) -> Option<(f32, f32)> {
        self.noise
    }

    /// Measures the noise of a line of [`LINE_WORDS`] words and filters its two pictures in place. Lines of another length
    /// are left as they are.
    pub fn filter(&mut self, words: &mut [u8]) {
        if words.len() != LINE_WORDS {
            return;
        }
        let mut quiet = [0f32; SPACE_A_QUIET_WORDS];
        for (q, &w) in quiet.iter_mut().zip(&words[SPACE_A_QUIET..]) {
            *q = f32::from(w);
        }
        let n = SPACE_A_QUIET_WORDS as f32;
        let mean = quiet.iter().sum::<f32>() / n;
        let var = quiet.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
        let robust = sdrkit_algo_dsp::denoise::robust_sigma(&quiet, &mut self.scratch)
            .unwrap_or(0.0)
            .powi(2);
        let (v, r) = match self.noise {
            None => (var, robust),
            Some((v, r)) => (
                v + LINE_NOISE_WEIGHT * (var - v),
                r + LINE_NOISE_WEIGHT * (robust - r),
            ),
        };
        self.noise = Some((v, r));
        for first in [PICTURE_A, PICTURE_B] {
            let picture = &mut words[first..first + PICTURE_WORDS];
            self.a.clear();
            self.a.extend(picture.iter().map(|&w| f32::from(w)));
            self.b.resize(PICTURE_WORDS, 0.0);
            let clicks = sdrkit_algo_dsp::denoise::line_clicks(
                &self.a,
                r.sqrt(),
                sdrkit_algo_dsp::denoise::LINE_CLICK_SIGMAS,
                &mut self.b,
            );
            let smooth = sdrkit_algo_dsp::denoise::line_wiener(
                &self.b,
                v,
                sdrkit_algo_dsp::denoise::LINE_WIENER_HALF,
                &mut self.a,
            );
            if clicks.is_ok() && smooth.is_ok() {
                for (w, x) in picture.iter_mut().zip(&self.a) {
                    *w = x.round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}
