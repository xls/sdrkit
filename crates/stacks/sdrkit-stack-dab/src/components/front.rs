//! The front-end components: channel centring (`dab.centring`) and the sample guard with its impulse blanker (`dab.guard`).

use super::items::{FrontStatus, OfdmStatusItem};
use crate::ofdm::SyncState;
use crate::settings::{CENTRING, IMPULSE_BLANKER};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::Values;
use sdrkit_core::{Complex32 as C, ConfigError, SettingError, SettingId, SettingSet, Value};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, RateRule,
    TagCarry, TagKind, TagSource, Timed,
};

/// The tag that the centring puts on the first sample after it moved the input: the shift in force from there, Hz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recentred {
    /// The whole shift, Hz.
    pub shift_hz: f64,
}
impl Item for Recentred {
    const KIND: &'static str = "dab.recentred";
}

sdrkit_core::settings! {
    /// The settings of [`Centring`].
    pub enum CentringSetting {
        /// Centring on or off.
        Centring = CENTRING,
    }
}

sdrkit_core::settings! {
    /// The settings of [`SampleGuard`].
    pub enum SampleGuardSetting {
        /// The impulse blanker on or off.
        ImpulseBlanker = IMPULSE_BLANKER,
    }
}

/// Channel centring (`dab.centring`): moves an ensemble found off centre to the middle while the receiver has not locked.
pub struct Centring {
    enabled: bool,
    rate: f64,
    n: usize,
    fft: Option<Fft>,
    win: Vec<f32>,
    buf: Vec<C>,
    acc: Vec<f64>,
    frames: usize,
    frames_needed: usize,
    since_lock: u64,
    locked: bool,
    shift: f64,
    nco: Option<Nco>,
    carry: TagCarry,
    tags: TagSource<Recentred>,
}

impl Default for Centring {
    fn default() -> Self {
        Centring {
            enabled: true,
            rate: 0.0,
            n: 256,
            fft: None,
            win: Vec::new(),
            buf: Vec::new(),
            acc: Vec::new(),
            frames: 0,
            frames_needed: 0,
            since_lock: 0,
            locked: false,
            shift: 0.0,
            nco: None,
            carry: TagCarry::same_rate(),
            tags: TagSource::new(),
        }
    }
}

impl Centring {
    /// The shift applied, Hz.
    pub fn shift_hz(&self) -> f64 {
        self.shift
    }

    /// The offset of the ensemble's centre from the middle, from the averaged spectrum (fft-shifted: index n/2 is DC), or
    /// `None` when no ensemble-like block stands out.
    pub fn find_centre(spec: &[f64], rate: f64) -> Option<f64> {
        let n = spec.len();
        let mut s = spec.to_vec();
        let dc = n / 2;
        if dc >= 4 && dc + 4 < n {
            let m = 0.5 * (s[dc - 3] + s[dc + 3]);
            for v in &mut s[dc - 1..=dc + 1] {
                *v = m;
            }
        }
        let w = ((1.536e6 * n as f64 / rate).round() as usize).max(1);
        let lo = (0.02 * n as f64) as usize;
        let hi = ((0.98 * n as f64) as usize).saturating_sub(w);
        if hi <= lo {
            return None;
        }
        let mut sorted = s.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let floor = sorted[n / 10] * w as f64;
        let mut sum: f64 = s[lo..lo + w].iter().sum();
        let mut best = (sum, lo);
        for st in lo + 1..=hi {
            sum += s[st + w - 1] - s[st - 1];
            if sum > best.0 {
                best = (sum, st);
            }
        }
        if best.0 < 4.0 * floor {
            return None;
        }
        // Lightly smoothed spectrum, half-power edges searched outwards from the strongest window.
        let sm: Vec<f64> = (0..n)
            .map(|i| s[i.saturating_sub(2)..(i + 3).min(n)].iter().sum::<f64>() / 5.0)
            .collect();
        let level = best.0 / w as f64;
        let half = level / 2.0;
        let mid = best.1 + w / 2;
        let mut l = mid;
        while l > 0 && sm[l] >= half {
            l -= 1;
        }
        let mut r = mid;
        while r + 1 < n && sm[r] >= half {
            r += 1;
        }
        if r - l < w / 3 {
            return None;
        }
        let c = (l + r) as f64 / 2.0 - (n / 2) as f64;
        Some(c * rate / n as f64)
    }
}

