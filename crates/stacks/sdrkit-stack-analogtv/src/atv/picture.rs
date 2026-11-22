//! The picture decoder: it decodes the rows that the colour lock orders ([`LineJob`]) into pictures: the luminance, and the
//! PAL, NTSC and SECAM chrominance by the burst loop and the line identification of each order. It counts the fields and
//! the pictures and measures the white peak and the SECAM chroma level.
//!
//! Standard: ITU-R BT.470-6 Table 2, ITU-R BT.1700.

use super::{Carrier, LineJob, PictureReport, RowJob};
use crate::settings::{
    BlackSetup, Deinterlace, NoiseReduction, BLACK_SETUP, CHROMA_DELAY, DEINTERLACE,
    NOISE_REDUCTION, SATURATION, TINT,
};
use crate::systems::{secam, Colour, Raster, R405, R525, R625, R819};
use crate::util::{cubic_row, Held, Interp8, DESIGN_BETA};
use sdrkit_algo_dsp::Sample;
use sdrkit_codec_image::colour::{rgb_from_differences, BT601, KU, KV};
use sdrkit_core::{
    Choice, Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Codec, Component, ComponentInfo, Format, Io, Kind, Picture, PixelFormat,
    PixelLayout, Port, RateFrom, Timed,
};
use std::f64::consts::TAU;
use std::sync::Arc;

/// The raster of a line count.
pub(super) fn raster_of(lines: u32) -> Raster {
    match lines {
        525 => R525,
        405 => R405,
        819 => R819,
        _ => R625,
    }
}

/// A centred ("same") convolution of `x` with symmetric `taps`, every `step`-th output, through a zero-padded copy.
fn conv_same(x: &[f32], taps: &[f32], pad: &mut Vec<f32>, out: &mut Vec<f32>, step: usize) {
    let h = taps.len() / 2;
    pad.clear();
    pad.resize(h, 0.0);
    pad.extend_from_slice(x);
    pad.resize(x.len() + 2 * h, 0.0);
    out.clear();
    let mut i = 0;
    while i < x.len() {
        out.push(<f32 as sdrkit_algo_dsp::Sample>::dot(
            taps,
            &pad[i..i + taps.len()],
        ));
        i += step;
    }
}

/// The same for complex samples.
fn conv_same_c(
    x: &[Complex32],
    taps: &[f32],
    pad: &mut Vec<Complex32>,
    out: &mut Vec<Complex32>,
    step: usize,
) {
    let h = taps.len() / 2;
    pad.clear();
    pad.resize(h, Complex32::default());
    pad.extend_from_slice(x);
    pad.resize(x.len() + 2 * h, Complex32::default());
    out.clear();
    let mut i = 0;
    while i < x.len() {
        out.push(<Complex32 as sdrkit_algo_dsp::Sample>::dot(
            taps,
            &pad[i..i + taps.len()],
        ));
        i += step;
    }
}

/// One row of pixels from the luminance `ys` and the (U, V) pairs `uv`, as B, G, R and 255: the BT.601 matrix of
/// [`rgb_from_differences`] with U and V scaled by `sat`, each value limited to 0 to 1 and rounded to 8 bits, halves away
/// from zero. Without `uv` the row is grey. Writes as many pixels as the shortest of the slices holds.
fn rgb_row_plain(ys: &[f32], uv: Option<&[[f32; 2]]>, sat: f32, out: &mut [[u8; 4]]) {
    let q = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    match uv {
        Some(uv) => {
            for ((px, &y), uv) in out.iter_mut().zip(ys).zip(uv) {
                let (u, v) = (uv[0] * sat, uv[1] * sat);
                let rgb = rgb_from_differences(BT601, y, u / KU, v / KV);
                *px = [q(rgb[2]), q(rgb[1]), q(rgb[0]), 255];
            }
        }
        None => {
            for (px, &y) in out.iter_mut().zip(ys) {
                let rgb = rgb_from_differences(BT601, y, 0.0 / KU, 0.0 / KV);
                *px = [q(rgb[2]), q(rgb[1]), q(rgb[0]), 255];
            }
        }
    }
}

