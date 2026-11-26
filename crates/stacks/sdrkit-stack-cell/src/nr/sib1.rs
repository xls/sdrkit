//! Reception of the NR SIB1 of a cell found in a capture: the grid of CORESET#0 in the Type0 PDCCH occasions, the blind
//! decode of DCI format 1_0 with the SI-RNTI, and the PDSCH it schedules. Only the SI-RNTI is tried.

use super::dlsch::*;
use super::phy::{Mib, SsbPattern};
use crate::decoder::Capture;
use crate::dsp::{resample_to, ResamplerCache};
use sdrkit_algo_dsp::fft::FftPlans;
use sdrkit_algo_fec::nr_sch::{NrSch, NrSchDecoder};
use sdrkit_algo_fec::polar::Polar;
use sdrkit_core::Complex32;
use sdrkit_util_bits::pack::pack;
use sdrkit_util_bits::{BitOrder, Pad};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

sdrkit_core::choice_enum! {
    /// How far the reception of SIB1 went.
    #[derive(Default, Serialize, Deserialize)]
    pub enum Sib1Step {
        /// No MIB, so nothing is known of SIB1.
        #[default]
        #[serde(rename = "no_mib")]
        NoMib = ("no_mib", "No MIB"),
        /// The cell has a MIB that schedules SIB1; no capture was searched for it yet.
        #[serde(rename = "not_tried")]
        NotTried = ("not_tried", "Not tried yet"),
        /// The MIB schedules no SIB1 on this SS/PBCH block.
        #[serde(rename = "not_scheduled")]
        NotScheduled = ("not_scheduled", "Not scheduled"),
        /// The radio frequency is not known, so the phase of the symbols is not known.
        #[serde(rename = "no_frequency")]
        NoFrequency = ("no_frequency", "Radio frequency unknown"),
        /// CORESET#0 does not lie within the capture.
        #[serde(rename = "too_narrow")]
        TooNarrow = ("too_narrow", "CORESET#0 outside the capture"),
        /// No control channel with the SI-RNTI was found in the occasions of the capture.
        #[serde(rename = "no_control")]
        NoControl = ("no_control", "No PDCCH for SIB1 found"),
        /// A control channel was decoded; the data channel failed its check.
        #[serde(rename = "no_data")]
        NoData = ("no_data", "PDCCH decoded, PDSCH failed"),
        /// A block passed its check; it is not a SIB1 that this stack reads.
        #[serde(rename = "unreadable")]
        Unreadable = ("unreadable", "Block decoded, not a readable SIB1"),
        /// SIB1 was decoded.
        #[serde(rename = "decoded")]
        Decoded = ("decoded", "Decoded"),
    }
}

/// What the search knows of a cell whose SIB1 is wanted.
#[derive(Clone, Debug)]
pub struct Sib1Target<'a> {
    /// The cell identity.
    pub pci: u16,
    /// The spacing of the SS/PBCH block, kHz.
    pub ssb_scs: u32,
    /// The MIB of the frame of `anchor`.
    pub mib: &'a Mib,
    /// The index of the SS/PBCH block of `anchor`.
    pub ssb: u8,
    /// The input sample at which the useful part of the PSS of that block starts.
    pub anchor: f64,
    /// The centre of the block in the capture, Hz from 0 Hz of the samples.
    pub ssb_hz: f64,
    /// The nominal radio frequency of the centre of the block, Hz, when the radio centre is known.
    pub ssb_nominal_hz: Option<f64>,
    /// Whether the symbols carry the phase of up-conversion (3GPP TS 38.211 clause 5.4).
    pub symbol_phase: bool,
    /// The time pattern of the blocks.
    pub pattern: SsbPattern,
}

/// The result of one try.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sib1Result {
    /// How far the reception went.
    pub step: Sib1Step,
    /// The sample rate that would hold CORESET#0 with the radio centre where it is, Hz (0 when no CORESET#0 is known).
    pub rate_needed_hz: f64,
    /// Slots of Type0 PDCCH occasions that lay whole in the capture and were searched.
    pub occasions: u32,
    /// PDCCHs decoded with the SI-RNTI.
    pub pdcch: u32,
    /// The DCI of the newest of them that schedules SIB1.
    pub dci: Option<Dci>,
    /// The transport block that passed its CRC.
    pub payload: Option<Vec<u8>>,
}

/// Least-squares line through pilot values: the value at the mean position, the slope per subcarrier, the mean position and
/// the mean squared residual.
#[derive(Clone, Copy, Debug, Default)]
struct Fit {
    a: Complex32,
    b: Complex32,
    x0: f32,
    noise: f32,
}

