//! Spectral band replication: the SBR bitstream of a fill element, the decoding of envelopes and noise floors, the HF
//! generator, the HF adjuster and the QMF banks around them. A mono element may carry parametric stereo in its extended
//! data.
//!
//! Standard: ISO/IEC 14496-3 clause 4.4.2.8 (syntax) and clause 4.6.18 (decoding).

mod freq;
mod qmf;
pub(crate) mod tables;

use crate::huffman::Book;
use crate::ps::Ps;
use crate::{Error, Result};
use freq::Freq;
use qmf::{Analysis, Synthesis};
use sdrkit_algo_dsp::Complex32 as C;
use sdrkit_util_bits::BitReader;
use std::sync::OnceLock;
use tables::{Tree, NOISE};

/// QMF slots from the start of the low band buffer to the first slot of the frame (t_HFGen).
const T_HFGEN: usize = 8;
/// QMF slots from the start of a frame to the slot that the HF adjuster writes first (t_HFAdj).
const T_HFADJ: usize = 2;
/// The extension id of parametric stereo in the extended data.
const EXTENSION_ID_PS: u64 = 2;
/// The gains of the limiter by `bs_limiter_gains` (clause 4.6.18.7.3), as printed in the PacketVideo AAC decoder
/// (`sbr_envelope_calc_tbl.cpp`, Apache License 2.0).
const LIM_GAIN: [f32; 4] = [0.70795, 1.0, 1.41254, 1.0e10];
/// The smoothing filter of the gains, newest slot first (clause 4.6.18.7.5), as printed in the PacketVideo AAC decoder
/// (`sbr_envelope_calc_tbl.cpp`).
const H_SMOOTH: [f32; 5] = [
    0.333_333_33,
    0.301_502_83,
    0.218_169_5,
    0.115_163_83,
    0.031_830_5,
];

/// The SBR header (clause 4.4.2.8, sbr_header).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Header {
    pub amp_res: bool,
    pub start_freq: u8,
    pub stop_freq: u8,
    pub xover_band: u8,
    pub freq_scale: u8,
    pub alter_scale: bool,
    pub noise_bands: u8,
    pub limiter_bands: u8,
    pub limiter_gains: u8,
    pub interpol_freq: bool,
    pub smoothing_mode: bool,
}

impl Default for Header {
    /// The values that a header without its extra parts sets.
    fn default() -> Self {
        Header {
            amp_res: true,
            start_freq: 0,
            stop_freq: 0,
            xover_band: 0,
            freq_scale: 2,
            alter_scale: true,
            noise_bands: 2,
            limiter_bands: 2,
            limiter_gains: 2,
            interpol_freq: true,
            smoothing_mode: true,
        }
    }
}

impl Header {
    fn read(r: &mut BitReader<'_>) -> Header {
        let mut h = Header {
            amp_res: r.bit(),
            start_freq: r.u(4) as u8,
            stop_freq: r.u(4) as u8,
            xover_band: r.u(3) as u8,
            ..Header::default()
        };
        r.skip(2);
        let extra1 = r.bit();
        let extra2 = r.bit();
        if extra1 {
            h.freq_scale = r.u(2) as u8;
            h.alter_scale = r.bit();
            h.noise_bands = r.u(2) as u8;
        }
        if extra2 {
            h.limiter_bands = r.u(2) as u8;
            h.limiter_gains = r.u(2) as u8;
            h.interpol_freq = r.bit();
            h.smoothing_mode = r.bit();
        }
        h
    }

    /// Whether the frequency tables of `self` and `o` differ.
    fn tables_differ(&self, o: &Header) -> bool {
        (
            self.start_freq,
            self.stop_freq,
            self.xover_band,
            self.freq_scale,
            self.alter_scale,
            self.noise_bands,
        ) != (
            o.start_freq,
            o.stop_freq,
            o.xover_band,
            o.freq_scale,
            o.alter_scale,
            o.noise_bands,
        ) || self.limiter_bands != o.limiter_bands
    }
}

/// The SBR Huffman books, in the order of [`tables`].
struct Books {
    env_1_5: [Book; 2],
    bal_1_5: [Book; 2],
    env_3_0: [Book; 2],
    bal_3_0: [Book; 2],
    noise_t: Book,
    noise_bal_t: Book,
}

fn book(t: &Tree) -> Book {
    Book::new(t.code.len(), |i| (t.code[i], t.len[i]))
}

fn books() -> &'static Books {
    static B: OnceLock<Books> = OnceLock::new();
    B.get_or_init(|| Books {
        env_1_5: [book(&tables::ENV_1_5_T), book(&tables::ENV_1_5_F)],
        bal_1_5: [book(&tables::BAL_1_5_T), book(&tables::BAL_1_5_F)],
        env_3_0: [book(&tables::ENV_3_0_T), book(&tables::ENV_3_0_F)],
        bal_3_0: [book(&tables::BAL_3_0_T), book(&tables::BAL_3_0_F)],
        noise_t: book(&tables::NOISE_3_0_T),
        noise_bal_t: book(&tables::NOISE_BAL_3_0_T),
    })
}

