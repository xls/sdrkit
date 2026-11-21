//! The receiver front of the 2-FSK links of `rc_links` and `mavlink_sik`: bursts anywhere in the capture, and for one link
//! rate the channel of a burst, its discriminator, the sync pattern and the bits after it.
//!
//! Bursts come from `sdrkit_algo_modem::bandburst` with frames of the smallest power of two from 64 to 2048 whose bins are at
//! most 40 kHz wide. For a link at rate `r`, a burst wider than `8 r + 200 kHz` is skipped; the burst is mixed to its centre,
//! filtered (cut-off `1.2 r + 20 kHz`, at least `4 fs / cut-off` taps), decimated by `floor(fs / (8 r))`, and the phase step of
//! each sample is the discriminator. The pattern is searched at steps of 1/8 symbol with `sdrkit_algo_modem::fsksync`.

use sdrkit_algo_dsp::approx;
use sdrkit_algo_modem::bandburst::{BandBurst, BandConfig};
use sdrkit_algo_modem::burstmix::BurstMixer;
use sdrkit_algo_modem::fsksync::{BitSync, Polarity, SyncHit};
use sdrkit_core::Complex32;

/// The widest bin of the burst detector, Hz.
pub const BIN_HZ: f64 = 40e3;
/// A rate is searched only when the sample rate is at least this many times the rate.
pub const RATE_MARGIN: f64 = 2.5;

/// The burst detector settings of the FSK links at `rate_hz` with threshold `threshold_db`.
pub fn band_config(rate_hz: f64, threshold_db: f64) -> BandConfig {
    let mut c = BandConfig::new(rate_hz, BandConfig::frame_for(rate_hz, BIN_HZ, 64, 2048));
    c.threshold_db = threshold_db;
    c
}

/// A burst taken from the detector: its samples and what the detector measured.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Burst {
    /// The samples at the input rate.
    pub samples: Vec<Complex32>,
    /// The stream index of the first sample.
    pub start: u64,
    /// The centre, Hz from the input centre.
    pub offset_hz: f64,
    /// The mean width, Hz.
    pub width_hz: f64,
    /// The peak level over the floor, dB.
    pub peak_db: f32,
}

impl Burst {
    /// Copies a detector burst into `self`, reusing its buffer.
    pub fn take(&mut self, b: &BandBurst<'_>) {
        self.samples.clear();
        self.samples.extend_from_slice(b.samples);
        self.start = b.start;
        self.offset_hz = b.offset_hz;
        self.width_hz = b.width_hz;
        self.peak_db = b.peak_db;
    }
}

/// The front of one burst for one rate (module documentation). The filters of each rate are made once.
#[derive(Default)]
pub struct FskFront {
    fs: f64,
    mixers: Vec<(f64, f64, BurstMixer)>,
    base: Vec<Complex32>,
    disc: Vec<f32>,
    sync: BitSync,
    sps: f64,
    out_rate: f64,
}

impl FskFront {
    /// A front for input at `fs` Hz.
    pub fn new(fs: f64) -> FskFront {
        FskFront {
            fs,
            ..FskFront::default()
        }
    }

    /// The input rate.
    pub fn rate_hz(&self) -> f64 {
        self.fs
    }

    /// The rate `r` is searched at this input rate.
    pub fn fits(&self, r: f64) -> bool {
        self.fs >= RATE_MARGIN * r
    }

    /// Makes the discriminator of burst `b` for a link at `rate` bit/s, mixed to `b.offset_hz + shift_hz`, with the filter
    /// cut-off `cutoff_hz` (`None`: `1.2 rate + 20 kHz`). False when the rate does not fit or the burst is too wide for it.
    pub fn demod(&mut self, b: &Burst, rate: f64, shift_hz: f64, cutoff_hz: Option<f64>) -> bool {
        if !self.fits(rate) || b.width_hz > 8.0 * rate + 200e3 {
            return false;
        }
        let cut = cutoff_hz.unwrap_or(1.2 * rate + 20e3);
        let i = match self
            .mixers
            .iter()
            .position(|(r, c, _)| *r == rate && *c == cut)
        {
            Some(i) => i,
            None => {
                let decim = ((self.fs / (8.0 * rate)).floor() as usize).max(1);
                let taps = (4.0 * self.fs / cut).ceil() as usize;
                let Ok(m) = BurstMixer::new(self.fs, decim, cut.min(self.fs / 2.0), taps) else {
                    return false;
                };
                self.mixers.push((rate, cut, m));
                self.mixers.len() - 1
            }
        };
        let m = &mut self.mixers[i].2;
        m.run(&b.samples, b.start, b.offset_hz + shift_hz, &mut self.base);
        self.out_rate = m.out_rate_hz();
        self.sps = self.out_rate / rate;
        self.disc.clear();
        self.disc.extend(self.base.windows(2).map(|w| {
            let z = w[1] * w[0].conj();
            approx::atan2(z.im, z.re)
        }));
        true
    }

    /// Samples a symbol of the last [`demod`](Self::demod).
    pub fn sps(&self) -> f64 {
        self.sps
    }

    /// The discriminator of the last [`demod`](Self::demod), radians a sample.
    pub fn disc(&self) -> &[f32] {
        &self.disc
    }

    /// The output rate of the last [`demod`](Self::demod), Hz.
    pub fn out_rate(&self) -> f64 {
        self.out_rate
    }

    /// Searches the last discriminator for `pattern` with at most `max_errors` bit errors.
    pub fn find(&mut self, pattern: &[u8], max_errors: u32, polarity: Polarity) -> Option<SyncHit> {
        self.sync
            .find(&self.disc, self.sps, pattern, max_errors, polarity, 8)
    }

    /// Up to `n` bits after the pattern of `hit` into `out`. Returns how many were read.
    pub fn bits(&self, hit: &SyncHit, n: usize, out: &mut Vec<u8>) -> usize {
        self.sync.bits(&self.disc, hit, n, out)
    }

    /// The carrier offset that the threshold of `hit` shows, Hz.
    pub fn cfo_hz(&self, hit: &SyncHit) -> f64 {
        f64::from(hit.threshold) * self.out_rate / std::f64::consts::TAU
    }

    /// The input sample index of the start of bit `k` after the pattern of `hit` of burst `b`.
    pub fn sample_of(&self, b: &Burst, hit: &SyncHit, k: usize) -> u64 {
        let decim = self.fs / self.out_rate.max(1.0);
        let at = hit.at + (hit.len + k) as f64 * hit.sps;
        b.start + (at * decim).round().max(0.0) as u64
    }
}

/// The bits of `bytes` (the most significant first unless `lsb_first`) appended to `out`.
pub fn bits_of(bytes: &[u8], lsb_first: bool, out: &mut Vec<u8>) {
    let order = if lsb_first {
        sdrkit_util_bits::BitOrder::Lsb
    } else {
        sdrkit_util_bits::BitOrder::Msb
    };
    sdrkit_util_bits::pack::unpack(bytes, order, out);
}

/// Whole bytes of `bits` (the first bit the most significant unless `lsb_first`) into `out` (cleared first).
pub fn bytes_of(bits: &[u8], lsb_first: bool, out: &mut Vec<u8>) {
    out.clear();
    let order = if lsb_first {
        sdrkit_util_bits::BitOrder::Lsb
    } else {
        sdrkit_util_bits::BitOrder::Msb
    };
    let n = bits.len() / 8 * 8;
    sdrkit_util_bits::pack::pack(&bits[..n], order, sdrkit_util_bits::Pad::Zero, out);
}
