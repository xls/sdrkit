//! A receiver of one keyed carrier: the channel filter (the carrier mixed to 0 Hz, summed down to the envelope rate, a
//! low-pass filter of the chosen width), the envelope, the key detector and the timing decoder. It takes complex samples at
//! any rate of at least the envelope rate of its width.

use sdrkit_algo_dsp::fir::{lowpass, Fir};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_modem::key::KeyDetector;
use sdrkit_core::Complex32;
use sdrkit_util_text::morse::{Classifier, Decoder, DecoderConfig, Event};

sdrkit_core::choice_enum! {
    /// The width of the channel filter of one carrier.
    pub enum FilterWidth {
        /// 50 Hz: the most sensitive, for slow keying.
        Hz50 = ("50hz", "50 Hz"),
        /// 100 Hz.
        Hz100 = ("100hz", "100 Hz"),
        /// 250 Hz: keying up to 60 WPM.
        Hz250 = ("250hz", "250 Hz"),
        /// 500 Hz: keying up to 60 WPM and a carrier that drifts.
        Hz500 = ("500hz", "500 Hz"),
    }
}

impl FilterWidth {
    /// The full width of the pass band, Hz.
    pub fn width_hz(self) -> f64 {
        match self {
            FilterWidth::Hz50 => 50.0,
            FilterWidth::Hz100 => 100.0,
            FilterWidth::Hz250 => 250.0,
            FilterWidth::Hz500 => 500.0,
        }
    }

    /// The pass and stop edges of the low-pass filter, Hz.
    fn edges(self) -> (f64, f64) {
        match self {
            FilterWidth::Hz50 => (25.0, 55.0),
            FilterWidth::Hz100 => (50.0, 110.0),
            FilterWidth::Hz250 => (125.0, 200.0),
            FilterWidth::Hz500 => (250.0, 375.0),
        }
    }

    /// The top speed that the filter passes, words a minute.
    pub fn max_wpm(self) -> f64 {
        match self {
            FilterWidth::Hz50 => 20.0,
            FilterWidth::Hz100 => 40.0,
            _ => 60.0,
        }
    }

    /// The least envelope rate, Hz: the input is summed down by the largest whole factor that keeps at least this rate.
    pub fn envelope_rate(self) -> f64 {
        match self {
            FilterWidth::Hz500 => 2000.0,
            _ => 1000.0,
        }
    }
}

/// The level of a mark over the highest mark before it at which the timing decoder starts again (20 dB in amplitude).
const MARK_JUMP: f32 = 10.0;

/// Why a [`MorseReceiver`] cannot be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateError;

impl std::fmt::Display for RateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "the Morse stack needs a sample rate of at least the envelope rate of its filter",
        )
    }
}

impl std::error::Error for RateError {}

/// The decimation and the low-pass filter of `width` for input at `rate`. An error when `rate` is below the envelope rate.
fn sum_down(rate: f64, width: FilterWidth) -> Result<(usize, Fir<Complex32>), RateError> {
    let dec = (rate / width.envelope_rate()).floor();
    if !(rate.is_finite() && dec >= 1.0) {
        return Err(RateError);
    }
    let (pass, stop) = width.edges();
    let taps = lowpass(rate / dec, pass, stop, 50.0).map_err(|_| RateError)?;
    Ok((dec as usize, Fir::new(&taps)))
}

/// The channel filter of one carrier: the carrier mixed to 0 Hz, summed down to the envelope rate and low-pass filtered to
/// the chosen width. No allocation after [`ChannelFilter::new`].
#[derive(Clone, Debug)]
pub struct ChannelFilter {
    rate: f64,
    offset_hz: f64,
    width: FilterWidth,
    nco: Nco,
    acc: Complex32,
    cnt: usize,
    dec: usize,
    lpf: Fir<Complex32>,
    filtered: Complex32,
}

impl ChannelFilter {
    /// The filter of `width` around the carrier at `offset_hz` in samples at `rate` Hz. An error when `rate` is below the
    /// envelope rate of `width`.
    pub fn new(rate: f64, offset_hz: f64, width: FilterWidth) -> Result<Self, RateError> {
        let (dec, lpf) = sum_down(rate, width)?;
        Ok(ChannelFilter {
            rate,
            offset_hz,
            width,
            nco: Nco::new(rate, -offset_hz),
            acc: Complex32::default(),
            cnt: 0,
            dec,
            lpf,
            filtered: Complex32::default(),
        })
    }

