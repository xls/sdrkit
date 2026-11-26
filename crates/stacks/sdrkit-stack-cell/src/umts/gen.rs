//! The UMTS test signal: WCDMA FDD downlinks at two samples a chip with a root raised cosine pulse. Each cell sends its CPICH,
//! the primary and secondary synchronisation codes of its code group (multiplied by the STTD symbol a), a dummy data channel of
//! random chips, and when it has a PLMN, the P-CCPCH with the BCH: the transport blocks alternate between the MIB and SIB 3, coded
//! as TS 25.212 gives it. Unit mean power.

use super::bch::{complete_list_block, complete_sib_block, encode, mib_data, Mib, FRAME_BITS, SIB_3, SIB_MIB};
use super::sib::{sib1_data, sib1_of, sib3_data, Sib3, SIB_1};
use super::CHIP_RATE;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::{self, Norm, Shape};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_modem::spreading::wcdma::{psc, ssc, DlScrambling, FRAME_CHIPS, SLOT_CHIPS, SSC_GROUPS, SYNC_CHIPS};
use sdrkit_algo_modem::spreading::ovsf;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};

/// The native rate of the generator and the working rate of the receiver: two samples a chip.
pub const NATIVE_RATE: f64 = 2.0 * CHIP_RATE;
/// Chips made at a time.
const BLOCK_CHIPS: usize = 2560;

/// One cell of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct UmtsCell {
    /// The primary scrambling code (0 to 511).
    pub code: usize,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The amplitude.
    pub amp: f32,
    /// The stream chip at which the cell's frame with SFN 0 starts.
    pub chip_offset: i64,
    /// The power of the dummy data channel.
    pub data_power: f32,
    /// The PLMN and the cell identity sent on the BCH; no P-CCPCH without them.
    pub broadcast: Option<(Mib, u32)>,
    /// The SCH symbol a is +1 (the P-CCPCH STTD encoded); the P-CCPCH itself is sent from one antenna.
    pub sttd: bool,
}

/// The cells of the stack's own test signal.
pub fn scene() -> Vec<UmtsCell> {
    let plmn = Mib {
        mcc: 1,
        mnc: 1,
        mnc_digits: 2,
    };
    vec![
        UmtsCell {
            code: 17,
            offset_hz: 0.0,
            amp: 1.0,
            chip_offset: 500,
            data_power: 0.4,
            broadcast: Some((plmn.clone(), 0x0012_3456)),
            sttd: false,
        },
        UmtsCell {
            code: 280,
            offset_hz: 2300.0,
            amp: 0.7,
            chip_offset: 9000,
            data_power: 0.3,
            broadcast: Some((plmn, 0x0023_4567)),
            sttd: false,
        },
    ]
}

/// The chips of one cell, made a frame at a time.
struct CellChips {
    cfg: UmtsCell,
    scr: Vec<(i8, i8)>,
    group: usize,
    psc: [i8; SYNC_CHIPS],
    sscs: Vec<[i8; SYNC_CHIPS]>,
    c1: Vec<i8>,
    rng: Rng,
    held: Option<i64>,
    chips: Vec<Complex32>,
    /// The channel bits of the TTI held, and the TTI.
    tti: Option<(i64, [[u8; FRAME_BITS]; 2])>,
}

impl CellChips {
    fn new(cfg: UmtsCell, seed: u64) -> Self {
        CellChips {
            scr: DlScrambling::primary(cfg.code % 512),
            group: (cfg.code % 512) / 8,
            psc: psc(),
            sscs: (1..=16).map(ssc).collect(),
            c1: ovsf(256, 1),
            rng: Rng::new(seed),
            held: None,
            chips: Vec::new(),
            tti: None,
            cfg,
        }
    }

    /// The channel bits of frame `f` (SFN `f` modulo 4096), from the TTI of frames `f & !1` and `f | 1`.
    fn pccpch_bits(&mut self, f: i64) -> Option<[u8; FRAME_BITS]> {
        let (mib, cell) = self.cfg.broadcast.clone()?;
        let tti = f.div_euclid(2);
        if self.tti.map(|t| t.0) != Some(tti) {
            let sfn = (2 * tti).rem_euclid(4096) as u32;
            // The MIB every other TTI, SIB 3 and SIB 1 together between. The location area is the upper 16 bits of the cell
            // identity, the routing area its low 8 bits.
            let block = if tti.rem_euclid(2) == 0 {
                complete_sib_block(sfn, SIB_MIB, &mib_data(&mib))
            } else {
                let s3 = Sib3 {
                    cell_id: cell,
                    ..Sib3::default()
                };
                let s1 = sib1_of((cell >> 12) as u16, cell as u8);
                complete_list_block(sfn, &[(SIB_3, sib3_data(&s3)), (SIB_1, sib1_data(&s1))])
                    .unwrap_or_else(|| complete_sib_block(sfn, SIB_3, &sib3_data(&s3)))
            };
            self.tti = Some((tti, encode(&block)));
        }
        self.tti.map(|t| t.1[f.rem_euclid(2) as usize])
    }