/// [`rgb_row_plain`] with NEON, eight pixels at a step: the same operations in the same order on each value, so the
/// bytes are the same.
#[cfg(target_arch = "aarch64")]
fn rgb_row_neon(ys: &[f32], uv: Option<&[[f32; 2]]>, sat: f32, out: &mut [[u8; 4]]) {
    use std::arch::aarch64::*;
    let n = ys
        .len()
        .min(out.len())
        .min(uv.map_or(usize::MAX, <[_]>::len));
    let blocks = n / 8;
    let kg = 1.0 - BT601.kr - BT601.kb;
    // SAFETY: NEON is in every aarch64 target of Rust. Block `k` reads `ys[8k..8k + 8]` and `uv[8k..8k + 8]` and writes
    // `out[8k..8k + 8]`, all below `n`; a `[f32; 2]` is two `f32` and a `[u8; 4]` is four `u8` in a row.
    unsafe {
        let (vsat, vku, vkv) = (vdupq_n_f32(sat), vdupq_n_f32(KU), vdupq_n_f32(KV));
        let (vkr, vkb, vkg) = (
            vdupq_n_f32(BT601.kr),
            vdupq_n_f32(BT601.kb),
            vdupq_n_f32(kg),
        );
        let (zero, one, top) = (vdupq_n_f32(0.0), vdupq_n_f32(1.0), vdupq_n_f32(255.0));
        for k in 0..blocks {
            let mut bgr = [[vdup_n_u16(0); 2]; 3];
            for half in 0..2 {
                let at = 8 * k + 4 * half;
                let y = vld1q_f32(ys.as_ptr().add(at));
                let (u, v) = match uv {
                    Some(uv) => {
                        let t = vld2q_f32(uv.as_ptr().add(at).cast::<f32>());
                        (vmulq_f32(t.0, vsat), vmulq_f32(t.1, vsat))
                    }
                    None => (zero, zero),
                };
                let (b_y, r_y) = (vdivq_f32(u, vku), vdivq_f32(v, vkv));
                let r = vaddq_f32(y, r_y);
                let mix = vaddq_f32(vmulq_f32(vkr, r_y), vmulq_f32(vkb, b_y));
                let g = vsubq_f32(y, vdivq_f32(mix, vkg));
                let b = vaddq_f32(y, b_y);
                for (c, x) in [b, g, r].into_iter().enumerate() {
                    let x = vmulq_f32(vminq_f32(vmaxq_f32(x, zero), one), top);
                    // To the nearest, halves away from zero, as `f32::round`.
                    bgr[c][half] = vqmovn_u32(vcvtaq_u32_f32(x));
                }
            }
            let byte = |c: [uint16x4_t; 2]| vqmovn_u16(vcombine_u16(c[0], c[1]));
            let px = uint8x8x4_t(byte(bgr[0]), byte(bgr[1]), byte(bgr[2]), vdup_n_u8(255));
            vst4_u8(out.as_mut_ptr().add(8 * k).cast::<u8>(), px);
        }
    }
    let done = 8 * blocks;
    rgb_row_plain(
        &ys[done..n],
        uv.map(|u| &u[done..n]),
        sat,
        &mut out[done..n],
    );
}

/// One row of pixels: [`rgb_row_plain`], by its NEON form where there is one.
fn rgb_row(ys: &[f32], uv: Option<&[[f32; 2]]>, sat: f32, out: &mut [[u8; 4]]) {
    #[cfg(target_arch = "aarch64")]
    rgb_row_neon(ys, uv, sat, out);
    #[cfg(not(target_arch = "aarch64"))]
    rgb_row_plain(ys, uv, sat, out);
}

/// SECAM decoding state.
#[derive(Clone, Debug, Default)]
struct SecamState {
    level: f64,
    prev_row: Vec<f32>,
    prev_is_r: Option<bool>,
}

/// Settings of the picture decoder.
#[derive(Clone, Debug)]
struct Opts {
    saturation: f64,
    tint: f64,
    deinterlace: Deinterlace,
    nr: NoiseReduction,
    setup: BlackSetup,
    chroma_delay_ns: f64,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            saturation: SATURATION.default,
            tint: TINT.default,
            deinterlace: DEINTERLACE.default,
            nr: NOISE_REDUCTION.default,
            setup: BLACK_SETUP.default,
            chroma_delay_ns: CHROMA_DELAY.default,
        }
    }
}

sdrkit_core::settings! {
    /// The settings of the picture decoder.
    pub enum PictureSetting {
        /// Saturation.
        Saturation = SATURATION,
        /// Tint.
        Tint = TINT,
        /// Fields.
        Deinterlace = DEINTERLACE,
        /// Noise reduction.
        NoiseReduction = NOISE_REDUCTION,
        /// Black setup.
        BlackSetup = BLACK_SETUP,
        /// Chroma delay.
        ChromaDelay = CHROMA_DELAY,
    }
}

/// The picture decoder.
pub struct PictureDecoder {
    fv: f64,
    /// FM video (no chroma delay correction by default).
    pub fm: bool,
    o: Opts,
    video: Held<f32>,
    base: Held<Complex32>,
    interp: Interp8,
    /// The colour that the filters are designed for.
    designed: Option<Colour>,
    secam: SecamState,
    // Filters (by colour).
    notch: Vec<f32>,
    chroma_lp: Vec<f32>,
    secam_lp: Vec<f32>,
    bell: Vec<Complex32>,
    /// The noise power gain of the SECAM chain (low-pass and bell) and the bell noise's correlation at one sample.
    bell_ng: f64,
    bell_r: f64,
    /// Noise to carrier power after the bell, followed over the lines (negative: not measured yet).
    secam_rho: f64,
    // Scratch.
    seg_v: Vec<f32>,
    seg_b: Vec<Complex32>,
    seg_y: Vec<f32>,
    seg_c: Vec<Complex32>,
    seg_d: Vec<Complex32>,
    /// The chrominance of a PAL or NTSC row at the pixels.
    row_d: Vec<Complex32>,
    pad_r: Vec<f32>,
    pad_c: Vec<Complex32>,
    sec_z: Vec<Complex32>,
    sec_d: Vec<f32>,
    sec_cur: Vec<f32>,
    ys: Vec<f32>,
    row_uv: Vec<[f32; 2]>,
    prev_uv: Vec<[f32; 2]>,
    prev_uv_counter: u64,
    // Picture.
    raster: Raster,
    frame: Vec<u8>,
    prev_pic: Vec<f32>,
    rows_done: [u32; 2],
    fields_ok: u64,
    fields_bad: u64,
    pictures: u64,
    snow_pictures: u64,
    white_peak: f64,
    frame_peak: f64,
}

