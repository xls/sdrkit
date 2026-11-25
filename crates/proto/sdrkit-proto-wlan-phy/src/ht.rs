//! The HT-mixed PHY of 802.11n for one spatial stream at 20 MHz with the convolutional code, MCS 0 to 7: the HT-SIG field with
//! its CRC, the HT data field in both directions, a PPDU builder for test signals and the decoder of the HT part of a PPDU
//! that the non-HT receiver hands over ([`PpduView`]). [`HtDecoder::new`] takes one MPDU of up to 4095 octets;
//! [`HtDecoder::with_max_psdu`] also takes an A-MPDU ([`HtDecoder::decode_psdu`], de-aggregated with [`crate::mac::Ampdu`]).
//!
//! The non-HT parts (L-STF, L-LTF, L-SIG coding, scrambler, `WIFI_K7`, the constellations) are those of
//! [`crate::ofdm`]. HT-SIG is two symbols of BPSK on the Q axis; HT-STF and HT-LTF follow; the data symbols carry
//! 52 data subcarriers (-28 to 28 without 0 and the pilots at -21, -7, 7, 21). Not decoded: MCS 8 and up (two or more spatial
//! streams), 40 MHz, LDPC, STBC, Ness other than 0, the greenfield format.
//!
//! Standard: IEEE Std 802.11-2020 clause 19.

use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_fec::conv::{codes, depuncture_frame, puncture, Encoder, Ends, Puncture, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::constellation::{Constellation, Demap};
use sdrkit_algo_modem::ofdm::ieee80211_pilot_polarity;
use crate::ofdm::{
    self, bin, encode_signal, ltf_time, map_symbol, scrambler_after, scrambler_seed, stf_bins,
    Modulation, PpduView, Rate, Signal, NFFT, SERVICE_BITS, TAIL_BITS,
};
use crate::Complex32;
use sdrkit_util_bits::crc::catalog::CRC8_CDR;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::{pack, seq, BitOrder};
use std::f64::consts::TAU;

/// Data subcarriers of an HT symbol.
pub const N_SD: usize = 52;
/// Interleaver columns of the HT PHY at 20 MHz.
pub const N_COL: usize = 13;
/// The longest PSDU [`HtDecoder::new`] takes, octets.
pub const MAX_PSDU: usize = 4095;
/// The longest PSDU HT-SIG can announce, octets.
pub const MAX_AMPDU: usize = 65535;
/// Samples from the start of the first L-LTF symbol to the first sample of the HT data field.
pub const DATA_START: usize = 128 + 80 + 160 + 80 + 80;
/// Samples into the cyclic prefix where each FFT window starts.
pub(crate) const BACKOFF: usize = 3;
/// The pilot subcarriers.
pub const PILOT_K: [i32; 4] = [-21, -7, 7, 21];
/// The base values of the pilots.
pub const PILOT_V: [f32; 4] = [1.0, 1.0, 1.0, -1.0];

/// The data subcarriers in the order the mapper fills them: -28 to 28 without 0 and the pilots.
pub const fn data_carriers() -> [i32; N_SD] {
    let mut v = [0i32; N_SD];
    let mut i = 0;
    let mut k = -28i32;
    while k <= 28 {
        if k != 0 && k != 7 && k != -7 && k != 21 && k != -21 {
            v[i] = k;
            i += 1;
        }
        k += 1;
    }
    v
}

/// The 256-QAM point set of the VHT PHY with the 802.11 bit labels (clause 21.3.10.9).
pub fn qam256() -> sdrkit_algo_dsp::constellation::PointSet {
    sdrkit_algo_dsp::constellation::PointSet::qam(256, sdrkit_algo_dsp::constellation::QamMap::IEEE_802_11)
        .unwrap_or_else(|_| unreachable!("256-QAM is a valid point set"))
}

/// How the four pilot values move from one data symbol to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PilotPattern {
    /// The values 1, 1, 1, -1 on -21, -7, 7, 21 in every symbol.
    Fixed,
    /// The values move one pilot further each symbol: pilot `m` of symbol `n` has value `m + n` (mod 4) of 1, 1, 1, -1.
    Rotating,
}

/// The value of pilot `m` (0 to 3) in data symbol `n` with polarity `p`.
pub fn pilot(pattern: PilotPattern, m: usize, n: usize, p: i8) -> f32 {
    let i = match pattern {
        PilotPattern::Fixed => m,
        PilotPattern::Rotating => (m + n) % 4,
    };
    PILOT_V[i] * f32::from(p)
}

/// The modulation and coding of an MCS 0 to 7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mcs(pub u8);

