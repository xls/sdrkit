//! HF weather fax: the USB audio, the fax demodulator, the start and stop tone detector, the phasing pulse detector and line fit,
//! joining a picture in progress, slant, and the picture builder with its controls.
//!
//! Standard: WMO-No. 386 Volume I Part I Attachment 4, ITU-T T.4 practice for radiofacsimile

use sdrkit_algo_dsp::agc::Agc;
use sdrkit_algo_dsp::dc::RealDcBlocker;
use sdrkit_algo_dsp::decim::ComplexDecimator;
use sdrkit_algo_dsp::fir::{self, Fir};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::goertzel::goertzel_windowed;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::level::M2m4;
use sdrkit_codec_image::line::{LineImage, Overflow};
use sdrkit_codec_image::{geom, PixelFormat};
use sdrkit_core::media::{Picture, PictureState};
use sdrkit_core::Complex32;
use std::collections::VecDeque;
use std::f64::consts::PI;

/// Width of a picture for an index of cooperation: IOC x pi cut to an integer.
pub fn width_for_ioc(ioc: u32) -> usize {
    (f64::from(ioc) * PI) as usize
}

/// The bandwidth the SNR of a fax signal is measured in (the voice channel it is sent in).
pub const SNR_BANDWIDTH_HZ: f64 = 3000.0;
/// The most lines a picture can have.
pub const MAX_LINES: usize = 20_000;
/// The fax centre above the dial.
pub const CENTRE_HZ: f64 = 1900.0;
/// Black and white.
pub const BLACK_HZ: f64 = 1500.0;
pub const WHITE_HZ: f64 = 2300.0;

/// The upper sideband from the dial as real audio: a complex band-pass from 100 to 3900 Hz, the real part, decimated to about
/// 12 kHz, with a slow AGC (attack 8 ms, decay 1.7 s, output envelope 0.3).
#[derive(Clone, Debug)]
pub struct UsbAudio {
    dec: ComplexDecimator,
    agc: Agc,
    buf: Vec<Complex32>,
}

impl UsbAudio {
    /// For complex input at `rate` (24 kHz produces 12 kHz audio). An error for a rate too low for the band (below 8 kHz).
    pub fn new(rate: f64) -> Result<Self, sdrkit_algo_dsp::DspError> {
        if !(rate >= 8000.0 && rate.is_finite()) {
            return Err(sdrkit_algo_dsp::DspError::OutOfRange {
                param: sdrkit_algo_dsp::Param::Rate,
                min: 8000.0,
                max: f64::INFINITY,
                got: rate,
            });
        }
        let taps = fir::bandpass_complex(rate, 100.0, 3900.0, 400.0, 60.0)?;
        let decim = ((rate / 12_000.0).round() as usize).max(1);
        Ok(UsbAudio {
            dec: ComplexDecimator::new(&taps, decim)?,
            agc: Agc::new(rate / decim as f64, 0.3, 8.3e-3, 1.67, 1e6),
            buf: Vec::new(),
        })
    }
    /// The output rate for input at `rate`.
    pub fn output_rate(rate: f64) -> f64 {
        rate / ((rate / 12_000.0).round()).max(1.0)
    }
    /// Appends the audio of `x` (within -1 to 1).
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        self.buf.clear();
        self.dec.process(x, &mut self.buf);
        let start = out.len();
        out.extend(self.buf.iter().map(|v| v.re));
        let a = &mut out[start..];
        self.agc.process_real(a);
        for v in a {
            *v = v.clamp(-1.0, 1.0);
        }
    }
}

/// The cut-offs of the narrow filters, Hz, narrowest first. The discriminator needs the signal over the noise of its filter:
/// a narrower filter keeps a weak picture above the FM threshold and loses fine detail of a strong one.
pub const NARROW_CUTS_HZ: [f64; 3] = [650.0, 800.0, 1000.0];
/// The SNR in 3 kHz from which each narrow filter after the first is used, dB.
const NARROW_FROM_DB: [f64; 2] = [3.0, 7.0];
/// The seconds of signal the SNR estimate remembers.
const SNR_MEMORY_S: f64 = 2.0;

/// The narrow filter for a picture at `snr_db` in 3 kHz (an index of [`NARROW_CUTS_HZ`]): the narrowest without an estimate.
fn narrow_for(snr_db: Option<f64>) -> usize {
    snr_db.map_or(0, |s| NARROW_FROM_DB.iter().filter(|&&from| s >= from).count())
}

/// The fax demodulator: a DC blocker, the mix by -1900 Hz, low-pass filters of equal length (three narrow ones and a wide one)
/// and discriminators in hertz. A fade or a gap holds the last value. The output frequencies are absolute (1500 black, 2300
/// white). The SNR is estimated from the second and fourth moments of the wide filter's samples
/// (`sdrkit_algo_modem::level::M2m4`: the signal has a constant envelope).
#[derive(Clone, Debug)]
pub struct FaxDemod {
    dc: RealDcBlocker,
    nco: Nco,
    narrow: [Fir<Complex32>; 3],
    wide: Fir<Complex32>,
    disc_n: [Discriminator; 3],
    disc_w: Discriminator,
    last_n: [f64; 3],
    last_w: f64,
    lvl: f32,
    moments: M2m4,
}

