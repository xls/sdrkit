//! The modes, and the encoder and decoder of whole frames: the bit layouts, the interpolation between internal frames, and
//! the order of the stages.
//!
//! Standard: D. Rowe, "Codec 2 Algorithm Description" (2025), sections 3.7, 3.8 and 4.

use crate::analysis::{Analyser, Model, NLP_TAPS};
use crate::codebooks::{Codebooks, Table};
use crate::consts::{log10f, pow10d, trunc_i, wo_min, LPC_ORD, MAX_AMP, N_SAMP, PI};
use crate::error::Error;
use crate::lpc::{self, LpcSynth, LspTables, WoE};
use crate::newamp1::{Indexes700, RateKDecoder, RateKEncoder};
use crate::synth::Synth;
use sdrkit_algo_dsp::Complex32;
use sdrkit_util_bits::gray::{gray, ungray};
use sdrkit_util_bits::BitReader;

/// A Codec 2 mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// 3200 bit/s: 20 ms frames of 64 bits (M17 voice).
    M3200,
    /// 1300 bit/s: 40 ms frames of 52 bits (FreeDV 1600 voice).
    M1300,
    /// 1200 bit/s: 40 ms frames of 48 bits, with vector-quantised LSPs and a joint pitch and energy quantiser.
    M1200,
    /// 700 bit/s, variant C: 40 ms frames of 28 bits (FreeDV 700C, 700D and 700E voice).
    M700C,
}

const TABLES_3200: [Table; 11] = [
    Table::LspDelta(0),
    Table::LspDelta(1),
    Table::LspDelta(2),
    Table::LspDelta(3),
    Table::LspDelta(4),
    Table::LspDelta(5),
    Table::LspDelta(6),
    Table::LspDelta(7),
    Table::LspDelta(8),
    Table::LspDelta(9),
    Table::NlpFir,
];
const TABLES_1300: [Table; 11] = [
    Table::Lsp(0),
    Table::Lsp(1),
    Table::Lsp(2),
    Table::Lsp(3),
    Table::Lsp(4),
    Table::Lsp(5),
    Table::Lsp(6),
    Table::Lsp(7),
    Table::Lsp(8),
    Table::Lsp(9),
    Table::NlpFir,
];
const TABLES_1200: [Table; 5] = [
    Table::LspVq(0),
    Table::LspVq(1),
    Table::LspVq(2),
    Table::WoE,
    Table::NlpFir,
];
const TABLES_700C: [Table; 4] = [Table::Vq(0), Table::Vq(1), Table::Energy, Table::NlpFir];

impl Mode {
    /// Every mode.
    pub const ALL: [Mode; 4] = [Mode::M3200, Mode::M1300, Mode::M1200, Mode::M700C];

    /// The bits in one frame.
    pub fn bits_per_frame(self) -> usize {
        match self {
            Mode::M3200 => 64,
            Mode::M1300 => 52,
            Mode::M1200 => 48,
            Mode::M700C => 28,
        }
    }

    /// The bytes in one frame. The unused bits at the end of the last byte are zero.
    pub fn bytes_per_frame(self) -> usize {
        self.bits_per_frame().div_ceil(8)
    }

    /// The 8 kHz speech samples in one frame.
    pub fn samples_per_frame(self) -> usize {
        self.internal_frames() * N_SAMP
    }

    /// The tables that the mode needs.
    pub fn tables(self) -> &'static [Table] {
        match self {
            Mode::M3200 => &TABLES_3200,
            Mode::M1300 => &TABLES_1300,
            Mode::M1200 => &TABLES_1200,
            Mode::M700C => &TABLES_700C,
        }
    }

    /// The output gain of the decoder.
    fn gain(self) -> f32 {
        match self {
            Mode::M700C => 1.5,
            _ => 1.0,
        }
    }

    fn internal_frames(self) -> usize {
        match self {
            Mode::M3200 => 2,
            _ => 4,
        }
    }
}

/// The 1300 LSP field widths.
const LSP_BITS_1300: [u32; LPC_ORD] = [4, 4, 4, 4, 4, 4, 4, 3, 3, 2];

/// Writes fields MSB first into an accumulator of up to 64 bits.
struct Packer {
    acc: u64,
    n: u32,
}

impl Packer {
    fn new() -> Self {
        Packer { acc: 0, n: 0 }
    }