impl Mcs {
    /// The modulation.
    pub fn modulation(self) -> Modulation {
        match self.0 {
            0 => Modulation::Bpsk,
            1 | 2 => Modulation::Qpsk,
            3 | 4 => Modulation::Qam16,
            _ => Modulation::Qam64,
        }
    }
    /// Coded bits per subcarrier.
    pub fn n_bpsc(self) -> usize {
        self.modulation().bits()
    }
    /// Coded bits per symbol.
    pub fn n_cbps(self) -> usize {
        N_SD * self.n_bpsc()
    }
    /// The coding rate as (numerator, denominator).
    pub fn coding(self) -> (usize, usize) {
        match self.0 {
            0 | 1 | 3 => (1, 2),
            5 => (2, 3),
            7 => (5, 6),
            _ => (3, 4),
        }
    }
    /// Data bits per symbol.
    pub fn n_dbps(self) -> usize {
        let (a, b) = self.coding();
        self.n_cbps() * a / b
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
    /// Data symbols of a PSDU of `length` octets.
    pub fn n_sym(self, length: usize) -> usize {
        (8 * length + SERVICE_BITS + TAIL_BITS).div_ceil(self.n_dbps())
    }
    /// The interleaver of one symbol.
    pub fn interleaver(self) -> Permutation {
        Permutation::wifi(self.n_cbps(), self.n_bpsc(), N_COL)
            .unwrap_or_else(|_| unreachable!("52 N_BPSC is a multiple of 13"))
    }
}

/// The HT-SIG field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HtSig {
    /// The MCS, 0 to 127.
    pub mcs: u8,
    /// 40 MHz.
    pub cbw40: bool,
    /// The PSDU length, octets.
    pub length: u16,
    /// Smoothing is allowed.
    pub smoothing: bool,
    /// Not a sounding PPDU.
    pub not_sounding: bool,
    /// The PSDU is an A-MPDU.
    pub aggregation: bool,
    /// Space-time streams over spatial streams, 0 to 3.
    pub stbc: u8,
    /// LDPC coding.
    pub ldpc: bool,
    /// The short guard interval.
    pub short_gi: bool,
    /// Extension spatial streams, 0 to 3.
    pub ness: u8,
}

/// Why an HT-SIG field cannot be read here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HtSigError {
    /// The CRC is wrong.
    Crc,
    /// The tail bits are not zero.
    Tail,
}

impl HtSig {
    /// The first 34 bits in air order, each field least significant bit first (the reserved bit is 1).
    fn head(&self) -> [u8; 34] {
        let mut b = [0u8; 34];
        let mut i = 0;
        let mut put = |v: u32, n: usize| {
            for k in 0..n {
                b[i] = ((v >> k) & 1) as u8;
                i += 1;
            }
        };
        put(u32::from(self.mcs), 7);
        put(u32::from(self.cbw40), 1);
        put(u32::from(self.length), 16);
        put(u32::from(self.smoothing), 1);
        put(u32::from(self.not_sounding), 1);
        put(1, 1);
        put(u32::from(self.aggregation), 1);
        put(u32::from(self.stbc), 2);
        put(u32::from(self.ldpc), 1);
        put(u32::from(self.short_gi), 1);
        put(u32::from(self.ness), 2);
        b
    }

    /// The CRC of the first 34 bits: CRC-8 with polynomial 07, register preset to FF, complemented.
    pub fn crc(&self) -> u8 {
        CRC8_CDR.bits(&self.head())
    }

    /// The 48 bits in air order: the fields, the CRC bit 7 first, six tail bits.
    pub fn bits(&self) -> [u8; 48] {
        let mut b = [0u8; 48];
        b[..34].copy_from_slice(&self.head());
        let c = self.crc();
        for k in 0..8 {
            b[34 + k] = (c >> (7 - k)) & 1;
        }
        b
    }

    /// Reads the 48 bits. Errors for a wrong CRC or a tail that is not zero.
    pub fn from_bits(b: &[u8; 48]) -> Result<HtSig, HtSigError> {
        if b[42..].iter().any(|&x| x & 1 != 0) {
            return Err(HtSigError::Tail);
        }
        let field = |from: usize, n: usize| -> u32 {
            (0..n).fold(0, |v, k| v | u32::from(b[from + k] & 1) << k)
        };
        let s = HtSig {
            mcs: field(0, 7) as u8,
            cbw40: field(7, 1) == 1,
            length: field(8, 16) as u16,
            smoothing: field(24, 1) == 1,
            not_sounding: field(25, 1) == 1,
            aggregation: field(27, 1) == 1,
            stbc: field(28, 2) as u8,
            ldpc: field(30, 1) == 1,
            short_gi: field(31, 1) == 1,
            ness: field(32, 2) as u8,
        };
        let crc = (0..8).fold(0u8, |v, k| v << 1 | (b[34 + k] & 1));
        if CRC8_CDR.bits(&b[..34]) != crc {
            return Err(HtSigError::Crc);
        }
        Ok(s)
    }