/// Reads one signed value with `book`, whose smallest value is `min`.
fn value(r: &mut BitReader<'_>, book: &Book, min: i32) -> Result<i32> {
    Ok(book.read(r).ok_or(Error::Syntax)? as i32 + min)
}

/// The time grid of one channel (clause 4.6.18.3.3).
#[derive(Clone, Copy, Debug, Default)]
struct Grid {
    fixfix: bool,
    n_env: usize,
    /// Envelope borders in time slots.
    t_e: [usize; 6],
    /// High frequency resolution by envelope.
    freq_res: [bool; 5],
    n_q: usize,
    /// Noise floor borders in time slots.
    t_q: [usize; 3],
    /// The envelope that starts at a transient, if any.
    l_a: Option<usize>,
}

/// The data of one channel in one frame.
#[derive(Clone, Debug)]
struct Data {
    grid: Grid,
    df_env: [bool; 5],
    df_noise: [bool; 2],
    invf: [u8; 5],
    amp_res: bool,
    env: [[i32; 64]; 5],
    noise: [[i32; 5]; 2],
    harmonic: [bool; 64],
}

impl Default for Data {
    fn default() -> Self {
        Data {
            grid: Grid::default(),
            df_env: [false; 5],
            df_noise: [false; 2],
            invf: [0; 5],
            amp_res: false,
            env: [[0; 64]; 5],
            noise: [[0; 5]; 2],
            harmonic: [false; 64],
        }
    }
}

/// The state of one channel from frame to frame.
#[derive(Clone, Debug)]
struct State {
    analysis: Analysis,
    synthesis: Synthesis,
    /// The low band, previous slots first.
    x_low: Vec<[C; 64]>,
    /// The high band, with the slots past the frame end.
    y: Vec<[C; 64]>,
    /// The subband samples into the synthesis bank.
    x: Vec<[C; 64]>,
    env_prev: [i32; 64],
    env_prev_hi: bool,
    noise_prev: [i32; 5],
    invf_prev: [u8; 5],
    bw_prev: [f32; 5],
    g_hist: [[f32; 64]; 5],
    q_hist: [[f32; 64]; 5],
    hist_full: bool,
    /// The last frame ended with a transient envelope.
    transient_end: bool,
    noise_idx: usize,
    sine_idx: usize,
    sine_prev: [bool; 64],
    e_orig: [[f32; 64]; 5],
    q_orig: [[f32; 5]; 2],
}

impl State {
    fn new(slots: usize, bands: usize) -> State {
        State {
            analysis: Analysis::default(),
            synthesis: Synthesis::new(bands),
            x_low: vec![[C::default(); 64]; T_HFGEN + 2 * slots],
            y: vec![[C::default(); 64]; T_HFGEN + 2 * slots],
            x: vec![[C::default(); 64]; 2 * slots],
            env_prev: [0; 64],
            env_prev_hi: false,
            noise_prev: [0; 5],
            invf_prev: [0; 5],
            bw_prev: [0.0; 5],
            g_hist: [[0.0; 64]; 5],
            q_hist: [[0.0; 64]; 5],
            hist_full: false,
            transient_end: false,
            noise_idx: 0,
            sine_idx: 0,
            sine_prev: [false; 64],
            e_orig: [[0.0; 64]; 5],
            q_orig: [[0.0; 5]; 2],
        }
    }
}

/// The SBR decoder of one element (one channel, or a channel pair).
pub(crate) struct Sbr {
    fs: u32,
    slots: usize,
    down: bool,
    header: Option<Header>,
    freq: Option<Freq>,
    reset: bool,
    coupling: bool,
    have_data: bool,
    data: [Data; 2],
    st: [State; 2],
    ps: Option<Box<Ps>>,
}

impl Sbr {
    /// The SBR decoder of an element whose core runs at `core_rate` with frames of `frame` samples. With `down`, the
    /// output stays at the core rate; with `ps`, a mono element may carry parametric stereo.
    ///
    /// # Errors
    /// [`Error::Unsupported`] for a frame length other than 1024 or 960.
    pub(crate) fn new(core_rate: u32, frame: usize, down: bool, ps: bool) -> Result<Sbr> {
        if frame != 1024 && frame != 960 {
            return Err(Error::Unsupported);
        }
        let slots = frame / 64;
        let bands = if down { 32 } else { 64 };
        Ok(Sbr {
            fs: 2 * core_rate,
            slots,
            down,
            header: None,
            freq: None,
            reset: true,
            coupling: false,
            have_data: false,
            data: [Data::default(), Data::default()],
            st: [State::new(slots, bands), State::new(slots, bands)],
            ps: ps.then(|| Box::new(Ps::new(2 * slots))),
        })
    }

