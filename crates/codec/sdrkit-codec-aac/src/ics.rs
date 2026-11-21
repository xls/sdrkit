//! The individual channel stream of AAC-LC: ICS info, section data, scale factors, pulse data, TNS data and the spectral
//! data, read into quantized values and then into the scaled spectrum.
//!
//! Standard: ISO/IEC 14496-3 clauses 4.4.2.7 (syntax), 4.6.1 to 4.6.3 (quantization, scale factors, noiseless coding),
//! 4.6.5 (pulse data) and 4.6.9 (TNS).

use crate::huffman::Book;
use crate::tables::{SPECTRAL, SWB_LONG, SWB_SHORT, TNS_MAX_LONG, TNS_MAX_SHORT};
use crate::{Error, Result};
use sdrkit_util_bits::BitReader;
use std::sync::OnceLock;

/// The most scale factor bands of one window.
pub(crate) const MAX_SFB: usize = 52;
/// The most TNS filter coefficients.
const MAX_ORDER: usize = 32;
/// Codebook of noise bands (PNS).
pub(crate) const NOISE_HCB: u8 = 13;
/// Codebook of intensity bands, out of phase.
pub(crate) const INTENSITY_HCB2: u8 = 14;
/// Codebook of intensity bands, in phase.
pub(crate) const INTENSITY_HCB: u8 = 15;

/// The Huffman books of the AAC-LC syntax, built once per process.
pub(crate) struct Books {
    pub spectral: [Book; 11],
    pub scf: Book,
}

/// The shared books.
pub(crate) fn books() -> &'static Books {
    static B: OnceLock<Books> = OnceLock::new();
    B.get_or_init(|| Books {
        spectral: std::array::from_fn(|i| {
            let (c, l) = SPECTRAL[i];
            Book::new(c.len(), |k| (u32::from(c[k]), l[k]))
        }),
        scf: Book::new(121, |k| {
            (crate::tables::SCF_CODE[k], crate::tables::SCF_LEN[k])
        }),
    })
}

/// `|q|^(4/3)` for `q` up to 8191 (clause 4.6.1.3).
fn pow43() -> &'static [f32; 8192] {
    static T: OnceLock<Box<[f32; 8192]>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Box::new([0f32; 8192]);
        for (i, v) in t.iter_mut().enumerate() {
            *v = (i as f64).powf(4.0 / 3.0) as f32;
        }
        t
    })
}

/// The scale factor band offsets and TNS limits of one sampling rate and frame length.
#[derive(Clone, Debug)]
pub(crate) struct Bands {
    /// Samples per channel of a frame (1024 or 960).
    pub frame: usize,
    long: [u16; MAX_SFB + 1],
    n_long: usize,
    short: [u16; 17],
    n_short: usize,
    tns_long: usize,
    tns_short: usize,
}

impl Bands {
    /// The bands of sampling frequency index `index` (0 for 96 kHz to 11 for 8 kHz) with frames of `frame` samples (1024 or
    /// 960). The offsets of a 960-sample frame stop at 960 and 120 (clause 4.5.4).
    ///
    /// # Errors
    /// [`Error::SampleRate`] for an index above 11 or another frame length.
    pub(crate) fn new(index: usize, frame: usize) -> Result<Bands> {
        if index >= SWB_LONG.len() || (frame != 1024 && frame != 960) {
            return Err(Error::SampleRate);
        }
        let mut b = Bands {
            frame,
            long: [0; MAX_SFB + 1],
            n_long: 0,
            short: [0; 17],
            n_short: 0,
            tns_long: usize::from(TNS_MAX_LONG[index]),
            tns_short: usize::from(TNS_MAX_SHORT[index]),
        };
        b.n_long = cut(SWB_LONG[index], frame, &mut b.long);
        b.n_short = cut(SWB_SHORT[index], frame / 8, &mut b.short);
        Ok(b)
    }