impl FaxDemod {
    /// For audio at `rate`: filters of Kaiser beta 5.65 and length 3.62 x rate / 600 made odd, cut-offs [`NARROW_CUTS_HZ`]
    /// (narrow) and 1400 Hz (wide). An error for a rate below 8 kHz.
    pub fn new(rate: f64) -> Result<Self, sdrkit_algo_dsp::DspError> {
        if !(rate >= 8000.0 && rate.is_finite()) {
            return Err(sdrkit_algo_dsp::DspError::OutOfRange {
                param: sdrkit_algo_dsp::Param::Rate,
                min: 8000.0,
                max: f64::INFINITY,
                got: rate,
            });
        }
        let mut len = (3.62 * rate / 600.0).ceil() as usize;
        if len.is_multiple_of(2) {
            len += 1;
        }
        let taps = |cut: f64| -> Result<Vec<f32>, sdrkit_algo_dsp::DspError> {
            Ok(fir::windowed_sinc(len, cut / rate, 5.65)?
                .iter()
                .map(|&v| v as f32)
                .collect())
        };
        // A deviation of 1 Hz: the discriminators give hertz.
        let disc = Discriminator::try_new(rate, 1.0)?;
        let [a, b, c] = NARROW_CUTS_HZ;
        let moments = M2m4::psk()
            .with_memory((SNR_MEMORY_S * rate) as u32)
            .map_err(|_| sdrkit_algo_dsp::DspError::OutOfRange {
                param: sdrkit_algo_dsp::Param::Rate,
                min: 8000.0,
                max: f64::INFINITY,
                got: rate,
            })?;
        Ok(FaxDemod {
            dc: RealDcBlocker::new(rate, 20.0),
            nco: Nco::new(rate, -CENTRE_HZ),
            narrow: [Fir::new(&taps(a)?), Fir::new(&taps(b)?), Fir::new(&taps(c)?)],
            wide: Fir::new(&taps(1400.0)?),
            disc_n: [disc.clone(), disc.clone(), disc.clone()],
            disc_w: disc,
            last_n: [CENTRE_HZ; 3],
            last_w: CENTRE_HZ,
            lvl: 0.0,
            moments,
        })
    }
    /// The filters' delay, samples.
    pub fn delay(&self) -> usize {
        (self.wide.len() - 1) / 2
    }
    /// The SNR in 3 kHz over the last seconds, dB: the estimate in the wide filter's 2.8 kHz, moved to 3 kHz.
    pub fn snr_db(&self) -> Option<f64> {
        let r = self.moments.estimate()?.ratio();
        (r.is_finite() && r > 0.0).then(|| 10.0 * (r * 2800.0 / SNR_BANDWIDTH_HZ).log10())
    }
    /// Takes one audio sample. Returns the narrow frequencies (in the order of [`NARROW_CUTS_HZ`]) and the wide frequency.
    pub fn push(&mut self, x: f32) -> ([f64; 3], f64) {
        let mut v = [if x.is_finite() { x } else { 0.0 }];
        self.dc.process(&mut v);
        let m = self.nco.next_sample() * v[0];
        let aw = self.wide.process_sample(m);
        self.moments.push(aw);
        let mut an = [Complex32::new(0.0, 0.0); 3];
        for (a, f) in an.iter_mut().zip(self.narrow.iter_mut()) {
            *a = f.process_sample(m);
        }
        // The level and the fade are judged on the widest narrow filter.
        let mag = an[2].norm();
        self.lvl += 0.002 * (mag - self.lvl);
        let fade = mag < 0.1 * self.lvl;
        let fw = f64::from(self.disc_w.process_sample(aw)) + CENTRE_HZ;
        for k in 0..3 {
            let f = f64::from(self.disc_n[k].process_sample(an[k])) + CENTRE_HZ;
            if !fade {
                self.last_n[k] = f;
            }
        }
        if !fade {
            self.last_w = fw;
        }
        (self.last_n, self.last_w)
    }
}

/// A tone the detector fired on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tone {
    Start576,
    Start288,
    Stop,
}

/// An event of the tone detector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToneEvent {
    pub tone: Tone,
    pub hz: f64,
    /// Audio sample index where the run of the tone began.
    pub at: u64,
    /// The tone's power over the audio power (the SNR measure of tones).
    pub ratio: f64,
}

/// The APT tone detector: windows of 0.1 s (hop 0.05 s), mean removed, Hann. 61 Goertzel bins from 200 to 800 Hz.
#[derive(Clone, Debug)]
pub struct ToneDetector {
    rate: f64,
    win: usize,
    hop: usize,
    buf: VecDeque<f32>,
    hann: Vec<f32>,
    scratch: Vec<f32>,
    since: usize,
    count: u64,
    run: Option<(Tone, f64, usize, u64)>,
    fired: bool,
    quiet: usize,
    /// The last tone frequency measured.
    pub last_hz: f64,
    /// The last tone power over the audio power.
    pub last_ratio: f64,
}

impl ToneDetector {
    /// For audio at `rate`.
    pub fn new(rate: f64) -> Self {
        let win = ((0.1 * rate).round() as usize).max(2);
        ToneDetector {
            rate,
            win,
            hop: (0.05 * rate).round() as usize,
            buf: VecDeque::with_capacity(win + 1),
            hann: Window::Hann.coefficients(win),
            scratch: Vec::with_capacity(win),
            since: 0,
            count: 0,
            run: None,
            fired: false,
            quiet: 0,
            last_hz: 0.0,
            last_ratio: 0.0,
        }
    }
    /// One audio sample. Returns a tone event when one fires.
    pub fn push(&mut self, x: f32) -> Option<ToneEvent> {
        self.buf.push_back(x);
        if self.buf.len() > self.win {
            self.buf.pop_front();
        }
        self.count += 1;
        self.since += 1;
        if self.buf.len() < self.win || self.since < self.hop {
            return None;
        }
        self.since = 0;
        let mean = self.buf.iter().sum::<f32>() / self.win as f32;
        self.scratch.clear();
        self.scratch.extend(self.buf.iter().map(|v| v - mean));
        let total: f32 = self
            .scratch
            .iter()
            .zip(&self.hann)
            .map(|(v, h)| (v * h) * (v * h))
            .sum();
        let mut mags = [0.0f64; 61];
        for (b, m) in mags.iter_mut().enumerate() {
            let f = (200.0 + 10.0 * b as f64) / self.rate;
            *m = goertzel_windowed(&self.scratch, &self.hann, f).norm();
        }
        let (mut bk, mut peak) = (0, mags[0]);
        for (i, &m) in mags.iter().enumerate() {
            if m.total_cmp(&peak).is_ge() {
                (bk, peak) = (i, m);
            }
        }
        let mut d = 0.0;
        if bk > 0 && bk < 60 {
            let (a, b, c) = (
                mags[bk - 1].max(1e-12).ln(),
                peak.max(1e-12).ln(),
                mags[bk + 1].max(1e-12).ln(),
            );
            let den = a - 2.0 * b + c;
            if den.abs() > 1e-12 {
                d = (0.5 * (a - c) / den).clamp(-0.5, 0.5);
            }
        }
        // The bins are 10 Hz apart. The window's resolution is 10 Hz too (0.1 s), so d is in bins.
        let hz = 200.0 + 10.0 * (bk as f64 + d);
        let sinc = if d.abs() < 1e-9 {
            1.0
        } else {
            (PI * d).sin() / (PI * d) / (1.0 - d * d)
        };
        let amp = peak / sinc.max(0.3);
        // Tone power: amplitude^2 x (window energy) / 2 against the windowed audio power (Hann: sum h^2 = 3N/8, sum h = N/2).
        let tone_p =
            2.0 * amp * amp / (self.win as f64 / 2.0).powi(2) * (3.0 * self.win as f64 / 8.0) / 2.0;
        let ratio = tone_p / f64::from(total).max(1e-20);
        self.last_ratio = ratio;
        let kind = if ratio >= 0.35 {
            if (hz - 300.0).abs() <= 55.0 {
                Some(Tone::Start576)
            } else if (hz - 675.0).abs() <= 55.0 {
                Some(Tone::Start288)
            } else if (hz - 450.0).abs() <= 55.0 {
                Some(Tone::Stop)
            } else {
                None
            }
        } else {
            None
        };
        let Some(kind) = kind else {
            self.quiet += 1;
            if self.quiet >= 3 {
                self.fired = false;
                self.run = None;
            }
            return None;
        };
        self.quiet = 0;
        self.last_hz = hz;
        let run = match self.run {
            Some((k, f0, n, at)) if k == kind && (hz - f0).abs() <= 8.0 => (k, f0, n + 1, at),
            _ => (kind, hz, 1, self.count.saturating_sub(self.win as u64)),
        };
        self.run = Some(run);
        let (k, f0, n, at) = run;
        let need = if k == Tone::Stop { 8 } else { 14 };
        if n >= need && !self.fired {
            self.fired = true;
            return Some(ToneEvent {
                tone: k,
                hz: f0.max(hz.min(f0 + 8.0)),
                at,
                ratio,
            });
        }
        None
    }
}