    /// Reads sbr_extension_data (clause 4.4.2.8) up to bit `end` of `r`. A frame whose SBR data does not parse keeps the
    /// low band only.
    pub(crate) fn read(&mut self, r: &mut BitReader<'_>, end: usize, pair: bool, crc: bool) {
        if crc {
            r.skip(10);
        }
        if r.bit() {
            let h = Header::read(r);
            if self.header.is_none_or(|o| o.tables_differ(&h)) {
                self.freq = Freq::new(&h, self.fs);
                self.reset = true;
            }
            self.header = Some(h);
        }
        self.have_data = false;
        let (Some(h), Some(f)) = (self.header, self.freq.as_ref()) else {
            return;
        };
        let f = f.clone();
        if self.read_data(r, end, pair, &h, &f).is_ok() && r.pos() <= end {
            self.have_data = true;
        }
    }

    fn read_data(
        &mut self,
        r: &mut BitReader<'_>,
        end: usize,
        pair: bool,
        h: &Header,
        f: &Freq,
    ) -> Result<()> {
        if r.bit() {
            r.skip(if pair { 8 } else { 4 });
        }
        if pair {
            self.coupling = r.bit();
            if self.coupling {
                self.read_grid(r, 0)?;
                self.data[1].grid = self.data[0].grid;
                self.read_dtdf(r, 0);
                self.read_dtdf(r, 1);
                self.read_invf(r, 0, f);
                self.data[1].invf = self.data[0].invf;
                self.read_envelope(r, 0, h, f)?;
                self.read_noise(r, 0, f)?;
                self.read_envelope(r, 1, h, f)?;
                self.read_noise(r, 1, f)?;
            } else {
                self.read_grid(r, 0)?;
                self.read_grid(r, 1)?;
                self.read_dtdf(r, 0);
                self.read_dtdf(r, 1);
                self.read_invf(r, 0, f);
                self.read_invf(r, 1, f);
                self.read_envelope(r, 0, h, f)?;
                self.read_envelope(r, 1, h, f)?;
                self.read_noise(r, 0, f)?;
                self.read_noise(r, 1, f)?;
            }
            for ch in 0..2 {
                self.read_harmonics(r, ch, f);
            }
        } else {
            self.coupling = false;
            self.read_grid(r, 0)?;
            self.read_dtdf(r, 0);
            self.read_invf(r, 0, f);
            self.read_envelope(r, 0, h, f)?;
            self.read_noise(r, 0, f)?;
            self.read_harmonics(r, 0, f);
        }
        if r.bit() {
            let mut count = r.u(4) as usize;
            if count == 15 {
                count += r.u(8) as usize;
            }
            let mut left = 8 * count;
            let stop = r.pos() + left;
            if stop > end {
                return Err(Error::Syntax);
            }
            while left > 7 {
                let id = r.u(2);
                left -= 2;
                if id == EXTENSION_ID_PS && !pair {
                    if let Some(ps) = self.ps.as_mut() {
                        let used = ps.read(r, left);
                        left = left.saturating_sub(used);
                        continue;
                    }
                }
                r.skip(left);
                left = 0;
            }
            r.skip(stop.saturating_sub(r.pos()));
        }
        if r.overrun() {
            return Err(Error::Syntax);
        }
        Ok(())
    }

