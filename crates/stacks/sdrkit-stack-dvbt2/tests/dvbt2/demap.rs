//! The de-mapper of rotated constellations against its former form. The former form measured the distances to the rotated
//! points; the present form turns the cell back and measures on the plain grid. Both get the same noisy cells here.

use super::common::coding;
use sdrkit_algo_fec::bch::{codes, Bch};
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::llr::{Llr8, Quantizer};
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::Complex32;
use sdrkit_stack_dvbt2::bicm::{point_set, BlockResult, Coding, Decoder, Encoder};
use sdrkit_stack_dvbt2::interleave::bits_to_cells;
use sdrkit_stack_dvbt2::params::*;
use sdrkit_util_bits::interleave::Permutation;

const MAX_ITER: u32 = 30;

/// The decoder of one PLP with the former de-mapper: a constellation of rotated points, the cell not turned.
struct Former {
    coding: Coding,
    bch: Bch,
    ldpc: LdpcDecoder,
    perm: Permutation,
    cons: Constellation,
    quant: Quantizer,
    z: Vec<Complex32>,
    w: Vec<f32>,
    llr_cells: Vec<f32>,
    llr: Vec<f32>,
    llr8: Vec<Llr8>,
    hard: Vec<u8>,
}

impl Former {
    fn new(c: Coding) -> Former {
        let key = outer_code_key(c.frame, c.rate).expect("code");
        let bch = Bch::from_code(codes::by_key(key).expect("bch")).expect("bch");
        let ldpc = LdpcCode::from_dvb(ldpc_table(c.frame, c.rate).expect("table")).expect("ldpc");
        let mut set = point_set(c.modulation);
        if c.rotated {
            set = set.rotated(c.modulation.rotation_degrees().to_radians());
        }
        let (n, nc) = (c.frame.n_ldpc(), c.cells());
        Former {
            coding: c,
            ldpc: LdpcDecoder::new(&ldpc, MinSum::default()),
            bch,
            perm: bits_to_cells(c.frame, c.modulation, c.rate).expect("perm"),
            cons: Constellation::new(set).expect("constellation"),
            quant: Quantizer::default(),
            z: vec![Complex32::default(); nc],
            w: vec![0.0; nc],
            llr_cells: vec![0.0; n],
            llr: vec![0.0; n],
            llr8: vec![0; n],
            hard: vec![0; n],
        }
    }

    /// The soft values of one block in code word order, before the quantiser.
    fn soft(&mut self, g: &[Complex32], weights: &[f32]) -> &[f32] {
        let nc = self.coding.cells();
        if self.coding.rotated {
            for q in 0..nc {
                let nq = (q + 1) % nc;
                self.z[q] = Complex32::new(g[q].re, g[nq].im);
                self.w[q] = weights[q].min(weights[nq]);
            }
        } else {
            self.z.copy_from_slice(g);
            self.w.copy_from_slice(weights);
        }
        self.cons
            .demap_block_weighted(&self.z, &self.w, Demap::MaxLog, &mut self.llr_cells)
            .expect("demap");
        self.perm
            .invert(&self.llr_cells, &mut self.llr)
            .expect("perm");
        &self.llr
    }

    fn decode(&mut self, g: &[Complex32], weights: &[f32], bbframe: &mut [u8]) -> BlockResult {
        self.soft(g, weights);
        let scale = self.quant.scale_for(&self.llr);
        self.quant
            .quantize(&self.llr, scale, &mut self.llr8)
            .expect("quantise");
        let mut res = BlockResult::default();
        match self.ldpc.decode(&self.llr8, MAX_ITER, &mut self.hard) {
            Ok(d) => {
                res.ldpc_ok = true;
                res.iterations = d.iterations;
            }
            Err(_) => res.iterations = MAX_ITER,
        }
        let nb = self.bch.n();
        if let Ok(d) = self.bch.decode(&mut self.hard[..nb]) {
            res.bch_ok = true;
            res.bch_corrected = d.corrected;
        }
        bbframe.copy_from_slice(&self.hard[..self.bch.k()]);
        res
    }
}

