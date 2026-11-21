//! The HT-mixed PHY of 802.11n for one spatial stream at 20 MHz with the convolutional code, MCS 0 to 7: the HT-SIG field with
//! its CRC, the HT data field in both directions, a PPDU builder for test signals and the decoder of the HT part of a PPDU
//! that the non-HT receiver hands over ([`PpduView`]).
//!
//! The non-HT parts (L-STF, L-LTF, L-SIG coding, scrambler, `WIFI_K7`, the constellations) are those of
//! `sdrkit_proto_wlan_phy::ofdm`. HT-SIG is two symbols of BPSK on the Q axis; HT-STF and HT-LTF follow; the data symbols carry
//! 52 data subcarriers (-28 to 28 without 0 and the pilots at -21, -7, 7, 21). Not decoded: MCS 8 and up, 40 MHz, aggregation,
//! LDPC, STBC, Ness other than 0, the greenfield format.
//!
//! Standard: IEEE Std 802.11-2020 clause 19.

use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_fec::conv::{codes, depuncture_frame, puncture, Encoder, Ends, Puncture, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::constellation::{Constellation, Demap};
use sdrkit_algo_modem::ofdm::ieee80211_pilot_polarity;
use sdrkit_core::Complex32;
use sdrkit_proto_wlan_phy::ofdm::{
    self, bin, encode_signal, ltf_time, map_symbol, scrambler_after, scrambler_seed, stf_bins,
    Modulation, PpduView, Rate, Signal, NFFT, SERVICE_BITS, TAIL_BITS,
};
use sdrkit_util_bits::crc::catalog::CRC8_CDR;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::{pack, seq, BitOrder};
use std::f64::consts::TAU;

/// Data subcarriers of an HT symbol.
pub const N_SD: usize = 52;
/// Interleaver columns of the HT PHY at 20 MHz.
pub const N_COL: usize = 13;
/// The longest PSDU this decoder takes, octets.
pub const MAX_PSDU: usize = 4095;
/// Samples from the start of the first L-LTF symbol to the first sample of the HT data field.
pub const DATA_START: usize = 128 + 80 + 160 + 80 + 80;
/// Samples into the cyclic prefix where each FFT window starts.
const BACKOFF: usize = 3;
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
fn symbol(fft: &mut Fft, mut x: [Complex32; NFFT], gi: usize, scale: f32, out: &mut Vec<Complex32>) {
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
        mcs: mcs.0,
        length: psdu.len() as u16,
        smoothing: true,
        not_sounding: true,
        short_gi,
        ..HtSig::default()
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
    fft: Fft,
    pol: [i8; 127],
    non_ht_dc: [i32; 48],
    dc: [i32; N_SD],
    sig_viterbi: Viterbi,
    viterbi: Viterbi,
    bpsk: Constellation,
    consts: Vec<Constellation>,
    perms: Vec<Option<Permutation>>,
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
    /// A decoder.
    pub fn new() -> HtDecoder {
        let steps = SERVICE_BITS + 8 * MAX_PSDU + TAIL_BITS + 260;
        let c = |m: Modulation| {
            Constellation::new(m.points()).unwrap_or_else(|_| unreachable!("valid constellations"))
        };
        HtDecoder {
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
                .collect(),
            perms: (0..8).map(|m| Some(Mcs(m).interleaver())).collect(),
            r6: Rate::R6.interleaver(),
            llr: Vec::with_capacity(2 * steps),
            scratch: vec![0.0; 6 * N_SD],
            sent: Vec::with_capacity(2 * steps),
            llr8: Vec::with_capacity(2 * steps),
            bits: Vec::with_capacity(steps),
            psdu: Vec::with_capacity(MAX_PSDU),
            h: [Complex32::default(); NFFT],
            weights: [0.0; N_SD],
        }
    }

    /// The FFT of the 64 samples of `v.samples` from `at`, with the carrier offset of `v` removed (phase 0 at `samples[0]`).
    fn fft_at(&mut self, v: &PpduView<'_>, at: usize) -> Option<[Complex32; NFFT]> {
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

    /// Reads the HT part of PPDU `v`.
    pub fn decode(&mut self, v: &PpduView<'_>) -> HtResult<'_> {
        // HT-SIG: two symbols after L-SIG, BPSK on the Q axis, the channel of the L-LTF.
        let lh = *v.channel;
        let mut cells = [Complex32::default(); 48];
        let mut sig_llr = [0f32; 96];
        for s in 0..2 {
            let Some(y) = self.fft_at(v, 128 + 80 * (1 + s) + 16 - BACKOFF) else {
                return HtResult::NotHt;
            };
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
                    return HtResult::NotHt;
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
                return HtResult::NotHt;
            }
        }
        let mut q = [0i8; 96];
        if Quantizer::default().quantize_auto(&sig_llr, &mut q).is_err() {
            return HtResult::NotHt;
        }
        let mut b = [0u8; 48];
        if self.sig_viterbi.decode_only(&q, Ends::Zero, &mut b).is_err() {
            return HtResult::NotHt;
        }
        let Ok(sig) = HtSig::from_bits(&b) else {
            return HtResult::NotHt;
        };
        if let Some(why) = sig.unsupported() {
            return HtResult::NotDecodable(why);
        }
        self.data(v, sig)
    }

    fn data(&mut self, v: &PpduView<'_>, sig: HtSig) -> HtResult<'_> {
        let mcs = Mcs(sig.mcs);
        let length = usize::from(sig.length);
        let n_sym = mcs.n_sym(length);
        let sym = sig.symbol_len();
        let gi = sym - NFFT;
        if v.samples.len() < DATA_START + n_sym * sym {
            return HtResult::NotDecodable("PPDU longer than its L-SIG");
        }
        // The channel of the HT-LTF on the 56 subcarriers.
        let Some(lt) = self.fft_at(v, DATA_START - 80 + 16 - BACKOFF) else {
            return HtResult::NotHt;
        };
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
        let ci = match mcs.n_bpsc() {
            1 => 0,
            2 => 1,
            4 => 2,
            _ => 3,
        };
        let n = mcs.n_cbps();
        self.llr.clear();
        let (mut fixed, mut rotating) = (0f32, 0f32);
        let (mut err, mut pow) = (0f64, 0f64);
        let mut cells = [Complex32::default(); N_SD];
        for i in 0..n_sym {
            let at = DATA_START + i * sym + gi - BACKOFF.min(gi);
            let Some(y) = self.fft_at(v, at) else {
                return HtResult::NotHt;
            };
            let pol = self.pol[(i + 3) % 127];
            // The pilot pattern whose values fit the received pilots better.
            let raw: [Complex32; 4] = std::array::from_fn(|j| {
                let hk = self.h[bin(PILOT_K[j])];
                if hk.norm_sqr() > 1e-20 {
                    y[bin(PILOT_K[j])] / hk
                } else {
                    Complex32::default()
                }
            });
            let fit = |p: PilotPattern| -> ([Complex32; 4], f32) {
                let a: [Complex32; 4] = std::array::from_fn(|j| raw[j] * pilot(p, j, i, pol));
                let s: Complex32 = a.iter().sum();
                (a, s.norm())
            };
            let (fa, fs) = fit(PilotPattern::Fixed);
            let (ra, rs) = fit(PilotPattern::Rotating);
            fixed += fs;
            rotating += rs;
            let pil = if fs >= rs { fa } else { ra };
            Self::derotate(&pil, &y, &self.h, &self.dc, &mut cells);
            let c = &self.consts[ci];
            for z in &cells {
                let d = c.nearest(*z);
                err += f64::from((z - d).norm_sqr());
                pow += f64::from(d.norm_sqr());
            }
            let start = self.llr.len();
            if c
                .demap_block_weighted(&cells, &self.weights, Demap::MaxLog, &mut self.scratch[..n])
                .is_err()
            {
                return HtResult::BadFcs(sig);
            }
            self.llr.resize(start + n, 0.0);
            let Some(Some(perm)) = self.perms.get(usize::from(sig.mcs)) else {
                return HtResult::BadFcs(sig);
            };
            if perm.invert(&self.scratch[..n], &mut self.llr[start..]).is_err() {
                return HtResult::BadFcs(sig);
            }
        }
        let pattern = if fixed >= rotating {
            PilotPattern::Fixed
        } else {
            PilotPattern::Rotating
        };
        let snr_db = 10.0 * (pow / err.max(1e-20)).log10();
        match self.decode_bits(mcs, length) {
            Some(seed) if sdrkit_proto_wlan_phy::mac::fcs_ok(&self.psdu) => HtResult::Frame(HtFrame {
                mpdu: &self.psdu[..length - 4],
                sig,
                seed,
                pattern,
                snr_db,
            }),
            _ => HtResult::BadFcs(sig),
        }
    }

    /// Depunctures, decodes and descrambles the LLRs of the data symbols into the PSDU; returns the scrambler seed.
    fn decode_bits(&mut self, mcs: Mcs, length: usize) -> Option<u8> {
        let steps = SERVICE_BITS + 8 * length + TAIL_BITS;
        let p = mcs.puncture();
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
        s.skip((SERVICE_BITS - 7) as u64);
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
