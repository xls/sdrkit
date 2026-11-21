//! Cancellation of a decoded cell from a subframe grid: the known signals of the cell are made again and
//! taken away from the grid, so that a weaker cell at the same timing can be read. The known signals are the CRS of every port,
//! PSS, SSS, PBCH, PCFICH, the decoded DCIs and the decoded PDSCH. The channel comes from the CRS.

use super::msg::Mib;
use super::phy::*;
use super::search::{estimate, Grid};
use sdrkit_algo_fec::turbo::size_at_least;
use sdrkit_core::Complex32;

/// What was decoded in one subframe of a cell.
#[derive(Clone, Debug, PartialEq)]
pub struct SubframeInfo {
    /// SFN.
    pub sfn: u32,
    /// Subframe.
    pub sf: usize,
    /// CFI.
    pub cfi: u8,
    /// DCIs: first CCE, the DCI, and its decoded transport block (bits) when the PDSCH decoded.
    pub dcis: Vec<(usize, Dci, Option<Vec<u8>>)>,
}

/// A decoded cell, enough to rebuild what it sends.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// PCI.
    pub pci: u16,
    /// RBs.
    pub n_rb: usize,
    /// Ports.
    pub ports: usize,
    /// MIB.
    pub mib: Mib,
    /// SFN of the first whole frame.
    pub sfn0: u32,
    /// Input sample of the first whole frame.
    pub start_in: f64,
    /// Its carrier, Hz from 0 in the capture.
    pub centre_hz: f64,
    /// What was decoded per subframe.
    pub subframes: Vec<SubframeInfo>,
}

fn put(tx: &mut [Grid], ports: usize, res: &[(usize, usize)], sym: &[Complex32]) {
    let y = txd_encode(sym, ports);
    for (i, &(k, l)) in res.iter().enumerate() {
        for (p, yp) in y.iter().enumerate().take(ports) {
            if let Some(v) = yp.get(i) {
                tx[p][l][k] = *v;
            }
        }
    }
}

