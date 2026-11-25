//! The VHT PHY of 802.11ac for a single user with one spatial stream at 20 MHz and the convolutional code, VHT-MCS 0 to 8: the
//! rate table, the symbol count from L-SIG, and a PPDU builder for test signals. [`crate::ppdu::PpduDecoder`] decodes the data.
//!
//! The fields after L-SIG: VHT-SIG-A (two symbols, the second with BPSK on the Q axis), VHT-STF, VHT-LTF, VHT-SIG-B, the data
//! symbols. At 20 MHz the tone plan, the long training symbol and the interleaver are those of [`crate::ht`]; the pilots rotate
//! and their polarity index starts at 4. The data field carries SERVICE (with the CRC of VHT-SIG-B), the A-MPDU, pad bits and
//! the tail at the end of the last symbol. VHT-MCS 9 does not exist for one stream at 20 MHz.
//!
//! Standard: IEEE Std 802.11-2020 clause 21.

use crate::ht::{self, pilot, PilotPattern, N_COL, N_SD, PILOT_K};
use crate::ofdm::{self, bin, encode_signal, ltf_time, map_symbol, stf_bins, Modulation, Rate, Signal, NFFT};
use crate::sig::{vht_sig_b_20, vht_sig_b_crc, VhtSigA};
use crate::{mac, Complex32};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_fec::conv::{codes, puncture, Encoder, Puncture};
use sdrkit_algo_modem::ofdm::ieee80211_pilot_polarity;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::{pack, seq, BitOrder};

/// Samples from the start of the first L-LTF symbol to the first sample of the VHT data field at 20 Msps.
pub const DATA_START: usize = 128 + 80 + 160 + 80 + 80 + 80;

/// A VHT-MCS of one spatial stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VhtMcs(pub u8);

impl VhtMcs {
    /// Coded bits per subcarrier: 1, 2, 4, 6 or 8 (256-QAM for MCS 8 and 9).
    pub fn n_bpsc(self) -> usize {
        match self.0 {
            0 => 1,
            1 | 2 => 2,
            3 | 4 => 4,
            5..=7 => 6,
            _ => 8,
        }
    }
    /// The coding rate as (numerator, denominator).
    pub fn coding(self) -> (usize, usize) {
        match self.0 {
            0 | 1 | 3 => (1, 2),
            5 => (2, 3),
            7 | 9 => (5, 6),
            _ => (3, 4),
        }
    }
    /// The puncturing pattern.
    pub fn puncture(self) -> Puncture<'static> {
        match self.coding() {
            (1, 2) => codes::NONE_1_2,
            (2, 3) => codes::WIFI_2_3,
            (5, 6) => codes::WIFI_5_6,
            _ => codes::WIFI_3_4,
        }
    }
    /// Data bits per symbol at 20 MHz.
    pub fn n_dbps(self) -> usize {
        let (a, b) = self.coding();
        N_SD * self.n_bpsc() * a / b
    }
    /// Whether the MCS exists for one stream at 20 MHz (0 to 8).
    pub fn valid_20(self) -> bool {
        self.0 <= 8
    }
    /// The data rate at 20 MHz, Mbit/s.
    pub fn rate_mbps(self, short_gi: bool) -> f64 {
        self.n_dbps() as f64 / if short_gi { 3.6 } else { 4.0 }
    }
}

/// The data symbols of a VHT PPDU from the L-SIG LENGTH: the time after L-SIG less VHT-SIG-A, VHT-STF, one VHT-LTF and
/// VHT-SIG-B (20 µs), in symbols of 4 µs, or of 3.6 µs with the short guard interval, less one when `disambiguation`.
pub fn n_sym(l_length: u16, short_gi: bool, disambiguation: bool) -> usize {
    // Units of 0.1 µs.
    let after = (usize::from(l_length) + 3) / 3 * 40;
    let data = after.saturating_sub(200);
    if short_gi {
        (data / 36).saturating_sub(usize::from(disambiguation))
    } else {
        data / 40
    }
}

/// The L-SIG LENGTH of a VHT PPDU with one VHT-LTF and `n_sym` data symbols.
pub fn l_length(n_sym: usize, short_gi: bool) -> u16 {
    let data = if short_gi { 40 * (n_sym * 36).div_ceil(40) } else { 40 * n_sym };
    (3 * (200 + data).div_ceil(40) - 3) as u16
}

/// The octets of the PSDU that fills the data symbols needed for `len` octets: `(N_SYM N_DBPS - 22) / 8`, with N_SYM.
pub fn psdu_len(len: usize, mcs: VhtMcs) -> (usize, usize) {
    let n_sym = (8 * len + 22).div_ceil(mcs.n_dbps());
    ((n_sym * mcs.n_dbps() - 22) / 8, n_sym)
}

