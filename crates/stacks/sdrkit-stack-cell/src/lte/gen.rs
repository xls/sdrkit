//! The LTE test signal: one eNodeB carrier with PSS, SSS, CRS, PBCH, PCFICH, PDCCH and PDSCH carrying SIB1 to
//! SIB5 on SI-RNTI, a P-RNTI assignment in subframe 9 (to check it is never decoded), and random load, at 15 kHz x FFT, resampled
//! to the output rate.

use super::msg::{Bcch, InterFreq, Mib, Plmn, Sib1, Sib2, Sib3, Sib4, Sib5, SibEntry};
use super::phy::*;
use crate::bands::earfcn_band;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_fec::turbo::size_at_least;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, DeclareSetting, GenConfig, Generator, Hz, Setting, SignalInfo,
};

/// SI-RNTI.
pub const SI_RNTI: u32 = 0xFFFF;
/// P-RNTI.
pub const P_RNTI: u32 = 0xFFFE;

/// A cell of the test signal.
#[derive(Clone, Debug)]
pub struct LteCell {
    /// Bandwidth in RBs.
    pub n_rb: usize,
    /// CRS ports.
    pub ports: usize,
    /// PCI.
    pub pci: u16,
    /// eNB identity.
    pub enb: u32,
    /// Sector.
    pub sector: u32,
    /// Band.
    pub band: u16,
    /// EARFCN.
    pub earfcn: u32,
    /// TAC.
    pub tac: u16,
    /// Relative gain, dB.
    pub gain_db: f64,
    /// Multipath per port: up to two (delay at 30.72 Msps, gain dB) taps besides the direct path.
    pub multipath: Vec<(usize, f64)>,
}

/// Scene 0 to 3 (with an optional PCI override).
pub fn scene(n: u32, pci: Option<u16>) -> LteCell {
    let (n_rb, ports, p, enb, sector, band, earfcn) = match n {
        1 => (25, 4, 301, 7001, 1, 3, 1300),
        2 => (6, 1, 7, 99, 0, 8, 3500),
        3 => (100, 2, 450, 900001, 3, 1, 300),
        _ => (50, 2, 123, 54321, 2, 20, 6300),
    };
    LteCell {
        n_rb,
        ports,
        pci: pci.unwrap_or(p),
        enb,
        sector,
        band,
        earfcn,
        tac: 0x1A2B + n as u16,
        gain_db: 0.0,
        multipath: Vec::new(),
    }
}

/// The SIB1 of a cell.
pub fn sib1_of(c: &LteCell) -> Sib1 {
    Sib1 {
        plmns: vec![(Plmn::new(1, 1, 2), false)],
        tac: c.tac,
        cell_id: c.enb * 256 + c.sector,
        barred: false,
        intra_freq_allowed: true,
        q_rx_lev_min_dbm: -124,
        p_max_dbm: Some(23),
        band: c.band,
        scheduling: vec![(16, vec![]), (32, vec![3]), (64, vec![4, 5]), (64, vec![6, 7, 8, 24])],
        si_window_ms: 20,
        value_tag: 5,
        ..Default::default()
    }
}

/// The SIB2 of a cell.
pub fn sib2_of(c: &LteCell) -> Sib2 {
    Sib2 {
        ra_preambles: 52,
        ramp_step: 1,
        target_power: 6,
        preamble_trans_max: 10,
        ra_window: 10,
        cr_timer: 7,
        max_harq_msg3: 4,
        mod_period_coeff: 4,
        paging_cycle: 128,
        nb: 2,
        root_sequence: 22,
        prach_config: 3,
        high_speed: false,
        zcz: 12,
        prach_freq_offset: 4,
        ref_signal_power: 15,
        p_b: 1,
        pusch: (1, false, 4, false, false, 0, false, 0),
        pucch: (0, 2, 0, 36),
        srs: None,
        p0_pusch: -96,
        alpha: 7,
        p0_pucch: -110,
        delta_f: [1, 1, 1, 1, 1],
        delta_preamble_msg3: 4,
        ul_cp_len2: false,
        timers: (1000, 1000, 1000, 10, 10000, 1),
        ul_earfcn: Some((c.earfcn + 18000) as u16),
        ul_bw_rb: Some(c.n_rb as u8),
        add_spectrum_emission: 1,
        time_alignment: None,
        ..Default::default()
    }
}

