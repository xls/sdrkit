//! The link finder of the STL stack: the averaged spectrum of the capture around the channel centre (floor, peak, occupied
//! region, its centroid, level and flatness), the automatic channel width and the link centre, and the component
//! `stl.link` that steers the FM channel with them.
//!
//! | Step | Rule |
//! |---|---|
//! | Spectrum | a 1024-point Hann power spectrum of every fourth frame of 1024 input samples, averaged over each report of 0.5 s; at least 8 frames |
//! | Floor | the median of the bins from 0.75 × max(100 kHz, the channel width) to 0.4 × the input rate from the centre; at least 24 bins, else no comparison |
//! | Peak | the strongest bin of the spectrum smoothed over 9 bins, within 0.4 × the input rate of the centre |
//! | Region | the run of smoothed bins over 2 × the floor around the peak, at least 8 bins; its width, its centroid weighted by the power over the floor, and over its inner 80 % the mean over the floor and the 10th percentile against the mean (flatness) |
//! | Width | Carson's rule 2 × (max(peak deviation hold, 20 kHz) + highest multiplex frequency), or the occupied width if more, × 1.35 + 10 kHz, within 100 kHz and the widest channel; it widens at once and narrows after 3 reports below 0.8 × the width the first time, 60 later |
//! | Centre | the centroid of the region, when it is more than 4 kHz from the centre |

use super::settings::{AUTO_WIDTH, MAX_WIDTH_HZ, MIN_WIDTH_HZ, WIDTH};
use super::sca::ScaReport;
use crate::fmdemod::DemodReport;
use crate::graph::ChannelSteer;
use sdrkit_algo_dsp::spectrum::{SpectrumAnalyzer, SpectrumConfig};
use sdrkit_core::{ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};

/// The FFT size of the link spectrum.
pub const FFT: usize = 1024;
/// Input samples from the start of one spectrum frame to the next.
pub const HOP: usize = 4 * FFT;
/// Ticks of the graph clock in one report.
pub const REPORT_TICKS: u32 = 5;
/// The width the automatic width starts at, Hz.
pub const START_WIDTH_HZ: f64 = 400e3;
/// The highest multiplex frequency without an SCA subcarrier (the RDS band), Hz.
pub const FMAX_HZ: f64 = 60e3;
/// The highest multiplex frequency with an SCA subcarrier, Hz.
pub const FMAX_SCA_HZ: f64 = 99e3;

/// The occupied region of the spectrum, measured from the channel centre.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Region {
    /// The occupied width, Hz.
    pub width_hz: f64,
    /// The centroid, Hz from the channel centre.
    pub centroid_hz: f64,
    /// The mean of the inner 80 % over the floor, dB.
    pub over_floor_db: f64,
    /// The 10th percentile of the inner 80 % against its mean, dB (0 is flat).
    pub flatness_db: f64,
}

/// What the spectrum of one report shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Findings {
    /// The floor comparison was possible.
    pub floor_ok: bool,
    /// The strongest smoothed bin over the floor, dB.
    pub peak_db: Option<f64>,
    /// The occupied region, when one stands out.
    pub region: Option<Region>,
}

/// The scratch space of [`analyse`], kept between reports so nothing is allocated.
#[derive(Clone, Debug, Default)]
pub struct Scratch {
    rel: Vec<f32>,
    smooth: Vec<f32>,
    pick: Vec<f32>,
}

/// The value at fraction `q` (0 to 1) of `v` in ascending order; `v` is reordered.
fn quantile(v: &mut [f32], q: f64) -> f32 {
    let k = ((v.len() - 1) as f64 * q).round() as usize;
    *v.select_nth_unstable_by(k, f32::total_cmp).1
}