/// Builds a single-user VHT PPDU at 20 Msps for the A-MPDU `ampdu` at `mcs` (0 to 8) with scrambler seed `seed`. The A-MPDU is
/// padded to the PSDU length with EOF delimiters of length 0 and then zero octets, as the MAC does. Every field has unit mean
/// power on its subcarriers.
pub fn build_ppdu(ampdu: &[u8], mcs: VhtMcs, short_gi: bool, seed: u8, out: &mut Vec<Complex32>) {
    let mut fft = Fft::inverse(NFFT);
    let l = 1.0 / 52f32.sqrt();
    let h = 1.0 / 56f32.sqrt();
    let pol = ieee80211_pilot_polarity();
    let (len, n_sym) = psdu_len(ampdu.len(), mcs);
    let mut psdu = ampdu.to_vec();
    while !psdu.len().is_multiple_of(4) && psdu.len() < len {
        psdu.push(0);
    }
    while psdu.len() + 4 <= len {
        mac::push_ampdu_subframe(&mut psdu, &[], true);
    }
    psdu.resize(len, 0);
    // L-STF, L-LTF, L-SIG.
    let mut x = stf_bins();
    fft.process(&mut x);
    out.extend((0..160).map(|i| x[i % NFFT] * l));
    let lt = ltf_time();
    out.extend_from_slice(&lt[32..]);
    out.extend_from_slice(&lt);
    out.extend_from_slice(&lt);
    let non_ht_dc = ofdm::data_carriers();
    let non_ht = |coded: &[u8], q: bool, p: i8| -> [Complex32; NFFT] {
        let mut cells = [Complex32::default(); 48];
        let _ = map_symbol(coded, Modulation::Bpsk, &mut cells);
        let mut x = [Complex32::default(); NFFT];
        for (&k, &c) in non_ht_dc.iter().zip(&cells) {
            x[bin(k)] = if q { Complex32::new(0.0, c.re) } else { c };
        }
        for (k, v) in ofdm::PILOTS {
            x[bin(k)] = Complex32::new(v * f32::from(p), 0.0);
        }
        x
    };
    let sig = encode_signal(&Signal {
        rate: Rate::R6,
        length: l_length(n_sym, short_gi),
    });
    ht::symbol(&mut fft, non_ht(&sig, false, pol[0]), 16, l, out);
    // VHT-SIG-A: BPSK, then BPSK on the Q axis.
    let a = VhtSigA::su(20, 1, mcs.0, short_gi, false, n_sym);
    let mut coded = Vec::with_capacity(96);
    Encoder::new(codes::WIFI_K7).encode(&a.bits(), &mut coded);
    let r6 = Rate::R6.interleaver();
    for s in 0..2 {
        let mut half = [0u8; 48];
        let _ = r6.apply(&coded[48 * s..48 * (s + 1)], &mut half);
        ht::symbol(&mut fft, non_ht(&half, s == 1, pol[1 + s]), 16, l, out);
    }
    // VHT-STF and VHT-LTF.
    let mut x = stf_bins();
    fft.process(&mut x);
    out.extend((0..80).map(|i| x[i % NFFT] * h));
    ht::symbol(&mut fft, ht::ht_ltf_bins(), 16, h, out);
    // VHT-SIG-B: 26 bits at rate 1/2 on the 52 data subcarriers.
    let dc = ht::data_carriers();
    let b = vht_sig_b_20(ampdu.len().div_ceil(4) as u32);
    let mut coded = Vec::with_capacity(52);
    Encoder::new(codes::WIFI_K7).encode(&b, &mut coded);
    let p52 = Permutation::wifi(N_SD, 1, N_COL).unwrap_or_else(|_| unreachable!("52 is a multiple of 13"));
    let mut sent = [0u8; 52];
    let _ = p52.apply(&coded, &mut sent);
    let mut x = [Complex32::default(); NFFT];
    for (&k, &bit) in dc.iter().zip(&sent) {
        x[bin(k)] = Complex32::new(if bit == 1 { 1.0 } else { -1.0 }, 0.0);
    }
    for (i, &k) in PILOT_K.iter().enumerate() {
        x[bin(k)] = Complex32::new(pilot(PilotPattern::Rotating, i, 0, pol[3]), 0.0);
    }
    ht::symbol(&mut fft, x, 16, h, out);
    // Data: SERVICE with the VHT-SIG-B CRC, the PSDU, pad bits, the tail.
    let n_data = n_sym * mcs.n_dbps();
    let mut bits = vec![0u8; 8];
    let crc = vht_sig_b_crc(&b);
    bits.extend((0..8).map(|k| (crc >> k) & 1));
    pack::unpack(&psdu, BitOrder::Lsb, &mut bits);
    bits.resize(n_data, 0);
    let mut s = seq::ieee80211(seed.max(1));
    for v in bits.iter_mut() {
        *v ^= s.bit();
    }
    bits[n_data - 6..].fill(0);
    let mut coded = Vec::with_capacity(2 * n_data);
    Encoder::new(codes::WIFI_K7).encode(&bits, &mut coded);
    let m = mcs.n_bpsc();
    let n_cbps = N_SD * m;
    let mut punctured = Vec::with_capacity(n_sym * n_cbps);
    puncture(&mcs.puncture(), 0, &coded, &mut punctured);
    let perm = Permutation::wifi(n_cbps, m, N_COL).unwrap_or_else(|_| unreachable!("52 N_BPSC is a multiple of 13"));
    let pts = match m {
        1 => Modulation::Bpsk.points(),
        2 => Modulation::Qpsk.points(),
        4 => Modulation::Qam16.points(),
        6 => Modulation::Qam64.points(),
        _ => ht::qam256(),
    };
    let gi = if short_gi { 8 } else { 16 };
    let mut sym = vec![0u8; n_cbps];
    for (n, src) in punctured.chunks_exact(n_cbps).enumerate() {
        let _ = perm.apply(src, &mut sym);
        let mut x = [Complex32::default(); NFFT];
        for (&k, g) in dc.iter().zip(sym.chunks_exact(m)) {
            let label = g.iter().fold(0usize, |a, &b| (a << 1) | usize::from(b & 1));
            x[bin(k)] = pts.points()[label];
        }
        let p = pol[(n + 4) % 127];
        for (i, &k) in PILOT_K.iter().enumerate() {
            x[bin(k)] = Complex32::new(pilot(PilotPattern::Rotating, i, n, p), 0.0);
        }
        ht::symbol(&mut fft, x, gi, h, out);
    }
    if short_gi {
        // The PPDU ends on a 4 µs boundary.
        let total = 40 * (n_sym * 36).div_ceil(40) * 2;
        out.extend(std::iter::repeat_n(Complex32::default(), total - n_sym * 72));
    }
}