/// The SIB3, SIB4 and SIB5 of a cell.
pub fn sib345_of(c: &LteCell) -> (Sib3, Sib4, Sib5) {
    let s3 = Sib3 {
        q_hyst_db: 4,
        s_non_intra: Some(4),
        thresh_serving_low: 3,
        priority: 5,
        q_rx_lev_min_dbm: -124,
        s_intra: Some(31),
        antenna_port1: c.ports > 1,
        neigh_cell_config: 1,
        t_reselection: 1,
        ..Default::default()
    };
    let s4 = Sib4 {
        neighbours: vec![((c.pci + 1) % 504, 0), ((c.pci + 2) % 504, 2), (17, -2)],
        ..Default::default()
    };
    let s5 = Sib5 {
        carriers: vec![
            InterFreq {
                earfcn: 1300,
                q_rx_lev_min_dbm: -124,
                t_reselection: 1,
                thresh_high: 5,
                thresh_low: 2,
                allowed_bw: 4,
                antenna_port1: true,
                priority: Some(6),
                neigh_cell_config: 1,
                neighbours: vec![(88, 0)],
                ..Default::default()
            },
            InterFreq {
                earfcn: 3500,
                q_rx_lev_min_dbm: -120,
                t_reselection: 1,
                thresh_high: 6,
                thresh_low: 1,
                allowed_bw: 3,
                antenna_port1: true,
                priority: Some(3),
                neigh_cell_config: 1,
                ..Default::default()
            },
        ],
    };
    (s3, s4, s5)
}