/// The findings of the averaged power spectrum `power` (bins in display order, bin `len / 2` at the input centre, `bin_hz`
/// apart) for a channel centred `centre_hz` from the input centre and `width_hz` wide, at input rate `rate`.
pub fn analyse(
    power: &[f32],
    bin_hz: f64,
    centre_hz: f64,
    width_hz: f64,
    rate: f64,
    s: &mut Scratch,
) -> Findings {
    let n = power.len();
    let half = n as i64 / 2;
    // The spectrum turned so that bin `n / 2` is the channel centre.
    let turn = (centre_hz / bin_hz).round() as i64;
    s.rel.clear();
    s.rel.extend((0..n as i64).map(|j| power[(j + turn).rem_euclid(n as i64) as usize]));
    let dist = |j: usize| (j as i64 - half).unsigned_abs() as f64 * bin_hz;
    let (lo, hi) = (0.75 * width_hz.max(100e3), 0.4 * rate);
    s.pick.clear();
    s.pick
        .extend((0..n).filter(|&j| (lo..=hi).contains(&dist(j))).map(|j| s.rel[j]));
    if s.pick.len() < 24 {
        return Findings::default();
    }
    let floor = quantile(&mut s.pick, 0.5).max(f32::MIN_POSITIVE) as f64;
    s.smooth.clear();
    s.smooth.extend((0..n).map(|j| {
        let (a, b) = (j.saturating_sub(4), (j + 5).min(n));
        s.rel[a..b].iter().sum::<f32>() / (b - a) as f32
    }));
    let Some((peak_j, peak)) = (0..n)
        .filter(|&j| dist(j) <= hi)
        .map(|j| (j, s.smooth[j] as f64))
        .max_by(|a, b| a.1.total_cmp(&b.1))
    else {
        return Findings::default();
    };
    let db = |x: f64| 10.0 * (x / floor).log10();
    let mut f = Findings {
        floor_ok: true,
        peak_db: Some(db(peak)),
        region: None,
    };
    let over = |j: usize| s.smooth[j] as f64 > 2.0 * floor && dist(j) <= hi;
    if !over(peak_j) {
        return f;
    }
    let (mut a, mut b) = (peak_j, peak_j);
    while a > 0 && over(a - 1) {
        a -= 1;
    }
    while b + 1 < n && over(b + 1) {
        b += 1;
    }
    let len = b - a + 1;
    if len < 8 {
        return f;
    }
    let (mut wsum, mut fsum) = (0.0, 0.0);
    for j in a..=b {
        let w = s.smooth[j] as f64 - floor;
        wsum += w;
        fsum += w * (j as i64 - half) as f64 * bin_hz;
    }
    let cut = len / 10;
    let inner = &s.smooth[a + cut..=b - cut];
    let mean = inner.iter().map(|&x| x as f64).sum::<f64>() / inner.len() as f64;
    s.pick.clear();
    s.pick.extend_from_slice(inner);
    let p10 = quantile(&mut s.pick, 0.1) as f64;
    f.region = Some(Region {
        width_hz: len as f64 * bin_hz,
        centroid_hz: if wsum > 0.0 { fsum / wsum } else { 0.0 },
        over_floor_db: db(mean),
        flatness_db: 10.0 * (p10.max(f64::MIN_POSITIVE) / mean).log10(),
    });
    f
}

/// The width that a link needs: Carson's rule on the deviation hold and the highest multiplex frequency, or the occupied
/// width if that is more, with 35 % and 10 kHz of margin, within 100 kHz and `max_hz`.
pub fn needed_width(hold_hz: f64, sca: bool, occupied_hz: Option<f64>, max_hz: f64) -> f64 {
    let fmax = if sca { FMAX_SCA_HZ } else { FMAX_HZ };
    let carson = 2.0 * (hold_hz.max(20e3) + fmax);
    let need = occupied_hz.map_or(carson, |w| carson.max(w));
    (need * 1.35 + 10e3).clamp(100e3, max_hz.max(100e3))
}

/// The automatic channel width: the peak deviation hold and the rule that widens at once and narrows slowly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoWidth {
    /// The width in use, Hz.
    pub width_hz: f64,
    /// The peak deviation hold, Hz.
    pub hold_hz: f64,
    max_hz: f64,
    narrowed: bool,
    below: u32,
}

impl AutoWidth {
    /// Reports below 0.8 × the width before the first narrowing.
    pub const FIRST_NARROW: u32 = 3;
    /// Reports below 0.8 × the width before a later narrowing.
    pub const LATER_NARROW: u32 = 60;

