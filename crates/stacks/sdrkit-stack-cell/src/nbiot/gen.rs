//! The NB-IoT test signal: one carrier at 1.92 Msps with NPSS, NSSS, NRS, NPBCH and SIB1-NB, resampled to the
//! output rate and moved by the raster offset of its scene.

use super::msg::{MibNb, OperationMode, Plmn, Sib1Nb};
use super::phy::*;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo,
};

/// A scene of the generator.
#[derive(Clone, Debug)]
pub struct NbScene {
    /// Operation mode.
    pub mode: OperationMode,
    /// PCI.
    pub pci: u16,
    /// NRS ports.
    pub ports: usize,
    /// schedulingInfoSIB1.
    pub sched: u8,
    /// Band.
    pub band: u16,
    /// TAC.
    pub tac: u16,
    /// Cell identity.
    pub cell_id: u32,
    /// Raster offset, Hz.
    pub raster_hz: f64,
    /// The in-band LTE cell's CRS: (ports, shift).
    pub crs: Option<(usize, usize)>,
    /// First frame number.
    pub first_frame: u32,
}

/// Scene 0 to 3.
pub fn scene(n: u32) -> NbScene {
    let first_frame = 37 * (n + 1);
    match n {
        1 => NbScene {
            mode: OperationMode::Guardband(OperationMode::raster_index(-7.5)),
            pci: 250,
            ports: 2,
            sched: 2,
            band: 20,
            tac: 0x2B3C,
            cell_id: 0x0234567,
            raster_hz: -7.5e3,
            crs: None,
            first_frame,
        },
        2 => NbScene {
            mode: OperationMode::InbandSamePci(9),
            pci: 17,
            ports: 2,
            sched: 0,
            band: 3,
            tac: 0x3C4D,
            cell_id: 0x0345678,
            raster_hz: -2.5e3,
            crs: Some((2, 17 % 6)),
            first_frame,
        },
        3 => NbScene {
            mode: OperationMode::InbandDifferentPci {
                four_ports: true,
                raster: OperationMode::raster_index(7.5),
            },
            pci: 400,
            ports: 1,
            sched: 4,
            band: 20,
            tac: 0x4D5E,
            cell_id: 0x0456789,
            raster_hz: 7.5e3,
            crs: Some((4, (400 + 2) % 6)),
            first_frame,
        },
        _ => NbScene {
            mode: OperationMode::Standalone,
            pci: 100,
            ports: 1,
            sched: 1,
            band: 8,
            tac: 0x1A2B,
            cell_id: 0x0123456,
            raster_hz: 2.5e3,
            crs: None,
            first_frame,
        },
    }
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct NbGenOptions {
    /// Random QPSK in the other downlink subframes.
    pub load: bool,
    /// The seed of the random load.
    pub seed: u64,
    /// Apply the per-frame NPBCH phase rotation of later releases (off: the signal the tests are built on).
    pub npbch_rotation: bool,
}

impl Default for NbGenOptions {
    fn default() -> Self {
        NbGenOptions {
            load: true,
            seed: 1,
            npbch_rotation: false,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The cell of the test signal.
    pub enum NbSceneChoice {
        /// A standalone carrier.
        Standalone = ("0", "Standalone"),
        /// A carrier in the guard band of an LTE carrier.
        GuardBand = ("1", "Guard band"),
        /// A carrier inside an LTE carrier, with the same PCI.
        InbandSame = ("2", "In-band, same PCI"),
        /// A carrier inside an LTE carrier, with another PCI.
        InbandOther = ("3", "In-band, other PCI"),
    }
}

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The cell of the test signal.
pub const GEN_SCENE: Setting<field::Choice<NbSceneChoice>> =
    Setting::choice("gen_scene", "Test signal scene", NbSceneChoice::Standalone)
        .section(TEST_SIGNAL)
        .advanced();
/// Whether the test signal fills the unused subframes.
pub const GEN_LOAD: Setting<field::Bool> = Setting::new("gen_load", "Test signal load", "", true)
    .section(TEST_SIGNAL)
    .advanced();
/// Whether the test signal rotates the NPBCH.
pub const GEN_NPBCH_ROTATION: Setting<field::Bool> = Setting::new(
    "gen_npbch_rotation",
    "Test signal NPBCH rotation",
    "",
    false,
)
.section(TEST_SIGNAL)
.advanced();

/// The test signal settings of the stack. The framework adds the standard `gen_*` settings (noise, offsets, impairments).
pub const SETTINGS: &[&dyn DeclareSetting] = &[&GEN_SCENE, &GEN_LOAD, &GEN_NPBCH_ROTATION];

/// The scene and the options that the test signal settings of `cfg` give.
pub fn options(cfg: &GenConfig) -> (NbScene, NbGenOptions) {
    let s = &cfg.settings;
    let o = NbGenOptions {
        seed: cfg.seed,
        load: s.get(&GEN_LOAD),
        npbch_rotation: s.get(&GEN_NPBCH_ROTATION),
    };
    (scene(u32::from(s.get(&GEN_SCENE).index())), o)
}

/// The NB-IoT generator.
pub struct NbGenerator {
    rate: f64,
    sc: NbScene,
    opt: NbGenOptions,
    ofdm: Ofdm,
    n_f: u32,
    native: Vec<Complex32>,
    out: Vec<Complex32>,
    pos: usize,
    rs: FractionalResampler<Complex32>,
    osc: Nco,
    data: Rng,
    mib_coded: Vec<u8>,
    mib_for: Option<u32>,
    sib1_bits: Vec<u8>,
    sib1_tbs: usize,
}

/// The amplitude of the carrier: unit mean power over its 12 subcarriers.
const A: f64 = 1.0;

impl NbGenerator {
    /// A generator of scene `sc` at `rate`.
    pub fn new(rate: f64, sc: NbScene, opt: NbGenOptions) -> Self {
        let rs = FractionalResampler::with_rates(RATE, rate).expect("rates");
        let sib = Sib1Nb {
            hsfn_msb: 3,
            plmns: vec![(Plmn::new(1, 1, 2), false)],
            tac: sc.tac,
            cell_id: sc.cell_id,
            barred: false,
            intra_freq_reselection: true,
            q_rx_lev_min_dbm: -128,
            q_qual_min_db: -20,
            p_max_dbm: Some(23),
            band: sc.band,
        };
        let (_, tbs) = sib1_schedule(sc.sched).unwrap_or((8, 208));
        NbGenerator {
            rate,
            osc: Nco::new(rate, sc.raster_hz),
            n_f: sc.first_frame % 1024,
            sib1_bits: sib.encode(),
            sib1_tbs: tbs,
            data: Rng::new(opt.seed ^ 0x5EED),
            sc,
            opt,
            ofdm: Ofdm::default(),
            native: Vec::new(),
            out: Vec::new(),
            pos: 0,
            rs,
            mib_coded: Vec::new(),
            mib_for: None,
        }
    }

    fn mib(&mut self) -> &[u8] {
        let block = self.n_f / 64;
        if self.mib_for != Some(block) {
            let m = MibNb {
                sfn_msb: (self.n_f >> 6) as u8 & 15,
                hsfn_lsb: 1,
                scheduling_info_sib1: self.sc.sched,
                value_tag: 5,
                access_barring: false,
                mode: self.sc.mode,
                additional_sib1: false,
            };
            self.mib_coded = encode_mib_nb(&m.to_bits(), self.sc.ports);
            let c = npbch_scrambling(self.sc.pci);
            for (b, s) in self.mib_coded.iter_mut().zip(c) {
                *b ^= s;
            }
            self.mib_for = Some(block);
        }
        &self.mib_coded
    }

    fn sib1_subframe(&self) -> Option<(u32, usize)> {
        let (reps, _) = sib1_schedule(self.sc.sched)?;
        let start = sib1_start(reps, self.sc.pci);
        let period = 256 / reps;
        let f = self.n_f % 256;
        if f < start {
            return None;
        }
        let rel = (f - start) % period;
        if rel < 16 && rel.is_multiple_of(2) {
            Some((self.n_f - rel, (rel / 2) as usize))
        } else {
            None
        }
    }

    fn put(
        &self,
        grid: &mut [[[Complex32; 12]; 14]; 2],
        ports: usize,
        res: &[(usize, usize)],
        sym: &[Complex32],
    ) {
        if ports == 1 {
            for (&(k, l), &x) in res.iter().zip(sym) {
                grid[0][l][k] = x;
            }
        } else {
            for (i, pair) in res.chunks(2).enumerate() {
                let x0 = sym.get(2 * i).copied().unwrap_or_default();
                let x1 = sym.get(2 * i + 1).copied().unwrap_or_default();
                let y = sfbc(x0, x1);
                for (j, &(k, l)) in pair.iter().enumerate() {
                    grid[0][l][k] = y[0][j];
                    grid[1][l][k] = y[1][j];
                }
            }
        }
    }

    fn frame(&mut self) {
        let pci = self.sc.pci;
        let ports = self.sc.ports;
        let n_f = self.n_f;
        let p1 = Complex32::from_polar(0.8, 0.6);
        for sfn in 0..10usize {
            let mut grid = [[[Complex32::new(0.0, 0.0); 12]; 14]; 2];
            let mut nrs = true;
            if sfn == 5 {
                let zc = npss_zc();
                for l in 3..14 {
                    for k in 0..11 {
                        grid[0][l][k] = zc[k] * NPSS_COVER[l - 3];
                    }
                }
                nrs = false;
            } else if sfn == 9 && n_f.is_multiple_of(2) {
                let d = nsss(pci, n_f);
                for (n, v) in d.iter().enumerate() {
                    grid[0][3 + n / 12][n % 12] = *v;
                }
                nrs = false;
            } else if sfn == 0 {
                let seg = ((n_f % 64) / 8) as usize;
                let coded = self.mib()[seg * 200..seg * 200 + 200].to_vec();
                let mut sym: Vec<Complex32> = coded.chunks(2).map(|c| qpsk(c[0], c[1])).collect();
                if self.opt.npbch_rotation {
                    for (x, r) in sym.iter_mut().zip(npbch_rotation(pci, n_f)) {
                        *x *= rotation_factor(r);
                    }
                }
                let res = npbch_res(pci);
                self.put(&mut grid, ports, &res, &sym);
            } else if let Some((first, idx)) = self.sib1_subframe().filter(|_| sfn == 4) {
                let crs = self.sc.crs.map(|(ports, shift)| (shift, ports));
                let l0 = if crs.is_some() { 3 } else { 0 };
                let res = npdsch_res(pci, ports, l0, crs);
                let g = 2 * res.len();
                let e = 8 * g;
                let coded = encode_sib1(&self.sib1_bits, self.sib1_tbs, e);
                let c = sib1_scrambling(pci, first, e);
                let bits: Vec<u8> = coded[idx * g..idx * g + g]
                    .iter()
                    .zip(&c[idx * g..idx * g + g])
                    .map(|(a, b)| a ^ b)
                    .collect();
                let sym: Vec<Complex32> = bits.chunks(2).map(|c| qpsk(c[0], c[1])).collect();
                self.put(&mut grid, ports, &res, &sym);
            } else if self.opt.load {
                let crs = self.sc.crs.map(|(ports, shift)| (shift, ports));
                let l0 = if crs.is_some() { 3 } else { 0 };
                let res = npdsch_res(pci, ports, l0, crs);
                let sym: Vec<Complex32> = (0..res.len())
                    .map(|_| qpsk(self.data.bit(), self.data.bit()))
                    .collect();
                self.put(&mut grid, ports, &res, &sym);
            }
            if nrs {
                for l in [5usize, 6, 12, 13] {
                    let ns = 2 * sfn + l / 7;
                    for port in 0..ports {
                        for (m, &k) in nrs_subcarriers(pci, port, l).iter().enumerate() {
                            grid[port][l][k] = nrs_value(pci, ns, l, m);
                        }
                    }
                }
            }
            // The in-band LTE cell's CRS (pattern only: QPSK of the cell's own sequence is not needed for these tests).
            if let Some((cp, shift)) = self.sc.crs {
                for l in 0..14 {
                    for k in 0..12 {
                        if is_crs(shift, cp, k, l) && !(is_nrs(pci, k, l) && nrs) {
                            let ns = (2 * sfn + l / 7) as u32;
                            grid[0][l][k] =
                                qpsk((ns as usize + k) as u8 & 1, (l + k / 3) as u8 & 1);
                        }
                    }
                }
            }
            // Ports summed (port 1 through its own gain and phase).
            let gain = (A / 12f64.sqrt()) as f32;
            for l in 0..14 {
                let mut a = [Complex32::new(0.0, 0.0); 12];
                for k in 0..12 {
                    a[k] = (grid[0][l][k] + grid[1][l][k] * p1) * gain;
                }
                self.ofdm.modulate(&a, l, &mut self.native);
            }
        }
        self.n_f = (self.n_f + 1) % 1024;
    }
}

impl Generator for NbGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos >= self.out.len() {
                self.out.clear();
                self.pos = 0;
                self.native.clear();
                self.frame();
                let native = std::mem::take(&mut self.native);
                self.rs.process(&native, &mut self.out);
                self.native = native;
                self.osc.mix_in_place(&mut self.out);
            }
            let n = (self.out.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.out[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        i
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // One NB-IoT carrier: 12 subcarriers of 15 kHz (36.211 10.2). The SNR of the tests is the SNR in this band.
        Some(SignalInfo::new(180e3))
    }
}

/// The generator of the test signal settings in `cfg`.
pub fn from_config(cfg: &GenConfig) -> NbGenerator {
    let (sc, opt) = options(cfg);
    NbGenerator::new(cfg.rate_hz, sc, opt)
}
