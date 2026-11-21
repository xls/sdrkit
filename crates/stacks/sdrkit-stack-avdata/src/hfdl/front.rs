//! The HFDL receiver: each channel brought to 4 samples per symbol through the root raised cosine, the preamble search, and for
//! each burst its carrier offset, timing and rate, the shared equaliser and carrier loop (with two fallbacks from the probes), and
//! the data decoder.
//! Standard: ARINC 635-4 clause 4.

use super::pdu::{self, Message, Why};
use super::phy::{self, preamble, rate, DataDecoder, Rate, CARRIER_HZ, PREAMBLE, SYMBOL_RATE, T};
use super::stations;
use super::{BurstOut, ChannelState, FrontState, MIN_RATE_HZ};
use super::{CHANNELS, CHANNEL_FILTER_HZ, SHARED_CHANNELISER, WHOLE_SPAN};
use crate::report::freq_list;
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::interp::{CatmullRom, Reader};
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, DecisionDirected};
use sdrkit_algo_modem::equalizer::{AdaptiveEq, EqConfig, Mode};
use sdrkit_algo_modem::{Constellation, ModemError, PointSet};
use sdrkit_core::{Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};
use std::f64::consts::TAU;

/// The sample of `x` at fractional position `t` (Catmull-Rom, zero outside).
#[inline]
fn at(x: &[Complex32], t: f64) -> Complex32 {
    Reader::new(CatmullRom).at(x, t)
}

/// The receive matched filter: root raised cosine, roll-off 0.35, 16 symbols, unit energy (ARINC 635 clause 4.3).
fn matched_filter() -> Fir<Complex32> {
    let h = taps(Shape::Rrc { beta: 0.35 }, SPS, 16.0, Norm::Energy).unwrap_or_else(|_| vec![1.0]);
    Fir::new(&h)
}

sdrkit_core::settings! {
    /// The settings of [`Front`].
    pub enum FrontSetting {
        /// The channel list.
        Channels = CHANNELS,
        /// Every table frequency inside the capture.
        WholeSpan = WHOLE_SPAN,
        /// One channel bank for every channel.
        SharedChanneliser = SHARED_CHANNELISER,
        /// The width of each channel's filter.
        ChannelFilterHz = CHANNEL_FILTER_HZ,
    }
}

/// Reads the `channels` setting: dial frequencies in kHz from 2 to 30 MHz.
///
/// # Errors
///
/// The byte position of the first word that is not such a frequency.
pub fn channel_list(t: &str) -> Result<Vec<f64>, usize> {
    freq_list(t, 1e3, (2e6, 30e6))
}

/// The channel rate.
pub const FS: f64 = 7200.0;
const SPS: f64 = 4.0;
const HIST: usize = 7200 * 7;

struct Chan {
    dial: f64,
    chan: Option<Channelizer>,
    rrc: Fir<Complex32>,
    hist: Vec<Complex32>,
    /// One symbol's phase step at every sample: `dstep[j] = hist[j + 4] conj(hist[j])` (for the cheap first look of the search).
    dstep: Vec<Complex32>,
    /// Running power at a stride of one symbol: `cum[j] = |hist[j]|^2 + cum[j - 4]`.
    cum: Vec<f64>,
    base: u64,
    n: u64,
    resume: u64,
    /// The next sample the search looks at (it stops while a burst is pending, so the looks never depend on the cuts).
    scan_at: u64,
    pending: Option<(u64, f64)>,
    peak: Option<(f64, u64, f64)>,
    st: ChannelState,
    near_dc: bool,
    /// The slot of its sub-band in the shared channeliser's output.
    slot: Option<usize>,
}

/// The HFDL front end.
pub struct Front {
    channels: String,
    whole_span: bool,
    filter_hz: f64,
    rate: f64,
    centre: f64,
    chans: Vec<Chan>,
    out_of_band: Vec<f64>,
    trials: Vec<Vec<Complex32>>,
    a2: Vec<f32>,
    /// The products of successive A1 A2 symbols.
    steps: Vec<f32>,
    /// Decoded bursts with the channel sample from which each can be decoded (their order out).
    bursts: Vec<(u64, BurstOut)>,
    idx: u64,
    why: [u64; 6],
    tmp: Vec<Complex32>,
    shared: bool,
    decoder: DataDecoder,
    bank: Option<super::bank::PolyBank>,
    bands: Vec<Vec<Complex32>>,
}

