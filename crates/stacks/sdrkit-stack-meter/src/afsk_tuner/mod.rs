//! AFSK tone tuner (`afsk_tuner`): for packet bursts of Bell 202 AFSK on FM (mark 1200 Hz, space 2200 Hz; APRS and AX.25,
//! whose bit stuffing keeps both tones in use), per burst the mark and space tones, the twist (space level over mark level),
//! the peak deviation, the length and the gap since the burst before. The tones and their levels are read from the
//! instantaneous frequency and envelope of the burst audio away from the changes of tone, so the share of each tone does
//! not enter the twist. It measures and decodes no packet. The frames, thresholds and rules are confirmed by no public
//! document.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{analytic, band_gain, band_pass, index_percentile, tx_stack, Ctx, TxTest};
use crate::typed::{
    AfskTunerBurstEvent, AfskTunerBurstRow, AfskTunerFields, AFSK_TUNER_BURST, AFSK_TUNER_BURSTS,
};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "afsk_tuner";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 24e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 24e3;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.2;
/// The samples of a frame.
pub const FRAME: usize = 512;
/// The band of the frame power, Hz.
pub const FRAME_BAND: (f64, f64) = (1000.0, 2500.0);
/// The frame powers the floor reads, s.
pub const FLOOR_WINDOW_S: f64 = 5.0;
/// The longest burst, s.
pub const MAX_BURST_S: f64 = 6.0;
/// The largest analysis length, samples.
pub const MAX_ANALYSIS: usize = 1 << 15;
/// The width of the moving sum of the fallback tone, Hz.
pub const FALLBACK_HZ: f64 = 120.0;
/// The stretch at each end of a burst left out of the tone readings, where the band filter wraps around, s.
pub const EDGE_S: f64 = 0.005;
/// The raised-cosine skirts of the band of the tone readings, outside the tones, Hz: short in time, so that a tone mixes
/// little into the bits of the other.
pub const TONE_SKIRT_HZ: f64 = 500.0;
/// The lag of the instantaneous frequency, read about the middle of the two tones; about a third of a bit of Bell 202, s.
pub const LAG_S: f64 = 0.000_25;
/// Samples this close to a change of tone are left out of the tone readings, about half a bit of Bell 202, s.
pub const MARGIN_S: f64 = 0.000_4;
/// The width of the histogram window and of the mean shift that find a tone, Hz.
pub const TONE_KERNEL_HZ: f64 = 40.0;
/// The bin of that histogram, Hz.
pub const TONE_BIN_HZ: f64 = 5.0;
/// The rounds of that mean shift.
pub const TONE_ROUNDS: usize = 20;
/// Samples whose frequency is this close to a tone give its level, Hz.
pub const LEVEL_HZ: f64 = 40.0;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures each AFSK packet burst on FM: mark and space tones, twist, peak deviation, length and \
gap; decodes no packet. The tone levels assume that both tones are in use, as AX.25 flags and bit stuffing make them. A TNC \
aimed at a pre-emphasised FM radio wants a few dB of mark-heavy audio (advice confirmed by no public document). The frames \
and thresholds are confirmed by no public document.";

/// The set mark tone.
pub const MARK_HZ: Setting<field::F64> = Setting::new("mark_hz", "Mark tone", "Hz", 1200.0)
    .range(300.0, 3000.0, 0.0)
    .help("1200 Hz in Bell 202.");
/// The set space tone.
pub const SPACE_HZ: Setting<field::F64> = Setting::new("space_hz", "Space tone", "Hz", 2200.0)
    .range(300.0, 3000.0, 0.0)
    .help("2200 Hz in Bell 202.");
/// The burst-on level over the floor.
pub const ON_RATIO: Setting<field::F64> =
    Setting::new("on_ratio", "Burst on over the floor", "", 6.0).range(1.5, 20.0, 0.0);
/// The burst-off level over the floor.
pub const OFF_RATIO: Setting<field::F64> =
    Setting::new("off_ratio", "Burst off over the floor", "", 3.0).range(1.5, 20.0, 0.0);