    /// The reason this decoder does not take the PPDU, or `None` when it does: MCS 8 and up, 40 MHz, aggregation, LDPC, STBC,
    /// Ness other than 0, a length under 14 or over 4095.
    pub fn unsupported(&self) -> Option<&'static str> {
        if self.mcs > 7 {
            Some("MCS 8 and up")
        } else if self.cbw40 {
            Some("40 MHz")
        } else if self.aggregation {
            Some("aggregation")
        } else if self.ldpc {
            Some("LDPC")
        } else if self.stbc != 0 {
            Some("STBC")
        } else if self.ness != 0 {
            Some("extension streams")
        } else if self.length < 14 {
            Some("length under 14")
        } else if usize::from(self.length) > MAX_PSDU {
            Some("length over 4095")
        } else {
            None
        }
    }

    /// The reason a decoder for PSDUs of up to `max_psdu` octets that takes A-MPDUs does not take the PPDU, or `None`.
    pub fn undecodable(&self, max_psdu: usize) -> Option<&'static str> {
        if self.mcs > 76 {
            Some("reserved MCS")
        } else if self.mcs > 7 {
            Some("more than one spatial stream")
        } else if self.cbw40 {
            Some("40 MHz")
        } else if self.ldpc {
            Some("LDPC")
        } else if self.stbc != 0 {
            Some("STBC")
        } else if self.ness != 0 {
            Some("extension streams")
        } else if usize::from(self.length) > max_psdu {
            Some("PSDU too long")
        } else if self.length == 0 {
            Some("no data")
        } else {
            None
        }
    }

    /// Spatial streams of the MCS (1 to 4), 0 for MCS 32 and reserved values. MCS 0 to 31 are equal modulation, 33 to 76 unequal.
    pub fn streams(&self) -> u8 {
        match self.mcs {
            0..=31 => self.mcs / 8 + 1,
            33..=38 => 2,
            39..=52 => 3,
            53..=76 => 4,
            _ => 0,
        }
    }

    /// The data rate of a one-stream MCS 0 to 7 at 20 MHz, Mbit/s; 0 for another MCS.
    pub fn rate_mbps(&self) -> f64 {
        if self.mcs > 7 {
            return 0.0;
        }
        let us = if self.short_gi { 3.6 } else { 4.0 };
        Mcs(self.mcs).n_dbps() as f64 / us
    }

    /// Samples of one data symbol at 20 Msps.
    pub fn symbol_len(&self) -> usize {
        if self.short_gi {
            72
        } else {
            80
        }
    }
}

/// The L-SIG LENGTH of an HT-mixed PPDU of `n_sym` data symbols: `3 × ceil((TXTIME - 20 µs) / 4 µs) - 3`, TXTIME the
/// preamble (36 µs) and the data symbols (3.6 or 4 µs).
pub fn l_length(n_sym: usize, short_gi: bool) -> u16 {
    let txtime = 200 + 80 + 40 + 40 + n_sym * if short_gi { 36 } else { 40 };
    (3 * (txtime - 200).div_ceil(40) - 3) as u16
}

/// The coded and interleaved bits of the HT data field of `psdu` (FCS included) at `mcs` with scrambler seed `seed`:
/// SERVICE, the PSDU least significant bit first, the tail and the pad, scrambled, the tail set to zero after scrambling,
/// encoded, punctured and interleaved symbol by symbol, appended to `out`.
pub fn encode_data(psdu: &[u8], mcs: Mcs, seed: u8, out: &mut Vec<u8>) {
    let n_sym = mcs.n_sym(psdu.len());
    let n_data = n_sym * mcs.n_dbps();
    let mut bits = vec![0u8; SERVICE_BITS];
    pack::unpack(psdu, BitOrder::Lsb, &mut bits);
    bits.resize(n_data, 0);
    let mut s = seq::ieee80211(seed.max(1));
    for b in bits.iter_mut() {
        *b ^= s.bit();
    }
    let tail = SERVICE_BITS + 8 * psdu.len();
    bits[tail..tail + TAIL_BITS].fill(0);
    let mut coded = Vec::with_capacity(2 * n_data);
    Encoder::new(codes::WIFI_K7).encode(&bits, &mut coded);
    let mut sent = Vec::with_capacity(n_sym * mcs.n_cbps());
    puncture(&mcs.puncture(), 0, &coded, &mut sent);
    let perm = mcs.interleaver();
    let start = out.len();
    out.resize(start + sent.len(), 0);
    for (src, dst) in sent
        .chunks_exact(mcs.n_cbps())
        .zip(out[start..].chunks_exact_mut(mcs.n_cbps()))
    {
        let _ = perm.apply(src, dst);
    }
}

/// The 96 coded bits of HT-SIG: the 48 bits encoded at rate 1/2, each half interleaved as a 6 Mbit/s symbol.
pub fn encode_ht_sig(sig: &HtSig) -> [u8; 96] {
    let mut coded = Vec::with_capacity(96);
    Encoder::new(codes::WIFI_K7).encode(&sig.bits(), &mut coded);
    let perm = Rate::R6.interleaver();
    let mut out = [0u8; 96];
    let (a, b) = out.split_at_mut(48);
    let _ = perm.apply(&coded[..48], a);
    let _ = perm.apply(&coded[48..], b);
    out
}

/// One OFDM symbol from its 64 bins: the time samples with a cyclic prefix of `gi`, scaled by `scale`.
pub(crate) fn symbol(fft: &mut Fft, mut x: [Complex32; NFFT], gi: usize, scale: f32, out: &mut Vec<Complex32>) {
    fft.process(&mut x);
    out.extend(x[NFFT - gi..].iter().map(|v| v * scale));
    out.extend(x.iter().map(|v| v * scale));
}

/// The HT-LTF on the 64 bins: the L-LTF values on -26 to 26, +1 on -28 and -27, -1 on 27 and 28.
pub fn ht_ltf_bins() -> [Complex32; NFFT] {
    let mut x = ofdm::ltf_bins();
    for (k, v) in [(-28, 1.0), (-27, 1.0), (27, -1.0), (28, -1.0)] {
        x[bin(k)] = Complex32::new(v, 0.0);
    }
    x
}

/// Builds an HT-mixed PPDU at 20 Msps for `psdu` (FCS included) at `mcs`, with the short guard interval when `short_gi`,
/// scrambler seed `seed` and pilots moving as `pattern`. The non-HT fields have unit mean power on their 52 subcarriers, the HT
/// fields on their 56.
pub fn build_ppdu(
    psdu: &[u8],
    mcs: Mcs,
    short_gi: bool,
    seed: u8,
    pattern: PilotPattern,
    out: &mut Vec<Complex32>,
) {
    let sig = HtSig {
        mcs: mcs.0,
        smoothing: true,
        not_sounding: true,
        short_gi,
        ..HtSig::default()
    };
    build_ppdu_sig(psdu, sig, mcs, seed, pattern, out);
}