    /// The band offsets of a long or short window: the number of bands plus one entries.
    pub(crate) fn offsets(&self, short: bool) -> &[u16] {
        if short {
            &self.short[..=self.n_short]
        } else {
            &self.long[..=self.n_long]
        }
    }

    /// Samples per channel of a short window.
    pub(crate) fn short_len(&self) -> usize {
        self.frame / 8
    }
}

/// Copies the offsets of `table` below `end` into `out`, closes them with `end`, and returns the number of bands.
fn cut(table: &[u16], end: usize, out: &mut [u16]) -> usize {
    let mut n = 0;
    for &o in table.iter().take_while(|&&o| usize::from(o) < end) {
        out[n] = o;
        n += 1;
    }
    out[n] = end as u16;
    n
}

/// The window sequence of a frame (clause 4.6.11.3.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Sequence {
    /// One long window.
    #[default]
    OnlyLong,
    /// A long window that ends in a short slope.
    LongStart,
    /// Eight short windows.
    EightShort,
    /// A long window that starts with a short slope.
    LongStop,
}

/// The ICS info of a channel (clause 4.4.2.7, ics_info).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct IcsInfo {
    pub seq: Sequence,
    /// Window shape: Kaiser-Bessel derived when set, sine when clear.
    pub kbd: bool,
    pub max_sfb: usize,
    pub n_groups: usize,
    pub group_len: [u8; 8],
}

impl IcsInfo {
    /// Reads ics_info for the band layout `bands`.
    ///
    /// # Errors
    /// [`Error::Syntax`] if max_sfb exceeds the bands, and [`Error::Unsupported`] for prediction data.
    pub(crate) fn read(r: &mut BitReader<'_>, bands: &Bands) -> Result<IcsInfo> {
        r.skip(1); // ics_reserved_bit
        let seq = match r.u(2) {
            0 => Sequence::OnlyLong,
            1 => Sequence::LongStart,
            2 => Sequence::EightShort,
            _ => Sequence::LongStop,
        };
        let kbd = r.bit();
        let mut info = IcsInfo {
            seq,
            kbd,
            ..IcsInfo::default()
        };
        if seq == Sequence::EightShort {
            info.max_sfb = r.u(4) as usize;
            let grouping = r.u(7) as u32;
            info.n_groups = 1;
            info.group_len[0] = 1;
            for i in 0..7 {
                if grouping & (1 << (6 - i)) != 0 {
                    info.group_len[info.n_groups - 1] += 1;
                } else {
                    info.group_len[info.n_groups] = 1;
                    info.n_groups += 1;
                }
            }
        } else {
            info.max_sfb = r.u(6) as usize;
            info.n_groups = 1;
            info.group_len[0] = 1;
            if r.bit() {
                return Err(Error::Unsupported);
            }
        }
        if info.max_sfb > bands.offsets(info.short()).len() - 1 {
            return Err(Error::Syntax);
        }
        Ok(info)
    }

    /// Whether the frame has eight short windows.
    pub(crate) fn short(&self) -> bool {
        self.seq == Sequence::EightShort
    }
}

/// One TNS filter with its LPC coefficients (clause 4.6.9.3).
#[derive(Clone, Copy, Debug)]
struct TnsFilter {
    length: u8,
    order: u8,
    down: bool,
    lpc: [f32; MAX_ORDER + 1],
}

impl Default for TnsFilter {
    fn default() -> Self {
        TnsFilter {
            length: 0,
            order: 0,
            down: false,
            lpc: [0.0; MAX_ORDER + 1],
        }
    }
}

/// The TNS data of a channel.
#[derive(Clone, Debug, Default)]
struct Tns {
    present: bool,
    n_filt: [u8; 8],
    filt: [[TnsFilter; 3]; 8],
}

/// One channel of an element: its side information, quantized values and scaled spectrum.
#[derive(Clone, Debug)]
pub(crate) struct Channel {
    pub info: IcsInfo,
    /// Codebook by group and band.
    pub cb: [[u8; MAX_SFB]; 8],
    /// Scale factor, intensity position or noise energy by group and band.
    pub sf: [[i32; MAX_SFB]; 8],
    tns: Tns,
    q: Vec<i32>,
    /// The scaled spectrum, window after window.
    pub spec: Vec<f32>,
}

