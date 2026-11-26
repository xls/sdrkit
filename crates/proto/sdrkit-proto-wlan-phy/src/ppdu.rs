//! The PPDU formats that start with the non-HT preamble and a 6 Mbit/s L-SIG: HT-mixed (802.11n), VHT (802.11ac), HE
//! (802.11ax) and EHT (802.11be). [`PpduDecoder`] takes a PPDU that the non-HT receiver hands over ([`PpduView`]), finds the
//! format, reads the signal fields and decodes the data of the formats it can.
//!
//! How a format is found (20 MHz, the primary channel):
//!
//! | Format | After L-SIG |
//! |---|---|
//! | HT-mixed | two symbols with BPSK on the Q axis (HT-SIG, CRC-8) |
//! | VHT | a BPSK symbol, then one with BPSK on the Q axis (VHT-SIG-A, CRC-8) |
//! | HE | L-SIG again (RL-SIG) with a LENGTH that is not a multiple of 3: remainder 1 is HE SU or HE TB, remainder 2 is HE MU, or HE ER SU when the second HE-SIG-A symbol is on the Q axis |
//! | EHT | RL-SIG with a LENGTH that is a multiple of 3, then U-SIG |
//!
//! Decoded: HT and VHT with one spatial stream, 20 MHz, the convolutional code. For every other PPDU the signal fields are
//! returned with the reason the data is not decoded ([`Undecoded`]). One antenna cannot separate two or more spatial streams,
//! and a 20 Msps input does not hold a 40 MHz or wider PPDU.
//!
//! Standard: IEEE Std 802.11-2020 clauses 19 and 21; IEEE Std 802.11ax-2021 clause 27; IEEE Std 802.11be-2024 clause 36.

use crate::ht::{self, DataFail, DataField, HtDecoder, HtSig, BACKOFF};
use crate::ofdm::{self, bin, encode_signal, ltf_time, stf_bins, PpduView, Rate, Signal, NFFT};
use crate::sig::{vht_sig_b_20_length, vht_sig_b_crc, HeFormat, HeSigA, USig, VhtSigA};
use crate::vht::{self, VhtMcs};
use crate::Complex32;
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_fec::conv::{codes, Encoder, Ends, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::constellation::{Constellation, Demap};
use sdrkit_algo_modem::ofdm::ieee80211_pilot_polarity;
use sdrkit_util_bits::interleave::Permutation;

/// The values of the four extra subcarriers -28, -27, 27, 28 of L-SIG, RL-SIG, HE-SIG-A and U-SIG symbols.
pub const EXTRA_TONES: [(i32, f32); 4] = [(-28, -1.0), (-27, -1.0), (27, -1.0), (28, 1.0)];

/// A PPDU format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    /// HT-mixed (802.11n).
    Ht,
    /// VHT (802.11ac).
    Vht,
    /// HE single-user (802.11ax).
    HeSu,
    /// HE extended-range single-user.
    HeErSu,
    /// HE multi-user.
    HeMu,
    /// HE trigger-based.
    HeTb,
    /// An HE PPDU whose HE-SIG-A failed its CRC.
    He,
    /// EHT (802.11be), or a later PHY version in U-SIG.
    Eht,
}

impl Format {
    /// A short name.
    pub fn name(self) -> &'static str {
        match self {
            Format::Ht => "HT",
            Format::Vht => "VHT",
            Format::HeSu => "HE SU",
            Format::HeErSu => "HE ER SU",
            Format::HeMu => "HE MU",
            Format::HeTb => "HE TB",
            Format::He => "HE",
            Format::Eht => "EHT",
        }
    }
}

