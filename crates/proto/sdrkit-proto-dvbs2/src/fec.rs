//! The DVB-S2 FEC frame on the codes of `sdrkit-algo-fec`: the LDPC code and the BCH code of each code rate and frame size, the
//! bit interleaver, and a decoder that keeps one set of them per MODCOD, so that a frame of a MODCOD seen before allocates nothing.
//!
//! Standard: ETSI EN 302 307-1 clauses 5.3.1 to 5.3.3, tables 5a and 5b.

use super::modcod::{Code, ModCod, Modulation};
use super::pl::{constellation, deinterleaver, interleaver};
use sdrkit_algo_fec::bch::{codes, Bch, BchCode};
use sdrkit_algo_fec::ldpc::tables::{dvb_s2, DvbTable};
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::Complex32;
use sdrkit_util_bits::interleave::Permutation;

/// The most LDPC iterations for one frame.
pub const MAX_ITER: u32 = 50;

/// The LDPC table of a code rate in a normal (64800) or short (16200) frame.
pub fn ldpc_table(rate: (u16, u16), short: bool) -> Option<&'static DvbTable> {
    use dvb_s2::*;
    Some(match (rate, short) {
        ((1, 4), false) => &S2_N_1_4,
        ((1, 3), false) => &S2_N_1_3,
        ((2, 5), false) => &S2_N_2_5,
        ((1, 2), false) => &S2_N_1_2,
        ((3, 5), false) => &S2_N_3_5,
        ((2, 3), false) => &S2_N_2_3,
        ((3, 4), false) => &S2_N_3_4,
        ((4, 5), false) => &S2_N_4_5,
        ((5, 6), false) => &S2_N_5_6,
        ((8, 9), false) => &S2_N_8_9,
        ((9, 10), false) => &S2_N_9_10,
        ((1, 4), true) => &S2_S_1_4,
        ((1, 3), true) => &S2_S_1_3,
        ((2, 5), true) => &S2_S_2_5,
        ((1, 2), true) => &S2_S_1_2,
        ((3, 5), true) => &S2_S_3_5,
        ((2, 3), true) => &S2_S_2_3,
        ((3, 4), true) => &S2_S_3_4,
        ((4, 5), true) => &S2_S_4_5,
        ((5, 6), true) => &S2_S_5_6,
        ((8, 9), true) => &S2_S_8_9,
        _ => return None,
    })
}

/// The LDPC check rule of the receiver for MODCOD `m` in a normal or short frame: offset min-sum (offset 1) for the
/// MODCODs listed below, else [`MinSum::for_rate`] of the true code rate. `None` when the frame size has no code.
///
/// | MODCOD            | frames | scale, offset |
/// |-------------------|--------|---------------|
/// | QPSK 1/4          | short  | 15/16, 1      |
/// | QPSK 2/5          | both   | 14/16, 1      |
/// | QPSK 3/5          | both   | 13/16, 1      |
/// | 8PSK 3/5          | both   | 14/16, 1      |
/// | 32APSK 3/4 to 5/6 | normal | 15/16, 1      |
///
/// The offset also lowers the check messages of magnitude 1 and 2, which a scale alone leaves unchanged. Each listed rule
/// decodes at 0.13 to 0.30 dB lower Es/N0 than [`MinSum::for_rate`] at frame error rates of 0.1 and 0.01, and loses no
/// more frames 0.3 dB above that point. On the other MODCODs measured no offset rule gains 0.1 dB; on 32APSK 8/9 and
/// 9/10 an offset loses 0.1 dB. Measured by `examples/ldpc_rule.rs`: white noise, the demapper and the 8-bit LLRs of
/// [`FrameDecoder`], [`MAX_ITER`] iterations, a frame lost when BCH fails.
pub fn ldpc_rule(m: &ModCod, short: bool) -> Option<MinSum> {
    let t = ldpc_table(m.rate, short)?;
    let offset = |scale_q4| MinSum { scale_q4, offset: 1 };
    Some(match (m.modulation, m.rate, short) {
        (Modulation::Qpsk, (1, 4), true) => offset(15),
        (Modulation::Qpsk, (2, 5), _) => offset(14),
        (Modulation::Qpsk, (3, 5), _) => offset(13),
        (Modulation::Psk8, (3, 5), _) => offset(14),
        (Modulation::Apsk32, (3, 4) | (4, 5) | (5, 6), false) => offset(15),
        _ => MinSum::for_rate(t.k as usize, t.n as usize),
    })
}

