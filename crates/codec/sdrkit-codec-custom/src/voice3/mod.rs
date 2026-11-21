//! The V3 and V1 speech codecs: an MDCT transform coder with band energies and an adaptive binary arithmetic coder, at 16 kHz
//! with 20 ms frames.
//!
//! ```
//! use sdrkit_codec_custom::voice3::*;
//! let v = Variant::V3(V3Rate::R16);
//! let mut enc = Encoder::new(v);
//! let mut dec = Decoder::new(v);
//! let mut frame = [0u8; 40];
//! enc.encode(&[0i16; FRAME_SAMPLES], &mut frame).unwrap();
//! let mut out = [0i16; FRAME_SAMPLES];
//! assert_eq!(dec.decode(Some(&frame), &mut out).unwrap(), Decoded::Good);
//! ```

mod arith;
mod tables;

use crate::Error;
use arith::{ArithDecoder, ArithEncoder, Model};
use std::sync::OnceLock;
use tables::V3_TRAINED;

/// Samples per frame (20 ms at 16 kHz).
pub const FRAME_SAMPLES: usize = 320;

/// The V3 bit rates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum V3Rate {
    /// 12 kbit/s (240 bits per frame, 280 coded bins), profile id 3.
    R12,
    /// 16 kbit/s (320 bits per frame), profile id 0.
    R16,
    /// 24 kbit/s (480 bits per frame), profile id 1.
    R24,
    /// 32 kbit/s (640 bits per frame), profile id 2.
    R32,
}

impl V3Rate {
    /// The profile id in the frame header.
    pub const fn profile_id(self) -> u32 {
        match self {
            V3Rate::R16 => 0,
            V3Rate::R24 => 1,
            V3Rate::R32 => 2,
            V3Rate::R12 => 3,
        }
    }

    /// The rate of a profile id.
    pub const fn from_profile_id(id: u32) -> Option<Self> {
        Some(match id {
            0 => V3Rate::R16,
            1 => V3Rate::R24,
            2 => V3Rate::R32,
            3 => V3Rate::R12,
            _ => return None,
        })
    }

    /// The frame size in bits.
    pub const fn frame_bits(self) -> usize {
        match self {
            V3Rate::R12 => 240,
            V3Rate::R16 => 320,
            V3Rate::R24 => 480,
            V3Rate::R32 => 640,
        }
    }
}

/// Which codec of the family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    /// V3 at one rate.
    V3(V3Rate),
    /// V1 at 16 kbit/s.
    V1,
}

impl Variant {
    /// The frame size in bits.
    pub const fn frame_bits(self) -> usize {
        match self {
            Variant::V3(r) => r.frame_bits(),
            Variant::V1 => 320,
        }
    }

    /// The frame size in bytes.
    pub const fn frame_bytes(self) -> usize {
        self.frame_bits() / 8
    }

    fn params(self) -> Params {
        match self {
            Variant::V3(r) => {
                let (coded, s0, tilt, nh) = match r {
                    V3Rate::R16 => (320, 3.0, 0.8, 0.9),
                    V3Rate::R24 => (320, 3.0, 0.4, 0.8),
                    V3Rate::R32 => (320, 3.0, 0.2, 0.8),
                    V3Rate::R12 => (280, 1.0, 0.4, 0.9),
                };
                Params {
                    v1: false,
                    edges: &V3_EDGES,
                    coded,
                    s0,
                    tilt,
                    nh,
                    e_step: 4.5,
                    e_max: 21,
                    e0_bits: 5,
                    profile: r.profile_id(),
                }
            }
            Variant::V1 => Params {
                v1: true,
                edges: &V1_EDGES,
                coded: 320,
                s0: 3.0,
                tilt: 0.0,
                nh: 0.0,
                e_step: 1.5,
                e_max: 63,
                e0_bits: 6,
                profile: 0,
            },
        }
    }
}

const V3_EDGES: [usize; 27] = [
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 46, 52, 59, 66, 75, 84, 96, 108, 124, 140, 160, 184,
    210, 240, 280, 320,
];
const V1_EDGES: [usize; 17] = [
    0, 4, 8, 12, 16, 20, 26, 32, 40, 52, 66, 84, 108, 140, 184, 240, 320,
];

