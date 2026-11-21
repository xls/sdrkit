//! MPEG audio Layer II: the bit allocation tables, the polyphase filterbanks, a decoder and an encoder. The encoder is for test
//! signals: it uses one scale factor per part, a greedy bit allocation and no psychoacoustic model.
//!
//! Standard: ISO/IEC 11172-3 clauses 2.4.1.6, 2.4.2.6, 2.4.3.3 and annexes 3-B and 3-C, ISO/IEC 13818-3 clause 2.4.3.

use crate::window::SYNTH_WINDOW;
use crate::{Error, Header, Result};
use sdrkit_util_bits::crc::catalog::CRC16_CMS;
use sdrkit_util_bits::{BitReader, BitWriter};
use std::sync::OnceLock;

/// Samples per channel in one frame.
pub const FRAME_SAMPLES: usize = 1152;
/// Sub-bands of the filterbank.
pub const SUBBANDS: usize = 32;

/// The quantization classes by their number of steps (ISO/IEC 11172-3 table 3-B.4).
const LEVELS: [u32; 18] = [
    0, 3, 5, 7, 9, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 16383, 32767, 65535,
];

/// Whether a class of `levels` steps is grouped, and the bits of one code (of a group of three when grouped). Table 3-B.4.
fn class_bits(levels: u32) -> (bool, usize) {
    match levels {
        3 => (true, 5),
        5 => (true, 7),
        9 => (true, 10),
        l => (false, (l + 1).trailing_zeros() as usize),
    }
}

const Q_A0: [u32; 16] = [
    0, 3, 7, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 16383, 32767, 65535,
];
const Q_A1: [u32; 16] = [
    0, 3, 5, 7, 9, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 65535,
];
const Q_A2: [u32; 8] = [0, 3, 5, 7, 9, 15, 31, 65535];
const Q_A3: [u32; 4] = [0, 3, 5, 65535];
const Q_C0: [u32; 16] = [
    0, 3, 5, 9, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 16383, 32767,
];
const Q_C1: [u32; 8] = [0, 3, 5, 9, 15, 31, 63, 127];
const Q_L2: [u32; 4] = [0, 3, 5, 9];

/// The allocation table of a header: per sub-band the width of the allocation field and the steps of each allocation code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocTable {
    /// Sub-bands coded (sblimit).
    pub sblimit: usize,
    /// Per sub-band: bits of the allocation field (nbal).
    pub nbal: [u8; SUBBANDS],
    /// Per sub-band: the steps of each allocation code.
    pub levels: [&'static [u32]; SUBBANDS],
}

impl AllocTable {
    /// The table for a header: ISO/IEC 11172-3 tables 3-B.2a to 3-B.2d for MPEG-1, ISO/IEC 13818-3 table B.1 for the half
    /// sampling rates.
    pub fn for_header(h: &Header) -> AllocTable {
        let mut t = AllocTable {
            sblimit: 0,
            nbal: [0; SUBBANDS],
            levels: [&[]; SUBBANDS],
        };
        let mut push = |n: usize, b: u8, q: &'static [u32]| {
            for _ in 0..n {
                t.nbal[t.sblimit] = b;
                t.levels[t.sblimit] = q;
                t.sblimit += 1;
            }
        };
        if !h.mpeg1 {
            push(4, 4, &Q_C0);
            push(7, 3, &Q_C1);
            push(19, 2, &Q_L2);
        } else {
            let per_ch = h.bitrate_kbps as usize / h.channels();
            if per_ch < 56 {
                // Tables 3-B.2c and 3-B.2d.
                push(2, 4, &Q_C0);
                push(if h.sample_rate == 32_000 { 10 } else { 6 }, 3, &Q_C1);
            } else {
                // Tables 3-B.2a and 3-B.2b.
                push(3, 4, &Q_A0);
                push(8, 4, &Q_A1);
                push(12, 3, &Q_A2);
                let last = if per_ch >= 96 && h.sample_rate != 48_000 {
                    7
                } else {
                    4
                };
                push(last, 2, &Q_A3);
            }
        }
        t
    }

    /// Bits of all allocation fields of one channel.
    fn alloc_bits(&self) -> usize {
        self.nbal[..self.sblimit].iter().map(|&b| b as usize).sum()
    }
}