/// The BCH code of a code rate in a normal or short frame.
pub fn dvb_bch(rate: (u16, u16), short: bool) -> Option<&'static BchCode> {
    let key = format!(
        "dvb_s2_{}_{}_{}",
        if short { "short" } else { "normal" },
        rate.0,
        rate.1
    );
    codes::by_key(&key)
}

/// The max-log demapper of a constellation whose points all have magnitude 1 (QPSK and 8PSK). The nearest point of a bit
/// value is then the point of the largest correlation with the symbol, so the LLR of a bit is twice the difference of two
/// such correlations over the noise variance, and no distance is made. The values are those of
/// `Constellation::demap` with `Demap::MaxLog`.
#[derive(Clone, Debug)]
pub struct PskDemap {
    bits: usize,
    re: [f32; 8],
    im: [f32; 8],
    /// For each bit and bit value: 0 for the points whose label has this value, else [`PskDemap::OUT`].
    out: [[[f32; 8]; 2]; 3],
}

impl PskDemap {
    /// What takes a point out of a search for the largest correlation.
    const OUT: f32 = 1e9;

    /// The demapper for `points` by label, bit 0 of the output from the highest label bit. `None` unless they are 4 or 8
    /// points of magnitude 1.
    pub fn new(points: &[Complex32]) -> Option<PskDemap> {
        let bits = match points.len() {
            4 => 2,
            8 => 3,
            _ => return None,
        };
        if points.iter().any(|p| (p.norm_sqr() - 1.0).abs() > 1e-4) {
            return None;
        }
        let mut d = PskDemap {
            bits,
            re: [0.0; 8],
            im: [0.0; 8],
            out: [[[Self::OUT; 8]; 2]; 3],
        };
        for (label, p) in points.iter().enumerate() {
            d.re[label] = p.re;
            d.im[label] = p.im;
            for b in 0..bits {
                d.out[b][(label >> (bits - 1 - b)) & 1][label] = 0.0;
            }
        }
        Some(d)
    }

    /// The LLRs of the bits of `z` for the weight `w` (1 over the noise variance) into `llr` (one value per bit).
    #[inline]
    pub fn llrs(&self, z: Complex32, w: f32, llr: &mut [f32]) {
        let mut corr = [0f32; 8];
        for k in 0..8 {
            corr[k] = z.re * self.re[k] + z.im * self.im[k];
        }
        for (o, out) in llr.iter_mut().zip(&self.out[..self.bits]) {
            let (mut c0, mut c1) = (f32::MIN, f32::MIN);
            for k in 0..8 {
                c0 = c0.max(corr[k] - out[0][k]);
                c1 = c1.max(corr[k] - out[1][k]);
            }
            *o = 2.0 * w * (c0 - c1);
        }
    }
}

/// The codes of one MODCOD and frame size.
pub struct S2Codec {
    /// The frame parameters.
    pub code: Code,
    /// The LDPC code.
    pub ldpc: LdpcCode,
    /// The BCH code.
    pub bch: Bch,
    /// The constellation, for the demapper.
    pub cons: Constellation,
    /// The demapper of QPSK and 8PSK.
    pub psk: Option<PskDemap>,
    /// The points by label, for the generator.
    pub points: Vec<Complex32>,
    /// The map from a received bit position to the code word position (`None` for QPSK).
    pub deinterleave: Option<Permutation>,
    /// The map from a code word position to the sent position (`None` for QPSK).
    pub interleave: Option<Permutation>,
    rule: MinSum,
    decoder: Option<LdpcDecoder>,
}

impl S2Codec {
    /// The codes of `m` in a normal or short frame. `None` when the frame size does not exist for the rate (9/10 short).
    pub fn new(m: &ModCod, short: bool) -> Option<S2Codec> {
        let code = m.code(short)?;
        let ldpc = LdpcCode::from_dvb(ldpc_table(m.rate, short)?).ok()?;
        let bch = Bch::from_code(dvb_bch(m.rate, short)?).ok()?;
        let cons = Constellation::new(super::pl::point_set(m)).ok()?;
        let points = constellation(m);
        Some(S2Codec {
            code,
            bch,
            cons,
            psk: PskDemap::new(&points),
            points,
            deinterleave: deinterleaver(m, code.nldpc),
            interleave: interleaver(m, code.nldpc),
            rule: ldpc_rule(m, short)?,
            ldpc,
            decoder: None,
        })
    }

    /// The check rule of the LDPC decoder.
    pub fn rule(&self) -> MinSum {
        self.rule
    }

    /// Sets the check rule of the LDPC decoder; the next decode makes a new decoder.
    pub fn set_rule(&mut self, rule: MinSum) {
        self.rule = rule;
        self.decoder = None;
    }