/// Why the data of a PPDU is not decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Undecoded {
    /// Two or more spatial streams: one antenna cannot separate them.
    Streams,
    /// Wider than the 20 MHz input.
    Width,
    /// LDPC coding.
    Ldpc,
    /// Space-time block coding.
    Stbc,
    /// A multi-user PPDU.
    MultiUser,
    /// A trigger-based PPDU.
    TriggerBased,
    /// The HE data field is not decoded.
    HeData,
    /// The EHT data field is not decoded.
    EhtData,
    /// The signal field after RL-SIG failed its CRC.
    SignalField,
    /// An MCS that the format does not have at this width, or a reserved value.
    Mcs,
    /// The PPDU is longer than the decoder holds, or than the samples.
    Length,
    /// The decoder refused the data symbols.
    Data,
    /// Another option of the format (HT extension streams, no data).
    Other,
    /// The data could be decoded, and the caller skipped it to keep up with its input.
    Busy,
}

impl Undecoded {
    /// The reason in a few words.
    pub fn text(self) -> &'static str {
        match self {
            Undecoded::Streams => "two or more spatial streams",
            Undecoded::Width => "wider than 20 MHz",
            Undecoded::Ldpc => "LDPC coding",
            Undecoded::Stbc => "space-time block coding",
            Undecoded::MultiUser => "multi-user",
            Undecoded::TriggerBased => "trigger-based",
            Undecoded::HeData => "HE data field",
            Undecoded::EhtData => "EHT data field",
            Undecoded::SignalField => "signal field CRC",
            Undecoded::Mcs => "MCS not valid",
            Undecoded::Length => "length",
            Undecoded::Data => "data symbols",
            Undecoded::Other => "other option",
            Undecoded::Busy => "receiver busy",
        }
    }
}

/// The signal fields of a PPDU as read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SigFields {
    /// HT-SIG.
    Ht(HtSig),
    /// VHT-SIG-A.
    Vht(VhtSigA),
    /// HE-SIG-A with the format it was read as.
    He(HeSigA, HeFormat),
    /// U-SIG.
    Eht(USig),
    /// None passed its CRC.
    None,
}

/// What the signal fields say about a PPDU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PpduInfo {
    /// The format.
    pub format: Format,
    /// The bandwidth, MHz (0: not known).
    pub bandwidth_mhz: u16,
    /// Space-time streams (0: the signal field does not give one number).
    pub streams: u8,
    /// The MCS of the data, when the signal field gives it.
    pub mcs: Option<u8>,
    /// LDPC coding.
    pub ldpc: bool,
    /// The short guard interval (HT, VHT).
    pub short_gi: bool,
    /// The PSDU is an A-MPDU.
    pub aggregated: bool,
    /// The BSS colour (HE, EHT).
    pub bss_color: Option<u8>,
    /// An uplink PPDU (HE, EHT).
    pub uplink: Option<bool>,
    /// The fields.
    pub sig: SigFields,
}

impl PpduInfo {
    fn of(format: Format, sig: SigFields) -> PpduInfo {
        PpduInfo {
            format,
            bandwidth_mhz: 0,
            streams: 0,
            mcs: None,
            ldpc: false,
            short_gi: false,
            aggregated: false,
            bss_color: None,
            uplink: None,
            sig,
        }
    }
}

/// What [`PpduDecoder::decode`] made of a PPDU.
#[derive(Clone, Debug, PartialEq)]
pub enum Ppdu<'a> {
    /// None of the formats here: the caller decodes it as non-HT.
    NonHt,
    /// A PPDU of one of the formats whose data is not decoded.
    Seen {
        /// The signal fields.
        info: PpduInfo,
        /// The reason.
        why: Undecoded,
    },
    /// The decoded PSDU. Nothing in it is checked: one MPDU with its FCS (HT without aggregation), else an A-MPDU for
    /// [`crate::mac::Ampdu`].
    Psdu {
        /// The signal fields.
        info: PpduInfo,
        /// The octets.
        psdu: &'a [u8],
        /// The scrambler seed.
        seed: u8,
        /// The signal-to-noise ratio from the decisions of the data cells, dB.
        snr_db: f64,
        /// VHT: the A-MPDU length of VHT-SIG-B in octets, when the CRC in the SERVICE field confirms it.
        sig_b_length: Option<usize>,
    },
}