    fn put(&mut self, v: u32, bits: u32, gray_coded: bool) {
        let v = if gray_coded { gray(v) } else { v };
        self.acc = (self.acc << bits) | (v as u64 & ((1u64 << bits) - 1));
        self.n += bits;
    }

    fn write(self, out: &mut [u8]) {
        let total = out.len() as u32 * 8;
        let v = (self.acc as u128) << (total - self.n);
        for (i, b) in out.iter_mut().enumerate() {
            *b = (v >> (total - 8 * (i as u32 + 1))) as u8;
        }
    }
}

fn get(r: &mut BitReader<'_>, bits: u32, gray_coded: bool) -> u32 {
    let v = r.u(bits) as u32;
    if gray_coded {
        ungray(v)
    } else {
        v
    }
}

/// The quantiser tables and state of a mode.
enum Quant {
    Delta(LspTables),
    Scalar(LspTables),
    Vq {
        cb: [Box<[f32]>; 3],
        ge: Box<[f32]>,
        woe: WoE,
    },
    RateK,
}

impl Quant {
    fn new(mode: Mode, books: &Codebooks) -> Result<Quant, Error> {
        let set = |f: fn(u8) -> Table| -> Result<LspTables, Error> {
            let mut out: [Box<[f32]>; LPC_ORD] = Default::default();
            for (i, o) in out.iter_mut().enumerate() {
                *o = books.need(f(i as u8))?;
            }
            Ok(out)
        };
        Ok(match mode {
            Mode::M3200 => Quant::Delta(set(Table::LspDelta)?),
            Mode::M1300 => Quant::Scalar(set(Table::Lsp)?),
            Mode::M1200 => Quant::Vq {
                cb: [
                    books.need(Table::LspVq(0))?,
                    books.need(Table::LspVq(1))?,
                    books.need(Table::LspVq(2))?,
                ],
                ge: books.need(Table::WoE)?,
                woe: WoE::new(),
            },
            Mode::M700C => Quant::RateK,
        })
    }
}

fn nlp_taps(books: &Codebooks) -> Result<[f32; NLP_TAPS], Error> {
    let t = books.need(Table::NlpFir)?;
    let mut h = [0.0f32; NLP_TAPS];
    h.copy_from_slice(&t);
    Ok(h)
}

/// The parameters of an internal frame of the LPC modes.
#[derive(Clone, Copy)]
struct LpcFrame {
    wo: f32,
    voiced: bool,
    lsp: [f32; LPC_ORD],
    e: f32,
}

/// `(1 - t) a + t b`. The product `t b` is in `f32`. The rest is in `f64` and rounds once.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    ((1.0 - t as f64) * a as f64 + (t * b) as f64) as f32
}

/// The LSPs at position `t` between two frames.
fn interp_lsp(prev: &[f32; LPC_ORD], next: &[f32; LPC_ORD], t: f32) -> [f32; LPC_ORD] {
    std::array::from_fn(|i| lerp(prev[i], next[i], t))
}

/// The pitch and voicing at position `t` between two frames, for the decoded voicing bit `v`.
fn interp_wo(prev: &LpcFrame, next: &LpcFrame, t: f32, v: bool) -> (f32, bool) {
    let voiced = v && (prev.voiced || next.voiced);
    let wo = if voiced {
        match (prev.voiced, next.voiced) {
            (true, true) => lerp(prev.wo, next.wo, t),
            (false, true) => next.wo,
            _ => prev.wo,
        }
    } else {
        wo_min()
    };
    (wo, voiced)
}

/// The energy by the logarithmic rule at position `t` between two frames. The product `t log10(next)` is in `f32`.
fn interp_log_energy(prev: f32, next: f32, t: f32) -> f32 {
    pow10d((1.0 - t as f64) * log10f(prev) as f64 + (t * log10f(next)) as f64)
}

/// A Codec 2 speech encoder of one mode.
///
/// ```no_run
/// use sdrkit_codec_codec2::{Codebooks, Encoder, Mode};
/// let books = Codebooks::load_dir(std::path::Path::new("codec2-tables")).unwrap();
/// let mut enc = Encoder::new(Mode::M3200, &books).unwrap();
/// let speech = [0i16; 160];
/// let mut frame = [0u8; 8];
/// enc.encode(&speech, &mut frame).unwrap();
/// ```
pub struct Encoder {
    mode: Mode,
    analyser: Analyser,
    model: Model,
    quant: Quant,
    ratek: Option<RateKEncoder>,
}