#[derive(Clone, Copy)]
struct Params {
    v1: bool,
    edges: &'static [usize],
    coded: usize,
    s0: f64,
    tilt: f64,
    nh: f64,
    e_step: f64,
    e_max: i32,
    e0_bits: u32,
    profile: u32,
}

impl Params {
    fn bands(&self) -> usize {
        self.edges.len() - 1
    }
    fn width(&self, b: usize) -> usize {
        self.edges[b + 1] - self.edges[b]
    }
    fn fc(&self, b: usize) -> f64 {
        (self.edges[b] + self.edges[b + 1]) as f64 / 2.0 * 25.0
    }
    fn rms(&self, e: i32) -> f64 {
        10f64.powf(e as f64 * self.e_step / 20.0)
    }
    fn step(&self, b: usize, off: u32, r: f64) -> f64 {
        if self.v1 {
            2f64.powf(off as f64 / 4.0 - 3.0) * r.max(1.0).powf(0.75)
        } else {
            let wt = 2f64.powf(self.tilt * (self.fc(b) - 3500.0).max(0.0) / 1000.0);
            let s = 2f64.powf(off as f64 / 4.0 - self.s0)
                * r.max(1.0).powf(0.5)
                * 100f64.powf(0.25)
                * wt;
            s.max(0.0)
        }
    }
    fn class(&self, r: f64, step: f64) -> usize {
        let lr = (r.max(1.0) / step).log2();
        (lr + 3.0).floor().clamp(0.0, 7.0) as usize
    }
}

/// The models of one frame.
struct Models {
    z: [Model; 3],
    sg: [Model; 3],
    me: [Model; 8],
    // V3: Zc[class][ctx]. V1: Zp[pos][ctx].
    zc: [[Model; 4]; 8],
    // V3: Mc[class][set][i]. V1: Mp[set][i] in row 0.
    mc: [[[Model; 6]; 2]; 8],
}

impl Models {
    fn new(v: Variant) -> Self {
        match v {
            Variant::V3(r) => {
                let t = &V3_TRAINED[r.profile_id() as usize];
                let mut m = Self {
                    z: [Model::new(0, 0); 3],
                    sg: [Model::new(0, 0); 3],
                    me: [Model::new(0, 0); 8],
                    zc: [[Model::new(0, 0); 4]; 8],
                    mc: [[[Model::new(0, 0); 6]; 2]; 8],
                };
                for i in 0..3 {
                    m.z[i] = Model::trained(t[i], 1500);
                    m.sg[i] = Model::trained(t[3 + i], 2048);
                }
                for i in 0..8 {
                    m.me[i] = Model::trained(t[6 + i], 1000);
                }
                for r in 0..8 {
                    for c in 0..4 {
                        let d = if c == 0 {
                            3000 - 300 * r as u32
                        } else {
                            1500 - 150 * r as u32
                        };
                        m.zc[r][c] = Model::trained(t[14 + 4 * r + c], d);
                    }
                    for j in 0..2 {
                        for i in 0..6 {
                            m.mc[r][j][i] =
                                Model::trained(t[46 + 12 * r + 6 * j + i], 1800 - 150 * r as u32);
                        }
                    }
                }
                m
            }
            Variant::V1 => {
                let mut zc = [[Model::new(1200, 0); 4]; 8];
                for row in zc.iter_mut() {
                    row[0] = Model::new(2600, 0);
                }
                Self {
                    z: [Model::new(1500, 0); 3],
                    sg: [Model::new(2048, 0); 3],
                    me: [Model::new(1000, 0); 8],
                    zc,
                    mc: [[[Model::new(1800, 0); 6]; 2]; 8],
                }
            }
        }
    }
}

struct Tables {
    // basis[n * 320 + k]
    basis: Vec<f64>,
    win_v3: Vec<f64>,
    win_v1: Vec<f64>,
}

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let pi = std::f64::consts::PI;
        let scale = (2.0f64 / 320.0).sqrt();
        let mut basis = vec![0f64; 640 * 320];
        for n in 0..640 {
            for k in 0..320 {
                basis[n * 320 + k] =
                    scale * (pi / 320.0 * (n as f64 + 0.5 + 160.0) * (k as f64 + 0.5)).cos();
            }
        }
        let win_v3 = (0..640)
            .map(|n| {
                let s = (pi * (n as f64 + 0.5) / 640.0).sin();
                (pi / 2.0 * s * s).sin()
            })
            .collect();
        let win_v1 = (0..640)
            .map(|n| (pi * (n as f64 + 0.5) / 640.0).sin())
            .collect();
        Tables {
            basis,
            win_v3,
            win_v1,
        }
    })
}