/// What [`PpduDecoder::read`] made of the signal fields of a PPDU.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Read {
    /// None of the formats here: the caller decodes it as non-HT.
    NonHt,
    /// A PPDU whose data is not decoded, with the reason.
    Seen {
        /// The signal fields.
        info: PpduInfo,
        /// The reason.
        why: Undecoded,
    },
    /// A PPDU whose data field [`PpduDecoder::decode_data`] decodes.
    Data(DataPlan),
}

/// A data field that [`PpduDecoder::read`] found decodable: the signal fields and the layout of the data symbols.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataPlan {
    /// The signal fields.
    pub info: PpduInfo,
    kind: PlanKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlanKind {
    Ht(HtSig),
    Vht { a: VhtSigA, n_sym: usize, steps: usize, mcs: VhtMcs },
}

/// A PPDU copied out of the receiver's buffers, for decoding on another thread. Its buffer is reused by each copy.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnedPpdu {
    samples: Vec<Complex32>,
    sample: u64,
    signal: Signal,
    offset_hz: f64,
    channel: [Complex32; NFFT],
    noise: f32,
    snr_db: f64,
    level_dbfs: f64,
}

impl Default for OwnedPpdu {
    fn default() -> Self {
        OwnedPpdu {
            samples: Vec::new(),
            sample: 0,
            signal: Signal {
                rate: Rate::R6,
                length: 0,
            },
            offset_hz: 0.0,
            channel: [Complex32::default(); NFFT],
            noise: 0.0,
            snr_db: 0.0,
            level_dbfs: 0.0,
        }
    }
}

impl OwnedPpdu {
    /// Copies `v`, all its samples included.
    pub fn copy_from(&mut self, v: &PpduView<'_>) {
        self.samples.clear();
        self.samples.extend_from_slice(v.samples);
        self.sample = v.sample;
        self.signal = v.signal;
        self.offset_hz = v.offset_hz;
        self.channel = *v.channel;
        self.noise = v.noise;
        self.snr_db = v.snr_db;
        self.level_dbfs = v.level_dbfs;
    }

    /// The copy as a view.
    pub fn view(&self) -> PpduView<'_> {
        PpduView {
            samples: &self.samples,
            sample: self.sample,
            signal: self.signal,
            offset_hz: self.offset_hz,
            channel: &self.channel,
            noise: self.noise,
            snr_db: self.snr_db,
            level_dbfs: self.level_dbfs,
        }
    }
}

/// The decoder of HT, VHT, HE and EHT PPDUs (module documentation). Sized once, then allocates nothing.
pub struct PpduDecoder {
    ht: HtDecoder,
    pol: [i8; 127],
    non_ht_dc: [i32; 48],
    dc52: [i32; 52],
    bpsk: Constellation,
    r6: Permutation,
    p52: Permutation,
    viterbi: Viterbi,
    max_bits: usize,
}

impl Default for PpduDecoder {
    fn default() -> Self {
        PpduDecoder::new()
    }
}

/// The 48 or 52 cells of a symbol turned from the Q axis to the I axis.
fn from_q<const N: usize>(c: &mut [Complex32; N]) {
    for v in c.iter_mut() {
        *v = Complex32::new(v.im, -v.re);
    }
}

/// Whether the cells carry more energy on the Q axis than on the I axis.
fn on_q(c: &[Complex32]) -> bool {
    let q: f32 = c.iter().map(|v| v.im * v.im).sum();
    let i: f32 = c.iter().map(|v| v.re * v.re).sum();
    q > i
}

impl PpduDecoder {
    /// A decoder for PSDUs of up to 65535 octets.
    pub fn new() -> PpduDecoder {
        PpduDecoder::with_max_psdu(ht::MAX_AMPDU)
    }