/// The shortest burst analysed.
pub const MIN_BURST_S: Setting<field::F64> =
    Setting::new("min_burst_s", "Shortest burst analysed", "s", 0.1).range(0.05, 2.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The mark tone.
        MarkHz = MARK_HZ,
        /// The space tone.
        SpaceHz = SPACE_HZ,
        /// The on level.
        OnRatio = ON_RATIO,
        /// The off level.
        OffRatio = OFF_RATIO,
        /// The shortest burst.
        MinBurstS = MIN_BURST_S,
    }
}

sdrkit_core::settings! {
    /// The settings of the `afsk_tuner` stack.
    pub enum AfskTunerSetting {
        /// The mark tone.
        MarkHz = MARK_HZ,
        /// The space tone.
        SpaceHz = SPACE_HZ,
        /// The on level.
        OnRatio = ON_RATIO,
        /// The off level.
        OffRatio = OFF_RATIO,
        /// The shortest burst.
        MinBurstS = MIN_BURST_S,
        /// The deviation of the test signal.
        GenDeviationHz = gen::GEN_DEVIATION_HZ,
        /// Its twist.
        GenTwistDb = gen::GEN_TWIST_DB,
        /// Its bit rate.
        GenBaud = gen::GEN_BAUD,
        /// The burst period.
        GenPeriodS = gen::GEN_PERIOD_S,
        /// The burst length.
        GenBurstS = gen::GEN_BURST_S,
        /// The first burst.
        GenFirstS = gen::GEN_FIRST_S,
    }
}

/// A burst in progress.
#[derive(Debug, Default)]
struct Burst {
    start: f64,
    audio: Vec<f64>,
    off_run: u32,
}

/// The measurement of `afsk_tuner`.
#[derive(Default)]
pub struct AfskTuner {
    front: Option<Front>,
    disc: Option<Discriminator>,
    fft: Option<Fft>,
    hann: Vec<f32>,
    buf: Vec<Complex32>,
    audio: Vec<f32>,
    frame: Vec<f64>,
    prev_frame: Vec<f64>,
    rate: f64,
    mark: f64,
    space: f64,
    on: f64,
    off: f64,
    min_burst: f64,
    t: f64,
    powers: VecDeque<(f64, f64)>,
    on_run: u32,
    burst: Option<Burst>,
    last_end: Option<f64>,
    bursts: i64,
    dev_range: Option<(f64, f64)>,
    last: Option<AfskTunerBurstRow>,
    log: Log<AfskTunerBurstRow>,
}

impl AfskTuner {
    fn frame_power(&mut self) -> f64 {
        let Some(fft) = self.fft.as_mut() else {
            return 0.0;
        };
        let mean = self.frame.iter().sum::<f64>() / FRAME as f64;
        for ((b, &v), &w) in self.buf.iter_mut().zip(&self.frame).zip(&self.hann) {
            *b = Complex32::new(((v - mean) * f64::from(w)) as f32, 0.0);
        }
        fft.process(&mut self.buf);
        let b = self.rate / FRAME as f64;
        let (a, z) = (
            (FRAME_BAND.0 / b).floor() as usize,
            (FRAME_BAND.1 / b).floor() as usize,
        );
        self.buf[a..=z]
            .iter()
            .map(|x| f64::from(x.norm_sqr()))
            .sum()
    }

    /// The tone of `band` from the instantaneous frequencies of `inst` (frequency, envelope) at `rate`: the fullest
    /// `TONE_KERNEL_HZ` window of a histogram of `TONE_BIN_HZ` bins, refined by a mean shift over a window as wide. `None`
    /// with fewer than 8 periods of the band's centre in it.
    fn tone(inst: &[(f64, f64)], band: (f64, f64), rate: f64) -> Option<f64> {
        let f: Vec<f64> = inst
            .iter()
            .map(|p| p.0)
            .filter(|&f| f >= band.0 && f <= band.1)
            .collect();
        if (f.len() as f64) < 8.0 * rate / (0.5 * (band.0 + band.1)) {
            return None;
        }
        let bins = ((band.1 - band.0) / TONE_BIN_HZ).ceil() as usize + 1;
        let mut hist = vec![0usize; bins];
        for &v in &f {
            hist[((v - band.0) / TONE_BIN_HZ) as usize] += 1;
        }
        let w = ((TONE_KERNEL_HZ / TONE_BIN_HZ) as usize).max(1);
        let top = (0..bins.saturating_sub(w - 1))
            .max_by_key(|&i| hist[i..i + w].iter().sum::<usize>())?;
        let mut c = band.0 + (top as f64 + 0.5 * w as f64) * TONE_BIN_HZ;
        for _ in 0..TONE_ROUNDS {
            let (s, n) = f
                .iter()
                .filter(|&&v| (v - c).abs() <= 0.5 * TONE_KERNEL_HZ)
                .fold((0.0, 0usize), |(s, n), &v| (s + v, n + 1));
            if n == 0 {
                break;
            }
            c = s / n as f64;
        }
        Some(c)
    }

