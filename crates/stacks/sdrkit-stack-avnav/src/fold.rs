//! The identification of a weak beacon from the average of its repeats. A beacon sends the same Morse identification again and
//! again with a fixed period. The component keeps the depth of each keying path (the 400 Hz tone, the 1020 Hz tone and the keyed
//! carrier) for three minutes, finds the period of each from its autocorrelation, averages the repeats and reads the Morse of
//! the average.
//! Standard: ICAO Annex 10 Volume I, 3.4; ITU-R M.1677-1 Part I.

use sdrkit_algo_dsp::denoise::MAD_TO_SIGMA;
use sdrkit_algo_dsp::goertzel::ToneBank;
use sdrkit_algo_dsp::stats::quantile_f64;
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use sdrkit_util_text::morse::{Decoder, DecoderConfig, Event};

/// Depth values a second.
pub const DEPTH_RATE_HZ: f64 = 50.0;
/// Depth values kept: three minutes.
pub const HISTORY: usize = 9000;
/// Depth values between two evaluations: 5 s.
pub const EVALUATE_EVERY: usize = 250;
/// The shortest and the longest period looked for, in depth values: 2 s and 30 s.
pub const MIN_LAG: usize = 100;
/// See [`MIN_LAG`].
pub const MAX_LAG: usize = 1500;
/// The least number of whole periods in the history.
pub const MIN_PERIODS: usize = 3;
/// The least robust z-score of the autocorrelation at the period.
pub const MIN_Z: f64 = 3.0;
/// The least autocorrelation at the period.
pub const MIN_CORRELATION: f64 = 0.1;
/// The least swing of the average (10th to 90th percentile) over its noise.
pub const MIN_QUALITY: f64 = 6.0;
/// The tones of the two tone paths, Hz.
pub const TONES_HZ: [f64; 2] = [400.0, 1020.0];

/// What an evaluation of the average read.
#[derive(Clone, Debug, PartialEq)]
pub struct FoldRead {
    /// The identification: the same word in this evaluation and the one before it.
    pub text: String,
    /// The period of the identification, s.
    pub period_s: f64,
    /// The robust z-score of the autocorrelation at the period.
    pub z: f64,
    /// The swing of the average over its noise.
    pub quality: f64,
}

/// The result of one evaluation (`ident.fold`): a reading, or none when no path gives the same word twice in a row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IdentFold {
    /// The reading.
    pub read: Option<FoldRead>,
}

impl Item for IdentFold {
    const KIND: &'static str = "ident.fold";
}

/// The period and the quality of a folded path. The average itself is in [`Work::profile`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Folded {
    /// The period, depth values (fractional).
    pub period: f64,
    /// The robust z-score of the autocorrelation at the period.
    pub z: f64,
    /// The swing of the average over its noise.
    pub quality: f64,
    /// The level half way between the 10th and the 90th percentile of the average.
    pub slice: f32,
}

/// The buffers of [`fold`] and [`read`]. They grow once.
#[derive(Clone, Debug, Default)]
pub struct Work {
    centred: Vec<f32>,
    correlation: Vec<f64>,
    scratch: Vec<f64>,
    values: Vec<f64>,
    /// The average of the repeats, one period long.
    pub profile: Vec<f32>,
    runs: Vec<(bool, usize)>,
    word: String,
    last: String,
    before: String,
}

