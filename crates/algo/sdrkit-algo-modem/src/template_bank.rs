//! FFT correlation of one stored stream with several templates at once (overlap-save): each block of the stream is transformed
//! once, multiplied by the conjugate spectrum of every template and transformed back. A search over a set of codes (the 32
//! SYNC-DL codes of TD-SCDMA, the PSS of several cells) then costs one forward and `templates` inverse FFTs per block.
//!
//! The correlation at position `p` is `sum_k x[p + k] conj(t[k])` over the template's samples, for `p` in `0 ..= x.len() - L`
//! (`L` the longest template; shorter templates are zero-padded at their end).
//!
//! ```
//! use sdrkit_algo_modem::template_bank::TemplateBank;
//! use sdrkit_algo_modem::Complex32;
//! let a: Vec<Complex32> = [1.0, -1.0, 1.0, 1.0].iter().map(|&v| Complex32::new(v, 0.0)).collect();
//! let b: Vec<Complex32> = [1.0, 1.0, -1.0, 1.0].iter().map(|&v| Complex32::new(v, 0.0)).collect();
//! let mut x = vec![Complex32::default(); 100];
//! x[40..44].copy_from_slice(&b);
//! let mut bank = TemplateBank::new(&[a, b], 32).unwrap();
//! let mut best = (0, 0, 0.0f32);
//! bank.process(&x, |t, first, c| {
//!     for (i, v) in c.iter().enumerate() {
//!         if v.norm_sqr() > best.2 {
//!             best = (t, first + i, v.norm_sqr());
//!         }
//!     }
//! });
//! assert_eq!((best.0, best.1), (1, 40));
//! assert!((best.2 - 16.0).abs() < 1e-3);
//! ```

use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;

/// The correlator (module documentation). It keeps its plans and buffers.
pub struct TemplateBank {
    n: usize,
    l: usize,
    spectra: Vec<Vec<Complex32>>,
    fwd: Fft,
    inv: Fft,
    block: Vec<Complex32>,
    work: Vec<Complex32>,
}

impl TemplateBank {
    /// A bank of `templates` (at least one, each at least one sample) with FFTs of `fft_size` points (a power of two, at least
    /// twice the longest template).
    ///
    /// # Errors
    ///
    /// [`ModemError`] for no template, an empty template, or an FFT size that is not a power of two or under twice the longest
    /// template.
    pub fn new(templates: &[Vec<Complex32>], fft_size: usize) -> Result<Self, ModemError> {
        Param::Template.check_count(templates.len(), 1, 4096)?;
        let l = templates.iter().map(Vec::len).max().unwrap_or(0);
        Param::Template.check_count(l, 1, 1 << 20)?;
        if !fft_size.is_power_of_two() || fft_size < 2 * l {
            return Param::FftSize.conflict(Param::Template);
        }
        let mut fwd = Fft::forward(fft_size);
        let spectra = templates
            .iter()
            .map(|t| {
                let mut s = vec![Complex32::default(); fft_size];
                s[..t.len()].copy_from_slice(t);
                fwd.process(&mut s);
                s.iter_mut().for_each(|v| *v = v.conj() / fft_size as f32);
                s
            })
            .collect();
        Ok(TemplateBank {
            n: fft_size,
            l,
            spectra,
            fwd,
            inv: Fft::inverse(fft_size),
            block: vec![Complex32::default(); fft_size],
            work: vec![Complex32::default(); fft_size],
        })
    }

    /// The number of templates.
    pub fn len(&self) -> usize {
        self.spectra.len()
    }

    /// Always false: a bank holds at least one template.
    pub fn is_empty(&self) -> bool {
        self.spectra.is_empty()
    }

    /// Correlates `x` with every template. For each block and template, `sink(template, first, correlations)` gets the
    /// correlations at positions `first ..`, in order; every position `0 ..= x.len() - L` is given once per template.
    pub fn process(&mut self, x: &[Complex32], mut sink: impl FnMut(usize, usize, &[Complex32])) {
        if x.len() < self.l {
            return;
        }
        let positions = x.len() - self.l + 1;
        let hop = self.n - self.l + 1;
        let mut start = 0;
        while start < positions {
            let end = (start + self.n).min(x.len());
            self.block.fill(Complex32::default());
            self.block[..end - start].copy_from_slice(&x[start..end]);
            self.fwd.process(&mut self.block);
            let valid = hop.min(positions - start);
            for (t, spec) in self.spectra.iter().enumerate() {
                for ((w, b), s) in self.work.iter_mut().zip(&self.block).zip(spec) {
                    *w = b * s;
                }
                self.inv.process(&mut self.work);
                sink(t, start, &self.work[..valid]);
            }
            start += hop;
        }
    }
}