/// Running sums of the frequency, so any fractional interval's mean is two look-ups. Sample n covers [n - 0.5, n + 0.5).
#[derive(Clone, Debug)]
pub struct Sums {
    first: u64,
    sums: VecDeque<f64>,
    cap: usize,
    acc: f64,
}

impl Sums {
    /// Keeping `cap` samples.
    pub fn new(cap: usize) -> Self {
        Sums {
            first: 0,
            sums: VecDeque::with_capacity(cap + 1),
            cap,
            acc: 0.0,
        }
    }
    /// The next sample.
    pub fn push(&mut self, v: f64) {
        self.acc += v;
        self.sums.push_back(self.acc);
        if self.sums.len() > self.cap {
            self.sums.pop_front();
            self.first += 1;
        }
    }
    /// The index of the next sample to come.
    pub fn end(&self) -> u64 {
        self.first + self.sums.len() as u64
    }
    /// The oldest index kept.
    pub fn first(&self) -> u64 {
        self.first
    }
    /// The integral up to position `t` (in samples, sample n covers [n - 0.5, n + 0.5)).
    fn integral(&self, t: f64) -> Option<f64> {
        let u = t + 0.5;
        let i = u.floor();
        let idx = i as i64 - self.first as i64;
        if idx < 1 || idx as usize >= self.sums.len() {
            return None;
        }
        let k = idx as usize;
        let below = self.sums[k - 1];
        let this = self.sums[k] - self.sums[k - 1];
        Some(below + this * (u - i))
    }
    /// The mean over [a, b).
    pub fn mean(&self, a: f64, b: f64) -> Option<f64> {
        if b <= a {
            return None;
        }
        Some((self.integral(b)? - self.integral(a)?) / (b - a))
    }
}

/// The states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FaxState {
    #[default]
    Waiting = 0,
    Phasing = 1,
    Receiving = 2,
    Stopped = 3,
}

impl FaxState {
    /// The name shown.
    pub fn name(self) -> &'static str {
        match self {
            FaxState::Waiting => "waiting for start tone",
            FaxState::Phasing => "phasing",
            FaxState::Receiving => "receiving",
            FaxState::Stopped => "stopped",
        }
    }
}

/// The clock errors the slant fit expects, ppm (one standard deviation).
const SLANT_PRIOR_PPM: f64 = 50.0;

/// Rows shorter than this are not filtered.
const ROW_FILTER_MIN_PIXELS: usize = 16;
/// The weight of a new row in the averaged noise.
const ROW_NOISE_WEIGHT: f64 = 0.1;

/// The row filter of the fax picture: the line Wiener filter over 7 pixels on the tones of a row (Hz), with the pixel noise
/// measured on the row itself as `sigma = 1.4826 median |f[j-1] - 2 f[j] + f[j+1]| / sqrt(6)` (the edges of a chart hardly
/// move the median); `sigma^2` is averaged over rows (the first sets it, then a weight of 0.1).
#[derive(Clone, Debug, Default)]
struct RowFilter {
    noise: Option<f64>,
    scratch: Vec<f32>,
    out: Vec<f32>,
}

impl RowFilter {
    /// Forgets the measured noise (a new picture).
    fn reset(&mut self) {
        self.noise = None;
    }

    /// Measures the noise of `tones` and filters them in place; rows under 16 pixels are left as they are.
    fn filter(&mut self, tones: &mut [f32]) {
        let n = tones.len();
        if n < ROW_FILTER_MIN_PIXELS {
            return;
        }
        self.scratch.clear();
        self.scratch
            .extend(tones.windows(3).map(|t| (t[0] - 2.0 * t[1] + t[2]).abs()));
        let k = self.scratch.len() / 2;
        let med = f64::from(*self.scratch.select_nth_unstable_by(k, f32::total_cmp).1);
        let sigma = sdrkit_algo_dsp::denoise::MAD_TO_SIGMA * med / 6f64.sqrt();
        let v = sigma * sigma;
        let nv = match self.noise {
            None => v,
            Some(old) => old + ROW_NOISE_WEIGHT * (v - old),
        };
        self.noise = Some(nv);
        self.out.resize(n, 0.0);
        if sdrkit_algo_dsp::denoise::line_wiener(
            tones,
            nv as f32,
            sdrkit_algo_dsp::denoise::LINE_WIENER_HALF,
            &mut self.out,
        )
        .is_ok()
        {
            tones.copy_from_slice(&self.out);
        }
    }
}

/// Settings of the fax decoder.
#[derive(Clone, Debug, PartialEq)]
pub struct FaxSettings {
    /// 0 detects.
    pub lpm: u32,
    /// 0 from the start tone.
    pub ioc: u32,
    pub auto_slant: bool,
    pub slant_ppm: f64,
    pub max_lines: usize,
    pub rotate: i32,
    /// The row filter below the strong-signal SNR.
    pub row_filter: bool,
}

impl Default for FaxSettings {
    fn default() -> Self {
        FaxSettings {
            lpm: 0,
            ioc: 0,
            auto_slant: true,
            slant_ppm: 0.0,
            max_lines: 8000,
            rotate: 0,
            row_filter: true,
        }
    }
}