    /// The rule at its start width, for channels at most `max_hz` wide.
    pub fn new(max_hz: f64) -> Self {
        AutoWidth {
            width_hz: START_WIDTH_HZ.min(max_hz),
            hold_hz: 0.0,
            max_hz,
            narrowed: false,
            below: 0,
        }
    }

    /// One report: the carrier, its peak deviation (`None` while the channel restarted, which keeps the hold), an SCA
    /// subcarrier, the occupied width. True if the width changed.
    pub fn step(
        &mut self,
        carrier: bool,
        peak_hz: Option<f64>,
        sca: bool,
        occupied_hz: Option<f64>,
    ) -> bool {
        match peak_hz {
            Some(p) if carrier => self.hold_hz = (0.9 * self.hold_hz).max(p),
            Some(_) => self.hold_hz *= 0.9,
            None => {}
        }
        let need = needed_width(self.hold_hz, sca, occupied_hz, self.max_hz);
        if need > 1.05 * self.width_hz {
            self.width_hz = need;
            self.below = 0;
            return true;
        }
        if need >= 0.8 * self.width_hz {
            self.below = 0;
            return false;
        }
        self.below += 1;
        let wait = if self.narrowed {
            Self::LATER_NARROW
        } else {
            Self::FIRST_NARROW
        };
        if self.below < wait {
            return false;
        }
        self.width_hz = need;
        self.narrowed = true;
        self.below = 0;
        true
    }
}

/// The link finder's report, every 0.5 s.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinkReport {
    /// What the spectrum shows.
    pub findings: Findings,
    /// The FM demodulator saw a carrier at the last tick.
    pub carrier: bool,
    /// The peak deviation over the report, Hz. `None` for the report after the channel restarted (at the start and at a
    /// new width), whose peak is the restart and not the link.
    pub peak_hz: Option<f64>,
    /// The channel width in use, Hz.
    pub width_hz: f64,
    /// The width follows the link.
    pub auto: bool,
    /// The channel centre from the tuned frequency, Hz.
    pub shift_hz: f64,
}
impl Item for LinkReport {
    const KIND: &'static str = "stl.link_report";
}

sdrkit_core::settings! {
    /// The settings of [`LinkComponent`].
    pub enum LinkSetting {
        /// The channel width follows the link.
        AutoWidth = AUTO_WIDTH,
        /// The channel width when it does not follow the link.
        Width = WIDTH,
    }
}

/// The link finder (`stl.link`): the spectrum of the input, the automatic width and the link centre. It steers the FM
/// channel with a [`ChannelSteer`] at the exact input sample of each change, and reports a [`LinkReport`] every
/// [`REPORT_TICKS`] ticks. It reads the FM demodulator's carrier and peak deviation and the SCA meter fed back from the tick
/// before.
pub struct LinkComponent {
    auto: bool,
    manual_hz: f64,
    rate: f64,
    signal_hz: f64,
    shift_hz: f64,
    rule: AutoWidth,
    spectrum: Option<SpectrumAnalyzer>,
    scratch: Scratch,
    ticks: u32,
    carrier: bool,
    peak_hz: f64,
    sca: bool,
    announce: bool,
    restarted: bool,
}

impl Default for LinkComponent {
    fn default() -> Self {
        LinkComponent {
            auto: AUTO_WIDTH.default,
            manual_hz: WIDTH.default,
            rate: 0.0,
            signal_hz: 0.0,
            shift_hz: 0.0,
            rule: AutoWidth::new(MAX_WIDTH_HZ),
            spectrum: None,
            scratch: Scratch::default(),
            ticks: 0,
            carrier: false,
            peak_hz: 0.0,
            sca: false,
            announce: false,
            restarted: false,
        }
    }
}

impl LinkComponent {
    /// The widest channel at this rate.
    fn max_width(&self) -> f64 {
        MAX_WIDTH_HZ.min(0.9 * self.rate)
    }
    /// The width in use.
    fn width(&self) -> f64 {
        if self.auto {
            self.rule.width_hz
        } else {
            self.manual_hz.clamp(MIN_WIDTH_HZ, self.max_width())
        }
    }
    fn steer(&self, io: &mut Io, at: u64) {
        let value = ChannelSteer {
            at,
            shift_hz: self.shift_hz,
            width_hz: self.width(),
        };
        let time = io.input_format(0).time_of(at);
        io.items_out::<ChannelSteer>(0).push(Timed { time, value });
    }
}