/// What a decoded cell sends in subframe `sf` of frame `sfn`, per port.
pub fn rebuild(d: &Decoded, sfn: u32, sf: usize) -> Vec<Grid> {
    let (n_rb, ports, pci) = (d.n_rb, d.ports, d.pci);
    let nsc = 12 * n_rb;
    let mut tx: Vec<Grid> = vec![vec![vec![Complex32::new(0.0, 0.0); nsc]; 14]; ports];
    for p in 0..ports {
        for l in 0..14 {
            for (k, v) in crs_res(pci, n_rb, p, 2 * sf + l / 7, l % 7, false) {
                tx[p][l][k] = v;
            }
        }
    }
    if sf == 0 || sf == 5 {
        let ps = pss(pci % 3);
        let ss = sss(pci / 3, pci % 3, sf == 5);
        for n in 0..62 {
            let k = 6 * n_rb - 31 + n;
            tx[0][6][k] = ps[n];
            tx[0][5][k] = Complex32::new(ss[n], 0.0);
        }
    }
    if sf == 0 {
        let m = Mib {
            sfn_msb: (sfn >> 2) as u8,
            ..d.mib
        };
        let mut b = m.to_bits().to_vec();
        let mask = match ports {
            1 => 0,
            2 => 0xFFFF,
            _ => 0x5555,
        };
        append_crc16(&mut b, mask as u16);
        let coded = tbcc_encode(&b, 1920);
        let scr = scrambling(pci as u32, 1920);
        let seg = (sfn % 4) as usize;
        let sym: Vec<Complex32> = (0..240)
            .map(|i| {
                qpsk(
                    coded[seg * 480 + 2 * i] ^ scr[seg * 480 + 2 * i],
                    coded[seg * 480 + 2 * i + 1] ^ scr[seg * 480 + 2 * i + 1],
                )
            })
            .collect();
        put(&mut tx, ports, &pbch_res(pci, n_rb, false), &sym);
    }
    if let Some(info) = d.subframes.iter().find(|s| s.sfn == sfn && s.sf == sf) {
        let ctl = control(pci, n_rb, ports, info.cfi, d.mib.ng_index, false);
        // PCFICH.
        let w = cfi_codeword(info.cfi);
        let scr = scrambling(
            ((sf as u32 + 1) * (2 * pci as u32 + 1)) << 9 | pci as u32,
            32,
        );
        let sym: Vec<Complex32> = (0..16)
            .map(|i| qpsk(w[2 * i] ^ scr[2 * i], w[2 * i + 1] ^ scr[2 * i + 1]))
            .collect();
        let res: Vec<(usize, usize)> = ctl
            .pcfich
            .iter()
            .flat_map(|r| r.ks.iter().map(move |&k| (k, r.l)))
            .collect();
        put(&mut tx, ports, &res, &sym);
        // PDCCH: the decoded DCIs on their CCEs.
        let n_reg = ctl.pdcch.len();
        let mut bits: Vec<Option<u8>> = vec![None; 8 * n_reg];
        for (cce, dci, _) in &info.dcis {
            let mut payload = dci_pack(dci, n_rb);
            append_crc16(&mut payload, 0xFFFF);
            let coded = tbcc_encode(&payload, 288);
            for (i, b) in coded.iter().enumerate() {
                if let Some(slot) = bits.get_mut(72 * cce + i) {
                    *slot = Some(*b);
                }
            }
        }
        let scr = scrambling((sf as u32) << 9 | pci as u32, 8 * n_reg);
        let sym: Vec<Complex32> = (0..4 * n_reg)
            .map(|i| match (bits[2 * i], bits[2 * i + 1]) {
                (Some(a), Some(b)) => qpsk(a ^ scr[2 * i], b ^ scr[2 * i + 1]),
                _ => Complex32::new(0.0, 0.0),
            })
            .collect();
        let y = txd_encode(&sym, ports);
        let perm = pdcch_permutation(n_reg, pci);
        for (i, reg) in ctl.pdcch.iter().enumerate() {
            for (j, &k) in reg.ks.iter().enumerate() {
                for p in 0..ports {
                    tx[p][reg.l][k] = y[p][4 * perm[i] + j];
                }
            }
        }
        // PDSCH of the decoded transport blocks.
        for (_, dci, tb) in &info.dcis {
            let Some(tb) = tb else { continue };
            let prbs = assignment_prbs(dci, n_rb);
            let res = pdsch_res(pci, n_rb, ports, false, ctl.symbols, sf, &prbs);
            let g = 2 * res.len();
            let layers = if ports > 1 { 2 } else { 1 };
            let e = (2 * layers) * (g / (2 * layers));
            let mut b = tb.clone();
            append_crc24a(&mut b);
            let Some(k) = size_at_least(b.len()) else {
                continue;
            };
            let f = k - b.len();
            let mut block = vec![0u8; f];
            block.extend_from_slice(&b);
            let rv = if dci.format == DciFormat::F1a {
                dci.rv as usize
            } else {
                0
            };
            let coded = turbo_encode(&block, f, rv as u8, e);
            let scr = scrambling(0xFFFF * (1 << 14) + ((sf as u32) << 9) + pci as u32, e);
            let sym: Vec<Complex32> = (0..e / 2)
                .map(|i| qpsk(coded[2 * i] ^ scr[2 * i], coded[2 * i + 1] ^ scr[2 * i + 1]))
                .collect();
            put(&mut tx, ports, &res, &sym);
        }
    }
    tx
}

/// Takes a decoded cell's signal out of subframe grid `g` in one round.
pub fn cancel(g: &mut Grid, d: &Decoded, sfn: u32, sf: usize) {
    let tx = rebuild(d, sfn, sf);
    let ch = estimate(g, d.pci, d.n_rb, sf, d.ports, false, 0);
    for (p, txp) in tx.iter().enumerate() {
        for (l, row) in txp.iter().enumerate() {
            for (k, v) in row.iter().enumerate() {
                if v.norm_sqr() > 0.0 {
                    g[l][k] -= ch.h[p][l][k] * v;
                }
            }
        }
    }
}
