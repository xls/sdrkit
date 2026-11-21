//! The voice codec 4 encoder: input path, LPC analysis, LSF quantisation, the codebook searches, the high band, VAD, DTX and SID.

use super::core::{
    adaptive, gc_level, gp_level, guard, lloyd, lsf_dequant, sharpen, split_lag, subframe_lsf,
    HIST, INTERP_W,
};
use super::decoder::Upsampler;
use super::frame::{self, sid_bits, Params, Sid, SidIdx, SID_STEP};
use super::highband::{t, EDGES, TIME_LEVELS, WIDTH};
use super::layout::{Config, Layout};
use super::lpc::{
    analyse, fir_step, interp, lsf_to_lpc, stabilise, synth_step, weight, Lpc, ORDER,
};
use super::math::{db, pow10, sin, Fft, Mdct, PI};
use super::pvq;
use super::tables::{KLT_BASIS, KLT_SD, LPC_WINDOW, LSF_MEAN, SPLIT_TAPS};
use crate::Error;
use sdrkit_util_bits::BitWriter;

/// The encoder settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EncoderSettings {
    /// Discontinuous transmission: silence becomes SID frames or no frame at all.
    pub dtx: bool,
    /// The input noise suppressor. It adds one hop of delay (80 samples at 16 kHz, 40 at 8 kHz).
    pub noise_suppressor: bool,
}

/// What the encoder made of one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Encoded {
    /// A speech frame of `n` bytes.
    Speech(usize),
    /// A SID frame of `n` bytes.
    Sid(usize),
    /// No frame (DTX). The container sends nothing or data.
    NoData,
}

impl Encoded {
    /// The number of bytes written.
    pub fn len(self) -> usize {
        match self {
            Encoded::Speech(n) | Encoded::Sid(n) => n,
            Encoded::NoData => 0,
        }
    }

    /// True for "no data".
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// The noise suppressor of section 3.8.12 (the decoder variant is in the enhancer).
pub(crate) struct NoiseSuppressor {
    m: usize,
    f: usize,
    fft: Fft,
    win: [f32; 160],
    s: [f32; 129],
    n: [f32; 129],
    g: [f32; 129],
    gam: [f32; 129],
    count: u32,
    prev: [f32; 80],
    tail: [f32; 80],
    re: [f32; 256],
    im: [f32; 256],
    decoder: bool,
}

impl NoiseSuppressor {
    pub(crate) fn new(rate16: bool, decoder: bool) -> Self {
        let m = if rate16 { 80 } else { 40 };
        let f = if rate16 { 256 } else { 128 };
        let mut win = [0f32; 160];
        for (n, w) in win.iter_mut().enumerate().take(2 * m) {
            *w = sin(PI * (n as f64 + 0.5) / (2 * m) as f64) as f32;
        }
        Self {
            m,
            f,
            fft: Fft::new(f),
            win,
            s: [0.0; 129],
            n: [0.0; 129],
            g: [1.0; 129],
            gam: [1.0; 129],
            count: 0,
            prev: [0.0; 80],
            tail: [0.0; 80],
            re: [0.0; 256],
            im: [0.0; 256],
            decoder,
        }
    }