impl Encoder {
    /// A new encoder of `mode`. It copies the tables that the mode needs. Errors if a table of the mode is missing.
    pub fn new(mode: Mode, books: &Codebooks) -> Result<Self, Error> {
        books.check(mode)?;
        let ratek = if mode == Mode::M700C {
            Some(RateKEncoder::new(
                books.need(Table::Vq(0))?,
                books.need(Table::Vq(1))?,
                books.need(Table::Energy)?,
            ))
        } else {
            None
        };
        Ok(Encoder {
            mode,
            analyser: Analyser::new(nlp_taps(books)?),
            model: Model::new(),
            quant: Quant::new(mode, books)?,
            ratek,
        })
    }

    /// The mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Encodes one frame. `speech` holds [`Mode::samples_per_frame`] samples at 8 kHz. `frame` receives
    /// [`Mode::bytes_per_frame`] bytes. Errors if a length is wrong. Nothing is allocated.
    pub fn encode(&mut self, speech: &[i16], frame: &mut [u8]) -> Result<(), Error> {
        let mode = self.mode;
        check_len("speech input", mode.samples_per_frame(), speech.len())?;
        check_len("frame output", mode.bytes_per_frame(), frame.len())?;
        let mut p = Packer::new();
        let mut lsp = [0.0f32; LPC_ORD];
        if mode == Mode::M1200 {
            for i in 0..4 {
                self.analyse(speech, i);
                p.put(self.model.voiced as u32, 1, true);
                if i % 2 == 1 {
                    let e = lpc::analyse(self.analyser.sn(), self.analyser.window(), &mut lsp);
                    if let Quant::Vq { ge, woe, .. } = &mut self.quant {
                        p.put(woe.encode(ge, self.model.wo, e), 8, true);
                    }
                }
            }
            if let Quant::Vq { cb, .. } = &self.quant {
                let mut idx = [0u32; 3];
                lpc::encode_lsp_vq(&lsp, cb, &mut idx);
                for i in idx {
                    p.put(i, 9, true);
                }
            }
            p.put(0, 1, true);
            p.write(frame);
            return Ok(());
        }

        let nf = mode.internal_frames();
        let mut voiced = [false; 4];
        for (i, v) in voiced.iter_mut().enumerate().take(nf) {
            self.analyse(speech, i);
            *v = self.model.voiced;
        }
        match &self.quant {
            Quant::Delta(t) | Quant::Scalar(t) => {
                let e = lpc::analyse(self.analyser.sn(), self.analyser.window(), &mut lsp);
                for &v in &voiced[..nf] {
                    p.put(v as u32, 1, true);
                }
                p.put(lpc::encode_wo(self.model.wo), 7, true);
                p.put(lpc::encode_energy(e), 5, true);
                let mut idx = [0u32; LPC_ORD];
                if mode == Mode::M3200 {
                    lpc::encode_lsp_delta(&lsp, t, &mut idx);
                    for &i in &idx {
                        p.put(i, 5, true);
                    }
                } else {
                    lpc::encode_lsp_scalar(&lsp, t, &mut idx);
                    for (&i, &b) in idx.iter().zip(&LSP_BITS_1300) {
                        p.put(i, b, true);
                    }
                }
            }
            _ => {
                let ix = match &self.ratek {
                    Some(r) => r.encode(&self.model),
                    None => Indexes700::default(),
                };
                p.put(ix.vq1, 9, false);
                p.put(ix.vq2, 9, false);
                p.put(ix.energy, 4, false);
                p.put(ix.pitch, 6, false);
            }
        }
        p.write(frame);
        Ok(())
    }

    fn analyse(&mut self, speech: &[i16], i: usize) {
        self.analyser
            .analyse(&speech[i * N_SAMP..(i + 1) * N_SAMP], &mut self.model);
    }
}

