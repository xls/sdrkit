//! The IEEE 802.11a/g OFDM PHY (IEEE Std 802.11-2020 clause 17, non-HT, 20 MHz).
//!
//! The shared pieces:
//!
//! | Step | Shared code | Clause |
//! |---|---|---|
//! | scrambler x^7 + x^4 + 1 | `sdrkit_util_bits::seq::ieee80211` | 17.3.5.5 |
//! | K = 7 code (133, 171) | `sdrkit_algo_fec::conv::codes::WIFI_K7` | 17.3.5.6 |
//! | puncturing 2/3, 3/4 | `sdrkit_algo_fec::conv::codes::{WIFI_2_3, WIFI_3_4}` | 17.3.5.7 |
//! | interleaver | `sdrkit_util_bits::interleave::Permutation::wifi` | 17.3.5.7 |
//! | BPSK, QPSK, 16-QAM, 64-QAM | `sdrkit_algo_dsp::constellation::QamMap::IEEE_802_11` | 17.3.5.8 |
//! | carriers and pilots | `sdrkit_algo_modem::ofdm::{OfdmLayout::ieee80211a, ieee80211_pilot_polarity, IEEE80211A_STF}` | 17.3.5.10 |
//!
//! This module holds the rate table, the SIGNAL field, the SERVICE field and the scrambler seed, the data-field coding chain in
//! both directions ([`encode_data`], [`DataDecoder`]), the training fields, a PPDU builder ([`build_ppdu`]) and the burst receiver
//! ([`OfdmReceiver`]).
//!
//! ```
//! use sdrkit_proto_wlan_phy::ofdm::{Rate, Signal};
//! let s = Signal { rate: Rate::R36, length: 100 };
//! // The SIGNAL bits of the IEEE 802.11-2020 annex I example: RATE 1011, R 0, LENGTH 100, parity 0, tail.
//! assert_eq!(s.bits(), [1, 0, 1, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
//! assert_eq!(Signal::from_bits(&s.bits()).unwrap(), s);
//! assert_eq!(Rate::R36.n_sym(100), 6);
//! ```

mod rx;

pub use rx::{OfdmFrame, OfdmReceiver, OfdmStats, PpduView};

use crate::{CheckKind, Complex32, Error, Malformation, Param, Result};
use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_fec::conv::{codes, depuncture_frame, puncture, Encoder, Ends, Puncture, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::constellation::{Constellation, Demap};
use sdrkit_algo_modem::ofdm::{ieee80211_pilot_polarity, IEEE80211A_STF};
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::{pack, seq, BitOrder};

/// The sample rate of the 20 MHz numerology, Hz.
pub const SAMPLE_RATE: f64 = 20e6;
/// FFT size.
pub const NFFT: usize = 64;
/// Cyclic prefix of a data symbol, samples (0.8 µs).
pub const GI: usize = 16;
/// Samples of one data symbol with its prefix (4 µs).
pub const SYMBOL_LEN: usize = 80;
/// Samples of the preamble: the short training field (160) and the long training field (160).
pub const PREAMBLE_LEN: usize = 320;
/// The longest PSDU the LENGTH field allows, octets.
pub const MAX_PSDU: usize = 4095;
/// Bits of the SERVICE field.
pub const SERVICE_BITS: usize = 16;
/// Tail bits after the PSDU.
pub const TAIL_BITS: usize = 6;
/// Data subcarriers per symbol.
pub const N_SD: usize = 48;
/// Interleaver columns of the non-HT PHY (equation 17-17).
pub const N_COL: usize = 16;

/// The long training sequence `L(-26..=26)` (equation 17-8).
pub const LTF: [i8; 53] = [
    1, 1, -1, -1, 1, 1, -1, 1, -1, 1, 1, 1, 1, 1, 1, -1, -1, 1, 1, -1, 1, -1, 1, 1, 1, 1, 0, 1, -1,
    -1, 1, 1, -1, 1, -1, 1, -1, -1, -1, -1, -1, 1, 1, -1, -1, 1, -1, 1, -1, 1, 1, 1, 1,
];

/// The pilot subcarriers and their base values (equation 17-24: P(-21, -7, 7, 21) = 1, 1, 1, -1).
pub const PILOTS: [(i32, f32); 4] = [(-21, 1.0), (-7, 1.0), (7, 1.0), (21, -1.0)];

/// The data subcarriers in the order the mapper fills them (equation 17-23): -26 to 26 without 0 and the pilots.
pub const fn data_carriers() -> [i32; N_SD] {
    let mut v = [0i32; N_SD];
    let mut i = 0;
    let mut k = -26i32;
    while k <= 26 {
        if k != 0 && k != 7 && k != -7 && k != 21 && k != -21 {
            v[i] = k;
            i += 1;
        }
        k += 1;
    }
    v
}

/// The FFT bin of subcarrier `k`.
#[inline]
pub fn bin(k: i32) -> usize {
    k.rem_euclid(NFFT as i32) as usize
}

/// The modulation of the data subcarriers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Modulation {
    /// BPSK, 1 bit per subcarrier.
    Bpsk,
    /// QPSK, 2 bits.
    Qpsk,
    /// 16-QAM, 4 bits.
    Qam16,
    /// 64-QAM, 6 bits.
    Qam64,
}