/// What the decoder reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FaxStatus {
    pub state: u8,
    pub state_name: &'static str,
    pub ioc: u32,
    pub lpm: u32,
    pub lines: usize,
    pub width: usize,
    pub slant_ppm: f64,
    pub last_tone_hz: f64,
    pub black_level_hz: f64,
    pub carrier_offset_hz: f64,
    pub phasing_lines: usize,
    pub snr_db: Option<f64>,
    pub change: u64,
    pub preview_rows: usize,
    pub pictures_started: u64,
    pub pictures_done: u64,
    pub end_reason: String,
}

/// The fax decoder on real audio.
pub struct FaxDecoder {
    rate: f64,
    demod: FaxDemod,
    tones: ToneDetector,
    /// The running sums of the narrowest filter's frequency.
    narrow: Sums,
    /// The running sums of the other narrow filters' frequencies, in the order of [`NARROW_CUTS_HZ`] after the first.
    narrower: [Sums; 2],
    /// The narrow filter of the picture being received (an index of [`NARROW_CUTS_HZ`]).
    picture_narrow: usize,
    /// The narrow filter that the phasing and the levels read now, from the SNR of the last seconds.
    phasing_narrow: usize,
    /// The running sums of the square of the widest narrow filter's frequency (the SNR of the phasing black is read there).
    narrow_sq: Sums,
    wide: Sums,
    n: u64,
    pub settings: FaxSettings,
    state: FaxState,
    ioc: u32,
    lpm: u32,
    mistune: f64,
    period: f64,
    nominal: f64,
    line_start: f64,
    image: Option<LineImage>,
    width: usize,
    change: u64,
    phasing_lines: usize,
    pulses: Vec<f64>,
    pending: Vec<(f64, f64)>,
    smooth: VecDeque<f64>,
    smooth_sum: f64,
    up: Option<f64>,
    above: bool,
    missing: usize,
    next_expected: f64,
    fit_slant_ppm: f64,
    applied_period: f64,
    snr_db: Option<f64>,
    black_meas: Vec<f64>,
    tone_ratio: f64,
    started: u64,
    done: u64,
    end_reason: String,
    join_seen: Option<(u32, u64)>,
    join_check: u64,
    picture: Option<(u64, u64, Picture)>,
    /// The number of the page: the number of its picture items.
    page: u32,
    /// A growing state of the page went out and no finished state yet.
    page_open: bool,
    /// The page ended. Its picture is finished.
    page_closed: bool,
    /// The finished pictures of pages that ended, until the component takes them.
    ended: Vec<Picture>,
    /// The next row to build.
    next_row: usize,
    roll: usize,
    /// The tones of the row being built (Hz).
    row_tones: Vec<f32>,
    row_filter: RowFilter,
}

impl FaxDecoder {
    /// A decoder for audio at `rate`. An error for a rate below 8 kHz.
    pub fn new(rate: f64) -> Result<Self, sdrkit_algo_dsp::DspError> {
        let hist = (13.0 * rate) as usize;
        Ok(FaxDecoder {
            rate,
            demod: FaxDemod::new(rate)?,
            tones: ToneDetector::new(rate),
            narrow: Sums::new(hist),
            narrower: [Sums::new(hist), Sums::new(hist)],
            picture_narrow: 0,
            phasing_narrow: 0,
            narrow_sq: Sums::new(hist),
            wide: Sums::new(hist),
            n: 0,
            settings: FaxSettings::default(),
            state: FaxState::Waiting,
            ioc: 576,
            lpm: 120,
            mistune: 0.0,
            period: rate * 0.5,
            nominal: rate * 0.5,
            line_start: 0.0,
            image: None,
            width: width_for_ioc(576),
            change: 0,
            phasing_lines: 0,
            pulses: Vec::new(),
            pending: Vec::new(),
            smooth: VecDeque::new(),
            smooth_sum: 0.0,
            up: None,
            above: false,
            missing: 0,
            next_expected: 0.0,
            fit_slant_ppm: 0.0,
            applied_period: rate * 0.5,
            snr_db: None,
            black_meas: Vec::new(),
            tone_ratio: 0.0,
            started: 0,
            done: 0,
            end_reason: String::new(),
            join_seen: None,
            join_check: 0,
            picture: None,
            page: 0,
            page_open: false,
            page_closed: false,
            ended: Vec::new(),
            next_row: 0,
            roll: 0,
            row_tones: Vec::new(),
            row_filter: RowFilter::default(),
        })
    }

    /// The state.
    pub fn state(&self) -> FaxState {
        self.state
    }
    /// The picture so far, before rotation (`None` before the first picture).
    pub fn image(&self) -> Option<&LineImage> {
        self.image.as_ref()
    }
    /// The rows received.
    pub fn lines(&self) -> usize {
        self.image.as_ref().map_or(0, |i| i.height() as usize)
    }
    /// The rows so far, top first, one after the other (`width` bytes each).
    pub fn pixels(&self) -> Vec<u8> {
        let mut v = Vec::new();
        if let Some(i) = &self.image {
            i.copy_to(&mut v);
        }
        v
    }
    /// The width in use.
    pub fn width(&self) -> usize {
        self.width
    }
    /// The change counter.
    pub fn change(&self) -> u64 {
        self.change
    }
    /// Moves the change counter (the picture is rebuilt at the next look).
    pub fn bump_change(&mut self) {
        self.change += 1;
    }

    fn black(&self) -> f64 {
        BLACK_HZ + self.mistune
    }
    fn white(&self) -> f64 {
        WHITE_HZ + self.mistune
    }
    fn mid(&self) -> f64 {
        (self.black() + self.white()) / 2.0
    }

    /// The mean frequency over an interval of input time (the filters' delay added).
    fn fmean(&self, a: f64, b: f64) -> Option<f64> {
        let d = self.demod.delay() as f64;
        self.narrow_sums(self.phasing_narrow).mean(a + d, b + d)
    }

    /// The running sums of narrow filter `k` (an index of [`NARROW_CUTS_HZ`]).
    fn narrow_sums(&self, k: usize) -> &Sums {
        match k {
            0 => &self.narrow,
            k => &self.narrower[k - 1],
        }
    }

    fn line_period_for(&self, lpm: u32) -> f64 {
        self.rate * 60.0 / f64::from(lpm)
    }

    /// Clears the picture and the decoder's lines. Phasing and the line count restart. The change counter moves.
    pub fn clear(&mut self) {
        self.end_page();
        self.clear_rows();
        self.state = FaxState::Waiting;
        self.phasing_lines = 0;
        self.pulses.clear();
        self.change += 1;
        self.picture = None;
    }

    /// Resets everything (a retune): state 0, no lines.
    pub fn reset(&mut self) {
        let s = self.settings.clone();
        let c = self.change;
        if let Ok(mut d) = FaxDecoder::new(self.rate) {
            d.carry_pictures(self);
            *self = d;
        }
        self.settings = s;
        self.change = c + 1;
    }

