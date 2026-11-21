//! The encoder analysis of one 10 ms frame: the windowed DFT, the non-linear pitch estimator, the pitch refinement, the
//! harmonic amplitudes and the voicing decision.
//!
//! Standard: D. Rowe, "Codec 2 Algorithm Description" (2025), sections 3.2, 3.4 and 3.5.

use crate::consts::{cosf, log10f, trunc_i, FFT_SIZE, MAX_AMP, M_PITCH, N_SAMP, PI, TWO_PI};
use sdrkit_algo_dsp::fft_exact::ExactFft;
use sdrkit_algo_dsp::Complex32;

/// The sinusoidal model of one 10 ms frame.
#[derive(Clone, Debug)]
pub(crate) struct Model {
    /// The fundamental in radians per sample.
    pub wo: f32,
    /// The number of harmonics.
    pub l: usize,
    /// The harmonic amplitudes, index 1 to `l`.
    pub a: [f32; MAX_AMP + 1],
    /// The voicing decision.
    pub voiced: bool,
}

impl Model {
    pub fn new() -> Self {
        Model {
            wo: 0.0,
            l: 0,
            a: [0.0; MAX_AMP + 1],
            voiced: false,
        }
    }
}

/// Half the analysis window length (the window spans 2 NW_HALF + 1 samples around the centre).
const NW_HALF: usize = 139;
/// The centre of the analysis buffer.
const CENTRE: usize = M_PITCH / 2;
/// The NLP low-pass filter length.
pub(crate) const NLP_TAPS: usize = 48;
/// The NLP decimation factor.
const DEC: usize = 5;
/// The NLP decimated length.
const NLP_DEC_LEN: usize = M_PITCH / DEC;
/// The lowest and highest NLP search bins (50 Hz and 400 Hz).
const NLP_BIN_MIN: usize = FFT_SIZE * DEC / 160;
const NLP_BIN_MAX: usize = FFT_SIZE * DEC / 20;

/// The complex forward FFT of 512 points. The size is a valid constant.
pub(crate) fn fft512() -> ExactFft {
    ExactFft::new(FFT_SIZE, false).expect("512 is a valid FFT size")
}

/// The non-linear pitch estimator.
struct Nlp {
    sq: [f32; M_PITCH],
    mem_x: f32,
    mem_y: f32,
    mem_fir: [f32; NLP_TAPS],
    h: [f32; NLP_TAPS],
    win: [f32; NLP_DEC_LEN],
    prev_f0: f32,
    power: Box<[f32]>,
}

impl Nlp {
    fn new(h: [f32; NLP_TAPS]) -> Self {
        let mut win = [0.0f32; NLP_DEC_LEN];
        for (i, w) in win.iter_mut().enumerate() {
            let c = cosf((2.0 * PI * i as f64 / (NLP_DEC_LEN - 1) as f64) as f32);
            *w = (0.5 - 0.5 * c as f64) as f32;
        }
        Nlp {
            sq: [0.0; M_PITCH],
            mem_x: 0.0,
            mem_y: 0.0,
            mem_fir: [0.0; NLP_TAPS],
            h,
            win,
            prev_f0: (1.0 / 0.02f64) as f32,
            power: vec![0.0; FFT_SIZE].into_boxed_slice(),
        }
    }

    /// Returns the pitch period in samples for the buffer `sn`.
    fn run(&mut self, sn: &[f32; M_PITCH], fft: &mut ExactFft, buf: &mut [Complex32]) -> f32 {
        let start = M_PITCH - N_SAMP;
        for i in start..M_PITCH {
            self.sq[i] = sn[i] * sn[i];
        }
        for i in start..M_PITCH {
            let x = self.sq[i];
            let y = x - self.mem_x;
            let y = (y as f64 + 0.95 * self.mem_y as f64) as f32;
            self.mem_x = x;
            self.mem_y = y;
            self.sq[i] = (y as f64 + 1.0) as f32;
        }
        for i in start..M_PITCH {
            self.mem_fir.copy_within(1.., 0);
            self.mem_fir[NLP_TAPS - 1] = self.sq[i];
            let mut acc = 0.0f32;
            for j in 0..NLP_TAPS {
                acc += self.mem_fir[j] * self.h[j];
            }
            self.sq[i] = acc;
        }
        buf.fill(Complex32::new(0.0, 0.0));
        for i in 0..NLP_DEC_LEN {
            buf[i].re = self.sq[i * DEC] * self.win[i];
        }
        // The buffer has the FFT size, so the transform cannot fail.
        let _ = fft.process(buf);
        for (p, c) in self.power.iter_mut().zip(buf.iter()) {
            *p = c.re * c.re + c.im * c.im;
        }
        let f = &self.power;

        let mut gmax = 0.0f32;
        let mut gmax_bin = NLP_BIN_MIN;
        for (k, &v) in f.iter().enumerate().take(NLP_BIN_MAX + 1).skip(NLP_BIN_MIN) {
            if v > gmax {
                gmax = v;
                gmax_bin = k;
            }
        }

        let mut cmax_bin = gmax_bin;
        let prev_bin = (self.prev_f0 * (FFT_SIZE * DEC) as f32 / 8000.0) as i32;
        let mut mult = 2;
        while gmax_bin / mult >= NLP_BIN_MIN {
            let b = gmax_bin / mult;
            let bmin = trunc_i(0.8 * b as f64).max(NLP_BIN_MIN as i32) as usize;
            let bmax = trunc_i(1.2 * b as f64) as usize;
            let thresh = if (bmin as i32) < prev_bin && prev_bin < bmax as i32 {
                (0.3 * 0.5 * gmax as f64) as f32
            } else {
                (0.3 * gmax as f64) as f32
            };
            let mut lmax = 0.0f32;
            let mut lmax_bin = bmin;
            for (k, &v) in f.iter().enumerate().take(bmax + 1).skip(bmin) {
                if v > lmax {
                    lmax = v;
                    lmax_bin = k;
                }
            }
            if lmax > thresh && lmax > f[lmax_bin - 1] && lmax > f[lmax_bin + 1] {
                cmax_bin = lmax_bin;
            }
            mult += 1;
        }
        let best_f0 = cmax_bin as f32 * 8000.0 / (FFT_SIZE * DEC) as f32;
        self.sq.copy_within(N_SAMP.., 0);
        self.prev_f0 = best_f0;
        8000.0 / best_f0
    }
}