impl Fit {
    fn of(points: &[(f32, Complex32)]) -> Fit {
        let n = points.len().max(1) as f32;
        let x0 = points.iter().map(|p| p.0).sum::<f32>() / n;
        let a = points.iter().map(|p| p.1).sum::<Complex32>() / n;
        let sxx: f32 = points.iter().map(|p| (p.0 - x0).powi(2)).sum();
        let b = if sxx > 0.0 {
            points.iter().map(|p| p.1 * (p.0 - x0)).sum::<Complex32>() / sxx
        } else {
            Complex32::new(0.0, 0.0)
        };
        let res: f32 = points.iter().map(|p| (p.1 - a - b * (p.0 - x0)).norm_sqr()).sum();
        Fit {
            a,
            b,
            x0,
            noise: res / (n - 2.0).max(1.0),
        }
    }
    fn at(&self, x: f32) -> Complex32 {
        self.a + self.b * (x - self.x0)
    }
}

/// The SIB1 receiver: the resamplers, the FFT plans and the decoders, kept between captures.
#[derive(Default)]
pub struct Sib1Receiver {
    cache: ResamplerCache,
    plans: FftPlans,
    polar: Vec<((usize, usize), Polar)>,
    sch: Option<NrSchDecoder>,
    time: Vec<Complex32>,
    grid: Vec<Complex32>,
}

impl Sib1Receiver {
    /// The grid of slot `slot` (14 symbols of `c0.n_sc()` subcarriers, in `self.grid`) whose first sample is input sample
    /// `start`; false when the slot is not whole in the capture.
    fn slot_grid(&mut self, cap: &Capture, c0: &Coreset0, f_cap: f64, f_nominal: Option<f64>, slot: usize, start: f64) -> bool {
        let num = c0.numerology();
        let first = num.slot_start(slot);
        let len = num.slot_start(slot + 1) - first;
        let step = cap.rate / num.rate();
        if start < 0.0 || start + (len as f64 + 8.0) * step >= cap.samples.len() as f64 {
            return false;
        }
        let n_sc = c0.n_sc();
        // Flat over the grid: the filter is flat to 0.6 of its cut-off.
        resample_to(
            &mut self.cache,
            &cap.samples,
            cap.rate,
            f_cap,
            num.rate(),
            c0.width_hz() / 0.6,
            start,
            0.0,
            len,
            &mut self.time,
        );
        let mut fft = self.plans.forward(num.fft);
        self.grid.clear();
        self.grid.resize(14 * n_sc, Complex32::new(0.0, 0.0));
        let mut buf = vec![Complex32::new(0.0, 0.0); num.fft];
        for l in 0..14 {
            let (s, cp) = num.symbol(14 * slot + l);
            // The window starts half a cyclic prefix early; the phase ramp of that is taken out per bin.
            let adv = cp / 2;
            let at = s - first + cp - adv;
            buf.copy_from_slice(&self.time[at..at + num.fft]);
            fft.process(&mut buf);
            let turn = f_nominal.map_or(Complex32::new(1.0, 0.0), |f| num.carrier_phase(f, 14 * slot + l).conj()) / num.fft as f32;
            for (k, g) in self.grid[l * n_sc..(l + 1) * n_sc].iter_mut().enumerate() {
                let bin = k as i64 - (n_sc / 2) as i64;
                let a = TAU * bin as f64 * adv as f64 / num.fft as f64;
                *g = buf[bin.rem_euclid(num.fft as i64) as usize] * Complex32::new(a.cos() as f32, a.sin() as f32) * turn;
            }
        }
        true
    }

    fn polar(&mut self, payload: usize, level: usize) -> Option<&mut Polar> {
        let key = (payload, level);
        if let Some(i) = self.polar.iter().position(|p| p.0 == key) {
            return Some(&mut self.polar[i].1);
        }
        self.polar.push((key, dci_code(payload, level)?));
        self.polar.last_mut().map(|p| &mut p.1)
    }