/// The scale factor of index `i`: 2^(1 - i/3) (table 3-B.1).
fn scalefactor(i: u32) -> f32 {
    2f32.powf(1.0 - i as f32 / 3.0)
}

/// The synthesis matrix N[i][k] = cos((16 + i)(2k + 1) pi / 64) (clause 3-A.2, figure 3-A.2).
fn synthesis_matrix() -> &'static [f32] {
    static N: OnceLock<Box<[f32]>> = OnceLock::new();
    N.get_or_init(|| {
        let mut n = vec![0f32; 64 * 32];
        for i in 0..64 {
            for k in 0..32 {
                n[i * 32 + k] =
                    (((16 + i) * (2 * k + 1)) as f64 * std::f64::consts::PI / 64.0).cos() as f32;
            }
        }
        n.into_boxed_slice()
    })
}

/// The analysis matrix M[i][k] = cos((2i + 1)(k - 16) pi / 64) (annex 3-C, figure 3-C.1).
fn analysis_matrix() -> &'static [f32] {
    static M: OnceLock<Box<[f32]>> = OnceLock::new();
    M.get_or_init(|| {
        let mut m = vec![0f32; 32 * 64];
        for i in 0..32 {
            for k in 0..64 {
                m[i * 64 + k] = (((2 * i + 1) as f64) * (k as f64 - 16.0) * std::f64::consts::PI
                    / 64.0)
                    .cos() as f32;
            }
        }
        m.into_boxed_slice()
    })
}

/// The polyphase synthesis filterbank of one channel (ISO/IEC 11172-3 figure 3-A.2).
#[derive(Clone, Debug)]
pub struct Synthesis {
    v: [f32; 1024],
    pos: usize,
}

impl Default for Synthesis {
    fn default() -> Self {
        Synthesis {
            v: [0.0; 1024],
            pos: 0,
        }
    }
}

impl Synthesis {
    /// 32 sub-band samples in, 32 output samples out.
    pub fn run(&mut self, s: &[f32; SUBBANDS], out: &mut [f32; SUBBANDS]) {
        let n = synthesis_matrix();
        self.pos = (self.pos + 1024 - 64) % 1024;
        for i in 0..64 {
            let row = &n[i * 32..(i + 1) * 32];
            self.v[(self.pos + i) % 1024] = row.iter().zip(s).map(|(a, b)| a * b).sum();
        }
        for (j, o) in out.iter_mut().enumerate() {
            let mut acc = 0f32;
            for i in 0..8 {
                acc += self.v[(self.pos + 128 * i + j) % 1024] * SYNTH_WINDOW[64 * i + j];
                acc += self.v[(self.pos + 128 * i + 96 + j) % 1024] * SYNTH_WINDOW[64 * i + 32 + j];
            }
            *o = acc;
        }
    }
}

/// The polyphase analysis filterbank of one channel (ISO/IEC 11172-3 figure 3-C.4).
#[derive(Clone, Debug)]
pub struct Analysis {
    x: [f32; 512],
}

impl Default for Analysis {
    fn default() -> Self {
        Analysis { x: [0.0; 512] }
    }
}

impl Analysis {
    /// 32 new input samples (oldest first) in, 32 sub-band samples out.
    pub fn run(&mut self, input: &[f32; SUBBANDS], s: &mut [f32; SUBBANDS]) {
        let m = analysis_matrix();
        self.x.copy_within(0..480, 32);
        for (i, &v) in input.iter().enumerate() {
            self.x[31 - i] = v;
        }
        let mut y = [0f32; 64];
        for (i, yv) in y.iter_mut().enumerate() {
            let mut acc = 0f32;
            for j in 0..8 {
                acc += self.x[i + 64 * j] * SYNTH_WINDOW[i + 64 * j] / 32.0;
            }
            *yv = acc;
        }
        for (i, sv) in s.iter_mut().enumerate() {
            let row = &m[i * 64..(i + 1) * 64];
            *sv = row.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
        }
    }
}