    /// Changes the width. Allocates.
    pub fn set_width(&mut self, width: FilterWidth) -> Result<(), RateError> {
        let (dec, lpf) = sum_down(self.rate, width)?;
        self.width = width;
        self.dec = dec;
        self.lpf = lpf;
        self.acc = Complex32::default();
        self.cnt = 0;
        Ok(())
    }

    /// Moves to the carrier at `hz`.
    pub fn set_offset_hz(&mut self, hz: f64) {
        self.offset_hz = hz;
        self.nco.set_freq(-hz);
    }

    /// The time the low-pass filter takes to settle, s.
    fn settle(&self) -> f64 {
        self.lpf.len() as f64 / self.envelope_rate()
    }

    /// The rate of [`ChannelFilter::filtered`], Hz.
    pub fn envelope_rate(&self) -> f64 {
        self.rate / self.dec as f64
    }

    /// The newest output: the carrier at 0 Hz.
    pub fn filtered(&self) -> Complex32 {
        self.filtered
    }

    /// One input sample. Returns whether an output sample was made.
    #[inline]
    pub fn push(&mut self, x: Complex32) -> bool {
        self.acc += x * self.nco.next_sample();
        self.cnt += 1;
        if self.cnt < self.dec {
            return false;
        }
        self.cnt = 0;
        self.filtered = self.lpf.process_sample(self.acc / self.dec as f32);
        self.acc = Complex32::default();
        true
    }
}

/// One keyed carrier from complex samples to Morse symbols. No allocation after [`MorseReceiver::new`].
#[derive(Clone, Debug)]
pub struct MorseReceiver {
    chan: ChannelFilter,
    /// The key detector.
    pub key: KeyDetector,
    /// The timing decoder.
    pub timing: Decoder,
    syms: Vec<Event>,
    /// Envelope samples since the key detector started, and the time of the key detector from which intervals go to the
    /// timing decoder, s. An interval that began earlier is partial or comes from levels that had not settled; of these
    /// only a pause goes.
    env_n: u64,
    feed_from: f64,
    settle: f64,
    /// The highest level of a mark given to the timing decoder since it started (0 before the first).
    mark_ref: f32,
}

impl MorseReceiver {
    /// A receiver of the carrier at `offset_hz` in samples at `rate` Hz, through a channel filter of `width`, with the adaptive
    /// (`adaptive`) or the clustering timing decoder, at a fixed speed of `wpm` (5 to 60) or following the speed (`wpm` 0). An
    /// error when `rate` is below the envelope rate of `width`.
    pub fn new(
        rate: f64,
        offset_hz: f64,
        adaptive: bool,
        wpm: f32,
        width: FilterWidth,
    ) -> Result<Self, RateError> {
        let chan = ChannelFilter::new(rate, offset_hz, width)?;
        let key = KeyDetector::new(chan.envelope_rate()).map_err(|_| RateError)?;
        let settle = chan.settle();
        Ok(MorseReceiver {
            chan,
            key,
            timing: timing(adaptive, wpm),
            syms: Vec::with_capacity(8),
            env_n: 0,
            feed_from: settle,
            settle,
            mark_ref: 0.0,
        })
    }

    /// Starts on `history`, the samples just before the live input. The history runs twice: the first time it sets the
    /// levels of the key detector and nothing goes to the timing decoder; the second time the timing decoder takes the whole
    /// intervals that begin after the filter settled, and a pause that ends after it. The symbols of the history are
    /// dropped.
    pub fn start(&mut self, history: &[Complex32]) {
        self.feed_from = f64::INFINITY;
        for &x in history {
            self.push(x);
        }
        self.feed_from = self.key_time() + self.settle;
        for &x in history {
            self.push(x);
        }
        self.syms.clear();
    }

    /// The time of the key detector, s.
    fn key_time(&self) -> f64 {
        self.env_n as f64 / self.envelope_rate()
    }

    /// The carrier frequency it listens to, Hz.
    pub fn offset_hz(&self) -> f64 {
        self.chan.offset_hz
    }