impl Channel {
    /// A channel of frames of `frame` samples.
    pub(crate) fn new(frame: usize) -> Channel {
        Channel {
            info: IcsInfo::default(),
            cb: [[0; MAX_SFB]; 8],
            sf: [[0; MAX_SFB]; 8],
            tns: Tns::default(),
            q: vec![0; frame],
            spec: vec![0.0; frame],
        }
    }

    /// Reads an individual_channel_stream (clause 4.4.2.7) and fills the scaled spectrum. With `common`, the ICS info is
    /// `common` (a channel pair with a common window); otherwise it is read.
    ///
    /// # Errors
    /// [`Error::Syntax`] for values the syntax does not allow, and [`Error::Unsupported`] for gain control and prediction.
    pub(crate) fn read(
        &mut self,
        r: &mut BitReader<'_>,
        bands: &Bands,
        common: Option<IcsInfo>,
    ) -> Result<()> {
        let global_gain = r.u(8) as i32;
        self.info = match common {
            Some(i) => i,
            None => IcsInfo::read(r, bands)?,
        };
        let books = books();
        self.read_sections(r)?;
        self.read_scale_factors(r, &books.scf, global_gain)?;
        let short = self.info.short();
        let offsets = bands.offsets(short);
        let mut pulse = [(0usize, 0i32); 4];
        let mut n_pulse = 0;
        if r.bit() {
            if short {
                return Err(Error::Syntax);
            }
            n_pulse = r.u(2) as usize + 1;
            let start = r.u(6) as usize;
            let mut k = usize::from(*offsets.get(start).ok_or(Error::Syntax)?);
            for p in pulse.iter_mut().take(n_pulse) {
                k += r.u(5) as usize;
                *p = (k, r.u(4) as i32);
            }
        }
        self.tns.present = r.bit();
        if self.tns.present {
            self.read_tns(r, short)?;
        }
        if r.bit() {
            return Err(Error::Unsupported); // gain control (SSR only)
        }
        self.read_spectrum(r, books, bands)?;
        for &(k, amp) in &pulse[..n_pulse] {
            let q = self.q.get_mut(k).ok_or(Error::Syntax)?;
            *q += if *q > 0 { amp } else { -amp };
        }
        self.dequantize(bands)
    }

    fn read_sections(&mut self, r: &mut BitReader<'_>) -> Result<()> {
        let bits = if self.info.short() { 3 } else { 5 };
        let esc = (1u64 << bits) - 1;
        let max = self.info.max_sfb;
        for g in 0..self.info.n_groups {
            let mut k = 0;
            while k < max {
                let cb = r.u(4) as u8;
                if cb == 12 {
                    return Err(Error::Syntax);
                }
                let mut len = 0;
                loop {
                    let inc = r.u(bits);
                    len += inc as usize;
                    if inc != esc || r.overrun() {
                        break;
                    }
                }
                if k + len > max || r.overrun() {
                    return Err(Error::Syntax);
                }
                self.cb[g][k..k + len].fill(cb);
                k += len;
            }
        }
        Ok(())
    }

    fn read_scale_factors(&mut self, r: &mut BitReader<'_>, scf: &Book, global: i32) -> Result<()> {
        let mut sf = global;
        let mut is = 0;
        let mut nrg = global - 90;
        let mut first_noise = true;
        for g in 0..self.info.n_groups {
            for b in 0..self.info.max_sfb {
                self.sf[g][b] = match self.cb[g][b] {
                    0 => 0,
                    INTENSITY_HCB | INTENSITY_HCB2 => {
                        is += scf.read(r).ok_or(Error::Syntax)? as i32 - 60;
                        is
                    }
                    NOISE_HCB => {
                        if first_noise {
                            first_noise = false;
                            nrg += r.u(9) as i32 - 256;
                        } else {
                            nrg += scf.read(r).ok_or(Error::Syntax)? as i32 - 60;
                        }
                        nrg
                    }
                    _ => {
                        sf += scf.read(r).ok_or(Error::Syntax)? as i32 - 60;
                        if !(0..=255).contains(&sf) {
                            return Err(Error::Syntax);
                        }
                        sf
                    }
                };
            }
        }
        Ok(())
    }

