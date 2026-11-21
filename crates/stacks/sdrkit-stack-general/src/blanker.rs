//! The impulse noise blanker. [`ImpulseBlanker`] is the algorithm, with one output for each input. [`ChannelBlanker`]
//! (`general.blanker`) is the component on the decimated channel.

use crate::smoothing::one_pole;
use crate::types::BlankerReport;
use sdrkit_algo_dsp::meter::db_to_power;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Port, RateFrom, Timed,
};

/// The impulse blanker.
#[derive(Clone, Debug)]
pub struct ImpulseBlanker {
    a: f32,
    thr: f32,
    mean: f32,
    started: bool,
    /// Samples in the mean so far, and how many make it warm (one time constant): until then it is a plain running mean of
    /// every sample, so a silent start (zeros from a filter's history) cannot lock it at zero.
    seen: u64,
    warm: u64,
    ring: Vec<Complex32>,
    /// Index of the next input sample.
    n: u64,
    delay: u64,
    in_blank: bool,
    hold: u32,
    gap_start: u64,
    last_good: Complex32,
    prev_end: u64,
    blanked: u64,
    counted: u64,
}

impl ImpulseBlanker {
    /// At `rate`, with a mean time constant `tau_s`, a threshold in dB and an output delay of `delay` samples (at least 4).
    pub fn new(rate: f64, tau_s: f64, threshold_db: f64, delay: usize) -> Self {
        let delay = delay.max(4);
        ImpulseBlanker {
            a: one_pole(tau_s, rate),
            thr: db_to_power(threshold_db) as f32,
            mean: 0.0,
            started: false,
            seen: 0,
            warm: (tau_s * rate).round().max(1.0) as u64,
            ring: vec![Complex32::default(); delay + 2],
            n: 0,
            delay: delay as u64,
            in_blank: false,
            hold: 0,
            gap_start: 0,
            last_good: Complex32::default(),
            prev_end: 0,
            blanked: 0,
            counted: 0,
        }
    }
    /// The delay in samples.
    pub fn delay(&self) -> usize {
        self.delay as usize
    }
    /// Changes the threshold (dB above the mean).
    pub fn set_threshold_db(&mut self, db: f64) {
        self.thr = db_to_power(db) as f32;
    }
    /// Clears the state (the mean starts again from the next sample).
    pub fn reset(&mut self) {
        self.mean = 0.0;
        self.started = false;
        self.seen = 0;
        self.ring.iter_mut().for_each(|v| *v = Complex32::default());
        self.n = 0;
        self.in_blank = false;
        self.hold = 0;
        self.gap_start = 0;
        self.last_good = Complex32::default();
        self.prev_end = 0;
    }
    /// Samples blanked and counted since the last [`take_share`](Self::take_share).
    pub fn take_share(&mut self) -> f32 {
        let s = if self.counted > 0 {
            100.0 * self.blanked as f32 / self.counted as f32
        } else {
            0.0
        };
        self.blanked = 0;
        self.counted = 0;
        s
    }
    #[inline]
    fn slot(&self, i: u64) -> usize {
        (i % self.ring.len() as u64) as usize
    }
    /// The oldest index not yet sent out.
    #[inline]
    fn first_pending(&self) -> u64 {
        self.n.saturating_sub(self.delay)
    }
    fn end_blank(&mut self, next: Complex32, m: u64) {
        let span = (m - self.gap_start + 1) as f32;
        let lg = self.last_good;
        let from = self.gap_start.max(self.first_pending());
        for k in from..m {
            let t = (k - self.gap_start + 1) as f32 / span;
            let s = self.slot(k);
            self.ring[s] = lg + (next - lg) * t;
        }
        self.in_blank = false;
        self.prev_end = m;
    }
    /// One sample in, one (delayed) sample out. `enabled` false passes everything through the same delay.
    #[inline]
    pub fn push(&mut self, x: Complex32, enabled: bool) -> Complex32 {
        let n = self.n;
        let p = x.norm_sqr();
        let limit = self.thr * self.mean;
        let impulsive = enabled && self.started && p > limit;
        if self.seen < self.warm {
            self.seen += 1;
            self.mean += (p - self.mean) / self.seen as f32;
            self.started = self.seen >= self.warm.min(64) && self.mean > 0.0;
        } else {
            self.mean += self.a * (p.min(limit) - self.mean);
        }
        let s = self.slot(n);
        self.ring[s] = x;
        if enabled {
            self.counted += 1;
        }
        if self.in_blank {
            if impulsive {
                self.hold = 3;
                self.ring[s] = self.last_good;
                self.blanked += 1;
            } else if self.hold > 0 {
                self.hold -= 1;
                self.ring[s] = self.last_good;
                self.blanked += 1;
            } else {
                self.end_blank(x, n);
            }
        } else if impulsive {
            let start = n
                .saturating_sub(3)
                .max(self.prev_end)
                .max(self.first_pending());
            self.gap_start = start;
            self.last_good = if start > 0 {
                self.ring[self.slot(start - 1)]
            } else {
                Complex32::default()
            };
            for k in start..=n {
                let sk = self.slot(k);
                self.ring[sk] = self.last_good;
            }
            self.blanked += n - start + 1;
            self.in_blank = true;
            self.hold = 3;
        }
        self.n += 1;
        if n >= self.delay {
            self.ring[self.slot(n - self.delay)]
        } else {
            Complex32::default()
        }
    }
    /// Blanks `buf` in place (delayed by [`delay`](Self::delay)).
    pub fn process(&mut self, buf: &mut [Complex32], enabled: bool) {
        for v in buf {
            *v = self.push(*v, enabled);
        }
    }
}

