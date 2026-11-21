//! The level meter: the front end (a channel of 24 ksps at 250 ksps or more, else a mix of the input by the negative offset),
//! power spectra of consecutive Hann windows summed over each sample interval, and the level of one transmitter in that sum.

use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The rate of the front end's output, Hz.
pub const WORK_RATE: f64 = 24_000.0;
/// The input rate from which the front end makes a 24 ksps channel, Hz.
pub const FRONT_END_FROM: f64 = 250e3;
/// The highest rate the meter transforms; above it every transmitter is out of span, Hz.
pub const MAX_RATE: f64 = 300e3;
/// The flat width of the front end's channel, Hz (0.8 of its output rate).
pub const CHANNEL_HZ: f64 = 19_200.0;
/// The share of the half rate inside which a transmitter is in span.
pub const SPAN_SHARE: f64 = 0.85;
/// The half width of the signal band, Hz.
pub const SIGNAL_HALF_HZ: f64 = 60.0;
/// The noise bands, Hz from the carrier, on each side.
pub const NOISE_HZ: (f64, f64) = (150.0, 400.0);
/// The level from which a transmitter is present, dB.
pub const PRESENT_DB: f64 = 6.0;
/// The power sum of a Hann window over its length, N × 0.375.
const HANN_POWER: f64 = 0.375;

/// The transform size at `rate`: the next power of two at or above `rate / 3`. `None` above [`MAX_RATE`].
pub fn fft_size(rate: f64) -> Option<usize> {
    if !(rate > 0.0 && rate <= MAX_RATE) {
        return None;
    }
    Some(((rate / 3.0).ceil() as usize).max(2).next_power_of_two())
}

/// Whether a carrier `offset_hz` from the centre of a stream at `rate` is in span: |offset| < 0.85 × rate / 2, and the
/// stream is transformed at all.
pub fn in_span(offset_hz: f64, rate: f64) -> bool {
    fft_size(rate).is_some() && offset_hz.abs() < SPAN_SHARE * rate / 2.0
}

/// The measure of one transmitter in one interval.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level {
    /// The carrier is in span.
    pub in_span: bool,
    /// The level is [`PRESENT_DB`] or more.
    pub present: bool,
    /// The mean power of a signal bin over the mean power of a noise bin, dB.
    pub level_db: Option<f64>,
    /// The power in the signal band relative to full scale (a full-scale tone is 1).
    pub power: Option<f64>,
    /// [`Level::power`] in dB, dBFS.
    pub absolute_dbfs: Option<f64>,
}

/// The power spectra summed over one interval.
#[derive(Clone, Debug, Default)]
pub struct Spectrum {
    /// The rate of the transformed stream, Hz.
    pub rate: f64,
    /// The sum of the power of each bin (bin 0 at DC, natural FFT order).
    pub power: Vec<f64>,
    /// The spectra summed.
    pub count: u32,
}

impl Spectrum {
    fn mean(&self, from: i64, to: i64) -> f64 {
        let n = self.power.len() as i64;
        let mut s = 0.0;
        for i in from..=to {
            s += self.power[i.rem_euclid(n) as usize];
        }
        s / (to - from + 1) as f64
    }

    /// The level of a carrier `offset_hz` from the centre. Out of span, or without a spectrum, only `in_span` is set.
    pub fn level(&self, offset_hz: f64) -> Level {
        let n = self.power.len();
        let mut l = Level {
            in_span: in_span(offset_hz, self.rate) && n > 0,
            ..Level::default()
        };
        if !l.in_span || self.count == 0 {
            return l;
        }
        let bin = self.rate / n as f64;
        let c = (offset_hz / bin).round() as i64;
        let h = ((SIGNAL_HALF_HZ / bin).round() as i64).max(1);
        let (n0, n1) = (
            (NOISE_HZ.0 / bin).round() as i64,
            (NOISE_HZ.1 / bin).round() as i64,
        );
        let signal = self.mean(c - h, c + h);
        let noise = self
            .mean(c + n0, c + n1)
            .min(self.mean(c - n1, c - n0))
            .max(f64::MIN_POSITIVE);
        let level = 10.0 * (signal.max(f64::MIN_POSITIVE) / noise).log10();
        let total = signal * (2 * h + 1) as f64;
        let power = total / (f64::from(self.count) * (n as f64).powi(2) * HANN_POWER);
        l.level_db = Some(level);
        l.present = level >= PRESENT_DB;
        l.power = Some(power);
        l.absolute_dbfs = Some(10.0 * power.max(1e-30).log10());
        l
    }
}

/// The front end: what the meter reads from the input.
enum Front {
    /// A channel of 24 ksps around the tuned frequency.
    Channel(Box<Channelizer>),
    /// The input mixed by the negative offset of the tuned frequency.
    Mix(Nco),
    /// The input as it is (the tuned frequency at the centre).
    Direct,
}