    /// A decoder for PSDUs of up to `max_psdu` octets.
    pub fn with_max_psdu(max_psdu: usize) -> PpduDecoder {
        let max_psdu = max_psdu.clamp(1, ht::MAX_AMPDU);
        PpduDecoder {
            ht: HtDecoder::with_max_psdu(max_psdu),
            pol: ieee80211_pilot_polarity(),
            non_ht_dc: ofdm::data_carriers(),
            dc52: ht::data_carriers(),
            bpsk: Constellation::new(ofdm::Modulation::Bpsk.points())
                .unwrap_or_else(|_| unreachable!("BPSK is valid")),
            r6: Rate::R6.interleaver(),
            p52: Permutation::wifi(52, 1, 13).unwrap_or_else(|_| unreachable!("52 is a multiple of 13")),
            viterbi: Viterbi::new(codes::WIFI_K7, 52).unwrap_or_else(|_| unreachable!("WIFI_K7 is a valid code")),
            max_bits: 16 + 8 * max_psdu + 6,
        }
    }

    /// The FFT bins of symbol `s` after the long training field (0 is L-SIG) with the common phase and the phase slope of its
    /// pilots removed, on subcarriers -28 to 28.
    fn bins(&mut self, v: &PpduView<'_>, s: usize) -> Option<[Complex32; NFFT]> {
        let mut y = self.ht.fft_at(v, 128 + 80 * s + 16 - BACKOFF)?;
        let pol = f32::from(self.pol[s % 127]);
        let pil: [Complex32; 4] = std::array::from_fn(|i| {
            let (k, val) = ofdm::PILOTS[i];
            let h = v.channel[bin(k)];
            if h.norm_sqr() > 1e-20 {
                y[bin(k)] / h * (val * pol)
            } else {
                Complex32::default()
            }
        });
        let acc: Complex32 = pil.iter().sum();
        let phi0 = approx::atan2(acc.im, acc.re);
        let (s0, c0) = approx::sin_cos(-f64::from(phi0));
        let r0 = Complex32::new(c0, s0);
        let (mut num, mut den) = (0f32, 0f32);
        for (&(k, _), &p) in ofdm::PILOTS.iter().zip(&pil) {
            let e = p * r0;
            num += k as f32 * approx::atan2(e.im, e.re);
            den += (k * k) as f32;
        }
        let slope = num / den;
        for k in -28i32..=28 {
            let (sn, cs) = approx::sin_cos(-f64::from(phi0 + slope * k as f32));
            y[bin(k)] *= Complex32::new(cs, sn);
        }
        Some(y)
    }

    /// The 48 equalised data cells of bins `y` and their weights `|h|^2 / noise`.
    fn cells48(&self, v: &PpduView<'_>, y: &[Complex32; NFFT]) -> ([Complex32; 48], [f32; 48]) {
        let mut c = [Complex32::default(); 48];
        let mut w = [0f32; 48];
        for (i, &k) in self.non_ht_dc.iter().enumerate() {
            let h = v.channel[bin(k)];
            if h.norm_sqr() > 1e-20 {
                c[i] = y[bin(k)] / h;
                w[i] = h.norm_sqr() / v.noise.max(1e-20);
            }
        }
        (c, w)
    }

    /// The deinterleaved LLRs of BPSK cells.
    fn llr<const N: usize>(&self, c: &[Complex32; N], w: &[f32; N], perm: &Permutation) -> Option<[f32; N]> {
        let mut raw = [0f32; N];
        self.bpsk.demap_block_weighted(c, w, Demap::MaxLog, &mut raw).ok()?;
        let mut out = [0f32; N];
        perm.invert(&raw, &mut out).ok()?;
        Some(out)
    }

    /// The bits of rate-1/2 coded LLRs that end with a zero tail.
    fn bits<const L: usize, const N: usize>(&mut self, llr: &[f32; L]) -> Option<[u8; N]> {
        let mut q = [0i8; L];
        Quantizer::default().quantize_auto(llr, &mut q).ok()?;
        let mut b = [0u8; N];
        self.viterbi.decode_only(&q, Ends::Zero, &mut b).ok()?;
        Some(b)
    }

    /// Whether symbol 1 repeats L-SIG: it decodes alone to the SIGNAL field of `v`.
    fn rl_sig(&mut self, v: &PpduView<'_>, y1: &[Complex32; NFFT]) -> bool {
        let (c, w) = self.cells48(v, y1);
        if on_q(&c) {
            return false;
        }
        let Some(llr) = self.llr(&c, &w, &self.r6) else {
            return false;
        };
        let Some(b) = self.bits::<48, 24>(&llr) else {
            return false;
        };
        Signal::from_bits(&b).is_ok_and(|s| s == v.signal)
    }