impl Modulation {
    /// Bits per subcarrier (N_BPSC).
    pub fn bits(self) -> usize {
        match self {
            Modulation::Bpsk => 1,
            Modulation::Qpsk => 2,
            Modulation::Qam16 => 4,
            Modulation::Qam64 => 6,
        }
    }

    /// The point set with the 802.11 bit labels (clause 17.3.5.8): BPSK 0 is -1, QPSK and QAM by `QamMap::IEEE_802_11`.
    pub fn points(self) -> PointSet {
        let made = match self {
            Modulation::Bpsk => PointSet::pam(2),
            Modulation::Qpsk => PointSet::qam(4, QamMap::IEEE_802_11),
            Modulation::Qam16 => PointSet::qam(16, QamMap::IEEE_802_11),
            Modulation::Qam64 => PointSet::qam(64, QamMap::IEEE_802_11),
        };
        made.unwrap_or_else(|_| unreachable!("the 802.11 constellations are valid"))
    }
}

/// The eight rates of the 20 MHz PHY (table 17-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rate {
    /// 6 Mbit/s: BPSK 1/2.
    R6,
    /// 9 Mbit/s: BPSK 3/4.
    R9,
    /// 12 Mbit/s: QPSK 1/2.
    R12,
    /// 18 Mbit/s: QPSK 3/4.
    R18,
    /// 24 Mbit/s: 16-QAM 1/2.
    R24,
    /// 36 Mbit/s: 16-QAM 3/4.
    R36,
    /// 48 Mbit/s: 64-QAM 2/3.
    R48,
    /// 54 Mbit/s: 64-QAM 3/4.
    R54,
}

impl Rate {
    /// Every rate, slowest first.
    pub const ALL: [Rate; 8] = [
        Rate::R6,
        Rate::R9,
        Rate::R12,
        Rate::R18,
        Rate::R24,
        Rate::R36,
        Rate::R48,
        Rate::R54,
    ];

    /// The data rate, Mbit/s.
    pub fn mbps(self) -> u32 {
        match self {
            Rate::R6 => 6,
            Rate::R9 => 9,
            Rate::R12 => 12,
            Rate::R18 => 18,
            Rate::R24 => 24,
            Rate::R36 => 36,
            Rate::R48 => 48,
            Rate::R54 => 54,
        }
    }