impl Component for Centring {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.centring",
            vec![Port::iq("in"), Port::typed::<OfdmStatusItem>("sync")],
            vec![Port::iq("out").same_format_as(0)],
        )
        .with_settings(CentringSetting::schema())
        .manual_tags()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        let mut n = 256;
        while n < 65_536 && (n as f64) * 1.536e6 / f.rate < 40.0 {
            n *= 2;
        }
        self.n = n;
        self.fft = Some(Fft::forward(n));
        self.win = (0..n)
            .map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos()) as f32)
            .collect();
        self.buf = Vec::with_capacity(n);
        self.acc = vec![0.0; n];
        self.frames = 0;
        self.frames_needed = ((0.3 * f.rate) / n as f64).ceil() as usize;
        self.since_lock = 0;
        self.locked = false;
        self.shift = 0.0;
        self.nco = None;
        self.carry = TagCarry::same_rate();
        Ok(vec![f.clone()])
    }

    fn process(&mut self, io: &mut Io) {
        for s in io.items::<OfdmStatusItem>(1) {
            self.locked = s.value.0.state == SyncState::Locked;
        }
        if self.locked {
            self.since_lock = 0;
        }
        let active = self.enabled && !self.locked && self.rate > 1.536e6 * 1.05;
        if !active {
            self.buf.clear();
        }
        let x = io.iq(0);
        let mut tag_at = None;
        {
            let out = io.iq_out(0);
            out.reserve(x.len());
            let n = self.n;
            for (i, &v) in x.iter().enumerate() {
                let y = match self.nco.as_mut() {
                    Some(o) => v * o.next_sample(),
                    None => v,
                };
                out.push(y);
                if !active {
                    continue;
                }
                self.since_lock += 1;
                self.buf.push(y * self.win[self.buf.len()]);
                if self.buf.len() < n {
                    continue;
                }
                let Some(fft) = self.fft.as_mut() else {
                    self.buf.clear();
                    continue;
                };
                fft.process(&mut self.buf);
                for (k, b) in self.buf.iter().enumerate() {
                    self.acc[(k + n / 2) % n] += b.norm_sqr() as f64;
                }
                self.buf.clear();
                self.frames += 1;
                if self.frames < self.frames_needed {
                    continue;
                }
                let found = Self::find_centre(&self.acc, self.rate);
                self.acc.iter_mut().for_each(|v| *v = 0.0);
                self.frames = 0;
                if let Some(c) = found {
                    if c.abs() > 60e3 && self.since_lock as f64 > 0.5 * self.rate {
                        self.shift += c;
                        self.nco = Some(Nco::new(self.rate, -self.shift));
                        self.since_lock = 0;
                        tag_at = Some(i + 1);
                    }
                }
            }
        }
        if let Some(at) = tag_at {
            let shift = self.shift;
            let tag = self.tags.tag(Recentred { shift_hz: shift });
            io.output(0).tag_here(at, TagKind::Typed(tag));
        }
        let (i0, o0) = io.pair(0, 0);
        self.carry.forward(i0, o0);
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match CentringSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            CentringSetting::Centring => self.enabled = CENTRING.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(CentringSetting::Centring, Value::Bool(self.enabled))])
    }
}

/// The sample guard and impulse blanker (`dab.guard`).
pub struct SampleGuard {
    blanker: bool,
    mean: f32,
    block_sum: f64,
    block_n: usize,
    bad: u64,
    blanked: u64,
    level_acc: f64,
    level_n: u64,
    report: u64,
    since: u64,
    rate: f64,
    input_rate: f64,
}

const GUARD_BLOCK: usize = 2048;

impl Default for SampleGuard {
    fn default() -> Self {
        SampleGuard {
            blanker: true,
            mean: 0.0,
            block_sum: 0.0,
            block_n: 0,
            bad: 0,
            blanked: 0,
            level_acc: 0.0,
            level_n: 0,
            report: 196_608,
            since: 0,
            rate: 0.0,
            input_rate: 0.0,
        }
    }
}

impl SampleGuard {
    /// Records the radio's own rate for the report.
    pub fn with_input_rate(mut self, r: f64) -> Self {
        self.input_rate = r;
        self
    }
}

impl Component for SampleGuard {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.guard",
            vec![Port::iq("in").rate(RateRule::Exact(crate::tables::RATE))],
            vec![
                Port::iq("out").same_format_as(0),
                Port::typed::<FrontStatus>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(SampleGuardSetting::schema())
        .in_place(0, 0)
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.rate = f.rate;
        self.report = (f.rate * 0.096).round() as u64;
        let blanker = self.blanker;
        let ir = self.input_rate;
        *self = SampleGuard {
            blanker,
            input_rate: ir,
            rate: f.rate,
            report: self.report,
            ..Default::default()
        };
        Ok(vec![
            f.clone(),
            Format::new(Kind::typed::<FrontStatus>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let blanker = self.blanker;
        let mut emit = None;
        {
            let x = io.pass_through::<C>(0, 0);
            for v in x.iter_mut() {
                if !(v.re.is_finite() && v.im.is_finite()) {
                    *v = C::default();
                    self.bad += 1;
                }
                let p = v.norm_sqr();
                self.level_acc += p as f64;
                self.level_n += 1;
                if self.mean > 0.0 {
                    if blanker && p > 16.0 * self.mean {
                        *v = C::default();
                        self.blanked += 1;
                    }
                    self.block_sum += p.min(16.0 * self.mean) as f64;
                } else {
                    self.block_sum += p as f64;
                }
                self.block_n += 1;
                if self.block_n == GUARD_BLOCK {
                    let m = (self.block_sum / GUARD_BLOCK as f64) as f32;
                    self.mean = if self.mean > 0.0 {
                        0.95 * self.mean + 0.05 * m
                    } else {
                        m
                    };
                    self.block_sum = 0.0;
                    self.block_n = 0;
                }
                self.since += 1;
                if self.since == self.report {
                    self.since = 0;
                    let lvl = if self.level_n > 0 {
                        self.level_acc / self.level_n as f64
                    } else {
                        0.0
                    };
                    emit = Some(FrontStatus {
                        bad_samples: self.bad,
                        impulses_blanked: self.blanked,
                        level_dbfs: (10.0 * lvl.max(1e-20).log10()) as f32,
                        centring_hz: 0.0,
                        input_rate: self.input_rate,
                    });
                    self.level_acc = 0.0;
                    self.level_n = 0;
                }
            }
        }
        let t = io.input(0).first() as f64 / self.rate.max(1.0);
        if let Some(s) = emit {
            io.items_out::<FrontStatus>(1)
                .push(Timed { time: t, value: s });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SampleGuardSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SampleGuardSetting::ImpulseBlanker => self.blanker = IMPULSE_BLANKER.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(
            SampleGuardSetting::ImpulseBlanker,
            Value::Bool(self.blanker),
        )])
    }
}