    fn read_grid(&mut self, r: &mut BitReader<'_>, ch: usize) -> Result<()> {
        let slots = self.slots;
        let mut g = Grid::default();
        let class = r.u(2);
        let mut rel_lead = [0usize; 4];
        let mut rel_trail = [0usize; 4];
        let (lead, trail, n_lead, n_trail, pointer);
        // ceil(log2(x)) bits for a pointer of x values.
        let bits = |x: usize| usize::BITS - (x - 1).leading_zeros();
        match class {
            0 => {
                g.fixfix = true;
                g.n_env = 1 << r.u(2);
                let hi = r.bit();
                g.freq_res = [hi; 5];
                if g.n_env > 4 {
                    return Err(Error::Syntax);
                }
                lead = 0;
                trail = slots;
                n_lead = g.n_env - 1;
                n_trail = 0;
                let step = (slots as f64 / g.n_env as f64 + 0.5).floor() as usize;
                rel_lead = [step; 4];
                pointer = 0;
            }
            1 => {
                let var1 = r.u(2) as usize;
                let n1 = r.u(2) as usize;
                for v in rel_trail.iter_mut().take(n1) {
                    *v = 2 * r.u(2) as usize + 2;
                }
                pointer = r.u(bits(n1 + 2)) as usize;
                g.n_env = n1 + 1;
                for e in 0..g.n_env {
                    g.freq_res[g.n_env - 1 - e] = r.bit();
                }
                lead = 0;
                trail = var1 + slots;
                n_lead = 0;
                n_trail = n1;
            }
            2 => {
                let var0 = r.u(2) as usize;
                let n0 = r.u(2) as usize;
                for v in rel_lead.iter_mut().take(n0) {
                    *v = 2 * r.u(2) as usize + 2;
                }
                pointer = r.u(bits(n0 + 2)) as usize;
                g.n_env = n0 + 1;
                for e in 0..g.n_env {
                    g.freq_res[e] = r.bit();
                }
                lead = var0;
                trail = slots;
                n_lead = n0;
                n_trail = 0;
            }
            _ => {
                let var0 = r.u(2) as usize;
                let var1 = r.u(2) as usize;
                let n0 = r.u(2) as usize;
                let n1 = r.u(2) as usize;
                for v in rel_lead.iter_mut().take(n0) {
                    *v = 2 * r.u(2) as usize + 2;
                }
                for v in rel_trail.iter_mut().take(n1) {
                    *v = 2 * r.u(2) as usize + 2;
                }
                pointer = r.u(bits(n0 + n1 + 2)) as usize;
                g.n_env = n0 + n1 + 1;
                if g.n_env > 5 {
                    return Err(Error::Syntax);
                }
                for e in 0..g.n_env {
                    g.freq_res[e] = r.bit();
                }
                lead = var0;
                trail = var1 + slots;
                n_lead = n0;
                n_trail = n1;
            }
        }
        let n = g.n_env;
        g.t_e[0] = lead;
        g.t_e[n] = trail;
        for l in 1..=n_lead.min(n - 1) {
            g.t_e[l] = g.t_e[l - 1] + rel_lead[l - 1];
        }
        for (i, &rel) in rel_trail.iter().take(n_trail).enumerate() {
            let l = n - 1 - i;
            g.t_e[l] = g.t_e[l + 1].checked_sub(rel).ok_or(Error::Syntax)?;
        }
        if g.t_e[..=n].windows(2).any(|w| w[1] <= w[0]) || trail > slots + 3 {
            return Err(Error::Syntax);
        }
        g.l_a = match class {
            0 => None,
            1 | 3 => (pointer > 0).then(|| n + 1 - pointer),
            _ => (pointer > 1).then(|| pointer - 1),
        };
        if g.l_a.is_some_and(|l| l > n) {
            return Err(Error::Syntax);
        }
        g.n_q = if n > 1 { 2 } else { 1 };
        g.t_q[0] = g.t_e[0];
        g.t_q[g.n_q] = g.t_e[n];
        if g.n_q == 2 {
            let middle = match class {
                0 => n / 2,
                2 => match pointer {
                    0 => 1,
                    1 => n - 1,
                    p => p - 1,
                },
                _ => {
                    if pointer > 1 {
                        n + 1 - pointer
                    } else {
                        n - 1
                    }
                }
            };
            if middle == 0 || middle >= n {
                return Err(Error::Syntax);
            }
            g.t_q[1] = g.t_e[middle];
        }
        self.data[ch].grid = g;
        Ok(())
    }

    fn read_dtdf(&mut self, r: &mut BitReader<'_>, ch: usize) {
        let d = &mut self.data[ch];
        for e in 0..d.grid.n_env {
            d.df_env[e] = r.bit();
        }
        for q in 0..d.grid.n_q {
            d.df_noise[q] = r.bit();
        }
    }

    fn read_invf(&mut self, r: &mut BitReader<'_>, ch: usize, f: &Freq) {
        for q in 0..f.n_q {
            self.data[ch].invf[q] = r.u(2) as u8;
        }
    }

    fn read_envelope(
        &mut self,
        r: &mut BitReader<'_>,
        ch: usize,
        h: &Header,
        f: &Freq,
    ) -> Result<()> {
        let bal = self.coupling && ch == 1;
        let d = &mut self.data[ch];
        d.amp_res = h.amp_res && !(d.grid.fixfix && d.grid.n_env == 1);
        let b = books();
        let (book, tree, start_bits) = match (bal, d.amp_res) {
            (false, false) => (&b.env_1_5, [&tables::ENV_1_5_T, &tables::ENV_1_5_F], 7),
            (false, true) => (&b.env_3_0, [&tables::ENV_3_0_T, &tables::ENV_3_0_F], 6),
            (true, false) => (&b.bal_1_5, [&tables::BAL_1_5_T, &tables::BAL_1_5_F], 6),
            (true, true) => (&b.bal_3_0, [&tables::BAL_3_0_T, &tables::BAL_3_0_F], 5),
        };
        for e in 0..d.grid.n_env {
            let n = f.n[usize::from(d.grid.freq_res[e])];
            if d.df_env[e] {
                for k in 0..n {
                    d.env[e][k] = value(r, &book[0], tree[0].min)?;
                }
            } else {
                d.env[e][0] = r.u(start_bits) as i32;
                for k in 1..n {
                    d.env[e][k] = value(r, &book[1], tree[1].min)?;
                }
            }
        }
        Ok(())
    }

