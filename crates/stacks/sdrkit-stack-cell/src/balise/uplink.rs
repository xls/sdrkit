//! The Eurobalise up-link front end: the channel (mixer, low-pass and decimation), the FM discriminator, the
//! burst detector, the slicer and the bit clock. The `balise.uplink` decoder.

use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Values;
use sdrkit_core::{Complex32, ConfigError};
use sdrkit_graph::{no_values, Decoder, Format, Item, Kind, Outbox, Port, Timed};
use std::f64::consts::{PI, TAU};

/// The bit rate, bit/s (SUBSET-036 4.2).
pub const BIT_RATE: f64 = 564_480.0;
/// The deviation, Hz (SUBSET-036 4.2).
pub const DEV: f64 = 282_500.0;
/// Seconds of signal between two time items.
pub const TICK_S: f64 = 0.1;
/// The bits decided after the signal was gone: the burst ends when the power stays low this long, seconds.
const TAIL_S: f64 = 30e-6;

/// One burst of the up-link.
#[derive(Clone, Debug, PartialEq)]
pub struct BaliseBurst {
    /// Signal time of its start, seconds.
    pub time: f64,
    /// Its bits, in the order received.
    pub bits: Vec<u8>,
    /// Duration, microseconds.
    pub duration_us: f64,
    /// The measured bit rate, bit/s.
    pub bit_rate_hz: f64,
    /// The mean slicer threshold over the burst (the carrier offset), Hz.
    pub carrier_offset_hz: f64,
    /// SNR, dB.
    pub snr_db: f64,
}

/// What the front end passes on (`balise.uplink_item`): a burst, or the signal time every [`TICK_S`].
#[derive(Clone, Debug, PartialEq)]
pub enum UplinkItem {
    /// A burst that ended.
    Burst(BaliseBurst),
    /// The signal time went on (the item's own time).
    Tick,
}

impl Item for UplinkItem {
    const KIND: &'static str = "balise.uplink_item";
}

/// The channel of the up-link: a phase-continuous mixer, then a Hamming-windowed sinc low-pass at 900 kHz evaluated only at the
/// outputs kept, decimating to about 3 MS/s.
pub struct Channel {
    fs: f64,
    r: f64,
    osc: Nco,
    fir: FirDecimator<Complex32>,
    mixed: Vec<Complex32>,
}

impl Channel {
    /// The channel for input at `fs`, mixed by `shift_hz` (the radio centre minus the up-link frequency).
    pub fn new(fs: f64, shift_hz: f64) -> Channel {
        let d = ((fs / 3e6).floor() as usize).max(1);
        let mut n = (3.5 * fs / 700e3).floor() as usize;
        if n.is_multiple_of(2) {
            n += 1;
        }
        let fc = 900e3 / fs;
        let m = (n - 1) as f64 / 2.0;
        let w = Window::Hamming.symmetric(n);
        let mut taps: Vec<f64> = (0..n)
            .map(|k| {
                let x = k as f64 - m;
                let s = if x == 0.0 {
                    2.0 * fc
                } else {
                    (TAU * fc * x).sin() / (PI * x)
                };
                s * w[k]
            })
            .collect();
        let sum: f64 = taps.iter().sum();
        taps.iter_mut().for_each(|t| *t /= sum);
        let taps: Vec<f32> = taps.into_iter().map(|t| t as f32).collect();
        Channel {
            fs,
            r: fs / d as f64,
            osc: Nco::new(fs, shift_hz),
            fir: FirDecimator::new(&taps, d),
            mixed: Vec::new(),
        }
    }

    /// The output rate, Hz.
    pub fn rate(&self) -> f64 {
        self.r
    }

    /// Mixes by a new shift from the next sample on.
    pub fn retune(&mut self, shift_hz: f64) {
        self.osc = Nco::new(self.fs, shift_hz);
    }

    /// Appends the channel samples of `x` to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        self.osc.mix_in_place(&mut self.mixed);
        self.fir.process(&self.mixed, out);
    }
}

#[derive(Default)]
struct Open {
    start_sample: u64,
    samples: u64,
    below: u64,
    bits: Vec<u8>,
    thr_sum: f64,
    thr_n: u64,
    snr: f64,
}