    fn make(&mut self, f: i64) {
        let bits = self.pccpch_bits(f);
        let a = if self.cfg.sttd { 1.0 } else { -1.0 };
        let dp = (self.cfg.data_power / 2.0).sqrt();
        let one_j = Complex32::new(1.0, 1.0);
        self.chips.clear();
        for i in 0..FRAME_CHIPS {
            let (si, sq) = self.scr[i];
            let s = Complex32::new(f32::from(si), f32::from(sq));
            let q = self.rng.next_u64();
            let d = Complex32::new(
                if q & 1 == 0 { dp } else { -dp },
                if q & 2 == 0 { dp } else { -dp },
            );
            let mut c = one_j * s * 0.3 + d * s * std::f32::consts::FRAC_1_SQRT_2;
            let slot = i / SLOT_CHIPS;
            let at = i % SLOT_CHIPS;
            if at < SYNC_CHIPS {
                let k = usize::from(SSC_GROUPS[self.group][slot]) - 1;
                c += one_j * (0.35 * a * f32::from(self.psc[at] + self.sscs[k][at]));
            } else if let Some(b) = bits.as_ref() {
                let sym = (at - SYNC_CHIPS) / 256;
                let n = 18 * slot + 2 * sym;
                let v = Complex32::new(1.0 - 2.0 * f32::from(b[n]), 1.0 - 2.0 * f32::from(b[n + 1]));
                c += v * s * (0.3 * f32::from(self.c1[(at - SYNC_CHIPS) % 256]));
            }
            self.chips.push(c * self.cfg.amp);
        }
        self.held = Some(f);
    }

    /// The chip at stream chip `t`.
    fn chip(&mut self, t: i64) -> Complex32 {
        let u = t - self.cfg.chip_offset;
        let f = u.div_euclid(FRAME_CHIPS as i64);
        if self.held != Some(f) {
            self.make(f);
        }
        self.chips[u.rem_euclid(FRAME_CHIPS as i64) as usize]
    }
}

/// The generator.
pub struct UmtsGenerator {
    rate: f64,
    cells: Vec<(CellChips, Nco)>,
    taps: Vec<f32>,
    t: i64,
    hist: Vec<Complex32>,
    native: Vec<Complex32>,
    scale: f32,
    resampler: Option<FractionalResampler<Complex32>>,
    out: Vec<Complex32>,
    pos: usize,
}

impl UmtsGenerator {
    /// The `cells` at output rate `rate`, from `seed`, with a root raised cosine of `roll_off`.
    pub fn new(rate: f64, cells: Vec<UmtsCell>, seed: u64, roll_off: f64) -> Self {
        let mut g = Self::build(rate, cells.clone(), seed, roll_off);
        let mut probe = Self::build(NATIVE_RATE, cells, seed, roll_off);
        let mut x = vec![Complex32::default(); (0.02 * NATIVE_RATE) as usize];
        probe.generate(&mut x);
        let p = x.iter().map(|v| v.norm_sqr()).sum::<f32>() / x.len() as f32;
        if p > 0.0 {
            g.scale = p.sqrt().recip();
        }
        g
    }

    fn build(rate: f64, cells: Vec<UmtsCell>, seed: u64, roll_off: f64) -> Self {
        let taps = pulse::taps(Shape::Rrc { beta: roll_off }, 2.0, 12.0, Norm::Energy)
            .unwrap_or_else(|_| vec![1.0]);
        let cells = cells
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let nco = Nco::new(NATIVE_RATE, c.offset_hz);
                (CellChips::new(c, seed ^ (0x554D_0000 + i as u64)), nco)
            })
            .collect();
        UmtsGenerator {
            rate,
            cells,
            hist: vec![Complex32::default(); taps.len()],
            taps,
            t: 0,
            native: Vec::new(),
            scale: 1.0,
            resampler: ((rate - NATIVE_RATE).abs() > 1e-6)
                .then(|| FractionalResampler::with_rates(NATIVE_RATE, rate).ok())
                .flatten(),
            out: Vec::new(),
            pos: 0,
        }
    }

    fn refill(&mut self) {
        let n = 2 * BLOCK_CHIPS;
        let mut sum = vec![Complex32::default(); n];
        let mut part = vec![Complex32::default(); n];
        for (cell, nco) in self.cells.iter_mut() {
            part.fill(Complex32::default());
            for c in 0..BLOCK_CHIPS {
                part[2 * c] = cell.chip(self.t + c as i64);
            }
            nco.mix_in_place(&mut part);
            for (s, p) in sum.iter_mut().zip(&part) {
                *s += p;
            }
        }
        self.t += BLOCK_CHIPS as i64;
        let l = self.taps.len();
        let mut ext = std::mem::take(&mut self.hist);
        ext.truncate(l - 1);
        ext.extend_from_slice(&sum);
        self.native.clear();
        for i in 0..n {
            let mut acc = Complex32::default();
            for (k, &h) in self.taps.iter().enumerate() {
                acc += ext[i + l - 1 - k] * h;
            }
            self.native.push(acc * self.scale);
        }
        self.hist = ext[ext.len() - (l - 1)..].to_vec();
        self.out.clear();
        self.pos = 0;
        match self.resampler.as_mut() {
            Some(r) => r.process(&self.native, &mut self.out),
            None => self.out.extend_from_slice(&self.native),
        }
    }
}

impl Generator for UmtsGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.pos >= self.out.len() {
                self.refill();
                continue;
            }
            let n = (self.out.len() - self.pos).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.out[self.pos..self.pos + n]);
            self.pos += n;
            i += n;
        }
        out.len()
    }
    /// The SNR of the test signal is measured over the 7.68 MHz of two samples a chip, as the stack's tests state it.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NATIVE_RATE).with_symbol_rate(CHIP_RATE))
    }
}