impl Default for Front {
    fn default() -> Self {
        Front {
            channels: String::new(),
            whole_span: WHOLE_SPAN.default,
            filter_hz: CHANNEL_FILTER_HZ.default,
            rate: 0.0,
            centre: 0.0,
            chans: Vec::new(),
            out_of_band: Vec::new(),
            trials: Vec::new(),
            a2: Vec::new(),
            steps: Vec::new(),
            bursts: Vec::new(),
            idx: 0,
            why: [0; 6],
            tmp: Vec::new(),
            shared: SHARED_CHANNELISER.default,
            decoder: DataDecoder::default(),
            bank: None,
            bands: Vec::new(),
        }
    }
}

/// The dials to watch (Hz, sorted, without duplicates): the list, or every table frequency inside the band. With `whole_span`, the
/// table in the capture plus the list.
pub fn plan(list: &str, whole_span: bool, centre: f64, rate: f64) -> Vec<(f64, bool)> {
    let listed: Vec<f64> = channel_list(list).unwrap_or_default();
    let in_band = |d: f64| (d + CARRIER_HZ - centre).abs() + 4e3 < rate / 2.0 * 0.9;
    let table: Vec<f64> = stations::all_khz()
        .into_iter()
        .map(|k| k as f64 * 1e3)
        .filter(|&d| in_band(d))
        .collect();
    let mut v = if listed.is_empty() {
        table
    } else if whole_span {
        let mut t = table;
        t.extend(listed);
        t
    } else {
        listed
    };
    v.sort_by(|a, b| a.total_cmp(b));
    v.dedup();
    v.into_iter().map(|d| (d, in_band(d))).collect()
}

/// The result of decoding one burst.
#[derive(Clone, Debug, Default)]
pub struct Decoded {
    /// Messages.
    pub messages: Vec<Message>,
    /// Why some or all were lost.
    pub why: Vec<Why>,
    /// Rate.
    pub rate: Option<Rate>,
    /// SNR on the known symbols, dB.
    pub snr_db: f32,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Level, dBFS.
    pub level_db: f32,
    /// Symbols from A1 to the end.
    pub symbols: usize,
    /// The equaliser's data points (before descrambling).
    pub points: Vec<Complex32>,
}

fn rot(w: f64, k: f64) -> Complex32 {
    let a = w * k;
    Complex32::new(a.cos() as f32, -(a.sin() as f32))
}

/// Coherent correlation of the A1 + A2 symbols at fractional sample `t` with a carrier offset of `f` Hz: (A1 sum, A2 sum, energy).
fn corr_a(x: &[Complex32], t: f64, f: f64, a: &[f32]) -> (Complex32, Complex32, f64) {
    let w = TAU * f / SYMBOL_RATE;
    let (mut c1, mut c2, mut e) = (Complex32::default(), Complex32::default(), 0.0f64);
    for (k, &ak) in a.iter().enumerate() {
        let y = at(x, t + k as f64 * SPS);
        e += y.norm_sqr() as f64;
        let v = y * ak * rot(w, k as f64);
        if k < 127 {
            c1 += v;
        } else {
            c2 += v;
        }
    }
    (c1, c2, e)
}

/// The feed-forward taps of the equaliser, at two per symbol.
const NF: usize = 25;
/// The feedback taps, in symbols.
const NB: usize = 6;
/// Symbols the equaliser's output lags its newest input.
const LAG: usize = NF / 4;

/// The shared decision-feedback equaliser with the shared carrier loop after it, stepped one symbol at a time.
struct Equaliser {
    eq: AdaptiveEq,
    carrier: CarrierSync<DecisionDirected>,
}

impl Equaliser {
    fn new(bits: usize) -> Result<Self, ModemError> {
        let set = match bits {
            1 => PointSet::bpsk(),
            b => PointSet::psk(1 << b, 0.0)?,
        };
        let c = Constellation::new(set)?;
        let cfg = EqConfig {
            feedback: NB,
            normalised: true,
            mode: Mode::Dd,
            dd_switch_mse: None,
            ..EqConfig::new(NF, 2)
        };
        Ok(Equaliser {
            eq: AdaptiveEq::new(cfg, c.clone())?,
            carrier: CarrierSync::new(DecisionDirected::new(c), CarrierConfig::new(0.0225))?,
        })
    }