    /// The LDPC decoder (made on first use).
    pub fn decoder(&mut self) -> &mut LdpcDecoder {
        let rule = self.rule;
        self.decoder.get_or_insert_with(|| LdpcDecoder::new(&self.ldpc, rule))
    }
}

/// The result of decoding one FEC frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameResult {
    /// LDPC iterations.
    pub iterations: u32,
    /// The LDPC checks held at the end.
    pub ldpc_ok: bool,
    /// The BCH word was a code word or was corrected.
    pub bch_ok: bool,
}

/// The receive side of the DVB-S2 FEC frame: demapping, de-interleaving, LDPC, BCH and the BBFRAME bytes. One [`S2Codec`] is
/// kept per MODCOD and frame size, so ACM and VCM streams cost one set of tables each.
#[derive(Default)]
pub struct FrameDecoder {
    codecs: Vec<((u8, bool), Option<S2Codec>)>,
    soft: Vec<f32>,
    llr: Vec<i8>,
    cw_llr: Vec<i8>,
    hard: Vec<u8>,
}

impl FrameDecoder {
    /// The codec of `m` and the frame size, made once.
    pub fn codec(&mut self, m: &ModCod, short: bool) -> Option<&mut S2Codec> {
        let key = (m.id, short);
        let i = match self.codecs.iter().position(|c| c.0 == key) {
            Some(i) => i,
            None => {
                self.codecs.push((key, S2Codec::new(m, short)));
                self.codecs.len() - 1
            }
        };
        self.codecs[i].1.as_mut()
    }

    /// The codecs made so far.
    pub fn codecs_made(&self) -> usize {
        self.codecs.len()
    }