    fn read_noise(&mut self, r: &mut BitReader<'_>, ch: usize, f: &Freq) -> Result<()> {
        let bal = self.coupling && ch == 1;
        let b = books();
        let (t_book, t_min, f_book, f_min) = if bal {
            (
                &b.noise_bal_t,
                tables::NOISE_BAL_3_0_T.min,
                &b.bal_3_0[1],
                tables::BAL_3_0_F.min,
            )
        } else {
            (
                &b.noise_t,
                tables::NOISE_3_0_T.min,
                &b.env_3_0[1],
                tables::ENV_3_0_F.min,
            )
        };
        let d = &mut self.data[ch];
        for q in 0..d.grid.n_q {
            if d.df_noise[q] {
                for k in 0..f.n_q {
                    d.noise[q][k] = value(r, t_book, t_min)?;
                }
            } else {
                d.noise[q][0] = r.u(5) as i32;
                for k in 1..f.n_q {
                    d.noise[q][k] = value(r, f_book, f_min)?;
                }
            }
        }
        Ok(())
    }

    fn read_harmonics(&mut self, r: &mut BitReader<'_>, ch: usize, f: &Freq) {
        let d = &mut self.data[ch];
        d.harmonic = [false; 64];
        if r.bit() {
            for k in 0..f.n[1] {
                d.harmonic[k] = r.bit();
            }
        }
    }

    /// Turns the delta values of channel `ch` into absolute values (clause 4.6.18.3.4).
    fn undelta(&mut self, ch: usize, f: &Freq) {
        let d = &mut self.data[ch];
        let st = &mut self.st[ch];
        for e in 0..d.grid.n_env {
            let hi = d.grid.freq_res[e];
            let n = f.n[usize::from(hi)];
            let (prev, prev_hi) = if e == 0 {
                (st.env_prev, st.env_prev_hi)
            } else {
                (d.env[e - 1], d.grid.freq_res[e - 1])
            };
            if d.df_env[e] {
                for k in 0..n {
                    let j = if hi == prev_hi {
                        k
                    } else if !hi {
                        // Low resolution after high: the high band that starts where this one starts.
                        let lo = f.table[0][k];
                        f.table[1][..=f.n[1]]
                            .iter()
                            .position(|&b| b == lo)
                            .unwrap_or(0)
                    } else {
                        // High resolution after low: the low band that contains this one.
                        let hb = f.table[1][k];
                        (0..f.n[0])
                            .rev()
                            .find(|&j| f.table[0][j] <= hb)
                            .unwrap_or(0)
                    };
                    d.env[e][k] += prev[j];
                }
            } else {
                for k in 1..n {
                    d.env[e][k] += d.env[e][k - 1];
                }
            }
        }
        let last = d.grid.n_env - 1;
        st.env_prev = d.env[last];
        st.env_prev_hi = d.grid.freq_res[last];
        for q in 0..d.grid.n_q {
            let prev = if q == 0 { st.noise_prev } else { d.noise[0] };
            if d.df_noise[q] {
                for (v, p) in d.noise[q][..f.n_q].iter_mut().zip(prev) {
                    *v += p;
                }
            } else {
                for k in 1..f.n_q {
                    d.noise[q][k] += d.noise[q][k - 1];
                }
            }
        }
        st.noise_prev = d.noise[d.grid.n_q - 1];
    }

    /// Dequantizes the envelopes and noise floors of the channels (clause 4.6.18.3.5).
    fn dequantize(&mut self, n_ch: usize, f: &Freq) {
        let coupled = self.coupling && n_ch == 2;
        for ch in 0..n_ch {
            let d = &self.data[ch];
            let a = if d.amp_res { 1.0 } else { 2.0 };
            let g = d.grid;
            for e in 0..g.n_env {
                for k in 0..f.n[usize::from(g.freq_res[e])] {
                    let v = if coupled {
                        let lvl = self.data[0].env[e][k] as f32 / a;
                        let pan = self.data[1].env[e][k] as f32 / a - 12.0;
                        let p = if ch == 0 { -pan } else { pan };
                        64.0 * 2f32.powf(lvl + 1.0) / (1.0 + 2f32.powf(p))
                    } else {
                        64.0 * 2f32.powf(d.env[e][k] as f32 / a)
                    };
                    self.st[ch].e_orig[e][k] = v;
                }
            }
            for q in 0..g.n_q {
                for k in 0..f.n_q {
                    let v = if coupled {
                        let lvl = self.data[0].noise[q][k] as f32;
                        let pan = self.data[1].noise[q][k] as f32 - 12.0;
                        let p = if ch == 0 { -pan } else { pan };
                        2f32.powf(6.0 - lvl + 1.0) / (1.0 + 2f32.powf(p))
                    } else {
                        2f32.powf(6.0 - d.noise[q][k] as f32)
                    };
                    self.st[ch].q_orig[q][k] = v;
                }
            }
        }
    }