    /// Processes `x` in place in hops. `comfort` marks a comfort-noise frame (decoder variant only).
    pub(crate) fn run(&mut self, x: &mut [f32], comfort: bool) {
        let m = self.m;
        let f = self.f;
        for hop in x.chunks_mut(m) {
            for n in 0..m {
                self.re[n] = self.prev[n] * self.win[n];
                self.re[m + n] = hop[n] * self.win[m + n];
            }
            self.re[2 * m..f].fill(0.0);
            self.im[..f].fill(0.0);
            self.prev[..m].copy_from_slice(hop);
            self.fft.run(&mut self.re[..f], &mut self.im[..f], false);
            for k in 0..=f / 2 {
                let p = self.re[k] * self.re[k] + self.im[k] * self.im[k];
                let (nk, xi_a, xi_b, floor);
                if !self.decoder {
                    self.s[k] = if self.count == 0 {
                        p
                    } else {
                        0.7 * self.s[k] + 0.3 * p
                    };
                    if self.count < 12 {
                        self.n[k] = if self.count == 0 {
                            self.s[k]
                        } else {
                            let c = self.count as f32;
                            (self.n[k] * c + self.s[k]) / (c + 1.0)
                        };
                    } else {
                        self.n[k] = self.s[k].min(1.004 * self.n[k]);
                    }
                    nk = 1.5 * self.n[k] + 1e-3;
                    xi_a = 0.98;
                    xi_b = 0.02;
                    floor = 0.16;
                } else {
                    if self.count == 0 {
                        self.n[k] = p;
                    } else if comfort {
                        self.n[k] = 0.8 * self.n[k] + 0.2 * p;
                    } else if p < 1.003 * self.n[k] && p > self.n[k] {
                        self.n[k] = p;
                    } else if p <= self.n[k] {
                        self.n[k] = 0.9 * self.n[k] + 0.1 * p;
                    } else {
                        self.n[k] *= 1.003;
                    }
                    nk = self.n[k] + 1e-3;
                    xi_a = 0.96;
                    xi_b = 0.04;
                    floor = 0.4;
                }
                let g = p / nk;
                let xi = xi_a * self.g[k] * self.g[k] * self.gam[k] + xi_b * (g - 1.0).max(0.0);
                let gk = (xi / (1.0 + xi)).max(floor);
                self.g[k] = gk;
                self.gam[k] = g;
                self.re[k] *= gk;
                self.im[k] *= gk;
            }
            for k in 1..f / 2 {
                self.re[f - k] = self.re[k];
                self.im[f - k] = -self.im[k];
            }
            self.fft.run(&mut self.re[..f], &mut self.im[..f], true);
            let ff = f as f32;
            for n in 0..m {
                hop[n] = self.tail[n] + (self.re[n] / ff) * self.win[n];
                self.tail[n] = (self.re[m + n] / ff) * self.win[m + n];
            }
            self.count += 1;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sent {
    Speech,
    Sid,
    NoData,
}

#[derive(Clone, Copy)]
struct Background {
    lsf: [f64; ORDER],
    res: f64,
    hb: f64,
    tilt: f64,
}

/// The voice codec 4 encoder.
pub struct Encoder {
    lay: Layout,
    settings: EncoderSettings,
    ns: Option<Box<NoiseSuppressor>>,
    dc_r: f64,
    dc_x1: f64,
    dc_y1: f64,
    down: [f32; 95 + 640],
    up: Upsampler,
    delay: [f32; 93 + 640],
    inbuf: [f32; 80 + 320 + 40],
    // Core state.
    prev_lsf: [f64; ORDER],
    prev_unq: [f64; ORDER],
    exc: [f32; HIST + 320],
    syn: [f32; ORDER],
    err_iir: [f32; ORDER],
    hist: [f64; 4],
    prev_gp: f64,
    prev_lag: i32,
    wsp: [f32; 148 + 320],
    wfir: [f32; ORDER],
    wiir: [f32; ORDER],
    ol_lag: usize,
    // High band.
    mdct: Mdct,
    hb_hist: [f32; 80],
    // VAD and DTX.
    vad_floor: f64,
    vad_count: u32,
    vad_run: u32,
    vad_hang: i32,
    last_sent: Sent,
    bg: Option<Background>,
    sent: Sid,
    sid_timer: u32,
    // Scratch.
    pcm: [f32; 640],
    lb: [f32; 320],
    hb: [f32; 640],
    pbuf: Vec<u8>,
}

struct Analysis {
    params: Params,
    res_level: f64,
    hb_level: f64,
    hb_tilt: f64,
    end_unq: [f64; ORDER],
}

impl Encoder {
    /// Makes an encoder for `cfg` with `settings`.
    ///
    /// Errors: [`Error::BadConfig`] when the configuration has no layout.
    pub fn new(cfg: Config, settings: EncoderSettings) -> Result<Self, Error> {
        let lay = Layout::new(cfg)?;
        let fs = cfg.rate.hz() as f64;
        let rate16 = lay.hb_on;
        Ok(Self {
            ns: settings
                .noise_suppressor
                .then(|| Box::new(NoiseSuppressor::new(rate16, false))),
            dc_r: 1.0 - 2.0 * PI * 20.0 / fs,
            dc_x1: 0.0,
            dc_y1: 0.0,
            down: [0.0; 95 + 640],
            up: Upsampler::new(),
            delay: [0.0; 93 + 640],
            inbuf: [0.0; 80 + 320 + 40],
            prev_lsf: LSF_MEAN,
            prev_unq: LSF_MEAN,
            exc: [0.0; HIST + 320],
            syn: [0.0; ORDER],
            err_iir: [0.0; ORDER],
            hist: [0.0; 4],
            prev_gp: 0.0,
            prev_lag: 40 * lay.res as i32,
            wsp: [0.0; 148 + 320],
            wfir: [0.0; ORDER],
            wiir: [0.0; ORDER],
            ol_lag: 60,
            mdct: Mdct::new(),
            hb_hist: [0.0; 80],
            vad_floor: 0.0,
            vad_count: 0,
            vad_run: 0,
            vad_hang: 0,
            last_sent: Sent::Speech,
            bg: None,
            sent: Sid {
                lsf: LSF_MEAN,
                res: 0.0,
                hb: 0.0,
                tilt: 0.0,
            },
            sid_timer: 0,
            pcm: [0.0; 640],
            lb: [0.0; 320],
            hb: [0.0; 640],
            pbuf: Vec::with_capacity(96),
            lay,
            settings,
        })
    }

    /// The frame layout.
    pub fn layout(&self) -> &Layout {
        &self.lay
    }

    /// Samples per frame at the codec rate.
    pub fn frame_samples(&self) -> usize {
        self.lay.frame_samples
    }

    /// The delay from encoder input to decoder output, in samples, with the noise suppressor when it is on.
    pub fn delay(&self) -> usize {
        let extra = match (self.settings.noise_suppressor, self.lay.hb_on) {
            (false, _) => 0,
            (true, true) => 80,
            (true, false) => 40,
        };
        self.lay.delay + extra
    }

    /// Encodes one frame of PCM into `out`. `out` must hold at least the payload size in bytes.
    ///
    /// Errors: [`Error::BadInputLength`] when `pcm` is not one frame, [`Error::OutputTooShort`] when `out` is too short.
    pub fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<Encoded, Error> {
        let n = self.lay.frame_samples;
        if pcm.len() != n {
            return Err(Error::BadInputLength { need: n });
        }
        let need = self.lay.payload_bytes.max(5);
        if out.len() < need {
            return Err(Error::OutputTooShort { need });
        }
        let e_db = self.input_path(pcm);
        let active = if self.settings.dtx {
            self.vad(e_db)
        } else {
            true
        };
        if active && self.last_sent != Sent::Speech {
            let lsf = self.sent.lsf;
            self.restart(&lsf);
        }
        let a = self.analyse_frame();
        if active {
            self.last_sent = Sent::Speech;
            self.bg = None;
            frame::pack(&self.lay, &a.params, &mut self.pbuf);
            let len = self.pbuf.len();
            out[..len].copy_from_slice(&self.pbuf);
            return Ok(Encoded::Speech(len));
        }
        let mut cur = Background {
            lsf: a.end_unq,
            res: a.res_level,
            hb: a.hb_level,
            tilt: a.hb_tilt,
        };
        if !self.lay.hb_on {
            cur.hb = 0.0;
            cur.tilt = 0.0;
        }
        cur.res = cur.res.max(0.0);
        cur.hb = cur.hb.max(0.0);
        let bg = match self.bg {
            None => cur,
            Some(old) => {
                let a = if self.lay.frame_ms == 40 { 0.75 } else { 0.85 };
                let mut b = old;
                for i in 0..ORDER {
                    b.lsf[i] = a * old.lsf[i] + (1.0 - a) * cur.lsf[i];
                }
                b.res = a * old.res + (1.0 - a) * cur.res;
                b.hb = a * old.hb + (1.0 - a) * cur.hb;
                b.tilt = a * old.tilt + (1.0 - a) * cur.tilt;
                b
            }
        };
        self.bg = Some(bg);
        self.sid_timer += self.lay.frame_ms;
        let mut dl = 0.0;
        for i in 0..ORDER {
            let d = bg.lsf[i] - self.sent.lsf[i];
            dl += d * d;
        }
        let send = self.last_sent == Sent::Speech
            || self.sid_timer >= 480
            || (self.sid_timer >= 80
                && ((bg.res - self.sent.res).abs() > 4.0
                    || (bg.hb - self.sent.hb.max(0.0)).abs() > 6.0
                    || dl > 0.04));
        if !send {
            self.last_sent = Sent::NoData;
            return Ok(Encoded::NoData);
        }
        let mut idx = SidIdx::default();
        for i in 0..ORDER {
            let n = (1u32 << sid_bits(i)) as f64;
            let v = ((bg.lsf[i] - LSF_MEAN[i]) / SID_STEP[i] + (n - 1.0) / 2.0 + 0.5).floor();
            idx.lsf[i] = v.clamp(0.0, n - 1.0) as u32;
        }
        idx.res = (bg.res / 1.5 + 0.5).floor().clamp(0.0, 63.0) as u32;
        idx.hb = (bg.hb / 3.0 + 0.5).floor().clamp(0.0, 31.0) as u32;
        idx.tilt = u32::from(bg.tilt >= -4.5);
        self.sent = frame::sid_dequant(&idx);
        self.sid_timer = 0;
        self.last_sent = Sent::Sid;
        self.pbuf.clear();
        let mut w = BitWriter::new(&mut self.pbuf);
        if self.lay.cfg.type_in_band {
            w.put(1, 1);
        }
        frame::sid_pack(&idx, &mut w);
        w.finish();
        let len = if self.lay.cfg.type_in_band {
            self.lay.payload_bytes
        } else {
            5
        };
        self.pbuf.resize(len, 0);
        out[..len].copy_from_slice(&self.pbuf);
        Ok(Encoded::Sid(len))
    }

    fn restart(&mut self, lsf: &[f64; ORDER]) {
        self.exc = [0.0; HIST + 320];
        self.syn = [0.0; ORDER];
        self.hist = [0.0; 4];
        self.prev_gp = 0.0;
        self.prev_lsf = *lsf;
        self.err_iir = [0.0; ORDER];
    }

    /// The input path of section 3.8.2. Fills the low band and the high band, shifts the input buffer, and returns the frame
    /// energy in dB.
    fn input_path(&mut self, pcm: &[i16]) -> f64 {
        let n = pcm.len();
        let nc = self.lay.nc;
        for (d, &s) in self.pcm.iter_mut().zip(pcm) {
            *d = s as f32;
        }
        if let Some(ns) = self.ns.as_mut() {
            ns.run(&mut self.pcm[..n], false);
        }
        for v in self.pcm[..n].iter_mut() {
            let x = *v as f64;
            let y = x - self.dc_x1 + self.dc_r * self.dc_y1;
            self.dc_x1 = x;
            self.dc_y1 = y;
            *v = y as f32;
        }
        let e_db = if self.lay.hb_on {
            self.down[95..95 + n].copy_from_slice(&self.pcm[..n]);
            for m in 0..nc {
                let base = 95 + 2 * m + 1;
                let mut acc = 0f32;
                for k in 0..95 {
                    acc += SPLIT_TAPS[k] * self.down[base - k];
                }
                self.lb[m] = acc;
            }
            self.down.copy_within(n..n + 95, 0);
            let mut up = [0f32; 640];
            self.up.run(&self.lb[..nc], &mut up[..n]);
            self.delay[93..93 + n].copy_from_slice(&self.pcm[..n]);
            for i in 0..n {
                self.hb[i] = self.delay[i] - up[i];
            }
            self.delay.copy_within(n..n + 93, 0);
            let mut sl = 0f64;
            let mut sh = 0f64;
            for &v in &self.lb[..nc] {
                sl += v as f64 * v as f64;
            }
            for &v in &self.hb[..n] {
                sh += v as f64 * v as f64;
            }
            db(sl / nc as f64 + sh / n as f64)
        } else {
            self.lb[..nc].copy_from_slice(&self.pcm[..nc]);
            let mut s = 0f64;
            for &v in &self.lb[..nc] {
                s += v as f64 * v as f64;
            }
            db(s / nc as f64)
        };
        let len = 80 + nc + 40;
        self.inbuf.copy_within(nc..len, 0);
        self.inbuf[len - nc..len].copy_from_slice(&self.lb[..nc]);
        e_db
    }

    fn vad(&mut self, e_db: f64) -> bool {
        let nc = self.lay.nc;
        let x = &self.lb[..nc];
        let mut e0 = 0f64;
        for &v in x {
            e0 += v as f64 * v as f64;
        }
        let mut voicing = 0f64;
        for tt in 20..=147usize.min(nc - 21) {
            let mut c = 0f64;
            let mut e = 0f64;
            for i in tt..nc {
                c += x[i] as f64 * x[i - tt] as f64;
                e += x[i - tt] as f64 * x[i - tt] as f64;
            }
            let r = c / ((e + 1.0) * (e0 + 1.0)).sqrt();
            if r > voicing {
                voicing = r;
            }
        }
        if self.vad_count == 0 {
            self.vad_floor = e_db;
        }
        self.vad_count += 1;
        if self.vad_count < 6 && e_db < self.vad_floor {
            self.vad_floor = e_db;
        }
        let fl = self.vad_floor;
        let speech = e_db > 18.0 && (e_db > fl + 9.0 || (e_db > fl + 4.0 && voicing > 0.5));
        if e_db < self.vad_floor {
            self.vad_floor = 0.7 * self.vad_floor + 0.3 * e_db;
        } else {
            self.vad_floor += 0.05 * self.lay.frame_ms as f64 / 20.0;
        }
        if !speech && e_db < self.vad_floor + 6.0 {
            self.vad_floor += 0.1 * (e_db - self.vad_floor);
        }
        let ms = self.lay.frame_ms;
        if speech {
            self.vad_run += ms;
            self.vad_hang = if self.vad_run >= 60 { 160 } else { 60 };
            true
        } else {
            self.vad_run = 0;
            if self.vad_hang > 0 {
                self.vad_hang -= ms as i32;
                true
            } else {
                false
            }
        }
    }

    /// LSF quantisation of section 3.8.4. Returns the indices and the quantised LSF.
    fn quantise_lsf(&self, lsf: &[f64; ORDER]) -> ([u32; 10], [f64; ORDER]) {
        let lay = &self.lay;
        let t = lay.table;
        let mut pred = [0f64; ORDER];
        let mut r = [0f64; ORDER];
        for i in 0..ORDER {
            pred[i] = LSF_MEAN[i] + lay.rho * (self.prev_lsf[i] - LSF_MEAN[i]);
            r[i] = lsf[i] - pred[i];
        }
        let mut idx = [0u32; 10];
        for k in 0..ORDER {
            let b = lay.lsf_bits[k];
            if b == 0 {
                continue;
            }
            let mut c = 0f64;
            for i in 0..ORDER {
                c += r[i] * KLT_BASIS[t][k][i];
            }
            let v = c / KLT_SD[t][k];
            let lv = lloyd(b);
            let mut j = lv.len() - 1;
            for (jj, w) in lv.windows(2).enumerate() {
                if v <= (w[0] + w[1]) / 2.0 {
                    j = jj;
                    break;
                }
            }
            idx[k] = j as u32;
        }
        let mut w = [0f64; ORDER];
        for i in 0..ORDER {
            let lo = if i == 0 { lsf[0] } else { lsf[i] - lsf[i - 1] };
            let hi = if i == 9 {
                PI - lsf[9]
            } else {
                lsf[i + 1] - lsf[i]
            };
            w[i] = 1.0 / lo.max(0.01) + 1.0 / hi.max(0.01);
        }
        let err = |idx: &[u32; 10]| {
            let q = lsf_dequant(lay, idx, &self.prev_lsf, false);
            let mut e = 0f64;
            for i in 0..ORDER {
                let d = q[i] - lsf[i];
                e += w[i] * d * d;
            }
            e
        };
        let mut best = err(&idx);
        for _ in 0..2 {
            let mut changed = false;
            for k in 0..ORDER {
                let b = lay.lsf_bits[k];
                if b == 0 {
                    continue;
                }
                let max = (1u32 << b) - 1;
                for d in [-1i32, 1] {
                    let c = idx[k] as i32 + d;
                    if c < 0 || c > max as i32 {
                        continue;
                    }
                    let mut trial = idx;
                    trial[k] = c as u32;
                    let e = err(&trial);
                    if e < best {
                        best = e;
                        idx = trial;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let q = lsf_dequant(lay, &idx, &self.prev_lsf, true);
        (idx, q)
    }

    fn analyse_frame(&mut self) -> Analysis {
        let lay = self.lay.clone();
        let nc = lay.nc;
        let l = lay.l;
        let len = 80 + nc + 40;
        let mut end = analyse(&self.inbuf[len - 280..len], &LPC_WINDOW).unwrap_or(self.prev_unq);
        stabilise(&mut end, 0.012);
        let mut mid_unq = end;
        if lay.halves == 2 {
            mid_unq = analyse(&self.inbuf[..280], &LPC_WINDOW).unwrap_or(self.prev_unq);
            stabilise(&mut mid_unq, 0.012);
        }
        let a_end = lsf_to_lpc(&end);
        let mut mem = [0f32; ORDER];
        for k in 1..=ORDER {
            mem[k - 1] = self.inbuf[80 - k];
        }
        let mut rs = 0f64;
        for n in 0..nc {
            let r = fir_step(&a_end, &mut mem, self.inbuf[80 + n]);
            rs += r as f64 * r as f64;
        }
        let res_level = db(rs / nc as f64);

        let mut p = Params::default();
        let (idx, cur) = self.quantise_lsf(&end);
        p.lsf = idx;
        let mut mid_q = cur;
        if lay.halves == 2 {
            let mut best = f64::INFINITY;
            for (k, &w) in INTERP_W.iter().enumerate() {
                let m = interp(&self.prev_lsf, &cur, w);
                let mut e = 0f64;
                for i in 0..ORDER {
                    let d = m[i] - mid_unq[i];
                    e += d * d;
                }
                if e < best {
                    best = e;
                    p.interp = k as u32;
                    mid_q = m;
                }
            }
        }
        // Weighted speech and open-loop pitch.
        let mut aq = [[0f32; ORDER + 1]; 8];
        let mut a1 = [[0f32; ORDER + 1]; 8];
        let mut a2 = [[0f32; ORDER + 1]; 8];
        for s in 0..lay.s {
            aq[s] = lsf_to_lpc(&subframe_lsf(&lay, s, &self.prev_lsf, &mid_q, &cur));
            let au = lsf_to_lpc(&subframe_lsf(&lay, s, &self.prev_unq, &mid_unq, &end));
            a1[s] = weight(&au, 0.94);
            a2[s] = weight(&au, 0.6);
            for n in 0..l {
                let x = self.inbuf[80 + s * l + n];
                let v = fir_step(&a1[s], &mut self.wfir, x);
                self.wsp[148 + s * l + n] = synth_step(&a2[s], &mut self.wiir, v);
            }
        }
        let mut ol = [0usize; 4];
        for (g, olg) in ol.iter_mut().enumerate().take(nc / 80) {
            *olg = self.open_loop(148 + 80 * g);
        }
        // Subframes.
        let low = 20 * lay.res as i32;
        let high = if lay.res == 2 { 295 } else { 147 };
        let per_half = lay.s / lay.halves;
        for s in 0..lay.s {
            self.subframe(
                &lay,
                s,
                s % per_half == 0,
                ol[s * l / 80],
                &aq[s],
                &a1[s],
                &a2[s],
                low,
                high,
                &mut p,
            );
        }
        self.exc.copy_within(nc..nc + HIST, 0);
        self.wsp.copy_within(nc..nc + 148, 0);
        self.prev_lsf = cur;
        self.prev_unq = end;
        // High band.
        let (mut hb_level, mut hb_tilt) = (0.0, 0.0);
        if lay.hb_on {
            let mut gs = 0.0;
            let mut ts = 0.0;
            for h in 0..lay.halves {
                let (g, slope) = self.hb_half(&lay, h, &mut p);
                gs += g;
                ts += slope;
            }
            hb_level = gs / lay.halves as f64;
            hb_tilt = ts / lay.halves as f64;
        }
        Analysis {
            params: p,
            res_level,
            hb_level,
            hb_tilt,
            end_unq: end,
        }
    }

    fn open_loop(&mut self, start: usize) -> usize {
        let w = &self.wsp;
        let mut best_v = [-1e30f64; 3];
        let mut best_t = [20usize, 40, 80];
        for tt in 20..=147usize {
            let mut c = 0f64;
            let mut e = 0f64;
            for n in 0..80 {
                let a = w[start + n] as f64;
                let b = w[start + n - tt] as f64;
                c += a * b;
                e += b * b;
            }
            e += 1.0;
            let mut v = c / e.sqrt();
            if tt.abs_diff(self.ol_lag) <= 3 {
                v *= 1.04;
            }
            let sec = if tt < 40 {
                0
            } else if tt < 80 {
                1
            } else {
                2
            };
            if v > best_v[sec] {
                best_v[sec] = v;
                best_t[sec] = tt;
            }
        }
        let mut tt = best_t[2];
        let mut cur = best_v[2];
        if best_v[1] >= 0.85 * cur {
            tt = best_t[1];
            cur = best_v[1];
        }
        if best_v[0] >= 0.85 * cur {
            tt = best_t[0];
        }
        self.ol_lag = tt;
        tt
    }

    #[allow(clippy::too_many_arguments)]
    fn subframe(
        &mut self,
        lay: &Layout,
        s: usize,
        first: bool,
        t0: usize,
        aq: &Lpc,
        a1: &Lpc,
        a2: &Lpc,
        low: i32,
        high: i32,
        p: &mut Params,
    ) {
        let l = lay.l;
        let pos = HIST + s * l;
        // Target and impulse response.
        let mut zir = [0f32; 80];
        {
            let mut syn = self.syn;
            let mut iir = self.err_iir;
            for z in zir.iter_mut().take(l) {
                *z = cascade(aq, a1, a2, &mut syn, &mut iir, 0.0);
            }
        }
        let mut h = [0f32; 80];
        {
            let mut syn = [0f32; ORDER];
            let mut iir = [0f32; ORDER];
            for (n, hv) in h.iter_mut().enumerate().take(l) {
                *hv = cascade(
                    aq,
                    a1,
                    a2,
                    &mut syn,
                    &mut iir,
                    if n == 0 { 1.0 } else { 0.0 },
                );
            }
        }
        let mut x = [0f32; 80];
        for n in 0..l {
            x[n] = self.wsp[148 + s * l + n] - zir[n];
        }
        // Lag search.
        let half = 1i32 << (lay.lagd_bits - 1);
        let (wlo, whi) = if first {
            (low, high)
        } else {
            (
                (self.prev_lag - half).max(low),
                (self.prev_lag + half - 1).min(high),
            )
        };
        let mut best_u = i32::MIN;
        let mut best_score = f64::NEG_INFINITY;
        let mut found = false;
        if first {
            for tt in (t0 as i32 - 4)..=(t0 as i32 + 4) {
                let u = tt * lay.res as i32;
                if u < low || u > high {
                    continue;
                }
                let sc = self.lag_score(lay, pos, u, &h, &x).0;
                if !found || sc > best_score {
                    best_score = sc;
                    best_u = u;
                    found = true;
                }
            }
        } else {
            for u in (self.prev_lag - half)..=(self.prev_lag + half - 1) {
                if u < low || u > high || (lay.res == 2 && u % 2 != 0) {
                    continue;
                }
                let sc = self.lag_score(lay, pos, u, &h, &x).0;
                if !found || sc > best_score {
                    best_score = sc;
                    best_u = u;
                    found = true;
                }
            }
        }
        if !found {
            best_u = if first {
                low
            } else {
                self.prev_lag.clamp(low, high)
            };
        } else if lay.res == 2 {
            let w = best_u;
            for c in [w - 1, w + 1] {
                if c < low || c > high || c < wlo || c > whi {
                    continue;
                }
                let sc = self.lag_score(lay, pos, c, &h, &x).0;
                if sc > best_score {
                    best_score = sc;
                    best_u = c;
                }
            }
        }
        let u = best_u;
        p.lag[s] = if first {
            (u - low) as u32
        } else {
            (u - self.prev_lag + half) as u32
        };
        adaptive(&mut self.exc, pos, l, u, lay.res);
        let mut y = [0f32; 80];
        filter_h(&h[..l], &self.exc[pos..pos + l], &mut y[..l]);
        let mut xy = 0f64;
        let mut yy = 0f64;
        for n in 0..l {
            xy += x[n] as f64 * y[n] as f64;
            yy += y[n] as f64 * y[n] as f64;
        }
        yy += 1e-3;
        let gpu = (xy / yy).clamp(0.0, 1.2);
        // Pulse search.
        let (ti, _) = split_lag(u, lay.res);
        let k = lay.pulses[s] as usize;
        let mut pulses = [0i32; 80];
        if k > 0 {
            let mut h2 = h;
            let beta = self.prev_gp.clamp(0.0, 0.8) as f32;
            if ti < l {
                for n in ti..l {
                    h2[n] += beta * h2[n - ti];
                }
            }
            let mut x2 = [0f32; 80];
            let gpf = gpu as f32;
            for n in 0..l {
                x2[n] = x[n] - gpf * y[n];
            }
            pulse_search(&h2[..l], &x2[..l], k, &mut pulses[..l]);
            p.pulse[s] = pvq::encode(&pulses[..l], k);
        }
        let mut c = [0f32; 80];
        sharpen(&pulses[..l], self.prev_gp, ti, &mut c[..l]);
        let mut z = [0f32; 80];
        filter_h(&h[..l], &c[..l], &mut z[..l]);
        // Gain search.
        let e: i64 = pulses[..l].iter().map(|&v| (v as i64) * (v as i64)).sum();
        let ec = if k > 0 { db(e as f64 / l as f64) } else { 0.0 };
        let mut pred = 33.0;
        for i in 0..4 {
            pred += [0.68, 0.58, 0.34, 0.19][i] * self.hist[i];
        }
        let base = if k > 0 {
            pow10((pred - ec) / 20.0)
        } else {
            0.0
        };
        let ngc = 1usize << lay.gc_bits;
        let mut gcv = [0f64; 32];
        for (j, g) in gcv.iter_mut().enumerate().take(ngc) {
            *g = base * pow10(gc_level(lay.gc_bits, j as u32) / 20.0);
        }
        let mut xz = 0f64;
        let mut zz = 0f64;
        let mut yz = 0f64;
        for n in 0..l {
            xz += x[n] as f64 * z[n] as f64;
            zz += z[n] as f64 * z[n] as f64;
            yz += y[n] as f64 * z[n] as f64;
        }
        let mut best = (0u32, 0u32);
        let mut best_err = f64::INFINITY;
        for i in 0..(1u32 << lay.gp_bits) {
            let gp = gp_level(lay.gp_bits, i);
            if gp > 1.0 && self.prev_gp > 1.0 {
                continue;
            }
            for (j, &gc) in gcv.iter().enumerate().take(ngc) {
                let err = gp * gp * yy - 2.0 * gp * xy + gc * gc * zz - 2.0 * gc * xz
                    + 2.0 * gp * gc * yz;
                if err < best_err {
                    best_err = err;
                    best = (i, j as u32);
                }
            }
        }
        p.gp[s] = best.0;
        p.gc[s] = best.1;
        let gp = gp_level(lay.gp_bits, best.0);
        let corr = gc_level(lay.gc_bits, best.1);
        self.hist = [corr, self.hist[0], self.hist[1], self.hist[2]];
        self.prev_gp = gp;
        self.prev_lag = u;
        let gpf = gp as f32;
        let gcf = gcv[best.1 as usize] as f32;
        for n in 0..l {
            let v = guard(gpf * self.exc[pos + n] + gcf * c[n]);
            self.exc[pos + n] = v;
            cascade(aq, a1, a2, &mut self.syn, &mut self.err_iir, v);
        }
    }

    /// The score of lag `u`: xy / sqrt(yy) of the filtered adaptive vector, built in a scratch copy.
    fn lag_score(
        &self,
        lay: &Layout,
        pos: usize,
        u: i32,
        h: &[f32; 80],
        x: &[f32; 80],
    ) -> (f64, f64, f64) {
        let l = lay.l;
        const BACK: usize = 160;
        let mut tmp = [0f32; BACK + 80];
        tmp[..BACK].copy_from_slice(&self.exc[pos - BACK..pos]);
        adaptive(&mut tmp, BACK, l, u, lay.res);
        let mut y = [0f32; 80];
        filter_h(&h[..l], &tmp[BACK..BACK + l], &mut y[..l]);
        let mut xy = 0f64;
        let mut yy = 0f64;
        for n in 0..l {
            xy += x[n] as f64 * y[n] as f64;
            yy += y[n] as f64 * y[n] as f64;
        }
        yy += 1e-3;
        (xy / yy.sqrt(), xy, yy)
    }

    /// The high-band analysis of one half (section 3.8.10). Returns G and the slope for the SID.
    fn hb_half(&mut self, lay: &Layout, h: usize, p: &mut Params) -> (f64, f64) {
        let mut e = [[0f64; 6]; 4];
        let mut tot = [0f64; 4];
        for b in 0..4 {
            let mut buf = [0f32; 160];
            buf[..80].copy_from_slice(&self.hb_hist);
            let blk = h * 4 + b;
            buf[80..].copy_from_slice(&self.hb[blk * 80..blk * 80 + 80]);
            self.hb_hist.copy_from_slice(&buf[80..]);
            let mut x = [0f32; 80];
            self.mdct.forward(&buf, &mut x);
            for j in 0..6 {
                let mut s = 0f64;
                for &xv in &x[EDGES[j]..EDGES[j + 1]] {
                    s += xv as f64 * xv as f64;
                }
                e[b][j] = s / WIDTH[j];
            }
            let mut s = 0f64;
            for &xv in &x[36..80] {
                s += xv as f64 * xv as f64;
            }
            tot[b] = s / 44.0;
        }
        let g = db((tot[0] + tot[1] + tot[2] + tot[3]) / 4.0);
        let nl = 1u32 << lay.hb_level_bits;
        p.hb_level[h] = if g < 1.0 {
            0
        } else {
            nearest(g, 0.0, 96.0 / nl as f64, nl)
        };
        let mut s = [0f64; 6];
        for j in 0..6 {
            s[j] = db((e[0][j] + e[1][j] + e[2][j] + e[3][j]) / 4.0) - g;
        }
        let mut num = 0.0;
        let mut den = 0.0;
        for j in 0..6 {
            num += s[j] * t(j);
            den += t(j) * t(j);
        }
        let slope = num / den;
        match lay.hb_mode {
            0 | 1 => {
                let i0 = nearest(slope, -21.0, 6.0, 8);
                p.hb_shape[h][0] = i0;
                if lay.hb_mode == 1 {
                    let mut m = 0.0;
                    for j in 0..6 {
                        m += t(j) * t(j);
                    }
                    m /= 6.0;
                    let mut n2 = 0.0;
                    let mut d2 = 0.0;
                    for j in 0..6 {
                        let q = t(j) * t(j) - m;
                        n2 += (s[j] - (-21.0 + 6.0 * i0 as f64) * t(j)) * q;
                        d2 += q * q;
                    }
                    p.hb_shape[h][1] = nearest(n2 / d2, -9.0, 6.0, 4);
                }
            }
            2 => {
                for j in 0..6 {
                    p.hb_shape[h][j] = nearest(s[j], -13.5, 6.0, 4);
                }
            }
            _ => {
                for j in 0..6 {
                    p.hb_shape[h][j] = nearest(s[j], -22.0, 4.0, 8);
                }
            }
        }
        if lay.hb_time_bits > 0 {
            let mut tb = [0f64; 4];
            let mut mx = f64::NEG_INFINITY;
            for b in 0..4 {
                tb[b] = db(tot[b]);
                if tb[b] > mx {
                    mx = tb[b];
                }
            }
            for b in 0..4 {
                let d = tb[b] - mx;
                let mut bi = 0;
                for (i, &lv) in TIME_LEVELS.iter().enumerate() {
                    if (d - lv).abs() < (d - TIME_LEVELS[bi]).abs() {
                        bi = i;
                    }
                }
                p.hb_time[h][b] = bi as u32;
            }
        }
        (g, slope)
    }
}

/// floor((v - lo) / step + 0.5) clamped to [0, n - 1].
fn nearest(v: f64, lo: f64, step: f64, n: u32) -> u32 {
    ((v - lo) / step + 0.5).floor().clamp(0.0, (n - 1) as f64) as u32
}

/// The weighted synthesis cascade of section 3.8.6 on one sample.
#[inline]
fn cascade(
    aq: &Lpc,
    a1: &Lpc,
    a2: &Lpc,
    syn: &mut [f32; ORDER],
    iir: &mut [f32; ORDER],
    u: f32,
) -> f32 {
    let mut s = u;
    for k in 1..=ORDER {
        s -= aq[k] * syn[k - 1];
    }
    let mut t = s;
    for k in 1..=ORDER {
        t += a1[k] * syn[k - 1];
    }
    for k in (1..ORDER).rev() {
        syn[k] = syn[k - 1];
    }
    syn[0] = s;
    synth_step(a2, iir, t)
}

/// y[n] = sum over k = 0 to n of h[k] v[n - k], in float.
fn filter_h(h: &[f32], v: &[f32], y: &mut [f32]) {
    for n in 0..y.len() {
        let mut acc = 0f32;
        for k in 0..=n {
            acc += h[k] * v[n - k];
        }
        y[n] = acc;
    }
}

#[inline]
fn sgn(v: f64) -> f64 {
    if v < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// The greedy pulse search with refinement (section 3.8.8 steps 3 to 6).
fn pulse_search(h2: &[f32], x2: &[f32], k: usize, y: &mut [i32]) {
    let l = h2.len();
    let mut d = [0f64; 80];
    for i in 0..l {
        let mut acc = 0f64;
        for n in i..l {
            acc += x2[n] as f64 * h2[n - i] as f64;
        }
        d[i] = acc;
    }
    let mut phi = [[0f64; 80]; 80];
    for dl in 0..l {
        let mut acc = 0f64;
        for i in (0..l - dl).rev() {
            acc += h2[l - 1 - i - dl] as f64 * h2[l - 1 - i] as f64;
            phi[i][i + dl] = acc;
            phi[i + dl][i] = acc;
        }
    }
    y.fill(0);
    let mut g = [0f64; 80];
    let mut cc = 0f64;
    let mut ee = 0f64;
    for _ in 0..k {
        let mut best = 0usize;
        let mut bn = 0f64;
        let mut bd = 1f64;
        for i in 0..l {
            let c = cc + d[i].abs();
            let e = ee + 2.0 * sgn(d[i]) * g[i] + phi[i][i];
            if e <= 0.0 {
                continue;
            }
            if c * c * bd > bn * e {
                bn = c * c;
                bd = e;
                best = i;
            }
        }
        let s = sgn(d[best]);
        cc += d[best].abs();
        ee = ee + 2.0 * s * g[best] + phi[best][best];
        y[best] += s as i32;
        for j in 0..l {
            g[j] += s * phi[best][j];
        }
    }
    for _ in 0..2 {
        let mut moved = false;
        for i in 0..l {
            if y[i] == 0 {
                continue;
            }
            let s = if y[i] < 0 { -1.0 } else { 1.0 };
            let c0 = cc - d[i].abs();
            let e0 = ee - 2.0 * s * g[i] + phi[i][i];
            y[i] -= s as i32;
            for j in 0..l {
                g[j] -= s * phi[i][j];
            }
            let mut best = i;
            let t = c0 + d[i].abs();
            let mut bn = t * t;
            let mut bd = ee;
            for j in 0..l {
                let c = c0 + d[j].abs();
                let e = e0 + 2.0 * sgn(d[j]) * g[j] + phi[j][j];
                if e <= 0.0 {
                    continue;
                }
                if c * c * bd > bn * e * (1.0 + 1e-9) {
                    bn = c * c;
                    bd = e;
                    best = j;
                }
            }
            let sb = sgn(d[best]);
            cc = c0 + d[best].abs();
            ee = e0 + 2.0 * sb * g[best] + phi[best][best];
            y[best] += sb as i32;
            for j in 0..l {
                g[j] += sb * phi[best][j];
            }
            if best != i {
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}