/// As [`build_ppdu`] with the HT-SIG field `ht_sig` as given, except its length, which is that of `psdu`. The data field is coded
/// at `mcs` with the guard interval of `ht_sig`: for an A-MPDU set `ht_sig.aggregation`; for a PPDU that a one-stream receiver
/// cannot decode, set the fields of `ht_sig` so (the data symbols then stand in for the real ones).
pub fn build_ppdu_sig(
    psdu: &[u8],
    ht_sig: HtSig,
    mcs: Mcs,
    seed: u8,
    pattern: PilotPattern,
    out: &mut Vec<Complex32>,
) {
    let short_gi = ht_sig.short_gi;
    let mut fft = Fft::inverse(NFFT);
    let l = 1.0 / 52f32.sqrt();
    let h = 1.0 / 56f32.sqrt();
    let pol = ieee80211_pilot_polarity();
    let n_sym = mcs.n_sym(psdu.len());
    // L-STF and L-LTF.
    let mut x = stf_bins();
    fft.process(&mut x);
    out.extend((0..160).map(|i| x[i % NFFT] * l));
    let lt = ltf_time();
    out.extend_from_slice(&lt[32..]);
    out.extend_from_slice(&lt);
    out.extend_from_slice(&lt);
    let non_ht_dc = ofdm::data_carriers();
    let put_pilots = |x: &mut [Complex32; NFFT], p: i8| {
        for (k, v) in ofdm::PILOTS {
            x[bin(k)] = Complex32::new(v * f32::from(p), 0.0);
        }
    };
    // L-SIG: 6 Mbit/s and the length that covers the PPDU.
    let sig = encode_signal(&Signal {
        rate: Rate::R6,
        length: l_length(n_sym, short_gi),
    });
    let mut cells = [Complex32::default(); 48];
    let _ = map_symbol(&sig, Modulation::Bpsk, &mut cells);
    let mut x = [Complex32::default(); NFFT];
    for (&k, &c) in non_ht_dc.iter().zip(&cells) {
        x[bin(k)] = c;
    }
    put_pilots(&mut x, pol[0]);
    symbol(&mut fft, x, 16, l, out);
    // HT-SIG: BPSK on the Q axis.
    let hs = HtSig {
        length: psdu.len() as u16,
        ..ht_sig
    };
    let coded = encode_ht_sig(&hs);
    for (s, half) in coded.as_chunks::<48>().0.iter().enumerate() {
        let _ = map_symbol(half, Modulation::Bpsk, &mut cells);
        let mut x = [Complex32::default(); NFFT];
        for (&k, &c) in non_ht_dc.iter().zip(&cells) {
            x[bin(k)] = Complex32::new(0.0, c.re);
        }
        put_pilots(&mut x, pol[1 + s]);
        symbol(&mut fft, x, 16, l, out);
    }
    // HT-STF (4 µs of the short training symbol) and HT-LTF.
    let mut x = stf_bins();
    fft.process(&mut x);
    out.extend((0..80).map(|i| x[i % NFFT] * h));
    symbol(&mut fft, ht_ltf_bins(), 16, h, out);
    // Data.
    let gi = if short_gi { 8 } else { 16 };
    let dc = data_carriers();
    let mut bits = Vec::with_capacity(n_sym * mcs.n_cbps());
    encode_data(psdu, mcs, seed, &mut bits);
    let pts = mcs.modulation().points();
    let m = mcs.n_bpsc();
    for (n, sym) in bits.chunks_exact(mcs.n_cbps()).enumerate() {
        let mut x = [Complex32::default(); NFFT];
        for (&k, g) in dc.iter().zip(sym.chunks_exact(m)) {
            let label = g.iter().fold(0usize, |a, &b| (a << 1) | usize::from(b & 1));
            x[bin(k)] = pts.points()[label];
        }
        let p = pol[(n + 3) % 127];
        for (i, &k) in PILOT_K.iter().enumerate() {
            x[bin(k)] = Complex32::new(pilot(pattern, i, n, p), 0.0);
        }
        symbol(&mut fft, x, gi, h, out);
    }
}

/// A one-stream data field with the convolutional code on the 56-tone plan of 20 MHz (HT and VHT).
pub(crate) struct DataField {
    /// The first sample of the data field.
    pub start: usize,
    /// Data symbols.
    pub n_sym: usize,
    /// Samples of a symbol: 80, or 72 with the short guard interval.
    pub sym: usize,
    /// Coded bits per subcarrier: 1, 2, 4, 6 or 8.
    pub n_bpsc: usize,
    /// The puncturing pattern.
    pub puncture: Puncture<'static>,
    /// The index of the first data symbol in the pilot polarity sequence: 3 for HT, 4 for VHT.
    pub pol0: usize,
    /// VHT: the pilots rotate, and VHT-SIG-B lies between the long training symbol and the data.
    pub rotating_only: bool,
    /// The scrambled bits to decode: SERVICE, the PSDU and the tail (HT), or every bit of the symbols (VHT).
    pub steps: usize,
}

