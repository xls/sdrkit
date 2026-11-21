//! The VDL Mode 2 front end: the channel plan, the channel bank (with a shared first stage), the burst search on the training
//! sequence, the burst demodulator (the shared timing and carrier loops) and decoder, and the tuning error tracker.
//! Standard: ICAO Doc 9776 clause 6 (D8PSK at 10.5 ksymbol/s, the burst format).

use super::phy::{self, read_header, BodyDecoder, SYMBOL_RATE, TRAINING};
use super::{BankState, Burst, ChannelState};
use super::{CHANNELS, TRACK_TUNING, TUNE_ERROR_HZ};
use crate::filters::blackman_lowpass;
use crate::history::History;
use crate::report::freq_list;
use sdrkit_algo_dsp::cic::Cic;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::interp::{CatmullRom, Reader};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, DecisionDirected};
use sdrkit_algo_modem::timing::{Gardner, SymbolSync, SymbolSyncConfig};
use sdrkit_algo_modem::{Constellation, PointSet};
use sdrkit_core::{Complex32, ConfigError, SettingError, SettingId, SettingSet, Value, Values};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, SlicePool,
    Timed,
};
use sdrkit_proto_link::hdlc::{Line, Receiver};
use sdrkit_util_bits::hdlc::HdlcDeframer;
use std::f64::consts::{FRAC_PI_4, TAU};

/// The sample of `x` at fractional position `t` (Catmull-Rom, zero outside).
#[inline]
fn at(x: &[Complex32], t: f64) -> Complex32 {
    Reader::new(CatmullRom).at(x, t)
}

sdrkit_core::settings! {
    /// The settings of [`Front`].
    pub enum FrontSetting {
        /// The channel list.
        Channels = CHANNELS,
        /// Follow the radio's tuning error.
        TrackTuning = TRACK_TUNING,
        /// The tuning error to start from.
        TuneErrorHz = TUNE_ERROR_HZ,
    }
}

/// Reads the `channels` setting: frequencies in MHz in the VHF band 118 to 137 MHz.
///
/// # Errors
///
/// The byte position of the first word that is not such a frequency.
pub fn channel_list(t: &str) -> Result<Vec<f64>, usize> {
    freq_list(t, 1e6, (117.975e6, 137e6))
}

/// The common signalling channel of VDL Mode 2 (ICAO Annex 10 Volume III), MHz: watched when the list is empty.
pub const CSC_MHZ: f64 = 136.975;
/// The channels to watch (Hz, sorted, no duplicates), each with whether it lies inside the capture: the list, or, when it
/// is empty, the common signalling channel and the frequency `station` that the session is tuned to (left out within
/// 12.5 kHz of the common signalling channel or when not above 0).
pub fn plan(list: &str, centre: f64, station: f64, rate: f64) -> Vec<(f64, bool)> {
    let listed: Vec<f64> = channel_list(list).unwrap_or_default();
    let in_band = |f: f64| (f - centre).abs() < rate / 2.0 - 14e3;
    let mut v: Vec<f64> = if listed.is_empty() {
        let csc = (CSC_MHZ * 1e6).round();
        let mut v = vec![csc];
        if station.is_finite() && station > 0.0 && (station - csc).abs() > 12.5e3 {
            v.push(station);
        }
        v
    } else {
        listed
    };
    v.sort_by(|a, b| a.total_cmp(b));
    v.dedup();
    v.into_iter().map(|f| (f, in_band(f))).collect()
}

const HIST: usize = 65_536;

struct Pending {
    /// Channel history index of training symbol 0.
    at: f64,
    coarse_hz: f64,
    len: Option<u32>,
}

struct Chan {
    freq: f64,
    nco: Nco,
    sums: Cic<Complex32>,
    fir: FirDecimator<Complex32>,
    search: Fir<Complex32>,
    decode: Fir<Complex32>,
    seg: Vec<Complex32>,
    filt: Vec<Complex32>,
    hist: History,
    /// Channel samples produced.
    n: u64,
    floor: f64,
    blk_sum: f64,
    blk_n: usize,
    peak: Option<(f64, u64, f64, u32)>,
    pending: std::collections::VecDeque<Pending>,
    resume: u64,
    st: ChannelState,
    scratch: Vec<Complex32>,
    scratch2: Vec<Complex32>,
    last_good_s: f64,
    /// The training symbols' sample offsets (round(k sps)) and phase turns, for the search.
    offs: [usize; 16],
    turns: [Complex32; 16],
}