/// The SIB6, SIB7, SIB8 and SIB24 of the test signal: UMTS FDD carriers 10563 and 4436 and a TDD carrier, two GSM groups
/// (an explicit list and a bitmap), a CDMA2000 1xRTT carrier with three cells, and the NR carriers 643334 (n78) and 152690
/// (n28).
pub fn irat_sibs() -> Vec<SibEntry> {
    use super::irat::*;
    let utra = |uarfcn, priority, q| UtraCarrier {
        uarfcn,
        priority,
        thresh_high: 8,
        thresh_low: 4,
        q_rx_lev_min: q,
        p_max: 24,
        q_qual_min: Some(-18),
    };
    let s6 = Sib6 {
        fdd: vec![utra(10563, Some(2), -58), utra(4436, None, -60)],
        tdd: vec![UtraCarrier { q_qual_min: None, ..utra(10088, Some(1), -50) }],
        t_reselection: 2,
        speed: None,
    };
    let s7 = Sib7 {
        t_reselection: 1,
        speed: Some((2, 1)),
        groups: vec![
            GeranGroup {
                first: 10,
                following: GeranFollowing::Explicit(vec![14, 60]),
                priority: Some(1),
                ncc_permitted: 0xFF,
                q_rx_lev_min: 5,
                thresh_high: 6,
                thresh_low: 2,
                ..Default::default()
            },
            GeranGroup {
                first: 600,
                pcs1900: false,
                following: GeranFollowing::Bitmap(vec![0b1000_0001]),
                ncc_permitted: 0x0F,
                q_rx_lev_min: 3,
                p_max: Some(33),
                thresh_high: 5,
                thresh_low: 1,
                ..Default::default()
            },
        ],
    };
    let s8 = Sib8 {
        search_window: Some(8),
        one_x_present: true,
        one_x_long_code: Some(0x2_1234_5678),
        one_x: Some(CdmaReselection {
            bands: vec![CdmaBand { band_class: 0, priority: Some(1), thresh_high: 20, thresh_low: 10 }],
            carriers: vec![CdmaCarrier { band_class: 0, arfcn: 283, pn: vec![12, 108, 300] }],
            t_reselection: 2,
            speed: None,
        }),
        ..Default::default()
    };
    let nr = |nr_arfcn, band, scs, priority| NrCarrier {
        nr_arfcn,
        bands: vec![band],
        mtc: Some((2, 0, 0)),
        ssb_scs_khz: scs,
        priority: Some(priority),
        thresh_high: 10,
        thresh_low: 4,
        q_rx_lev_min: -64,
        p_max: 23,
        ns_pmax: vec![(Some(20), 1)],
        derive_ssb_index: true,
        ..Default::default()
    };
    let s24 = Sib24 {
        carriers: vec![nr(643_334, 78, 30, 6), NrCarrier { thresh_q: Some((3, 1)), ..nr(152_690, 28, 15, 5) }],
        t_reselection: 1,
        speed: None,
        late: None,
    };
    vec![SibEntry::Sib6(s6), SibEntry::Sib7(s7), SibEntry::Sib8(Box::new(s8)), SibEntry::Sib24(s24)]
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct LteGenOptions {
    /// Random QPSK on the free RBs.
    pub load: bool,
    /// The first frame's SFN.
    pub sfn0: u32,
    /// Leave out this many samples at the start (a start inside a frame).
    pub skip: usize,
    /// The seed of the random load and the paging data.
    pub seed: u64,
}

impl Default for LteGenOptions {
    fn default() -> Self {
        LteGenOptions {
            load: true,
            sfn0: 0,
            skip: 0,
            seed: 1,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The cell of the test signal.
    pub enum LteScene {
        /// A 10 MHz cell with two ports.
        Ten = ("0", "10 MHz, 2 ports"),
        /// A 5 MHz cell with four ports.
        Five = ("1", "5 MHz, 4 ports"),
        /// A 1.4 MHz cell.
        Narrow = ("2", "1.4 MHz"),
        /// A 20 MHz cell.
        Twenty = ("3", "20 MHz"),
    }
}

/// The cell of the test signal.
pub const GEN_SCENE: Setting<field::Choice<LteScene>> =
    Setting::choice("gen_scene", "Test signal scene", LteScene::Ten)
        .section(TEST_SIGNAL)
        .advanced();
/// The SFN of the first frame of the test signal.
pub const GEN_SFN0: Setting<field::I64> = Setting::new("gen_sfn0", "Test signal first SFN", "", 0)
    .range(0.0, 1023.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();
/// Whether the test signal fills the unused resource blocks.
pub const GEN_LOAD: Setting<field::Bool> = Setting::new("gen_load", "Test signal load", "", true)
    .section(TEST_SIGNAL)
    .advanced();

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";

/// The test signal settings of the stack. The framework adds the standard `gen_*` settings (noise, offsets, impairments).
pub const SETTINGS: &[&dyn DeclareSetting] = &[&GEN_SCENE, &GEN_SFN0, &GEN_LOAD];

/// The cell and the options that the test signal settings of `cfg` give.
pub fn options(cfg: &GenConfig) -> (LteCell, LteGenOptions) {
    let s = &cfg.settings;
    let o = LteGenOptions {
        seed: cfg.seed,
        sfn0: s.get(&GEN_SFN0) as u32,
        load: s.get(&GEN_LOAD),
        ..Default::default()
    };
    (scene(u32::from(s.get(&GEN_SCENE).index()), None), o)
}

/// The generator of the test signal settings in `cfg`.
pub fn from_config(cfg: &GenConfig) -> Box<dyn Generator> {
    let (cell, opt) = options(cfg);
    Box::new(LteGenerator::new(cfg.rate_hz, cell, opt))
}

struct Assignment {
    rnti: u32,
    dci: Dci,
    tb: Vec<u8>,
    rv: usize,
}

/// The encoded transport block of a message for 1A (column 2 then 3) or 1C.
fn allocate(
    msg: &[u8],
    format: DciFormat,
    n_rb: usize,
    ports: usize,
    sib1: bool,
    rv: usize,
) -> Option<(Dci, Vec<u8>)> {
    let b = match ports {
        1 => 128,
        2 => 116,
        _ => 104,
    };
    let (mcs, tbs, tpc) = match format {
        DciFormat::F1a => {
            if let Some(i) = TBS_COL2.iter().position(|&t| t as usize >= msg.len()) {
                (i, TBS_COL2[i] as usize, 0)
            } else {
                let i = TBS_COL3.iter().position(|&t| t as usize >= msg.len())?;
                (i, TBS_COL3[i] as usize, 1)
            }
        }
        DciFormat::F1c => {
            let i = TBS_1C.iter().position(|&t| t as usize >= msg.len())?;
            (i, TBS_1C[i] as usize, 0)
        }
    };
    let mut l = ((tbs + 24) * 3 / (2 * b) + 1).max(2);
    if format == DciFormat::F1c {
        let st = n_step(n_rb);
        l = l.div_ceil(st) * st;
    }
    let cap = if format == DciFormat::F1c || !sib1 {
        let st = if format == DciFormat::F1c {
            n_step(n_rb)
        } else {
            1
        };
        (n_vrb_gap1(n_rb).min(16) / st) * st
    } else {
        n_rb.min(16)
    };
    l = l.min(cap).max(1);
    let (distributed, start) = if sib1 {
        (false, n_rb.saturating_sub(l + 1))
    } else {
        (true, 0)
    };
    let mut tb = msg.to_vec();
    tb.resize(tbs, 0);
    Some((
        Dci {
            format,
            distributed,
            gap2: false,
            start,
            len: l,
            mcs: mcs as u8,
            harq: 0,
            ndi: 0,
            rv: rv as u8,
            tpc,
        },
        tb,
    ))
}

/// The LTE generator.
pub struct LteGenerator {
    rate: f64,
    cell: LteCell,
    opt: LteGenOptions,
    n: usize,
    native_rate: f64,
    ifft: Fft,
    sfn: u32,
    native: Vec<Complex32>,
    port_bufs: Vec<Vec<Complex32>>,
    tail: Vec<Vec<Complex32>>,
    out: Vec<Complex32>,
    pos: usize,
    rs: FractionalResampler<Complex32>,
    data: Rng,
    sib1: Vec<u8>,
    si2: Vec<u8>,
    si3: Vec<u8>,
    si45: Vec<u8>,
    si_irat: Vec<u8>,
    skip: usize,
}

impl LteGenerator {
    /// A generator of `cell` at `rate`.
    pub fn new(rate: f64, cell: LteCell, opt: LteGenOptions) -> Self {
        let n = fft_size(cell.n_rb);
        let native_rate = 15e3 * n as f64;
        let rs = FractionalResampler::with_rates(native_rate, rate).expect("rates");
        let (s3, s4, s5) = sib345_of(&cell);
        let enc = |b: Bcch| b.encode();
        LteGenerator {
            rate,
            n,
            native_rate,
            ifft: Fft::inverse(n),
            sfn: opt.sfn0 % 1024,
            native: Vec::new(),
            port_bufs: vec![Vec::new(); 4],
            tail: vec![Vec::new(); 4],
            out: Vec::new(),
            pos: 0,
            rs,
            data: Rng::new(7 + cell.pci as u64 + opt.seed * 1000),
            sib1: enc(Bcch::Sib1(Box::new(sib1_of(&cell)))),
            si2: enc(Bcch::Si(vec![SibEntry::Sib2(Box::new(sib2_of(&cell)))])),
            si3: enc(Bcch::Si(vec![SibEntry::Sib3(s3)])),
            si45: enc(Bcch::Si(vec![SibEntry::Sib4(s4), SibEntry::Sib5(s5)])),
            si_irat: enc(Bcch::Si(irat_sibs())),
            skip: opt.skip,
            cell,
            opt,
        }
    }

    /// The cell.
    pub fn cell(&self) -> &LteCell {
        &self.cell
    }

    fn assignments(&mut self, sf: usize) -> Vec<Assignment> {
        let sfn = self.sfn;
        let c = &self.cell;
        let mut v = Vec::new();
        let mut add = |msg: &[u8], f: DciFormat, sib1: bool, rv: usize, rnti: u32| {
            if let Some((dci, tb)) = allocate(msg, f, c.n_rb, c.ports, sib1, rv) {
                v.push(Assignment { rnti, dci, tb, rv });
            }
        };
        if sf == 5 && sfn.is_multiple_of(2) {
            add(
                &self.sib1.clone(),
                DciFormat::F1a,
                true,
                sib1_rv(sfn),
                SI_RNTI,
            );
        }
        if sf == 3 && sfn.is_multiple_of(16) {
            add(&self.si2.clone(), DciFormat::F1a, false, 0, SI_RNTI);
        }
        if sf == 4 && sfn % 32 == 2 {
            add(&self.si3.clone(), DciFormat::F1a, false, 0, SI_RNTI);
        }
        if sf == 1 && sfn % 64 == 4 {
            add(&self.si45.clone(), DciFormat::F1c, false, 0, SI_RNTI);
        }
        // The fourth SI message: its window starts in the frames with SFN mod 64 = 6 (TS 36.331 clause 5.2.3).
        if sf == 2 && sfn % 64 == 6 {
            add(&self.si_irat.clone(), DciFormat::F1a, false, 0, SI_RNTI);
        }
        if sf == 9 {
            let tb: Vec<u8> = (0..72).map(|_| self.data.bit()).collect();
            let dci = Dci {
                format: DciFormat::F1a,
                distributed: false,
                gap2: false,
                start: 1,
                len: 3.min(c.n_rb - 1),
                mcs: 2,
                harq: 0,
                ndi: 0,
                rv: 0,
                tpc: 0,
            };
            v.push(Assignment {
                rnti: P_RNTI,
                dci,
                tb,
                rv: 0,
            });
        }
        v
    }

    fn subframe(&mut self, sf: usize) {
        let c = self.cell.clone();
        let n_rb = c.n_rb;
        let nsc = 12 * n_rb;
        let ports = c.ports;
        let pci = c.pci;
        let nsym = 14;
        let mut grid = vec![vec![vec![Complex32::new(0.0, 0.0); nsc]; nsym]; ports];
        let mut used = vec![vec![false; nsc]; nsym];
        // CRS.
        for p in 0..ports {
            for l in 0..nsym {
                let ns = 2 * sf + l / 7;
                for (k, v) in crs_res(pci, n_rb, p, ns, l % 7, false) {
                    grid[p][l][k] = v;
                }
            }
        }
        for l in 0..nsym {
            for (k, u) in used[l].iter_mut().enumerate() {
                *u = is_crs(pci, ports, l, k, false);
            }
        }
        // PSS and SSS.
        if sf == 0 || sf == 5 {
            let p = pss(pci % 3);
            let s = sss(pci / 3, pci % 3, sf == 5);
            for n in 0..62 {
                let k = 6 * n_rb - 31 + n;
                grid[0][6][k] = p[n];
                grid[0][5][k] = Complex32::new(s[n], 0.0);
            }
            for l in [5, 6] {
                for u in used[l].iter_mut().skip(6 * n_rb - 36).take(72) {
                    *u = true;
                }
            }
        }
        // PBCH.
        if sf == 0 {
            let mib = Mib {
                bw_index: N_RB.iter().position(|&x| x == n_rb).unwrap_or(0) as u8,
                phich_extended: false,
                ng_index: 1,
                sfn_msb: (self.sfn >> 2) as u8,
                spare: 0,
            };
            let mut b = mib.to_bits().to_vec();
            let mask = match ports {
                1 => 0,
                2 => 0xFFFF,
                _ => 0x5555,
            };
            append_crc16(&mut b, mask as u16);
            let coded = tbcc_encode(&b, 1920);
            let scr = scrambling(pci as u32, 1920);
            let seg = (self.sfn % 4) as usize;
            let bits: Vec<u8> = (0..480)
                .map(|i| coded[seg * 480 + i] ^ scr[seg * 480 + i])
                .collect();
            let sym: Vec<Complex32> = bits.chunks(2).map(|q| qpsk(q[0], q[1])).collect();
            let y = txd_encode(&sym, ports);
            for (i, &(k, l)) in pbch_res(pci, n_rb, false).iter().enumerate() {
                for p in 0..ports {
                    grid[p][l][k] = y[p][i];
                }
            }
            for l in 7..11 {
                for u in used[l].iter_mut().skip(6 * n_rb - 36).take(72) {
                    *u = true;
                }
            }
        }
        // Control region.
        let cfi = 2u8;
        let ctl = control(pci, n_rb, ports, cfi, 1, false);
        for l in 0..ctl.symbols {
            for u in used[l].iter_mut() {
                *u = true;
            }
        }
        {
            let w = cfi_codeword(cfi);
            let scr = scrambling(
                ((sf as u32 + 1) * (2 * pci as u32 + 1)) << 9 | pci as u32,
                32,
            );
            let bits: Vec<u8> = w.iter().zip(&scr).map(|(a, b)| a ^ b).collect();
            let sym: Vec<Complex32> = bits.chunks(2).map(|q| qpsk(q[0], q[1])).collect();
            let y = txd_encode(&sym, ports);
            for (i, reg) in ctl.pcfich.iter().enumerate() {
                for (j, &k) in reg.ks.iter().enumerate() {
                    for p in 0..ports {
                        grid[p][reg.l][k] = y[p][4 * i + j];
                    }
                }
            }
        }
        let assigns = self.assignments(sf);
        let n_reg = ctl.pdcch.len();
        let n_cce = n_reg / 9;
        let mut pdcch_bits: Vec<Option<u8>> = vec![None; 8 * n_reg];
        let mut cce = 0usize;
        let mut kept = Vec::new();
        for a in assigns {
            if cce + 4 > n_cce {
                continue;
            }
            let mut payload = dci_pack(&a.dci, n_rb);
            append_crc16(&mut payload, a.rnti as u16);
            let coded = tbcc_encode(&payload, 288);
            for (i, b) in coded.iter().enumerate() {
                pdcch_bits[72 * cce + i] = Some(*b);
            }
            cce += 4;
            kept.push(a);
        }
        let scr = scrambling((sf as u32) << 9 | pci as u32, 8 * n_reg);
        let sym: Vec<Complex32> = (0..4 * n_reg)
            .map(|i| match (pdcch_bits[2 * i], pdcch_bits[2 * i + 1]) {
                (Some(a), Some(b)) => qpsk(a ^ scr[2 * i], b ^ scr[2 * i + 1]),
                _ => Complex32::new(0.0, 0.0),
            })
            .collect();
        let y = txd_encode(&sym, ports);
        let perm = pdcch_permutation(n_reg, pci);
        for (i, reg) in ctl.pdcch.iter().enumerate() {
            let q = perm[i];
            for (j, &k) in reg.ks.iter().enumerate() {
                for p in 0..ports {
                    grid[p][reg.l][k] = y[p][4 * q + j];
                }
            }
        }
        // PDSCH.
        let mut busy_rb = vec![false; n_rb];
        for a in &kept {
            let prbs = assignment_prbs(&a.dci, n_rb);
            for s in &prbs {
                for &r in s {
                    busy_rb[r] = true;
                }
            }
            let res = pdsch_res(pci, n_rb, ports, false, ctl.symbols, sf, &prbs);
            let g = 2 * res.len();
            let layers = if ports > 1 { 2 } else { 1 };
            let e = (2 * layers) * (g / (2 * layers));
            let mut tb = a.tb.clone();
            append_crc24a(&mut tb);
            let k = size_at_least(tb.len()).unwrap_or(6144);
            let f = k - tb.len();
            let mut block = vec![0u8; f];
            block.extend_from_slice(&tb);
            let coded = turbo_encode(&block, f, a.rv as u8, e);
            let c_init = a.rnti * (1 << 14) + ((sf as u32) << 9) + pci as u32;
            let scr = scrambling(c_init, e);
            let bits: Vec<u8> = coded.iter().zip(&scr).map(|(x, s)| x ^ s).collect();
            let sym: Vec<Complex32> = bits.chunks(2).map(|q| qpsk(q[0], q[1])).collect();
            let y = txd_encode(&sym, ports);
            for (i, &(k, l)) in res.iter().enumerate() {
                if i < y[0].len() {
                    for p in 0..ports {
                        grid[p][l][k] = y[p][i];
                    }
                }
                used[l][k] = true;
            }
        }
        // Load on the free RBs of the data region.
        if self.opt.load {
            let mut free: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
            for (r, b) in busy_rb.iter().enumerate() {
                if !b {
                    free[0].push(r);
                    free[1].push(r);
                }
            }
            let res = pdsch_res(pci, n_rb, ports, false, ctl.symbols, sf, &free);
            let sym: Vec<Complex32> = (0..res.len())
                .map(|_| qpsk(self.data.bit(), self.data.bit()))
                .collect();
            let y = txd_encode(&sym, ports);
            for (i, &(k, l)) in res.iter().enumerate() {
                if used[l][k] {
                    continue;
                }
                for p in 0..ports {
                    grid[p][l][k] = y[p][i];
                }
            }
        }
        // OFDM per port, multipath, sum.
        let n = self.n;
        let sf_len = 15 * n;
        for p in 0..ports {
            let buf = &mut self.port_bufs[p];
            buf.clear();
            for l in 0..nsym {
                let mut f = vec![Complex32::new(0.0, 0.0); n];
                for k in 0..nsc {
                    f[bin(n_rb, n, k)] = grid[p][l][k];
                }
                self.ifft.process(&mut f);
                let (_, cp) = symbol_start(n, false, l);
                buf.extend_from_slice(&f[n - cp..]);
                buf.extend_from_slice(&f);
            }
        }
        let gain = 10f32.powf(c.gain_db as f32 / 20.0);
        let mut sum = vec![Complex32::new(0.0, 0.0); sf_len];
        for p in 0..ports {
            let buf = &self.port_bufs[p];
            let tail = &mut self.tail[p];
            for (i, s) in sum.iter_mut().enumerate() {
                *s += buf[i];
            }
            // Echoes: a tap at a delay (30.72 Msps samples) and a gain, carried into the next subframe.
            tail.resize(sf_len + 4096, Complex32::new(0.0, 0.0));
            for &(d, g) in &c.multipath {
                let dn = (d as f64 * self.native_rate / 30.72e6).round() as usize;
                let a = 10f32.powf(g as f32 / 20.0);
                for i in 0..sf_len {
                    tail[i + dn] += buf[i] * a;
                }
            }
            for (i, s) in sum.iter_mut().enumerate() {
                *s += tail[i];
            }
            tail.drain(..sf_len);
        }
        // Unit mean power: the sum of all ports, unit-power REs over the carrier.
        let scale = gain / (nsc as f32).sqrt();
        for s in sum.iter_mut() {
            *s *= scale;
        }
        self.native.extend_from_slice(&sum);
    }
}

impl Generator for LteGenerator {
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
                for sf in 0..10 {
                    self.subframe(sf);
                }
                self.sfn = (self.sfn + 1) % 1024;
                let native = std::mem::take(&mut self.native);
                self.rs.process(&native, &mut self.out);
                self.native = native;
                if self.skip > 0 {
                    let k = self.skip.min(self.out.len());
                    self.out.drain(..k);
                    self.skip -= k;
                    continue;
                }
            }
            let n = (self.out.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.out[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        i
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The occupied band: the resource blocks of the carrier (36.211 6.2). The SNR of the tests is the SNR in this band.
        Some(SignalInfo::new(self.cell.n_rb as f64 * 180e3))
    }
}

/// The band a scene's EARFCN is in (for tests).
pub fn band_of(c: &LteCell) -> u16 {
    earfcn_band(c.earfcn)
}