    /// VHT-SIG-A from symbols 1 and 2, when symbol 2 is on the Q axis and the field passes its checks.
    fn vht_sig_a(&mut self, v: &PpduView<'_>, y1: &[Complex32; NFFT]) -> Option<VhtSigA> {
        let y2 = self.bins(v, 2)?;
        let (c1, w) = self.cells48(v, y1);
        let (mut c2, _) = self.cells48(v, &y2);
        if on_q(&c1) || !on_q(&c2) {
            return None;
        }
        from_q(&mut c2);
        let mut llr = [0f32; 96];
        llr[..48].copy_from_slice(&self.llr(&c1, &w, &self.r6)?);
        llr[48..].copy_from_slice(&self.llr(&c2, &w, &self.r6)?);
        let b = self.bits::<96, 48>(&llr)?;
        VhtSigA::from_bits(&b).ok()
    }

    /// The 52 bits of a signal field of two BPSK symbols on the 52 data subcarriers of -28 to 28 (HE-SIG-A, U-SIG): symbols
    /// `s1` and `s2`. The channel of -28, -27, 27 and 28 comes from L-SIG and RL-SIG (`y0`, `y1`).
    fn sig52(
        &mut self,
        v: &PpduView<'_>,
        y0: &[Complex32; NFFT],
        y1: &[Complex32; NFFT],
        s1: usize,
        s2: usize,
    ) -> Option<[u8; 52]> {
        let mut h = *v.channel;
        for (k, val) in EXTRA_TONES {
            h[bin(k)] = (y0[bin(k)] + y1[bin(k)]) * (0.5 * val);
        }
        let mut llr = [0f32; 104];
        for (i, s) in [s1, s2].into_iter().enumerate() {
            let y = self.bins(v, s)?;
            let mut c = [Complex32::default(); 52];
            let mut w = [0f32; 52];
            for (j, &k) in self.dc52.iter().enumerate() {
                let hk = h[bin(k)];
                if hk.norm_sqr() > 1e-20 {
                    c[j] = y[bin(k)] / hk;
                    w[j] = hk.norm_sqr() / v.noise.max(1e-20);
                }
            }
            llr[52 * i..52 * (i + 1)].copy_from_slice(&self.llr(&c, &w, &self.p52)?);
        }
        self.bits::<104, 52>(&llr)
    }