/// The bank, search, demodulator and decoder for every channel in band.
pub struct Front {
    channels: String,
    state_scratch: Vec<ChannelState>,
    state_pool: SlicePool<ChannelState>,
    track: bool,
    guess: f64,
    rate: f64,
    centre: f64,
    /// Where the session is tuned, Hz.
    station: f64,
    chans: Vec<Chan>,
    out_of_band: Vec<f64>,
    shared: Option<(Nco, FirDecimator<Complex32>)>,
    r1: f64,
    fc: f64,
    sps: f64,
    tune_err: f64,
    /// The good bursts' offsets since the tuning error was last applied (sum, count): order-free, so the cuts do not matter.
    offsets: (f64, u32),
    idx: u64,
    s0: Vec<Complex32>,
    s1: Vec<Complex32>,
    s2: Vec<Complex32>,
    s3: Vec<Complex32>,
    s4: Vec<Complex32>,
    /// Decoded bursts with the channel sample at which each became decodable (the order they are given in).
    bursts: Vec<(u64, Burst)>,
    decode_taps: Vec<f32>,
    burst_found: bool,
    decoder: BurstDecoder,
    refused: String,
}

impl Default for Front {
    fn default() -> Self {
        Front {
            channels: String::new(),
            state_scratch: Vec::new(),
            state_pool: SlicePool::default(),
            track: TRACK_TUNING.default,
            guess: TUNE_ERROR_HZ.default,
            rate: 0.0,
            centre: 0.0,
            station: 0.0,
            chans: Vec::new(),
            out_of_band: Vec::new(),
            shared: None,
            r1: 0.0,
            fc: 0.0,
            sps: 0.0,
            tune_err: 0.0,
            offsets: (0.0, 0),
            idx: 0,
            s0: Vec::new(),
            s1: Vec::new(),
            s2: Vec::new(),
            s3: Vec::new(),
            s4: Vec::new(),
            bursts: Vec::new(),
            decode_taps: Vec::new(),
            burst_found: false,
            decoder: BurstDecoder::default(),
            refused: String::new(),
        }
    }
}

impl Front {
    fn build(&mut self) {
        self.chans.clear();
        self.out_of_band.clear();
        self.shared = None;
        self.refused.clear();
        if !(self.rate.is_finite() && self.rate >= super::MIN_RATE_HZ) {
            self.refused = format!(
                "sample rate {:.0} kHz is too low, use at least {:.0} kHz",
                self.rate.max(0.0) / 1e3,
                super::MIN_RATE_HZ / 1e3
            );
            return;
        }
        let fs = self.rate;
        let p = plan(&self.channels, self.centre, self.station, fs);
        let inb: Vec<f64> = p.iter().filter(|c| c.1).map(|c| c.0).collect();
        self.out_of_band = p.iter().filter(|c| !c.1).map(|c| c.0).collect();
        let (Some(&lo), Some(&hi)) = (inb.first(), inb.last()) else {
            return;
        };
        let da = ((fs / 320e3).floor() as usize).max(1);
        let mid = (lo + hi) / 2.0;
        let half = (hi - lo) / 2.0 + 24e3;
        // The shared first stage: the largest factor up to Da whose output still holds the group with a margin.
        let ds = if da >= 3 && inb.len() >= 2 {
            da.min((fs / (2.0 * (half + 40e3))).floor() as usize)
        } else {
            1
        };
        let (r_in, mixoff) = if ds >= 2 {
            let r1 = fs / ds as f64;
            let taps = sdrkit_algo_dsp::fir::lowpass(fs, half, r1 - half - 16e3, 70.0)
                .expect("the filter settings are valid");
            self.shared = Some((
                Nco::new(fs, -(mid - self.centre)),
                FirDecimator::new(&taps, ds),
            ));
            (r1, mid)
        } else {
            (fs, self.centre)
        };
        let d1 = if ds >= 2 {
            ((r_in / 320e3).floor() as usize).max(1)
        } else {
            da
        };
        let stage = r_in / d1 as f64;
        let db = ((stage / 80e3).floor() as usize).max(1);
        let fc = stage / db as f64;
        let trans = (fc - 16.6e3 - 22e3).max(5e3);
        let ntaps = ((6.6 * stage / trans) as usize).clamp(15, 401) | 1;
        let taps = blackman_lowpass(stage, 22e3 + trans / 2.0, ntaps);
        self.r1 = r_in;
        self.fc = fc;
        self.sps = fc / SYMBOL_RATE;
        let search_taps = blackman_lowpass(fc, 15e3, ((6.6 * fc / 6e3) as usize) | 1);
        self.decode_taps = blackman_lowpass(fc, 7.5e3, ((6.6 * fc / 3.4e3) as usize) | 1);
        for f in inb {
            let sums = match Cic::new(3, d1) {
                Ok(c) => c,
                Err(e) => {
                    self.chans.clear();
                    self.refused = e.to_string();
                    return;
                }
            };
            self.chans.push(Chan {
                freq: f,
                nco: Nco::new(r_in, -(f - mixoff + self.tune_err)),
                sums,
                fir: FirDecimator::new(&taps, db),
                search: Fir::new(&search_taps),
                decode: Fir::new(&self.decode_taps),
                seg: Vec::new(),
                filt: Vec::new(),
                hist: History::new(HIST),
                n: 0,
                floor: 0.0,
                blk_sum: 0.0,
                blk_n: 0,
                peak: None,
                pending: Default::default(),
                resume: 0,
                st: ChannelState {
                    freq_hz: f,
                    in_band: true,
                    ..Default::default()
                },
                scratch: Vec::new(),
                scratch2: Vec::new(),
                last_good_s: f64::NEG_INFINITY,
                offs: std::array::from_fn(|k| (k as f64 * self.sps).round() as usize),
                turns: training_turns(),
            });
        }
    }