    /// Tries the PDCCH candidates of the slot in `self.grid` with CORESET#0 from symbol `first`. Returns the DCI found.
    fn pdcch(&mut self, c0: &Coreset0, pci: u16, slot: usize, first: usize) -> Option<Dci> {
        if first + c0.n_symb > 14 {
            return None;
        }
        let n_sc = c0.n_sc();
        let per = 6 / c0.n_symb;
        let pilots: Vec<Vec<Complex32>> = (0..c0.n_symb).map(|j| pdcch_dmrs(pci, slot, first + j, c0.n_rb)).collect();
        // One line per REG bundle: the precoder is the same over a bundle only.
        let mut fits = vec![Fit::default(); c0.n_cce()];
        let mut points = Vec::with_capacity(18);
        for (b, fit) in fits.iter_mut().enumerate() {
            points.clear();
            for rb in b * per..(b + 1) * per {
                for (j, r) in pilots.iter().enumerate() {
                    for i in 0..3 {
                        let k = 12 * rb + 4 * i + 1;
                        points.push((k as f32, self.grid[(first + j) * n_sc + k] * r[3 * rb + i].conj()));
                    }
                }
            }
            *fit = Fit::of(&points);
        }
        let payload = Dci::len(c0.n_rb);
        let mut llr = Vec::new();
        let mut out = [0u8; 164];
        for (level, cce) in c0.candidates() {
            let rbs = c0.pdcch_rbs(pci, cce, level);
            let (mut power, mut noise) = (0f32, 0f32);
            for j in cce..cce + level {
                let f = &fits[c0.bundle(pci, j)];
                power += f.a.norm_sqr();
                noise += f.noise;
            }
            // Pilots of noise alone give a power of a few hundredths of the noise.
            if power < 0.12 * noise || noise <= 0.0 {
                continue;
            }
            let noise = noise / level as f32;
            let gain = 2.0 * std::f32::consts::SQRT_2 / noise;
            let scr = pdcch_scrambling(pci, 108 * level);
            llr.clear();
            for l in first..first + c0.n_symb {
                for &rb in &rbs {
                    let fit = &fits[rb / per];
                    for i in 0..12 {
                        if i % 4 == 1 {
                            continue;
                        }
                        let k = 12 * rb + i;
                        let z = self.grid[l * n_sc + k] * fit.at(k as f32).conj() * gain;
                        llr.push(z.re);
                        llr.push(z.im);
                    }
                }
            }
            for (v, &s) in llr.iter_mut().zip(&scr) {
                if s == 1 {
                    *v = -*v;
                }
                *v = v.clamp(-1e3, 1e3);
            }
            if llr.iter().any(|v| !v.is_finite()) {
                continue;
            }
            let Some(code) = self.polar(payload, level) else {
                continue;
            };
            let k = payload + 24;
            if code.decode(&llr, |c| dci_crc_ok(c, SI_RNTI), &mut out[..k]).is_ok() {
                if let Some(d) = Dci::unpack(&out[..payload], c0.n_rb) {
                    return Some(d);
                }
            }
        }
        None
    }

    /// The soft bits of the PDSCH of `p` in the slot in `self.grid`, descrambled.
    fn pdsch_soft(&mut self, c0: &Coreset0, pci: u16, slot: usize, p: &Pdsch) -> Option<Vec<f32>> {
        let n_sc = c0.n_sc();
        if p.start + p.len > 14 || p.prbs.iter().any(|&rb| rb >= c0.n_rb) {
            return None;
        }
        // One line per group of two blocks and DM-RS symbol.
        let groups = c0.n_rb.div_ceil(2);
        let mut fits: Vec<Vec<Option<Fit>>> = Vec::new();
        let (mut noise, mut count) = (0f32, 0u32);
        let mut points = Vec::with_capacity(12);
        for &l in &p.dmrs_symbols {
            let r = pdsch_dmrs(pci, slot, l, c0.n_rb);
            let mut row = vec![None; groups];
            for (g, slot_fit) in row.iter_mut().enumerate() {
                points.clear();
                for rb in [2 * g, 2 * g + 1] {
                    if !p.prbs.contains(&rb) {
                        continue;
                    }
                    for i in 0..6 {
                        let k = 12 * rb + 2 * i;
                        points.push((k as f32, self.grid[l * n_sc + k] * r[6 * rb + i].conj() / p.dmrs_gain()));
                    }
                }
                if !points.is_empty() {
                    let f = Fit::of(&points);
                    noise += f.noise;
                    count += 1;
                    *slot_fit = Some(f);
                }
            }
            fits.push(row);
        }
        if count == 0 || noise <= 0.0 {
            return None;
        }
        // The pilots are stronger than the data by the DM-RS gain, so their residual is smaller by its square.
        let noise = noise / count as f32 * p.dmrs_gain().powi(2);
        let gain = 2.0 * std::f32::consts::SQRT_2 / noise;
        let res = p.data_res();
        let scr = pdsch_scrambling(pci, 2 * res.len());
        let mut llr = Vec::with_capacity(2 * res.len());
        let d = &p.dmrs_symbols;
        for &(k, l) in &res {
            let g = k / 24;
            // Between two DM-RS symbols the channel is taken on the line between them, outside from the nearest.
            let after = d.iter().position(|&s| s >= l);
            let h = match after {
                Some(0) => fits[0][g].map(|f| f.at(k as f32)),
                None => fits[d.len() - 1][g].map(|f| f.at(k as f32)),
                Some(i) => match (fits[i - 1][g], fits[i][g]) {
                    (Some(f0), Some(f1)) => {
                        let w = (l - d[i - 1]) as f32 / (d[i] - d[i - 1]) as f32;
                        Some(f0.at(k as f32) * (1.0 - w) + f1.at(k as f32) * w)
                    }
                    _ => None,
                },
            }?;
            let z = self.grid[l * n_sc + k] * h.conj() * gain;
            llr.push(z.re);
            llr.push(z.im);
        }
        for (v, &s) in llr.iter_mut().zip(&scr) {
            if s == 1 {
                *v = -*v;
            }
            *v = v.clamp(-1e3, 1e3);
        }
        llr.iter().all(|v| v.is_finite()).then_some(llr)
    }