const ENABLED: Setting<field::Bool> = Setting::new("enabled", "Blanker", "", false);
const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold", "Blanker threshold", "dB", 12.0).range(6.0, 30.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`ChannelBlanker`].
    pub enum ChannelBlankerSetting {
        /// The blanker works.
        Enabled = ENABLED,
        /// The level above the 0.1 s mean that marks an impulse (dB).
        Threshold = THRESHOLD,
    }
}

/// The channel noise blanker (`general.blanker`): `Iq` to `Iq` at the decimated rate, with a 0.1 s mean, and a `report` output of
/// the share blanked once a second.
pub struct ChannelBlanker {
    enabled: bool,
    threshold: f64,
    nb: Option<ImpulseBlanker>,
    rate: f64,
    count: u64,
    second: u64,
}

impl Default for ChannelBlanker {
    fn default() -> Self {
        ChannelBlanker {
            enabled: false,
            threshold: 12.0,
            nb: None,
            rate: 0.0,
            count: 0,
            second: 1,
        }
    }
}

impl Component for ChannelBlanker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.blanker",
            vec![Port::iq("in")],
            vec![
                Port::iq("out").same_format_as(0),
                Port::typed::<BlankerReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .in_place(0, 0)
        .with_settings(ChannelBlankerSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.nb = Some(ImpulseBlanker::new(f.rate, 0.1, self.threshold, 8));
        self.second = (f.rate.round() as u64).max(1);
        self.count = 0;
        Ok(vec![
            f.clone(),
            Format::new(sdrkit_graph::Kind::typed::<BlankerReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let start = io.output(0).end();
        let enabled = self.enabled;
        let Some(nb) = self.nb.as_mut() else { return };
        let buf = io.pass_through::<Complex32>(0, 0);
        let n = buf.len() as u64;
        nb.process(buf, enabled);
        let before = self.count;
        self.count += n;
        if self.count / self.second != before / self.second {
            let share = nb.take_share();
            let t = (start + n) as f64 / self.rate;
            let thr = self.threshold as f32;
            io.items_out::<BlankerReport>(1).push(Timed {
                time: t,
                value: BlankerReport {
                    blanked_pct: share,
                    threshold_db: thr,
                },
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ChannelBlankerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChannelBlankerSetting::Enabled => self.enabled = ENABLED.read(id, v)?,
            ChannelBlankerSetting::Threshold => {
                self.threshold = THRESHOLD.read(id, v)?;
                if let Some(nb) = &mut self.nb {
                    nb.set_threshold_db(self.threshold);
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ChannelBlankerSetting::Enabled, Value::Bool(self.enabled)),
            (ChannelBlankerSetting::Threshold, Value::F64(self.threshold)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_impulse_is_replaced_by_a_line() {
        let mut nb = ImpulseBlanker::new(48e3, 0.05, 12.0, 16);
        let mut x: Vec<Complex32> = (0..400)
            .map(|i| Complex32::new(0.1 + 0.0001 * i as f32, 0.0))
            .collect();
        x[200] = Complex32::new(5.0, 5.0);
        let y: Vec<Complex32> = x.iter().map(|&v| nb.push(v, true)).collect();
        let d = nb.delay();
        for (i, v) in y.iter().enumerate().skip(d) {
            let want = x[i - d];
            let k = i - d;
            if (197..=203).contains(&k) {
                assert!((v.re - (0.1 + 0.0001 * k as f32)).abs() < 1e-3, "{k}: {v}");
            } else {
                assert_eq!(*v, want);
            }
        }
        assert!(nb.take_share() > 0.0);
    }

    #[test]
    fn disabled_is_a_pure_delay() {
        let mut nb = ImpulseBlanker::new(48e3, 0.05, 12.0, 8);
        let x: Vec<Complex32> = (0..100)
            .map(|i| Complex32::new(if i == 50 { 9.0 } else { 0.1 }, 0.0))
            .collect();
        let y: Vec<Complex32> = x.iter().map(|&v| nb.push(v, false)).collect();
        assert_eq!(&y[8..], &x[..92]);
    }
}