fn window(v: Variant) -> &'static [f64] {
    match v {
        Variant::V3(_) => &tables().win_v3,
        Variant::V1 => &tables().win_v1,
    }
}

fn mdct(x: &[f64; 640], w: &[f64], out: &mut [f64; 320]) {
    let b = &tables().basis;
    *out = [0.0; 320];
    for n in 0..640 {
        let xw = x[n] * w[n];
        let row = &b[n * 320..n * 320 + 320];
        for k in 0..320 {
            out[k] += xw * row[k];
        }
    }
}

fn imdct(x: &[f64; 320], w: &[f64], y: &mut [f64; 640]) {
    let b = &tables().basis;
    for n in 0..640 {
        let row = &b[n * 320..n * 320 + 320];
        let mut acc = 0.0;
        for k in 0..320 {
            acc += x[k] * row[k];
        }
        y[n] = w[n] * acc;
    }
}

/// The V3 and V1 encoder. It codes each 20 ms frame from the previous 320 samples and the current 320 samples.
pub struct Encoder {
    variant: Variant,
    p: Params,
    prev: [f64; 320],
    trial: Vec<u8>,
    best: Vec<u8>,
}

impl Encoder {
    /// Makes an encoder.
    pub fn new(variant: Variant) -> Self {
        let _ = tables();
        Self {
            variant,
            p: variant.params(),
            prev: [0.0; 320],
            trial: Vec::with_capacity(700),
            best: Vec::with_capacity(700),
        }
    }

    /// The frame size in bytes.
    pub fn frame_bytes(&self) -> usize {
        self.variant.frame_bytes()
    }

    /// Encodes 320 samples into one frame of `frame_bytes()` bytes.
    ///
    /// Errors: [`Error::BadInputLength`] when `pcm` is not 320 samples, [`Error::OutputTooShort`] when `out` is too short.
    pub fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<usize, Error> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(Error::BadInputLength {
                need: FRAME_SAMPLES,
            });
        }
        let nb = self.variant.frame_bytes();
        if out.len() < nb {
            return Err(Error::OutputTooShort { need: nb });
        }
        let mut x = [0f64; 640];
        x[..320].copy_from_slice(&self.prev);
        for (d, &s) in x[320..].iter_mut().zip(pcm) {
            *d = s as f64;
        }
        self.prev.copy_from_slice(&x[320..]);
        let mut spec = [0f64; 320];
        mdct(&x, window(self.variant), &mut spec);
        let p = self.p;
        let mut e = [0i32; 26];
        for b in 0..p.bands() {
            let mut s = 0f64;
            for &v in &spec[p.edges[b]..p.edges[b + 1]] {
                s += v * v;
            }
            let v = (10.0 * (s / p.width(b) as f64 + 1e-9).log10() / p.e_step).round_ties_even();
            e[b] = (v as i32).clamp(0, p.e_max);
        }
        let budget = self.variant.frame_bits() - 9;
        let (mut lo, mut hi) = (0i32, 31i32);
        let mut found: Option<u32> = None;
        while lo <= hi {
            let mid = (lo + hi) / 2;
            let used = code_body(
                self.variant,
                &p,
                &spec,
                &e,
                Some(mid as u32),
                &mut self.trial,
            );
            if used <= budget {
                std::mem::swap(&mut self.trial, &mut self.best);
                found = Some(mid as u32);
                hi = mid - 1;
            } else {
                lo = mid + 1;
            }
        }
        let (kind, off) = match found {
            Some(o) => (0u32, o),
            None => {
                code_body(self.variant, &p, &spec, &e, None, &mut self.best);
                (1, 0)
            }
        };
        let o = &mut out[..nb];
        o.fill(0);
        let mut bits = [0u8; 640];
        let mut n = 0;
        let mut put = |v: u32, w: u32| {
            for i in (0..w).rev() {
                bits[n] = ((v >> i) & 1) as u8;
                n += 1;
            }
        };
        if p.v1 {
            put(kind, 4);
        } else {
            put(p.profile, 2);
            put(kind, 2);
        }
        put(off, 5);
        for &b in self.best.iter().take(budget) {
            bits[n] = b;
            n += 1;
        }
        for (i, &b) in bits[..self.variant.frame_bits()].iter().enumerate() {
            o[i / 8] |= b << (7 - i % 8);
        }
        Ok(nb)
    }
}