    /// Tries to receive the SIB1 of the cell `t` in `cap`.
    pub fn receive(&mut self, cap: &Capture, t: &Sib1Target<'_>) -> Sib1Result {
        let mut out = Sib1Result::default();
        let Some(c0) = Coreset0::from_mib(t.ssb_scs, t.mib) else {
            out.step = Sib1Step::NotScheduled;
            return out;
        };
        let rel = c0.centre_from_ssb(t.ssb_scs, t.mib.k_ssb);
        let f_cap = t.ssb_hz + rel;
        out.rate_needed_hz = 2.0 * (f_cap.abs() + c0.width_hz() / 2.0);
        if out.rate_needed_hz > cap.rate {
            out.step = Sib1Step::TooNarrow;
            return out;
        }
        let nominal = match (t.symbol_phase, t.ssb_nominal_hz) {
            (false, _) => None,
            (true, Some(f)) => Some(f + rel),
            (true, None) => {
                out.step = Sib1Step::NoFrequency;
                return out;
            }
        };
        let num = c0.numerology();
        // The frame start: the PSS is the first symbol of its block, in the half frame the MIB names.
        let ssb = Numerology {
            scs_khz: t.ssb_scs,
            fft: 256,
        };
        let (s, cp) = ssb.symbol(t.pattern.first_symbol(t.ssb as usize));
        let frame0 = t.anchor / cap.rate - (s + cp) as f64 / ssb.rate() - f64::from(t.mib.hrf) * 5e-3;
        let (n0, parity, first) = type0_occasion(t.mib.search_space0, num.mu(), t.ssb as usize, c0.n_symb);
        let slots = num.slots_per_frame();
        let frames = (cap.samples.len() as f64 / cap.rate / 0.01).ceil() as i64 + 1;
        out.step = Sib1Step::NoControl;
        if let Some(d) = self.sch.as_mut() {
            d.reset();
        }
        let lo = (-frame0 / 0.01).floor() as i64 - 1;
        for f in lo..lo + frames + 2 {
            if (i64::from(t.mib.sfn) + f).rem_euclid(2) != parity as i64 {
                continue;
            }
            for d in 0..2 {
                let slot = (n0 + d) % slots;
                let start = (frame0 + f as f64 * 0.01 + num.slot_start(n0 + d) as f64 / num.rate()) * cap.rate;
                if !self.slot_grid(cap, &c0, f_cap, nominal, slot, start) {
                    continue;
                }
                out.occasions += 1;
                let Some(dci) = self.pdcch(&c0, t.pci, slot, first) else {
                    continue;
                };
                out.pdcch += 1;
                // A DCI for another SI message is not a step towards SIB1.
                if dci.si_message {
                    continue;
                }
                out.dci = Some(dci);
                out.step = Sib1Step::NoData;
                let (Some((qm, r)), Some(p)) = (mcs_qpsk(dci.mcs), Pdsch::from_dci(&dci, c0.n_rb, t.mib.dmrs_pos3)) else {
                    continue;
                };
                let Some(size) = tbs(p.re_per_prb(), p.prbs.len(), qm, r) else {
                    continue;
                };
                let Ok(sch) = NrSch::new(size, f64::from(r) / 1024.0) else {
                    continue;
                };
                let Some(llr) = self.pdsch_soft(&c0, t.pci, slot, &p) else {
                    continue;
                };
                // Transmissions of one block in the capture are combined; another size starts anew.
                if self.sch.as_ref().is_none_or(|d| d.sch() != &sch) {
                    self.sch = NrSchDecoder::new(&sch).ok();
                }
                let Some(dec) = self.sch.as_mut() else {
                    continue;
                };
                if dec.add(&llr, dci.rv, qm).is_err() {
                    continue;
                }
                if let Ok(bits) = dec.decode(25) {
                    let mut bytes = Vec::new();
                    pack(&bits, BitOrder::Msb, Pad::Zero, &mut bytes);
                    out.payload = Some(bytes);
                    out.step = Sib1Step::Decoded;
                    return out;
                }
            }
        }
        out
    }
}
