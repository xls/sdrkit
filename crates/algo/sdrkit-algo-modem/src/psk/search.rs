//! Coarse carrier searches before the loops track: a sum of power spectra, the strongest band of a wide capture, and the carrier
//! by the raised-cosine template or the M-th power line.

use crate::freq::{BandSearch, PowerLine, TemplateFit};
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
use sdrkit_algo_dsp::window::Window;

/// The sum of Hann-window power spectra of consecutive frames of `n` samples, lowest frequency first.
pub struct SpectrumSum {
    n: usize,
    welch: Welch,
    frame: Vec<Complex32>,
    pow: Vec<f32>,
    acc: Vec<f32>,
    frames: usize,
}

impl SpectrumSum {
    /// An empty sum of `n`-point frames. Errors unless `n` is 2 to 2^20.
    pub fn new(n: usize) -> Result<Self, ModemError> {
        Ok(SpectrumSum {
            n,
            welch: Welch::new(n, Window::Hann, 0.0, Scaling::Tone)?,
            frame: Vec::with_capacity(n),
            pow: vec![0.0; n],
            acc: vec![0.0; n],
            frames: 0,
        })
    }

    /// Points of a frame.
    pub fn len(&self) -> usize {
        self.n
    }

    /// True when no frame has been summed.
    pub fn is_empty(&self) -> bool {
        self.frames == 0
    }

    /// Frames summed.
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// The sum, lowest frequency first (DC at `n / 2`).
    pub fn sum(&self) -> &[f32] {
        &self.acc
    }

    /// Adds the spectra of the whole frames `x` completes.
    pub fn push(&mut self, x: &[Complex32]) {
        self.push_until(x, usize::MAX);
    }

    /// Adds frames from `x` until `frames` frames are summed. Returns the samples of `x` used up to and with the one that completed
    /// the last frame, or `None` when `x` ran out first.
    pub fn push_until(&mut self, x: &[Complex32], frames: usize) -> Option<usize> {
        for (i, &v) in x.iter().enumerate() {
            if self.frames >= frames {
                return Some(i);
            }
            self.frame.push(v);
            if self.frame.len() == self.n {
                if self.welch.compute(&self.frame, &mut self.pow).is_ok() {
                    for (a, p) in self.acc.iter_mut().zip(self.pow.iter()) {
                        *a += *p;
                    }
                    self.frames += 1;
                }
                self.frame.clear();
                if self.frames >= frames {
                    return Some(i + 1);
                }
            }
        }
        None
    }

    /// Starts the sum again.
    pub fn reset(&mut self) {
        self.acc.fill(0.0);
        self.frames = 0;
        self.frame.clear();
    }
}

/// The band locator of a wide capture: an averaged power spectrum searched for the strongest band as wide as the signal, each
/// bin counted at most 4 times the median. Places that never locked can be skipped.
pub struct BandLocator {
    rate: f64,
    symbol_rate: f64,
    want: usize,
    sum: SpectrumSum,
    search: BandSearch,
    tried: Vec<f64>,
    done: bool,
}

/// Places the band locator skips at most (the newest kept).
const LOCATOR_SKIPS: usize = 8;

impl BandLocator {
    /// A locator for input at `rate` and a signal of `symbol_rate` with roll-off `roll_off`. The bins are at most an eighth of
    /// the symbol rate. The average covers about 10 000 bins of the signal's width, and at most a quarter of a second. Errors for
    /// values that are not positive or a signal wider than the capture.
    pub fn new(rate: f64, symbol_rate: f64, roll_off: f64) -> Result<Self, ModemError> {
        Param::Rate.check_positive(rate)?;
        Param::SymbolRate.check_positive(symbol_rate)?;
        Param::RollOff.check_in(roll_off, 0.0, f64::INFINITY)?;
        let mut n = 256usize;
        while rate / (n as f64) > symbol_rate / 8.0 && n < 65536 {
            n *= 2;
        }
        let bin = rate / n as f64;
        let max_blocks = ((0.25 * rate) / n as f64).floor().max(4.0) as usize;
        let width = symbol_rate * (1.0 + roll_off);
        let bins = (width / bin).max(1.0);
        let want = ((10_000.0 / bins).ceil() as usize).clamp(4, max_blocks);
        Ok(BandLocator {
            rate,
            symbol_rate,
            want,
            sum: SpectrumSum::new(n)?,
            search: BandSearch::new(n, width.max(bin), bin)?,
            tried: Vec::with_capacity(LOCATOR_SKIPS),
            done: false,
        })
    }

    /// Starts again, skipping `failed_hz` (Hz from the centre) and the last places skipped before. A value of 0 is not skipped.
    pub fn restart(&mut self, failed_hz: f64) {
        if failed_hz != 0.0 {
            if self.tried.len() == LOCATOR_SKIPS {
                self.tried.remove(0);
            }
            self.tried.push(failed_hz);
        }
        self.search.clear_skips();
        for &t in &self.tried {
            self.search.skip(t, 0.3 * self.symbol_rate);
        }
        self.done = false;
        self.sum.reset();
    }