impl Component for LinkComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "stl.link",
            vec![
                Port::iq("in"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<ScaReport>("sca"),
            ],
            vec![
                Port::typed::<ChannelSteer>("steer").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<LinkReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(LinkSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        (self.rate, self.signal_hz, self.shift_hz) = (f.rate, f.signal_offset_hz(), 0.0);
        self.rule = AutoWidth::new(self.max_width());
        self.spectrum = (f.rate > 0.0)
            .then(|| {
                let cfg = SpectrumConfig {
                    frames_per_second: f.rate / HOP as f64,
                    average: usize::MAX,
                    ..SpectrumConfig::spec(FFT, f.rate)
                };
                SpectrumAnalyzer::new(cfg).ok()
            })
            .flatten();
        (self.ticks, self.carrier, self.peak_hz, self.sca) = (0, false, 0.0, false);
        (self.announce, self.restarted) = (true, true);
        Ok(vec![
            Format::new(Kind::typed::<ChannelSteer>(), 0.0),
            Format::new(Kind::typed::<LinkReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        if std::mem::take(&mut self.announce) {
            self.steer(io, first);
        }
        for d in io.items::<DemodReport>(1) {
            self.carrier = d.value.carrier;
            self.peak_hz = self.peak_hz.max(d.value.dev_khz as f64 * 1e3);
        }
        if let Some(s) = io.items::<ScaReport>(2).last() {
            self.sca = s.value.present();
        }
        let x = io.iq(0);
        let end = first + x.len() as u64;
        if let Some(a) = &mut self.spectrum {
            a.push(x);
        }
        if !io.tick() {
            return;
        }
        self.ticks += 1;
        if !self.ticks.is_multiple_of(REPORT_TICKS) {
            return;
        }
        let width = self.width();
        let findings = match &mut self.spectrum {
            Some(a) if a.frames() >= 8 => {
                let bin_hz = self.rate / FFT as f64;
                let f = analyse(
                    a.power(),
                    bin_hz,
                    self.signal_hz + self.shift_hz,
                    width,
                    self.rate,
                    &mut self.scratch,
                );
                a.reset();
                f
            }
            _ => Findings::default(),
        };
        let occupied = findings.region.map(|r| r.width_hz);
        let peak = (!std::mem::take(&mut self.restarted)).then_some(self.peak_hz);
        let resized = self.rule.step(self.carrier, peak, self.sca, occupied) && self.auto;
        self.restarted = resized;
        let mut changed = resized;
        if let Some(r) = findings.region {
            if r.centroid_hz.abs() > 4e3 {
                let limit = 0.4 * self.rate;
                self.shift_hz = (self.shift_hz + r.centroid_hz).clamp(-limit, limit);
                changed = true;
            }
        }
        if changed {
            self.steer(io, end);
        }
        let report = LinkReport {
            findings,
            carrier: self.carrier,
            peak_hz: peak,
            width_hz: self.width(),
            auto: self.auto,
            shift_hz: self.shift_hz,
        };
        io.items_out::<LinkReport>(1).push(Timed {
            time: 0.0,
            value: report,
        });
        self.peak_hz = 0.0;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match LinkSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            LinkSetting::AutoWidth => self.auto = AUTO_WIDTH.read(id, v)?,
            LinkSetting::Width => self.manual_hz = WIDTH.read(id, v)?,
        }
        self.announce = self.rate > 0.0;
        self.restarted |= self.announce;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (LinkSetting::AutoWidth, Value::Bool(self.auto)),
            (LinkSetting::Width, Value::Hz(self.manual_hz)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_worked_widths() {
        // Composite, 36.3 kHz peak, RDS, no SCA: 270.0 kHz, after three reports.
        let mut w = AutoWidth::new(400e3);
        assert!(!w.step(true, Some(36.3e3), false, None));
        assert!(!w.step(true, Some(36.3e3), false, None));
        assert!(w.step(true, Some(36.3e3), false, None));
        assert!((w.width_hz - 270.01e3).abs() < 10.0, "{}", w.width_hz);
        // Composite, 45.2 kHz peak, with an SCA subcarrier: 399.3 kHz, so the width stays at 400 kHz.
        let need = needed_width(45.2e3, true, None, 400e3);
        assert!((need - 399.34e3).abs() < 10.0, "{need}");
        let mut w = AutoWidth::new(400e3);
        for _ in 0..10 {
            assert!(!w.step(true, Some(45.2e3), true, None));
        }
        assert_eq!(w.width_hz, 400e3);
        // Mono, 40 kHz peak: 280 kHz.
        assert!((needed_width(40e3, false, None, 400e3) - 280e3).abs() < 1.0);
        // A wider occupied region wins; the result stays within 100 kHz and the widest channel.
        assert!((needed_width(10e3, false, Some(238e3), 400e3) - 331.3e3).abs() < 1.0);
        assert_eq!(needed_width(0.0, false, Some(1e6), 400e3), 400e3);
        // After the first narrowing, a later one waits 60 reports; a widening comes at once.
        let mut w = AutoWidth::new(400e3);
        for _ in 0..3 {
            w.step(true, Some(36.3e3), false, None);
        }
        assert!(w.step(true, Some(75e3), false, None));
        assert!(w.width_hz > 370e3);
        let (mut first_below, mut k) = (None, 0);
        while !w.step(true, Some(10e3), false, None) {
            k += 1;
            if w.below == 1 {
                first_below.get_or_insert(k);
            }
        }
        assert_eq!(k + 1 - first_below.unwrap(), AutoWidth::LATER_NARROW as usize - 1);
    }

    /// A flat power spectrum of `n` bins at `floor`, with `width_bins` bins at `level` times the floor centred at bin
    /// `n / 2 + at`.
    fn spectrum(n: usize, floor: f32, level: f32, width_bins: usize, at: i64) -> Vec<f32> {
        let mut p = vec![floor; n];
        let start = n as i64 / 2 + at - width_bins as i64 / 2;
        for j in start..start + width_bins as i64 {
            p[j as usize] = floor * level;
        }
        p
    }

    #[test]
    fn a_flat_band_reads_its_width_level_centroid_and_flatness() {
        let (rate, n) = (1e6, 1024);
        let bin = rate / n as f64;
        let mut s = Scratch::default();
        // 238 kHz wide, 20 dB over the floor, 25 kHz above the centre.
        let p = spectrum(n, 1e-6, 100.0, (238e3 / bin) as usize, (25e3 / bin) as i64);
        let f = analyse(&p, bin, 0.0, 400e3, rate, &mut s);
        let r = f.region.expect("a region");
        assert!(f.floor_ok);
        // The smoothing over 9 bins widens the region by up to 4 bins on each side.
        let sent = (238e3 / bin).floor() * bin;
        assert!(r.width_hz >= sent && r.width_hz <= sent + 8.0 * bin, "{}", r.width_hz);
        assert!((r.over_floor_db - 20.0).abs() < 0.5, "{}", r.over_floor_db);
        assert!(r.flatness_db > -1.0, "{}", r.flatness_db);
        assert!((r.centroid_hz - 25e3).abs() < bin, "{}", r.centroid_hz);
        // Measured from a centre at the region, the centroid is near 0.
        let f = analyse(&p, bin, 25.0 * bin, 400e3, rate, &mut s);
        assert!(f.region.unwrap().centroid_hz.abs() < bin);
        // Noise alone has no region; too low a rate for the channel has no comparison.
        let flat = vec![1e-6f32; n];
        let f = analyse(&flat, bin, 0.0, 400e3, rate, &mut s);
        assert!(f.floor_ok && f.region.is_none());
        let f = analyse(&flat, 500e3 / n as f64, 0.0, 400e3, 500e3, &mut s);
        assert!(!f.floor_ok && f.peak_db.is_none());
    }
}