    /// Reads PPDU `v`: [`read`](Self::read), then [`decode_data`](Self::decode_data) for a decodable data field.
    pub fn decode(&mut self, v: &PpduView<'_>) -> Ppdu<'_> {
        match self.read(v) {
            Read::NonHt => Ppdu::NonHt,
            Read::Seen { info, why } => Ppdu::Seen { info, why },
            Read::Data(plan) => self.decode_data(v, &plan),
        }
    }

    /// Reads the signal fields of PPDU `v` and finds whether its data field can be decoded. The data symbols are not touched.
    pub fn read(&mut self, v: &PpduView<'_>) -> Read {
        if v.signal.rate != Rate::R6 {
            return Read::NonHt;
        }
        if let Some(sig) = self.ht.ht_sig(v) {
            return self.ht_plan(sig);
        }
        let Some(y1) = self.bins(v, 1) else {
            return Read::NonHt;
        };
        if self.rl_sig(v, &y1) {
            let Some(y0) = self.bins(v, 0) else {
                return Read::NonHt;
            };
            let (info, why) = match v.signal.length % 3 {
                0 => self.eht(v, &y0, &y1),
                r => self.he(v, &y0, &y1, r),
            };
            return Read::Seen { info, why };
        }
        match self.vht_sig_a(v, &y1) {
            Some(a) => self.vht_plan(v, a),
            None => Read::NonHt,
        }
    }

    /// Decodes the data field of PPDU `v` that [`read`](Self::read) planned. Any decoder made with the same PSDU size gives the
    /// same result as the one that read the signal fields.
    pub fn decode_data(&mut self, v: &PpduView<'_>, plan: &DataPlan) -> Ppdu<'_> {
        let info = plan.info;
        match plan.kind {
            PlanKind::Ht(sig) => match self.ht.data(v, sig) {
                Ok((seed, _, snr_db)) => Ppdu::Psdu {
                    info,
                    psdu: self.ht.psdu(),
                    seed,
                    snr_db,
                    sig_b_length: None,
                },
                Err(DataFail::Short) => Ppdu::Seen {
                    info,
                    why: Undecoded::Length,
                },
                Err(_) => Ppdu::Seen {
                    info,
                    why: Undecoded::Data,
                },
            },
            PlanKind::Vht { a, n_sym, steps, mcs } => self.vht_data(v, info, a, n_sym, steps, mcs),
        }
    }

    fn ht_plan(&self, sig: HtSig) -> Read {
        let mut info = PpduInfo::of(Format::Ht, SigFields::Ht(sig));
        info.bandwidth_mhz = if sig.cbw40 { 40 } else { 20 };
        info.streams = sig.streams();
        info.mcs = Some(sig.mcs);
        info.ldpc = sig.ldpc;
        info.short_gi = sig.short_gi;
        info.aggregated = sig.aggregation;
        let why = if sig.mcs > 76 || sig.mcs == 32 {
            Some(Undecoded::Mcs)
        } else if sig.mcs > 7 {
            Some(Undecoded::Streams)
        } else if sig.cbw40 {
            Some(Undecoded::Width)
        } else if sig.ldpc {
            Some(Undecoded::Ldpc)
        } else if sig.stbc != 0 {
            Some(Undecoded::Stbc)
        } else if sig.ness != 0 || sig.length == 0 {
            Some(Undecoded::Other)
        } else if 16 + 8 * usize::from(sig.length) + 6 > self.max_bits {
            Some(Undecoded::Length)
        } else {
            None
        };
        match why {
            Some(why) => Read::Seen { info, why },
            None => Read::Data(DataPlan {
                info,
                kind: PlanKind::Ht(sig),
            }),
        }
    }

    fn vht_plan(&self, v: &PpduView<'_>, a: VhtSigA) -> Read {
        let mut info = PpduInfo::of(Format::Vht, SigFields::Vht(a));
        info.bandwidth_mhz = a.bandwidth_mhz();
        info.streams = a.nsts();
        info.ldpc = a.ldpc();
        info.short_gi = a.short_gi();
        info.aggregated = true;
        if !a.is_su() {
            return Read::Seen {
                info,
                why: Undecoded::MultiUser,
            };
        }
        info.mcs = Some(a.mcs());
        let mcs = VhtMcs(a.mcs());
        let n_sym = vht::n_sym(v.signal.length, a.short_gi(), a.short_gi_disambiguation());
        let steps = n_sym * mcs.n_dbps();
        let why = if a.stbc() {
            Some(Undecoded::Stbc)
        } else if a.nsts() > 1 {
            Some(Undecoded::Streams)
        } else if a.bandwidth_mhz() != 20 {
            Some(Undecoded::Width)
        } else if a.ldpc() {
            Some(Undecoded::Ldpc)
        } else if a.mcs() > 9 || !mcs.valid_20() {
            Some(Undecoded::Mcs)
        } else if n_sym == 0 {
            Some(Undecoded::Other)
        } else if steps > self.max_bits {
            Some(Undecoded::Length)
        } else {
            None
        };
        match why {
            Some(why) => Read::Seen { info, why },
            None => Read::Data(DataPlan {
                info,
                kind: PlanKind::Vht { a, n_sym, steps, mcs },
            }),
        }
    }

    fn vht_data(
        &mut self,
        v: &PpduView<'_>,
        info: PpduInfo,
        a: VhtSigA,
        n_sym: usize,
        steps: usize,
        mcs: VhtMcs,
    ) -> Ppdu<'_> {
        let sig_b = self.ht.vht_sig_b(v, vht::DATA_START - 80);
        let field = DataField {
            start: vht::DATA_START,
            n_sym,
            sym: if a.short_gi() { 72 } else { 80 },
            n_bpsc: mcs.n_bpsc(),
            puncture: mcs.puncture(),
            pol0: 4,
            rotating_only: true,
            steps,
        };
        match self.ht.data_field(v, &field) {
            Ok((seed, _, snr_db)) => {
                let crc = self.ht.service_high();
                Ppdu::Psdu {
                    info,
                    psdu: self.ht.psdu(),
                    seed,
                    snr_db,
                    sig_b_length: sig_b
                        .filter(|b| vht_sig_b_crc(b) == crc)
                        .map(|b| vht_sig_b_20_length(&b)),
                }
            }
            Err(DataFail::Short) => Ppdu::Seen {
                info,
                why: Undecoded::Length,
            },
            Err(_) => Ppdu::Seen {
                info,
                why: Undecoded::Data,
            },
        }
    }

    fn he(&mut self, v: &PpduView<'_>, y0: &[Complex32; NFFT], y1: &[Complex32; NFFT], rem: u16) -> (PpduInfo, Undecoded) {
        // Remainder 2: HE ER SU sends each half of HE-SIG-A twice, the repeat of the first half on the Q axis.
        let er = rem == 2
            && self.bins(v, 3).is_some_and(|y| {
                let (c, _) = self.cells48(v, &y);
                on_q(&c)
            });
        let (s1, s2) = if er { (2, 4) } else { (2, 3) };
        let Some(a) = self.sig52(v, y0, y1, s1, s2).and_then(|b| HeSigA::from_bits(&b).ok()) else {
            return (PpduInfo::of(Format::He, SigFields::None), Undecoded::SignalField);
        };
        let f = match (rem, er, a.format_su()) {
            (2, true, _) => HeFormat::ErSu,
            (2, false, _) => HeFormat::Mu,
            (_, _, true) => HeFormat::Su,
            _ => HeFormat::Tb,
        };
        let format = match f {
            HeFormat::Su => Format::HeSu,
            HeFormat::ErSu => Format::HeErSu,
            HeFormat::Mu => Format::HeMu,
            HeFormat::Tb => Format::HeTb,
        };
        let mut info = PpduInfo::of(format, SigFields::He(a, f));
        info.bandwidth_mhz = a.bandwidth_mhz(f);
        info.streams = a.nsts(f).unwrap_or(0);
        info.mcs = a.mcs(f);
        info.ldpc = a.ldpc(f).unwrap_or(false);
        info.aggregated = true;
        info.bss_color = Some(a.bss_color(f));
        info.uplink = Some(a.uplink(f));
        let why = match f {
            HeFormat::Mu => Undecoded::MultiUser,
            HeFormat::Tb => Undecoded::TriggerBased,
            _ if info.streams > 1 => Undecoded::Streams,
            _ if info.bandwidth_mhz != 20 => Undecoded::Width,
            _ => Undecoded::HeData,
        };
        (info, why)
    }

    fn eht(&mut self, v: &PpduView<'_>, y0: &[Complex32; NFFT], y1: &[Complex32; NFFT]) -> (PpduInfo, Undecoded) {
        let Some(u) = self.sig52(v, y0, y1, 2, 3).and_then(|b| USig::from_bits(&b).ok()) else {
            return (PpduInfo::of(Format::Eht, SigFields::None), Undecoded::SignalField);
        };
        let mut info = PpduInfo::of(Format::Eht, SigFields::Eht(u));
        info.bandwidth_mhz = u.bandwidth_mhz();
        info.aggregated = true;
        info.bss_color = Some(u.bss_color());
        info.uplink = Some(u.uplink());
        let why = if u.is_tb() {
            Undecoded::TriggerBased
        } else if info.bandwidth_mhz != 20 {
            Undecoded::Width
        } else {
            Undecoded::EhtData
        };
        (info, why)
    }
}

impl std::fmt::Debug for PpduDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PpduDecoder").finish()
    }
}