    /// Feeds samples. Once, when the average is complete, returns the samples used up to its last one and the band's centre (Hz
    /// from the centre of the capture). The centre is `None` when the band lies within 0.15 of the symbol rate of the centre.
    pub fn push(&mut self, x: &[Complex32]) -> Option<(usize, Option<f64>)> {
        if self.done {
            return None;
        }
        let used = self.sum.push_until(x, self.want)?;
        self.done = true;
        let half = self.rate / 2.0;
        let c = self
            .search
            .find(self.sum.sum(), -half, half)
            .map(|b| b.centre_hz)
            .filter(|c| c.abs() > 0.15 * self.symbol_rate);
        Some((used, c))
    }
}

/// How a carrier search looks for the carrier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SearchKind {
    /// Correlate 48 averaged 1024-point spectra with the raised-cosine shape of the link, within plus and minus `range_hz`.
    Template {
        /// The search range, Hz.
        range_hz: f64,
    },
    /// The line of the signal raised to `power` (2 for BPSK, 4 for QPSK) in 4096-point spectra, tried after 8, 16, 32 and 64 of
    /// them. The line must stand `min_db` over the median, or 14.8, 12.6, 10.8 and 7.8 dB at those counts when `None`.
    PowerLine {
        /// 2 or 4.
        power: u32,
        /// The least height of the line, dB.
        min_db: Option<f64>,
    },
}

/// Everything a [`CarrierSearch`] needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchConfig {
    /// The search.
    pub kind: SearchKind,
    /// The sample rate, Hz.
    pub rate: f64,
    /// The symbol rate, Hz.
    pub symbol_rate: f64,
    /// The root raised cosine roll-off.
    pub roll_off: f64,
}

/// What a carrier search ended with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SearchOutcome {
    /// The carrier, Hz from the centre.
    Found(f64),
    /// No line stood high enough after the longest average.
    NotFound,
}

/// Points of the template search's spectra.
const TEMPLATE_FFT: usize = 1024;
/// Spectra the template search averages.
const TEMPLATE_FRAMES: usize = 48;
/// Points of the power-line search's spectra.
const LINE_FFT: usize = 4096;

enum Searcher {
    Template {
        sum: Box<SpectrumSum>,
        fit: TemplateFit,
        range: f64,
    },
    Line {
        line: Box<PowerLine>,
        buf: Vec<Complex32>,
        min_db: Option<f64>,
    },
}

/// A coarse carrier search over a stream of samples.
pub struct CarrierSearch {
    rate: f64,
    s: Searcher,
}

impl CarrierSearch {
    /// A search for `cfg` that rejects lines within a few bins of each frequency in `rejected_hz`. Errors for values out of range.
    pub fn new(cfg: SearchConfig, rejected_hz: &[f64]) -> Result<Self, ModemError> {
        let rate = cfg.rate;
        Param::Rate.check_positive(rate)?;
        Param::SymbolRate.check_positive(cfg.symbol_rate)?;
        let s = match cfg.kind {
            SearchKind::Template { range_hz } => Searcher::Template {
                sum: Box::new(SpectrumSum::new(TEMPLATE_FFT)?),
                fit: TemplateFit::new(
                    rate / TEMPLATE_FFT as f64,
                    cfg.symbol_rate,
                    cfg.roll_off.clamp(0.01, 1.0),
                    TEMPLATE_FFT,
                    range_hz,
                )?,
                range: range_hz,
            },
            SearchKind::PowerLine { power, min_db } => {
                let mut line = PowerLine::new(power, LINE_FFT)?;
                let bin = 1.0 / LINE_FFT as f64 / power as f64;
                let width = (3.0 * bin).max(0.002 * cfg.symbol_rate / rate);
                let mut list = [0.0f64; 8];
                let k = rejected_hz.len().min(8);
                for (l, f) in list.iter_mut().zip(rejected_hz) {
                    *l = f / rate;
                }
                line.set_reject(&list[..k], width)?;
                Searcher::Line {
                    line: Box::new(line),
                    buf: Vec::with_capacity(64 * LINE_FFT),
                    min_db,
                }
            }
        };
        Ok(CarrierSearch { rate, s })
    }