/// Why the data field gave no PSDU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DataFail {
    /// The samples end before the data field does.
    Short,
    /// The samples do not hold the symbol.
    NoSignal,
    /// The demapper or the decoder refused the symbols.
    Decode,
}

/// What the decoder made of a PPDU.
#[derive(Clone, Debug, PartialEq)]
pub enum HtResult<'a> {
    /// Not an HT-mixed PPDU (no energy on the Q axis in the first HT-SIG symbol, or a bad HT-SIG).
    NotHt,
    /// An HT-mixed PPDU that this decoder does not take, with the reason.
    NotDecodable(&'static str),
    /// The FCS of the PSDU is wrong.
    BadFcs(HtSig),
    /// A PSDU with a good FCS.
    Frame(HtFrame<'a>),
}

/// What [`HtDecoder::decode_psdu`] made of a PPDU.
#[derive(Clone, Debug, PartialEq)]
pub enum HtPsdu<'a> {
    /// Not an HT-mixed PPDU.
    NotHt,
    /// An HT-mixed PPDU whose HT-SIG passed and whose data this decoder does not take, with the reason.
    NotDecodable(HtSig, &'static str),
    /// The decoded PSDU of HT-SIG's length. Nothing is checked: an MPDU with its FCS, or an A-MPDU when `sig.aggregation`.
    Psdu {
        /// The octets.
        psdu: &'a [u8],
        /// The HT-SIG field.
        sig: HtSig,
        /// The scrambler seed.
        seed: u8,
        /// The signal-to-noise ratio from the decisions of the data cells, dB.
        snr_db: f64,
    },
}

/// A PSDU with a good FCS.
#[derive(Clone, Debug, PartialEq)]
pub struct HtFrame<'a> {
    /// The MPDU without its FCS.
    pub mpdu: &'a [u8],
    /// The HT-SIG field.
    pub sig: HtSig,
    /// The scrambler seed.
    pub seed: u8,
    /// The pilot pattern that fitted the data symbols better.
    pub pattern: PilotPattern,
    /// The signal-to-noise ratio from the decisions of the data cells, dB.
    pub snr_db: f64,
}

/// The decoder of the HT part of a PPDU. Sized once for the longest PSDU, then allocates nothing.
pub struct HtDecoder {
    max_psdu: usize,
    fft: Fft,
    pol: [i8; 127],
    non_ht_dc: [i32; 48],
    dc: [i32; N_SD],
    sig_viterbi: Viterbi,
    viterbi: Viterbi,
    bpsk: Constellation,
    consts: Vec<Constellation>,
    /// The interleavers of 1, 2, 4, 6 and 8 coded bits per subcarrier.
    perms: Vec<Permutation>,
    service_high: u8,
    r6: Permutation,
    llr: Vec<f32>,
    scratch: Vec<f32>,
    sent: Vec<i8>,
    llr8: Vec<i8>,
    bits: Vec<u8>,
    psdu: Vec<u8>,
    h: [Complex32; NFFT],
    weights: [f32; N_SD],
}

impl Default for HtDecoder {
    fn default() -> Self {
        HtDecoder::new()
    }
}

impl HtDecoder {
    /// A decoder for one MPDU of up to 4095 octets.
    pub fn new() -> HtDecoder {
        HtDecoder::with_max_psdu(MAX_PSDU)
    }

    /// A decoder for PSDUs of up to `max_psdu` octets (at most 65535), for [`decode_psdu`](Self::decode_psdu).
    pub fn with_max_psdu(max_psdu: usize) -> HtDecoder {
        let max_psdu = max_psdu.clamp(1, MAX_AMPDU);
        let steps = SERVICE_BITS + 8 * max_psdu + TAIL_BITS + 260;
        let c = |m: Modulation| {
            Constellation::new(m.points()).unwrap_or_else(|_| unreachable!("valid constellations"))
        };
        HtDecoder {
            max_psdu,
            fft: Fft::forward(NFFT),
            pol: ieee80211_pilot_polarity(),
            non_ht_dc: ofdm::data_carriers(),
            dc: data_carriers(),
            sig_viterbi: Viterbi::new(codes::WIFI_K7, 48)
                .unwrap_or_else(|_| unreachable!("WIFI_K7 is a valid code")),
            viterbi: Viterbi::new(codes::WIFI_K7, steps)
                .unwrap_or_else(|_| unreachable!("WIFI_K7 is a valid code")),
            bpsk: c(Modulation::Bpsk),
            consts: [Modulation::Bpsk, Modulation::Qpsk, Modulation::Qam16, Modulation::Qam64]
                .into_iter()
                .map(c)
                .chain(std::iter::once(
                    Constellation::new(qam256()).unwrap_or_else(|_| unreachable!("valid constellations")),
                ))
                .collect(),
            perms: [1, 2, 4, 6, 8]
                .into_iter()
                .map(|m| {
                    Permutation::wifi(N_SD * m, m, N_COL)
                        .unwrap_or_else(|_| unreachable!("52 N_BPSC is a multiple of 13"))
                })
                .collect(),
            service_high: 0,
            r6: Rate::R6.interleaver(),
            llr: Vec::with_capacity(2 * steps),
            scratch: vec![0.0; 8 * N_SD],
            sent: Vec::with_capacity(2 * steps),
            llr8: Vec::with_capacity(2 * steps),
            bits: Vec::with_capacity(steps),
            psdu: Vec::with_capacity(max_psdu),
            h: [Complex32::default(); NFFT],
            weights: [0.0; N_SD],
        }
    }