    fn read_tns(&mut self, r: &mut BitReader<'_>, short: bool) -> Result<()> {
        let windows = if short { 8 } else { 1 };
        let (filt_bits, len_bits, order_bits) = if short { (1, 4, 3) } else { (2, 6, 5) };
        for w in 0..windows {
            let n = r.u(filt_bits) as usize;
            self.tns.n_filt[w] = n as u8;
            if n == 0 {
                continue;
            }
            let res = r.u(1) as u32 + 3;
            for f in 0..n {
                let filt = &mut self.tns.filt[w][f];
                filt.length = r.u(len_bits) as u8;
                filt.order = r.u(order_bits) as u8;
                let order = usize::from(filt.order);
                if order == 0 {
                    continue;
                }
                filt.down = r.bit();
                let bits = res - r.u(1) as u32;
                // Dequantized reflection coefficients (clause 4.6.9.3), then the step-up to LPC coefficients.
                let half = f64::from(1u32 << (res - 1));
                let fac_pos = (half - 0.5) / std::f64::consts::FRAC_PI_2;
                let fac_neg = (half + 0.5) / std::f64::consts::FRAC_PI_2;
                let mut parcor = [0f32; MAX_ORDER];
                for p in parcor.iter_mut().take(order) {
                    let v = r.i(bits) as f64;
                    *p = (v / if v >= 0.0 { fac_pos } else { fac_neg }).sin() as f32;
                }
                let a = &mut filt.lpc;
                a[0] = 1.0;
                for m in 1..=order {
                    let k = parcor[m - 1];
                    for i in 1..=m / 2 {
                        let (x, y) = (a[i], a[m - i]);
                        a[i] = x + k * y;
                        if i != m - i {
                            a[m - i] = y + k * x;
                        }
                    }
                    a[m] = k;
                }
            }
        }
        Ok(())
    }

    fn read_spectrum(&mut self, r: &mut BitReader<'_>, books: &Books, bands: &Bands) -> Result<()> {
        self.q.fill(0);
        let short = self.info.short();
        let offsets = bands.offsets(short);
        let win_len = if short {
            bands.short_len()
        } else {
            bands.frame
        };
        let mut w0 = 0;
        for g in 0..self.info.n_groups {
            let glen = usize::from(self.info.group_len[g]);
            for b in 0..self.info.max_sfb {
                let cb = self.cb[g][b];
                if cb == 0 || cb >= NOISE_HCB {
                    continue;
                }
                let book = &books.spectral[usize::from(cb) - 1];
                let (k0, k1) = (usize::from(offsets[b]), usize::from(offsets[b + 1]));
                for w in w0..w0 + glen {
                    let base = w * win_len;
                    read_band(r, book, cb, &mut self.q[base + k0..base + k1])?;
                }
            }
            w0 += glen;
        }
        if r.overrun() {
            return Err(Error::Syntax);
        }
        Ok(())
    }