impl Default for PictureDecoder {
    fn default() -> Self {
        PictureDecoder {
            fv: 1.0,
            fm: false,
            o: Opts::default(),
            video: Held::new(1 << 15),
            base: Held::new(1 << 15),
            interp: Interp8::default(),
            designed: None,
            secam: SecamState::default(),
            notch: Vec::new(),
            chroma_lp: Vec::new(),
            secam_lp: Vec::new(),
            bell: Vec::new(),
            seg_v: Vec::new(),
            seg_b: Vec::new(),
            seg_y: Vec::new(),
            seg_c: Vec::new(),
            seg_d: Vec::new(),
            row_d: Vec::new(),
            pad_r: Vec::new(),
            pad_c: Vec::new(),
            sec_z: Vec::new(),
            sec_d: Vec::new(),
            sec_cur: Vec::new(),
            ys: Vec::new(),
            row_uv: Vec::new(),
            prev_uv: Vec::new(),
            prev_uv_counter: 0,
            raster: R625,
            frame: Vec::new(),
            prev_pic: Vec::new(),
            rows_done: [0; 2],
            fields_ok: 0,
            fields_bad: 0,
            pictures: 0,
            snow_pictures: 0,
            white_peak: 0.0,
            frame_peak: 0.0,
            bell_ng: 0.0,
            bell_r: 0.0,
            secam_rho: -1.0,
        }
    }
}

impl PictureDecoder {
    fn us(&self, x: f64) -> f64 {
        x * 1e-6 * self.fv
    }

    fn reset(&mut self) {
        self.secam_rho = -1.0;
        self.secam = SecamState::default();
        self.rows_done = [0; 2];
        self.prev_uv.clear();
        self.prev_pic.clear();
        self.frame.iter_mut().for_each(|b| *b = 0);
    }

    /// Designs the filters of the colour `designed`.
    fn design_filters(&mut self) {
        let fv = self.fv;
        let half = 0.49 * fv;
        self.chroma_lp = sdrkit_algo_dsp::fir::lowpass(fv, 1.1e6, 2.4e6, 40.0)
            .expect("the filter settings are valid");
        self.secam_lp = sdrkit_algo_dsp::fir::lowpass(fv, 0.9e6, 1.6e6, 40.0)
            .expect("the filter settings are valid");
        match self.designed {
            Some(Colour::Secam) => {
                let (lo, hi) = (secam::F_BELL - 0.65e6, (secam::F_BELL + 0.65e6).min(half));
                self.notch = sdrkit_algo_dsp::fir::bandpass_real(fv, lo, hi, 0.6e6, 30.0)
                    .expect("the filter settings are valid");
            }
            Some(c) if c != Colour::Mono => {
                let f = c
                    .params()
                    .expect("a PAL or NTSC standard has a subcarrier")
                    .fsc;
                let (lo, hi) = (f - 1.25e6, (f + 1.25e6).min(half));
                self.notch = sdrkit_algo_dsp::fir::bandpass_real(fv, lo, hi, 0.6e6, 30.0)
                    .expect("the filter settings are valid");
            }
            _ => self.notch.clear(),
        }
        // The SECAM bell at the decimated rate.
        let d2 = (fv / 2.5e6).round().max(1.0);
        let fs2 = fv / d2;
        self.bell = sdrkit_algo_dsp::fir::design_complex_by_response(27, fs2, DESIGN_BETA, |f| {
            let ff = (f + secam::F_BELL) / secam::F_BELL - secam::F_BELL / (f + secam::F_BELL);
            num_complex::Complex64::new(1.0, 1.26 * ff)
                / num_complex::Complex64::new(1.0, secam::BELL_Q * ff)
        })
        .unwrap_or_default();
        // White noise through the low-pass (at the video rate, every d2-th output kept) and the bell.
        let lp_ng: f64 = self.secam_lp.iter().map(|&t| (t as f64) * (t as f64)).sum();
        let b_ng: f64 = self.bell.iter().map(|t| t.norm_sqr() as f64).sum();
        let lag1: Complex32 = self
            .bell
            .windows(2)
            .fold(Complex32::default(), |a, w| a + w[1] * w[0].conj());
        self.bell_ng = lp_ng * b_ng;
        self.bell_r = if b_ng > 0.0 {
            lag1.norm() as f64 / b_ng
        } else {
            0.0
        };
    }