    /// The FFT of the 64 samples of `v.samples` from `at`, with the carrier offset of `v` removed (phase 0 at `samples[0]`).
    pub(crate) fn fft_at(&mut self, v: &PpduView<'_>, at: usize) -> Option<[Complex32; NFFT]> {
        let s = v.samples.get(at..at + NFFT)?;
        let w = -TAU * v.offset_hz / ofdm::SAMPLE_RATE;
        let mut x = [Complex32::default(); NFFT];
        let (s1, c1) = approx::sin_cos(w);
        let step = Complex32::new(c1, s1);
        let (s0, c0) = approx::sin_cos(w * at as f64);
        let mut rot = Complex32::new(c0, s0);
        for (o, &y) in x.iter_mut().zip(s) {
            *o = y * rot;
            rot *= step;
        }
        self.fft.process(&mut x);
        Some(x)
    }

    /// The common phase and phase slope of a symbol from its four pilots `p` (already divided by the channel and the
    /// expected values), applied to the cells of carriers `ks`.
    fn derotate(pilots: &[Complex32; 4], y: &[Complex32; NFFT], h: &[Complex32; NFFT], ks: &[i32], out: &mut [Complex32]) {
        let acc: Complex32 = pilots.iter().sum();
        let phi0 = approx::atan2(acc.im, acc.re);
        let (mut num, mut den) = (0f32, 0f32);
        for (&k, &p) in PILOT_K.iter().zip(pilots) {
            let (s, c) = approx::sin_cos(-f64::from(phi0));
            let e = p * Complex32::new(c, s);
            num += k as f32 * approx::atan2(e.im, e.re);
            den += (k * k) as f32;
        }
        let slope = num / den;
        for (o, &k) in out.iter_mut().zip(ks) {
            let hk = h[bin(k)];
            let ph = -(phi0 + slope * k as f32);
            let (s, c) = approx::sin_cos(f64::from(ph));
            *o = if hk.norm_sqr() > 1e-20 {
                y[bin(k)] / hk * Complex32::new(c, s)
            } else {
                Complex32::default()
            };
        }
    }