    /// The RATE bits R1 to R4 (table 17-6), R1 first.
    pub fn rate_bits(self) -> [u8; 4] {
        match self {
            Rate::R6 => [1, 1, 0, 1],
            Rate::R9 => [1, 1, 1, 1],
            Rate::R12 => [0, 1, 0, 1],
            Rate::R18 => [0, 1, 1, 1],
            Rate::R24 => [1, 0, 0, 1],
            Rate::R36 => [1, 0, 1, 1],
            Rate::R48 => [0, 0, 0, 1],
            Rate::R54 => [0, 0, 1, 1],
        }
    }

    /// The rate of RATE bits R1 to R4.
    pub fn from_rate_bits(b: [u8; 4]) -> Option<Rate> {
        Rate::ALL.into_iter().find(|r| r.rate_bits() == b)
    }

    /// The modulation.
    pub fn modulation(self) -> Modulation {
        match self {
            Rate::R6 | Rate::R9 => Modulation::Bpsk,
            Rate::R12 | Rate::R18 => Modulation::Qpsk,
            Rate::R24 | Rate::R36 => Modulation::Qam16,
            Rate::R48 | Rate::R54 => Modulation::Qam64,
        }
    }

    /// The puncturing pattern of the coding rate.
    pub fn puncture(self) -> Puncture<'static> {
        match self {
            Rate::R6 | Rate::R12 | Rate::R24 => codes::NONE_1_2,
            Rate::R48 => codes::WIFI_2_3,
            _ => codes::WIFI_3_4,
        }
    }

    /// The coding rate as (numerator, denominator).
    pub fn coding(self) -> (usize, usize) {
        match self {
            Rate::R6 | Rate::R12 | Rate::R24 => (1, 2),
            Rate::R48 => (2, 3),
            _ => (3, 4),
        }
    }

    /// Coded bits per subcarrier (N_BPSC).
    pub fn n_bpsc(self) -> usize {
        self.modulation().bits()
    }

    /// Coded bits per symbol (N_CBPS).
    pub fn n_cbps(self) -> usize {
        N_SD * self.n_bpsc()
    }

    /// Data bits per symbol (N_DBPS).
    pub fn n_dbps(self) -> usize {
        let (a, b) = self.coding();
        self.n_cbps() * a / b
    }

    /// Data symbols of a PSDU of `length` octets (equation 17-11).
    pub fn n_sym(self, length: usize) -> usize {
        (SERVICE_BITS + 8 * length + TAIL_BITS).div_ceil(self.n_dbps())
    }

    /// The interleaver of one symbol (equations 17-17 and 17-18).
    pub fn interleaver(self) -> Permutation {
        Permutation::wifi(self.n_cbps(), self.n_bpsc(), N_COL)
            .unwrap_or_else(|_| unreachable!("N_CBPS is a multiple of 16"))
    }
}

/// The SIGNAL field (clause 17.3.4): RATE, a reserved bit, LENGTH, even parity over the first 17 bits, six tail bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signal {
    /// The rate of the DATA field.
    pub rate: Rate,
    /// The PSDU length, octets (1 to 4095).
    pub length: u16,
}

impl Signal {
    /// The 24 SIGNAL bits in the order sent.
    pub fn bits(&self) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[..4].copy_from_slice(&self.rate.rate_bits());
        for i in 0..12 {
            b[5 + i] = ((self.length >> i) & 1) as u8;
        }
        b[17] = b[..17].iter().fold(0, |a, &x| a ^ x);
        b
    }

    /// Reads the SIGNAL bits (the 18 bits before the tail are enough). Errors for bad parity, a reserved bit set, an unknown
    /// RATE or a LENGTH of 0.
    pub fn from_bits(b: &[u8]) -> Result<Signal> {
        if b.len() < 18 {
            return Err(Error::InputTooShort);
        }
        if b[..18].iter().fold(0, |a, &x| a ^ (x & 1)) != 0 {
            return Err(Error::Check(CheckKind::SignalParity));
        }
        if b[4] != 0 {
            return Err(Error::Malformed(Malformation::SignalReserved));
        }
        let rate = Rate::from_rate_bits([b[0], b[1], b[2], b[3]])
            .ok_or(Error::Malformed(Malformation::UnknownRate))?;
        let length = pack::to_u64_lsb(&b[5..17]) as u16;
        if length == 0 {
            return Err(Error::Malformed(Malformation::ZeroLength));
        }
        Ok(Signal { rate, length })
    }
}

