//! The NR test signal: SS/PBCH bursts at 256 x SCS, each cell at its offset, resampled to the output rate; and for a cell
//! with a SIB1 the grid of CORESET#0 with the Type0 PDCCH and the PDSCH that carries SIB1, as a second part of the sum.
//! Every symbol has the phase of up-conversion to the cell's radio frequency (3GPP TS 38.211 clause 5.4).

use super::dlsch::*;
use super::msg::{Downlink, PlmnInfo, Plmn, ScsCarrier, SiMessage, Sib1, SsbInfo};
use super::phy::*;
use sdrkit_algo_fec::nr_sch::NrSch;
use sdrkit_util_bits::pack::unpack;
use sdrkit_util_bits::BitOrder;
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
    /// The nominal radio frequency of SS_REF, Hz.
    pub ssb_hz: f64,
    /// SIB1 and how it is sent. None sends no CORESET#0.
    pub sib1: Option<Sib1Gen>,
    /// The time pattern of the blocks.
    pub pattern: SsbPattern,
}

/// The SIB1 of a cell of the test signal and its transmission.
#[derive(Clone, Debug)]
pub struct Sib1Gen {
    /// The message.
    pub msg: Sib1,
    /// The row of the default time allocation table A (0 to 15).
    pub tdra: u8,
    /// The first block and the number of blocks of the PDSCH within CORESET#0.
    pub blocks: (usize, usize),
    /// Interleaved mapping of virtual to physical blocks.
    pub interleaved: bool,
    /// The aggregation level of the PDCCH (4, 8 or 16).
    pub level: usize,
    /// The redundancy versions of successive transmissions.
    pub rv_cycle: Vec<u8>,
}

impl Sib1Gen {
    /// A SIB1 for a cell of network 001-01 in band `band` with a carrier of `carrier_rb` blocks, sent over all of a
    /// CORESET#0 of `n_rb` blocks at aggregation level 8 with the redundancy versions 0, 2, 3, 1.
    pub fn typical(pci: u16, band: u16, scs_khz: u16, carrier_rb: u16, n_rb: usize) -> Sib1Gen {
        Sib1Gen {
            msg: Sib1 {
                q_rx_lev_min: Some(-60),
                q_qual_min: None,
                networks: vec![PlmnInfo {
                    plmns: vec![Plmn::new(1, 1, 2)],
                    tac: Some(0x00_0A_01),
                    ranac: None,
                    cell_identity: (0x00_1234u64 << 12) | u64::from(pci),
                    reserved_operator: false,
                    gnb_id_length: Some(24),
                }],
                reserved_other_use: false,
                si_window_slots: Some(20),
                si_messages: vec![SiMessage {
                    broadcasting: true,
                    period_frames: 16,
                    sibs: vec![(2, Some(1), false), (3, Some(0), false)],
                }],
                downlink: Some(Downlink {
                    bands: vec![band],
                    offset_to_point_a: 12,
                    carriers: vec![ScsCarrier {
                        offset_to_carrier: 0,
                        scs_khz,
                        bandwidth_rb: carrier_rb,
                    }],
                    bwp_location_and_bandwidth: riv_encode(275, 0, usize::from(carrier_rb)) as u16,
                    bwp_scs_khz: scs_khz,
                }),
                ssb: Some(SsbInfo {
                    positions: 0xFF,
                    period_ms: 20,
                    tdd: false,
                    uplink: true,
                }),
            },
            tdra: 0,
            blocks: (0, n_rb),
            interleaved: false,
            level: 8,
            rv_cycle: vec![0, 2, 3, 1],
        }
    }
}

fn default_gains() -> Vec<f64> {
    (0..8).map(|i: i32| -1.5 * (i - 2).abs() as f64).collect()
}