    fn start(&mut self, ioc: u32, at: f64) {
        if self.state == FaxState::Receiving && self.lines() > 0 {
            self.finish("new start");
        }
        self.state = FaxState::Phasing;
        self.ioc = if self.settings.ioc != 0 {
            self.settings.ioc
        } else {
            ioc
        };
        self.lpm = if self.settings.lpm != 0 {
            self.settings.lpm
        } else {
            120
        };
        self.width = width_for_ioc(self.ioc);
        self.nominal = self.line_period_for(self.lpm);
        self.period = self.nominal;
        self.pulses.clear();
        self.pending.clear();
        self.phasing_lines = 0;
        self.missing = 0;
        self.page += 1;
        (self.page_open, self.page_closed) = (false, false);
        self.new_image();
        self.next_expected = at;
        self.black_meas.clear();
        self.roll = 0;
        self.started += 1;
        self.change += 1;
    }

    fn finish(&mut self, reason: &str) {
        if self.lines() > 0 {
            self.done += 1;
        }
        self.end_reason = reason.into();
        self.end_page();
    }

    /// Ends the picture of the page: if a state of it went out or it has lines, its finished state waits in the
    /// list that [`FaxDecoder::take_ended`] gives.
    pub fn end_page(&mut self) {
        if self.page_closed || !(self.page_open || self.lines() > 0) {
            return;
        }
        if let Some(p) = self.draw(PictureState::Finished) {
            self.ended.push(p);
        }
        (self.page_open, self.page_closed) = (false, true);
    }

    /// The finished pictures of the pages that ended after the last call.
    pub fn take_ended(&mut self) -> Vec<Picture> {
        std::mem::take(&mut self.ended)
    }

    /// Takes the page number and the finished pictures of `old`, the decoder that this one replaces, and ends the
    /// page of `old`. The numbers of the pages go on.
    pub fn carry_pictures(&mut self, old: &mut FaxDecoder) {
        old.end_page();
        self.page = old.page;
        self.ended = old.take_ended();
    }

    /// The page as a picture item in the state `state`, turned by the rotation setting.
    fn draw(&self, state: PictureState) -> Option<Picture> {
        let img = self.image.as_ref()?;
        let r = geom::rotate(&img.to_raster(), self.settings.rotate);
        Some(r.to_picture("fax", self.page.max(1), state))
    }

    fn effective_period(&self) -> f64 {
        let auto = if self.settings.auto_slant {
            self.fit_slant_ppm
        } else {
            0.0
        };
        self.nominal * (1.0 + (auto + self.settings.slant_ppm) * 1e-6)
    }

    /// Feeds audio samples.
    pub fn process(&mut self, audio: &[f32]) {
        for &x in audio {
            let (narrow, fwide) = self.demod.push(x);
            if self.n.is_multiple_of(256) {
                self.phasing_narrow = narrow_for(self.demod.snr_db());
            }
            let fnar = narrow[self.phasing_narrow];
            self.narrow.push(narrow[0]);
            self.narrower[0].push(narrow[1]);
            self.narrower[1].push(narrow[2]);
            self.narrow_sq.push(narrow[2] * narrow[2]);
            self.wide.push(fwide);
            self.n += 1;
            // The 4 ms smoothing for the pulse detector (narrow filter).
            let m = ((0.004 * self.rate) as usize).max(1);
            self.smooth.push_back(fnar);
            self.smooth_sum += fnar;
            if self.smooth.len() > m {
                if let Some(old) = self.smooth.pop_front() {
                    self.smooth_sum -= old;
                }
            }
            if let Some(ev) = self.tones.push(x) {
                self.tone_ratio = ev.ratio;
                self.on_tone(ev);
            }
            let delay = self.demod.delay() as f64 + m as f64 / 2.0;
            let t = self.n as f64 - 1.0 - delay;
            let s = self.smooth_sum / self.smooth.len() as f64;
            match self.state {
                FaxState::Phasing => self.phasing_sample(t, s),
                FaxState::Waiting => {
                    self.phasing_sample(t, s);
                    self.join_tick();
                }
                _ => {}
            }
            if self.state == FaxState::Receiving {
                self.build_rows();
            }
        }
    }

    fn on_tone(&mut self, ev: ToneEvent) {
        match ev.tone {
            Tone::Start576 | Tone::Start288 => {
                let nominal = if ev.tone == Tone::Start576 {
                    300.0
                } else {
                    675.0
                };
                self.mistune = ((ev.hz - nominal) * 4.0).clamp(-80.0, 80.0);
                self.mistune = (ev.hz - nominal).clamp(-80.0, 80.0);
                let ioc = if ev.tone == Tone::Start576 { 576 } else { 288 };
                self.start(ioc, self.n as f64);
            }
            Tone::Stop => {
                if self.state == FaxState::Receiving {
                    self.cut_tone_rows();
                    self.finish("stop tone");
                    self.state = FaxState::Stopped;
                    self.change += 1;
                } else if self.state == FaxState::Phasing {
                    self.state = FaxState::Stopped;
                }
            }
        }
    }

    /// Of the last 6 rows, the first that spends at least 3 of its 20 parts more than 150 Hz below the black level ends the picture.
    fn cut_tone_rows(&mut self) {
        let n = self.lines();
        let black = self.black();
        for r in n.saturating_sub(6)..n {
            let start = self.row_start(r);
            let p = self.effective_period();
            let mut low = 0;
            for k in 0..20 {
                if let Some(m) = self.fmean(
                    start + p * k as f64 / 20.0,
                    start + p * (k + 1) as f64 / 20.0,
                ) {
                    if m < black - 150.0 {
                        low += 1;
                    }
                }
            }
            if low >= 3 {
                self.truncate(r);
                break;
            }
        }
        self.next_row = self.lines();
    }

    fn row_start(&self, r: usize) -> f64 {
        self.line_start + self.effective_period() * r as f64 + self.demod.delay() as f64 * 0.0
    }