/// The encoder analysis state.
pub(crate) struct Analyser {
    sn: [f32; M_PITCH],
    w: [f32; M_PITCH],
    wspec: Box<[f32]>,
    sw: Box<[Complex32]>,
    buf: Box<[Complex32]>,
    fft: ExactFft,
    nlp: Nlp,
}

impl Analyser {
    /// A new state. `h` holds the 48 taps of the pitch estimator filter.
    pub fn new(h: [f32; NLP_TAPS]) -> Self {
        let mut fft = fft512();
        let mut w = [0.0f32; M_PITCH];
        let lo = CENTRE - NW_HALF;
        let hi = CENTRE + NW_HALF;
        let denom = (hi - lo) as f64;
        for i in lo..hi {
            let j = (i - lo) as f64;
            let c = cosf((TWO_PI * j / denom) as f32);
            w[i] = (0.5 - 0.5 * c as f64) as f32;
        }
        let mut s = 0.0f32;
        for v in &w {
            s += v * v;
        }
        let g = (1.0 / ((s * FFT_SIZE as f32) as f64).sqrt()) as f32;
        for v in &mut w {
            *v *= g;
        }

        let mut buf = vec![Complex32::new(0.0, 0.0); FFT_SIZE].into_boxed_slice();
        for n in 0..NW_HALF {
            buf[n].re = w[CENTRE + n];
            buf[FFT_SIZE - NW_HALF + n].re = w[lo + n];
        }
        let _ = fft.process(&mut buf);
        let mut wspec = vec![0.0f32; FFT_SIZE].into_boxed_slice();
        let half = FFT_SIZE / 2;
        for i in 0..half {
            wspec[i] = buf[i + half].re;
            wspec[i + half] = buf[i].re;
        }

        Analyser {
            sn: [1.0; M_PITCH],
            w,
            wspec,
            sw: vec![Complex32::new(0.0, 0.0); FFT_SIZE].into_boxed_slice(),
            buf,
            fft,
            nlp: Nlp::new(h),
        }
    }

    /// The analysis buffer after the last call to [`Analyser::analyse`].
    pub fn sn(&self) -> &[f32; M_PITCH] {
        &self.sn
    }

    /// The analysis window.
    pub fn window(&self) -> &[f32; M_PITCH] {
        &self.w
    }

    /// Adds 80 new samples and analyses the frame. `pcm` holds exactly [`N_SAMP`] samples.
    pub fn analyse(&mut self, pcm: &[i16], model: &mut Model) {
        self.sn.copy_within(N_SAMP.., 0);
        for (d, &s) in self.sn[M_PITCH - N_SAMP..].iter_mut().zip(pcm) {
            *d = s as f32;
        }

        self.sw.fill(Complex32::new(0.0, 0.0));
        let lo = CENTRE - NW_HALF;
        for n in 0..NW_HALF {
            self.sw[n].re = self.sn[CENTRE + n] * self.w[CENTRE + n];
            self.sw[FFT_SIZE - NW_HALF + n].re = self.sn[lo + n] * self.w[lo + n];
        }
        let _ = self.fft.process(&mut self.sw);

        let pitch = self.nlp.run(&self.sn, &mut self.fft, &mut self.buf);
        let mut wo = (TWO_PI / pitch as f64) as f32;
        wo = self.refine(wo, 5.0, 1.0);
        wo = self.refine(wo, 1.0, 0.25);
        if (wo as f64) < TWO_PI / 160.0 {
            wo = (TWO_PI / 160.0) as f32;
        }
        let hi = (TWO_PI / 20.0) as f32;
        if wo > hi {
            wo = hi;
        }
        let mut l = ((PI / wo as f64) as f32).floor() as usize;
        if (wo * l as f32) as f64 >= 0.95 * PI {
            l -= 1;
        }
        model.wo = wo;
        model.l = l;
        self.amplitudes(model);
        model.voiced = self.voicing(model);
    }