    fn setup_level(&self, r: &Raster) -> f64 {
        match self.o.setup {
            BlackSetup::None => 0.0,
            BlackSetup::Ire75 => 0.075,
            BlackSetup::System => {
                if r.lines == 525 {
                    0.075
                } else {
                    0.0
                }
            }
        }
    }

    /// Decodes one visible line into the frame.
    fn decode_row(&mut self, job: &RowJob) {
        let li = &job.line;
        let row = job.row;
        if let Some(c) = job.decided {
            if self.designed != Some(c) {
                self.designed = Some(c);
                self.design_filters();
            }
        }
        let r = raster_of(li.lines);
        let w = r.width as usize;
        let x0 = li.t0 + self.us(r.blank_end_us);
        let dx = self.us(r.active_us) / w as f64;
        let margin = 40usize.max(self.notch.len());
        let a = x0.floor() as i64 - margin as i64;
        let b = (x0 + dx * w as f64).ceil() as i64 + margin as i64;
        let mut scratch = std::mem::take(&mut self.seg_v);
        let seg_v = self.video.span(a, b, job.held, &mut scratch);
        // The place of pixel `i` in the segment, and the samples `[lo, hi)` that the interpolator reads for the row.
        let place = |i: usize| x0 + (i as f64 + 0.5) * dx - a as f64;
        let lo = place(0).floor() as i64 - 3;
        let hi = place(w.max(1) - 1).floor() as i64 + 5;
        // Luminance: the colour band taken out, at the samples that the row reads.
        let luma: &[f32] = if self.notch.is_empty() || !job.colour {
            seg_v
        } else {
            let (len, h) = (self.notch.len(), self.notch.len() / 2);
            self.seg_y.clear();
            if lo >= h as i64 && hi > lo && hi as usize - 1 - h + len <= seg_v.len() {
                self.seg_y.resize(seg_v.len(), 0.0);
                for i in lo as usize..hi as usize {
                    let band = <f32 as Sample>::dot(&self.notch, &seg_v[i - h..i - h + len]);
                    self.seg_y[i] = seg_v[i] - band;
                }
            } else {
                conv_same(seg_v, &self.notch, &mut self.pad_r, &mut self.seg_y, 1);
                for (y, v) in self.seg_y.iter_mut().zip(seg_v) {
                    *y = v - *y;
                }
            }
            &self.seg_y
        };
        let setup = self.setup_level(&r) as f32;
        let gain = li.gain * job.gain as f32;
        let (blank, pol) = (li.blank, li.pol);
        let mut ys = std::mem::take(&mut self.ys);
        ys.resize(w, 0.0);
        self.interp.row(luma, place, &mut ys[..w]);
        for y in ys.iter_mut() {
            let yy = (blank - pol * *y) / gain;
            *y = (yy - setup) / (1.0 - setup);
        }
        self.seg_v = scratch;
        let peak = ys.iter().cloned().fold(f32::MIN, f32::max) as f64;
        self.frame_peak = self.frame_peak.max(peak);
        // Chrominance.
        if self.row_uv.len() != w {
            self.row_uv = vec![[0.0; 2]; w];
        }
        let mut have = false;
        if job.colour {
            match (job.decided, &job.carrier) {
                (Some(Colour::Secam), _) => have = self.secam_row(job, &r, x0, dx, w),
                (Some(_), Some(l)) => {
                    self.pal_ntsc_row(job, l, x0, dx, w, a, b);
                    have = true;
                }
                _ => {}
            }
        }
        let sat = self.o.saturation as f32;
        let stride = w * 4;
        let base = row as usize * stride;
        if self.frame.len() != stride * r.height as usize {
            self.frame = vec![0; stride * r.height as usize];
        }
        let (pixels, _) = self.frame[base..base + stride].as_chunks_mut::<4>();
        rgb_row(&ys[..w], have.then_some(&self.row_uv[..w]), sat, pixels);
        self.ys = ys;
    }