    /// The pulse detector during phasing (and while waiting, to join late in the phasing).
    fn phasing_sample(&mut self, t: f64, s: f64) {
        let mid = self.mid();
        let above = s > mid;
        if above && !self.above {
            self.up = Some(t);
        } else if !above && self.above {
            if let Some(u) = self.up.take() {
                self.pending.push((u, t));
            }
        }
        self.above = above;
        // A pulse is judged once the samples just outside it have arrived.
        let p_nom = if self.state == FaxState::Phasing {
            self.nominal
        } else {
            self.line_period_for(120)
        };
        while let Some(&(u, d)) = self.pending.first() {
            if (u + d) / 2.0 + 0.07 * p_nom > t {
                break;
            }
            self.pending.remove(0);
            self.pulse(u, d);
        }
        if self.state != FaxState::Phasing {
            return;
        }
        // The line after the last pulse: a pulse is expected at last + period.
        if let Some(&last) = self.pulses.last() {
            let exp = last + self.period;
            if t > exp + 0.12 * self.period && self.next_expected < exp {
                self.next_expected = exp;
                // Whether the line after the expected boundary is the picture (white or content, not black).
                let after = self.fmean(exp + 0.03 * self.period, exp + 0.12 * self.period);
                let white_like = after.is_some_and(|m| m > mid);
                if white_like {
                    self.begin_picture(exp);
                } else {
                    self.missing += 1;
                    if self.missing >= 3 {
                        let first_missing = exp - 2.0 * self.period;
                        self.begin_picture(first_missing);
                    } else {
                        // Pretend a pulse to keep the chain going.
                        self.pulses.push(exp);
                    }
                }
            }
        }
    }

    /// The centre of a phasing pulse of `width` samples near `c`: the place of the largest mean frequency over a window of the
    /// pulse's width, within half a width of `c`, to a fraction of a sample. The mean over the whole pulse is far less noisy than
    /// the two crossings of the mid level.
    fn pulse_centre(&self, c: f64, width: f64) -> f64 {
        let reach = (0.5 * width).round() as i64;
        let at = |s: i64| self.fmean(c + s as f64 - 0.5 * width, c + s as f64 + 0.5 * width);
        let mut best: Option<(i64, f64)> = None;
        for s in -reach..=reach {
            if let Some(m) = at(s) {
                if best.is_none_or(|(_, b)| m > b) {
                    best = Some((s, m));
                }
            }
        }
        let Some((s, m)) = best else {
            return c;
        };
        let d = match (at(s - 1), at(s + 1)) {
            (Some(a), Some(b)) if a + b - 2.0 * m < 0.0 => {
                (0.5 * (a - b) / (a + b - 2.0 * m)).clamp(-0.5, 0.5)
            }
            _ => 0.0,
        };
        c + s as f64 + d
    }

    fn pulse(&mut self, up: f64, down: f64) {
        let p_nom = if self.state == FaxState::Phasing {
            self.nominal
        } else {
            self.line_period_for(120)
        };
        let width = down - up;
        let want = 0.05 * p_nom;
        if width < 0.6 * want || width > 1.6 * want {
            return;
        }
        let c = (up + down) / 2.0;
        // Contrast: just outside the pulse (3.5 to 6 % of the line either side) the line must be black again.
        let black_lim = self.mid() - 0.25 * (self.white() - self.black());
        let l = self.fmean(c - 0.06 * p_nom, c - 0.035 * p_nom);
        let r = self.fmean(c + 0.035 * p_nom, c + 0.06 * p_nom);
        match (l, r) {
            (Some(a), Some(b)) if a < black_lim && b < black_lim => {}
            _ => return,
        }
        let c = self.pulse_centre(c, want);
        if self.state == FaxState::Waiting {
            // Joining late in the phasing: a chain of equally spaced pulses.
            self.pulses.push(c);
            if self.pulses.len() > 8 {
                self.pulses.remove(0);
            }
            let n = self.pulses.len();
            if n >= 4 {
                let d: Vec<f64> = self.pulses.windows(2).map(|w| w[1] - w[0]).collect();
                let tail = &d[d.len() - 3..];
                for lpm in [60u32, 90, 100, 120, 180, 240] {
                    if self.settings.lpm != 0 && self.settings.lpm != lpm {
                        continue;
                    }
                    let p = self.line_period_for(lpm);
                    if tail.iter().all(|x| (x - p).abs() < 0.01 * p) {
                        let keep: Vec<f64> = self.pulses[n - 4..].to_vec();
                        let ioc = if self.settings.ioc != 0 {
                            self.settings.ioc
                        } else {
                            576
                        };
                        self.start(ioc, c);
                        self.lpm = lpm;
                        self.nominal = p;
                        self.period = p;
                        self.pulses = keep;
                        self.phasing_lines = 3;
                        self.fit();
                        return;
                    }
                }
            }
            return;
        }
        // Phasing: part of the chain when it lies about one period after the last.
        if let Some(&last) = self.pulses.last() {
            let k = ((c - last) / self.period).round();
            if k < 1.0 || ((c - last) - k * self.period).abs() > 0.02 * self.period {
                if self.pulses.len() < 2 {
                    self.pulses.clear();
                    self.pulses.push(c);
                }
                return;
            }
        }
        self.pulses.push(c);
        self.missing = 0;
        self.phasing_lines += 1;
        // The black of the phasing line before (88 % of it), for the mistuning without a start tone and for the SNR.
        if self.pulses.len() >= 2 {
            let a = self.pulses[self.pulses.len() - 2];
            let p = c - a;
            let d = self.demod.delay() as f64;
            if let (Some(m), Some(mw), Some(m2)) = (
                self.fmean(a + 0.06 * p, c - 0.06 * p),
                self.narrower[1].mean(a + 0.06 * p + d, c - 0.06 * p + d),
                self.mean_sq(a + 0.06 * p, c - 0.06 * p),
            ) {
                self.black_meas.push(m);
                let var = (m2 - mw * mw).max(1e-6);
                let snr = 20.0 * (400.0 / var.sqrt()).log10();
                self.snr_db = Some(self.snr_db.map_or(snr, |s| s + 0.3 * (snr - s)));
            }
        }
        self.fit();
    }

    /// The mean square of the widest narrow filter's frequency over an interval of input time (the filters' delay added): with
    /// its mean, the variance of the samples, so the SNR of the phasing black reads the noise of one sample.
    fn mean_sq(&self, a: f64, b: f64) -> Option<f64> {
        let d = self.demod.delay() as f64;
        self.narrow_sq.mean(a + d, b + d)
    }