    /// Runs SBR over one core frame: `core` holds the core output of each channel of the element, and `out0` (and
    /// `out1` for a pair or for parametric stereo) receive the output.
    pub(crate) fn process(
        &mut self,
        core: &[Vec<f32>],
        out0: &mut [f32],
        out1: Option<&mut [f32]>,
    ) {
        let n_ch = core.len();
        let nslots = 2 * self.slots;
        let freq = if self.have_data {
            self.freq.clone()
        } else {
            None
        };
        if let Some(f) = freq.as_ref() {
            for ch in 0..n_ch {
                self.undelta(ch, f);
            }
            self.dequantize(n_ch, f);
        }
        let h = self.header.unwrap_or_default();
        let reset = std::mem::take(&mut self.reset);
        for (ch, input) in core.iter().enumerate() {
            let st = &mut self.st[ch];
            for (l, x) in input.as_chunks::<32>().0.iter().take(nslots).enumerate() {
                st.analysis.run(x, &mut st.x_low[T_HFGEN + l]);
            }
            let kx = match freq.as_ref() {
                Some(f) => {
                    if reset {
                        st.bw_prev = [0.0; 5];
                        st.invf_prev = [0; 5];
                        st.hist_full = false;
                    }
                    let d = &self.data[ch];
                    generate(st, d, f);
                    adjust(st, d, f, &h);
                    f.kx
                }
                None => 32,
            };
            let m = freq.as_ref().map_or(0, |f| f.m);
            for l in 0..nslots {
                let (xl, y) = (&st.x_low[l + T_HFADJ], &st.y[l + T_HFADJ]);
                let x = &mut st.x[l];
                x[..kx].copy_from_slice(&xl[..kx]);
                x[kx..kx + m].copy_from_slice(&y[kx..kx + m]);
                x[kx + m..].fill(C::default());
            }
            st.x_low.copy_within(nslots..nslots + T_HFGEN, 0);
            st.y.copy_within(nslots..nslots + T_HFGEN, 0);
            for s in &mut st.y[T_HFGEN..] {
                *s = [C::default(); 64];
            }
        }
        let bands = if self.down { 32 } else { 64 };
        if n_ch == 1 {
            if let (Some(ps), Some(out1)) = (self.ps.as_mut(), out1) {
                let [s0, s1] = &mut self.st;
                ps.process(&mut s0.x, &mut s1.x);
                synthesize(s0, out0, bands);
                synthesize(s1, out1, bands);
            } else {
                synthesize(&mut self.st[0], out0, bands);
            }
        } else {
            synthesize(&mut self.st[0], out0, bands);
            if let Some(o) = out1 {
                synthesize(&mut self.st[1], o, bands);
            }
        }
    }
}

fn synthesize(st: &mut State, out: &mut [f32], bands: usize) {
    for (l, x) in st.x.iter().enumerate() {
        st.synthesis.run(x, &mut out[l * bands..(l + 1) * bands]);
    }
}

/// The chirp factor of a noise band from its inverse filtering mode and the last one (clause 4.6.18.6.2), as printed in
/// the PacketVideo AAC decoder (`sbr_inv_filt_levelemphasis.cpp`).
fn chirp(mode: u8, prev_mode: u8, prev_bw: f32) -> f32 {
    let new = match (mode, prev_mode) {
        (1, 0) => 0.6,
        (1, _) => 0.75,
        (2, _) => 0.9,
        (3, _) => 0.98,
        (_, 1) => 0.6,
        _ => 0.0,
    };
    let bw = if new < prev_bw {
        0.75 * new + 0.25 * prev_bw
    } else {
        0.90625 * new + 0.09375 * prev_bw
    };
    if bw < 0.015625 {
        0.0
    } else {
        bw.min(0.99609375)
    }
}

