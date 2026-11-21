//! The DVB-C demodulator core: DC removal, acquisition of carrier, width and symbol rate, mixer, matched filter with Gardner
//! timing, the fractionally spaced equaliser (CMA, then decision directed), the carrier stages, the QAM order and inversion
//! trial, phase smoothing, decisions and the differential demapper. It passes aligned bytes to the outer decoder.

use super::qam::{Constellation, Demapper, Qam};
use sdrkit_algo_dsp::approx;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::spectrum::{bridge_dc, cyclic_lines};
use sdrkit_algo_dsp::stats::centred_mean;
use sdrkit_core::Complex32;
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_proto_dvbs2::receive::{timing, Timing};
use sdrkit_proto_dvbs2::spectrum::{area_carrier, Psd};
use sdrkit_proto_mpegts::outer::{sync_search, RS_N as RSW};

/// Equaliser taps.
pub const TAPS: usize = 28;
const _: () = assert!(TAPS <= SUM);
/// The main tap, counted from the oldest sample of the line: 19 taps (9.5 symbols) of post-cursor before it, 8 taps (4 symbols)
/// of pre-cursor after it.
pub const MAIN: usize = 19;
/// The usual symbol rates of cable networks, tried in this order when the spectrum shows no clear rate.
pub const USUAL_RATES: [f64; 14] = [
    6.952e6, 6.950e6, 6.900e6, 6.875e6, 6.885e6, 6.111e6, 6.000e6, 5.900e6, 5.361e6, 5.274e6,
    5.217e6, 5.057e6, 4.000e6, 3.450e6,
];
/// The noise bandwidth per symbol of the symbol timing loop. It is fixed because the equaliser after it follows the residual
/// timing error, and a wider loop adds jitter that the higher QAM orders cannot take.
const TIMING_BN_T: f64 = 0.0005;
/// How far the symbol clock can be from the measured rate, ppm. It covers the error of the rate measurement.
const TIMING_PPM: f64 = 3000.0;
/// The recent symbols that the constellation plot shows.
pub const CONSTELLATION_LEN: usize = 2048;
const ACQ_LEN: usize = 4 * 65536;
const CMA_LEN: u64 = 400_000;
const FREQ_N: usize = 16384;
const DD_TRY: u64 = 60_000;
const SMOOTH: usize = 1024;
const OVERLAP: usize = 16;
const SMOOTH_W: [f32; 5] = [0.97, 0.88, 0.75, 0.59, 0.41];

/// The receiver's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DvbcConfig {
    /// A manual symbol rate, Hz (0: automatic).
    pub manual_rs: f64,
    /// A manual order (None: automatic).
    pub qam: Option<Qam>,
    /// Annex C (roll-off 0.13).
    pub annex_c: bool,
}