    /// The two input samples that end the window of symbol `k` (the window is centred on it).
    fn input(x: &[Complex32], ts: &dyn Fn(usize) -> f64, k: usize) -> [Complex32; 2] {
        let t = ts(k + LAG);
        [at(x, t - SPS / 2.0), at(x, t)]
    }

    /// Fills the window ahead of symbol `k` without adapting, and restarts the carrier loop.
    fn start(&mut self, x: &[Complex32], ts: &dyn Fn(usize) -> f64, k: usize) {
        self.carrier.reset();
        self.eq.set_mode(Mode::Frozen);
        for j in k.saturating_sub(LAG)..k {
            let _ = self.eq.step(&Self::input(x, ts, j), None);
        }
        self.eq.set_mode(Mode::Dd);
    }

    /// Symbol `k`: the output after the carrier loop and the decision. Adapts towards `known` when given, else towards the decision.
    fn step(
        &mut self,
        x: &[Complex32],
        ts: &dyn Fn(usize) -> f64,
        k: usize,
        known: Option<Complex32>,
        mu: f32,
    ) -> (Complex32, Complex32) {
        let _ = self.eq.set_mu(mu);
        let turn = TAU * self.carrier.phase();
        self.eq
            .set_rotation(Complex32::new(turn.cos() as f32, -(turn.sin() as f32)));
        let z = self
            .eq
            .step(&Self::input(x, ts, k), known)
            .unwrap_or_default();
        let d = self.eq.last_decision();
        let raw = z * Complex32::new(turn.cos() as f32, turn.sin() as f32);
        self.carrier.push_known(raw, d);
        (z, d)
    }
}

/// Demodulates and decodes a burst whose A1 starts near sample `t0` of `x` (channel samples after the matched filter, 4 per symbol),
/// with a coarse offset `f0`.
pub fn decode_burst(dec: &mut DataDecoder, x: &[Complex32], t0: f64, f0: f64) -> Option<Decoded> {
    decode_burst_ex(dec, x, t0, f0).ok()
}

/// [`decode_burst`]: `Err(true)` when the burst is not all in yet, `Err(false)` when the preamble is rejected.
pub fn decode_burst_ex(
    dec: &mut DataDecoder,
    x: &[Complex32],
    t0: f64,
    f0: f64,
) -> Result<Decoded, bool> {
    let a: Vec<f32> = phy::a_seq()
        .iter()
        .chain(phy::a_seq().iter())
        .copied()
        .collect();
    // Coherent offset search over +-250 Hz around the trial, then timing, then the phase step from A1 to A2.
    let mut best = (0.0f64, f0, t0);
    let mut f = f0 - 250.0;
    while f <= f0 + 250.0 {
        for dt in [-2.0, -1.0, 0.0, 1.0, 2.0] {
            let (c1, c2, e) = corr_a(x, t0 + dt, f, &a);
            let m = (c1 + c2).norm() as f64 / (e * a.len() as f64).sqrt().max(1e-30);
            if m > best.0 {
                best = (m, f, t0 + dt);
            }
        }
        f += 10.0;
    }
    let (_, mut fo, mut t) = best;
    let mut bm = 0.0;
    let mut tt = t;
    let mut k = -8;
    while k <= 8 {
        let tc = t + k as f64 / 4.0;
        let (c1, c2, e) = corr_a(x, tc, fo, &a);
        let m = (c1 + c2).norm() as f64 / e.sqrt().max(1e-30);
        if m > bm {
            bm = m;
            tt = tc;
        }
        k += 1;
    }
    t = tt;
    let (c1, c2, e) = corr_a(x, t, fo, &a);
    let dphi = (c2 * c1.conj()).arg() as f64;
    fo += dphi / TAU * SYMBOL_RATE / 127.0;
    let coh = (c1 + c2).norm() as f64 / (e * a.len() as f64).sqrt().max(1e-30);
    if coh < 0.3 {
        return Err(false);
    }
    // The rate from M1.
    let w = TAU * fo / SYMBOL_RATE;
    let mut bi = (0.0f64, 0usize);
    for i in 0..8 {
        let m = phy::m_seq(i);
        let mut c = Complex32::default();
        for (k, &mk) in m.iter().enumerate() {
            let kk = 254 + k;
            c += at(x, t + kk as f64 * SPS) * mk * rot(w, kk as f64);
        }
        if c.norm() as f64 > bi.0 {
            bi = (c.norm() as f64, i);
        }
    }
    let r = rate(bi.1);
    let total = r.total_symbols();
    if (t + total as f64 * SPS + 8.0) as usize > x.len() {
        return Err(true);
    }
    let known = preamble(r.index);
    // Gain and phase from the preamble.
    let mut g = Complex32::default();
    for (k, &s) in known.iter().enumerate() {
        g += at(x, t + k as f64 * SPS) * s * rot(w, k as f64);
    }
    g /= known.len() as f32;
    if g.norm_sqr() == 0.0 {
        return Err(false);
    }
    let level = 10.0 * (g.norm_sqr() as f64).max(1e-20).log10();
    let ginv = g.conj() / g.norm_sqr();
    // The samples turned by the offset and normalised.
    let n = (total as f64 * SPS + 16.0) as usize + 32;
    let start = (t - 64.0).max(0.0).floor() as usize;
    let seg: Vec<Complex32> = (0..n + 64)
        .map(|i| {
            let s = start + i;
            let v = x.get(s).copied().unwrap_or_default();
            v * ginv * rot(w / SPS, s as f64 - t)
        })
        .collect();
    let t1 = t - start as f64;
    let mut tries: Vec<Decoded> = Vec::new();
    for mode in 0..3 {
        let d = demod(dec, &seg, t1, r, &known, mode);
        let ok = !d.0.is_empty();
        let (msgs, why) = pdu::read(&d.0);
        let mut dec = Decoded {
            messages: msgs,
            why,
            rate: Some(r),
            snr_db: d.1,
            offset_hz: fo,
            level_db: level as f32,
            symbols: total,
            points: d.2,
        };
        if !ok {
            dec.why = vec![Why::Short];
        }
        if !dec.messages.is_empty() {
            return Ok(dec);
        }
        tries.push(dec);
    }
    tries.into_iter().next().ok_or(false)
}