/// A Codec 2 speech decoder of one mode.
///
/// ```no_run
/// use sdrkit_codec_codec2::{Codebooks, Decoder, Mode};
/// let books = Codebooks::load_dir(std::path::Path::new("codec2-tables")).unwrap();
/// let mut dec = Decoder::new(Mode::M700C, &books).unwrap();
/// let frame = [0u8; 4];
/// let mut speech = [0i16; 320];
/// dec.decode(&frame, &mut speech).unwrap();
/// ```
pub struct Decoder {
    mode: Mode,
    synth: Synth,
    lpc_synth: LpcSynth,
    quant: Quant,
    ratek: Option<RateKDecoder>,
    prev: LpcFrame,
    models: [Model; 4],
    h: [[Complex32; MAX_AMP + 1]; 4],
}

impl Decoder {
    /// A new decoder of `mode`. It copies the tables that the mode needs. Errors if a table of the mode is missing.
    pub fn new(mode: Mode, books: &Codebooks) -> Result<Self, Error> {
        books.check(mode)?;
        let ratek = if mode == Mode::M700C {
            Some(RateKDecoder::new(
                books.need(Table::Vq(0))?,
                books.need(Table::Vq(1))?,
                books.need(Table::Energy)?,
            ))
        } else {
            None
        };
        Ok(Decoder {
            mode,
            synth: Synth::new(mode.gain()),
            lpc_synth: LpcSynth::new(),
            quant: Quant::new(mode, books)?,
            ratek,
            prev: LpcFrame {
                wo: wo_min(),
                voiced: false,
                lsp: std::array::from_fn(|i| (i as f64 * PI / (LPC_ORD + 1) as f64) as f32),
                e: 1.0,
            },
            models: std::array::from_fn(|_| Model::new()),
            h: [[Complex32::new(0.0, 0.0); MAX_AMP + 1]; 4],
        })
    }