    /// Decodes the data symbols of one PLFRAME (`syms`, descrambled, pilots removed, unit mean power) of MODCOD `m` with noise
    /// variance `noise_var` into the BBFRAME bytes in `out` (`kbch / 8` bytes, still BB scrambled). `None` when the frame is
    /// short of symbols or the MODCOD has no code.
    pub fn decode(
        &mut self,
        m: &ModCod,
        short: bool,
        syms: &[Complex32],
        noise_var: f32,
        out: &mut Vec<u8>,
    ) -> Option<FrameResult> {
        let FrameDecoder {
            codecs,
            soft,
            llr,
            cw_llr,
            hard,
        } = self;
        let key = (m.id, short);
        let i = match codecs.iter().position(|c| c.0 == key) {
            Some(i) => i,
            None => {
                codecs.push((key, S2Codec::new(m, short)));
                codecs.len() - 1
            }
        };
        let c = codecs[i].1.as_mut()?;
        let n = c.code.nldpc;
        let bits = c.cons.bits();
        if syms.len() * bits < n {
            return None;
        }
        soft.clear();
        soft.resize(n, 0.0);
        let nv = if noise_var.is_finite() && noise_var > 1e-6 {
            noise_var
        } else {
            1e-6
        };
        let finite = |z: &Complex32| {
            if z.re.is_finite() && z.im.is_finite() {
                *z
            } else {
                Complex32::new(0.0, 0.0)
            }
        };
        match &c.psk {
            Some(psk) => {
                let w = 1.0 / nv;
                for (z, o) in syms.iter().zip(soft.chunks_exact_mut(bits)) {
                    psk.llrs(finite(z), w, o);
                }
            }
            None => {
                for (z, o) in syms.iter().zip(soft.chunks_exact_mut(bits)) {
                    let _ = c.cons.demap(finite(z), nv, Demap::MaxLog, o);
                }
            }
        }
        llr.clear();
        llr.resize(n, 0);
        let q = Quantizer::default();
        let scale = q.scale_for(soft);
        q.quantize(soft, scale, llr).ok()?;
        cw_llr.clear();
        cw_llr.resize(n, 0);
        match &c.deinterleave {
            Some(p) => p.apply(llr, cw_llr).ok()?,
            None => cw_llr.copy_from_slice(llr),
        }
        hard.clear();
        hard.resize(n, 0);
        let (iterations, ldpc_ok) = match c.decoder().decode(cw_llr, MAX_ITER, hard) {
            Ok(d) => (d.iterations, true),
            Err(_) => (MAX_ITER, false),
        };
        let kldpc = c.code.kldpc;
        let bch_ok = c.bch.decode(&mut hard[..kldpc]).is_ok();
        out.clear();
        out.extend(
            hard[..c.code.kbch]
                .chunks(8)
                .map(|b| b.iter().fold(0u8, |a, &v| (a << 1) | (v & 1))),
        );
        Some(FrameResult {
            iterations,
            ldpc_ok,
            bch_ok,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(seed: &mut u64) -> f32 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        (*seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }

    /// The demapper of QPSK and 8PSK returns the max-log LLRs of the general demapper.
    #[test]
    fn psk_demapper_is_the_max_log_demapper() {
        let mut seed = 0x243f_6a88_85a3_08d3u64;
        for id in [4u8, 13] {
            let m = ModCod::new(id).unwrap();
            let cons = Constellation::new(super::super::pl::point_set(&m)).unwrap();
            let psk = PskDemap::new(&constellation(&m)).unwrap();
            let bits = cons.bits();
            for _ in 0..4000 {
                let z = Complex32::new(1.5 * noise(&mut seed), 1.5 * noise(&mut seed));
                let nv = 0.02 + noise(&mut seed).abs();
                let (mut want, mut got) = ([0f32; 3], [0f32; 3]);
                cons.demap(z, nv, Demap::MaxLog, &mut want[..bits]).unwrap();
                psk.llrs(z, 1.0 / nv, &mut got[..bits]);
                for b in 0..bits {
                    assert!(
                        (want[b] - got[b]).abs() <= 1e-4 * (1.0 + want[b].abs()),
                        "MODCOD {id} bit {b} at {z}: {} for {}",
                        got[b],
                        want[b]
                    );
                }
            }
        }
        assert!(PskDemap::new(&constellation(&ModCod::new(18).unwrap())).is_none());
    }

    /// The min-sum scale of each code follows its true rate: the short codes of the names 1/4 and 1/2 hold 1/5 and 4/9.
    #[test]
    fn the_min_sum_scale_follows_the_true_code_rate() {
        let scale = |rate, short| {
            let code = LdpcCode::from_dvb(ldpc_table(rate, short).unwrap()).unwrap();
            MinSum::for_rate(code.k(), code.n()).scale_q4
        };
        for short in [false, true] {
            assert_eq!(scale((1, 4), short), 15);
            assert_eq!(scale((1, 3), short), 14);
            assert_eq!(scale((2, 5), short), 14);
            for rate in [(3, 5), (2, 3), (3, 4), (4, 5), (5, 6), (8, 9)] {
                assert_eq!(scale(rate, short), 13, "{rate:?} short {short}");
            }
        }
        assert_eq!([scale((1, 2), false), scale((1, 2), true)], [13, 14]);
        assert_eq!(scale((9, 10), false), 13);
    }

    /// The receiver uses offset min-sum on the listed MODCODs and the scale of the code rate on the others.
    #[test]
    fn the_receiver_rule_has_an_offset_on_the_listed_modcods() {
        let rule = |id: u8, short: bool| ldpc_rule(&ModCod::new(id).unwrap(), short).unwrap();
        let offset = |scale_q4| MinSum { scale_q4, offset: 1 };
        assert_eq!(rule(1, true), offset(15));
        assert_eq!(rule(1, false), MinSum::with_scale(15));
        for short in [false, true] {
            assert_eq!(rule(3, short), offset(14));
            assert_eq!(rule(5, short), offset(13));
            assert_eq!(rule(12, short), offset(14));
            assert_eq!(rule(13, short), MinSum::with_scale(13));
        }
        assert_eq!([rule(4, false), rule(4, true)], [MinSum::with_scale(13), MinSum::with_scale(14)]);
        assert_eq!([rule(24, false), rule(25, false), rule(26, false)], [offset(15); 3]);
        assert_eq!(rule(27, false), MinSum::with_scale(13));
        assert_eq!([rule(24, true), rule(25, true)], [MinSum::with_scale(13); 2]);
        assert!(ldpc_rule(&ModCod::new(11).unwrap(), true).is_none());
        let mut c = S2Codec::new(&ModCod::new(5).unwrap(), true).unwrap();
        assert_eq!(c.rule(), offset(13));
        c.set_rule(MinSum::default());
        assert_eq!(c.rule(), MinSum::default());
    }

    /// Every LDPC code of DVB-S2 decodes on the fastest kernel of the machine, with the rule of each MODCOD.
    #[cfg(target_arch = "aarch64")]
    #[test]
    fn every_ldpc_code_runs_on_the_neon_kernel() {
        for m in ModCod::all() {
            for short in [false, true] {
                let Some(rule) = ldpc_rule(&m, short) else {
                    assert_eq!((m.rate, short), ((9, 10), true));
                    continue;
                };
                let code = LdpcCode::from_dvb(ldpc_table(m.rate, short).unwrap()).unwrap();
                let d = LdpcDecoder::new(&code, rule);
                assert_eq!(d.backend(), "neon", "{} short {short}", m.name());
            }
        }
    }
}