/// The scrambler start state whose first seven outputs are `first7` (the scrambled zero bits 0 to 6 of SERVICE, first received
/// first). Errors for fewer than 7 bits or the all-zero output, which no allowed state makes.
pub fn scrambler_seed(first7: &[u8]) -> Result<u8> {
    if first7.len() < 7 {
        return Err(Error::InputTooShort);
    }
    let want = pack::to_u64(&first7[..7]);
    (1u8..128)
        .find(|&s| {
            let mut l = seq::ieee80211(s);
            (0..7).fold(0u64, |a, _| (a << 1) | l.bit() as u64) == want
        })
        .ok_or(Error::Malformed(Malformation::NoScramblerState))
}

/// The register that continues a scrambler whose first seven outputs were `first7`.
pub fn scrambler_after(first7: &[u8]) -> Result<Lfsr> {
    Ok(seq::ieee80211_from_outputs(first7)?)
}

/// Builds the coded and interleaved bits of the DATA field (clause 17.3.5): SERVICE (16 zeros), the PSDU least significant bit
/// first, six tail bits and pad bits, scrambled from `seed`, the tail set to zero after scrambling, encoded, punctured and
/// interleaved symbol by symbol. `out` gets `n_sym * n_cbps` bits. Errors for an empty or too long PSDU, or a seed of 0.
pub fn encode_data(psdu: &[u8], rate: Rate, seed: u8, out: &mut Vec<u8>) -> Result<()> {
    if psdu.is_empty() || psdu.len() > MAX_PSDU {
        return Err(Error::BadParameter(Param::PsduLength));
    }
    if seed & 0x7F == 0 {
        return Err(Error::BadParameter(Param::ScramblerSeed));
    }
    let n_sym = rate.n_sym(psdu.len());
    let n_data = n_sym * rate.n_dbps();
    let mut bits = vec![0u8; SERVICE_BITS];
    pack::unpack(psdu, BitOrder::Lsb, &mut bits);
    bits.resize(n_data, 0);
    let mut s = seq::ieee80211(seed);
    for b in bits.iter_mut() {
        *b ^= s.bit();
    }
    let tail = SERVICE_BITS + 8 * psdu.len();
    bits[tail..tail + TAIL_BITS].fill(0);
    let mut coded = Vec::with_capacity(2 * n_data);
    Encoder::new(codes::WIFI_K7).encode(&bits, &mut coded);
    let mut sent = Vec::with_capacity(n_sym * rate.n_cbps());
    puncture(&rate.puncture(), 0, &coded, &mut sent);
    let perm = rate.interleaver();
    let start = out.len();
    out.resize(start + sent.len(), 0);
    for (src, dst) in sent
        .chunks_exact(rate.n_cbps())
        .zip(out[start..].chunks_exact_mut(rate.n_cbps()))
    {
        perm.apply(src, dst)?;
    }
    Ok(())
}

/// The coded SIGNAL bits: the 24 bits encoded at rate 1/2 and interleaved as BPSK (clause 17.3.4.4).
pub fn encode_signal(signal: &Signal) -> [u8; 48] {
    let mut coded = Vec::with_capacity(48);
    Encoder::new(codes::WIFI_K7).encode(&signal.bits(), &mut coded);
    let mut out = [0u8; 48];
    let _ = Rate::R6.interleaver().apply(&coded, &mut out);
    out
}