/// The CRC-16 of a Layer II frame over header bits 16 to 31 and frame bits `from` to `to` (ISO/IEC 11172-3 clause 2.4.3.1:
/// generator 0x8005, start value 0xFFFF, CRC-16/CMS).
pub fn header_crc(frame: &[u8], from: usize, to: usize) -> u16 {
    let mut d = CRC16_CMS.digest();
    let bit = |i: usize| frame.get(i / 8).map_or(0, |b| (b >> (7 - i % 8)) & 1);
    for i in (16..32).chain(from..to) {
        d.update_bits(&[bit(i)]);
    }
    d.finalize()
}

/// A Layer II decoder of mono or stereo frames. It allocates nothing after [`Decoder::new`].
#[derive(Clone, Debug, Default)]
pub struct Decoder {
    synth: [Synthesis; 2],
}

impl Decoder {
    /// A decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes one frame and appends 1152 sample pairs to `out` as interleaved stereo. A mono frame provides the same sample on
    /// both channels. Returns the header.
    ///
    /// # Errors
    /// The errors of [`Header::parse`], [`Error::NotLayer2`] for another layer, [`Error::Truncated`] when the frame is shorter
    /// than its header says, [`Error::Crc`] when the CRC is present and does not match, and [`Error::ScaleFactor`] for a scale
    /// factor index of 63.
    pub fn decode(&mut self, frame: &[u8], out: &mut Vec<f32>) -> Result<Header> {
        let h = Header::parse(frame)?;
        if h.layer != 2 {
            return Err(Error::NotLayer2);
        }
        // A frame without padding can be one byte short of a padded length (DAB logical frames).
        if frame.len() < h.frame_len().saturating_sub(1) {
            return Err(Error::Truncated);
        }
        let t = AllocTable::for_header(&h);
        let nch = h.channels();
        let bound = if h.mode == 1 {
            (4 + 4 * h.mode_extension as usize).min(t.sblimit)
        } else {
            t.sblimit
        };
        let mut r = BitReader::new(&frame[4..]);
        if h.crc {
            r.skip(16);
        }
        let protected_from = r.pos();
        let mut alloc = [[0u32; SUBBANDS]; 2];
        for sb in 0..t.sblimit {
            let nb = t.nbal[sb] as u32;
            let levels = t.levels[sb];
            if sb < bound {
                for a in alloc.iter_mut().take(nch) {
                    a[sb] = levels[r.u(nb) as usize];
                }
            } else {
                let v = levels[r.u(nb) as usize];
                alloc[0][sb] = v;
                alloc[1][sb] = v;
            }
        }
        let mut scfsi = [[0u8; SUBBANDS]; 2];
        for sb in 0..t.sblimit {
            for ch in 0..nch {
                if alloc[ch][sb] != 0 {
                    scfsi[ch][sb] = r.u(2) as u8;
                }
            }
        }
        if r.overrun() {
            return Err(Error::Truncated);
        }
        // The CRC covers header bits 16 to 31 and the bit allocation and scfsi fields (clause 2.4.3.1).
        if h.crc
            && header_crc(frame, 32 + protected_from, 32 + r.pos())
                != u16::from_be_bytes([frame[4], frame[5]])
        {
            return Err(Error::Crc);
        }
        let mut scf = [[[0f32; 3]; SUBBANDS]; 2];
        for sb in 0..t.sblimit {
            for ch in 0..nch {
                if alloc[ch][sb] == 0 {
                    continue;
                }
                let mut rd = || -> Result<u32> {
                    match r.u(6) as u32 {
                        63 => Err(Error::ScaleFactor),
                        v => Ok(v),
                    }
                };
                // Table 3-B.3 transmission patterns: scfsi 0 sends three, 1 and 3 send two, 2 sends one.
                let s = match scfsi[ch][sb] {
                    0 => [rd()?, rd()?, rd()?],
                    1 => {
                        let a = rd()?;
                        let b = rd()?;
                        [a, a, b]
                    }
                    2 => {
                        let a = rd()?;
                        [a, a, a]
                    }
                    _ => {
                        let a = rd()?;
                        let b = rd()?;
                        [a, b, b]
                    }
                };
                for p in 0..3 {
                    scf[ch][sb][p] = scalefactor(s[p]);
                }
            }
        }
        let start = out.len();
        out.reserve(FRAME_SAMPLES * 2);
        let mut pcm = [[0f32; SUBBANDS]; 2];
        for gr in 0..12 {
            let part = gr / 4;
            let mut sb_samples = [[[0f32; SUBBANDS]; 2]; 3];
            for sb in 0..t.sblimit {
                let chans = if sb < bound { nch } else { 1 };
                for ch in 0..chans {
                    let levels = alloc[ch][sb];
                    if levels == 0 {
                        continue;
                    }
                    let (grouped, bits) = class_bits(levels);
                    let mut c = [0u32; 3];
                    if grouped {
                        let mut v = r.u(bits as u32) as u32;
                        for cv in c.iter_mut() {
                            *cv = v % levels;
                            v /= levels;
                        }
                    } else {
                        for cv in c.iter_mut() {
                            *cv = r.u(bits as u32) as u32;
                        }
                    }
                    for k in 0..3 {
                        // Requantization (clause 2.4.3.3.4): s'' = (2c - (steps - 1)) / steps, then times the scale factor.
                        let x = (2.0 * c[k] as f32 - (levels - 1) as f32) / levels as f32;
                        if sb < bound {
                            sb_samples[k][ch][sb] = x * scf[ch][sb][part];
                        } else {
                            for (cc, s) in sb_samples[k].iter_mut().enumerate().take(nch) {
                                s[sb] = x * scf[cc][sb][part];
                            }
                        }
                    }
                }
            }
            for granule in &sb_samples {
                for ch in 0..nch {
                    self.synth[ch].run(&granule[ch], &mut pcm[ch]);
                }
                let right = if nch == 2 { pcm[1] } else { pcm[0] };
                for (l, r) in pcm[0].iter().zip(right) {
                    out.push(*l);
                    out.push(r);
                }
            }
        }
        if r.overrun() {
            out.truncate(start);
            return Err(Error::Truncated);
        }
        Ok(h)
    }
}