/// Folds the depth values `x` (oldest first, [`DEPTH_RATE_HZ`] a second) at their period and leaves the average in
/// `w.profile`.
///
/// The mean is removed. The autocorrelation `r(L) / r(0)` is taken for lags of 2 to 30 s, at most a third of `x`. The period is
/// the shortest lag that is a local peak within 80 % of the highest value (two and three periods correlate as well as one). It
/// is refused when the robust z-score of `r` at the period, against the median and 1.4826 times the median absolute deviation
/// of `r` over the lags, is under 3, or `r` is under 0.1. A parabola through the peak and its neighbours gives the fractional
/// period. The newest whole periods (at least 3) are averaged, read with linear interpolation. The noise of the average is
/// `sqrt(mean residual variance / (K - 1))` for `K` periods; the quality is the swing from the 10th to the 90th percentile over
/// that noise, refused under 6.
pub fn fold(x: &[f32], w: &mut Work) -> Option<Folded> {
    let n = x.len();
    let max_lag = MAX_LAG.min(n / 3);
    if max_lag < MIN_LAG + 2 {
        return None;
    }
    let mean = x.iter().map(|&v| f64::from(v)).sum::<f64>() / n as f64;
    w.centred.clear();
    w.centred
        .extend(x.iter().map(|&v| (f64::from(v) - mean) as f32));
    let y = &w.centred;
    let r0: f64 = y.iter().map(|&v| f64::from(v) * f64::from(v)).sum();
    if r0 <= 0.0 {
        return None;
    }
    // One lag below the shortest and one above the longest, for the test of a local peak.
    let first = MIN_LAG - 1;
    w.correlation.clear();
    for lag in first..=max_lag + 1 {
        let s: f32 = y[..n - lag].iter().zip(&y[lag..]).map(|(a, b)| a * b).sum();
        w.correlation.push(f64::from(s) / r0);
    }
    let r = &w.correlation;
    let inner = 1..r.len() - 1;
    let highest = r[inner.clone()].iter().copied().fold(f64::MIN, f64::max);
    if highest <= 0.0 {
        return None;
    }
    let k = inner
        .clone()
        .find(|&k| r[k] >= 0.8 * highest && r[k] >= r[k - 1] && r[k] >= r[k + 1])?;
    let median = quantile_f64(&r[inner.clone()], 0.5, &mut w.scratch)?;
    w.values.clear();
    w.values.extend(r[inner].iter().map(|v| (v - median).abs()));
    let deviation = MAD_TO_SIGMA * quantile_f64(&w.values, 0.5, &mut w.scratch)?;
    if deviation <= 0.0 {
        return None;
    }
    let z = (r[k] - median) / deviation;
    if z < MIN_Z || r[k] < MIN_CORRELATION {
        return None;
    }
    let bend = r[k - 1] - 2.0 * r[k] + r[k + 1];
    let offset = if bend < 0.0 {
        (0.5 * (r[k - 1] - r[k + 1]) / bend).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let period = (first + k) as f64 + offset;
    let periods = (n as f64 / period).floor() as usize;
    if periods < MIN_PERIODS {
        return None;
    }
    let len = period.round() as usize;
    let at = |t: f64| {
        let t = t.clamp(0.0, (n - 1) as f64);
        let i = (t.floor() as usize).min(n - 2);
        let f = (t - i as f64) as f32;
        x[i] + f * (x[i + 1] - x[i])
    };
    w.profile.clear();
    let mut residual = 0.0f64;
    for j in 0..len {
        let (mut sum, mut sq) = (0.0f64, 0.0f64);
        for p in 0..periods {
            let v = f64::from(at(n as f64 - (p + 1) as f64 * period + j as f64));
            sum += v;
            sq += v * v;
        }
        let m = sum / periods as f64;
        residual += (sq / periods as f64 - m * m).max(0.0);
        w.profile.push(m as f32);
    }
    let noise = (residual / len as f64 / (periods - 1) as f64).sqrt();
    w.values.clear();
    w.values.extend(w.profile.iter().map(|&v| f64::from(v)));
    let low = quantile_f64(&w.values, 0.1, &mut w.scratch)?;
    let high = quantile_f64(&w.values, 0.9, &mut w.scratch)?;
    if noise <= 0.0 {
        return None;
    }
    let quality = (high - low) / noise;
    if quality < MIN_QUALITY {
        return None;
    }
    Some(Folded {
        period,
        z,
        quality,
        slice: (0.5 * (low + high)) as f32,
    })
}

/// Reads the Morse of the average in `w.profile`, sliced at `slice`. The reading starts after the longest gap of the period and
/// runs three times over the period. The identification is the last word; it must equal the word before it and hold letters
/// only. `None` otherwise.
pub fn read<'a>(slice: f32, dec: &mut Decoder, w: &'a mut Work) -> Option<&'a str> {
    let len = w.profile.len();
    let on = |i: usize| w.profile[i % len] > slice;
    // The longest gap, in a period that repeats.
    let mut longest = (0usize, 0usize);
    let mut run = 0usize;
    for i in 0..2 * len {
        if on(i) {
            run = 0;
        } else {
            run += 1;
            if run > longest.0 && run <= len {
                longest = (run, i + 1);
            }
        }
    }
    if longest.0 == 0 || longest.0 >= len {
        return None;
    }
    let start = longest.1 % len;
    w.runs.clear();
    for i in 0..len {
        let v = on(start + i);
        match w.runs.last_mut() {
            Some((state, count)) if *state == v => *count += 1,
            _ => w.runs.push((v, 1)),
        }
    }
    dec.reset();
    w.last.clear();
    w.before.clear();
    w.word.clear();
    for _ in 0..3 {
        for k in 0..=w.runs.len() {
            {
                let word = &mut w.word;
                let mut put = |e: Event| {
                    if let Event::Char(s) = e {
                        word.push_str(s.text_or("?"));
                    }
                };
                match w.runs.get(k) {
                    Some(&(state, count)) => {
                        dec.push(state, count as f64 / DEPTH_RATE_HZ, &mut put);
                    }
                    // The period ends with its longest gap: the word ends there.
                    None => dec.flush(&mut put),
                }
            }
            if !w.word.is_empty() {
                std::mem::swap(&mut w.before, &mut w.last);
                std::mem::swap(&mut w.last, &mut w.word);
                w.word.clear();
            }
        }
    }
    let letters = !w.last.is_empty() && w.last.bytes().all(|b| b.is_ascii_uppercase());
    (letters && w.last == w.before).then_some(w.last.as_str())
}