/// The data octets and the SNR: `mode` 0 the equaliser, 1 gain and phase from the probes, 2 the probes with their own timing.
fn demod(
    dec: &mut DataDecoder,
    x: &[Complex32],
    t: f64,
    r: Rate,
    known: &[f32],
    mode: u8,
) -> (Vec<u8>, f32, Vec<Complex32>) {
    let mut pts: Vec<Complex32> = Vec::with_capacity(r.data_symbols());
    let mut wts: Vec<f32> = Vec::with_capacity(r.data_symbols());
    let mut err_known = 0.0f64;
    let mut n_known = 0usize;
    let ts = |k: usize| t + k as f64 * SPS;
    if mode == 0 {
        let Ok(mut eq) = Equaliser::new(r.bits) else {
            return (Vec::new(), 0.0, pts);
        };
        for mu in [0.25f32, 0.1] {
            eq.start(x, &ts, 0);
            for (k, &s) in known.iter().enumerate() {
                let (z, d) = eq.step(x, &ts, k, Some(Complex32::new(s, 0.0)), mu);
                if mu < 0.2 {
                    err_known += (z - d).norm_sqr() as f64;
                    n_known += 1;
                }
            }
        }
        // Data with decisions (the constellations are symmetric, so the scrambler's sign does not change them), probes known.
        let mut block_err = vec![0.0f32; r.blocks + 1];
        block_err[0] = (err_known / n_known.max(1) as f64) as f32;
        for blk in 0..r.blocks {
            let b0 = PREAMBLE + blk * 45;
            for j in 0..30 {
                let (z, _) = eq.step(x, &ts, b0 + j, None, 0.15);
                pts.push(z);
            }
            let mut pe = 0.0f32;
            for (j, &s) in T.iter().enumerate() {
                let (z, d) = eq.step(x, &ts, b0 + 30 + j, Some(Complex32::new(s, 0.0)), 0.5);
                pe += (z - d).norm_sqr();
            }
            block_err[blk + 1] = pe / 15.0;
        }
        for blk in 0..r.blocks {
            let e = 0.5 * (block_err[blk] + block_err[blk + 1]);
            for _ in 0..30 {
                wts.push(1.0 / e.max(1e-3));
            }
        }
    } else {
        // Gain and phase (and in mode 2, the timing) from the probes on both sides of each data block.
        let mut tshift = 0.0f64;
        let probe = |b0: usize, ts0: f64| -> (Complex32, f32) {
            let mut h = Complex32::default();
            for (j, &s) in T.iter().enumerate() {
                h += at(x, ts0 + (b0 + j) as f64 * SPS) * s;
            }
            h /= 15.0;
            let mut e = 0.0;
            for (j, &s) in T.iter().enumerate() {
                let z = at(x, ts0 + (b0 + j) as f64 * SPS)
                    / if h.norm_sqr() > 0.0 {
                        h
                    } else {
                        Complex32::new(1.0, 0.0)
                    };
                e += (z - s).norm_sqr();
            }
            (h, e / 15.0)
        };
        let mut prev = probe(PREAMBLE - 15, t);
        err_known = prev.1 as f64;
        n_known = 1;
        for blk in 0..r.blocks {
            let b0 = PREAMBLE + blk * 45;
            let mut tsb = t + tshift;
            if mode == 2 {
                // Follow the clock: the probe timing within +-1 sample.
                let mut best = (0.0f32, 0.0f64);
                for k in -4..=4 {
                    let d = k as f64 / 4.0;
                    let (h, _) = probe(b0 + 30, t + tshift + d);
                    if h.norm() > best.0 {
                        best = (h.norm(), d);
                    }
                }
                tshift += 0.5 * best.1;
                tsb = t + tshift;
            }
            let next = probe(b0 + 30, tsb);
            for j in 0..30 {
                let a = (j as f32 + 8.0) / 45.0;
                let h = prev.0 * (1.0 - a) + next.0 * a;
                let z = at(x, tsb + (b0 + j) as f64 * SPS)
                    / if h.norm_sqr() > 0.0 {
                        h
                    } else {
                        Complex32::new(1.0, 0.0)
                    };
                pts.push(z);
                wts.push(1.0 / (0.5 * (prev.1 + next.1)).max(1e-3));
            }
            err_known += next.1 as f64;
            n_known += 1;
            prev = next;
        }
    }
    let mut out = Vec::new();
    dec.decode(&pts, &wts, r, &mut out);
    // A PDU is padded to the burst: an MPDU or SPDU carries its own length. Keep it all.
    out.truncate(r.octets());
    let snr = -10.0 * (err_known / n_known.max(1) as f64).max(1e-6).log10();
    (out, snr as f32, pts)
}