    fn retune_channels(&mut self) {
        let mixoff = if self.shared.is_some() {
            let lo = self.chans.first().map_or(0.0, |c| c.freq);
            let hi = self.chans.last().map_or(0.0, |c| c.freq);
            (lo + hi) / 2.0
        } else {
            self.centre
        };
        for c in self.chans.iter_mut() {
            c.nco.set_freq(-(c.freq - mixoff + self.tune_err));
        }
    }

    fn state(&mut self) -> BankState {
        let chans = &mut self.state_scratch;
        chans.clear();
        chans.extend(self.chans.iter().map(|c| c.st));
        for &f in &self.out_of_band {
            chans.push(ChannelState {
                freq_hz: f,
                in_band: false,
                ..Default::default()
            });
        }
        chans.sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
        let chans = self
            .state_pool
            .exact(chans.len(), |o| o.copy_from_slice(&self.state_scratch));
        BankState {
            time_s: if self.rate > 0.0 {
                self.idx as f64 / self.rate
            } else {
                0.0
            },
            rate_hz: self.rate,
            centre_hz: self.centre,
            tune_error_hz: self.tune_err,
            channels: chans,
            burst_found: self.burst_found,
            refused: self.refused.clone(),
        }
    }
}

/// The differential correlation of the training sequence at `t` (fractional index in `y`): (sum, energy, mean power, first4, last4).
/// The training sequence's phase steps turned back: `e^(-j pi/4 (T[k] - T[k-1]))` for k = 1 .. 15 (index 0 unused).
fn training_turns() -> [Complex32; 16] {
    crate::d8psk::turns(|k| TRAINING[k] - TRAINING[k - 1])
}

/// The search's look at one sample: the 16 samples a symbol apart from `j` in `x` (at the integer offsets `offs`); `None` when the
/// energy test fails (most samples: then no correlation is made), else (differential correlation, energy).
#[inline]
fn train_look(
    x: &[Complex32],
    j: usize,
    offs: &[usize; 16],
    turns: &[Complex32; 16],
    floor: f64,
) -> Option<(Complex32, f64)> {
    let mut v = [Complex32::default(); 16];
    let mut pw = [0.0f32; 16];
    for k in 0..16 {
        v[k] = x[j + offs[k]];
        pw[k] = v[k].norm_sqr();
    }
    let sum: f32 = pw.iter().sum();
    let mean = sum as f64 / 16.0;
    let f4 = (pw[0] + pw[1] + pw[2] + pw[3]) as f64 / 4.0;
    let l4 = (pw[12] + pw[13] + pw[14] + pw[15]) as f64 / 4.0;
    if !(mean > 3.0 * floor && f4 >= 0.3 * mean && l4 >= 0.3 * mean) {
        return None;
    }
    let mut s = Complex32::default();
    let mut e = 0.0f32;
    for k in 1..16 {
        s += v[k] * v[k - 1].conj() * turns[k];
        e += (pw[k] * pw[k - 1]).sqrt();
    }
    (e > 0.0).then_some((s, e as f64))
}