/// The burst detector, the slicer and the bit clock, fed one channel sample at a time with its frequency (Hz) and power.
pub struct Bursts {
    r: f64,
    sps: f64,
    pe: f64,
    nf: f64,
    nout: u64,
    mean_buf: Vec<f32>,
    mean_pos: usize,
    mean_sum: f32,
    peak: f32,
    valley: f32,
    level: bool,
    clock: f64,
    open: Option<Open>,
}

impl Bursts {
    /// A detector for channel samples at `r` Hz.
    pub fn new(r: f64) -> Bursts {
        let sps = r / BIT_RATE;
        Bursts {
            r,
            sps,
            pe: 0.0,
            nf: 0.0,
            nout: 0,
            mean_buf: vec![0.0; ((sps / 2.0).round() as usize).max(1)],
            mean_pos: 0,
            mean_sum: 0.0,
            peak: DEV as f32,
            valley: -DEV as f32,
            level: false,
            clock: -0.5,
            open: None,
        }
    }

    /// Channel samples taken so far.
    pub fn samples(&self) -> u64 {
        self.nout
    }

    /// Forgets a burst half received (a retune).
    pub fn drop_open(&mut self) {
        self.open = None;
    }

    fn reset_slicer(&mut self) {
        self.peak = DEV as f32;
        self.valley = -DEV as f32;
        self.clock = -0.5;
        self.mean_buf.iter_mut().for_each(|v| *v = 0.0);
        self.mean_sum = 0.0;
        self.mean_pos = 0;
    }

    fn end_burst(&mut self, out: &mut Vec<BaliseBurst>) {
        let Some(mut b) = self.open.take() else {
            return;
        };
        let tail_bits = (TAIL_S * BIT_RATE).round() as usize;
        let keep = b.bits.len().saturating_sub(tail_bits);
        b.bits.truncate(keep);
        let dur_samples = b.samples.saturating_sub(b.below).max(1);
        let duration = dur_samples as f64 / self.r;
        out.push(BaliseBurst {
            time: b.start_sample as f64 / self.r,
            duration_us: duration * 1e6,
            bit_rate_hz: b.bits.len() as f64 / duration,
            carrier_offset_hz: b.thr_sum / b.thr_n.max(1) as f64,
            snr_db: b.snr,
            bits: b.bits,
        });
    }

    /// Takes one channel sample: its frequency `f_hz` (the discriminator output) and its power `p`. A burst that ends is added to
    /// `out`.
    pub fn push(&mut self, f_hz: f32, p: f32, out: &mut Vec<BaliseBurst>) {
        let p = p as f64;
        self.pe += 0.05 * (p - self.pe);
        if self.nout < 2000 {
            self.nf = self.pe;
        } else if self.pe < self.nf {
            self.nf += 0.01 * (self.pe - self.nf);
        } else {
            self.nf += 1e-5 * (self.pe - self.nf);
        }
        self.nout += 1;
        let snr_now = 10.0 * (self.pe / self.nf.max(1e-30) - 1.0).max(1e-3).log10();
        if self.open.is_none() {
            if self.nout > 2000 && self.pe > 8.0 * self.nf {
                self.reset_slicer();
                self.open = Some(Open {
                    start_sample: self.nout - 1,
                    snr: snr_now,
                    ..Default::default()
                });
            } else {
                return;
            }
        }
        // Slicer.
        let half = self.mean_buf.len();
        self.mean_sum += f_hz - self.mean_buf[self.mean_pos];
        self.mean_buf[self.mean_pos] = f_hz;
        self.mean_pos = (self.mean_pos + 1) % half;
        let v = self.mean_sum / half as f32;
        let decay = 1.0 / (40.0 * self.sps as f32);
        let lim = 3.0 * DEV as f32;
        if v > self.peak {
            self.peak = v;
        } else {
            self.peak -= (self.peak - v) * decay;
        }
        if v < self.valley {
            self.valley = v;
        } else {
            self.valley -= (self.valley - v) * decay;
        }
        self.peak = self.peak.clamp(-lim, lim);
        self.valley = self.valley.clamp(-lim, lim);
        let thr = (self.peak + self.valley) / 2.0;
        let level = v > thr;
        // Bit clock: a level change pulls the phase towards the transition.
        self.clock += 1.0 / self.sps;
        if level != self.level {
            self.clock *= 0.75;
        }
        self.level = level;
        let pe = self.pe;
        let nf = self.nf;
        let ends = {
            let Some(b) = self.open.as_mut() else { return };
            if self.clock >= 0.5 {
                b.bits.push(level as u8);
                self.clock -= 1.0;
            }
            b.samples += 1;
            b.thr_sum += thr as f64;
            b.thr_n += 1;
            b.snr = 0.98 * b.snr + 0.02 * snr_now;
            if pe < 4.0 * nf {
                b.below += 1;
                b.below as f64 >= TAIL_S * self.r
            } else {
                b.below = 0;
                false
            }
        };
        if ends {
            self.end_burst(out);
        }
    }
}