impl Default for DvbcConfig {
    fn default() -> Self {
        DvbcConfig {
            manual_rs: 0.0,
            qam: None,
            annex_c: false,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The stage the demodulator is in.
    pub enum Stage {
        /// Measuring the spectrum.
        Spectrum = ("spectrum", "Measuring the spectrum"),
        /// Blind equaliser.
        Cma = ("cma", "Blind equaliser (CMA)"),
        /// Residual frequency from the fourth power.
        Freq = ("freq", "Carrier frequency"),
        /// Starting phase.
        Phase = ("phase", "Carrier phase"),
        /// Decision-directed convergence for the trial order.
        Dd = ("dd", "Trying the QAM order"),
        /// Collecting the bits of the trial.
        Trial = ("trial", "Checking the QAM order"),
        /// Locked and decoding.
        Locked = ("locked", "Decoding"),
    }
}

sdrkit_core::choice_enum! {
    /// Why the demodulator does not lock.
    pub enum Note {
        /// The spectrum shows no carrier.
        NoCarrier = ("no_carrier", "No carrier in the spectrum"),
        /// The symbol rate does not fit the input rate.
        RateNotUsable = ("rate_not_usable", "Symbol rate not usable for this input rate"),
        /// No lock at the manual symbol rate, and the measured rate differs from it.
        ManualRateDiffers = ("manual_rate_differs", "No lock at the manual symbol rate: the measured one differs"),
        /// No lock at the manual symbol rate.
        ManualRateNoLock = ("manual_rate_no_lock", "No lock at the manual symbol rate: the signal can be too weak for this QAM order"),
    }
}

/// What the core measures (for the results).
#[derive(Clone, Copy, Debug, Default)]
pub struct CoreReport {
    /// The reason the demodulator does not lock, when known.
    pub note: Option<Note>,
    /// The order in use (trial or locked).
    pub qam: Option<Qam>,
    /// Symbol rate in use, Hz (timing loop included).
    pub symbol_rate: f64,
    /// The nominal rate the loops started from.
    pub nominal_rate: f64,
    /// The rate measured from the spectrum.
    pub measured_rate: f64,
    /// The manual rate is in use.
    pub manual_used: bool,
    /// -3 dB width, Hz.
    pub width_hz: f64,
    /// Spectrum inverted.
    pub inverted: bool,
    /// Symbol clock against the nominal rate, ppm.
    pub clock_ppm: f64,
    /// Coarse offset plus the carrier loop, Hz.
    pub cfo_hz: f64,
    /// MER, dB, from the trial stage on.
    pub mer_db: Option<f64>,
    /// Level of the last 4096 mixed samples, dBFS.
    pub level_dbfs: f64,
    /// Lock stages: spectrum, timing, equaliser, carrier, sync.
    pub stages: [bool; 5],
    /// Reacquisitions.
    pub searches: u64,
    /// Failed attempts.
    pub attempts: u32,
}

/// The demodulator.
pub struct DvbcCore {
    fs: f64,
    cfg: DvbcConfig,
    /// The stage.
    pub stage: Stage,
    dc: Complex32,
    mix_hz: f64,
    mix: Nco,
    level_acc: f64,
    level_n: u32,
    acq: Vec<Complex32>,
    attempt: u32,
    sync: Option<Timing<32>>,
    rs_nominal: f64,
    mixed: Vec<Complex32>,
    half: Vec<Complex32>,
    on_time_next: bool,
    line: Line,
    lpos: usize,
    taps: Taps,
    cma_taps: Taps,
    stage_n: u64,
    front_symbols: u64,
    fbuf: Vec<Complex32>,
    phase_acc: Complex32,
    theta: f64,
    nu: f64,
    /// The phasor that takes the carrier phase `theta` off a symbol.
    rot: Complex32,
    /// The phase error of the last symbol, which the carrier loop has not used yet.
    carrier_e: f64,
    orders: Vec<Qam>,
    order_idx: usize,
    cons: Constellation,
    trial_z: Vec<Complex32>,
    sig_p: f64,
    err_p: f64,
    mer_valid: bool,
    // Locked.
    inverted: bool,
    demap: Demapper,
    sm: Vec<Complex32>,
    /// The decision of each symbol of `sm` before smoothing: label and point.
    sm_d: Vec<(u16, Complex32)>,
    sm_start: usize,
    phasor: Vec<Complex32>,
    prod: Vec<Complex32>,
    dec: Vec<(u16, Complex32)>,
    bits: Vec<u8>,
    skip_bits: usize,
    odd: bool,
    /// Aligned bytes for the outer decoder, taken by the caller.
    pub out_bytes: Vec<u8>,
    /// The next bytes start a new alignment.
    pub out_restart: bool,
    /// Recent symbols after equaliser and carrier loop (a ring of [`CONSTELLATION_LEN`]).
    pub constellation: Vec<Complex32>,
    cpos: usize,
    /// Log lines, taken by the caller.
    pub log: Vec<String>,
    /// Measurements.
    pub report: CoreReport,
    /// Equaliser taps for the picture (magnitudes, dB) are read from here.
    fft4: Option<Fft>,
}

impl DvbcCore {
    /// A demodulator for input at `fs`.
    pub fn new(fs: f64, cfg: DvbcConfig) -> DvbcCore {
        let mut c = DvbcCore {
            fs,
            cfg,
            stage: Stage::Spectrum,
            dc: Complex32::new(0.0, 0.0),
            mix_hz: 0.0,
            mix: Nco::new(fs.max(1.0), 0.0),
            level_acc: 0.0,
            level_n: 0,
            acq: Vec::with_capacity(ACQ_LEN),
            attempt: 0,
            sync: None,
            rs_nominal: 0.0,
            mixed: Vec::new(),
            half: Vec::new(),
            on_time_next: true,
            line: Line {
                re: [0.0; 2 * TAPS],
                im: [0.0; 2 * TAPS],
            },
            lpos: TAPS - 1,
            taps: Taps::identity(),
            cma_taps: Taps::identity(),
            stage_n: 0,
            front_symbols: 0,
            fbuf: Vec::new(),
            phase_acc: Complex32::new(0.0, 0.0),
            theta: 0.0,
            nu: 0.0,
            rot: Complex32::new(1.0, 0.0),
            carrier_e: 0.0,
            orders: Vec::new(),
            order_idx: 0,
            cons: Constellation::new(Qam::Q64),
            trial_z: Vec::new(),
            sig_p: 0.0,
            err_p: 0.0,
            mer_valid: false,
            inverted: false,
            demap: Demapper::new(Qam::Q64),
            sm: Vec::new(),
            sm_d: Vec::new(),
            sm_start: 0,
            phasor: Vec::new(),
            prod: Vec::new(),
            dec: Vec::new(),
            bits: Vec::new(),
            skip_bits: 0,
            odd: false,
            out_bytes: Vec::new(),
            out_restart: false,
            constellation: Vec::with_capacity(CONSTELLATION_LEN),
            cpos: 0,
            log: Vec::new(),
            report: CoreReport::default(),
            fft4: None,
        };
        c.set_stage(Stage::Spectrum);
        c
    }

    /// The settings.
    pub fn config(&self) -> DvbcConfig {
        self.cfg
    }

    /// New settings: acquisition starts again.
    pub fn set_config(&mut self, cfg: DvbcConfig) {
        self.cfg = cfg;
        self.restart_search();
    }

    /// Back to the spectrum (a retune, a new setting).
    pub fn restart_search(&mut self) {
        self.attempt = 0;
        self.sync = None;
        self.acq.clear();
        self.mix_hz = 0.0;
        self.set_stage(Stage::Spectrum);
        self.out_restart = false;
        self.out_bytes.clear();
        self.report.mer_db = None;
        self.mer_valid = false;
        self.report.qam = None;
        self.report.stages = [false; 5];
    }

    /// The outer decoder lost the stream: back to the blind equaliser.
    pub fn lost_stream(&mut self) {
        self.report.searches += 1;
        self.log.push("lost the stream, searching again".into());
        self.taps = Taps::identity();
        self.set_stage(Stage::Cma);
        self.out_bytes.clear();
        self.out_restart = false;
    }

    fn set_stage(&mut self, s: Stage) {
        self.stage = s;
        self.stage_n = 0;
        if s != Stage::Locked {
            self.report.stages[4] = false;
        }
    }

    /// Non-finite loop state: start again from nothing.
    fn overflow(&mut self) {
        self.log
            .push("the signal overflowed the loops (numbers out of range): starting again".into());
        self.restart_search();
    }

    /// Runs samples through the demodulator.
    pub fn process(&mut self, x: &[Complex32]) {
        if self.stage == Stage::Spectrum {
            for &s in x {
                let v = remove_dc(&mut self.dc, s);
                if self.acq.len() < ACQ_LEN {
                    self.acq.push(v);
                }
            }
            if self.acq.len() >= ACQ_LEN {
                self.analyse();
                self.acq.clear();
            }
            return;
        }
        // DC removal and mixer.
        self.mixed.clear();
        let mut lvl = 0.0f64;
        let mut dc = self.dc;
        self.mixed.extend(x.iter().map(|&s| remove_dc(&mut dc, s)));
        self.dc = dc;
        self.mix.mix_in_place(&mut self.mixed);
        // Eight samples are added at a time, so that one long chain of additions does not set the speed.
        let (eights, rest) = self.mixed.as_chunks::<8>();
        for c in eights {
            let p = c.map(|v| v.norm_sqr());
            lvl += (((p[0] + p[4]) + (p[2] + p[6])) + ((p[1] + p[5]) + (p[3] + p[7]))) as f64;
        }
        for v in rest {
            lvl += v.norm_sqr() as f64;
        }
        self.level_acc += lvl;
        self.level_n += x.len() as u32;
        if self.level_n >= 4096 {
            self.report.level_dbfs = power_to_db(self.level_acc / self.level_n as f64);
            self.level_acc = 0.0;
            self.level_n = 0;
        }
        let Some(sync) = self.sync.as_mut() else {
            return;
        };
        self.half.clear();
        let mixed = std::mem::take(&mut self.mixed);
        sync.process(&mixed, &mut self.half);
        self.mixed = mixed;
        let half = std::mem::take(&mut self.half);
        for &h in &half {
            self.lpos = if self.lpos + 1 == TAPS {
                0
            } else {
                self.lpos + 1
            };
            self.line.put(self.lpos, h);
            let on = self.on_time_next;
            self.on_time_next = !on;
            // The output is computed when an on-time sample arrives: the line then holds on-time samples at even distances from
            // the newest, the main tap among them.
            if !on {
                continue;
            }
            self.symbol();
            if self.stage == Stage::Spectrum {
                break;
            }
        }
        self.half = half;
        if let Some(s) = &self.sync {
            if !s.clock_ppm().is_finite() || !self.theta.is_finite() {
                self.overflow();
            }
        }
    }

    /// The equaliser output for the last TAPS samples.
    #[inline]
    fn equalise(&self) -> Complex32 {
        let (xr, xi) = self.line.window(self.lpos);
        let t = &self.taps;
        // The products of every tap first, then their sum: both run several taps at a time.
        let mut pr = [0f32; SUM];
        let mut pi = [0f32; SUM];
        for i in 0..TAPS {
            pr[i] = t.re[i] * xr[i] - t.im[i] * xi[i];
            pi[i] = t.re[i] * xi[i] + t.im[i] * xr[i];
        }
        Complex32::new(sum(&pr), sum(&pi))
    }

    /// One step of the tap update against error `e` with step size `mu`: `taps -= mu e conj(x)`.
    #[inline]
    fn adapt(&mut self, e: Complex32, mu: f32) {
        let g = e * mu;
        let (xr, xi) = self.line.window(self.lpos);
        let t = &mut self.taps;
        for i in 0..TAPS {
            t.re[i] -= g.re * xr[i] + g.im * xi[i];
            t.im[i] -= g.im * xr[i] - g.re * xi[i];
        }
    }

    /// Starts the carrier loop at phase `theta` with no frequency.
    fn set_carrier(&mut self, theta: f64) {
        self.theta = theta;
        self.nu = 0.0;
        self.carrier_e = 0.0;
        let (s, c) = approx::sin_cos(theta);
        self.rot = Complex32::new(c, -s);
    }

    /// The carrier loop, second order, on the phase error of symbol `z` against its decision `d`. The error of a symbol
    /// moves the phase one symbol later, so the phasor of the next symbol does not wait for this decision.
    fn carrier_loop(&mut self, z: Complex32, d: Complex32) {
        let now = ((z * d.conj()).im / d.norm_sqr().max(1e-9)) as f64;
        let e = std::mem::replace(&mut self.carrier_e, now);
        self.theta += self.nu + 0.01 * e;
        self.nu += 1e-5 * e;
        if self.theta > std::f64::consts::PI {
            self.theta -= 2.0 * std::f64::consts::PI;
        } else if self.theta < -std::f64::consts::PI {
            self.theta += 2.0 * std::f64::consts::PI;
        }
        let (s, c) = approx::sin_cos(self.theta);
        self.rot = Complex32::new(c, -s);
    }

    /// The MER of the recent symbols, dB.
    fn mer(&self) -> f64 {
        power_to_db(self.sig_p / self.err_p.max(1e-12))
    }

    fn measure(&mut self, z: Complex32, d: Complex32) {
        let a = 0.001;
        self.sig_p += a * (d.norm_sqr() as f64 - self.sig_p);
        self.err_p += a * ((z - d).norm_sqr() as f64 - self.err_p);
        if self.constellation.len() < CONSTELLATION_LEN {
            self.constellation.push(z);
        } else {
            self.constellation[self.cpos] = z;
            self.cpos = (self.cpos + 1) % CONSTELLATION_LEN;
        }
    }

    fn symbol(&mut self) {
        self.front_symbols += 1;
        self.stage_n += 1;
        self.report.stages[1] = self.front_symbols > 20_000;
        let y = self.equalise();
        match self.stage {
            Stage::Spectrum => {}
            Stage::Cma => {
                let e = y * (y.norm_sqr() - self.cma_radius());
                let mu = if self.stage_n < 120_000 {
                    4e-4
                } else if self.stage_n < 250_000 {
                    1e-4
                } else {
                    3e-5
                };
                self.adapt(e, mu);
                if self.stage_n >= CMA_LEN {
                    self.cma_taps = self.taps;
                    self.fbuf.clear();
                    self.set_stage(Stage::Freq);
                    self.report.stages[2] = true;
                }
            }
            Stage::Freq => {
                self.fbuf.push(y);
                if self.fbuf.len() >= 4 * FREQ_N {
                    self.frequency_stage();
                }
            }
            Stage::Phase => {
                if self.stage_n > 64 {
                    let y2 = y * y;
                    self.phase_acc += y2 * y2;
                }
                if self.stage_n >= 64 + 2048 {
                    let arg = (self.phase_acc.im as f64).atan2(self.phase_acc.re as f64);
                    self.set_carrier((arg - std::f64::consts::PI) / 4.0);
                    self.sig_p = 1.0;
                    self.err_p = 0.1;
                    self.mer_valid = true;
                    self.report.stages[3] = true;
                    self.set_stage(Stage::Dd);
                }
            }
            Stage::Dd | Stage::Trial => {
                let rot = self.rot;
                let z = y * rot;
                let (_, d) = self.cons.decide(z);
                self.adapt((z - d) * rot.conj(), 3e-4);
                self.carrier_loop(z, d);
                self.measure(z, d);
                if self.stage == Stage::Dd {
                    if self.stage_n >= DD_TRY {
                        self.trial_z.clear();
                        self.set_stage(Stage::Trial);
                    }
                } else {
                    self.trial_z.push(z);
                    let need = (22 * RSW * 8).div_ceil(self.cons.qam.bits());
                    if self.trial_z.len() >= need {
                        self.trial();
                    }
                }
            }
            Stage::Locked => {
                let rot = self.rot;
                let z = y * rot;
                let (label, d) = self.cons.decide(z);
                self.odd = !self.odd;
                if self.odd {
                    self.adapt((z - d) * rot.conj(), 3e-4);
                }
                self.carrier_loop(z, d);
                self.measure(z, d);
                self.sm.push(z);
                self.sm_d.push((label, d));
                if self.sm.len() >= self.sm_start + SMOOTH + OVERLAP {
                    self.smooth_block();
                }
            }
        }
    }

    fn frequency_stage(&mut self) {
        let fft = self.fft4.get_or_insert_with(|| Fft::forward(FREQ_N));
        let mut acc = vec![0f64; FREQ_N];
        let mut buf = vec![Complex32::new(0.0, 0.0); FREQ_N];
        for b in 0..4 {
            for (o, y) in buf.iter_mut().zip(&self.fbuf[b * FREQ_N..(b + 1) * FREQ_N]) {
                let y2 = y * y;
                *o = y2 * y2;
            }
            fft.process(&mut buf);
            for (a, v) in acc.iter_mut().zip(&buf) {
                *a += v.norm_sqr() as f64;
            }
        }
        let k = (0..FREQ_N)
            .max_by(|&a, &b| acc[a].total_cmp(&acc[b]))
            .unwrap_or(0);
        let (ym, y0, yp) = (
            acc[(k + FREQ_N - 1) % FREQ_N],
            acc[k],
            acc[(k + 1) % FREQ_N],
        );
        let den = ym - 2.0 * y0 + yp;
        let d = if den.abs() > 0.0 {
            0.5 * (ym - yp) / den
        } else {
            0.0
        };
        let kk = if k > FREQ_N / 2 {
            k as f64 - FREQ_N as f64
        } else {
            k as f64
        } + d.clamp(-0.5, 0.5);
        let w = 2.0 * std::f64::consts::PI * kk / FREQ_N as f64 / 4.0; // radians per symbol
        let df = w * self.rs_now() / (2.0 * std::f64::consts::PI);
        if !df.is_finite() {
            self.attempt += 1;
            self.sync = None;
            self.set_stage(Stage::Spectrum);
            return;
        }
        self.set_mixer(self.mix_hz + df);
        self.start_order(0);
    }

    fn start_order(&mut self, idx: usize) {
        self.orders = match self.cfg.qam {
            Some(q) => vec![q],
            None => Qam::TRIAL.to_vec(),
        };
        self.order_idx = idx;
        self.cons = Constellation::new(self.orders[idx]);
        self.report.qam = Some(self.orders[idx]);
        self.taps = self.cma_taps;
        self.phase_acc = Complex32::new(0.0, 0.0);
        self.mer_valid = false;
        self.set_stage(Stage::Phase);
    }

    fn trial(&mut self) {
        let qam = self.cons.qam;
        let mut found = None;
        for inv in [false, true] {
            let mut d = Demapper::new(qam);
            let mut bits = Vec::with_capacity(self.trial_z.len() * qam.bits());
            for &z in &self.trial_z {
                d.push(z, inv, &mut bits);
            }
            if let Some(hit) = sync_search(&bits, &sdrkit_proto_dvbs2::sync_rule(20, false)) {
                found = Some((inv, hit));
                break;
            }
        }
        let mer = self.mer();
        let clock = self.report.clock_ppm;
        match found {
            Some((inv, hit)) => {
                self.inverted = inv;
                self.report.inverted = inv;
                self.report.stages[4] = true;
                let rs = self.rs_now();
                self.log.push(format!(
                    "{} locked, {:.4} Msym/s, MER {:.1} dB{}",
                    qam.name(),
                    rs / 1e6,
                    mer,
                    if inv { " (spectrum inverted)" } else { "" }
                ));
                self.demap = Demapper::new(qam);
                self.sm.clear();
                self.sm_d.clear();
                self.sm_start = 0;
                self.bits.clear();
                self.skip_bits = hit.start;
                self.out_restart = true;
                self.out_bytes.clear();
                let tz = std::mem::take(&mut self.trial_z);
                self.set_stage(Stage::Locked);
                for z in &tz {
                    self.sm.push(*z);
                    self.sm_d.push(self.cons.decide(*z));
                    if self.sm.len() >= self.sm_start + SMOOTH + OVERLAP {
                        self.smooth_block();
                    }
                }
                self.trial_z = tz;
            }
            None => {
                self.log.push(format!(
                    "no sync as {}: MER {:.1} dB, clock {:+.0} ppm, carrier {:+.0} Hz",
                    qam.name(),
                    mer,
                    clock,
                    self.nu * self.rs_now() / (2.0 * std::f64::consts::PI)
                ));
                if self.order_idx + 1 < self.orders.len() {
                    self.start_order(self.order_idx + 1);
                } else {
                    self.attempt += 1;
                    self.report.attempts = self.attempt;
                    if self.cfg.manual_rs > 0.0 && self.attempt >= 3 {
                        let m = self.report.measured_rate;
                        self.report.note = Some(
                            if m > 0.0
                                && ((m - self.cfg.manual_rs) / self.cfg.manual_rs).abs() > 0.01
                            {
                                Note::ManualRateDiffers
                            } else {
                                Note::ManualRateNoLock
                            },
                        );
                    }
                    self.sync = None;
                    self.report.qam = None;
                    self.report.mer_db = None;
                    self.mer_valid = false;
                    self.set_stage(Stage::Spectrum);
                }
            }
        }
    }

    /// Smooths the carrier phase over the next SMOOTH symbols and demaps them. Each of three passes decides every symbol with
    /// the phase of the pass before, then takes the phase of a symbol from the weighted sum of `z * conj(decision)` of its
    /// neighbours.
    fn smooth_block(&mut self) {
        const REACH: usize = SMOOTH_W.len();
        const ONE: Complex32 = Complex32::new(1.0, 0.0);
        let lo = self.sm_start.saturating_sub(OVERLAP);
        let hi = (self.sm_start + SMOOTH + OVERLAP).min(self.sm.len());
        let n = hi - lo;
        let z = &self.sm[lo..hi];
        self.dec.clear();
        self.dec.extend_from_slice(&self.sm_d[lo..hi]);
        // The products, with REACH zeros before and after them.
        self.prod.clear();
        self.prod.resize(n + 2 * REACH, Complex32::new(0.0, 0.0));
        self.phasor.clear();
        self.phasor.resize(n, ONE);
        for pass in 0..3 {
            if pass > 0 {
                for ((d, z), r) in self.dec.iter_mut().zip(z).zip(&self.phasor) {
                    let zr = z * r;
                    if !self.cons.near(zr, d.1) {
                        *d = self.cons.decide(zr);
                    }
                }
            }
            for ((p, z), d) in self.prod[REACH..].iter_mut().zip(z).zip(&self.dec) {
                *p = z * d.1.conj();
            }
            self.phasor.fill(Complex32::new(0.0, 0.0));
            for (k, &w) in SMOOTH_W.iter().enumerate() {
                let before = &self.prod[REACH - 1 - k..];
                let after = &self.prod[REACH + 1 + k..];
                for ((r, b), a) in self.phasor.iter_mut().zip(before).zip(after) {
                    *r += (b + a) * w;
                }
            }
            // The sum becomes the unit phasor that takes its phase off.
            for r in &mut self.phasor {
                let m = r.norm_sqr();
                *r = if m > 0.0 {
                    r.conj() * (1.0 / m.sqrt())
                } else {
                    ONE
                };
            }
        }
        let core_lo = self.sm_start - lo;
        let core_hi = core_lo + SMOOTH.min(n - core_lo);
        for i in core_lo..core_hi {
            let zr = z[i] * self.phasor[i];
            let (label, d) = self.dec[i];
            if self.inverted {
                self.demap.push(zr, true, &mut self.bits);
            } else if self.cons.near(zr, d) {
                self.demap.push_label(label, &mut self.bits);
            } else {
                self.demap.push(zr, false, &mut self.bits);
            }
        }
        // Bits to aligned bytes.
        let mut i = 0;
        if self.skip_bits > 0 {
            let k = self.skip_bits.min(self.bits.len());
            self.skip_bits -= k;
            i = k;
        }
        while i + 8 <= self.bits.len() {
            let mut b = 0u8;
            for j in 0..8 {
                b = (b << 1) | self.bits[i + j];
            }
            self.out_bytes.push(b);
            i += 8;
        }
        self.bits.drain(..i);
        self.sm_start += core_hi - core_lo;
        // Keep the overlap before the next core.
        if self.sm_start > OVERLAP {
            let drop = self.sm_start - OVERLAP;
            self.sm.drain(..drop);
            self.sm_d.drain(..drop);
            self.sm_start -= drop;
        }
    }

    fn rs_now(&self) -> f64 {
        self.sync.as_ref().map_or(0.0, |s| self.fs / s.sps_now())
    }

    fn set_mixer(&mut self, hz: f64) {
        self.mix_hz = hz;
        self.mix.set_freq(-hz);
    }

    fn analyse(&mut self) {
        let alpha = if self.cfg.annex_c { 0.13 } else { 0.15 };
        let mut psd = Psd::new(4096, 4096);
        psd.push(&self.acq);
        let mut s = Vec::new();
        psd.spectrum(&mut s);
        bridge_dc(&mut s);
        let mut sm = Vec::new();
        centred_mean(&s, 16, &mut sm);
        let Some(shape) = area_carrier(&sm, self.fs, alpha) else {
            self.report.note = Some(Note::NoCarrier);
            self.report.stages[0] = false;
            return;
        };
        self.report.stages[0] = true;
        self.report.width_hz = shape.width_hz;
        let width = shape.width_hz;
        let lines = cyclic_lines(&self.acq, self.fs, 65536, 0.3e6, 0.95 * self.fs, 30.0);
        let mut cands: Vec<(f64, f64)> = lines
            .into_iter()
            .filter(|(f, _)| (f - width).abs() <= 0.2 * width)
            .map(|(f, r)| {
                let usual = USUAL_RATES.iter().any(|u| ((f - u) / u).abs() < 300e-6);
                (f, if usual { r * 1000.0 } else { r })
            })
            .collect();
        cands.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut list: Vec<f64> = Vec::new();
        for (f, _) in cands.iter().take(8) {
            if !list.iter().any(|g| ((g - f) / f).abs() < 300e-6) {
                list.push(*f);
            }
        }
        let mut usual: Vec<f64> = USUAL_RATES
            .iter()
            .copied()
            .filter(|u| ((u - width) / width).abs() <= 0.15)
            .collect();
        usual.sort_by(|a, b| (a - width).abs().total_cmp(&(b - width).abs()));
        for u in usual {
            if !list.iter().any(|g| ((g - u) / u).abs() < 300e-6) {
                list.push(u);
            }
        }
        if list.is_empty() {
            list.push(width);
        }
        self.report.measured_rate = list[0];
        let a = self.attempt as usize;
        let (rs, manual) = if self.cfg.manual_rs > 0.0 {
            if a < 2 || a.is_multiple_of(2) {
                (self.cfg.manual_rs, true)
            } else {
                (list[(a / 2) % list.len()], false)
            }
        } else {
            (list[a % list.len()], false)
        };
        if !(0.5e6..=0.95 * self.fs).contains(&rs) {
            self.report.note = Some(Note::RateNotUsable);
            self.attempt += 1;
            return;
        }
        self.report.manual_used = manual;
        self.log.push(format!(
            "carrier {:+.1} kHz, -3 dB width {:.3} MHz, symbol rate line {:.4} Msym/s, using {:.4} Msym/s{}",
            shape.centre_hz / 1e3,
            width / 1e6,
            list[0] / 1e6,
            rs / 1e6,
            if manual { " (manual)" } else { "" }
        ));
        self.rs_nominal = rs;
        self.report.nominal_rate = rs;
        self.set_mixer(shape.centre_hz);
        self.sync = match timing(
            self.fs / rs,
            alpha,
            TIMING_BN_T,
            TIMING_BN_T,
            TIMING_PPM,
            true,
        ) {
            Ok(s) => Some(s),
            Err(e) => {
                self.log
                    .push(format!("the symbol timing cannot start: {e}"));
                None
            }
        };
        self.on_time_next = true;
        self.taps = Taps::identity();
        self.front_symbols = 0;
        self.set_carrier(0.0);
        self.set_stage(Stage::Cma);
    }

    /// Updates the measurements that follow the loops.
    pub fn update_report(&mut self) {
        let rs = self.rs_now();
        if rs > 0.0 {
            self.report.symbol_rate = rs;
            self.report.clock_ppm = (rs / self.rs_nominal - 1.0) * 1e6;
        }
        if self.mer_valid {
            self.report.mer_db = Some(self.mer());
        }
        self.report.cfo_hz = self.mix_hz + self.nu * rs / (2.0 * std::f64::consts::PI);
        if self.stage != Stage::Locked {
            self.report.inverted = self.inverted && self.stage == Stage::Locked;
        }
    }

    /// Writes the equaliser tap magnitudes in dB (strongest 0 dB) to `out`, one per tap ([`TAPS`]). The main tap is [`MAIN`].
    pub fn tap_db(&self, out: &mut [f32]) {
        let t = &self.taps;
        let norm = |i: usize| t.re[i].hypot(t.im[i]);
        let m = (0..TAPS).map(norm).fold(1e-12, f32::max);
        for (i, o) in out.iter_mut().enumerate().take(TAPS) {
            *o = 20.0 * (norm(i) / m).max(1e-6).log10();
        }
    }

    /// The symbol rate in use (0 before acquisition).
    pub fn symbol_rate(&self) -> f64 {
        self.rs_now()
    }

    /// Bits per symbol of the order in use.
    pub fn bits_per_symbol(&self) -> usize {
        self.cons.qam.bits()
    }

    /// The constant modulus radius `E|a|^4 / E|a|^2` of the order in use (the manual order, else the order of the last trial)
    /// that the blind equaliser drives `|y|^2` to.
    pub fn cma_radius(&self) -> f32 {
        cma_radius(self.cfg.qam.unwrap_or(self.cons.qam))
    }
}

/// The constant modulus radius of a DVB-C order (EN 300 429 figure 7 points, `sdrkit_algo_modem::equalizer::cma_radius`).
pub fn cma_radius(qam: Qam) -> f32 {
    // The blind equaliser asks for each symbol: the radius of each order is worked out once.
    static RADII: std::sync::OnceLock<Vec<(Qam, f32)>> = std::sync::OnceLock::new();
    RADII
        .get_or_init(|| {
            [Qam::Q16, Qam::Q32, Qam::Q64, Qam::Q128, Qam::Q256]
                .iter()
                .map(|&q| (q, Constellation::new(q).cma_radius()))
                .collect()
        })
        .iter()
        .find(|(q, _)| *q == qam)
        .map_or(0.0, |(_, r)| *r)
}

/// The weight of one sample in the mean that follows the DC of the input.
const DC_STEP: f32 = 1e-4;

/// Moves the mean `dc` towards `s` and returns `s` without it.
#[inline]
fn remove_dc(dc: &mut Complex32, s: Complex32) -> Complex32 {
    *dc = *dc * (1.0 - DC_STEP) + s * DC_STEP;
    s - *dc
}

/// The length of the array that [`sum`] adds.
const SUM: usize = 32;

/// The sum of `p`, added in halves: each step adds the upper half of the values to the lower half.
#[inline]
fn sum(p: &[f32; SUM]) -> f32 {
    let mut a = [0f32; 16];
    for k in 0..16 {
        a[k] = p[k] + p[k + 16];
    }
    let mut b = [0f32; 8];
    for k in 0..8 {
        b[k] = a[k] + a[k + 8];
    }
    let mut c = [0f32; 4];
    for k in 0..4 {
        c[k] = b[k] + b[k + 4];
    }
    (c[0] + c[2]) + (c[1] + c[3])
}

/// The equaliser taps, oldest sample first, real and imaginary parts apart so that the sums run several taps at a time.
#[derive(Clone, Copy)]
struct Taps {
    re: [f32; TAPS],
    im: [f32; TAPS],
}

impl Taps {
    /// The taps that pass the main sample and nothing else.
    fn identity() -> Taps {
        let mut t = Taps {
            re: [0.0; TAPS],
            im: [0.0; TAPS],
        };
        t.re[MAIN] = 1.0;
        t
    }
}

/// The delay line of the equaliser. Each sample is written twice, TAPS apart, so the last TAPS samples are always one
/// slice.
struct Line {
    re: [f32; 2 * TAPS],
    im: [f32; 2 * TAPS],
}

impl Line {
    /// Writes `h` at position `pos` (below TAPS).
    #[inline]
    fn put(&mut self, pos: usize, h: Complex32) {
        self.re[pos] = h.re;
        self.re[pos + TAPS] = h.re;
        self.im[pos] = h.im;
        self.im[pos + TAPS] = h.im;
    }

    /// The last TAPS samples, oldest first, when the newest was written at `pos`: real and imaginary parts.
    #[inline]
    fn window(&self, pos: usize) -> (&[f32; TAPS], &[f32; TAPS]) {
        fn part(x: &[f32; 2 * TAPS], pos: usize) -> &[f32; TAPS] {
            x[pos + 1..]
                .first_chunk()
                .unwrap_or_else(|| unreachable!("a position is below TAPS"))
        }
        (part(&self.re, pos), part(&self.im, pos))
    }
}