/// Codes the energies and, with an offset, the coefficients. Returns the bits if the coder finished now. The finished code is
/// in `out`.
fn code_body(
    v: Variant,
    p: &Params,
    spec: &[f64; 320],
    e: &[i32; 26],
    off: Option<u32>,
    out: &mut Vec<u8>,
) -> usize {
    let mut m = Models::new(v);
    let mut c = ArithEncoder::new(out);
    c.raw(e[0] as u32, p.e0_bits);
    let mut prev = e[0];
    let mut pc = 0usize;
    for &eb in e.iter().take(p.bands()).skip(1) {
        let d = eb - prev;
        c.bit(d != 0, &mut m.z[pc]);
        if d != 0 {
            c.bit(d < 0, &mut m.sg[pc]);
            let mag = d.unsigned_abs() - 1;
            for i in 0..20u32 {
                c.bit(mag == i, &mut m.me[i.min(7) as usize]);
                if mag == i {
                    break;
                }
            }
            if mag >= 20 {
                c.raw(mag - 20, 6);
            }
        }
        prev = (prev + d).clamp(0, p.e_max);
        pc = d.unsigned_abs().min(2) as usize;
    }
    if let Some(off) = off {
        let mut q = [0i32; 320];
        let mut class = [0usize; 320];
        for b in 0..p.bands() {
            let r = p.rms(e[b]);
            let step = p.step(b, off, r);
            let cl = if p.v1 { 0 } else { p.class(r, step) };
            for k in p.edges[b]..p.edges[b + 1] {
                class[k] = cl;
                let x = spec[k];
                q[k] = if p.v1 {
                    ((x / step).round_ties_even()).clamp(-269.0, 269.0) as i32
                } else {
                    let a = ((x.abs() / step + 0.4).floor()).min(4109.0) as i32;
                    if x < 0.0 {
                        -a
                    } else if x > 0.0 {
                        a
                    } else {
                        0
                    }
                };
            }
        }
        if !p.v1 {
            let orig = q;
            for k in 120..320 {
                if orig[k].abs() == 1 && orig[(k + 319) % 320] == 0 && orig[(k + 1) % 320] == 0 {
                    q[k] = 0;
                }
            }
        }
        let (mut a1, mut a2) = (0u32, 0u32);
        for k in 0..p.coded {
            let ctx = (2 * a1 + a2).min(3) as usize;
            let row = if p.v1 { pos_class(k) } else { class[k] };
            c.bit(q[k] != 0, &mut m.zc[row][ctx]);
            if q[k] != 0 {
                c.raw(u32::from(q[k] < 0), 1);
                let mag = q[k].unsigned_abs() - 1;
                let j = usize::from(a1 + a2 > 2);
                let mrow = if p.v1 { 0 } else { class[k] };
                for i in 0..14u32 {
                    c.bit(mag == i, &mut m.mc[mrow][j][i.min(5) as usize]);
                    if mag == i {
                        break;
                    }
                }
                if mag >= 14 {
                    if p.v1 {
                        c.raw((mag - 14).min(255), 8);
                    } else {
                        c.raw((mag - 14).min(4095), 12);
                    }
                }
            }
            a2 = a1;
            a1 = q[k].unsigned_abs().min(3);
        }
    }
    let used = c.bits_if_finished();
    c.finish();
    used
}

fn pos_class(k: usize) -> usize {
    if k < 40 {
        0
    } else if k < 100 {
        1
    } else if k < 180 {
        2
    } else {
        3
    }
}

/// The decoder noise generator of section 4.10.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 = (self.0.wrapping_mul(1_103_515_245).wrapping_add(12345)) & 0x7FFF_FFFF;
        (self.0 >> 8) as f64 / 4_194_304.0 - 1.0
    }
}