/// The HF generator (clause 4.6.18.6): the second-order linear prediction of each low subband and the patches into the
/// high band of `st.y`.
fn generate(st: &mut State, d: &Data, f: &Freq) {
    let g = &d.grid;
    let nslots = st.x.len();
    let mut bw = [0f32; 5];
    for (q, b) in bw.iter_mut().enumerate().take(f.n_q) {
        *b = chirp(d.invf[q], st.invf_prev[q], st.bw_prev[q]);
    }
    st.bw_prev = bw;
    st.invf_prev = d.invf;
    let (l0, l1) = (2 * g.t_e[0], 2 * g.t_e[g.n_env]);
    let mut k = f.kx;
    for &(start, width) in &f.patch[..f.n_patch] {
        for x in 0..usize::from(width) {
            let p = usize::from(start) + x;
            let (a0, a1) = predict(&st.x_low, p, nslots);
            let q = (0..f.n_q)
                .rev()
                .find(|&q| usize::from(f.noise[q]) <= k)
                .unwrap_or(0);
            let b = bw[q];
            let (c0, c1) = (a0 * b, a1 * (b * b));
            for l in l0..l1 {
                let i = l + T_HFADJ;
                let v = st.x_low[i][p] + c0 * st.x_low[i - 1][p] + c1 * st.x_low[i - 2][p];
                st.y[i][k] = v;
            }
            k += 1;
        }
    }
}

/// The prediction coefficients alpha0 and alpha1 of low subband `p` by the covariance method (clause 4.6.18.6.2).
fn predict(x: &[[C; 64]], p: usize, nslots: usize) -> (C, C) {
    let mut phi = [[C::default(); 3]; 3];
    for n in 0..nslots + 6 {
        let s = [
            x[n + T_HFADJ][p],
            x[n + T_HFADJ - 1][p],
            x[n + T_HFADJ - 2][p],
        ];
        for i in 0..3 {
            for j in i..3 {
                phi[i][j] += s[i] * s[j].conj();
            }
        }
    }
    let (p01, p02, p11, p12, p22) = (phi[0][1], phi[0][2], phi[1][1].re, phi[1][2], phi[2][2].re);
    let det = p22 * p11 - p12.norm_sqr() / (1.0 + 1e-6);
    let a1 = if det != 0.0 {
        (p01 * p12 - p02 * p11) / det
    } else {
        C::default()
    };
    let a0 = if p11 != 0.0 {
        -(p01 + a1 * p12.conj()) / p11
    } else {
        C::default()
    };
    if a0.norm_sqr() >= 16.0 || a1.norm_sqr() >= 16.0 {
        (C::default(), C::default())
    } else {
        (a0, a1)
    }
}