    fn fit(&mut self) {
        let n = self.pulses.len();
        if n < 2 {
            return;
        }
        // Least squares through the pulse times against their line numbers.
        let p0 = self.pulses[0];
        let idx: Vec<f64> = self
            .pulses
            .iter()
            .map(|t| ((t - p0) / self.nominal).round())
            .collect();
        let mi = idx.iter().sum::<f64>() / n as f64;
        let mt = self.pulses.iter().sum::<f64>() / n as f64;
        let (mut sxy, mut sxx) = (0.0, 0.0);
        for (i, t) in idx.iter().zip(&self.pulses) {
            sxy += (i - mi) * (t - mt);
            sxx += (i - mi) * (i - mi);
        }
        if sxx > 0.0 && n >= 4 {
            let slope = sxy / sxx;
            let ppm = (slope / self.nominal - 1.0) * 1e6;
            if ppm.abs() < 1000.0 {
                // The slope is as uncertain as the pulse times scatter about the line. Against clock errors of about
                // `SLANT_PRIOR_PPM`, the most likely slant is the fitted one times the share of the prior in the two variances:
                // the fit itself at a strong signal, near 0 when the pulses of a weak one say little.
                let scatter: f64 = idx
                    .iter()
                    .zip(&self.pulses)
                    .map(|(i, t)| (t - mt - slope * (i - mi)).powi(2))
                    .sum::<f64>()
                    / (n - 2) as f64;
                let sigma_ppm = (scatter / sxx).sqrt() / self.nominal * 1e6;
                let share = SLANT_PRIOR_PPM.powi(2) / (SLANT_PRIOR_PPM.powi(2) + sigma_ppm.powi(2));
                self.fit_slant_ppm = share * ppm;
                self.period = self.nominal * (1.0 + self.fit_slant_ppm * 1e-6);
            }
        }
        // The line start: the line through the mean pulse, at the first pulse.
        let slope = self.period;
        self.line_start = mt - slope * mi + slope * idx[0];
    }

    fn begin_picture(&mut self, at: f64) {
        self.state = FaxState::Receiving;
        // Levels from the phasing's black when no start tone measured a mistuning.
        if self.mistune == 0.0 && !self.black_meas.is_empty() {
            let mut v = self.black_meas.clone();
            v.sort_by(|a, b| a.total_cmp(b));
            let m = v[v.len() / 2];
            // The black of a phasing line includes the pulse edges' 12 %: about 1500 Hz when tuned.
            self.mistune = (m - BLACK_HZ).clamp(-80.0, 80.0);
        }
        self.line_start = at;
        self.picture_narrow = narrow_for(self.demod.snr_db());
        self.applied_period = self.effective_period();
        self.clear_rows();
        self.change += 1;
    }