/// Maps the coded bits of one symbol to its 48 data subcarrier values (in [`data_carriers`] order). Errors when the bit count is
/// not N_CBPS of the modulation.
pub fn map_symbol(coded: &[u8], modulation: Modulation, out: &mut [Complex32; N_SD]) -> Result<()> {
    let m = modulation.bits();
    if coded.len() != N_SD * m {
        return Err(Error::BadParameter(Param::CodedBitsPerSymbol));
    }
    let pts = modulation.points();
    for (o, g) in out.iter_mut().zip(coded.chunks_exact(m)) {
        let label = g.iter().fold(0usize, |a, &b| (a << 1) | (b & 1) as usize);
        *o = pts.points()[label];
    }
    Ok(())
}

/// The frequency-domain short training symbol on the 64 bins (equation 17-6), with the sqrt(13/6) factor.
pub fn stf_bins() -> [Complex32; NFFT] {
    let a = (13.0f32 / 6.0).sqrt();
    let mut x = [Complex32::default(); NFFT];
    for (i, &v) in IEEE80211A_STF.iter().enumerate() {
        x[bin(i as i32 - 26)] = Complex32::new(a * v as f32, a * v as f32);
    }
    x
}

/// The frequency-domain long training symbol on the 64 bins (equation 17-8).
pub fn ltf_bins() -> [Complex32; NFFT] {
    let mut x = [Complex32::default(); NFFT];
    for (i, &v) in LTF.iter().enumerate() {
        x[bin(i as i32 - 26)] = Complex32::new(v as f32, 0.0);
    }
    x
}

/// The time-domain long training symbol (64 samples) at the scale [`build_ppdu`] uses.
pub fn ltf_time() -> [Complex32; NFFT] {
    let mut x = ltf_bins();
    Fft::inverse(NFFT).process(&mut x);
    let s = 1.0 / 52f32.sqrt();
    x.iter_mut().for_each(|v| *v *= s);
    x
}

/// Builds a non-HT PPDU at 20 Msps for a PSDU (FCS included): the short and long training fields, SIGNAL and the DATA symbols,
/// `320 + 80 * (1 + n_sym)` samples appended to `out`. The data subcarriers have unit mean power. There is no time-domain
/// windowing between symbols. Errors as [`encode_data`].
pub fn build_ppdu(psdu: &[u8], rate: Rate, seed: u8, out: &mut Vec<Complex32>) -> Result<()> {
    let mut coded = Vec::new();
    encode_data(psdu, rate, seed, &mut coded)?;
    let mut fft = Fft::inverse(NFFT);
    let scale = 1.0 / 52f32.sqrt();
    let pol = ieee80211_pilot_polarity();
    let dc = data_carriers();
    let mut x = stf_bins();
    fft.process(&mut x);
    out.extend((0..160).map(|i| x[i % NFFT] * scale));
    let l = ltf_time();
    out.extend_from_slice(&l[32..]);
    out.extend_from_slice(&l);
    out.extend_from_slice(&l);
    let mut symbol = |cells: &[Complex32; N_SD], p: i8, out: &mut Vec<Complex32>| {
        let mut x = [Complex32::default(); NFFT];
        for (&k, &c) in dc.iter().zip(cells) {
            x[bin(k)] = c;
        }
        for &(k, v) in &PILOTS {
            x[bin(k)] = Complex32::new(v * p as f32, 0.0);
        }
        fft.process(&mut x);
        out.extend(x[NFFT - GI..].iter().map(|v| v * scale));
        out.extend(x.iter().map(|v| v * scale));
    };
    let sig = encode_signal(&Signal {
        rate,
        length: psdu.len() as u16,
    });
    let mut cells = [Complex32::default(); N_SD];
    map_symbol(&sig, Modulation::Bpsk, &mut cells)?;
    symbol(&cells, pol[0], out);
    for (i, sym) in coded.chunks_exact(rate.n_cbps()).enumerate() {
        map_symbol(sym, rate.modulation(), &mut cells)?;
        symbol(&cells, pol[(i + 1) % 127], out);
    }
    Ok(())
}