const ENABLED: Setting<field::Bool> = Setting::new("enabled", "Average repeated idents", "", true)
    .help(
        "The identification is also read from the average of its repeats over up to three minutes.",
    );

sdrkit_core::settings! {
    /// The settings of [`IdentFolder`].
    pub enum IdentFolderSetting {
        /// The average of the repeats is read.
        Enabled = ENABLED,
    }
}

/// One keying path: its depth values in a ring.
#[derive(Clone, Debug, Default)]
struct Path {
    values: Vec<f32>,
    next: usize,
    filled: usize,
}

impl Path {
    fn push(&mut self, v: f32) {
        if self.values.len() < HISTORY {
            self.values.resize(HISTORY, 0.0);
        }
        self.values[self.next] = v;
        self.next = (self.next + 1) % HISTORY;
        self.filled = (self.filled + 1).min(HISTORY);
    }
    /// The values, oldest first, into `out`.
    fn ordered(&self, out: &mut Vec<f32>) {
        out.clear();
        let begin = (self.next + HISTORY - self.filled) % HISTORY;
        out.extend((0..self.filled).map(|i| self.values[(begin + i) % HISTORY]));
    }
}

/// The identification folder (`avnav.ident_folder`): real `am` (the modulation over the carrier) and the channel `iq` in,
/// [`IdentFold`] out every 5 s. Every 20 ms it takes the depth of the 400 Hz and the 1020 Hz tone of `am` and the mean level of
/// `iq`. Each evaluation folds every path ([`fold`]) and reads it ([`read`]); the path with the best quality wins. A word is
/// reported when two evaluations in a row read it.
pub struct IdentFolder {
    enabled: bool,
    rate: f64,
    block: usize,
    bank: Option<ToneBank>,
    paths: [Path; 3],
    level: f64,
    level_n: usize,
    since: usize,
    ordered: Vec<f32>,
    work: Work,
    decoder: Decoder,
    previous: String,
    candidate: String,
}

impl Default for IdentFolder {
    fn default() -> Self {
        IdentFolder {
            enabled: ENABLED.default,
            rate: 0.0,
            block: 1,
            bank: None,
            paths: Default::default(),
            level: 0.0,
            level_n: 0,
            since: 0,
            ordered: Vec::new(),
            work: Work::default(),
            decoder: Decoder::new(DecoderConfig::idents())
                .unwrap_or_else(|_| unreachable!("the identification settings are valid")),
            previous: String::new(),
            candidate: String::new(),
        }
    }
}

impl IdentFolder {
    fn clear(&mut self) {
        for p in &mut self.paths {
            p.next = 0;
            p.filled = 0;
        }
        self.level = 0.0;
        self.level_n = 0;
        self.since = 0;
        self.previous.clear();
    }

    /// One evaluation of the three paths.
    fn evaluate(&mut self) -> IdentFold {
        let mut best: Option<Folded> = None;
        self.candidate.clear();
        for p in &self.paths {
            p.ordered(&mut self.ordered);
            let Some(f) = fold(&self.ordered, &mut self.work) else {
                continue;
            };
            if best.is_some_and(|b| b.quality >= f.quality) {
                continue;
            }
            if let Some(text) = read(f.slice, &mut self.decoder, &mut self.work) {
                self.candidate.clear();
                self.candidate.push_str(text);
                best = Some(f);
            }
        }
        let read = match best {
            Some(f) if self.candidate == self.previous => Some(FoldRead {
                text: self.candidate.clone(),
                period_s: f.period / DEPTH_RATE_HZ,
                z: f.z,
                quality: f.quality,
            }),
            _ => None,
        };
        std::mem::swap(&mut self.previous, &mut self.candidate);
        IdentFold { read }
    }
}