fn train_corr(y: &dyn Fn(f64) -> Complex32, t: f64, sps: f64) -> (Complex32, f64, f64, f64, f64) {
    let mut s = Complex32::default();
    let mut e = 0.0f64;
    let mut prev = y(t);
    let mut pw = [0.0f64; 16];
    pw[0] = prev.norm_sqr() as f64;
    for k in 1..16 {
        let cur = y(t + k as f64 * sps);
        pw[k] = cur.norm_sqr() as f64;
        let d = (TRAINING[k] - TRAINING[k - 1]) as f64 * FRAC_PI_4;
        let rot = Complex32::new(d.cos() as f32, -(d.sin() as f32));
        s += cur * prev.conj() * rot;
        e += (pw[k] * pw[k - 1]).sqrt();
        prev = cur;
    }
    let mean = pw.iter().sum::<f64>() / 16.0;
    let f4 = pw[..4].iter().sum::<f64>() / 4.0;
    let l4 = pw[12..].iter().sum::<f64>() / 4.0;
    (s, e, mean, f4, l4)
}

/// A burst decoded from the samples of one channel.
pub struct Decoded {
    /// Good frames (FCS removed).
    pub frames: Vec<Vec<u8>>,
    /// Frames whose FCS failed.
    pub fcs_bad: u32,
    /// Octets corrected.
    pub corrected: usize,
    /// The header needed its FEC.
    pub header_fixed: bool,
    /// The bad reason when nothing passed.
    pub bad: bool,
    /// Carrier offset of the burst, Hz.
    pub offset_hz: f64,
    /// Signal power over decision error power, dB.
    pub snr_db: f64,
    /// Amplitude squared, dB.
    pub level_db: f64,
    /// Symbols used.
    pub symbols: usize,
    /// The transmission length.
    pub len: u32,
    /// The header was refused.
    pub header_refused: bool,
}

impl Decoded {
    fn refused() -> Self {
        Decoded {
            frames: vec![],
            fcs_bad: 0,
            corrected: 0,
            header_fixed: false,
            bad: true,
            offset_hz: 0.0,
            snr_db: 0.0,
            level_db: 0.0,
            symbols: 0,
            len: 0,
            header_refused: true,
        }
    }
}

/// The burst demodulator and decoder, with what it keeps from burst to burst (the Reed-Solomon codes, the HDLC receiver and the
/// buffers).
pub struct BurstDecoder {
    body: BodyDecoder,
    hdlc: Receiver,
    bits: Vec<u8>,
    decoded: phy::Body,
    /// The timing loop and its samples per symbol.
    sync: Option<(f64, SymbolSync<Complex32, Gardner, CatmullRom>)>,
    carrier: CarrierSync<DecisionDirected>,
    symbols: Vec<Complex32>,
}

impl Default for BurstDecoder {
    fn default() -> Self {
        let psk8 = Constellation::new(PointSet::psk(8, 0.0).expect("8PSK is a valid set"))
            .expect("8PSK is a valid constellation");
        let mut cc = CarrierConfig::new(0.038);
        cc.zeta = 0.95;
        cc.max_freq = 0.2;
        BurstDecoder {
            sync: None,
            carrier: CarrierSync::new(DecisionDirected::new(psk8), cc)
                .expect("the carrier loop settings are valid"),
            symbols: Vec::with_capacity(4096),
            body: BodyDecoder::default(),
            // Frames hold at least the two addresses, the control octet and the FCS (Doc 9776 clause 5.2).
            hdlc: Receiver::with_deframer(Line::Nrz, HdlcDeframer::x25(9, 2048).deliver_bad(true)),
            bits: Vec::with_capacity(4096),
            decoded: phy::Body::default(),
        }
    }
}