    /// Scales the quantized values: `sign(q) |q|^(4/3) 2^((sf - 100) / 4)` (clause 4.6.2.3).
    fn dequantize(&mut self, bands: &Bands) -> Result<()> {
        let pow = pow43();
        self.spec.fill(0.0);
        let short = self.info.short();
        let offsets = bands.offsets(short);
        let win_len = if short {
            bands.short_len()
        } else {
            bands.frame
        };
        let mut w0 = 0;
        for g in 0..self.info.n_groups {
            let glen = usize::from(self.info.group_len[g]);
            for b in 0..self.info.max_sfb {
                let cb = self.cb[g][b];
                if cb == 0 || cb >= NOISE_HCB {
                    continue;
                }
                let gain = 2f32.powf(0.25 * (self.sf[g][b] - 100) as f32);
                let (k0, k1) = (usize::from(offsets[b]), usize::from(offsets[b + 1]));
                for w in w0..w0 + glen {
                    let base = w * win_len;
                    for k in base + k0..base + k1 {
                        let q = self.q[k];
                        let m = *pow.get(q.unsigned_abs() as usize).ok_or(Error::Syntax)? * gain;
                        self.spec[k] = if q < 0 { -m } else { m };
                    }
                }
            }
            w0 += glen;
        }
        Ok(())
    }

    /// Applies the TNS filters to the spectrum (clause 4.6.9.3).
    pub(crate) fn apply_tns(&mut self, bands: &Bands) {
        if !self.tns.present {
            return;
        }
        let short = self.info.short();
        let offsets = bands.offsets(short);
        let n_bands = offsets.len() - 1;
        let (windows, win_len, max_tns) = if short {
            (8, bands.short_len(), bands.tns_short)
        } else {
            (1, bands.frame, bands.tns_long)
        };
        let limit = |b: usize| usize::from(offsets[b.min(max_tns).min(self.info.max_sfb)]);
        for w in 0..windows {
            let mut top = n_bands;
            for f in 0..usize::from(self.tns.n_filt[w]) {
                let filt = &self.tns.filt[w][f];
                let bottom = top.saturating_sub(usize::from(filt.length));
                let order = usize::from(filt.order);
                let (start, end) = (limit(bottom), limit(top));
                top = bottom;
                if order == 0 || end <= start {
                    continue;
                }
                let x = &mut self.spec[w * win_len + start..w * win_len + end];
                let mut state = [0f32; MAX_ORDER];
                let n = x.len();
                for j in 0..n {
                    let k = if filt.down { n - 1 - j } else { j };
                    let mut y = x[k];
                    for (a, s) in filt.lpc[1..=order].iter().zip(&state) {
                        y -= a * s;
                    }
                    state.copy_within(0..order - 1, 1);
                    state[0] = y;
                    x[k] = y;
                }
            }
        }
    }
}

/// Reads the values of one band of one window with codebook `cb` (1 to 11) into `q` (clause 4.6.3).
fn read_band(r: &mut BitReader<'_>, book: &Book, cb: u8, q: &mut [i32]) -> Result<()> {
    let (dim, signed, m) = match cb {
        1 | 2 => (4, true, 3),
        3 | 4 => (4, false, 3),
        5 | 6 => (2, true, 9),
        7 | 8 => (2, false, 8),
        9 | 10 => (2, false, 13),
        _ => (2, false, 17),
    };
    for chunk in q.chunks_exact_mut(dim) {
        let idx = book.read(r).ok_or(Error::Syntax)? as i32;
        if dim == 4 {
            chunk[0] = idx / 27;
            chunk[1] = idx / 9 % 3;
            chunk[2] = idx / 3 % 3;
            chunk[3] = idx % 3;
        } else {
            chunk[0] = idx / m;
            chunk[1] = idx % m;
        }
        if signed {
            let off = if dim == 4 { 1 } else { 4 };
            for v in chunk.iter_mut() {
                *v -= off;
            }
            continue;
        }
        for v in chunk.iter_mut() {
            if *v != 0 && r.bit() {
                *v = -*v;
            }
        }
        if cb == 11 {
            for v in chunk.iter_mut() {
                if v.unsigned_abs() == 16 {
                    let mut n = 0;
                    while r.bit() {
                        n += 1;
                        if n > 8 {
                            return Err(Error::Syntax);
                        }
                    }
                    let e = (1i32 << (n + 4)) + r.u(n + 4) as i32;
                    *v = if *v < 0 { -e } else { e };
                }
            }
        }
    }
    Ok(())
}