    /// Reads the HT part of PPDU `v`: one MPDU with a good FCS.
    pub fn decode(&mut self, v: &PpduView<'_>) -> HtResult<'_> {
        let Some(sig) = self.ht_sig(v) else {
            return HtResult::NotHt;
        };
        if let Some(why) = sig.unsupported() {
            return HtResult::NotDecodable(why);
        }
        match self.data(v, sig) {
            Err(DataFail::Short) => HtResult::NotDecodable("PPDU longer than its L-SIG"),
            Err(DataFail::NoSignal) => HtResult::NotHt,
            Err(DataFail::Decode) => HtResult::BadFcs(sig),
            Ok((seed, pattern, snr_db)) if crate::mac::fcs_ok(&self.psdu) => HtResult::Frame(HtFrame {
                mpdu: &self.psdu[..self.psdu.len() - 4],
                sig,
                seed,
                pattern,
                snr_db,
            }),
            Ok(_) => HtResult::BadFcs(sig),
        }
    }

    /// Reads the HT part of PPDU `v` and returns the PSDU as decoded, an A-MPDU included.
    pub fn decode_psdu(&mut self, v: &PpduView<'_>) -> HtPsdu<'_> {
        let Some(sig) = self.ht_sig(v) else {
            return HtPsdu::NotHt;
        };
        if let Some(why) = sig.undecodable(self.max_psdu) {
            return HtPsdu::NotDecodable(sig, why);
        }
        match self.data(v, sig) {
            Err(DataFail::Short) => HtPsdu::NotDecodable(sig, "PPDU longer than its L-SIG"),
            Err(DataFail::NoSignal) => HtPsdu::NotHt,
            Err(DataFail::Decode) => HtPsdu::NotDecodable(sig, "data not decoded"),
            Ok((seed, _, snr_db)) => HtPsdu::Psdu {
                psdu: &self.psdu,
                sig,
                seed,
                snr_db,
            },
        }
    }

    /// The HT-SIG field of `v`, `None` when the PPDU is not HT-mixed or the field fails its CRC.
    pub fn ht_sig(&mut self, v: &PpduView<'_>) -> Option<HtSig> {
        // HT-SIG: two symbols after L-SIG, BPSK on the Q axis, the channel of the L-LTF.
        let lh = *v.channel;
        let mut cells = [Complex32::default(); 48];
        let mut sig_llr = [0f32; 96];
        for s in 0..2 {
            let y = self.fft_at(v, 128 + 80 * (1 + s) + 16 - BACKOFF)?;
            let pol = f32::from(self.pol[1 + s]);
            let pil: [Complex32; 4] = std::array::from_fn(|i| {
                let (k, val) = ofdm::PILOTS[i];
                let hk = lh[bin(k)];
                if hk.norm_sqr() > 1e-20 {
                    y[bin(k)] / hk * (val * pol)
                } else {
                    Complex32::default()
                }
            });
            Self::derotate(&pil, &y, &lh, &self.non_ht_dc, &mut cells);
            if s == 0 {
                let q: f32 = cells.iter().map(|c| c.im * c.im).sum();
                let i: f32 = cells.iter().map(|c| c.re * c.re).sum();
                if q < 1.3 * i {
                    return None;
                }
            }
            // The Q axis turned to the I axis: multiply by -j.
            for c in cells.iter_mut() {
                *c = Complex32::new(c.im, -c.re);
            }
            let w = [1f32; 48];
            if self
                .bpsk
                .demap_block_weighted(&cells, &w, Demap::MaxLog, &mut self.scratch[..48])
                .is_err()
                || self.r6.invert(&self.scratch[..48], &mut sig_llr[48 * s..48 * (s + 1)]).is_err()
            {
                return None;
            }
        }
        let mut q = [0i8; 96];
        if Quantizer::default().quantize_auto(&sig_llr, &mut q).is_err() {
            return None;
        }
        let mut b = [0u8; 48];
        if self.sig_viterbi.decode_only(&q, Ends::Zero, &mut b).is_err() {
            return None;
        }
        HtSig::from_bits(&b).ok()
    }

    /// Decodes the HT data field into `self.psdu`: the scrambler seed, the pilot pattern and the SNR of the data cells.
    fn data(&mut self, v: &PpduView<'_>, sig: HtSig) -> Result<(u8, PilotPattern, f64), DataFail> {
        let mcs = Mcs(sig.mcs);
        let length = usize::from(sig.length);
        self.data_field(
            v,
            &DataField {
                start: DATA_START,
                n_sym: mcs.n_sym(length),
                sym: sig.symbol_len(),
                n_bpsc: mcs.n_bpsc(),
                puncture: mcs.puncture(),
                pol0: 3,
                rotating_only: false,
                steps: SERVICE_BITS + 8 * length + TAIL_BITS,
            },
        )
    }

    /// Sets the channel from the HT-LTF or VHT-LTF symbol whose guard interval starts at `ltf_at`.
    fn estimate(&mut self, v: &PpduView<'_>, ltf_at: usize) -> Result<(), DataFail> {
        let lt = self.fft_at(v, ltf_at + 16 - BACKOFF).ok_or(DataFail::NoSignal)?;
        let known = ht_ltf_bins();
        let mut mean = 0f32;
        for k in -28i32..=28 {
            let b = bin(k);
            self.h[b] = if k == 0 { Complex32::default() } else { lt[b] * known[b].re };
            mean += self.h[b].norm_sqr();
        }
        let mean = (mean / 56.0).max(1e-20);
        for (w, &k) in self.weights.iter_mut().zip(&self.dc) {
            *w = self.h[bin(k)].norm_sqr() / mean;
        }
        Ok(())
    }

    /// The 52 equalised data cells of the symbol whose FFT window starts at `at`, with polarity `pol`, as symbol `n` of the
    /// pilot patterns. Returns the cells and how well the fixed and the rotating pattern fit the pilots.
    fn cells(
        &mut self,
        v: &PpduView<'_>,
        at: usize,
        pol: i8,
        n: usize,
        rotating_only: bool,
    ) -> Result<([Complex32; N_SD], f32, f32), DataFail> {
        let y = self.fft_at(v, at).ok_or(DataFail::NoSignal)?;
        let raw: [Complex32; 4] = std::array::from_fn(|j| {
            let hk = self.h[bin(PILOT_K[j])];
            if hk.norm_sqr() > 1e-20 {
                y[bin(PILOT_K[j])] / hk
            } else {
                Complex32::default()
            }
        });
        let fit = |p: PilotPattern| -> ([Complex32; 4], f32) {
            let a: [Complex32; 4] = std::array::from_fn(|j| raw[j] * pilot(p, j, n, pol));
            let s: Complex32 = a.iter().sum();
            (a, s.norm())
        };
        let (ra, rs) = fit(PilotPattern::Rotating);
        let (fa, fs) = if rotating_only { (ra, 0.0) } else { fit(PilotPattern::Fixed) };
        let pil = if fs >= rs && !rotating_only { fa } else { ra };
        let mut cells = [Complex32::default(); N_SD];
        Self::derotate(&pil, &y, &self.h, &self.dc, &mut cells);
        Ok((cells, fs, rs))
    }

    /// Decodes a data field on the 56-tone plan into `self.psdu` (`(steps - 22) / 8` octets): the scrambler seed, the pilot
    /// pattern and the SNR of the data cells. The channel comes from the long training symbol that ends at `p.start` (HT) or one
    /// symbol before it (VHT, where VHT-SIG-B lies between).
    pub(crate) fn data_field(&mut self, v: &PpduView<'_>, p: &DataField) -> Result<(u8, PilotPattern, f64), DataFail> {
        let gi = p.sym - NFFT;
        if v.samples.len() < p.start + p.n_sym * p.sym {
            return Err(DataFail::Short);
        }
        let ltf_at = if p.rotating_only { p.start - 160 } else { p.start - 80 };
        self.estimate(v, ltf_at)?;
        let ci = match p.n_bpsc {
            1 => 0,
            2 => 1,
            4 => 2,
            6 => 3,
            _ => 4,
        };
        let n = N_SD * p.n_bpsc;
        self.llr.clear();
        let (mut fixed, mut rotating) = (0f32, 0f32);
        let (mut err, mut pow) = (0f64, 0f64);
        for i in 0..p.n_sym {
            let at = p.start + i * p.sym + gi - BACKOFF.min(gi);
            let pol = self.pol[(i + p.pol0) % 127];
            let (cells, fs, rs) = self.cells(v, at, pol, i, p.rotating_only)?;
            fixed += fs;
            rotating += rs;
            let c = &self.consts[ci];
            for z in &cells {
                let d = c.nearest(*z);
                err += f64::from((z - d).norm_sqr());
                pow += f64::from(d.norm_sqr());
            }
            let start = self.llr.len();
            c.demap_block_weighted(&cells, &self.weights, Demap::MaxLog, &mut self.scratch[..n])
                .map_err(|_| DataFail::Decode)?;
            self.llr.resize(start + n, 0.0);
            self.perms[ci]
                .invert(&self.scratch[..n], &mut self.llr[start..])
                .map_err(|_| DataFail::Decode)?;
        }
        let pattern = if fixed >= rotating && !p.rotating_only {
            PilotPattern::Fixed
        } else {
            PilotPattern::Rotating
        };
        let snr_db = 10.0 * (pow / err.max(1e-20)).log10();
        match self.decode_bits(p.puncture, p.steps) {
            Some(seed) => Ok((seed, pattern, snr_db)),
            None => Err(DataFail::Decode),
        }
    }

    /// The 26 bits of the VHT-SIG-B symbol whose guard interval starts at `at`, with the channel of the VHT-LTF before it.
    pub(crate) fn vht_sig_b(&mut self, v: &PpduView<'_>, at: usize) -> Option<[u8; 26]> {
        self.estimate(v, at - 80).ok()?;
        let pol = self.pol[3];
        let (cells, _, _) = self.cells(v, at + 16 - BACKOFF, pol, 0, true).ok()?;
        let mut llr = [0f32; 52];
        self.consts[0]
            .demap_block_weighted(&cells, &self.weights, Demap::MaxLog, &mut self.scratch[..52])
            .ok()?;
        self.perms[0].invert(&self.scratch[..52], &mut llr).ok()?;
        let mut q = [0i8; 52];
        Quantizer::default().quantize_auto(&llr, &mut q).ok()?;
        let mut b = [0u8; 26];
        self.sig_viterbi.decode_only(&q, Ends::Zero, &mut b).ok()?;
        Some(b)
    }

    /// The service bits 8 to 15 of the last decoded data field, descrambled (the VHT-SIG-B CRC of a VHT PPDU).
    pub(crate) fn service_high(&self) -> u8 {
        self.service_high
    }

    /// The last decoded PSDU.
    pub(crate) fn psdu(&self) -> &[u8] {
        &self.psdu
    }

    /// Depunctures, decodes and descrambles the LLRs of the data symbols into the PSDU of `(steps - 22) / 8` octets, where the
    /// tail is the last six of the `steps` bits; returns the scrambler seed.
    fn decode_bits(&mut self, p: Puncture<'static>, steps: usize) -> Option<u8> {
        let length = steps.checked_sub(SERVICE_BITS + TAIL_BITS)? / 8;
        if length > self.max_psdu + 1 {
            return None;
        }
        let sent = p.punctured_len(0, 2 * steps);
        if self.llr.len() < sent {
            return None;
        }
        self.llr8.clear();
        self.llr8.resize(2 * steps, 0);
        if sent == 2 * steps {
            Quantizer::default()
                .quantize_auto(&self.llr[..sent], &mut self.llr8)
                .ok()?;
        } else {
            self.sent.clear();
            self.sent.resize(sent, 0);
            Quantizer::default()
                .quantize_auto(&self.llr[..sent], &mut self.sent)
                .ok()?;
            depuncture_frame(&p, 0, &self.sent, &mut self.llr8).ok()?;
        }
        self.bits.clear();
        self.bits.resize(steps - TAIL_BITS, 0);
        self.viterbi
            .decode_only(&self.llr8, Ends::Zero, &mut self.bits)
            .ok()?;
        let seed = scrambler_seed(&self.bits[..7]).ok()?;
        let mut s = scrambler_after(&self.bits[..7]).ok()?;
        s.skip(1);
        self.service_high = 0;
        for k in 0..8 {
            self.service_high |= ((self.bits[8 + k] ^ s.bit()) & 1) << k;
        }
        self.psdu.clear();
        pack::pack(
            &self.bits[SERVICE_BITS..SERVICE_BITS + 8 * length],
            BitOrder::Lsb,
            sdrkit_util_bits::Pad::Zero,
            &mut self.psdu,
        );
        for b in self.psdu.iter_mut() {
            *b ^= s.byte(BitOrder::Lsb);
        }
        Some(seed)
    }
}

impl std::fmt::Debug for HtDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HtDecoder").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ht_sig_crc_has_its_worked_value() {
        let s = HtSig {
            mcs: 1,
            length: 300,
            smoothing: true,
            not_sounding: true,
            ..HtSig::default()
        };
        assert_eq!(s.crc(), 0x39);
        assert_eq!(HtSig::from_bits(&s.bits()), Ok(s));
        let mut b = s.bits();
        b[10] ^= 1;
        assert_eq!(HtSig::from_bits(&b), Err(HtSigError::Crc));
    }

    #[test]
    fn the_rates_of_the_mcs_table() {
        let dbps: Vec<usize> = (0..8).map(|m| Mcs(m).n_dbps()).collect();
        assert_eq!(dbps, [26, 52, 78, 104, 156, 208, 234, 260]);
        assert_eq!(l_length(10, false), 3 * (160 + 400usize).div_ceil(40) as u16 - 3);
    }
}