/// The HF adjuster (clause 4.6.18.7): the gains from the transmitted and the measured envelopes, the limiter, the noise
/// floor, the sinusoids and the smoothing, applied to the high band of `st.y`.
fn adjust(st: &mut State, d: &Data, f: &Freq, h: &Header) {
    let g = &d.grid;
    let (kx, m) = (f.kx, f.m);
    let eps = 1.0f32;
    let eps0 = 1e-12f32;
    // The sinusoid of each subband: in the middle of a high resolution band with bs_add_harmonic set.
    let mut sine = [false; 64];
    for i in 0..f.n[1] {
        if d.harmonic[i] {
            let mid = (usize::from(f.table[1][i + 1]) + usize::from(f.table[1][i])) / 2;
            sine[mid - kx] = true;
        }
    }
    let h_sl = if h.smoothing_mode { 0 } else { 4 };
    let fill = !st.hist_full;
    st.hist_full = true;
    let mut first_slot = true;
    for l in 0..g.n_env {
        let res = usize::from(g.freq_res[l]);
        let table = &f.table[res][..=f.n[res]];
        let (i0, i1) = (2 * g.t_e[l], 2 * g.t_e[l + 1]);
        let q_env = if g.n_q == 2 && g.t_e[l] >= g.t_q[1] {
            1
        } else {
            0
        };
        let transient = g.l_a == Some(l) || (l == 0 && st.transient_end);
        let delta = if transient { 0.0 } else { 1.0 };
        let mut e_curr = [0f32; 64];
        let mut e_orig = [0f32; 64];
        let mut q_map = [0f32; 64];
        let mut s_index = [false; 64];
        let mut s_mapped = [false; 64];
        for b in 0..f.n[res] {
            let (lo, hi) = (usize::from(table[b]), usize::from(table[b + 1]));
            let mut any_sine = false;
            for k in lo..hi {
                let mm = k - kx;
                e_orig[mm] = st.e_orig[l][b];
                let qb = (0..f.n_q)
                    .rev()
                    .find(|&q| usize::from(f.noise[q]) <= k)
                    .unwrap_or(0);
                q_map[mm] = st.q_orig[q_env][qb];
                s_index[mm] = sine[mm] && (g.l_a.is_none_or(|la| l >= la) || st.sine_prev[mm]);
                any_sine |= s_index[mm];
            }
            let span = (i1 - i0) as f32;
            if h.interpol_freq {
                for k in lo..hi {
                    let e: f32 = (i0..i1).map(|i| st.y[i + T_HFADJ][k].norm_sqr()).sum();
                    e_curr[k - kx] = e / span;
                }
            } else {
                let e: f32 = (lo..hi)
                    .map(|k| {
                        (i0..i1)
                            .map(|i| st.y[i + T_HFADJ][k].norm_sqr())
                            .sum::<f32>()
                    })
                    .sum();
                let e = e / (span * (hi - lo) as f32);
                e_curr[lo - kx..hi - kx].fill(e);
            }
            s_mapped[lo - kx..hi - kx].fill(any_sine);
        }
        // Gains, noise and sinusoid levels by subband (clause 4.6.18.7.4).
        let mut gain = [0f32; 64];
        let mut q_m = [0f32; 64];
        let mut s_m = [0f32; 64];
        for mm in 0..m {
            let (eo, q) = (e_orig[mm], q_map[mm]);
            q_m[mm] = eo * q / (1.0 + q);
            s_m[mm] = if s_index[mm] { eo / (1.0 + q) } else { 0.0 };
            gain[mm] = if s_mapped[mm] {
                (eo / (eps + e_curr[mm]) * q / (1.0 + q)).sqrt()
            } else {
                (eo / ((eps + e_curr[mm]) * (1.0 + delta * q))).sqrt()
            };
        }
        // The limiter and the gain boost.
        for b in 0..f.n_lim {
            let (lo, hi) = (usize::from(f.lim[b]) - kx, usize::from(f.lim[b + 1]) - kx);
            if lo >= hi {
                continue;
            }
            let so: f32 = e_orig[lo..hi].iter().sum();
            let sc: f32 = e_curr[lo..hi].iter().sum();
            let g_max = (((eps0 + so) / (eps0 + sc)).sqrt()
                * LIM_GAIN[usize::from(h.limiter_gains)])
            .min(1e5);
            for mm in lo..hi {
                if gain[mm] > g_max {
                    q_m[mm] *= (g_max / gain[mm]).powi(2);
                    gain[mm] = g_max;
                }
            }
            let mut den = 0.0;
            for mm in lo..hi {
                den += e_curr[mm] * gain[mm] * gain[mm] + s_m[mm];
                if s_m[mm] == 0.0 && !transient {
                    den += q_m[mm];
                }
            }
            let boost = ((eps0 + so) / (eps0 + den)).sqrt().min(1.584_893_2);
            for mm in lo..hi {
                gain[mm] *= boost;
                q_m[mm] *= boost * boost;
                s_m[mm] *= boost * boost;
            }
        }
        // Noise and sinusoid levels as amplitudes.
        for mm in 0..m {
            q_m[mm] = q_m[mm].sqrt();
            s_m[mm] = s_m[mm].sqrt();
        }
        // Assembly (clause 4.6.18.7.5).
        for i in i0..i1 {
            if first_slot && fill {
                for j in 0..5 {
                    st.g_hist[j][..m].copy_from_slice(&gain[..m]);
                    st.q_hist[j][..m].copy_from_slice(&q_m[..m]);
                }
            }
            first_slot = false;
            st.g_hist.copy_within(0..4, 1);
            st.q_hist.copy_within(0..4, 1);
            st.g_hist[0][..m].copy_from_slice(&gain[..m]);
            st.q_hist[0][..m].copy_from_slice(&q_m[..m]);
            let sidx = st.sine_idx;
            st.sine_idx = (st.sine_idx + 1) & 3;
            let slot = &mut st.y[i + T_HFADJ];
            for mm in 0..m {
                let (gf, mut qf) = if h_sl == 0 || transient {
                    (gain[mm], q_m[mm])
                } else {
                    let mut gs = 0.0;
                    let mut qs = 0.0;
                    for (j, &w) in H_SMOOTH.iter().enumerate() {
                        gs += st.g_hist[j][mm] * w;
                        qs += st.q_hist[j][mm] * w;
                    }
                    (gs, qs)
                };
                if s_m[mm] != 0.0 || transient {
                    qf = 0.0;
                }
                st.noise_idx = (st.noise_idx + 1) & 511;
                let k = mm + kx;
                let mut v = slot[k] * gf;
                if qf != 0.0 {
                    let (nr, ni) = NOISE[st.noise_idx];
                    v += C::new(qf * nr, qf * ni);
                }
                if s_m[mm] != 0.0 {
                    let s = s_m[mm];
                    let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
                    v += match sidx {
                        0 => C::new(s, 0.0),
                        1 => C::new(0.0, s * sign),
                        2 => C::new(-s, 0.0),
                        _ => C::new(0.0, -s * sign),
                    };
                }
                slot[k] = v;
            }
        }
        if l == g.n_env - 1 {
            st.sine_prev[..m].copy_from_slice(&s_index[..m]);
        }
    }
    st.transient_end = g.l_a == Some(g.n_env);
}