    #[allow(clippy::too_many_arguments)]
    fn pal_ntsc_row(
        &mut self,
        job: &RowJob,
        l: &Carrier,
        x0: f64,
        dx: f64,
        w: usize,
        a: i64,
        b: i64,
    ) {
        let li = &job.line;
        let norm = -li.pol * li.gain * job.gain as f32;
        let dc = job.dc;
        let delay_ns = if self.o.chroma_delay_ns < 0.0 {
            if self.fm {
                0.0
            } else {
                170.0
            }
        } else {
            self.o.chroma_delay_ns
        };
        let dshift = self.us(delay_ns / 1000.0);
        // The place of pixel `i` in the low-passed chrominance (every second sample), and the outputs `[lo, hi)` of the
        // low-pass that the row reads.
        let place = |i: usize| (x0 + (i as f64 + 0.5) * dx + dshift - a as f64) / 2.0;
        let lo = place(0).floor() as i64 - 1;
        let hi = place(w.max(1) - 1).floor() as i64 + 3;
        let (si, co) = l.phase_at(a as f64, self.fv).sin_cos();
        let mut rot = Complex32::new(co as f32, -si as f32);
        let w_step = TAU * l.fsc / self.fv;
        let step = Complex32::new(w_step.cos() as f32, -w_step.sin() as f32);
        let mut scratch = std::mem::take(&mut self.seg_b);
        let seg_b = self.base.span(a, b, job.held, &mut scratch);
        let (len, h) = (self.chroma_lp.len(), self.chroma_lp.len() / 2);
        let outputs = seg_b.len().div_ceil(2);
        // The low-pass at every second sample (the band is about 1.3 MHz). The outputs that no pixel reads stay zero when
        // every window of the others lies inside the segment.
        let part = lo >= 0
            && hi > lo
            && hi as usize <= outputs
            && 2 * lo as usize >= h
            && 2 * (hi as usize - 1) - h + len <= seg_b.len();
        let (from, to) = if part {
            (lo as usize, hi as usize)
        } else {
            (0, outputs)
        };
        self.seg_c.clear();
        if part {
            let (first, last) = (2 * from - h, 2 * (to - 1) - h + len);
            // The mixer turns sample by sample from the start of the segment, read or not.
            for _ in 0..first {
                rot *= step;
            }
            self.seg_c.resize(first, Complex32::default());
            for &x in &seg_b[first..last] {
                self.seg_c.push((x - dc) * rot);
                rot *= step;
            }
            self.seg_d.clear();
            self.seg_d.resize(outputs, Complex32::default());
            for m in from..to {
                let at = 2 * m - h;
                self.seg_d[m] =
                    <Complex32 as Sample>::dot(&self.chroma_lp, &self.seg_c[at..at + len]);
            }
        } else {
            for &x in seg_b {
                self.seg_c.push((x - dc) * rot);
                rot *= step;
            }
            conv_same_c(
                &self.seg_c,
                &self.chroma_lp,
                &mut self.pad_c,
                &mut self.seg_d,
                2,
            );
        }
        self.seg_b = scratch;
        // Automatic chroma control: the burst is sent with the chrominance through the same channel, so a ghost or a tilt that
        // changes the chrominance changes the burst alike; the burst's level (share of nominal, against the sync-referenced gain
        // already corrected for a squeezed sync) sets the chrominance gain.
        let acc = if l.amp > 0.3 {
            (job.gain / l.amp).clamp(0.5, 2.0)
        } else {
            1.0
        };
        let inv = acc as f32 / norm;
        for d in &mut self.seg_d[from..to] {
            *d *= inv;
        }
        // V sign of this line from the burst (A lines have the burst's V part positive).
        let vs = if l.v_switch {
            let par = li.odd as usize;
            if l.vre[par] >= 0.0 {
                1.0
            } else {
                -1.0
            }
        } else {
            1.0
        };
        let (st, ct) = (self.o.tint.to_radians() as f32).sin_cos();
        let use_prev =
            l.v_switch && self.prev_uv.len() == w && self.prev_uv_counter + 1 == li.counter;
        let mut ds = std::mem::take(&mut self.row_d);
        ds.resize(w, Complex32::default());
        cubic_row(&self.seg_d, place, &mut ds);
        for (i, d) in ds.iter().enumerate() {
            let mut v = d.re * vs;
            let mut u = -d.im;
            if !l.v_switch && self.o.tint != 0.0 {
                let (uu, vv) = (u * ct - v * st, u * st + v * ct);
                u = uu;
                v = vv;
            }
            self.row_uv[i] = [u, v];
        }
        self.row_d = ds;
        if l.v_switch {
            if use_prev {
                for i in 0..w {
                    let cur = self.row_uv[i];
                    self.row_uv[i][0] = 0.5 * (cur[0] + self.prev_uv[i][0]);
                    self.row_uv[i][1] = 0.5 * (cur[1] + self.prev_uv[i][1]);
                    self.prev_uv[i] = cur;
                }
            } else {
                self.prev_uv.clear();
                self.prev_uv.extend_from_slice(&self.row_uv);
            }
            self.prev_uv_counter = li.counter;
        }
    }