impl Front {
    fn build(&mut self) {
        self.chans.clear();
        self.out_of_band.clear();
        self.bank = None;
        self.bands.clear();
        if !(self.rate.is_finite() && self.rate >= MIN_RATE_HZ) {
            return;
        }
        let a = phy::a_seq();
        self.a2 = a.iter().chain(a.iter()).copied().collect();
        self.steps = self.a2.windows(2).map(|w| w[0] * w[1]).collect();
        // Nine trial offsets, -400 .. +400 Hz.
        self.trials = (-4..=4)
            .map(|k| {
                let f = k as f64 * 100.0;
                let w = TAU * f / SYMBOL_RATE;
                self.a2
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| rot(w, i as f64) * v)
                    .collect()
            })
            .collect();
        let dials = plan(&self.channels, self.whole_span, self.centre, self.rate);
        let n_in = dials.iter().filter(|d| d.1).count();
        // Wide captures with several channels: the shared channeliser, each channel then working from its sub-band.
        let mut band_list: Vec<usize> = Vec::new();
        let probe = (self.shared && self.rate > super::bank::SHARED_ABOVE_HZ && n_in >= 3)
            .then(|| super::bank::PolyBank::new(self.rate, vec![]).ok())
            .flatten();
        for (d, inb) in dials {
            if !inb {
                self.out_of_band.push(d);
                continue;
            }
            let off = d + CARRIER_HZ - self.centre;
            let (in_rate, in_off, slot) = match &probe {
                Some(b) => {
                    let (k, rest) = b.band_of(off);
                    let slot = band_list.iter().position(|&x| x == k).unwrap_or_else(|| {
                        band_list.push(k);
                        band_list.len() - 1
                    });
                    (b.band_rate(), rest, Some(slot))
                }
                None => (self.rate, off, None),
            };
            let mut cfg =
                ChannelConfig::new(in_rate, in_off, self.filter_hz.clamp(2600.0, 5600.0), FS);
            cfg.exact_rate = true;
            self.chans.push(Chan {
                dial: d,
                chan: Channelizer::new(cfg).ok(),
                rrc: matched_filter(),
                hist: Vec::with_capacity(HIST),
                dstep: Vec::with_capacity(HIST),
                cum: Vec::with_capacity(HIST),
                base: 0,
                n: 0,
                resume: 0,
                scan_at: 0,
                pending: None,
                peak: None,
                st: ChannelState {
                    dial_hz: d,
                    station: stations::on_khz((d / 1e3).round() as u32).map(|s| s.id),
                    in_band: true,
                    ..Default::default()
                },
                near_dc: off.abs() < 2.5e3,
                slot,
            });
        }
        if probe.is_some() {
            self.bands = vec![Vec::new(); band_list.len()];
            self.bank = super::bank::PolyBank::new(self.rate, band_list).ok();
        }
    }
}