/// Scene 0 to 3 with the radio centre each assumes.
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
                mib: Mib {
                    dmrs_pos3: true,
                    ..mib(100, 3, 0, 6, false)
                },
                period_ms: 20,
                second_half: false,
                ssb_hz: 795.85e6,
                sib1: Some(Sib1Gen::typical(222, 28, 15, 52, 24)),
                pattern: SsbPattern::AC,
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
                ssb_hz: super::TEST_SIGNAL_HZ,
                sib1: None,
                pattern: SsbPattern::AC,
            }],
        ),
        // n5 at GSCN 2195 with 30 kHz blocks: Case B, 4 blocks, CORESET#0 of 24 blocks of 30 kHz.
        3 => (
            878.45e6,
            vec![NrCellCfg {
                pci: 333,
                scs: 30,
                lmax: 4,
                gains_db: vec![0.0, -1.5, -3.0, -4.5],
                offset_hz: 0.0,
                mib: Mib {
                    scs_common_30: true,
                    ..mib(200, 2, 4, 4, false)
                },
                period_ms: 20,
                second_half: false,
                ssb_hz: 878.45e6,
                sib1: Some(Sib1Gen::typical(333, 5, 30, 51, 24)),
                pattern: SsbPattern::B,
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
                    ssb_hz: super::TEST_SIGNAL_HZ - 4.32e6,
                    // CORESET#0 of 48 blocks reaches 13.7 MHz below the centre: outside the test signal, not sent.
                    sib1: None,
                    pattern: SsbPattern::AC,
                },
                NrCellCfg {
                    pci: 1007,
                    scs: 30,
                    lmax: 8,
                    gains_db: vec![-12.0, -9.0, -6.0, -3.0, 0.0, -3.0, -6.0, -9.0],
                    offset_hz: 4.32e6,
                    mib: Mib {
                        scs_common_30: true,
                        dmrs_pos3: true,
                        ..mib(33, 6, 0, 18, false)
                    },
                    period_ms: 20,
                    second_half: false,
                    ssb_hz: super::TEST_SIGNAL_HZ + 4.32e6,
                    sib1: Some(Sib1Gen::typical(1007, 78, 30, 106, 24)),
                    pattern: SsbPattern::AC,
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
        /// An n5 cell with 30 kHz blocks of Case B.
        N5CaseB = ("3", "n5 cell, 30 kHz blocks"),
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
    carrier: Option<CarrierPart>,
}

/// The part of a cell's signal on the grid of CORESET#0.
struct CarrierPart {
    c0: Coreset0,
    sib1: Sib1Gen,
    ifft: Fft,
    native: Vec<Complex32>,
    rs: FractionalResampler<Complex32>,
    out: Vec<Complex32>,
    osc: Nco,
    /// The frequency of the 0 Hz bin of the grid, Hz.
    centre_hz: f64,
    /// Transmissions so far, per SS/PBCH block.
    sent: [usize; 8],
}

/// The symbols of one slot on the grid of CORESET#0 (14 rows of `n_sc`) with the PDCCH and the PDSCH of SIB1 for cell
/// `pci`, or none when the message does not fit a QPSK transport block of the allocation.
fn sib1_slot(c0: &Coreset0, g: &Sib1Gen, mib: &Mib, pci: u16, slot: usize, first: usize, rv: u8) -> Option<Vec<Vec<Complex32>>> {
    let n_sc = c0.n_sc();
    let mut dci = Dci {
        riv: riv_encode(c0.n_rb, g.blocks.0, g.blocks.1),
        tdra: g.tdra,
        interleaved: g.interleaved,
        mcs: 0,
        rv,
        si_message: false,
    };
    let p = Pdsch::from_dci(&dci, c0.n_rb, mib.dmrs_pos3)?;
    let mut bits = Vec::new();
    unpack(&g.msg.write(), BitOrder::Msb, &mut bits);
    // The lowest MCS whose transport block holds the message; the rest is padding.
    let (mcs, size, r) = (0..10u8).find_map(|m| {
        let (qm, r) = mcs_qpsk(m)?;
        let size = tbs(p.re_per_prb(), p.prbs.len(), qm, r)?;
        (size >= bits.len()).then_some((m, size, r))
    })?;
    dci.mcs = mcs;
    bits.resize(size, 0);
    let sq = std::f32::consts::FRAC_1_SQRT_2;
    let qpsk = |a: u8, b: u8| Complex32::new(sq * (1.0 - 2.0 * f32::from(a)), sq * (1.0 - 2.0 * f32::from(b)));
    let mut grid = vec![vec![Complex32::new(0.0, 0.0); n_sc]; 14];
    // PDCCH on the first candidate of the level.
    let level = g.level.min(c0.n_cce());
    let (_, cce) = c0.candidates().into_iter().find(|c| c.0 == level)?;
    let mut code = dci_code(Dci::len(c0.n_rb), level)?;
    let mut coded = vec![0u8; 108 * level];
    code.encode(&dci_with_crc(&dci.pack(c0.n_rb), SI_RNTI), &mut coded).ok()?;
    let scr = pdcch_scrambling(pci, coded.len());
    let rbs = c0.pdcch_rbs(pci, cce, level);
    let mut m = 0;
    for l in first..first + c0.n_symb {
        let r = pdcch_dmrs(pci, slot, l, c0.n_rb);
        for &rb in &rbs {
            for i in 0..12 {
                grid[l][12 * rb + i] = if i % 4 == 1 {
                    r[3 * rb + i / 4]
                } else {
                    m += 2;
                    qpsk(coded[m - 2] ^ scr[m - 2], coded[m - 1] ^ scr[m - 1])
                };
            }
        }
    }
    // PDSCH.
    let res = p.data_res();
    let sch = NrSch::new(size, f64::from(r) / 1024.0).ok()?;
    let f = sch.encode(&bits, rv, 2, 2 * res.len()).ok()?;
    let scr = pdsch_scrambling(pci, f.len());
    for (i, &(k, l)) in res.iter().enumerate() {
        grid[l][k] = qpsk(f[2 * i] ^ scr[2 * i], f[2 * i + 1] ^ scr[2 * i + 1]);
    }
    for &l in &p.dmrs_symbols {
        let r = pdsch_dmrs(pci, slot, l, c0.n_rb);
        for &rb in &p.prbs {
            for i in 0..6 {
                grid[l][12 * rb + 2 * i] = r[6 * rb + i] * p.dmrs_gain();
            }
        }
    }
    Some(grid)
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
                let l0 = self.cfg.pattern.first_symbol(i);
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
        let num = Numerology { scs_khz: scs, fft: 256 };
        // The slots of SIB1 in this half frame, as times from its start; the load of the block's band is off in them.
        let busy = self.carrier_half_frame(in_burst, scale);
        for (l, row) in syms.iter_mut().enumerate() {
            let t = num.symbol(l).0 as f64 / num.rate();
            if !ssb_syms[l] && busy.iter().any(|&(a, b)| t + 1e-9 >= a - 80e-6 && t < b) {
                row.fill(Complex32::new(0.0, 0.0));
            }
            let turn = num.carrier_phase(self.cfg.ssb_hz, l);
            let mut buf: Vec<Complex32> = row.iter().map(|v| v * turn).collect();
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

impl CellState {
    /// Makes the half frame of the CORESET#0 part (when the cell has one) and returns the time ranges of the slots it
    /// fills, seconds from the start of the half frame. A block whose occasion lies in a slot with SS/PBCH blocks is not
    /// served: the test signal keeps the two apart instead of rate matching around the blocks.
    fn carrier_half_frame(&mut self, in_burst: bool, scale: f32) -> Vec<(f64, f64)> {
        let mut busy = Vec::new();
        let Some(part) = self.carrier.as_mut() else {
            return busy;
        };
        let (c0, cfg) = (part.c0, &self.cfg);
        let num = c0.numerology();
        let n_sc = c0.n_sc();
        let slots_half = num.slots_per_frame() / 2;
        let hf = self.half % 2;
        let slot_s = |s: usize| num.slot_start(s) as f64 / num.rate();
        // The times of the SS/PBCH blocks of this half frame.
        let ssb_num = Numerology { scs_khz: cfg.scs, fft: 256 };
        let blocks: Vec<(f64, f64)> = (0..cfg.lmax as usize)
            .filter(|_| in_burst)
            .map(|i| {
                let l = cfg.pattern.first_symbol(i);
                (ssb_num.symbol(l).0 as f64 / ssb_num.rate(), ssb_num.symbol(l + 4).0 as f64 / ssb_num.rate())
            })
            .collect();
        let mut grid = vec![vec![Complex32::new(0.0, 0.0); n_sc]; 14 * slots_half];
        for i in 0..cfg.lmax as usize {
            let gain = 10f64.powf(cfg.gains_db.get(i).copied().unwrap_or(-100.0) / 20.0) as f32;
            let (n0, parity, first) = type0_occasion(cfg.mib.search_space0, num.mu(), i, c0.n_symb);
            if gain < 1e-4 || self.sfn as usize % 2 != parity || n0 / slots_half != hf {
                continue;
            }
            let s = n0 % slots_half;
            let (a, b) = (slot_s(s), slot_s(s + 1));
            if blocks.iter().any(|&(x, y)| x < b && y > a) {
                continue;
            }
            let rv = part.sib1.rv_cycle.get(part.sent[i] % part.sib1.rv_cycle.len().max(1)).copied().unwrap_or(0);
            let Some(slot) = sib1_slot(&c0, &part.sib1, &cfg.mib, cfg.pci, n0, first, rv) else {
                continue;
            };
            part.sent[i] += 1;
            for (l, row) in slot.into_iter().enumerate() {
                for (g, v) in grid[14 * s + l].iter_mut().zip(row) {
                    *g += v * gain;
                }
            }
            busy.push((a, b));
        }
        for (l, row) in grid.iter().enumerate() {
            let mut buf = vec![Complex32::new(0.0, 0.0); num.fft];
            let abs = 14 * slots_half * hf + l;
            let turn = num.carrier_phase(part.centre_hz, abs) * scale;
            for (k, v) in row.iter().enumerate() {
                let bin = (k as i64 - (n_sc / 2) as i64).rem_euclid(num.fft as i64) as usize;
                buf[bin] = v * turn;
            }
            part.ifft.process(&mut buf);
            let (_, cp) = num.symbol(abs);
            part.native.extend_from_slice(&buf[num.fft - cp..]);
            part.native.extend_from_slice(&buf);
        }
        busy
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
                let mut rs = FractionalResampler::with_rates(native, rate).expect("rates");
                // Both parts of a cell start at the same time: the filter delays are taken out.
                let _ = rs.set_start(0.0);
                let carrier = cfg.sib1.clone().and_then(|sib1| {
                    let c0 = Coreset0::from_mib(cfg.scs, &cfg.mib)?;
                    let num = c0.numerology();
                    let rel = c0.centre_from_ssb(cfg.scs, cfg.mib.k_ssb);
                    let mut rs = FractionalResampler::with_rates(num.rate(), rate).ok()?;
                    rs.set_start(0.0).ok()?;
                    Some(CarrierPart {
                        c0,
                        sib1,
                        ifft: Fft::inverse(num.fft),
                        native: Vec::new(),
                        rs,
                        out: Vec::new(),
                        osc: Nco::new(rate, cfg.offset_hz + rel),
                        centre_hz: cfg.ssb_hz + rel,
                        sent: [0; 8],
                    })
                });
                CellState {
                    carrier,
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
                if let Some(part) = c.carrier.as_mut() {
                    let mut o = Vec::new();
                    part.rs.process(&part.native, &mut o);
                    part.native.clear();
                    part.osc.mix_in_place(&mut o);
                    part.out.extend(o);
                }
            }
        }
        let n = self
            .cells
            .iter()
            .map(|c| c.out.len().min(c.carrier.as_ref().map_or(usize::MAX, |p| p.out.len())))
            .min()
            .unwrap_or(0);
        let mut sum = vec![Complex32::new(0.0, 0.0); n];
        for c in self.cells.iter_mut() {
            for (s, v) in sum.iter_mut().zip(c.out.drain(..n)) {
                *s += v;
            }
            if let Some(part) = c.carrier.as_mut() {
                for (s, v) in sum.iter_mut().zip(part.out.drain(..n)) {
                    *s += v;
                }
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