    fn secam_row(&mut self, job: &RowJob, r: &Raster, x0: f64, dx: f64, w: usize) -> bool {
        let li = &job.line;
        // Which colour difference this line carries.
        let Some(odd_is_r) = job.odd_is_r else {
            return false;
        };
        let is_r = li.odd == odd_is_r;
        let fv = self.fv;
        let g = (li.pol * li.gain).abs().max(1e-9) as f64 * job.gain;
        let d2 = (fv / 2.5e6).round().max(1.0) as usize;
        let fs2 = fv / d2 as f64;
        let a = (li.t0 + self.us(5.0)).floor() as i64;
        let b = (li.t0 + self.us(r.blank_end_us + r.active_us + 5.0)).ceil() as i64;
        let wmix = TAU * secam::F_BELL / fv;
        let dc = job.dc;
        self.seg_c.clear();
        let (si, co) = (wmix * a as f64).sin_cos();
        let mut rot = Complex32::new(co as f32, -si as f32);
        let step = Complex32::new(wmix.cos() as f32, -wmix.sin() as f32);
        let mut scratch = std::mem::take(&mut self.seg_b);
        for &x in self.base.span(a, b, job.held, &mut scratch) {
            self.seg_c.push((x - dc) * rot);
            rot *= step;
        }
        self.seg_b = scratch;
        // Low-pass and decimate.
        conv_same_c(
            &self.seg_c,
            &self.secam_lp,
            &mut self.pad_c,
            &mut self.seg_d,
            d2,
        );
        // The bell.
        let nb = self.bell.len();
        let hb = nb / 2;
        let mut z = std::mem::take(&mut self.sec_z);
        z.clear();
        for i in 0..self.seg_d.len() {
            let mut acc = Complex32::default();
            for (j, t) in self.bell.iter().enumerate() {
                let idx = i as i64 + hb as i64 - j as i64;
                if idx >= 0 && (idx as usize) < self.seg_d.len() {
                    acc += self.seg_d[idx as usize] * *t;
                }
            }
            z.push(acc);
        }
        // Discriminator (power weighted over three steps), de-emphasis, gate.
        let (f0, dev, kk) = if is_r {
            (secam::F0R, secam::DEV_R, secam::KR)
        } else {
            (secam::F0B, secam::DEV_B, secam::KB)
        };
        let w1 = TAU * secam::F1;
        let k2 = 2.0 * fs2;
        // Inverse of (1 + s / w1) / (1 + s / (3 w1)).
        let (b0, b1) = (1.0 + k2 / (3.0 * w1), 1.0 - k2 / (3.0 * w1));
        let (a0, a1) = (1.0 + k2 / w1, 1.0 - k2 / w1);
        // In noise the bell's noise, gathered around its centre, pulls the discriminator towards it: the measured swing about the
        // centre shrinks by C / (C + N r). The bell restores a constant amplitude, so over the active line the spread of the
        // amplitude is the noise's (N = 2 var |z|) and its mean square is C + N; both are followed over the lines.
        let (ia, ib) = (
            ((self.us(r.blank_end_us + 1.0) - (a as f64 - li.t0)) / d2 as f64).max(0.0) as usize,
            (((self.us(r.blank_end_us + r.active_us - 1.0) - (a as f64 - li.t0)) / d2 as f64)
                .max(0.0) as usize)
                .min(z.len()),
        );
        if ib > ia + 8 {
            let (mut s1, mut s2) = (0.0f64, 0.0f64);
            for v in &z[ia..ib] {
                let m = v.norm() as f64 / g;
                s1 += m;
                s2 += m * m;
            }
            let n = (ib - ia) as f64;
            let (m1, m2) = (s1 / n, s2 / n);
            let noise = 2.0 * (m2 - m1 * m1).max(0.0);
            let carrier = (m2 - noise).max(1e-12);
            let rho = (noise / carrier).min(1.0);
            self.secam_rho = if self.secam_rho < 0.0 {
                rho
            } else {
                self.secam_rho + 0.05 * (rho - self.secam_rho)
            };
        }
        let stretch = 1.0 + self.secam_rho.max(0.0) * self.bell_r;
        let (mut xp, mut yp) = (0.0f64, 0.0f64);
        let mut dvals = std::mem::take(&mut self.sec_d);
        dvals.clear();
        dvals.resize(z.len(), 0.0);
        let mut lev = 0.0f64;
        let mut lev_n = 0usize;
        // The chroma level over the active line (the subcarrier is there whichever way the lines are identified).
        let lev_a =
            ((self.us(r.blank_end_us + 1.0) - (a as f64 - li.t0)) / d2 as f64).max(0.0) as usize;
        let lev_b = ((self.us(r.blank_end_us + r.active_us - 1.0) - (a as f64 - li.t0)) / d2 as f64)
            .max(0.0) as usize;
        for i in 1..z.len() {
            let mut acc = z[i] * z[i - 1].conj();
            if i >= 2 {
                acc += z[i - 1] * z[i - 2].conj();
            }
            if i + 1 < z.len() {
                acc += z[i + 1] * z[i].conj();
            }
            let f = secam::F_BELL + stretch * (acc.im as f64).atan2(acc.re as f64) * fs2 / TAU;
            let amp = z[i].norm() as f64 / g;
            let mut d = (f - f0) / dev;
            if amp < 0.3 * secam::M0 {
                d = 0.0;
            }
            if i > lev_a && i < lev_b {
                lev += amp / secam::M0;
                lev_n += 1;
            }
            let y = (b0 * d + b1 * xp - a1 * yp) / a0;
            xp = d;
            yp = y;
            dvals[i] = y as f32;
        }
        if lev_n > 0 {
            self.secam.level += 0.1 * (lev / lev_n as f64 - self.secam.level);
        }
        // Delays: the low-pass is centred; the bell is centred (taps around hb).
        let mut cur = std::mem::take(&mut self.sec_cur);
        cur.clear();
        cur.resize(w, 0.0);
        cubic_row(
            &dvals,
            |i| (x0 + (i as f64 + 0.5) * dx - a as f64) / d2 as f64,
            &mut cur,
        );
        for c in cur.iter_mut() {
            *c /= kk as f32;
        }
        let have_other = self.secam.prev_row.len() == w && self.secam.prev_is_r == Some(!is_r);
        for i in 0..w {
            let o = if have_other {
                self.secam.prev_row[i]
            } else {
                0.0
            };
            let (ry, by) = if is_r { (cur[i], o) } else { (o, cur[i]) };
            self.row_uv[i] = [by * KU, ry * KV];
        }
        // This line's colour difference is the next line's other one; the old row's buffer is reused.
        self.sec_cur = std::mem::replace(&mut self.secam.prev_row, cur);
        self.secam.prev_is_r = Some(is_r);
        self.sec_z = z;
        self.sec_d = dvals;
        have_other
    }