impl Chan {
    fn search(
        &mut self,
        trials: &[Vec<Complex32>],
        steps: &[f32],
        dec: &mut DataDecoder,
        out: &mut Vec<(u64, BurstOut)>,
        why: &mut [u64; 6],
    ) {
        let span = (254.0 * SPS) as u64;
        let thr = if self.near_dc { 0.52 } else { 0.50 };
        let mut i = self.scan_at.max(self.base + span);
        while i < self.n && self.pending.is_none() {
            if !i.is_multiple_of(2) || i - span < self.resume {
                i += 1;
                continue;
            }
            let s0 = i - span;
            let off = (s0 - self.base) as usize;
            let h = &self.hist[off..];
            // Energy over the 254 symbols.
            let e = self.cum[off + 4 * 253] - if off >= 4 { self.cum[off - 4] } else { 0.0 };
            // A cheap first look: the symbol-to-symbol phase steps against those of A1 A2 (blind to the carrier offset). Noise alone
            // measures about 0.06. A burst at -3 dB about 0.3. Only a look above 0.12 (or a peak being followed) runs the nine
            // trials.
            if self.peak.is_none() {
                let d = &self.dstep[off..];
                let mut acc = Complex32::default();
                for (k, &c) in steps.iter().enumerate() {
                    acc += d[k * 4] * c;
                }
                if (acc.norm() as f64) < 0.12 * e {
                    i += 1;
                    continue;
                }
            }
            let mut best = (0.0f64, 0usize);
            for (ti, tr) in trials.iter().enumerate() {
                let mut sum = 0.0f64;
                for p in 0..32 {
                    let mut c = Complex32::default();
                    for k in p * 8..((p + 1) * 8).min(254) {
                        c += h[k * 4] * tr[k];
                    }
                    sum += c.norm() as f64;
                }
                let m = sum / (254.0 * e).sqrt().max(1e-30);
                if m > best.0 {
                    best = (m, ti);
                }
            }
            match self.peak {
                None => {
                    if best.0 >= thr {
                        self.peak = Some((best.0, s0, (best.1 as f64 - 4.0) * 100.0));
                    }
                }
                Some((pm, ps, pf)) => {
                    if best.0 > pm {
                        self.peak = Some((best.0, s0, (best.1 as f64 - 4.0) * 100.0));
                    } else if s0 > ps + 600 {
                        self.peak = None;
                        self.pending = Some((ps, pf));
                        self.st.preambles += 1;
                    }
                }
            }
            i += 1;
        }
        self.scan_at = i;
        // Decode the pending burst once it is all in (the longest takes 4.5 s).
        if let Some((s0, f0)) = self.pending {
            let need_pre = s0 + ((PREAMBLE + 10) as f64 * SPS) as u64;
            if self.n < need_pre {
                return;
            }
            let t0 = (s0 - self.base) as f64;
            // The rate decides how much is needed: try once the shortest is in, else wait for the longest.
            let one = s0 + ((rate(0).total_symbols() + 4) as f64 * SPS) as u64 + 16;
            let two = s0 + ((rate(4).total_symbols() + 4) as f64 * SPS) as u64 + 16;
            if self.n < one {
                return;
            }
            let r = match decode_burst_ex(dec, &self.hist, t0, f0) {
                Err(true) if self.n < two => return,
                Err(_) => None,
                Ok(d) => Some(d),
            };
            self.pending = None;
            match r.map(|d| (d.rate.filter(|_| !d.messages.is_empty()), d)) {
                Some((Some(rr), d)) => {
                    self.resume = s0 + ((d.symbols.saturating_sub(40)) as f64 * SPS) as u64;
                    // The burst starts with its prekey, before A1.
                    let time = s0 as f64 / FS - phy::PREKEY as f64 / SYMBOL_RATE;
                    self.st.good += 1;
                    self.st.last_heard_s = time;
                    self.st.level_db = d.level_db;
                    self.st.snr_db = d.snr_db;
                    self.st.last_offset_hz = d.offset_hz as f32;
                    self.st.last_bps = rr.bps;
                    for w in &d.why {
                        why[*w as usize + 1] += 1;
                    }
                    let ready = if rr.slots == 1 { one } else { two };
                    out.push((
                        ready,
                        BurstOut {
                            time_s: time,
                            dial_hz: self.dial,
                            level_db: d.level_db,
                            snr_db: d.snr_db,
                            offset_hz: d.offset_hz,
                            bps: rr.bps,
                            slots: rr.slots,
                            messages: d.messages,
                        },
                    ));
                }
                Some((None, d)) => {
                    self.st.bad += 1;
                    for w in &d.why {
                        why[*w as usize + 1] += 1;
                    }
                    self.resume = s0 + (254.0 * SPS) as u64;
                }
                None => {
                    why[0] += 1;
                    self.resume = s0 + (254.0 * SPS) as u64;
                }
            }
        }
    }
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.hfdl_front",
            vec![Port::iq("in")],
            vec![
                Port::typed::<BurstOut>("bursts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FrontState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FrontSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.idx = 0;
        self.why = [0; 6];
        self.build();
        Ok(vec![
            Format::new(Kind::typed::<BurstOut>(), 0.0),
            Format::new(Kind::typed::<FrontState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let trials = std::mem::take(&mut self.trials);
        if let Some(bank) = self.bank.as_mut() {
            self.bands.iter_mut().for_each(Vec::clear);
            bank.process(x, &mut self.bands);
        }
        for c in self.chans.iter_mut() {
            self.tmp.clear();
            let input: &[Complex32] = match c.slot {
                Some(k) => &self.bands[k],
                None => x,
            };
            if let Some(ch) = c.chan.as_mut() {
                ch.process(input, &mut self.tmp);
            }
            for &v in &self.tmp {
                c.hist.push(c.rrc.process_sample(v));
                c.n += 1;
            }
            for j in c.cum.len()..c.hist.len() {
                let p = c.hist[j].norm_sqr() as f64 + if j >= 4 { c.cum[j - 4] } else { 0.0 };
                c.cum.push(p);
            }
            for j in c.dstep.len()..c.hist.len().saturating_sub(4) {
                c.dstep.push(c.hist[j + 4] * c.hist[j].conj());
            }
            if c.hist.len() > HIST {
                let keep_from = c
                    .pending
                    .map_or(c.n.saturating_sub(HIST as u64 - 1000), |p| {
                        p.0.saturating_sub(64)
                    })
                    .max(c.base);
                let drop = (keep_from - c.base) as usize;
                let drop = drop.min(c.hist.len().saturating_sub(4096));
                c.hist.drain(..drop);
                c.dstep.drain(..drop.min(c.dstep.len()));
                c.cum.drain(..drop);
                c.base += drop as u64;
            }
            c.search(
                &trials,
                &self.steps,
                &mut self.decoder,
                &mut self.bursts,
                &mut self.why,
            );
        }
        self.trials = trials;
        self.idx += x.len() as u64;
        if !self.bursts.is_empty() {
            // In the order they became decodable, then by channel: the same whatever the cuts.
            self.bursts
                .sort_by(|a, b| a.0.cmp(&b.0).then(a.1.dial_hz.total_cmp(&b.1.dial_hz)));
            let o = io.items_out::<BurstOut>(0);
            for (_, b) in self.bursts.drain(..) {
                o.push(Timed {
                    time: b.time_s,
                    value: b,
                });
            }
        }
        if io.tick() {
            let mut chs: Vec<ChannelState> = self.chans.iter().map(|c| c.st.clone()).collect();
            for &d in &self.out_of_band {
                chs.push(ChannelState {
                    dial_hz: d,
                    station: stations::on_khz((d / 1e3).round() as u32).map(|s| s.id),
                    ..Default::default()
                });
            }
            chs.sort_by(|a, b| a.dial_hz.total_cmp(&b.dial_hz));
            let t = if self.rate.is_finite() && self.rate > 0.0 {
                self.idx as f64 / self.rate
            } else {
                0.0
            };
            io.items_out::<FrontState>(1).push(Timed {
                time: t,
                value: FrontState {
                    time_s: t,
                    rate_hz: self.rate,
                    centre_hz: self.centre,
                    channels: chs,
                    why: self.why,
                },
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::Channels => {
                let t = CHANNELS.read(id, v)?;
                channel_list(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                t.clone_into(&mut self.channels);
            }
            FrontSetting::WholeSpan => self.whole_span = WHOLE_SPAN.read(id, v)?,
            FrontSetting::SharedChanneliser => self.shared = SHARED_CHANNELISER.read(id, v)?,
            FrontSetting::ChannelFilterHz => self.filter_hz = CHANNEL_FILTER_HZ.read(id, v)?,
        }
        if self.rate > 0.0 {
            self.build();
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (FrontSetting::Channels, Value::Text(self.channels.clone())),
            (FrontSetting::WholeSpan, Value::Bool(self.whole_span)),
            (FrontSetting::SharedChanneliser, Value::Bool(self.shared)),
            (FrontSetting::ChannelFilterHz, Value::Hz(self.filter_hz)),
        ])
    }
}

/// The channel samples (carrier to 0 Hz, 7200 Hz, matched filter) of a dial in a capture, for tools and tests.
pub fn channel_samples(rate: f64, centre: f64, dial: f64, x: &[Complex32]) -> Vec<Complex32> {
    let mut cfg = ChannelConfig::new(rate, dial + CARRIER_HZ - centre, 3400.0, FS);
    cfg.exact_rate = true;
    let Ok(mut ch) = Channelizer::new(cfg) else {
        return vec![];
    };
    let mut y = Vec::new();
    ch.process(x, &mut y);
    let mut f = matched_filter();
    y.iter().map(|&v| f.process_sample(v)).collect()
}

/// The bursts in a whole buffer of channel samples: (A1 start, coarse offset), as the receiver's search finds them.
pub fn find_preambles(x: &[Complex32]) -> Vec<(f64, f64)> {
    let a = phy::a_seq();
    let a2: Vec<f32> = a.iter().chain(a.iter()).copied().collect();
    let trials: Vec<Vec<Complex32>> = (-4..=4)
        .map(|k| {
            let w = TAU * (k as f64 * 100.0) / SYMBOL_RATE;
            a2.iter()
                .enumerate()
                .map(|(i, &v)| rot(w, i as f64) * v)
                .collect()
        })
        .collect();
    let mut out = Vec::new();
    let mut i = 1016usize;
    let mut peak: Option<(f64, usize, f64)> = None;
    while i + 1 < x.len() {
        let s0 = i - 1016;
        let h = &x[s0..];
        let mut e = 0.0f64;
        for k in 0..254 {
            e += h[k * 4].norm_sqr() as f64;
        }
        let mut best = (0.0f64, 0usize);
        for (ti, tr) in trials.iter().enumerate() {
            let mut sum = 0.0f64;
            for p in 0..32 {
                let mut c = Complex32::default();
                for k in p * 8..((p + 1) * 8).min(254) {
                    c += h[k * 4] * tr[k];
                }
                sum += c.norm() as f64;
            }
            let m = sum / (254.0 * e).sqrt().max(1e-30);
            if m > best.0 {
                best = (m, ti);
            }
        }
        match peak {
            None if best.0 >= 0.5 => peak = Some((best.0, s0, (best.1 as f64 - 4.0) * 100.0)),
            Some((pm, ps, pf)) => {
                if best.0 > pm {
                    peak = Some((best.0, s0, (best.1 as f64 - 4.0) * 100.0));
                } else if s0 > ps + 600 {
                    out.push((ps as f64, pf));
                    peak = None;
                    i = ps + 1016 + 254 * 4;
                }
            }
            _ => {}
        }
        i += 2;
    }
    out
}