impl Component for IdentFolder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.ident_folder",
            vec![Port::real("am"), Port::iq("iq")],
            vec![Port::typed::<IdentFold>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(IdentFolderSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate <= 0.0 {
            return Err(ConfigError::NoRate);
        }
        self.rate = f.rate;
        self.block = ((f.rate / DEPTH_RATE_HZ).round() as usize).max(8);
        let tones: Vec<f64> = TONES_HZ
            .iter()
            .copied()
            .filter(|t| *t < 0.45 * f.rate)
            .collect();
        self.bank = Some(ToneBank::with_rate(f.rate, &tones, self.block)?);
        self.clear();
        Ok(vec![Format {
            rate: 0.0,
            ..f.with_kind(Kind::typed::<IdentFold>())
        }])
    }
    fn process(&mut self, io: &mut Io) {
        if !self.enabled {
            return;
        }
        let first = io.input(0).first();
        let rate = self.rate;
        // The keyed carrier: the mean level of each block of the channel.
        for s in io.iq(1) {
            self.level += f64::from(s.norm());
            self.level_n += 1;
            if self.level_n >= self.block {
                self.paths[2].push((self.level / self.block as f64) as f32);
                self.level = 0.0;
                self.level_n = 0;
            }
        }
        let Some(mut bank) = self.bank.take() else {
            return;
        };
        let x = io.real(0);
        let mut due: Option<usize> = None;
        let paths = &mut self.paths;
        let since = &mut self.since;
        bank.process(x, |end, amps| {
            for (p, a) in paths.iter_mut().zip(amps) {
                p.push(a.norm());
            }
            *since += 1;
            if *since >= EVALUATE_EVERY {
                *since = 0;
                due = Some(end);
            }
        });
        self.bank = Some(bank);
        if let Some(end) = due {
            let value = self.evaluate();
            io.items_out::<IdentFold>(0).push(Timed {
                time: (first + end as u64) as f64 / rate,
                value,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match IdentFolderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            IdentFolderSetting::Enabled => {
                self.enabled = ENABLED.read(id, v)?;
                if !self.enabled {
                    self.clear();
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(IdentFolderSetting::Enabled, Value::Bool(self.enabled))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_util_text::morse::Keyer;

    /// The keying of `text` at `wpm` as depth values: `repeats` periods of `period` values each, a mark 0.8 and a gap 0, with
    /// noise of standard deviation `noise` added.
    fn keyed(
        text: &str,
        wpm: f64,
        period: usize,
        repeats: usize,
        noise: f64,
        seed: u64,
    ) -> Vec<f32> {
        let mut one = Vec::new();
        for k in Keyer::new(wpm).unwrap().key(text) {
            let end = one.len() as f64 / DEPTH_RATE_HZ + k.seconds;
            while (one.len() as f64) / DEPTH_RATE_HZ < end - 1e-9 {
                one.push(if k.on { 0.8f32 } else { 0.0 });
            }
        }
        assert!(one.len() < period);
        one.resize(period, 0.0);
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        (0..repeats)
            .flat_map(|_| one.clone())
            .map(|v| v + (rng.normal() * noise) as f32)
            .collect()
    }

    fn folded_text(x: &[f32]) -> Option<(String, Folded)> {
        let mut w = Work::default();
        let mut dec = Decoder::new(DecoderConfig::idents()).unwrap();
        let f = fold(x, &mut w)?;
        read(f.slice, &mut dec, &mut w).map(|t| (t.to_string(), f))
    }

    #[test]
    fn a_clean_identification_is_folded_and_read() {
        let x = keyed("BND", 7.0, 290, 6, 0.0, 1);
        let (text, f) = folded_text(&x).unwrap();
        assert_eq!(text, "BND");
        assert!((f.period - 290.0).abs() < 0.5, "{f:?}");
    }

    /// Noise of 0.6 of the depth gives a wrong level in one value of five; the average of 30 repeats reads the word.
    #[test]
    fn noise_above_the_keying_is_averaged_down() {
        for seed in 1..=5 {
            let x = keyed("BND", 7.0, 290, 30, 0.5, seed);
            let (text, f) = folded_text(&x).unwrap_or_else(|| panic!("seed {seed}"));
            assert_eq!(text, "BND", "seed {seed}");
            assert!(f.quality >= MIN_QUALITY);
        }
    }

    /// Noise alone has no period: 200 histories of three minutes give no reading.
    #[test]
    fn noise_alone_gives_no_reading() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
        for run in 0..200 {
            let x: Vec<f32> = (0..HISTORY).map(|_| rng.normal().abs() as f32).collect();
            assert_eq!(folded_text(&x).map(|r| r.0), None, "run {run}");
        }
    }
}