/// The SNR of each quantization class in dB (table 3-C.5), for the encoder's allocation.
const CLASS_SNR: [f32; 18] = [
    0.0, 7.0, 11.0, 16.0, 20.84, 25.28, 31.59, 37.75, 43.84, 49.89, 55.93, 61.96, 67.98, 74.01,
    80.03, 86.05, 92.01, 98.01,
];

/// The scale factor index of one part of one sub-band, in the order of the bit stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScaleFactor {
    /// Channel: 0 or 1.
    pub channel: u8,
    /// Sub-band: 0 to 31.
    pub subband: u8,
    /// Index: 0 to 62.
    pub index: u8,
}

/// A Layer II encoder for test signals: one frame from 1152 samples per channel. It allocates nothing after
/// [`Encoder::new`] once `frame` has the capacity of one frame.
#[derive(Clone, Debug)]
pub struct Encoder {
    header: Header,
    table: AllocTable,
    analysis: [Analysis; 2],
    scf: [ScaleFactor; 2 * SUBBANDS * 3],
    n_scf: usize,
}

impl Encoder {
    /// An encoder at `bitrate_kbps`, `sample_rate` Hz, mono or stereo, with a CRC. Rates of 32, 44.1 and 48 kHz give MPEG-1,
    /// and 16, 22.05 and 24 kHz give MPEG-2.
    ///
    /// # Errors
    /// [`Error::BitRate`] or [`Error::SampleRate`] when Layer II has no code for the value.
    pub fn new(bitrate_kbps: u16, sample_rate: u32, stereo: bool) -> Result<Self> {
        let header = Header {
            mpeg1: matches!(sample_rate, 48_000 | 44_100 | 32_000),
            layer: 2,
            crc: true,
            bitrate_kbps,
            sample_rate,
            padding: false,
            private: false,
            mode: if stereo { 0 } else { 3 },
            mode_extension: 0,
            copyright: false,
            original_copy: true,
            emphasis: 0,
        };
        header.bytes()?;
        Ok(Encoder {
            table: AllocTable::for_header(&header),
            header,
            analysis: Default::default(),
            scf: [ScaleFactor::default(); 2 * SUBBANDS * 3],
            n_scf: 0,
        })
    }