    fn end_field(&mut self, field: u8, io: &mut Io, t: f64, video_snr: f64) {
        let r = self.raster;
        let done = self.rows_done[field as usize];
        if done as f64 >= 0.9 * r.rows as f64 {
            self.fields_ok += 1;
        } else {
            self.fields_bad += 1;
        }
        self.rows_done[field as usize] = 0;
        let publish = self.o.deinterlace == Deinterlace::Bob || field == 1;
        if publish && done > 0 {
            self.white_peak = self.frame_peak;
            self.frame_peak = 0.0;
            self.publish(field, io, t, video_snr);
        }
    }

    fn publish(&mut self, field: u8, io: &mut Io, t: f64, video_snr: f64) {
        let r = self.raster;
        let (w, h) = (r.width as usize, r.height as usize);
        let mut data = self.frame.clone();
        if self.o.deinterlace == Deinterlace::Bob {
            // The other field's rows: the mean of their neighbours.
            let stride = w * 4;
            for row in 0..h {
                if (row % 2) as u8 != field {
                    let up = if row > 0 { row - 1 } else { row + 1 };
                    let dn = if row + 1 < h { row + 1 } else { row - 1 };
                    for i in 0..stride {
                        data[row * stride + i] = ((data[up * stride + i] as u16
                            + data[dn * stride + i] as u16)
                            / 2) as u8;
                    }
                }
            }
        }
        // Temporal noise reduction.
        let k = match self.o.nr {
            NoiseReduction::Off => 1.0,
            NoiseReduction::Light => 0.5,
            NoiseReduction::Medium => 0.3,
            NoiseReduction::Strong => 0.15,
            NoiseReduction::Auto => {
                if video_snr >= 30.0 {
                    1.0
                } else {
                    (0.12 + 0.88 * ((video_snr - 8.0) / 22.0).max(0.0).powi(2)).max(0.12)
                }
            }
        };
        if k < 1.0 {
            if self.prev_pic.len() != data.len() {
                self.prev_pic = data.iter().map(|&v| v as f32).collect();
            } else {
                let sigma = 10f64.powf(-video_snr / 20.0);
                let tt = (5.0 * sigma * 255.0 + 8.0) as f32;
                for (i, px) in data.iter_mut().enumerate() {
                    if i % 4 == 3 {
                        continue;
                    }
                    let p = self.prev_pic[i];
                    let d = (*px as f32 - p).abs();
                    let kk = if d > tt {
                        (k as f32 + (d - tt) / tt).min(1.0)
                    } else {
                        k as f32
                    };
                    let v = p + kk * (*px as f32 - p);
                    self.prev_pic[i] = v;
                    *px = v.round().clamp(0.0, 255.0) as u8;
                }
            }
        } else {
            self.prev_pic.clear();
        }
        let layout = PixelLayout::packed(PixelFormat::Bgra, w as u32, h as u32);
        let pic = Picture {
            codec: Codec::Pixels(PixelFormat::Bgra),
            layout: Some(layout),
            name: None,
            id: Picture::WHOLE,
            state: sdrkit_core::PictureState::Finished,
            data: Arc::from(data.into_boxed_slice()),
        };
        self.pictures += 1;
        io.items_out::<Picture>(0).push(Timed {
            time: t,
            value: pic,
        });
    }

    /// Carries out one order of the colour lock.
    fn job(&mut self, job: &LineJob, io: &mut Io) {
        match job {
            LineJob::Restart => {
                self.reset();
                self.fields_ok = 0;
                self.fields_bad = 0;
                self.pictures = 0;
            }
            LineJob::SecamRestart => self.secam = SecamState::default(),
            LineJob::Raster(lines) => {
                let r = raster_of(*lines);
                self.raster = r;
                self.frame = vec![0; (r.width * r.height * 4) as usize];
                self.prev_pic.clear();
                self.rows_done = [0; 2];
            }
            LineJob::EndField {
                field,
                time,
                video_snr,
            } => self.end_field(*field, io, *time, *video_snr),
            LineJob::Row(row) => {
                self.decode_row(row);
                self.rows_done[row.field as usize] += 1;
            }
        }
    }
}