/// The whole front end: [`Channel`], then the shared FM discriminator (Hz), then [`Bursts`].
pub struct FrontEnd {
    channel: Channel,
    disc: Discriminator,
    bursts: Bursts,
    y: Vec<Complex32>,
}

impl FrontEnd {
    /// The front end for input at `fs`, mixed by `shift_hz`.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NoRate`] for a rate that is not positive and finite. The [`DspError`](sdrkit_algo_dsp::DspError) of the
    /// discriminator, as [`ConfigError::Stage`].
    pub fn new(fs: f64, shift_hz: f64) -> Result<FrontEnd, ConfigError> {
        if !(fs > 0.0 && fs.is_finite()) {
            return Err(ConfigError::NoRate);
        }
        let channel = Channel::new(fs, shift_hz);
        let r = channel.rate();
        // A deviation of 1 Hz outputs 1.0: the output is in Hz.
        let disc = Discriminator::try_new(r, 1.0)?;
        Ok(FrontEnd {
            channel,
            disc,
            bursts: Bursts::new(r),
            y: Vec::new(),
        })
    }

    /// The channel rate, Hz.
    pub fn rate(&self) -> f64 {
        self.channel.rate()
    }

    /// Signal time of the newest channel sample, seconds.
    pub fn time_s(&self) -> f64 {
        self.bursts.samples() as f64 / self.channel.rate()
    }

    /// Mixes by a new shift from the next sample on, and forgets a burst half received.
    pub fn retune(&mut self, shift_hz: f64) {
        self.channel.retune(shift_hz);
        self.bursts.drop_open();
    }

    /// Takes input samples; every burst that ends is added to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<BaliseBurst>) {
        self.y.clear();
        self.channel.process(x, &mut self.y);
        for &s in &self.y {
            let f = self.disc.process_sample(s);
            self.bursts.push(f, s.norm_sqr(), out);
        }
    }
}

/// The `balise.uplink` decoder: IQ in, [`UplinkItem`]s out.
#[derive(Default)]
pub struct Uplink {
    fe: Option<FrontEnd>,
    next_tick: f64,
    found: Vec<BaliseBurst>,
}

impl Decoder for Uplink {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "balise.uplink"
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<UplinkItem>("out")]
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.fe = Some(FrontEnd::new(f.rate, f.centre_hz - f.tuned_hz)?);
        self.next_tick = TICK_S;
        Ok(vec![Format::new(Kind::typed::<UplinkItem>(), 0.0)])
    }
    fn retune(&mut self, f: &Format) {
        if let Some(fe) = self.fe.as_mut() {
            fe.retune(f.centre_hz - f.tuned_hz);
        }
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let Some(fe) = self.fe.as_mut() else { return };
        fe.process(x, &mut self.found);
        let items = out.items::<UplinkItem>(0);
        for b in self.found.drain(..) {
            items.push(Timed {
                time: b.time,
                value: UplinkItem::Burst(b),
            });
        }
        let now = fe.time_s();
        while now >= self.next_tick {
            items.push(Timed {
                time: self.next_tick,
                value: UplinkItem::Tick,
            });
            self.next_tick += TICK_S;
        }
    }
}