impl BurstDecoder {
    /// Demodulates and decodes a burst whose training starts at `t0` in `x` (channel samples, already turned by the coarse
    /// offset and filtered by the decoding filter). `None` when the training is not coherent or the samples end too soon.
    pub fn demod(&mut self, x: &[Complex32], t0: f64, sps: f64) -> Option<Decoded> {
        self.demod_mode(x, t0, sps, false)
    }

    /// [`BurstDecoder::demod`], stopping after the header when `header_only` (the length is in [`Decoded::len`]).
    pub fn demod_mode(
        &mut self,
        x: &[Complex32],
        t0: f64,
        sps: f64,
        header_only: bool,
    ) -> Option<Decoded> {
        let y = |t: f64| at(x, t);
        // Fine timing: -3 .. +3 samples in eighth-sample steps.
        let mut best = (0.0f64, t0, Complex32::default(), 0.0f64);
        for k in -24..=24 {
            let t = t0 + k as f64 / 8.0;
            let (s, e, ..) = train_corr(&y, t, sps);
            let m = s.norm() as f64 / e.max(1e-30);
            if m > best.0 {
                best = (m, t, s, e);
            }
        }
        let (_, t, s, e) = best;
        if (s.norm() as f64) < 0.5 * e {
            return None;
        }
        // Radians per symbol.
        let w = s.arg() as f64;
        // The symbols through the shared timing loop, started at the training's first symbol.
        if self.sync.as_ref().is_none_or(|(at, _)| *at != sps) {
            let mut cfg = SymbolSyncConfig::new(sps);
            cfg.bn_t = 0.01;
            cfg.pulse = Shape::Rc { beta: 0.6 };
            cfg.max_ppm = 5000.0;
            self.sync = Some((sps, SymbolSync::new(Gardner, CatmullRom, cfg).ok()?));
        }
        let (_, sync) = self.sync.as_mut()?;
        self.symbols.clear();
        sync.run(x, t, &mut self.symbols).ok()?;
        if self.symbols.len() < 16 {
            return None;
        }
        // Gain and phase from the de-rotated training symbols.
        let mut g = Complex32::default();
        for (k, &tp) in TRAINING.iter().enumerate() {
            let ph = tp as f64 * FRAC_PI_4 + w * k as f64;
            g += self.symbols[k] * Complex32::new(ph.cos() as f32, -(ph.sin() as f32));
        }
        g /= 16.0;
        if g.norm_sqr() == 0.0 {
            return None;
        }
        let ginv = g.conj() / g.norm_sqr();
        self.carrier.reset();
        self.carrier.set_freq(w / TAU);
        let mut prev_dec = 0i32;
        let mut sig = 0.0f64;
        let mut errp = 0.0f64;
        self.bits.clear();
        let mut scr = phy::scrambler();
        let mut need = 9usize;
        let mut header: Option<(u32, bool)> = None;
        let mut n_sym = 0usize;
        let mut kk = 0usize;
        loop {
            if kk >= 16 + need {
                if header.is_none() {
                    let Some(h) = read_header(&self.bits) else {
                        return Some(Decoded::refused());
                    };
                    if header_only {
                        return Some(Decoded {
                            len: h.0,
                            header_refused: false,
                            ..Decoded::refused()
                        });
                    }
                    header = Some(h);
                    need = phy::symbols_for(h.0);
                    if kk >= 16 + need {
                        break;
                    }
                    continue;
                }
                break;
            }
            // The last symbols need samples past the end: the burst is not all in yet.
            if kk + 2 >= self.symbols.len() {
                return None;
            }
            let raw = self.symbols[kk] * ginv;
            let (z, dec) = if kk < 16 {
                let dph = TRAINING[kk] as f64 * FRAC_PI_4;
                let known = Complex32::new(dph.cos() as f32, dph.sin() as f32);
                (self.carrier.push_known(raw, known), TRAINING[kk])
            } else {
                let z = self.carrier.push(raw);
                (z, (z.arg() as f64 / FRAC_PI_4).round() as i32)
            };
            let dph = dec as f64 * FRAC_PI_4;
            let dz = Complex32::new(dph.cos() as f32, dph.sin() as f32);
            sig += 1.0;
            errp += (z - dz).norm_sqr() as f64;
            if kk >= 16 {
                let step = (dec - prev_dec).rem_euclid(8) as u8;
                let b = phy::bits_of(step);
                for i in (0..3).rev() {
                    self.bits.push(((b >> i) & 1) ^ scr.bit());
                }
                n_sym += 1;
            }
            prev_dec = dec;
            kk += 1;
        }
        let (len, hfix) = header?;
        let mut body = std::mem::take(&mut self.decoded);
        self.body.decode(&self.bits[25..], len, &mut body);
        let mut frames = Vec::new();
        let mut fcs_bad = 0;
        self.hdlc.reset();
        for &b in &body.hdlc {
            if let Some(end) = self.hdlc.push(b) {
                if end.fcs_ok {
                    frames.push(self.hdlc.frame().to_vec());
                } else {
                    fcs_bad += 1;
                }
            }
        }
        let amp2 = g.norm_sqr() as f64;
        let d = Decoded {
            bad: body.failed || frames.is_empty(),
            frames,
            fcs_bad,
            corrected: body.corrected,
            header_fixed: hfix,
            offset_hz: w * SYMBOL_RATE / TAU,
            snr_db: 10.0 * (sig / errp.max(1e-12)).log10(),
            level_db: 10.0 * amp2.max(1e-20).log10(),
            symbols: 16 + n_sym,
            len,
            header_refused: false,
        };
        self.decoded = body;
        Some(d)
    }