/// What the decoder made of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Decoded {
    /// A good frame.
    Good,
    /// A lost or bad frame, concealed.
    Concealed,
}

/// The V3 and V1 decoder. Its output is 320 samples behind the encoder input.
pub struct Decoder {
    variant: Variant,
    last: [f64; 320],
    run: u32,
    ola: [f64; 320],
    noise: Noise,
    bits: [u8; 640],
}

impl Decoder {
    /// Makes a decoder for one variant. A V3 decoder accepts only frames of its own rate.
    pub fn new(variant: Variant) -> Self {
        let _ = tables();
        Self {
            variant,
            last: [0.0; 320],
            run: 0,
            ola: [0.0; 320],
            noise: Noise(12345),
            bits: [0; 640],
        }
    }

    /// Decodes one frame into 320 samples. `None` or a frame with a wrong size, a wrong profile, a reserved type or an
    /// implausible body is concealed.
    ///
    /// Errors: [`Error::OutputTooShort`] when `out` is shorter than 320 samples.
    pub fn decode(&mut self, frame: Option<&[u8]>, out: &mut [i16]) -> Result<Decoded, Error> {
        if out.len() < FRAME_SAMPLES {
            return Err(Error::OutputTooShort {
                need: FRAME_SAMPLES,
            });
        }
        let mut spec = [0f64; 320];
        let good = frame.is_some_and(|f| self.decode_spectrum(f, &mut spec));
        if good {
            self.last = spec;
            self.run = 0;
        } else {
            self.run += 1;
            let att = 0.6f64.powi(self.run as i32);
            for k in 0..320 {
                let nz = self.noise.next();
                spec[k] = self.last[k] * att;
                if nz < 0.0 {
                    spec[k] = -spec[k];
                }
            }
        }
        let mut y = [0f64; 640];
        imdct(&spec, window(self.variant), &mut y);
        for n in 0..320 {
            let v = self.ola[n] + y[n];
            self.ola[n] = y[320 + n];
            out[n] = v.round_ties_even().clamp(-32768.0, 32767.0) as i16;
        }
        Ok(if good {
            Decoded::Good
        } else {
            Decoded::Concealed
        })
    }