    /// The header of every frame.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The scale factors of the last frame, in the order of the bit stream.
    pub fn scale_factors(&self) -> &[ScaleFactor] {
        &self.scf[..self.n_scf]
    }

    /// Encodes 1152 samples per channel into `frame` (cleared first). `right` is ignored for mono. `reserve` bytes at the end
    /// stay zero for data that the carrier adds (DAB: X-PAD, ScF-CRC and F-PAD).
    ///
    /// # Errors
    /// [`Error::InputLength`] when a channel has fewer than 1152 samples.
    pub fn encode(
        &mut self,
        left: &[f32],
        right: &[f32],
        reserve: usize,
        frame: &mut Vec<u8>,
    ) -> Result<()> {
        let h = self.header;
        let t = self.table;
        let nch = h.channels();
        if left.len() < FRAME_SAMPLES || (nch == 2 && right.len() < FRAME_SAMPLES) {
            return Err(Error::InputLength);
        }
        let mut sbs = [[[0f32; SUBBANDS]; 36]; 2];
        for (ch, (bank, a)) in sbs
            .iter_mut()
            .zip(self.analysis.iter_mut())
            .enumerate()
            .take(nch)
        {
            let x = if ch == 0 { left } else { right };
            for (s, chunk) in bank.iter_mut().zip(x.as_chunks::<SUBBANDS>().0) {
                a.run(chunk, s);
            }
        }
        // Scale factors per part of 12 samples, and the energy of each sub-band.
        let mut sfi = [[[0u8; 3]; SUBBANDS]; 2];
        let mut energy = [[0f32; SUBBANDS]; 2];
        for ch in 0..nch {
            for sb in 0..t.sblimit {
                for p in 0..3 {
                    let m = (0..12)
                        .map(|i| sbs[ch][p * 12 + i][sb].abs())
                        .fold(0f32, f32::max);
                    let mut idx = 62u8;
                    while idx > 0 && scalefactor(idx as u32) < m {
                        idx -= 1;
                    }
                    sfi[ch][sb][p] = idx;
                }
                energy[ch][sb] = (0..36).map(|i| sbs[ch][i][sb].powi(2)).sum::<f32>() / 36.0;
            }
        }
        let mut scfsi = [[0u8; SUBBANDS]; 2];
        for ch in 0..nch {
            for sb in 0..t.sblimit {
                let s = sfi[ch][sb];
                scfsi[ch][sb] = if s[0] == s[1] && s[1] == s[2] { 2 } else { 0 };
            }
        }
        // Greedy bit allocation: the sub-band with the most noise gets the next class while bits remain.
        let total = h.frame_len() * 8;
        let fixed = 32 + 16 + nch * t.alloc_bits() + reserve * 8;
        let mut avail = total as i64 - fixed as i64;
        let mut code = [[0usize; SUBBANDS]; 2];
        let cost_of = |l: u32| -> i64 {
            if l == 0 {
                0
            } else {
                let (g, b) = class_bits(l);
                12 * if g { b } else { 3 * b } as i64
            }
        };
        loop {
            let mut best: Option<(f32, usize, usize, i64)> = None;
            for ch in 0..nch {
                for sb in 0..t.sblimit {
                    let c = code[ch][sb];
                    if c + 1 >= t.levels[sb].len() || energy[ch][sb] < 1e-12 {
                        continue;
                    }
                    let lv = t.levels[sb][c];
                    let mut cost = cost_of(t.levels[sb][c + 1]) - cost_of(lv);
                    if lv == 0 {
                        cost += 2 + if scfsi[ch][sb] == 2 { 6 } else { 18 };
                    }
                    if cost > avail {
                        continue;
                    }
                    let snr = CLASS_SNR[LEVELS.iter().position(|&x| x == lv).unwrap_or(0)];
                    let noise = 10.0 * energy[ch][sb].log10() - snr;
                    if best.is_none_or(|b| noise > b.0) {
                        best = Some((noise, ch, sb, cost));
                    }
                }
            }
            match best {
                Some((noise, ch, sb, cost)) if noise > -140.0 => {
                    code[ch][sb] += 1;
                    avail -= cost;
                }
                _ => break,
            }
        }
        frame.clear();
        let mut w = BitWriter::new(frame);
        // The header is valid: `new` made sure of it.
        w.bytes(&h.bytes().unwrap_or_default());
        w.put(0, 16);
        for sb in 0..t.sblimit {
            for c in code.iter().take(nch) {
                w.put(c[sb] as u64, t.nbal[sb] as u32);
            }
        }
        let mut crc_end = 48 + nch * t.alloc_bits();
        for sb in 0..t.sblimit {
            for ch in 0..nch {
                if code[ch][sb] != 0 {
                    w.put(scfsi[ch][sb] as u64, 2);
                    crc_end += 2;
                }
            }
        }
        self.n_scf = 0;
        for sb in 0..t.sblimit {
            for ch in 0..nch {
                if code[ch][sb] == 0 {
                    continue;
                }
                let s = sfi[ch][sb];
                let parts: &[usize] = if scfsi[ch][sb] == 2 { &[0] } else { &[0, 1, 2] };
                for &p in parts {
                    w.put(s[p] as u64, 6);
                    self.scf[self.n_scf] = ScaleFactor {
                        channel: ch as u8,
                        subband: sb as u8,
                        index: s[p],
                    };
                    self.n_scf += 1;
                }
            }
        }
        for gr in 0..12 {
            let part = gr / 4;
            for sb in 0..t.sblimit {
                for ch in 0..nch {
                    let c = code[ch][sb];
                    if c == 0 {
                        continue;
                    }
                    let levels = t.levels[sb][c];
                    let scf = scalefactor(sfi[ch][sb][part] as u32);
                    let q: [u32; 3] = std::array::from_fn(|k| {
                        let x = (sbs[ch][gr * 3 + k][sb] / scf).clamp(-1.0, 1.0);
                        ((x * levels as f32 + (levels - 1) as f32) / 2.0)
                            .round()
                            .clamp(0.0, (levels - 1) as f32) as u32
                    });
                    let (grouped, bits) = class_bits(levels);
                    if grouped {
                        w.put((q[0] + levels * (q[1] + levels * q[2])) as u64, bits as u32);
                    } else {
                        for v in q {
                            w.put(v as u64, bits as u32);
                        }
                    }
                }
            }
        }
        w.finish();
        frame.resize(h.frame_len(), 0);
        let crc = header_crc(frame, 48, crc_end);
        frame[4..6].copy_from_slice(&crc.to_be_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(b: [u8; 4]) -> Header {
        Header::parse(&b).unwrap()
    }

    #[test]
    fn allocation_tables_b2a_to_b2d_and_13818_b1() {
        // Table 3-B.2a: 48 kHz, 128 kbit/s stereo (64 per channel): sblimit 27.
        let t = AllocTable::for_header(&header([0xFF, 0xFD, 0x84, 0x04]));
        assert_eq!(t.sblimit, 27);
        assert_eq!((t.nbal[0], t.nbal[3], t.nbal[11], t.nbal[23]), (4, 4, 3, 2));
        // Table 3-B.2b: 44.1 kHz, 192 kbit/s stereo (96 per channel): sblimit 30.
        let t = AllocTable::for_header(&header([0xFF, 0xFD, 0xA0, 0x04]));
        assert_eq!(t.sblimit, 30);
        // Table 3-B.2c: 48 kHz, 64 kbit/s stereo (32 per channel): sblimit 8.
        let t = AllocTable::for_header(&header([0xFF, 0xFD, 0x44, 0x04]));
        assert_eq!(t.sblimit, 8);
        // Table 3-B.2d: 32 kHz, 64 kbit/s stereo: sblimit 12.
        let t = AllocTable::for_header(&header([0xFF, 0xFD, 0x48, 0x04]));
        assert_eq!(t.sblimit, 12);
        // ISO/IEC 13818-3 table B.1: 24 kHz, any rate: sblimit 30, nbal 4, 3 and 2.
        let t = AllocTable::for_header(&header([0xFF, 0xF5, 0x84, 0xC4]));
        assert_eq!(t.sblimit, 30);
        assert_eq!((t.nbal[3], t.nbal[4], t.nbal[10], t.nbal[11]), (4, 3, 3, 2));
    }

    #[test]
    fn grouped_classes_of_table_3_b_4() {
        assert_eq!(class_bits(3), (true, 5));
        assert_eq!(class_bits(5), (true, 7));
        assert_eq!(class_bits(9), (true, 10));
        assert_eq!(class_bits(7), (false, 3));
        assert_eq!(class_bits(65535), (false, 16));
    }

    #[test]
    fn crc_covers_the_side_information() {
        let mut enc = Encoder::new(128, 48_000, true).unwrap();
        let tone: Vec<f32> = (0..FRAME_SAMPLES)
            .map(|i| (i as f32 * 0.05).sin() * 0.3)
            .collect();
        let mut frame = Vec::new();
        enc.encode(&tone, &tone, 6, &mut frame).unwrap();
        let mut out = Vec::new();
        assert!(Decoder::new().decode(&frame, &mut out).is_ok());
        // A bit of the bit allocation, which the CRC covers.
        frame[6] ^= 0x10;
        assert_eq!(Decoder::new().decode(&frame, &mut out), Err(Error::Crc));
    }

    #[test]
    fn refuses_bad_input() {
        let mut out = Vec::new();
        let mut d = Decoder::new();
        assert_eq!(d.decode(&[0xFF], &mut out), Err(Error::Truncated));
        // Layer III.
        assert_eq!(
            d.decode(&[0xFF, 0xFB, 0x90, 0x64], &mut out),
            Err(Error::NotLayer2)
        );
        // A Layer II header without its frame.
        assert_eq!(
            d.decode(&[0xFF, 0xFD, 0x84, 0x04, 0xFF], &mut out),
            Err(Error::Truncated)
        );
        assert!(out.is_empty());
        assert_eq!(Encoder::new(130, 48_000, true).err(), Some(Error::BitRate));
        assert_eq!(
            Encoder::new(128, 8_000, true).err(),
            Some(Error::SampleRate)
        );
        let mut e = Encoder::new(128, 48_000, true).unwrap();
        assert_eq!(
            e.encode(&[0.0; 10], &[0.0; 10], 0, &mut Vec::new()),
            Err(Error::InputLength)
        );
    }

    #[test]
    fn encode_then_decode_a_tone() {
        for (rate, stereo, len) in [(48_000, true, 384), (24_000, false, 384)] {
            let bitrate = if stereo { 128 } else { 64 };
            let mut enc = Encoder::new(bitrate, rate, stereo).unwrap();
            assert_eq!(
                Header::parse(&enc.header().bytes().unwrap()).unwrap(),
                *enc.header()
            );
            let mut dec = Decoder::new();
            let n = FRAME_SAMPLES * 12;
            let f = rate as f32;
            let l: Vec<f32> = (0..n)
                .map(|i| 0.5 * (std::f32::consts::TAU * 1500.0 * i as f32 / f).sin())
                .collect();
            let r: Vec<f32> = (0..n)
                .map(|i| 0.5 * (std::f32::consts::TAU * 750.0 * i as f32 / f).sin())
                .collect();
            let (mut out, mut frame) = (Vec::new(), Vec::new());
            for k in 0..12 {
                let s = k * FRAME_SAMPLES..(k + 1) * FRAME_SAMPLES;
                enc.encode(&l[s.clone()], &r[s], 6, &mut frame).unwrap();
                assert_eq!(frame.len(), len);
                dec.decode(&frame, &mut out).unwrap();
            }
            // The filterbanks delay by 481 samples: compare the level of the left tone in the second half.
            let left: Vec<f32> = out.iter().step_by(2).copied().skip(n / 2).collect();
            let rms = (left.iter().map(|x| x * x).sum::<f32>() / left.len() as f32).sqrt();
            assert!((rms - 0.5 / 2f32.sqrt()).abs() < 0.02, "{rate}: rms {rms}");
        }
    }
}