    /// The samples of `inst` (frequency, envelope) that lie `margin` samples or more inside a run of samples on one side
    /// of `fc` Hz: away from the changes of tone, where the band filter mixes the two tones.
    fn run_middles(inst: &[(f64, f64)], fc: f64, margin: usize) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        let mut a = 0;
        for i in 1..=inst.len() {
            if i == inst.len() || (inst[i].0 > fc) != (inst[a].0 > fc) {
                if i - a > 2 * margin {
                    out.extend_from_slice(&inst[a + margin..i - margin]);
                }
                a = i;
            }
        }
        out
    }

    /// The level of the tone at `f` Hz: the median envelope of the samples of `inst` whose frequency is within `LEVEL_HZ`
    /// of it. `None` with fewer than 16 of them.
    fn level(inst: &[(f64, f64)], f: f64) -> Option<f64> {
        let mut a: Vec<f64> = inst
            .iter()
            .filter(|p| (p.0 - f).abs() <= LEVEL_HZ)
            .map(|p| p.1)
            .collect();
        if a.len() < 16 {
            return None;
        }
        index_percentile(&mut a, 0.5)
    }

    /// The top of the moving sum of `spec` (bin width `b`) inside `band`.
    fn tone_of_spectrum(spec: &[f64], b: f64, band: (f64, f64)) -> Option<f64> {
        let w = ((FALLBACK_HZ / b).round() as usize).max(1);
        let (a, z) = (
            (band.0 / b).ceil() as usize,
            ((band.1 / b).floor() as usize).min(spec.len() - 1),
        );
        (a..=z.saturating_sub(w - 1))
            .max_by(|&i, &j| {
                spec[i..i + w]
                    .iter()
                    .sum::<f64>()
                    .total_cmp(&spec[j..j + w].iter().sum::<f64>())
            })
            .map(|i| (i as f64 + (w as f64 - 1.0) / 2.0) * b)
    }

    fn analyse(&mut self, burst: Burst, c: &mut Ctx) {
        let r = self.rate;
        let len = burst.audio.len() as f64 / r - 3.0 * FRAME as f64 / r;
        let end = burst.start + len;
        if len < self.min_burst {
            return;
        }
        let want = burst.audio.len().min(MAX_ANALYSIS);
        let n = 1usize << (usize::BITS - 1 - want.leading_zeros());
        let x: Vec<f64> = {
            let m = burst.audio[..n].iter().sum::<f64>() / n as f64;
            burst.audio[..n].iter().map(|v| v - m).collect()
        };
        let mut buf: Vec<Complex32> = x
            .iter()
            .zip(Window::Hann.coefficients(n))
            .map(|(&v, w)| Complex32::new((v * f64::from(w)) as f32, 0.0))
            .collect();
        Fft::forward(n).process(&mut buf);
        let b = r / n as f64;
        let spec: Vec<f64> = buf[..n / 2]
            .iter()
            .map(|z| f64::from(z.norm_sqr()))
            .collect();
        let band_sum = |lo: f64, hi: f64| -> f64 {
            let (a, z) = (
                (lo / b).ceil() as usize,
                ((hi / b).floor() as usize).min(spec.len() - 1),
            );
            spec[a..=z].iter().sum()
        };
        let mark_band = (self.mark - 300.0, self.mark + 400.0);
        let space_band = (self.space - 400.0, self.space + 400.0);
        let (lo, hi) = (self.mark - 400.0, self.space + 500.0);
        let bl = band_pass(&x, r, |f| {
            if f.abs() >= lo && f.abs() <= hi {
                1.0
            } else {
                0.0
            }
        });
        let mut abs: Vec<f64> = bl.iter().map(|v| v.abs()).collect();
        let dev = index_percentile(&mut abs, 0.995).unwrap_or(0.0);
        let z = analytic(&x, r, |f| {
            band_gain(
                f,
                self.mark - TONE_SKIRT_HZ,
                self.space + TONE_SKIRT_HZ,
                TONE_SKIRT_HZ,
            )
        });
        let edge = ((EDGE_S * r) as usize).min(z.len() / 4).max(1);
        let lag = ((LAG_S * r).round() as usize).max(1);
        let fc = 0.5 * (self.mark + self.space);
        let turn = Complex32::from_polar(1.0, (-TAU * fc * lag as f64 / r) as f32);
        let inst: Vec<(f64, f64)> = z[edge..z.len() - edge]
            .windows(lag + 1)
            .map(|w| {
                let d = w[lag] * w[0].conj() * turn;
                (
                    fc + f64::from(d.im).atan2(f64::from(d.re)) * r / (TAU * lag as f64),
                    f64::from(w[lag / 2].norm()),
                )
            })
            .collect();
        let inst = Self::run_middles(&inst, fc, ((MARGIN_S * r).round() as usize).max(1));
        let (Some(mark), Some(space)) = (
            Self::tone(&inst, mark_band, r).or_else(|| Self::tone_of_spectrum(&spec, b, mark_band)),
            Self::tone(&inst, space_band, r)
                .or_else(|| Self::tone_of_spectrum(&spec, b, space_band)),
        ) else {
            return;
        };
        let twist = match (Self::level(&inst, mark), Self::level(&inst, space)) {
            (Some(m), Some(s)) => 20.0 * (s / m.max(1e-30)).log10(),
            _ => {
                10.0 * (band_sum(space_band.0, space_band.1)
                    / band_sum(mark_band.0, mark_band.1).max(1e-30))
                .log10()
            }
        };
        let row = AfskTunerBurstRow {
            time_s: Seconds(burst.start),
            length_s: Seconds(len),
            mark_hz: Hertz(mark),
            space_hz: Hertz(space),
            twist_db: twist,
            peak_deviation_khz: dev / 1000.0,
            gap_s: self.last_end.map(|e| Seconds(burst.start - e)),
        };
        self.last_end = Some(end);
        self.bursts += 1;
        let d = row.peak_deviation_khz;
        self.dev_range = Some(self.dev_range.map_or((d, d), |(a, z)| (a.min(d), z.max(d))));
        c.tally.good(self.t);
        c.events.push(AFSK_TUNER_BURST.event(
            burst.start,
            c.dial(),
            &AfskTunerBurstEvent {
                time_s: row.time_s,
                length_s: row.length_s,
                mark_hz: row.mark_hz,
                space_hz: row.space_hz,
                twist_db: row.twist_db,
                peak_deviation_khz: row.peak_deviation_khz,
                gap_s: row.gap_s,
            },
        ));
        self.last = Some(row.clone());
        self.log.push(row);
    }

    fn frame_done(&mut self, c: &mut Ctx) {
        let e = self.frame_power();
        let frame_s = FRAME as f64 / self.rate;
        self.t += frame_s;
        let t = self.t;
        if e > 0.0 {
            self.powers.push_back((t, e));
        }
        while self
            .powers
            .front()
            .is_some_and(|&(a, _)| t - a > FLOOR_WINDOW_S)
        {
            self.powers.pop_front();
        }
        let mut v: Vec<f64> = self.powers.iter().map(|p| p.1).collect();
        let floor = index_percentile(&mut v, 0.2).unwrap_or(0.0);
        let on = e > self.on * floor;
        let off = e < self.off * floor;
        match self.burst.as_mut() {
            None => {
                self.on_run = if on { self.on_run + 1 } else { 0 };
                if self.on_run >= 2 {
                    let mut audio = std::mem::take(&mut self.prev_frame);
                    audio.extend_from_slice(&self.frame);
                    self.burst = Some(Burst {
                        start: t - 2.0 * frame_s,
                        audio,
                        off_run: 0,
                    });
                    self.on_run = 0;
                    c.tally.energy(t);
                }
            }
            Some(b) => {
                b.audio.extend_from_slice(&self.frame);
                b.off_run = if off { b.off_run + 1 } else { 0 };
                c.tally.energy(t);
                if b.off_run >= 3 || b.audio.len() as f64 / self.rate >= MAX_BURST_S {
                    let b = self.burst.take().unwrap_or_default();
                    self.analyse(b, c);
                }
            }
        }
        self.prev_frame.clear();
        self.prev_frame.extend_from_slice(&self.frame);
        self.frame.clear();
    }
}