    fn decode_spectrum(&mut self, frame: &[u8], spec: &mut [f64; 320]) -> bool {
        let v = self.variant;
        let nbits = v.frame_bits();
        if frame.len() != v.frame_bytes() {
            return false;
        }
        for i in 0..nbits {
            self.bits[i] = (frame[i / 8] >> (7 - i % 8)) & 1;
        }
        let bits = &self.bits[..nbits];
        let field = |s: usize, w: usize| {
            bits[s..s + w]
                .iter()
                .fold(0u32, |a, &b| (a << 1) | b as u32)
        };
        let p = v.params();
        if !p.v1 && field(0, 2) != p.profile {
            return false;
        }
        let kind = if p.v1 { field(0, 4) } else { field(2, 2) };
        if kind > 1 {
            return false;
        }
        let off = field(4, 5);
        let body = &bits[9..];
        let mut m = Models::new(v);
        let mut d = ArithDecoder::new(body);
        let mut e = [0i32; 26];
        e[0] = d.raw(p.e0_bits) as i32;
        let mut prev = e[0];
        let mut pc = 0usize;
        for eb in e.iter_mut().take(p.bands()).skip(1) {
            let mut delta = 0i32;
            if d.bit(&mut m.z[pc]) {
                let neg = d.bit(&mut m.sg[pc]);
                let mut mag = 20;
                for i in 0..20u32 {
                    if d.bit(&mut m.me[i.min(7) as usize]) {
                        mag = i;
                        break;
                    }
                }
                if mag == 20 {
                    mag = 20 + d.raw(6);
                }
                delta = mag as i32 + 1;
                if neg {
                    delta = -delta;
                }
            }
            *eb = (prev + delta).clamp(0, p.e_max);
            prev = *eb;
            pc = delta.unsigned_abs().min(2) as usize;
        }
        let mut r = [0f64; 26];
        let mut step = [1f64; 26];
        let mut q = [0i32; 320];
        for b in 0..p.bands() {
            r[b] = p.rms(e[b]);
        }
        if kind == 0 {
            let mut class = [0usize; 320];
            for b in 0..p.bands() {
                step[b] = p.step(b, off, r[b]);
                let cl = if p.v1 { 0 } else { p.class(r[b], step[b]) };
                class[p.edges[b]..p.edges[b + 1]].fill(cl);
            }
            let (mut a1, mut a2) = (0u32, 0u32);
            for k in 0..p.coded {
                let ctx = (2 * a1 + a2).min(3) as usize;
                let row = if p.v1 { pos_class(k) } else { class[k] };
                if d.bit(&mut m.zc[row][ctx]) {
                    let neg = d.raw(1) == 1;
                    let j = usize::from(a1 + a2 > 2);
                    let mrow = if p.v1 { 0 } else { class[k] };
                    let mut mag = 14;
                    for i in 0..14u32 {
                        if d.bit(&mut m.mc[mrow][j][i.min(5) as usize]) {
                            mag = i;
                            break;
                        }
                    }
                    if mag == 14 {
                        mag = 14 + if p.v1 { d.raw(8) } else { d.raw(12) };
                    }
                    let a = mag as i32 + 1;
                    q[k] = if neg { -a } else { a };
                }
                a2 = a1;
                a1 = q[k].unsigned_abs().min(3);
            }
            for b in 0..p.bands() {
                let mut s = 0f64;
                for &qk in &q[p.edges[b]..p.edges[b + 1]] {
                    let x = qk as f64 * step[b];
                    s += x * x;
                }
                if s > 3.0 * r[b] * r[b] * p.width(b) as f64 + 4.0 * step[b] * step[b] {
                    return false;
                }
            }
        }
        if d.bits_read() > body.len() + 34 {
            return false;
        }
        let mut nz = [0f64; 320];
        for v in nz.iter_mut() {
            *v = self.noise.next();
        }
        let sqrt3 = 3f64.sqrt();
        for b in 0..p.bands() {
            let (lo_k, hi_k) = (p.edges[b], p.edges[b + 1]);
            let width = p.width(b);
            let mut sx = 0f64;
            let mut nzc = 0usize;
            for k in lo_k..hi_k {
                spec[k] = q[k] as f64 * step[b];
                sx += spec[k] * spec[k];
                if q[k] == 0 {
                    nzc += 1;
                }
            }
            if nzc == 0 {
                continue;
            }
            let tot = r[b] * r[b] * width as f64;
            let rem = (tot - sx).max(0.0);
            let fc = p.fc(b);
            let mut amp;
            if p.v1 {
                amp = (rem / nzc as f64).sqrt() * 0.7;
                if kind == 0 {
                    amp = amp.min(0.5 * step[b]);
                }
            } else {
                let share = 1.0 - nzc as f64 / width as f64;
                let hf = ((fc - 3000.0) / 1500.0).clamp(0.0, 1.0);
                let fl = 0.5 + (p.nh - 0.5) * hf;
                let cap = 0.5 + 1.5 * hf;
                amp = (rem / nzc as f64).sqrt() * fl * if fc < 3000.0 { 1.0 - share } else { 1.0 };
                if kind == 0 {
                    amp = amp.min(cap * step[b]);
                }
            }
            if !p.v1 && (1..=24).contains(&b) && fc > 3000.0 {
                let lo = (r[b - 1] + 1e-3).ln();
                let hi = (r[b + 1] + 1e-3).ln();
                let mut sl = [0f64; 40];
                let mut ms = 0f64;
                for (i, s) in sl.iter_mut().enumerate().take(width) {
                    let tt = if width == 1 {
                        0.0
                    } else {
                        -0.5 + i as f64 / (width - 1) as f64
                    };
                    *s = (tt * (hi - lo)).exp();
                    ms += *s * *s;
                }
                let nrm = (ms / width as f64).sqrt();
                for (i, k) in (lo_k..hi_k).enumerate() {
                    if q[k] == 0 {
                        spec[k] += nz[k] * amp * sqrt3 * sl[i] / nrm;
                    }
                }
            } else {
                for k in lo_k..hi_k {
                    if q[k] == 0 {
                        spec[k] += nz[k] * amp * sqrt3;
                    }
                }
            }
        }
        true
    }
}
