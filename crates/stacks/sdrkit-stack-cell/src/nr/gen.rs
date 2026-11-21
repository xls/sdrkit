//! The NR test signal: SS/PBCH bursts at 256 x SCS, each cell at its offset, resampled to the output rate.

use super::phy::*;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo,
};

/// One NR cell of the test signal.
#[derive(Clone, Debug)]
pub struct NrCellCfg {
    /// PCI.
    pub pci: u16,
    /// SSB subcarrier spacing, kHz.
    pub scs: u32,
    /// Lmax.
    pub lmax: u8,
    /// Beam gains per SSB index, dB.
    pub gains_db: Vec<f64>,
    /// The cell's SS_REF relative to the radio centre, Hz.
    pub offset_hz: f64,
    /// MIB (the SFN is the first frame's).
    pub mib: Mib,
    /// Burst period, ms (20 or 5).
    pub period_ms: u32,
    /// Start in the second half frame.
    pub second_half: bool,
}

fn default_gains() -> Vec<f64> {
    (0..8).map(|i: i32| -1.5 * (i - 2).abs() as f64).collect()
}

/// Scene 0 to 2 with the radio centre each assumes.
pub fn scene(n: u32) -> (f64, Vec<NrCellCfg>) {
    let mib = |sfn, c0, ss, k, barred| Mib {
        sfn,
        hrf: 0,
        scs_common_30: false,
        k_ssb: k,
        dmrs_pos3: false,
        coreset0: c0,
        search_space0: ss,
        barred,
        intra_freq_allowed: true,
    };
    match n {
        1 => (
            795.85e6,
            vec![NrCellCfg {
                pci: 222,
                scs: 15,
                lmax: 4,
                gains_db: vec![-3.0, -1.5, 0.0, -1.5],
                offset_hz: 0.0,
                mib: mib(100, 3, 0, 6, false),
                period_ms: 20,
                second_half: false,
            }],
        ),
        2 => (
            super::TEST_SIGNAL_HZ,
            vec![NrCellCfg {
                pci: 17,
                scs: 30,
                lmax: 8,
                gains_db: default_gains(),
                offset_hz: 0.0,
                mib: mib(900, 0, 0, 31, true),
                period_ms: 20,
                second_half: false,
            }],
        ),
        _ => (
            super::TEST_SIGNAL_HZ,
            vec![
                NrCellCfg {
                    pci: 501,
                    scs: 30,
                    lmax: 8,
                    gains_db: default_gains(),
                    offset_hz: -4.32e6,
                    mib: Mib {
                        scs_common_30: true,
                        ..mib(512, 12, 2, 4, false)
                    },
                    period_ms: 20,
                    second_half: false,
                },
                NrCellCfg {
                    pci: 1007,
                    scs: 30,
                    lmax: 8,
                    gains_db: vec![-12.0, -9.0, -6.0, -3.0, 0.0, -3.0, -6.0, -9.0],
                    offset_hz: 4.32e6,
                    mib: Mib {
                        scs_common_30: true,
                        ..mib(33, 6, 0, 18, false)
                    },
                    period_ms: 20,
                    second_half: false,
                },
            ],
        ),
    }
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct NrGenOptions {
    /// Random QPSK around the SSB and on the other symbols.
    pub load: bool,
    /// The seed of the random load.
    pub seed: u64,
}

impl Default for NrGenOptions {
    fn default() -> Self {
        NrGenOptions {
            load: true,
            seed: 1,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The cells of the test signal.
    pub enum NrScene {
        /// Two n78 cells.
        TwoN78 = ("0", "Two n78 cells"),
        /// One n28 cell.
        N28 = ("1", "n28 cell"),
        /// A barred n78 cell.
        BarredN78 = ("2", "Barred n78 cell"),
    }
}

/// The cells of the test signal.
pub const GEN_SCENE: Setting<field::Choice<NrScene>> =
    Setting::choice("gen_scene", "Test signal scene", NrScene::TwoN78)
        .section("Test signal")
        .advanced();

/// The test signal settings of the stack. The framework adds the standard `gen_*` settings (noise, offsets, impairments).
pub const SETTINGS: &[&dyn DeclareSetting] = &[&GEN_SCENE];

/// The generator of the test signal settings in `cfg`, with the cells placed against the scene's radio centre.
pub fn from_config(cfg: &GenConfig) -> NrGenerator {
    let opt = NrGenOptions {
        seed: cfg.seed,
        ..Default::default()
    };
    let sc = u32::from(cfg.settings.get(&GEN_SCENE).index());
    NrGenerator::new(cfg.rate_hz, scene(sc).1, opt)
}

struct CellState {
    cfg: NrCellCfg,
    ifft: Fft,
    half: usize,
    sfn: u16,
    native: Vec<Complex32>,
    rs: FractionalResampler<Complex32>,
    out: Vec<Complex32>,
    osc: Nco,
    data: Rng,
}

impl CellState {
    fn half_frame(&mut self, load: bool) {
        let scs = self.cfg.scs;
        let nsym = 70 * scs as usize / 15;
        let in_burst = {
            let hf = self.half % 2;
            let period_half = (self.cfg.period_ms / 5) as usize;
            if self.cfg.period_ms <= 5 {
                true
            } else {
                hf == 0 && self.half.is_multiple_of(period_half.max(1))
            }
        };
        let mut syms: Vec<[Complex32; 256]> = vec![[Complex32::new(0.0, 0.0); 256]; nsym];
        let gain_of =
            |i: usize, g: &[f64]| 10f64.powf(g.get(i).copied().unwrap_or(-100.0) / 20.0) as f32;
        let mut ssb_syms = vec![false; nsym];
        if in_burst {
            let pci = self.cfg.pci;
            let (nid1, nid2) = (pci / 3, pci % 3);
            let p = pss(nid2);
            let s = sss(nid1, nid2);
            for i in 0..self.cfg.lmax as usize {
                let g = gain_of(i, &self.cfg.gains_db);
                if g < 1e-4 {
                    continue;
                }
                let l0 = ssb_symbol(i);
                let mut grid = [[Complex32::new(0.0, 0.0); 240]; 4];
                for n in 0..127 {
                    grid[0][56 + n] = Complex32::new(p[n], 0.0);
                    grid[2][56 + n] = Complex32::new(s[n], 0.0);
                }
                let hf = (self.half % 2) as u8;
                let ibar = if self.cfg.lmax == 4 {
                    i as u8 + 4 * hf
                } else {
                    i as u8
                };
                let r = dmrs(pci, ibar);
                for (m, &(k, l)) in dmrs_res(pci).iter().enumerate() {
                    grid[l][k] = r[m];
                }
                let mib = Mib {
                    sfn: self.sfn,
                    hrf: hf,
                    ..self.cfg.mib.clone()
                };
                let bits = encode_pbch(&mib, pci, i as u8, self.cfg.lmax);
                let sq = std::f32::consts::FRAC_1_SQRT_2;
                for (m, &(k, l)) in pbch_res(pci).iter().enumerate() {
                    grid[l][k] = Complex32::new(
                        sq * (1.0 - 2.0 * bits[2 * m] as f32),
                        sq * (1.0 - 2.0 * bits[2 * m + 1] as f32),
                    );
                }
                for (j, row) in grid.iter().enumerate() {
                    let l = l0 + j;
                    if l < nsym {
                        ssb_syms[l] = true;
                        for k in 0..240 {
                            let bin = (k as i64 - 120).rem_euclid(256) as usize;
                            syms[l][bin] = row[k] * g;
                        }
                    }
                }
            }
        }
        if load {
            let sq = std::f32::consts::FRAC_1_SQRT_2;
            for (l, row) in syms.iter_mut().enumerate() {
                for k in 0..240usize {
                    let in_ssb = ssb_syms[l]
                        && (row[(k as i64 - 120).rem_euclid(256) as usize].norm() > 0.0);
                    if in_ssb {
                        continue;
                    }
                    let bin = (k as i64 - 120).rem_euclid(256) as usize;
                    let (a, b) = (self.data.bit(), self.data.bit());
                    row[bin] =
                        Complex32::new(sq * (1.0 - 2.0 * a as f32), sq * (1.0 - 2.0 * b as f32))
                            * 0.5;
                }
            }
        }
        // Unit mean power on the SSB symbols: 240 unit REs over 256 bins.
        let scale = 1.0 / 240f32.sqrt();
        for (l, row) in syms.iter_mut().enumerate() {
            let mut buf = row.to_vec();
            self.ifft.process(&mut buf);
            let (_, cp) = symbol_start(scs, l);
            for n in (256 - cp)..256 {
                self.native.push(buf[n] * scale);
            }
            for v in &buf {
                self.native.push(v * scale);
            }
        }
        self.half += 1;
        if self.half.is_multiple_of(2) {
            self.sfn = (self.sfn + 1) % 1024;
        }
    }
}

/// The NR generator.
pub struct NrGenerator {
    rate: f64,
    cells: Vec<CellState>,
    max_scs_hz: f64,
    load: bool,
    buf: Vec<Complex32>,
    pos: usize,
}

impl NrGenerator {
    /// Cells at their offsets from the radio centre.
    pub fn new(rate: f64, cells: Vec<NrCellCfg>, opt: NrGenOptions) -> Self {
        let max_scs_hz = cells.iter().map(|c| c.scs).max().unwrap_or(15) as f64 * 1e3;
        let cells = cells
            .into_iter()
            .map(|cfg| {
                let native = 256.0 * cfg.scs as f64 * 1e3;
                let rs = FractionalResampler::with_rates(native, rate).expect("rates");
                CellState {
                    ifft: Fft::inverse(256),
                    half: if cfg.second_half { 1 } else { 0 },
                    sfn: cfg.mib.sfn,
                    native: Vec::new(),
                    rs,
                    out: Vec::new(),
                    osc: Nco::new(rate, cfg.offset_hz),
                    data: Rng::new(opt.seed ^ cfg.pci as u64),
                    cfg,
                }
            })
            .collect();
        NrGenerator {
            rate,
            cells,
            max_scs_hz,
            load: opt.load,
            buf: Vec::new(),
            pos: 0,
        }
    }

    fn refill(&mut self) {
        for c in self.cells.iter_mut() {
            while c.out.len() < 20000 {
                c.native.clear();
                c.half_frame(self.load);
                let native = std::mem::take(&mut c.native);
                let mut o = Vec::new();
                c.rs.process(&native, &mut o);
                c.osc.mix_in_place(&mut o);
                c.out.extend(o);
                c.native = native;
            }
        }
        let n = self.cells.iter().map(|c| c.out.len()).min().unwrap_or(0);
        let mut sum = vec![Complex32::new(0.0, 0.0); n];
        for c in self.cells.iter_mut() {
            for (s, v) in sum.iter_mut().zip(c.out.drain(..n)) {
                *s += v;
            }
        }
        self.buf.drain(..self.pos);
        self.pos = 0;
        self.buf.extend(sum);
    }
}

impl Generator for NrGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos >= self.buf.len() {
                self.refill();
                if self.buf.is_empty() {
                    break;
                }
            }
            let n = (self.buf.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        i
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The SS/PBCH block: 240 subcarriers (38.211 7.4.3). The SNR of the tests is the SNR in this band.
        Some(SignalInfo::new(240.0 * self.max_scs_hz))
    }
}