/// Decodes the DATA field from soft values: depuncturing, quantising to `i8` LLRs, the Viterbi decoder and the descrambler.
/// Sized once for the longest PSDU, then allocates nothing.
#[derive(Clone, Debug)]
pub struct DataDecoder {
    viterbi: Viterbi,
    sent: Vec<i8>,
    llr: Vec<i8>,
    bits: Vec<u8>,
    psdu: Vec<u8>,
}

impl Default for DataDecoder {
    fn default() -> Self {
        DataDecoder::new()
    }
}

impl DataDecoder {
    /// A decoder for PSDUs up to [`MAX_PSDU`] octets.
    pub fn new() -> DataDecoder {
        let steps = SERVICE_BITS + 8 * MAX_PSDU + TAIL_BITS + 216;
        DataDecoder {
            viterbi: Viterbi::new(codes::WIFI_K7, steps)
                .unwrap_or_else(|_| unreachable!("WIFI_K7 is a valid code")),
            sent: Vec::with_capacity(2 * steps),
            llr: Vec::with_capacity(2 * steps),
            bits: Vec::with_capacity(steps),
            psdu: Vec::with_capacity(MAX_PSDU),
        }
    }

    /// Decodes the deinterleaved LLRs of every DATA symbol (positive means 0, `n_sym * n_cbps` values) of a PSDU of `length`
    /// octets. Returns the PSDU (FCS included) and the scrambler seed. Errors when the count of values is wrong.
    pub fn decode(&mut self, llr: &[f32], rate: Rate, length: usize) -> Result<(&[u8], u8)> {
        if length == 0 || length > MAX_PSDU {
            return Err(Error::BadParameter(Param::PsduLength));
        }
        let n_sym = rate.n_sym(length);
        if llr.len() != n_sym * rate.n_cbps() {
            return Err(Error::BadParameter(Param::LlrCount));
        }
        // The values up to the end of the tail; the pad bits after it are not decoded.
        let steps = SERVICE_BITS + 8 * length + TAIL_BITS;
        let puncture = rate.puncture();
        let sent = puncture.punctured_len(0, 2 * steps);
        self.llr.clear();
        self.llr.resize(2 * steps, 0);
        if sent == 2 * steps {
            // Rate 1/2: nothing was removed.
            Quantizer::default().quantize_auto(&llr[..sent], &mut self.llr)?;
        } else {
            self.sent.clear();
            self.sent.resize(sent, 0);
            Quantizer::default().quantize_auto(&llr[..sent], &mut self.sent)?;
            depuncture_frame(&puncture, 0, &self.sent, &mut self.llr)?;
        }
        self.bits.clear();
        self.bits.resize(steps - TAIL_BITS, 0);
        self.viterbi
            .decode_only(&self.llr, Ends::Zero, &mut self.bits)?;
        let seed = scrambler_seed(&self.bits[..7])?;
        let mut s = scrambler_after(&self.bits[..7])?;
        // The scrambler runs on over the rest of SERVICE, then over the PSDU a byte at a time.
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
        Ok((&self.psdu, seed))
    }
}

/// Demaps the 48 equalised data cells of one symbol into deinterleaved LLRs (positive means 0) appended to `out`. `weights` are
/// `|H|^2 / noise` per cell. `scratch` must hold N_CBPS values.
pub(crate) fn demap_symbol(
    c: &Constellation,
    perm: &Permutation,
    cells: &[Complex32; N_SD],
    weights: &[f32; N_SD],
    scratch: &mut [f32],
    out: &mut Vec<f32>,
) -> Result<()> {
    let m = c.bits();
    let n = N_SD * m;
    c.demap_block_weighted(cells, weights, Demap::MaxLog, &mut scratch[..n])?;
    let start = out.len();
    out.resize(start + n, 0.0);
    perm.invert(&scratch[..n], &mut out[start..])?;
    Ok(())
}