/// The present de-mapper as [`Decoder::decode`] runs it, up to the soft values in code word order.
fn present_soft(c: Coding, g: &[Complex32], weights: &[f32]) -> Vec<f32> {
    let nc = c.cells();
    let cons = Constellation::new(point_set(c.modulation)).expect("constellation");
    let a = -c.modulation.rotation_degrees().to_radians();
    let turn = Complex32::new(a.cos() as f32, a.sin() as f32);
    let (mut z, mut w) = (g.to_vec(), weights.to_vec());
    if c.rotated {
        for q in 0..nc {
            let nq = (q + 1) % nc;
            z[q] = Complex32::new(g[q].re, g[nq].im) * turn;
            w[q] = weights[q].min(weights[nq]);
        }
    }
    let mut cells = vec![0f32; c.frame.n_ldpc()];
    cons.demap_block_weighted(&z, &w, Demap::MaxLog, &mut cells)
        .expect("demap");
    let mut llr = vec![0f32; cells.len()];
    bits_to_cells(c.frame, c.modulation, c.rate)
        .expect("perm")
        .invert(&cells, &mut llr)
        .expect("perm");
    llr
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

/// One block: random BBFRAME bits, its cells, noise of the SNR, and the weights. `fade` gives every cell its own gain
/// (some of them 0); the noise of a cell then grows as the equaliser divides by the gain.
fn block(
    enc: &mut Encoder,
    snr_db: f32,
    fade: bool,
    seed: u64,
) -> (Vec<u8>, Vec<Complex32>, Vec<f32>) {
    let c = *enc.coding();
    let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let bits: Vec<u8> = (0..c.k_bch()).map(|_| (rng.next() & 1) as u8).collect();
    let mut g = vec![Complex32::default(); c.cells()];
    enc.encode(&bits, &mut g).expect("encode");
    let sigma2 = 10f32.powf(-snr_db / 10.0);
    let mut noise = vec![Complex32::default(); g.len()];
    Awgn::with_power(sigma2 as f64, seed)
        .expect("noise")
        .process(&mut noise);
    let mut weights = vec![0f32; g.len()];
    for ((gg, n), w) in g.iter_mut().zip(&noise).zip(weights.iter_mut()) {
        let h2 = if !fade {
            1.0
        } else if rng.next().is_multiple_of(16) {
            0.0
        } else {
            0.05 + (rng.next() % 1000) as f32 / 400.0
        };
        if h2 > 0.0 {
            *gg += *n / h2.sqrt();
        } else {
            *gg = *n;
        }
        *w = h2 / sigma2;
    }
    (bits, g, weights)
}

const MODS: [(Modulation, f32); 4] = [
    (Modulation::Qpsk, 3.0),
    (Modulation::Qam16, 8.8),
    (Modulation::Qam64, 13.4),
    (Modulation::Qam256, 18.6),
];

/// Without rotation both forms are one code path and the soft values are the same bits. With rotation the soft values of
/// the two forms differ by rounding only, for equal and for unequal weights, and the quantised values that the LDPC
/// decoder gets are the same.
#[test]
fn soft_values_of_the_turned_cell_are_those_of_the_rotated_points() {
    for (m, base) in MODS {
        for rotated in [false, true] {
            for fade in [false, true] {
                for (k, snr) in [base - 2.0, base, base + 3.0, base + 12.0]
                    .into_iter()
                    .enumerate()
                {
                    let c = coding(FecFrame::Short, CodeRate::R2_3, m, rotated);
                    let mut enc = Encoder::new(c).expect("encoder");
                    let (_, g, w) = block(&mut enc, snr, fade, 100 + k as u64);
                    let mut former = Former::new(c);
                    let a = former.soft(&g, &w).to_vec();
                    let b = present_soft(c, &g, &w);
                    if !rotated {
                        assert!(a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()));
                        continue;
                    }
                    let peak = a.iter().fold(0f32, |p, v| p.max(v.abs()));
                    let worst = a
                        .iter()
                        .zip(&b)
                        .fold(0f32, |p, (x, y)| p.max((x - y).abs()));
                    let q = Quantizer::default();
                    let (mut qa, mut qb) = (vec![0 as Llr8; a.len()], vec![0 as Llr8; b.len()]);
                    q.quantize(&a, q.scale_for(&a), &mut qa).expect("quantise");
                    q.quantize(&b, q.scale_for(&b), &mut qb).expect("quantise");
                    let differ = qa.iter().zip(&qb).filter(|(x, y)| x != y).count();
                    let step = qa
                        .iter()
                        .zip(&qb)
                        .map(|(x, y)| (*x as i32 - *y as i32).abs())
                        .max()
                        .unwrap_or(0);
                    eprintln!(
                        "{m:?} fade {fade} snr {snr:.1}: worst {worst:.3e} of peak {peak:.3e}, \
                         {differ} of {} quantised values differ, by at most {step}",
                        a.len()
                    );
                    assert!(
                        worst <= 2e-4 * peak.max(1.0),
                        "{m:?} {snr}: {worst} of {peak}"
                    );
                    assert_eq!(
                        (differ, step),
                        (0, 0),
                        "{m:?} {snr}: the input of the LDPC decoder differs"
                    );
                }
            }
        }
    }
}

/// Around the threshold SNR of each constellation the present decoder loses no more blocks than the former one, on the
/// plain channel and on a channel with unequal weights.
#[test]
fn block_errors_at_the_threshold_are_not_above_the_former_ones() {
    const BLOCKS: u64 = 24;
    for (m, base) in MODS {
        for fade in [false, true] {
            let c = coding(FecFrame::Short, CodeRate::R2_3, m, true);
            let mut enc = Encoder::new(c).expect("encoder");
            let mut former = Former::new(c);
            let mut present = Decoder::new(c, MAX_ITER).expect("decoder");
            let (mut lost_f, mut lost_p, mut iter_f, mut iter_p) = (0u32, 0u32, 0u32, 0u32);
            let mut line = String::new();
            // The channel with unequal weights needs more SNR for the same code.
            let (first, width) = if fade {
                (base + 1.0, 0.7)
            } else {
                (base - 0.9, 0.3)
            };
            for step in 0..7 {
                let snr = first + width * step as f32;
                let (mut f, mut p) = (0u32, 0u32);
                for b in 0..BLOCKS {
                    let (bits, g, w) = block(&mut enc, snr, fade, 1000 * (step + 1) + b);
                    let (mut of, mut op) = (vec![0u8; bits.len()], vec![0u8; bits.len()]);
                    let rf = former.decode(&g, &w, &mut of);
                    let rp = present.decode(&g, &w, &mut op).expect("decode");
                    f += (of != bits) as u32;
                    p += (op != bits) as u32;
                    iter_f += rf.iterations;
                    iter_p += rp.iterations;
                }
                line.push_str(&format!(" {snr:.1}:{f}/{p}"));
                lost_f += f;
                lost_p += p;
            }
            eprintln!(
                "{m:?} fade {fade} lost former/present of {BLOCKS} at{line}; iterations {iter_f}/{iter_p}"
            );
            assert!(
                lost_f > 0 && lost_f < 7 * BLOCKS as u32,
                "{m:?}: the sweep misses the threshold"
            );
            assert!(
                lost_p <= lost_f,
                "{m:?} fade {fade}: {lost_p} lost, formerly {lost_f}"
            );
        }
    }
}