    /// Feeds samples. Returns the samples used, and the outcome once the search ends (the samples after it are not used).
    ///
    /// # Errors
    ///
    /// [`ModemError`] if the fit or the line estimate refuses the averaged spectrum. The search then ends.
    pub fn push(&mut self, x: &[Complex32]) -> Result<(usize, Option<SearchOutcome>), ModemError> {
        match &mut self.s {
            Searcher::Template { sum, fit, range } => match sum.push_until(x, TEMPLATE_FRAMES) {
                None => Ok((x.len(), None)),
                Some(used) => {
                    let f = fit.fit(sum.sum(), *range)?;
                    Ok((used, Some(SearchOutcome::Found(f.freq_hz))))
                }
            },
            Searcher::Line { line, buf, min_db } => {
                for (i, &v) in x.iter().enumerate() {
                    buf.push(v);
                    let frames = buf.len() / LINE_FFT;
                    if buf.len() % LINE_FFT != 0 || !matches!(frames, 8 | 16 | 32 | 64) {
                        continue;
                    }
                    // A lower bar as more frames are averaged.
                    let need = min_db.unwrap_or(match frames {
                        8 => 14.8,
                        16 => 12.6,
                        32 => 10.8,
                        _ => 7.8,
                    });
                    let l = line.estimate(buf)?;
                    if l.snr_db as f64 >= need {
                        return Ok((i + 1, Some(SearchOutcome::Found(l.freq * self.rate))));
                    }
                    if frames == 64 {
                        return Ok((i + 1, Some(SearchOutcome::NotFound)));
                    }
                }
                Ok((x.len(), None))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::modulate::Linear;

    fn qpsk(n: usize, rs_sps: f64, f: f64, beta: f64) -> Vec<Complex32> {
        let mut m = Linear::new(sdrkit_algo_dsp::pulse::Shape::Rrc { beta }, 16.0, rs_sps).unwrap();
        let mut s = 3u32;
        let syms: Vec<Complex32> = (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                super::super::PskOrder::Qpsk.point((s >> 16) as usize)
            })
            .collect();
        let mut x = Vec::new();
        m.push(&syms, &mut x);
        for (i, v) in x.iter_mut().enumerate() {
            *v *= Complex32::from_polar(1.0, (std::f64::consts::TAU * f * i as f64) as f32);
        }
        x
    }

    #[test]
    fn template_finds_a_qpsk_carrier() {
        // 72 kBd at 288 kHz (4 samples a symbol), carrier at +3 kHz.
        let rate = 288e3;
        let x = qpsk(20_000, 4.0, 3000.0 / rate, 0.6);
        let cfg = SearchConfig {
            kind: SearchKind::Template { range_hz: 15e3 },
            rate,
            symbol_rate: 72e3,
            roll_off: 0.6,
        };
        let mut s = CarrierSearch::new(cfg, &[]).unwrap();
        let (used, out) = s.push(&x).unwrap();
        assert_eq!(used, TEMPLATE_FFT * TEMPLATE_FRAMES);
        let Some(SearchOutcome::Found(f)) = out else {
            panic!("{out:?}")
        };
        assert!((f - 3000.0).abs() < 500.0, "{f}");
    }

    #[test]
    fn template_fits_a_signal_nearly_as_wide_as_the_capture() {
        // 2.8 MBd at 4 MHz, roll-off 0.5: the signal with its floor margin is wider than the capture. Carrier at +20 kHz.
        let rate = 4e6;
        let x = qpsk(40_000, rate / 2.8e6, 20e3 / rate, 0.5);
        let cfg = SearchConfig {
            kind: SearchKind::Template { range_hz: 75e3 },
            rate,
            symbol_rate: 2.8e6,
            roll_off: 0.5,
        };
        let mut s = CarrierSearch::new(cfg, &[]).unwrap();
        let (used, out) = s.push(&x).unwrap();
        assert_eq!(used, TEMPLATE_FFT * TEMPLATE_FRAMES);
        let Some(SearchOutcome::Found(f)) = out else {
            panic!("{out:?}")
        };
        assert!((f - 20e3).abs() < rate / TEMPLATE_FFT as f64, "{f}");
    }

    #[test]
    fn power_line_finds_a_qpsk_carrier() {
        let rate = 48e3;
        let x = qpsk(40_000, 8.0, 1234.0 / rate, 0.35);
        let cfg = SearchConfig {
            kind: SearchKind::PowerLine {
                power: 4,
                min_db: None,
            },
            rate,
            symbol_rate: 6e3,
            roll_off: 0.35,
        };
        let mut s = CarrierSearch::new(cfg, &[]).unwrap();
        let (used, out) = s.push(&x).unwrap();
        assert_eq!(used, 8 * LINE_FFT);
        let Some(SearchOutcome::Found(f)) = out else {
            panic!("{out:?}")
        };
        assert!((f - 1234.0).abs() < 5.0, "{f}");
    }

    #[test]
    fn locator_finds_the_band_and_skips_a_failed_place() {
        // A 10 kBd signal at +60 kHz in a 500 kHz capture with a little noise floor.
        let rate = 500e3;
        let mut x = qpsk(20_000, 50.0, 60e3 / rate, 0.35);
        let mut s = 9u32;
        for v in x.iter_mut() {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            *v += Complex32::new(((s >> 8) as f32 / 16_777_216.0 - 0.5) * 0.05, 0.0);
        }
        let mut l = BandLocator::new(rate, 10e3, 0.35).unwrap();
        let (_, c) = l.push(&x).unwrap();
        assert!((c.unwrap() - 60e3).abs() < 3e3, "{c:?}");
        assert!(l.push(&x).is_none());
        l.restart(60e3);
        let (_, c) = l.push(&x).unwrap();
        assert!(c.is_none_or(|c| (c - 60e3).abs() > 3e3));
    }
}