/// Builds the start of an HE or EHT PPDU at 20 Msps for test signals: L-STF, L-LTF, L-SIG with LENGTH `l_length`, RL-SIG and
/// the signal field `sig` (HE-SIG-A or U-SIG, 52 bits in air order). With `er` each half of the field is sent twice, as in an HE
/// ER SU PPDU. The fields after the signal field are not built: symbols of pseudo-random QPSK on 56 subcarriers fill the time
/// that `l_length` gives.
pub fn build_he_preamble(l_length: u16, sig: &[u8; 52], er: bool, out: &mut Vec<Complex32>) {
    let mut fft = Fft::inverse(NFFT);
    let scale = 1.0 / 56f32.sqrt();
    let pol = ieee80211_pilot_polarity();
    let mut x = stf_bins();
    fft.process(&mut x);
    out.extend((0..160).map(|i| x[i % NFFT] / 52f32.sqrt()));
    let lt = ltf_time();
    out.extend_from_slice(&lt[32..]);
    out.extend_from_slice(&lt);
    out.extend_from_slice(&lt);
    let framed = |x: &mut [Complex32; NFFT], n: usize| {
        for (k, v) in ofdm::PILOTS {
            x[bin(k)] = Complex32::new(v * f32::from(pol[n]), 0.0);
        }
        for (k, v) in EXTRA_TONES {
            x[bin(k)] = Complex32::new(v, 0.0);
        }
    };
    let bpsk = |b: u8| if b & 1 == 1 { 1.0 } else { -1.0 };
    // L-SIG and RL-SIG.
    let coded = encode_signal(&Signal {
        rate: Rate::R6,
        length: l_length,
    });
    let non_ht_dc = ofdm::data_carriers();
    for n in 0..2 {
        let mut x = [Complex32::default(); NFFT];
        for (&k, &b) in non_ht_dc.iter().zip(&coded) {
            x[bin(k)] = Complex32::new(bpsk(b), 0.0);
        }
        framed(&mut x, n);
        ht::symbol(&mut fft, x, 16, scale, out);
    }
    // The signal field: rate 1/2, each symbol interleaved over the 52 data subcarriers.
    let mut coded = Vec::with_capacity(104);
    Encoder::new(codes::WIFI_K7).encode(sig, &mut coded);
    let p52 = Permutation::wifi(52, 1, 13).unwrap_or_else(|_| unreachable!("52 is a multiple of 13"));
    let dc52 = ht::data_carriers();
    let mut n = 2;
    for half in coded.as_chunks::<52>().0 {
        let mut sent = [0u8; 52];
        let _ = p52.apply(half, &mut sent);
        for repeat in 0..if er { 2 } else { 1 } {
            let mut x = [Complex32::default(); NFFT];
            // The repeat of an HE ER SU PPDU is sent without the interleaver; the repeat of the first half is on the Q axis.
            let bits: &[u8] = if repeat == 1 { half } else { &sent };
            for (&k, &b) in dc52.iter().zip(bits) {
                x[bin(k)] = if repeat == 1 && n == 3 {
                    Complex32::new(0.0, bpsk(b))
                } else {
                    Complex32::new(bpsk(b), 0.0)
                };
            }
            framed(&mut x, n);
            ht::symbol(&mut fft, x, 16, scale, out);
            n += 1;
        }
    }
    // Filler for the rest of the time L-SIG gives.
    let total = (usize::from(l_length) + 3).div_ceil(3);
    let mut s: u32 = 0x1234_5678;
    for _ in n - 1..total {
        let mut x = [Complex32::default(); NFFT];
        for k in (-28i32..=28).filter(|&k| k != 0) {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let a = std::f32::consts::FRAC_1_SQRT_2;
            x[bin(k)] = Complex32::new(if s >> 30 & 1 == 1 { a } else { -a }, if s >> 31 == 1 { a } else { -a });
        }
        ht::symbol(&mut fft, x, 16, scale, out);
    }
}