impl Component for PictureDecoder {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "atv.picture",
            vec![
                Port::real("video"),
                Port::iq("base"),
                Port::typed::<LineJob>("jobs"),
                Port::typed::<Picture>("snow"),
                Port::typed::<PictureReport>("colour"),
            ],
            vec![
                Port::typed::<Picture>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<PictureReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(PictureSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fv = inputs[0].rate;
        let n = ((25.0 * 100e-6 * self.fv) as usize)
            .next_power_of_two()
            .max(16);
        self.video = Held::new(n);
        self.base = Held::new(n);
        self.reset();
        self.fields_ok = 0;
        self.fields_bad = 0;
        self.pictures = 0;
        self.snow_pictures = 0;
        self.designed = None;
        self.design_filters();
        Ok(vec![
            Format::new(Kind::typed::<Picture>(), 0.0),
            Format::new(Kind::typed::<PictureReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        self.video.extend(io.real(0));
        self.base.extend(io.iq(1));
        // The orders are of lines that this pass completed: their samples are held.
        for k in 0..io.items::<LineJob>(2).len() {
            let job = io.items::<LineJob>(2)[k].value;
            self.job(&job, io);
        }
        for p in io.items::<Picture>(3) {
            self.snow_pictures += 1;
            io.items_out::<Picture>(0).push(Timed {
                time: p.time,
                value: p.value.clone(),
            });
        }
        if io.tick() {
            let mut rep = io
                .items::<PictureReport>(4)
                .last()
                .map(|r| r.value.clone())
                .unwrap_or_default();
            rep.chroma_level = self.secam.level;
            rep.fields_ok = self.fields_ok;
            rep.fields_bad = self.fields_bad;
            rep.pictures = self.pictures;
            rep.snow_pictures = self.snow_pictures;
            rep.white_peak = self.white_peak;
            let t = io.input_format(0).time_of(io.input(0).first());
            io.items_out::<PictureReport>(1).push(Timed {
                time: t,
                value: rep,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PictureSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PictureSetting::Saturation => self.o.saturation = SATURATION.read(id, v)?,
            PictureSetting::Tint => self.o.tint = TINT.read(id, v)?,
            PictureSetting::Deinterlace => self.o.deinterlace = DEINTERLACE.read(id, v)?,
            PictureSetting::NoiseReduction => self.o.nr = NOISE_REDUCTION.read(id, v)?,
            PictureSetting::BlackSetup => self.o.setup = BLACK_SETUP.read(id, v)?,
            PictureSetting::ChromaDelay => self.o.chroma_delay_ns = CHROMA_DELAY.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let o = &self.o;
        values_of([
            (PictureSetting::Saturation, Value::F64(o.saturation)),
            (PictureSetting::Tint, Value::F64(o.tint)),
            (
                PictureSetting::Deinterlace,
                Value::Choice(o.deinterlace.index()),
            ),
            (PictureSetting::NoiseReduction, Value::Choice(o.nr.index())),
            (PictureSetting::BlackSetup, Value::Choice(o.setup.index())),
            (PictureSetting::ChromaDelay, Value::F64(o.chroma_delay_ns)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_neon_row_is_the_plain_row_byte_for_byte() {
        // A fixed pseudo-random row with values outside 0 to 1, halves, zeros of both signs and NaN.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut ys: Vec<f32> = (0..4000).map(|_| 1.6 * next() - 0.3).collect();
        let uv: Vec<[f32; 2]> = (0..4000).map(|_| [next() - 0.5, next() - 0.5]).collect();
        for (k, v) in [0.0, -0.0, 1.0, f32::NAN, f32::INFINITY, -1e30]
            .into_iter()
            .enumerate()
        {
            ys[17 * k + 3] = v;
        }
        for k in 0..256 {
            ys[1000 + k] = (k as f32 + 0.5) / 255.0;
        }
        for n in (0..40).chain([768, 4000]) {
            for (uv, sat) in [(Some(&uv[..n]), 1.0), (Some(&uv[..n]), 1.7), (None, 1.0)] {
                let mut plain = vec![[0u8; 4]; n];
                let mut fast = vec![[1u8; 4]; n];
                rgb_row_plain(&ys[..n], uv, sat, &mut plain);
                rgb_row(&ys[..n], uv, sat, &mut fast);
                assert_eq!(plain, fast, "{n} pixels");
                // Grey pixels are those of (U, V) = (0, 0).
                if uv.is_none() {
                    rgb_row_plain(&ys[..n], Some(&vec![[0.0; 2]; n]), sat, &mut fast);
                    assert_eq!(plain, fast);
                }
            }
        }
    }
}