/// The front end and the interval spectra. Interval `k` holds the windows whose last sample has index `k × L .. (k + 1) × L`
/// (L the interval in samples of the transformed stream); it is closed when the sample `(k + 1) × L` arrives. The windows
/// follow each other without overlap from the first sample.
pub struct Meter {
    front: Front,
    rate: f64,
    n: Option<usize>,
    fft: Option<WindowedFft>,
    win: Vec<Complex32>,
    work: Vec<Complex32>,
    acc: Vec<f64>,
    count: u32,
    len: u64,
    taken: u64,
    interval: u64,
}

impl Meter {
    /// A meter for input at `rate` whose tuned frequency is `offset_hz` from the centre, with intervals of `interval_s`. At
    /// [`FRONT_END_FROM`] or more the front end makes a 24 ksps channel; below, the input is mixed by `-offset_hz`. `None`
    /// if the channel cannot be built.
    pub fn new(rate: f64, offset_hz: f64, interval_s: f64) -> Option<Self> {
        let (front, work_rate) = if rate >= FRONT_END_FROM {
            let mut cfg = ChannelConfig::new(rate, offset_hz, CHANNEL_HZ, WORK_RATE);
            cfg.exact_rate = true;
            let ch = Channelizer::new(cfg).ok()?;
            let r = ch.output_rate();
            (Front::Channel(Box::new(ch)), r)
        } else if offset_hz != 0.0 {
            (Front::Mix(Nco::new(rate, -offset_hz)), rate)
        } else {
            (Front::Direct, rate)
        };
        Some(Meter::direct(front, work_rate, interval_s))
    }

    /// A meter without a front end for a stream at `rate` (the tuned frequency at its centre).
    pub fn plain(rate: f64, interval_s: f64) -> Self {
        Meter::direct(Front::Direct, rate, interval_s)
    }

    fn direct(front: Front, rate: f64, interval_s: f64) -> Self {
        let n = fft_size(rate);
        let fft = n.and_then(|n| {
            let mut f = WindowedFft::new(n, n, Window::Hann).ok()?;
            for (i, w) in f.window_mut().iter_mut().enumerate() {
                *w = (0.5 - 0.5 * (TAU * (i as f64 + 0.5) / n as f64).cos()) as f32;
            }
            Some(f)
        });
        Meter {
            front,
            rate,
            n,
            fft,
            win: Vec::with_capacity(n.unwrap_or(0)),
            work: Vec::new(),
            acc: vec![0.0; n.unwrap_or(0)],
            count: 0,
            len: ((interval_s * rate).round() as u64).max(1),
            taken: 0,
            interval: 0,
        }
    }

    /// The rate of the transformed stream, Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The transform size; `None` above [`MAX_RATE`].
    pub fn fft_size(&self) -> Option<usize> {
        self.n
    }

    /// Takes input samples `x`; `done` gets the index and the spectrum of each interval that closes.
    pub fn process(&mut self, x: &[Complex32], mut done: impl FnMut(u64, &Spectrum)) {
        let mut work = std::mem::take(&mut self.work);
        work.clear();
        let samples: &[Complex32] = match &mut self.front {
            Front::Channel(ch) => {
                ch.process(x, &mut work);
                &work
            }
            Front::Mix(nco) => {
                work.extend_from_slice(x);
                nco.mix_in_place(&mut work);
                &work
            }
            Front::Direct => x,
        };
        self.take(samples, &mut done);
        self.work = work;
    }

    fn take(&mut self, mut x: &[Complex32], done: &mut impl FnMut(u64, &Spectrum)) {
        while !x.is_empty() {
            let close = (self.interval + 1) * self.len;
            let mut m = x.len().min((close - self.taken) as usize);
            if let Some(n) = self.n {
                m = m.min(n - self.win.len());
                self.win.extend_from_slice(&x[..m]);
                if self.win.len() == n {
                    if let Some(f) = self.fft.as_mut() {
                        for (a, p) in self.acc.iter_mut().zip(f.run_power(&self.win)) {
                            *a += f64::from(*p);
                        }
                        self.count += 1;
                    }
                    self.win.clear();
                }
            }
            self.taken += m as u64;
            x = &x[m..];
            if self.taken == close {
                let s = Spectrum {
                    rate: self.rate,
                    power: std::mem::take(&mut self.acc),
                    count: self.count,
                };
                done(self.interval, &s);
                self.acc = s.power;
                self.acc.fill(0.0);
                self.count = 0;
                self.interval += 1;
            }
        }
    }

    /// Starts the interval sums again from the next sample; the front end keeps its state.
    pub fn restart(&mut self) {
        self.acc.fill(0.0);
        self.count = 0;
        self.win.clear();
        self.taken = 0;
        self.interval = 0;
    }
}