    /// Moves to the carrier at `hz`.
    pub fn set_offset_hz(&mut self, hz: f64) {
        self.chan.set_offset_hz(hz);
    }

    /// Changes the timing decoder (and forgets what it learned).
    pub fn set_timing(&mut self, adaptive: bool, wpm: f32) {
        self.timing = timing(adaptive, wpm);
        self.mark_ref = 0.0;
    }

    /// Changes the channel filter. The key detector starts again; the timing decoder keeps what it learned. Allocates.
    pub fn set_width(&mut self, width: FilterWidth) -> Result<(), RateError> {
        let mut chan = self.chan.clone();
        chan.set_width(width)?;
        self.key = KeyDetector::new(chan.envelope_rate()).map_err(|_| RateError)?;
        self.chan = chan;
        self.settle = self.chan.settle();
        self.env_n = 0;
        self.feed_from = self.settle;
        self.mark_ref = 0.0;
        Ok(())
    }

    /// The width of the channel filter.
    pub fn width(&self) -> FilterWidth {
        self.chan.width
    }

    /// The input rate, Hz.
    pub fn rate(&self) -> f64 {
        self.chan.rate
    }

    /// The rate of the envelope and of [`MorseReceiver::filtered`], Hz.
    pub fn envelope_rate(&self) -> f64 {
        self.chan.envelope_rate()
    }

    /// The newest output of the channel filter: the carrier at 0 Hz.
    pub fn filtered(&self) -> Complex32 {
        self.chan.filtered()
    }

    /// The mark level over the noise level, dB.
    pub fn snr_db(&self) -> f32 {
        self.key.snr_db()
    }

    /// The speed of the sender, words a minute (0 until known).
    pub fn wpm(&self) -> f64 {
        if self.timing.is_locked() {
            self.timing.wpm()
        } else {
            0.0
        }
    }

    /// One sample. Returns whether an envelope sample was made. The symbols it decided wait in
    /// [`MorseReceiver::take_symbols`].
    #[inline]
    pub fn push(&mut self, x: Complex32) -> bool {
        if !self.chan.push(x) {
            return false;
        }
        let env = self.chan.filtered().norm();
        self.env_n += 1;
        let now = self.key_time();
        let syms = &mut self.syms;
        match self.key.step(env, self.timing.dot()) {
            Some(k) => {
                // A whole interval, or a pause: a space of at least seven dots is a pause whatever part of it was seen.
                let whole = k.end - k.seconds >= self.feed_from;
                let pause =
                    !k.on && k.end >= self.feed_from && k.seconds >= 7.0 * self.timing.dot();
                if whole || pause {
                    if k.on {
                        // A mark 20 dB over every mark before is another sender, such as a strong carrier after the
                        // leakage of its neighbours: the timing decoder forgets what it learned.
                        if self.mark_ref > 0.0 && k.level > MARK_JUMP * self.mark_ref {
                            self.timing.reset();
                        }
                        self.mark_ref = self.mark_ref.max(k.level);
                    }
                    self.timing.push(k.on, k.seconds, &mut |e| syms.push(e));
                }
            }
            None => {
                let up = self.key.seconds_in_state();
                if !self.key.is_on() && now >= self.feed_from {
                    self.timing.idle(up, &mut |e| syms.push(e));
                }
            }
        }
        true
    }

    /// Whether symbols wait.
    pub fn has_symbols(&self) -> bool {
        !self.syms.is_empty()
    }

    /// The symbols decided since the last call, oldest first.
    pub fn take_symbols(&mut self) -> std::vec::Drain<'_, Event> {
        self.syms.drain(..)
    }
}

/// The timing decoder: adaptive or two-cluster, at a fixed speed of `wpm` (5 or more) or following the speed.
fn timing(adaptive: bool, wpm: f32) -> Decoder {
    let classifier = if adaptive {
        Classifier::Adaptive
    } else {
        Classifier::Clusters
    };
    let cfg = if wpm >= 5.0 {
        DecoderConfig::fixed(wpm.min(60.0) as f64)
    } else {
        DecoderConfig::following(20.0)
    };
    // Both settings are in range.
    Decoder::new(DecoderConfig { classifier, ..cfg })
        .unwrap_or_else(|_| unreachable!("valid Morse speeds"))
}