    /// The transmission length from the header: `None` when the training is not coherent (no burst after all), `Some(None)`
    /// when the header is refused.
    fn header_len(&mut self, x: &mut Vec<Complex32>, t0: f64, sps: f64) -> Option<Option<u32>> {
        let n = x.len();
        x.resize(n + 64, Complex32::default());
        let d = self.demod_mode(x, t0, sps, true);
        x.truncate(n);
        let d = d?;
        Some((!d.header_refused).then_some(d.len))
    }
}

impl Chan {
    /// Runs the search over the new samples `[from, n)` of the history, then decodes the pending burst when it is complete.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn search(
        &mut self,
        fc: f64,
        sps: f64,
        decoder: &mut BurstDecoder,
        sdly: f64,
        t_base: f64,
        out: &mut Vec<(u64, Burst)>,
        any_burst: &mut bool,
        tune_err: f64,
        offs: &mut (f64, u32),
    ) {
        let span = (15.0 * sps).ceil() as u64 + 2;
        let start = self.n - self.scratch.len() as u64;
        // `scratch` holds the newly filtered search samples starting at index `start`.
        for i in 0..self.scratch.len() {
            let idx = start + i as u64;
            let p = self.scratch[i].norm_sqr() as f64;
            self.blk_sum += p;
            self.blk_n += 1;
            if self.blk_n == 512 {
                let m = self.blk_sum / 512.0;
                self.floor = if self.floor == 0.0 || m < self.floor {
                    if self.floor == 0.0 {
                        m
                    } else {
                        0.7 * self.floor + 0.3 * m
                    }
                } else {
                    (self.floor * 1.003).min(m)
                };
                self.blk_sum = 0.0;
                self.blk_n = 0;
            }
            if idx < span + 1 || idx < self.resume + span {
                continue;
            }
            let sh = &self.scratch2;
            let base = self.n - sh.len() as u64;
            // The 16 samples a symbol apart from idx - span: all inside the kept search history.
            let first = (idx - span) as i64 - base as i64;
            let look = if first >= 0 && first as usize + self.offs[15] < sh.len() {
                train_look(sh, first as usize, &self.offs, &self.turns, self.floor)
            } else {
                None
            };
            let (s, m) = match look {
                Some((s, e)) => (s, s.norm() as f64 / e),
                None => (Complex32::default(), 0.0),
            };
            match &mut self.peak {
                None => {
                    if m >= 0.56 {
                        self.peak = Some((m, idx - span, s.arg() as f64, 0));
                    }
                }
                Some(pk) => {
                    pk.3 += 1;
                    if m > pk.0 {
                        *pk = (m, idx - span, s.arg() as f64, pk.3);
                    }
                    let symbols = pk.3 as f64 / sps;
                    if (m < 0.88 * pk.0 && m < 0.56) || symbols > 18.0 {
                        let (_, at, ang, _) = *pk;
                        self.peak = None;
                        {
                            self.pending.push_back(Pending {
                                at: at as f64 - sdly,
                                coarse_hz: ang * SYMBOL_RATE / TAU,
                                len: None,
                            });
                            self.st.bursts_found += 1;
                            self.resume = self.resume.max(at + (2.0 * sps) as u64);
                            *any_burst = true;
                        }
                    }
                }
            }
        }
        // Decode the pending burst when enough samples are in.
        while let Some(p) = self.pending.front() {
            let half = self.decode.len() as u64;
            let at = p.at.max(0.0);
            let coarse = p.coarse_hz;
            let from = (at as u64).saturating_sub((6.0 * sps) as u64 + half);
            let have = self.n;
            // The burst's samples turned by the coarse offset through the decoding filter, into `filt`. Returns where the
            // training starts in them.
            let filtered = |c: &mut Chan, to: u64| -> f64 {
                c.hist.copy(from, to, &mut c.seg);
                let mut nco = Nco::new(fc, -coarse);
                nco.mix_in_place(&mut c.seg);
                c.decode.reset();
                c.filt.clear();
                for &v in &c.seg {
                    c.filt.push(c.decode.process_sample(v));
                }
                at - from as f64 + c.decode.group_delay()
            };
            let len = match p.len {
                Some(l) => l,
                None => {
                    let need_hdr = at as u64 + ((16 + 9 + 4) as f64 * sps) as u64 + half;
                    if have < need_hdr {
                        break;
                    }
                    let t0 = filtered(self, need_hdr);
                    match decoder.header_len(&mut self.filt, t0, sps) {
                        None => {
                            self.pending.pop_front();
                            continue;
                        }
                        Some(Some(l)) => {
                            if let Some(head) = self.pending.front_mut() {
                                head.len = Some(l);
                            }
                            l
                        }
                        Some(None) => {
                            // A refused header is almost always noise that looked like a training sequence: counted apart.
                            self.pending.pop_front();
                            self.st.headers_refused += 1;
                            continue;
                        }
                    }
                }
            };
            let total = phy::symbols_for(len);
            let end = at as u64 + ((16 + total + 4) as f64 * sps) as u64 + half;
            if end > from + HIST as u64 - 4096 {
                self.pending.pop_front();
                self.st.bad += 1;
                continue;
            }
            if have < end {
                break;
            }
            let t0 = filtered(self, end);
            let r = decoder.demod(&self.filt, t0, sps);
            self.pending.pop_front();
            match r {
                Some(d) if !d.bad => {
                    self.resume = at as u64 + (d.symbols as f64 * sps) as u64 + sdly as u64;
                    let past = at + d.symbols as f64 * sps;
                    self.pending.retain(|q| q.at >= past);
                    let off = coarse + d.offset_hz;
                    offs.0 += off;
                    offs.1 += 1;
                    let time = t_base + at / fc;
                    self.st.good += 1;
                    self.st.frames += d.frames.len() as u64;
                    self.st.corrected += d.corrected as u64;
                    self.st.headers_fixed += d.header_fixed as u64;
                    self.st.fcs_bad += d.fcs_bad as u64;
                    self.st.level_db = d.level_db as f32;
                    self.st.snr_db = d.snr_db as f32;
                    self.st.last_heard_s = time;
                    self.st.last_offset_hz = (off + tune_err) as f32;
                    self.last_good_s = time;
                    out.push((
                        end,
                        Burst {
                            time_s: time,
                            freq_hz: self.freq,
                            level_db: d.level_db as f32,
                            snr_db: d.snr_db as f32,
                            offset_hz: off + tune_err,
                            corrected: d.corrected as u32,
                            header_fixed: d.header_fixed,
                            fcs_bad: d.fcs_bad,
                            frames: d.frames,
                        },
                    ));
                }
                _ => self.st.bad += 1,
            }
        }
    }
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.vdl2_front",
            vec![Port::iq("in")],
            vec![
                Port::typed::<Burst>("bursts").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<BankState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FrontSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.station = inputs[0].tuned_hz;
        self.idx = 0;
        self.build();
        Ok(vec![
            Format::new(Kind::typed::<Burst>(), 0.0),
            Format::new(Kind::typed::<BankState>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let tick = io.tick();
        if !self.chans.is_empty() {
            // Pieces end on multiples of 8192 input samples, where the tuning error can change: the same places whatever the cuts.
            let mut rest = x;
            while !rest.is_empty() {
                let room = 8192 - (self.idx % 8192) as usize;
                let (piece, r) = rest.split_at(room.min(rest.len()));
                rest = r;
                self.s0.clear();
                self.s0.extend_from_slice(piece);
                let input: &[Complex32] = if let Some((nco, dec)) = self.shared.as_mut() {
                    nco.mix_in_place(&mut self.s0);
                    self.s3.clear();
                    dec.process(&self.s0, &mut self.s3);
                    &self.s3
                } else {
                    &self.s0
                };
                let t_base = 0.0;
                let mut any = false;
                for c in self.chans.iter_mut() {
                    self.s1.resize(input.len(), Complex32::default());
                    c.nco.mix(input, &mut self.s1);
                    self.s2.clear();
                    for &v in &self.s1 {
                        if let Some(y) = c.sums.push(v) {
                            self.s2.push(y);
                        }
                    }
                    self.s4.clear();
                    c.fir.process(&self.s2, &mut self.s4);
                    c.scratch.clear();
                    for &v in &self.s4 {
                        c.hist.push(v);
                        c.scratch.push(c.search.process_sample(v));
                    }
                    c.n += self.s4.len() as u64;
                    // The search history (filtered), kept for the correlation span.
                    c.scratch2.extend_from_slice(&c.scratch);
                    let keep = (20.0 * self.sps) as usize + 8;
                    if c.scratch2.len() > keep + c.scratch.len() + 4096 {
                        let d = c.scratch2.len() - keep - c.scratch.len();
                        c.scratch2.drain(..d);
                    }
                    let fc = self.fc;
                    // Delay of the search filter: the burst starts this many samples earlier in the unfiltered history.
                    let sdly = c.search.group_delay();
                    c.search(
                        fc,
                        self.sps,
                        &mut self.decoder,
                        sdly,
                        t_base,
                        &mut self.bursts,
                        &mut any,
                        self.tune_err,
                        &mut self.offsets,
                    );
                }
                self.burst_found |= any;
                self.idx += piece.len() as u64;
                // At a boundary, with no channel in the middle of a burst: the error moves by 0.3 x the mean offset of the good bursts.
                if self.idx.is_multiple_of(8192)
                    && self.offsets.1 > 0
                    && self
                        .chans
                        .iter()
                        .all(|c| c.pending.is_empty() && c.peak.is_none())
                {
                    if self.track {
                        let mean = self.offsets.0 / self.offsets.1 as f64;
                        self.tune_err = (self.tune_err + 0.3 * mean).clamp(-6e3, 6e3);
                        self.retune_channels();
                    }
                    self.offsets = (0.0, 0);
                }
            }
            // Burst times: channel index to signal time.
        }
        if !self.bursts.is_empty() {
            // In the order they became decodable (then by channel): the same order whatever the cuts.
            self.bursts
                .sort_by(|a, b| a.0.cmp(&b.0).then(a.1.freq_hz.total_cmp(&b.1.freq_hz)));
            let o = io.items_out::<Burst>(0);
            for (_, b) in self.bursts.drain(..) {
                o.push(Timed {
                    time: b.time_s,
                    value: b,
                });
            }
        }
        if tick {
            let s = self.state();
            let t = s.time_s;
            io.items_out::<BankState>(1)
                .push(Timed { time: t, value: s });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FrontSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FrontSetting::Channels => {
                let t = CHANNELS.read(id, v)?;
                channel_list(t).map_err(|at| SettingError::Malformed { id, at: Some(at) })?;
                t.clone_into(&mut self.channels);
            }
            FrontSetting::TrackTuning => {
                self.track = TRACK_TUNING.read(id, v)?;
                return Ok(Applied::Live);
            }
            FrontSetting::TuneErrorHz => {
                self.guess = TUNE_ERROR_HZ.read(id, v)?;
                self.tune_err = self.guess;
                self.retune_channels();
                return Ok(Applied::Live);
            }
        }
        if self.rate > 0.0 {
            self.build();
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (FrontSetting::Channels, Value::Text(self.channels.clone())),
            (FrontSetting::TrackTuning, Value::Bool(self.track)),
            (FrontSetting::TuneErrorHz, Value::Hz(self.guess)),
        ])
    }
}