    /// One pass of the harmonic-sum pitch refinement around the period `2 pi / wo`, `half` periods each side in steps of `step`.
    fn refine(&self, wo: f32, half: f64, step: f32) -> f32 {
        let l = trunc_i(PI / wo as f64).max(0) as usize;
        let r = (TWO_PI / FFT_SIZE as f64) as f32;
        let q = (1.0 / r as f64) as f32;
        let mut wom = wo;
        let mut em = 0.0f32;
        let pmin = (TWO_PI / wo as f64 - half) as f32;
        let pmax = (TWO_PI / wo as f64 + half) as f32;
        let mut p = pmin;
        while p <= pmax {
            let w = (TWO_PI / p as f64) as f32;
            let bstep = w * q;
            let mut acc = bstep;
            let mut e = 0.0f32;
            for _ in 1..=l {
                let b = trunc_i(acc as f64 + 0.5) as usize;
                if let Some(c) = self.sw.get(b) {
                    e += c.re * c.re + c.im * c.im;
                }
                acc += bstep;
            }
            if e > em {
                em = e;
                wom = w;
            }
            p += step;
        }
        wom
    }

    fn amplitudes(&self, model: &mut Model) {
        let r = (TWO_PI / FFT_SIZE as f64) as f32;
        let q = (1.0 / r as f64) as f32 as f64;
        let wo = model.wo as f64;
        for m in 1..=model.l {
            let am = trunc_i((m as f64 - 0.5) * wo * q + 0.5) as usize;
            let bm = trunc_i((m as f64 + 0.5) * wo * q + 0.5) as usize;
            let mut den = 0.0f32;
            for c in &self.sw[am.min(FFT_SIZE)..bm.min(FFT_SIZE)] {
                den += c.re * c.re + c.im * c.im;
            }
            model.a[m] = den.sqrt();
        }
        for a in &mut model.a[model.l + 1..] {
            *a = 0.0;
        }
    }

    fn voicing(&self, model: &Model) -> bool {
        let l = model.l;
        let wo = model.wo;
        let a = &model.a;
        let l1000 = trunc_i(l as f64 * 1000.0 / 4000.0) as usize;
        let mut sig = 1e-4f64 as f32;
        for v in &a[1..=l1000] {
            sig += v * v;
        }
        let mut error = 1e-4f64 as f32;
        for li in 1..=l1000 {
            let lf = li as f64;
            let al = (((lf - 0.5) * wo as f64 * 512.0 / TWO_PI) as f32).ceil() as usize;
            let bl = (((lf + 0.5) * wo as f64 * 512.0 / TWO_PI) as f32).ceil() as usize;
            let lw = li as f32 * wo * 512.0;
            let offset = trunc_i(256.0 - lw as f64 / TWO_PI + 0.5);
            let (mut nre, mut nim, mut den) = (0.0f32, 0.0f32, 0.0f32);
            for k in al..bl {
                let wv = self.wv(offset, k);
                nre += self.sw[k].re * wv;
                nim += self.sw[k].im * wv;
                den += wv * wv;
            }
            let (bre, bim) = (nre / den, nim / den);
            for k in al..bl {
                let wv = self.wv(offset, k);
                let er = self.sw[k].re - bre * wv;
                let ei = self.sw[k].im - bim * wv;
                error += er * er;
                error += ei * ei;
            }
        }
        let snr = (10.0 * log10f(sig / error) as f64) as f32;
        let mut voiced = snr > 6.0;

        let l2000 = trunc_i(l as f64 * 2000.0 / 4000.0) as usize;
        let l4000 = trunc_i(l as f64 * 4000.0 / 4000.0) as usize;
        let mut elow = 1e-4f64 as f32;
        for v in &a[1..=l2000] {
            elow += v * v;
        }
        let mut ehigh = 1e-4f64 as f32;
        for v in &a[l2000..=l4000] {
            ehigh += v * v;
        }
        let eratio = (10.0 * log10f(elow / ehigh) as f64) as f32;
        if !voiced && eratio > 10.0 {
            voiced = true;
        }
        if voiced && eratio < -10.0 {
            voiced = false;
        }
        if voiced && eratio < -4.0 && wo <= (60.0 * TWO_PI / 8000.0) as f32 {
            voiced = false;
        }
        voiced
    }

    /// The window spectrum at `offset + k`, or 0 outside the table.
    #[inline]
    fn wv(&self, offset: i32, k: usize) -> f32 {
        let i = offset + k as i32;
        if (0..FFT_SIZE as i32).contains(&i) {
            self.wspec[i as usize]
        } else {
            0.0
        }
    }
}