    /// The mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Decodes one frame. `frame` holds [`Mode::bytes_per_frame`] bytes. `speech` receives [`Mode::samples_per_frame`]
    /// samples at 8 kHz. Errors if a length is wrong. Any bit pattern decodes. Nothing is allocated.
    pub fn decode(&mut self, frame: &[u8], speech: &mut [i16]) -> Result<(), Error> {
        let mode = self.mode;
        check_len("frame input", mode.bytes_per_frame(), frame.len())?;
        check_len("speech output", mode.samples_per_frame(), speech.len())?;
        let nf = mode.internal_frames();
        let mut r = BitReader::new(frame);
        let mut frames = [self.prev; 4];
        match &mut self.quant {
            Quant::Delta(t) | Quant::Scalar(t) => {
                let mut vbits = [false; 4];
                for v in &mut vbits[..nf] {
                    *v = get(&mut r, 1, true) == 1;
                }
                let wo = lpc::decode_wo(get(&mut r, 7, true));
                let e = lpc::decode_energy(get(&mut r, 5, true));
                let mut idx = [0u32; LPC_ORD];
                let mut lsp = [0.0f32; LPC_ORD];
                if mode == Mode::M3200 {
                    for i in &mut idx {
                        *i = get(&mut r, 5, true);
                    }
                    lpc::decode_lsp_delta(&idx, t, &mut lsp);
                } else {
                    for (i, &b) in idx.iter_mut().zip(&LSP_BITS_1300) {
                        *i = get(&mut r, b, true);
                    }
                    lpc::decode_lsp_scalar(&idx, t, &mut lsp);
                    lpc::order_and_widen(&mut lsp);
                }
                let next = LpcFrame {
                    wo,
                    voiced: vbits[nf - 1],
                    lsp,
                    e,
                };
                let prev = self.prev;
                for i in 0..nf - 1 {
                    let t = (i + 1) as f32 / nf as f32;
                    let (wo, voiced) = interp_wo(&prev, &next, t, vbits[i]);
                    let e = if mode == Mode::M3200 {
                        (prev.e * next.e).sqrt()
                    } else {
                        interp_log_energy(prev.e, next.e, t)
                    };
                    frames[i] = LpcFrame {
                        wo,
                        voiced,
                        lsp: interp_lsp(&prev.lsp, &next.lsp, t),
                        e,
                    };
                }
                frames[nf - 1] = next;
            }
            Quant::Vq { cb, ge, woe } => {
                let v0 = get(&mut r, 1, true) == 1;
                let v1 = get(&mut r, 1, true) == 1;
                let (wo1, e1) = woe.decode(ge, get(&mut r, 8, true));
                let v2 = get(&mut r, 1, true) == 1;
                let v3 = get(&mut r, 1, true) == 1;
                let (wo3, e3) = woe.decode(ge, get(&mut r, 8, true));
                let idx = [
                    get(&mut r, 9, true),
                    get(&mut r, 9, true),
                    get(&mut r, 9, true),
                ];
                let mut lsp = [0.0f32; LPC_ORD];
                lpc::decode_lsp_vq(&idx, cb, &mut lsp);
                lpc::order_and_widen(&mut lsp);
                let prev = self.prev;
                let f3 = LpcFrame {
                    wo: wo3,
                    voiced: v3,
                    lsp,
                    e: e3,
                };
                let f1 = LpcFrame {
                    wo: wo1,
                    voiced: v1,
                    lsp: interp_lsp(&prev.lsp, &lsp, 0.5),
                    e: e1,
                };
                let (wo0, vv0) = interp_wo(&prev, &f1, 0.5, v0);
                let f0 = LpcFrame {
                    wo: wo0,
                    voiced: vv0,
                    lsp: interp_lsp(&prev.lsp, &lsp, 0.25),
                    e: (prev.e * e1).sqrt(),
                };
                let (wo2, vv2) = interp_wo(&f1, &f3, 0.5, v2);
                let f2 = LpcFrame {
                    wo: wo2,
                    voiced: vv2,
                    lsp: interp_lsp(&prev.lsp, &lsp, 0.75),
                    e: (e1 * e3).sqrt(),
                };
                frames = [f0, f1, f2, f3];
            }
            Quant::RateK => {
                let ix = Indexes700 {
                    vq1: get(&mut r, 9, false),
                    vq2: get(&mut r, 9, false),
                    energy: get(&mut r, 4, false),
                    pitch: get(&mut r, 6, false),
                };
                if let Some(rk) = &mut self.ratek {
                    rk.decode(ix, &mut self.models, &mut self.h);
                }
                for i in 0..nf {
                    self.synth.frame(
                        &self.models[i],
                        &self.h[i],
                        &mut speech[i * N_SAMP..(i + 1) * N_SAMP],
                    );
                }
                return Ok(());
            }
        }
        for (i, f) in frames.iter().enumerate().take(nf) {
            let model = &mut self.models[i];
            model.wo = f.wo;
            model.voiced = f.voiced;
            model.l = (trunc_i(PI / f.wo as f64).max(0) as usize).min(MAX_AMP);
            self.lpc_synth.run(&f.lsp, f.e, model, &mut self.h[i]);
            self.synth
                .frame(model, &self.h[i], &mut speech[i * N_SAMP..(i + 1) * N_SAMP]);
        }
        self.prev = frames[nf - 1];
        Ok(())
    }
}

fn check_len(what: &'static str, expected: usize, got: usize) -> Result<(), Error> {
    if expected == got {
        Ok(())
    } else {
        Err(Error::Length {
            what,
            expected,
            got,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packer_writes_msb_first_with_gray_and_zero_tail() {
        let mut p = Packer::new();
        p.put(1, 1, true);
        p.put(5, 3, true); // Gray of 5 is 7.
        p.put(0x3ff, 10, false);
        let mut out = [0u8; 2];
        p.write(&mut out);
        // 1 111 1111111111 00
        assert_eq!(out, [0xff, 0xfc]);
        let mut r = BitReader::new(&out);
        assert_eq!(get(&mut r, 1, true), 1);
        assert_eq!(get(&mut r, 3, true), 5);
        assert_eq!(get(&mut r, 10, false), 0x3ff);
    }

    #[test]
    fn full_64_bit_frame_packs() {
        let mut p = Packer::new();
        for _ in 0..8 {
            p.put(0xa5, 8, false);
        }
        let mut out = [0u8; 8];
        p.write(&mut out);
        assert_eq!(out, [0xa5; 8]);
    }

    #[test]
    fn mode_sizes() {
        let sizes: Vec<_> = Mode::ALL
            .iter()
            .map(|m| {
                (
                    m.bits_per_frame(),
                    m.bytes_per_frame(),
                    m.samples_per_frame(),
                )
            })
            .collect();
        assert_eq!(
            sizes,
            [(64, 8, 160), (52, 7, 320), (48, 6, 320), (28, 4, 320)]
        );
    }
}