impl TxTest for AfskTuner {
    const ID: &'static str = ID;
    const NAME: &'static str = "afsk_tuner.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let d = (rate_hz / WORK_HZ).floor().max(1.0);
        let front = Front::new(rate_hz, WORK_HZ, 8000f64.min(0.45 * rate_hz / d), SKIP_S);
        self.rate = front.rate();
        self.disc = Discriminator::new(self.rate, 1.0).ok();
        self.front = Some(front);
        self.fft = Some(Fft::forward(FRAME));
        self.hann = Window::Hann.coefficients(FRAME);
        self.buf = vec![Complex32::default(); FRAME];
        self.mark = v.get(&MARK_HZ);
        self.space = v.get(&SPACE_HZ);
        self.on = v.get(&ON_RATIO);
        self.off = v.get(&OFF_RATIO);
        self.min_burst = v.get(&MIN_BURST_S);
        self.frame.clear();
        self.prev_frame.clear();
        self.powers.clear();
        self.on_run = 0;
        self.burst = None;
        self.t = self.t.max(SKIP_S);
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let (Some(front), Some(disc)) = (self.front.as_mut(), self.disc.as_mut()) else {
            return;
        };
        self.audio.clear();
        disc.process(front.process(x), &mut self.audio);
        let audio = std::mem::take(&mut self.audio);
        for &a in &audio {
            self.frame.push(f64::from(a));
            if self.frame.len() == FRAME {
                self.frame_done(c);
            }
        }
        self.audio = audio;
    }
    fn clear(&mut self) {
        self.bursts = 0;
        self.dev_range = None;
        self.last = None;
        self.last_end = None;
        self.burst = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&AFSK_TUNER_BURSTS, p);
        let l = self.last.as_ref();
        p.put(&AfskTunerFields {
            mark_hz: l.map(|r| r.mark_hz),
            space_hz: l.map(|r| r.space_hz),
            mark_error_hz: l.map(|r| Hertz(r.mark_hz.0 - self.mark)),
            space_error_hz: l.map(|r| Hertz(r.space_hz.0 - self.space)),
            twist_db: l.map(|r| r.twist_db),
            peak_deviation_khz: l.map(|r| r.peak_deviation_khz),
            length_s: l.map(|r| r.length_s),
            gap_s: l.and_then(|r| r.gap_s),
            bursts: self.bursts,
            deviation_min_khz: self.dev_range.map(|d| d.0),
            deviation_max_khz: self.dev_range.map(|d| d.1),
        });
        match l {
            Some(r) => format!(
                "{} bursts; last: mark {:.0} Hz, space {:.0} Hz, twist {:+.1} dB, {:.2} kHz",
                self.bursts, r.mark_hz.0, r.space_hz.0, r.twist_db, r.peak_deviation_khz
            ),
            None if self.burst.is_some() => "burst in progress".to_string(),
            None => "waiting for a burst".to_string(),
        }
    }
}

const KEYS: [&str; 5] = [
    "mark_hz",
    "space_hz",
    "on_ratio",
    "off_ratio",
    "min_burst_s",
];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("bursts of Bell 202 AFSK every 2 s", o, 0.0);
    c.seconds = o.seconds.max(8.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = crate::instrument::number(r, AfskTunerFields::MARK_HZ.key);
        vec![CheckLine::new(
            AfskTunerFields::MARK_HZ.key,
            (f - 1200.0).abs() < 25.0,
            format!("{f:.0} Hz, 1200 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: AfskTuner,
    settings: AfskTunerSetting,
    keys: KEYS,
    name: "AFSK tone tuner",
    blurb: "Per packet burst of Bell 202 AFSK on FM: mark and space tones, twist, peak deviation, length and gap.",
    tuning: (26e6, 6e9, 48e3, 16e3, true),
    rate_text: "The AFSK tuner needs at least 24 ksps",
    generator: |cfg| gen::AfskGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