    fn build_rows(&mut self) {
        let p = self.effective_period();
        if (p - self.applied_period).abs() > 1e-9 {
            // Straighten the rows already received: row r shifts by r x dP / samples per pixel.
            let dp = p - self.applied_period;
            let spp = p / self.width as f64;
            if let Some(img) = &mut self.image {
                img.deslant(dp / spp);
            }
            self.applied_period = p;
            self.change += 1;
        }
        let delay = self.demod.delay() as f64;
        loop {
            if self.lines() >= self.settings.max_lines.max(1) {
                self.state = FaxState::Stopped;
                self.finish("timeout");
                return;
            }
            let r = self.next_row;
            let start = self.line_start + p * r as f64;
            let end = start + p;
            if end + delay + 2.0 > self.n as f64 {
                return;
            }
            let use_wide = self.snr_db.is_some_and(|s| s > 22.0);
            let sums = match (use_wide, self.picture_narrow) {
                (true, _) => &self.wide,
                (false, 0) => &self.narrow,
                (false, k) => &self.narrower[k - 1],
            };
            let (black, white) = (self.black(), self.white());
            let w = self.width;
            let Some(img) = &mut self.image else {
                return;
            };
            let Ok(row) = img.next_row() else {
                self.state = FaxState::Stopped;
                self.finish("timeout");
                return;
            };
            // The tone of each pixel (Hz); a pixel with no samples reads white.
            let tones = &mut self.row_tones;
            tones.clear();
            tones.extend((0..w).map(|x| {
                let a = start + p * x as f64 / w as f64 + delay;
                let b = start + p * (x + 1) as f64 / w as f64 + delay;
                sums.mean(a, b).unwrap_or(white) as f32
            }));
            if self.settings.row_filter && !use_wide {
                self.row_filter.filter(tones);
            }
            for (px, &m) in row.iter_mut().zip(tones.iter()) {
                *px = ((f64::from(m) - black) / (white - black) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            if self.roll > 0 {
                row.rotate_left(self.roll % w);
            }
            self.next_row += 1;
            self.change += 1;
        }
    }

    /// Joining mid-picture: a picture repeats almost exactly one line later. It reads the widest narrow filter: through a
    /// narrower one the lines of a weak phasing signal repeat as well as a picture's.
    fn join_tick(&mut self) {
        if self.n < self.join_check + (0.5 * self.rate) as u64 {
            return;
        }
        self.join_check = self.n;
        let end = self.n as f64 - self.demod.delay() as f64 - 2.0;
        let mut found = None;
        let lpms: Vec<u32> = if self.settings.lpm != 0 {
            vec![self.settings.lpm]
        } else {
            vec![240, 180, 120, 100, 90, 60]
        };
        for lpm in lpms {
            let p = self.line_period_for(lpm);
            if end - 2.0 * p < self.narrower[1].first() as f64 + 2.0 {
                continue;
            }
            let n = 256;
            let seg = |off: f64| -> Option<Vec<f64>> {
                (0..n)
                    .map(|k| {
                        self.narrower[1].mean(
                            end - 2.0 * p + off + p * k as f64 / n as f64,
                            end - 2.0 * p + off + p * (k + 1) as f64 / n as f64,
                        )
                    })
                    .collect()
            };
            let (Some(a), Some(b), Some(c)) = (seg(0.0), seg(p), seg(0.37 * p)) else {
                continue;
            };
            let corr = pearson(&a, &b);
            let anti = pearson(&a, &c);
            let mean = a.iter().sum::<f64>() / n as f64;
            let sd = (a.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64).sqrt();
            let inside = mean > BLACK_HZ - 100.0 && mean < WHITE_HZ + 100.0;
            if corr > 0.9 && anti < 0.6 && sd > 60.0 && inside {
                found = Some(lpm);
            }
        }
        match (found, self.join_seen) {
            (Some(l), Some((l0, t0))) if l == l0 && self.n - t0 >= (0.5 * self.rate) as u64 => {
                // Seen twice: start receiving. White is the commonest frequency (a chart is mostly white).
                let ioc = if self.settings.ioc != 0 {
                    self.settings.ioc
                } else {
                    576
                };
                self.start(ioc, end);
                self.lpm = l;
                self.nominal = self.line_period_for(l);
                self.period = self.nominal;
                let p = self.nominal;
                let mut hist = [0usize; 64];
                for k in 0..512 {
                    if let Some(m) = self.narrower[1].mean(
                        end - p + p * k as f64 / 512.0,
                        end - p + p * (k + 1) as f64 / 512.0,
                    ) {
                        let b = (((m - 1300.0) / 20.0).floor()).clamp(0.0, 63.0) as usize;
                        hist[b] += 1;
                    }
                }
                let mut bk = 0;
                for (i, &h) in hist.iter().enumerate() {
                    if h >= hist[bk] {
                        bk = i;
                    }
                }
                let white = 1300.0 + 20.0 * bk as f64 + 10.0;
                self.mistune = (white - WHITE_HZ).clamp(-80.0, 80.0);
                self.begin_picture(end - p);
                self.join_seen = None;
            }
            (Some(l), _) => {
                if self.join_seen.is_none_or(|s| s.0 != l) {
                    self.join_seen = Some((l, self.n));
                }
            }
            (None, _) => self.join_seen = None,
        }
    }

    /// Rolls the picture so the chart's frame is at the edges: a steady white band flanked by dark columns, about 1 % of the
    /// width each (nothing moves when no such band stands out).
    pub fn roll_to_frame(&mut self) {
        let Some(img) = &mut self.image else {
            return;
        };
        if img.height() < 8 {
            return;
        }
        let w = self.width;
        let mut col = vec![0f64; w];
        for i in 0..img.height() {
            if let Some(r) = img.row(i) {
                for (x, v) in r.iter().enumerate() {
                    col[x] += f64::from(*v);
                }
            }
        }
        let n = img.height() as f64;
        for c in &mut col {
            *c /= n;
        }
        let band = (w / 100).max(2);
        let mut best = (0usize, 0.0f64);
        for x in 0..w {
            let wsum: f64 = (0..band).map(|k| col[(x + k) % w]).sum::<f64>() / band as f64;
            let l: f64 = (1..=band).map(|k| col[(x + w - k) % w]).sum::<f64>() / band as f64;
            let r: f64 = (0..band).map(|k| col[(x + band + k) % w]).sum::<f64>() / band as f64;
            let score = wsum - l.max(r);
            if wsum > 200.0 && score > best.1 {
                best = (x, score);
            }
        }
        if best.1 > 60.0 {
            let shift = (best.0 + band / 2) % w;
            img.roll(shift);
            self.roll = (self.roll + shift) % w;
            self.change += 1;
        }
    }

    /// The picture so far as the standard picture item (raw grey, `L008`, 0 black to 255 white), turned by the rotation setting,
    /// with the number that changes whenever it does. `None` before the first picture. The picture grows until the
    /// page ends. A page that ended comes again, finished, only when a setting draws it again.
    pub fn picture(&mut self) -> Option<(u64, Picture)> {
        let img = self.image.as_ref()?;
        if self.page_closed && self.lines() == 0 {
            return None;
        }
        let key = (img.version(), self.change);
        if let Some((v, c, p)) = &self.picture {
            if (*v, *c) == key {
                return Some((self.change, p.clone()));
            }
        }
        let state = if self.page_closed {
            PictureState::Finished
        } else {
            PictureState::Growing
        };
        let p = self.draw(state)?;
        self.page_open = !self.page_closed;
        self.picture = Some((key.0, key.1, p.clone()));
        Some((self.change, p))
    }

    fn new_image(&mut self) {
        let max = self.settings.max_lines.clamp(1, MAX_LINES) as u32;
        let same = self
            .image
            .as_ref()
            .is_some_and(|i| i.width() as usize == self.width && i.max_rows() == max);
        if same {
            if let Some(i) = &mut self.image {
                i.clear();
            }
        } else {
            self.image = LineImage::with_capacity(
                PixelFormat::Grey8,
                self.width as u32,
                max,
                Overflow::Stop,
            )
            .ok();
        }
        self.next_row = 0;
        self.row_filter.reset();
    }

    fn clear_rows(&mut self) {
        if let Some(i) = &mut self.image {
            i.clear();
        }
        self.next_row = 0;
        self.row_filter.reset();
    }

    /// Keeps the first `n` rows.
    fn truncate(&mut self, n: usize) {
        let Some(img) = &mut self.image else {
            return;
        };
        let mut keep = Vec::new();
        for i in 0..n.min(img.height() as usize) as u32 {
            if let Some(r) = img.row(i) {
                keep.extend_from_slice(r);
            }
        }
        img.clear();
        for r in keep.chunks_exact(self.width.max(1)) {
            let _ = img.push_row(r);
        }
    }

    /// The status.
    pub fn status(&self) -> FaxStatus {
        let black = if self.state == FaxState::Waiting && self.mistune == 0.0 {
            BLACK_HZ
        } else {
            self.black()
        };
        let snr = match self.state {
            FaxState::Phasing | FaxState::Receiving => self.snr_db,
            _ => (self.tone_ratio > 0.0)
                .then(|| 10.0 * (self.tone_ratio / (1.0 - self.tone_ratio).max(1e-6)).log10()),
        };
        FaxStatus {
            state: self.state as u8,
            state_name: self.state.name(),
            ioc: self.ioc,
            lpm: self.lpm,
            lines: self.lines(),
            width: self.width,
            slant_ppm: if self.settings.auto_slant {
                self.fit_slant_ppm
            } else {
                0.0
            } + self.settings.slant_ppm,
            last_tone_hz: self.tones.last_hz,
            black_level_hz: black,
            carrier_offset_hz: black - BLACK_HZ,
            phasing_lines: self.phasing_lines,
            snr_db: snr,
            change: self.change,
            preview_rows: self.lines() * 256 / self.width.max(1),
            pictures_started: self.started,
            pictures_done: self.done,
            end_reason: self.end_reason.clone(),
        }
    }
}

/// Pearson correlation.
pub fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    if n < 2.0 {
        return 0.0;
    }
    let ma = a.iter().sum::<f64>() / n;
    let mb = b.iter().sum::<f64>() / n;
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma).powi(2);
        sbb += (y - mb).powi(2);
    }
    if saa <= 0.0 || sbb <= 0.0 {
        0.0
    } else {
        sab / (saa * sbb).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_and_rates() {
        assert_eq!(width_for_ioc(576), 1809);
        assert_eq!(width_for_ioc(288), 904);
        assert!(FaxDecoder::new(4000.0).is_err());
        assert!(UsbAudio::new(f64::NAN).is_err());
    }

    #[test]
    fn sums_give_fractional_means() {
        let mut s = Sums::new(100);
        for v in 0..50 {
            s.push(f64::from(v));
        }
        assert!((s.mean(10.0, 20.0).unwrap() - 15.0).abs() < 1e-9);
        assert!((s.mean(9.5, 10.5).unwrap() - 10.0).abs() < 1e-9);
    }
}
