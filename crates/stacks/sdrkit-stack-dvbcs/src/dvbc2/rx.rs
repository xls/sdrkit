//! The front of the DVB-C2 receiver: samples in, the equalised cells of the data slices of one PLP out ([`C2Cells`]).
//!
//! Front end (DC removal, resampling to 64/7 Msps with the clock correction), symbol timing and guard from the guard
//! correlation folded over 8 symbol periods, the preamble by its pilot energy pattern, the absolute carrier index from the
//! preamble pilots, the preamble header and L1 part 2, then per data symbol: the common phase step of the absolute carrier
//! grid, the channel from the pilots on the D_X grid averaged over symbols, the common phase and timing slope (fed to the
//! carrier and window loops and to the clock), the noise, and the cells of the data slices equalised and passed on to the
//! decoding stage ([`super::decode`]).
//!
//! Standard: ETSI EN 302 769 V1.3.1.

use super::decode::{C2Cells, PathSetup, SliceSpec};
use super::gen::{layout_of, slice_payload};
use super::header::{preamble_fields, HeaderDecoder};
use super::l1::{L1Codec, L1Part2, L1Sizes};
use super::packets::{SliceSetup, Thresholds};
use super::signal::{
    freq_addresses, l1_cell, sequences, Cell, Guard, BLOCK, DATA_SYMBOLS, D_Y,
    FFT, L1_CELLS, PILOT_AMP, Raster,
};
use super::tables::{CodeRate, ModCod, Qam, W_RM_DEFAULT};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_fec::twistblock::TwistedBlock;
use sdrkit_algo_modem::ofdm::GridInterp;
use sdrkit_core::{Choice, Complex32};
use sdrkit_graph::Timed;
use std::f64::consts::TAU;
use std::sync::Arc;

/// What the receiver does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RxConfig {
    /// The RF centre of the samples.
    pub centre_hz: f64,
    /// The input rate.
    pub rate_hz: f64,
    /// The channel raster.
    pub raster: Raster,
    /// The centre is the true RF centre; off: the full carrier index search.
    pub centre_known: bool,
    /// The PLP; `None` for the first data PLP of the first slice in the window.
    pub plp: Option<u8>,
    /// The data slice; `None` for the first slice in the window that carries the PLP.
    pub slice: Option<u8>,
    /// The most LDPC iterations.
    pub iterations: u32,
    /// The guard correlation metric that accepts a timing.
    pub timing_metric: f32,
    /// The scattered pilot energy ratio below which a symbol is a preamble.
    pub preamble_ratio: f32,
    /// The preamble pilot correlation that accepts a carrier index.
    pub index_metric: f32,
    /// Carriers either side of the nominal index that the search tries.
    pub index_span: usize,
    /// The header and alignment thresholds.
    pub packets: Thresholds,
    /// Frames without a preamble before a new timing search.
    pub preamble_misses: u32,
    /// The time interleaver has one column per carrier.
    pub ti_full_width: bool,
    /// The header scrambling word w^RM, w^RM_0 the most significant bit.
    pub w_rm: u32,
    /// BBFRAMEs of a bundled PLP held to put them in ISCR order.
    pub bundle_hold: usize,
}

impl Default for RxConfig {
    fn default() -> Self {
        RxConfig {
            centre_hz: 346e6,
            rate_hz: 10e6,
            raster: Raster::R8,
            centre_known: true,
            plp: None,
            slice: None,
            iterations: 50,
            timing_metric: 0.22,
            preamble_ratio: 3.0,
            index_metric: 0.35,
            index_span: 64,
            packets: Thresholds::default(),
            preamble_misses: 3,
            ti_full_width: true,
            w_rm: W_RM_DEFAULT,
            bundle_hold: 16,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How far the receiver has come.
    pub enum State {
        /// Looking for the symbol timing.
        Searching = ("searching", "Searching"),
        /// Timing found; looking for a preamble.
        Timing = ("timing", "Timing found"),
        /// Decoding the L1.
        L1 = ("l1", "Decoding the L1"),
        /// Data symbols.
        Data = ("data", "Data"),
    }
}

/// The measurements and counters of the receiver.
#[derive(Clone, Debug, Default)]
pub struct Status {
    /// The state.
    pub state: Option<State>,
    /// The guard interval found.
    pub guard: Option<Guard>,
    /// The carrier offset, Hz (from the absolute carrier grid once the index is known).
    pub cfo_hz: f64,
    /// The clock error, ppm.
    pub clock_ppm: f64,
    /// The absolute carrier at the centre of the window.
    pub window_carrier: Option<usize>,
    /// Es/N0 of the data cells from the pilots, dB.
    pub esn0_db: Option<f64>,
    /// The last good L1 part 2.
    pub l1: Option<L1Part2>,
    /// The slice and PLP decoded.
    pub chosen: Option<(u8, u8)>,
    /// The common PLP decoded with the chosen PLP.
    pub common: Option<u8>,
    /// The slices in the window that carry the chosen PLP, when it is bundled.
    pub bundled_slices: usize,
    /// The frequency to tune to when the chosen slice, or the slice of its common PLP, lies outside what the samples hold:
    /// the tuning position of the L1 whose window holds both.
    pub retune_hz: Option<f64>,
    /// The data slice that the decoding needs and that reaches out of the window (DSLICE_ID), and by how much, Hz.
    pub outside: Option<(u8, f64)>,
    /// L1 decodes that failed.
    pub l1_bad: u64,
    /// L1 decodes that succeeded.
    pub l1_ok: u64,
    /// Times the receiver went back to the timing or preamble search.
    pub sync_losses: u64,
    /// C2 frames received.
    pub frames: u64,
}

/// The step of the DC tracker per input sample: slow enough that the carriers next to 0 Hz keep their phase.
const DC_RATE: f32 = 1e-6;
/// Guard correlation over this many symbol periods.
const FOLD: usize = 8;
/// Carriers either side of the window centre that the receiver uses (inside the resampler's flat band).
pub const HALF_WINDOW: usize = 1760;
/// The longest a channel grid value is averaged, symbols of its update.
const GRID_AVERAGE: f32 = 8.0;
/// Native samples after a move of the window during which the resampler still holds samples of the old window.
const SETTLE: usize = 512;

/// What the data path of one L1 configuration holds: the window (channel, pilots) and the slices decoded in it.
struct DataPath {
    dx: usize,
    k0: usize,
    /// Carriers used: k_lo ..= k_hi.
    k_lo: usize,
    k_hi: usize,
    /// The channel grid on multiples of D_X from `g0`.
    g0: usize,
    grid: Vec<Complex32>,
    count: Vec<f32>,
    /// Grid values with no pilot (notches).
    holes: Vec<usize>,
    interp: GridInterp,
    /// The channel over k_lo ..= k_hi.
    h: Vec<Complex32>,
    /// The pilots of each symbol class: (carrier offset from k_lo, value).
    pilots: [Vec<(u32, f32)>; D_Y],
    /// The carrier at 0 Hz of the input, whose cell is erased.
    dc: Option<usize>,
    a: Vec<Complex32>,
    /// The slices decoded; the first carries the chosen PLP.
    slices: Vec<SlicePath>,
}

/// One data slice of the data path: its carriers and data cells.
struct SlicePath {
    ds: (usize, usize),
    payload: [Vec<u32>; D_Y],
    id: u8,
    /// The PLP whose PLP_START aligns the slice.
    plp: Option<u8>,
}

/// A decode of the preamble header with one interleaver parity: (agreement, odd, bits, cells, weights).
type HeaderTry = (u32, bool, u16, Vec<Complex32>, Vec<f32>);

/// The receiver core.
pub struct C2Receiver {
    cfg: RxConfig,
    dc: Complex32,
    resampler: Option<FractionalResampler<Complex32>>,
    correction: f64,
    tmp: Vec<Complex32>,
    /// Native samples; `buf[0]` is native sample `base`.
    buf: Vec<Complex32>,
    base: u64,
    nco_freq: f64,
    nco_phase: f64,
    nco_at: u64,
    state: State,
    guard: Option<Guard>,
    /// The start of the next FFT window (absolute native sample).
    win: u64,
    /// Symbols since the timing was found.
    sym: u64,
    fft: Fft,
    bins: Vec<Complex32>,
    k0: Option<usize>,
    data_like: u32,
    /// Symbols since the receiver last decoded a preamble or found the timing.
    since_timing: u32,
    /// In the L1 state: the preamble symbol and the number of them; in the data state: the data symbol.
    pre_index: usize,
    sizes: Option<L1Sizes>,
    l1_parity: bool,
    frame_sym: usize,
    missed: u32,
    hdr: HeaderDecoder,
    codec: Option<L1Codec>,
    l1_cells: Vec<Complex32>,
    l1_w: Vec<f32>,
    l1_bits: Vec<u8>,
    /// The channel of the last preamble symbol over the window (index: bin + HALF_WINDOW).
    pre_h: Vec<Complex32>,
    pre_noise: f32,
    path: Option<DataPath>,
    noise: f32,
    phase_prev: Option<f64>,
    /// The symbol of the last loop measurement.
    meas_sym: Option<u64>,
    timing_prev: Option<f64>,
    shifts: f64,
    drift_acc: f64,
    drift_n: u32,
    /// The shift of the input before the resampler that moves the window onto the chosen slice, Hz.
    shift_hz: f64,
    shift_phase: f64,
    /// The window moved while the data path was built.
    moved: bool,
    /// The first native sample after a move of the window that holds the new window.
    settle: u64,
    /// An L1 was decoded since the last move of the window.
    move_proved: bool,
    /// A move of the window ended in a loss before an L1 was decoded: the receiver does not move again.
    move_failed: bool,
    /// The time of the samples, seconds.
    now: f64,
    /// Items for the decoding stage not yet sent.
    cells: Vec<Timed<C2Cells>>,
    /// The measurements and counters.
    pub status: Status,
    log: Vec<String>,
}

impl C2Receiver {
    /// A receiver.
    pub fn new(cfg: RxConfig) -> C2Receiver {
        let native = cfg.raster.rate();
        let low = cfg.rate_hz.min(native);
        // The flat band holds the window, HALF_WINDOW carriers either side.
        let pass = (0.432 * native).min(0.45 * low);
        let resampler = FractionalResampler::new(cfg.rate_hz, native, pass, (low - pass).max(pass * 1.05), 60.0).ok();
        C2Receiver {
            cfg,
            dc: Complex32::default(),
            resampler,
            correction: 1.0,
            tmp: Vec::new(),
            buf: Vec::new(),
            base: 0,
            nco_freq: 0.0,
            nco_phase: 0.0,
            nco_at: 0,
            state: State::Searching,
            guard: None,
            win: 0,
            sym: 0,
            fft: Fft::forward(FFT),
            bins: vec![Complex32::default(); FFT],
            k0: None,
            data_like: 0,
            since_timing: 0,
            pre_index: 0,
            sizes: None,
            l1_parity: false,
            frame_sym: 0,
            missed: 0,
            hdr: HeaderDecoder::new(cfg.w_rm),
            codec: None,
            l1_cells: Vec::new(),
            l1_w: Vec::new(),
            l1_bits: Vec::new(),
            pre_h: vec![Complex32::default(); 2 * HALF_WINDOW + 1],
            pre_noise: 1.0,
            path: None,
            noise: 0.0,
            phase_prev: None,
            meas_sym: None,
            timing_prev: None,
            shifts: 0.0,
            drift_acc: 0.0,
            drift_n: 0,
            shift_hz: 0.0,
            shift_phase: 0.0,
            moved: false,
            settle: 0,
            move_proved: false,
            move_failed: false,
            now: 0.0,
            // A new receiver: the decoding stage starts again.
            cells: vec![Timed {
                time: 0.0,
                value: C2Cells::Reset,
            }],
            status: Status {
                state: Some(State::Searching),
                ..Status::default()
            },
            log: Vec::new(),
        }
    }

    /// The configuration.
    pub fn config(&self) -> &RxConfig {
        &self.cfg
    }

    /// Chooses another PLP or slice.
    pub fn set_choice(&mut self, plp: Option<u8>, slice: Option<u8>) {
        if (plp, slice) != (self.cfg.plp, self.cfg.slice) {
            self.cfg.plp = plp;
            self.cfg.slice = slice;
            self.path = None;
        }
    }

    /// Sets the most LDPC iterations; the decoding stage learns it with the next samples.
    pub fn set_iterations(&mut self, n: u32) {
        self.cfg.iterations = n;
        self.send(C2Cells::Iterations(n));
    }

    /// Takes the log lines written since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// Feeds samples; `out` receives the items for the decoding stage, in order, at the signal time of the end of `x`.
    pub fn feed(&mut self, x: &[Complex32], out: &mut Vec<Timed<C2Cells>>) {
        self.now += x.len() as f64 / self.cfg.rate_hz;
        // DC removal.
        self.tmp.clear();
        for &v in x {
            self.dc += (v - self.dc) * DC_RATE;
            self.tmp.push(v - self.dc);
        }
        if self.shift_hz != 0.0 {
            let step = -self.shift_hz / self.cfg.rate_hz;
            for (i, v) in self.tmp.iter_mut().enumerate() {
                if i % 256 == 0 {
                    self.shift_phase = self.shift_phase.rem_euclid(1.0);
                }
                let a = TAU * self.shift_phase;
                *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
                self.shift_phase += step;
            }
        }
        match &mut self.resampler {
            Some(r) => {
                let tmp = std::mem::take(&mut self.tmp);
                r.process(&tmp, &mut self.buf);
                self.tmp = tmp;
            }
            None => self.buf.extend_from_slice(&self.tmp),
        }
        self.run();
        out.append(&mut self.cells);
        self.trim();
    }

    /// Queues an item for the decoding stage.
    fn send(&mut self, value: C2Cells) {
        self.cells.push(Timed { time: self.now, value });
    }

    /// Drops samples before the next window.
    fn trim(&mut self) {
        let keep_from = self.win.saturating_sub(self.base).min(self.buf.len() as u64) as usize;
        let limit = 64 * FFT;
        if self.state == State::Searching {
            // Keep enough for one search.
            let need = FOLD * (FFT + 64) + FFT + 64;
            if self.buf.len() > need + limit {
                let cut = self.buf.len() - need;
                self.buf.drain(..cut);
                self.base += cut as u64;
            }
            return;
        }
        if keep_from > limit {
            self.buf.drain(..keep_from);
            self.base += keep_from as u64;
        }
    }

    fn set_state(&mut self, s: State) {
        if self.state != s {
            if s == State::Searching || (s == State::Timing && self.state != State::Searching) {
                self.status.sync_losses += u64::from(self.state != State::Searching);
            }
            self.state = s;
            self.status.state = Some(s);
        }
    }

    fn lose(&mut self, why: &str) {
        self.log.push(format!("lost: {why}"));
        self.set_state(State::Searching);
        self.guard = None;
        self.k0 = None;
        self.path = None;
        self.sizes = None;
        self.data_like = 0;
        self.since_timing = 0;
        self.missed = 0;
        self.phase_prev = None;
        self.meas_sym = None;
        self.timing_prev = None;
        self.status.window_carrier = None;
        // The loops start again from nothing.
        self.correction = 1.0;
        if let Some(r) = self.resampler.as_mut() {
            r.set_correction(1.0);
        }
        self.drift_acc = 0.0;
        self.drift_n = 0;
        self.shifts = 0.0;
        self.noise = 0.0;
        if self.shift_hz != 0.0 && !self.move_proved {
            // The samples did not hold the slice (a radio filter narrower than the sample rate, for one).
            self.move_failed = true;
        }
        self.shift_hz = 0.0;
        self.shift_phase = 0.0;
        self.send(C2Cells::Lost);
        // The next search starts at the samples not yet used.
        let used = self.win.saturating_sub(self.base).min(self.buf.len() as u64) as usize;
        self.buf.drain(..used);
        self.base += used as u64;
    }

    fn run(&mut self) {
        loop {
            if self.state == State::Searching {
                if !self.search() {
                    return;
                }
                continue;
            }
            let Some(g) = self.guard else { return };
            let start = self.win.saturating_sub(self.base) as usize;
            if self.win < self.base {
                // The window fell behind the buffer: start again.
                self.lose("samples dropped");
                continue;
            }
            if start + FFT > self.buf.len() {
                return;
            }
            self.fft_window(start);
            self.win += g.symbol_len() as u64;
            self.symbol(g);
            self.sym += 1;
        }
    }

    /// The carrier offset when bin 0 is carrier `k0`, Hz.
    fn offset_hz(&self, k0: usize) -> f64 {
        self.cfg.centre_hz + self.shift_hz + self.nco_freq * self.cfg.raster.rate() - k0 as f64 * self.cfg.raster.spacing()
    }

    /// The bin of the carrier at 0 Hz of the input.
    fn dc_bin(&self) -> isize {
        -(self.shift_hz / self.cfg.raster.spacing()).round() as isize
    }

    /// The NCO phase (cycles) at absolute native sample `n`.
    fn nco_phase_at(&self, n: u64) -> f64 {
        self.nco_phase + self.nco_freq * (n as f64 - self.nco_at as f64)
    }

    /// Changes the NCO frequency from sample `n` on, keeping its phase continuous.
    fn set_nco(&mut self, f: f64, n: u64) {
        self.nco_phase = self.nco_phase_at(n).rem_euclid(1.0);
        self.nco_at = n;
        self.nco_freq = f;
    }

    /// FFT of the window at `start` (index into `buf`) with the NCO removed, into `bins`.
    fn fft_window(&mut self, start: usize) {
        let n = self.base + start as u64;
        let ph = self.nco_phase_at(n);
        let step = -TAU * self.nco_freq;
        let unit = Complex32::new(step.cos() as f32, step.sin() as f32);
        let mut turn = Complex32::default();
        // A rotator re-anchored every 256 samples.
        for (i, (o, &v)) in self.bins.iter_mut().zip(&self.buf[start..start + FFT]).enumerate() {
            if i % 256 == 0 {
                let a = -TAU * ph + step * i as f64;
                turn = Complex32::new(a.cos() as f32, a.sin() as f32);
            }
            *o = v * turn;
            turn *= unit;
        }
        self.fft.process(&mut self.bins);
    }

    /// The bin of FFT output for signed carrier offset `b`.
    fn bin(&self, b: isize) -> Complex32 {
        self.bins[b.rem_euclid(FFT as isize) as usize]
    }

    /// The timing search: the guard correlation folded over [`FOLD`] symbol periods for each guard. Returns false when more
    /// samples are needed.
    fn search(&mut self) -> bool {
        let need = FOLD * (FFT + 64) + FFT + 64;
        if self.buf.len() < need {
            return false;
        }
        let mut best: Option<(f32, Guard, usize, Complex32)> = None;
        for g in [Guard::G1_64, Guard::G1_128] {
            let gl = g.samples();
            let ns = g.symbol_len();
            let x = &self.buf;
            let span = FOLD * ns;
            // Products and energies, with a moving sum over the guard.
            let mut c = vec![Complex32::default(); ns];
            let mut e = vec![0f32; ns];
            let mut run_c = Complex32::default();
            let mut run_e = 0f32;
            for n in 0..span + gl {
                let p = x[n] * x[n + FFT].conj();
                let q = 0.5 * (x[n].norm_sqr() + x[n + FFT].norm_sqr());
                run_c += p;
                run_e += q;
                if n >= gl {
                    run_c -= x[n - gl] * x[n - gl + FFT].conj();
                    run_e -= 0.5 * (x[n - gl].norm_sqr() + x[n - gl + FFT].norm_sqr());
                }
                if n + 1 >= gl {
                    let start = n + 1 - gl;
                    if start < span {
                        c[start % ns] += run_c;
                        e[start % ns] += run_e;
                    }
                }
            }
            for phase in 0..ns {
                let m = if e[phase] > 0.0 { c[phase].norm() / e[phase] } else { 0.0 };
                if best.is_none_or(|b| m > b.0) {
                    best = Some((m, g, phase, c[phase]));
                }
            }
        }
        let Some((metric, g, phase, c)) = best else {
            return false;
        };
        if metric < self.cfg.timing_metric {
            // Nothing: move on by one fold.
            let cut = FOLD * (FFT + 64);
            self.buf.drain(..cut);
            self.base += cut as u64;
            return true;
        }
        // The fractional carrier offset: the phase of x(n) conj(x(n + N)) is -2 pi eps.
        let eps = -(c.im.atan2(c.re) as f64) / TAU;
        let n0 = self.base + phase as u64;
        self.set_nco(eps / FFT as f64, n0);
        self.guard = Some(g);
        self.status.guard = Some(g);
        // The window starts half a guard into the guard of a symbol.
        self.win = n0 + (g.samples() / 2) as u64;
        self.sym = 0;
        self.data_like = 0;
        self.since_timing = 0;
        self.k0 = None;
        self.log.push(format!(
            "timing found: guard {}, metric {metric:.2}, offset {:.0} Hz",
            g.label(),
            eps * self.cfg.raster.spacing()
        ));
        self.status.cfo_hz = eps * self.cfg.raster.spacing();
        self.set_state(State::Timing);
        true
    }

    /// The ratio of the strongest scattered pilot phase energy to the median, over the window.
    fn pilot_ratio(&self, g: Guard) -> f32 {
        let period = g.d_x() * D_Y;
        let mut e = vec![0f32; period];
        for b in -(HALF_WINDOW as isize)..=HALF_WINDOW as isize {
            e[b.rem_euclid(period as isize) as usize] += self.bin(b).norm_sqr();
        }
        let max = e.iter().copied().fold(0f32, f32::max);
        e.sort_by(f32::total_cmp);
        let med = e[period / 2];
        if med > 0.0 {
            max / med
        } else {
            0.0
        }
    }

    /// The common phase step of the absolute carrier grid for the current symbol: the factor that undoes it.
    fn grid_phase(&self, k0: usize, g: Guard) -> Complex32 {
        let ns = g.symbol_len() as u64;
        let turns = ((k0 as u64 % FFT as u64) * ((self.sym % FFT as u64) * ns % FFT as u64)) % FFT as u64;
        let a = TAU * turns as f64 / FFT as f64;
        Complex32::new(a.cos() as f32, a.sin() as f32)
    }

    fn symbol(&mut self, g: Guard) {
        match self.state {
            State::Searching => {}
            State::Timing => self.timing_symbol(g),
            State::L1 => self.preamble_symbol(g),
            State::Data => self.data_symbol(g),
        }
    }

    fn timing_symbol(&mut self, g: Guard) {
        if self.win < self.settle {
            return;
        }
        // Frames of symbols without a decoded preamble: the timing is wrong.
        self.since_timing += 1;
        if self.since_timing > self.cfg.preamble_misses * (DATA_SYMBOLS as u32 + 16) {
            self.lose("no preamble");
            return;
        }
        let ratio = self.pilot_ratio(g);
        if ratio >= self.cfg.preamble_ratio {
            self.data_like += 1;
            return;
        }
        // A preamble right after data symbols, or the first symbol of the samples, is the first of its frame.
        if self.data_like == 0 && self.sym > 0 {
            return;
        }
        self.data_like = 0;
        if self.k0.is_none() {
            let k = if self.cfg.centre_known {
                self.index_known(g)
            } else {
                self.index_unknown(g)
            };
            let Some(k0) = k else {
                self.log.push("preamble seen, no carrier index".into());
                return;
            };
            self.k0 = Some(k0);
            self.status.window_carrier = Some(k0);
            self.status.cfo_hz = self.offset_hz(k0);
            self.log.push(format!("carrier index {k0}, offset {:.0} Hz", self.status.cfo_hz));
        }
        self.pre_index = 0;
        self.sizes = None;
        self.l1_cells.clear();
        self.l1_w.clear();
        self.set_state(State::L1);
        self.preamble_symbol(g);
        if self.state == State::L1 && self.sizes.is_some() {
            self.recentre(g);
        }
    }

    /// Moves the window so that the useful part starts half a guard after it, from the mean delay of the preamble channel.
    fn recentre(&mut self, g: Guard) {
        let mut acc = Complex32::default();
        for j in 1..self.pre_h.len() / 6 {
            acc += self.pre_h[6 * j] * self.pre_h[6 * (j - 1)].conj();
        }
        if acc.norm_sqr() == 0.0 {
            return;
        }
        // A delay of tau samples turns carrier b by -2 pi b tau / N.
        let tau = -(acc.im.atan2(acc.re) as f64) * FFT as f64 / (TAU * 6.0);
        let shift = (tau - g.samples() as f64 / 2.0).round() as i64;
        if shift != 0 {
            self.win = (self.win as i64 + shift) as u64;
            for (i, h) in self.pre_h.iter_mut().enumerate() {
                let b = i as f64 - HALF_WINDOW as f64;
                let a = TAU * b * shift as f64 / FFT as f64;
                *h *= Complex32::new(a.cos() as f32, a.sin() as f32);
            }
            self.log.push(format!("window moved by {shift} samples"));
        }
    }

    /// The products of neighbouring preamble pilots for the pilot phase of carrier index `k0`, with their sign from w^P:
    /// (sum of d sign, sum of |d|).
    fn index_metric(&self, k0: usize) -> f32 {
        let lo = -(HALF_WINDOW as isize) + 6;
        let hi = HALF_WINDOW as isize;
        let seq = sequences();
        let mut s = Complex32::default();
        let mut m = 0f32;
        let first = lo + ((6 - (k0 as isize + lo).rem_euclid(6)) % 6);
        let mut b = first;
        while b <= hi {
            let k = (k0 as isize + b) as usize;
            if !k.is_multiple_of(BLOCK) {
                let d = self.bin(b) * self.bin(b - 6).conj();
                m += d.norm();
                if seq.w_p(k) == 1 {
                    s -= d;
                } else {
                    s += d;
                }
            }
            b += 6;
        }
        if m > 0.0 {
            s.norm() / m
        } else {
            0.0
        }
    }

    /// The carrier index of bin 0 with the centre known: the best of the positions within the span of the nominal one.
    fn index_known(&mut self, _g: Guard) -> Option<usize> {
        let f = self.cfg.centre_hz + self.shift_hz + self.nco_freq * self.cfg.raster.rate();
        let nominal = (f / self.cfg.raster.spacing()).round() as isize;
        let span = self.cfg.index_span as isize;
        let mut best = (0f32, 0isize);
        for d in -span..=span {
            let k = nominal + d;
            if k < HALF_WINDOW as isize {
                continue;
            }
            let m = self.index_metric(k as usize);
            if m > best.0 {
                best = (m, k);
            }
        }
        self.log.push(format!("carrier index metric {:.2}", best.0));
        (best.0 >= self.cfg.index_metric).then_some(best.1 as usize)
    }

    /// The carrier index of bin 0 with the centre unknown: the pilot phase from the pilot energy, then every pair of
    /// (k mod 3408, k mod 2047), joined by the Chinese remainder theorem.
    fn index_unknown(&mut self, _g: Guard) -> Option<usize> {
        let half = HALF_WINDOW as isize;
        let mut e = [0f32; 6];
        for b in -half..=half {
            e[b.rem_euclid(6) as usize] += self.bin(b).norm_sqr();
        }
        let p = (0..6).max_by(|&a, &b| e[a].total_cmp(&e[b]))? as isize;
        // Pilot bins b = p (mod 6); products with the pilot 6 below.
        let first = -half + 6 + (p - (-half + 6)).rem_euclid(6);
        let bs: Vec<isize> = (0..).map(|j| first + 6 * j).take_while(|&b| b <= half).collect();
        let d: Vec<Complex32> = bs.iter().map(|&b| self.bin(b) * self.bin(b - 6).conj()).collect();
        let total: f32 = d.iter().map(|v| v.norm()).sum();
        if total == 0.0 {
            return None;
        }
        let seq = sequences();
        let wseq: Vec<f32> = (0..2047).map(|i| if seq.w(i) == 1 { -1.0 } else { 1.0 }).collect();
        let mut best = (0f32, 0usize);
        let mut u = vec![Complex32::default(); d.len()];
        // a = k0 mod 3408 with k0 + p = 0 (mod 6).
        for a in (0..BLOCK).filter(|a| (*a as isize + p).rem_euclid(6) == 0) {
            for (j, (&b, &dj)) in bs.iter().zip(&d).enumerate() {
                let kk = (a as isize + b).rem_euclid(BLOCK as isize) as usize;
                u[j] = if kk == 0 {
                    Complex32::default()
                } else if seq.w_prime(kk / 6) == 1 {
                    -dj
                } else {
                    dj
                };
            }
            for c in 0..2047usize {
                let mut s = Complex32::default();
                for (j, &b) in bs.iter().enumerate() {
                    s += u[j] * wseq[(c as isize + b).rem_euclid(2047) as usize];
                }
                let m = s.norm() / total;
                if m > best.0 {
                    best = (m, a * 2047 + c);
                }
            }
        }
        self.log.push(format!("full carrier index search metric {:.2}", best.0));
        if best.0 < self.cfg.index_metric {
            return None;
        }
        let (a, c) = (best.1 / 2047, best.1 % 2047);
        // k = a (mod 3408), k = c (mod 2047).
        let m = BLOCK * 2047;
        (0..2047)
            .map(|t| a + BLOCK * t)
            .find(|k| k % 2047 == c)
            .map(|k| k % m)
    }

    /// One preamble symbol: channel, L1 block cells, header, the L1 row of this symbol; after the last one the L1.
    fn preamble_symbol(&mut self, g: Guard) {
        let Some(k0) = self.k0 else {
            self.set_state(State::Timing);
            return;
        };
        let rot = self.grid_phase(k0, g);
        for v in self.bins.iter_mut() {
            *v *= rot;
        }
        let a_pp = g.a_pp();
        let half = HALF_WINDOW as isize;
        // Pilots every 6 carriers.
        let first_b = -half + (6 - (k0 as isize - half).rem_euclid(6)) % 6;
        let first_k = (k0 as isize + first_b) as usize;
        let n_p = ((half - first_b) / 6 + 1) as usize;
        let mut r = Vec::new();
        let seq = sequences();
        seq.pilot_bits(first_k, n_p, &mut r);
        let mut grid: Vec<Complex32> = (0..n_p)
            .map(|j| {
                let b = first_b + 6 * j as isize;
                let p = if r[j] == 0 { a_pp } else { -a_pp };
                self.bin(b) / p
            })
            .collect();
        // A pilot at 0 Hz of the input takes the mean of its neighbours.
        let dc_b = self.dc_bin();
        if first_b <= dc_b && (dc_b - first_b) % 6 == 0 {
            let j = ((dc_b - first_b) / 6) as usize;
            if j > 0 && j + 1 < n_p {
                grid[j] = (grid[j - 1] + grid[j + 1]) * 0.5;
            }
        }
        // Noise from second differences with the mean delay taken out.
        let mut acc = Complex32::default();
        for j in 1..n_p {
            acc += grid[j] * grid[j - 1].conj();
        }
        let theta = acc.im.atan2(acc.re);
        let mags: Vec<f32> = grid.iter().map(|h| h.norm_sqr()).collect();
        let mut sorted = mags.clone();
        sorted.sort_by(f32::total_cmp);
        let med = sorted[sorted.len() / 2];
        let (mut sum, mut cnt) = (0f64, 0usize);
        for j in 1..n_p - 1 {
            if mags[j - 1] < 0.25 * med || mags[j] < 0.25 * med || mags[j + 1] < 0.25 * med {
                continue;
            }
            let rotj = |i: usize| grid[i] * Complex32::from_polar(1.0, -theta * i as f32);
            let d2 = rotj(j - 1) - rotj(j) * 2.0 + rotj(j + 1);
            sum += f64::from(d2.norm_sqr());
            cnt += 1;
        }
        let noise = if cnt > 0 {
            ((a_pp * a_pp) as f64 * sum / cnt as f64 / 6.0) as f32
        } else {
            1.0
        };
        let noise = noise.max(1e-9 * med);
        self.pre_noise = noise;
        // The channel over the window.
        let interp = GridInterp::new(6, 4).ok();
        let mut h = vec![Complex32::default(); (n_p - 1) * 6 + 1];
        if let Some(gi) = interp {
            let _ = gi.fill(&grid, &mut h);
        }
        grid.clear();
        for (i, o) in self.pre_h.iter_mut().enumerate() {
            let b = i as isize - half;
            let j = (b - first_b).clamp(0, h.len() as isize - 1) as usize;
            *o = h[j];
        }
        // The L1 block cells, both copies of a cell combined.
        let mut cz = vec![Complex32::default(); L1_CELLS];
        let mut cw = vec![0f32; L1_CELLS];
        let sys = self.status.l1.as_ref().map(|l| (l.k_min(), l.k_max()));
        for b in -half..=half {
            let k = (k0 as isize + b) as usize;
            if let Some((lo, hi)) = sys {
                if k < lo || k > hi {
                    continue;
                }
            }
            if b == dc_b {
                // The carrier at 0 Hz of the input is erased.
                continue;
            }
            let Some(q) = l1_cell(k % BLOCK) else { continue };
            let hh = self.pre_h[(b + half) as usize];
            let p = hh.norm_sqr();
            if p <= 0.0 {
                continue;
            }
            let mut z = self.bin(b) / hh;
            if seq.w(k) == 1 {
                z = -z;
            }
            let wt = p / noise;
            cz[q] += z * wt;
            cw[q] += wt;
        }
        // Frequency de-interleaving: x[H(q)] = a[q]; both parities on the first symbol.
        let parities: &[bool] = if self.pre_index == 0 { &[false, true] } else { &[self.l1_parity ^ (self.pre_index % 2 == 1)] };
        let mut best: Option<HeaderTry> = None;
        let mut addr = Vec::new();
        for &odd in parities {
            freq_addresses(odd, L1_CELLS, &mut addr);
            let mut xz = vec![Complex32::default(); L1_CELLS];
            let mut xw = vec![0f32; L1_CELLS];
            for q in 0..L1_CELLS {
                let i = addr[q] as usize;
                if cw[q] > 0.0 {
                    xz[i] = cz[q] / cw[q];
                    xw[i] = cw[q];
                }
            }
            if let Some(d) = self.hdr.decode(&xz[..32], &xw[..32], true) {
                if best.as_ref().is_none_or(|b| d.agree > b.0) {
                    best = Some((d.agree, odd, d.bits, xz, xw));
                }
            }
        }
        let Some((agree, odd, bits, xz, xw)) = best else {
            self.preamble_failed();
            return;
        };
        if self.pre_index == 0 {
            if agree < self.cfg.packets.agree {
                self.preamble_failed();
                return;
            }
            let (info, mode) = preamble_fields(bits);
            let Some(sizes) = L1Sizes::new(usize::from(info), mode) else {
                self.preamble_failed();
                return;
            };
            self.sizes = Some(sizes);
            self.l1_parity = odd;
            if self.codec.as_ref().map(|c| *c.sizes()) != Some(sizes) {
                self.codec = L1Codec::new(sizes);
            }
        }
        let Some(sizes) = self.sizes else { return };
        // Combine the cyclic repetitions of header and row.
        let row = sizes.cells_per_symbol();
        let unit = 32 + row;
        let mut uz = vec![Complex32::default(); unit];
        let mut uw = vec![0f32; unit];
        for i in 0..L1_CELLS {
            uz[i % unit] += xz[i] * xw[i];
            uw[i % unit] += xw[i];
        }
        for j in 32..unit {
            self.l1_cells.push(if uw[j] > 0.0 { uz[j] / uw[j] } else { Complex32::default() });
            self.l1_w.push(uw[j]);
        }
        self.pre_index += 1;
        if self.pre_index < sizes.symbols {
            return;
        }
        // The L1.
        let decoded = match self.codec.as_mut() {
            Some(c) => c.decode(&self.l1_cells, &self.l1_w, &mut self.l1_bits),
            None => None,
        };
        let l1 = decoded.and_then(|_| L1Part2::from_bits(&self.l1_bits));
        self.l1_cells.clear();
        self.l1_w.clear();
        match l1 {
            Some(l1) => {
                self.status.l1_ok += 1;
                self.move_proved = true;
                self.since_timing = 0;
                self.missed = 0;
                if self.status.l1.as_ref() != Some(&l1) {
                    let same_layout = self.status.l1.as_ref().is_some_and(|o| same_shape(o, &l1));
                    if !same_layout {
                        self.path = None;
                    }
                    self.status.l1 = Some(l1);
                }
            }
            None => {
                self.status.l1_bad += 1;
                self.log.push("L1 failed".into());
                // The last good L1 serves one frame; a second failure in a row means the frame is lost: back to the
                // preamble search.
                if self.status.l1.is_none() || self.missed > 0 {
                    self.preamble_failed();
                    return;
                }
                self.missed += 1;
            }
        }
        self.status.frames += 1;
        self.frame_sym = 0;
        let mut starts = None;
        if self.path.is_none() {
            self.path = self.build_path(g);
            if std::mem::take(&mut self.moved) {
                // The window moved onto the slice: its channel comes from the next preamble.
                self.data_like = 0;
                self.since_timing = 0;
                self.state = State::Timing;
                self.status.state = Some(State::Timing);
                return;
            }
        } else if let (Some(p), Some(l1)) = (self.path.as_ref(), self.status.l1.as_ref()) {
            starts = Some(p.slices.iter().map(|sp| plp_start(l1, sp.id, sp.plp)).collect());
        }
        self.init_grid(starts);
        self.set_state(State::Data);
    }

    fn preamble_failed(&mut self) {
        self.missed += 1;
        self.l1_cells.clear();
        self.l1_w.clear();
        if self.missed >= self.cfg.preamble_misses {
            self.lose("no preamble");
            return;
        }
        self.data_like = 0;
        self.set_state(State::Timing);
    }

    /// The data path of the current L1 for the chosen slice and PLP: the slice of the PLP (all slices in the window that carry
    /// it when it is bundled) and the slice of the common PLP of its group.
    ///
    /// When the slice of the PLP or of its common PLP reaches out of the window, the tuning positions that the L1 gives for the
    /// two slices are tried: the first whose window holds both is the target. The window moves onto the target when the
    /// samples hold it (`moved` is set); otherwise `retune_hz` names it. With no such position `outside` names the slice out of
    /// the window, and the data PLP is decoded without its common PLP when its own slice is inside.
    fn build_path(&mut self, g: Guard) -> Option<DataPath> {
        let l1 = self.status.l1.clone()?;
        let k0 = self.k0?;
        let dx = l1.guard.d_x();
        if l1.guard != g {
            self.log.push("the L1 guard differs from the guard found".into());
        }
        let layout = layout_of(&l1);
        // The carriers that a window centred on carrier `c` holds.
        let window = |c: usize| (layout.k_min.max(c.saturating_sub(HALF_WINDOW)), layout.k_max.min(c + HALF_WINDOW));
        let (k_lo, k_hi) = window(k0);
        if k_hi <= k_lo {
            return None;
        }
        let start = l1.k_min();
        let covers = |c: usize, ds: (usize, usize)| {
            let (lo, hi) = window(c);
            ds.0 >= lo && ds.1 <= hi
        };
        let inside = |ds: (usize, usize)| covers(k0, ds);
        // The chosen slice and PLP: the first that fits the settings, one inside the window first.
        let mut candidates = Vec::new();
        for s in &l1.slices {
            if self.cfg.slice.is_some_and(|id| id != s.id) {
                continue;
            }
            let Some(ds) = s.carriers(start, dx) else { continue };
            let plp = match self.cfg.plp {
                Some(id) => s.plps.iter().find(|p| p.id == id),
                None => s.plps.iter().find(|p| p.plp_type != 0),
            };
            if let Some(p) = plp {
                candidates.push((s.clone(), *p, ds));
            }
        }
        let Some((s, p, ds)) = candidates.iter().find(|c| inside(c.2)).or(candidates.first()).cloned() else {
            self.log.push("no slice carries the PLP".into());
            return None;
        };
        // The common PLP of the group of a grouped data PLP, with its slice.
        let common = (p.plp_type == 1)
            .then(|| {
                l1.slices.iter().find_map(|o| {
                    let c = o.plps.iter().find(|q| q.plp_type == 0 && q.group == p.group)?;
                    Some((o.clone(), *c, o.carriers(start, dx)?))
                })
            })
            .flatten();
        let both = |c: usize| covers(c, ds) && common.as_ref().is_none_or(|o| covers(c, o.2));
        let sp = self.cfg.raster.spacing();
        self.status.retune_hz = None;
        self.status.outside = None;
        let with_common = both(k0);
        if !with_common {
            // The slice that reaches out of the window, and by how many carriers.
            let out = [Some((s.id, ds)), common.as_ref().map(|o| (o.0.id, o.2))]
                .into_iter()
                .flatten()
                .find(|o| !inside(o.1));
            if let Some((id, d)) = out {
                let by = k_lo.saturating_sub(d.0).max(d.1.saturating_sub(k_hi));
                self.status.outside = Some((id, by as f64 * sp));
            }
            let target = [Some(s.tune_carrier(start, dx)), common.as_ref().map(|o| o.0.tune_carrier(start, dx))]
                .into_iter()
                .flatten()
                .find(|&t| t >= HALF_WINDOW && both(t));
            match target {
                Some(tune) => {
                    // The window moves when the samples hold it at the tuning position.
                    let from_input = tune as f64 * sp - (self.cfg.centre_hz + self.nco_freq * self.cfg.raster.rate());
                    let held = from_input.abs() + HALF_WINDOW as f64 * sp <= 0.45 * self.cfg.rate_hz;
                    if held && !self.move_failed {
                        let dk = tune as f64 - k0 as f64;
                        self.shift_hz += dk * sp;
                        self.shift_phase = 0.0;
                        // The samples already resampled hold the old window only: the preamble search waits for new ones.
                        self.settle = self.base + self.buf.len() as u64 + SETTLE as u64;
                        self.move_proved = false;
                        self.missed = 0;
                        self.k0 = Some(tune);
                        self.status.window_carrier = Some(tune);
                        self.status.chosen = None;
                        self.moved = true;
                        self.log.push(format!("window moved by {dk} carriers to the tuning position {tune}"));
                        return None;
                    }
                    self.status.retune_hz = Some(tune as f64 * sp);
                    self.log.push(format!("a slice lies outside the samples: tune to {:.0} Hz", tune as f64 * sp));
                }
                None => {
                    if let Some((id, by)) = self.status.outside {
                        self.log.push(format!(
                            "slice {id} reaches {by:.0} Hz out of the window; no tuning position of the L1 holds the data \
                             slice with the slice of its common PLP"
                        ));
                    }
                }
            }
            if !inside(ds) {
                self.status.chosen = None;
                return None;
            }
        }
        self.status.chosen = Some((s.id, p.id));
        // The slices to decode and the PLPs each passes on.
        let mut want: Vec<(super::l1::Slice, [Option<u8>; 2])> = vec![(s.clone(), [Some(p.id), None])];
        let mut bundled = 1;
        if p.bundled {
            for o in l1.slices.iter().filter(|o| o.id != s.id) {
                if !o.plps.iter().any(|q| q.id == p.id && q.bundled) {
                    continue;
                }
                match o.carriers(start, dx) {
                    Some(d) if inside(d) => {
                        want.push((o.clone(), [Some(p.id), None]));
                        bundled += 1;
                    }
                    _ => self.log.push(format!("bundled slice {} lies outside the window: its packets are lost", o.id)),
                }
            }
        }
        self.status.bundled_slices = bundled;
        let mut roles = vec![(p.id, p.payload, false)];
        self.status.common = None;
        if let Some((o, c, _)) = common.filter(|_| with_common) {
            match want.iter_mut().find(|w| w.0.id == o.id && o.type2) {
                Some(w) => w.1[1] = Some(c.id),
                None => want.push((o, [Some(c.id), None])),
            }
            roles.push((c.id, c.payload, true));
            self.status.common = Some(c.id);
        }
        let mut pilots: [Vec<(u32, f32)>; D_Y] = Default::default();
        let first6 = k_lo.div_ceil(6) * 6;
        let mut r = Vec::new();
        sequences().pilot_bits(first6, (k_hi - first6) / 6 + 1, &mut r);
        let dc = usize::try_from(k0 as isize + self.dc_bin()).ok();
        for (d, list) in pilots.iter_mut().enumerate() {
            for k in k_lo..=k_hi {
                // The carrier at 0 Hz of the input carries what is left of the DC: its pilot is not used.
                if layout.class(k, d) == Cell::Pilot && Some(k) != dc {
                    let bit = r[(k - first6) / 6];
                    list.push(((k - k_lo) as u32, if bit == 0 { PILOT_AMP } else { -PILOT_AMP }));
                }
            }
        }
        let mut slices = Vec::new();
        let mut specs = Vec::new();
        let mut qam = None;
        for (o, plps) in want {
            let ds = o.carriers(start, dx)?;
            let first = o.plps.iter().find(|q| Some(q.id) == plps[0])?;
            let modcod = (!o.type2)
                .then(|| {
                    Some(ModCod {
                        long: first.long,
                        qam: Qam::from_code(first.plp_mod)?,
                        rate: CodeRate::from_code(first.plp_cod)?,
                    })
                })
                .flatten();
            if slices.is_empty() {
                qam = modcod.map(|m| m.qam);
            }
            let setup = SliceSetup {
                type2: o.type2,
                robust: !o.efficient_header,
                plps,
                modcod,
                w_rm: self.cfg.w_rm,
            };
            let payload: [Vec<u32>; D_Y] = std::array::from_fn(|d| {
                let mut v = Vec::new();
                slice_payload(&layout, ds, d, self.cfg.ti_full_width, &mut v);
                v
            });
            let depth = o.depth();
            let cols = ds.1 - ds.0 + 1;
            self.log.push(format!(
                "slice {} carriers {} to {}, PLP {:?}{}",
                o.id,
                ds.0,
                ds.1,
                plps.iter().flatten().collect::<Vec<_>>(),
                modcod.map(|m| format!(", {}", m.text())).unwrap_or_default()
            ));
            specs.push(SliceSpec {
                id: o.id,
                payload: payload.clone(),
                ti: TwistedBlock::new(depth, cols).ok()?,
                depth,
                cols,
                setup,
                plp_start: (!o.type2).then_some(usize::from(first.start)),
            });
            slices.push(SlicePath {
                ds,
                payload,
                id: o.id,
                plp: plps[0],
            });
        }
        let g0 = k_lo.div_ceil(dx) * dx;
        let n_grid = (k_hi - g0) / dx + 1;
        let holes = (0..n_grid)
            .filter(|&i| layout.class(g0 + i * dx, 0) == Cell::Off)
            .collect();
        let interp = GridInterp::new(dx, 6).ok()?;
        let path = DataPath {
            dx,
            k0,
            k_lo,
            k_hi,
            g0,
            grid: vec![Complex32::default(); n_grid],
            count: vec![0.0; n_grid],
            holes,
            interp,
            h: vec![Complex32::default(); k_hi - k_lo + 1],
            pilots,
            dc,
            a: Vec::new(),
            slices,
        };
        self.send(C2Cells::Path(Arc::new(PathSetup {
            slices: specs,
            qam,
            roles,
            bundled,
            plp: p.id,
            common: self.status.common,
            packets: self.cfg.packets,
            iterations: self.cfg.iterations,
            bundle_hold: self.cfg.bundle_hold,
        })));
        Some(path)
    }

    /// Starts the channel grid of the data symbols from the last preamble's channel, and the frame in the decoding stage with
    /// the PLP_START of each slice in `starts` when the L1 gave it again.
    fn init_grid(&mut self, starts: Option<Vec<Option<usize>>>) {
        if self.path.is_none() {
            return;
        }
        self.send(C2Cells::Frame(starts));
        let Some(p) = self.path.as_mut() else { return };
        let half = HALF_WINDOW as isize;
        for (i, (g, c)) in p.grid.iter_mut().zip(p.count.iter_mut()).enumerate() {
            let k = p.g0 + i * p.dx;
            let b = k as isize - p.k0 as isize;
            if *c == 0.0 {
                *g = self.pre_h[(b + half).clamp(0, 2 * half) as usize];
                *c = 1.0;
            }
        }
        if self.noise == 0.0 {
            self.noise = self.pre_noise;
        }
        Self::interpolate(p);
    }

    fn interpolate(p: &mut DataPath) {
        for &i in &p.holes {
            let left = (0..i).rev().find(|j| !p.holes.contains(j));
            let right = (i + 1..p.grid.len()).find(|j| !p.holes.contains(j));
            p.grid[i] = match (left, right) {
                (Some(a), Some(b)) => {
                    let t = (i - a) as f32 / (b - a) as f32;
                    p.grid[a] * (1.0 - t) + p.grid[b] * t
                }
                (Some(a), None) => p.grid[a],
                (None, Some(b)) => p.grid[b],
                (None, None) => Complex32::default(),
            };
        }
        let off = p.g0 - p.k_lo;
        let n = p.h.len() - off;
        let mut tmp = std::mem::take(&mut p.h);
        if p.interp.fill(&p.grid, &mut tmp[off..off + n.min(p.grid.len() * p.dx)]).is_err() {
            p.h = tmp;
            return;
        }
        let edge = tmp[off];
        for v in tmp[..off].iter_mut() {
            *v = edge;
        }
        let last = (off + n.min(p.grid.len() * p.dx)).min(tmp.len());
        if last < tmp.len() {
            let e = tmp[last - 1];
            for v in tmp[last..].iter_mut() {
                *v = e;
            }
        }
        p.h = tmp;
    }

    fn data_symbol(&mut self, g: Guard) {
        let l = self.frame_sym;
        self.frame_sym += 1;
        if self.frame_sym >= DATA_SYMBOLS {
            self.frame_sym = 0;
            self.pre_index = 0;
            self.sizes = None;
            self.set_state(State::L1);
        }
        let Some(k0) = self.k0 else { return };
        let rot = self.grid_phase(k0, g);
        let Some(mut p) = self.path.take() else { return };
        let d = l % D_Y;
        let n = p.k_hi - p.k_lo + 1;
        let mut y = std::mem::take(&mut p.a);
        y.clear();
        let first = (p.k_lo as isize - k0 as isize).rem_euclid(FFT as isize) as usize;
        for i in 0..n {
            y.push(self.bins[(first + i) % FFT] * rot);
        }
        // Common phase and slope against the channel.
        let fit = fit_phase(&p.pilots[d], &y, &p.h, (p.k_lo as isize - k0 as isize) as f64);
        let (phi, slope) = fit.unwrap_or((0.0, 0.0));
        let base_b = p.k_lo as f64 - k0 as f64;
        let step = Complex32::from_polar(1.0, -slope as f32);
        let mut turn = Complex32::from_polar(1.0, -(phi + slope * base_b) as f32);
        for (i, v) in y.iter_mut().enumerate() {
            if i % 256 == 0 {
                turn = Complex32::from_polar(1.0, -(phi + slope * (base_b + i as f64)) as f32);
            }
            *v *= turn;
            turn *= step;
        }
        // Grid update and noise.
        let (mut err, mut cnt) = (0f64, 0usize);
        for &(off, val) in &p.pilots[d] {
            let k = p.k_lo + off as usize;
            if k < p.g0 || !(k - p.g0).is_multiple_of(p.dx) {
                continue;
            }
            let gi = (k - p.g0) / p.dx;
            let est = y[off as usize] / val;
            err += f64::from((y[off as usize] - p.grid[gi] * val).norm_sqr());
            cnt += 1;
            let c = &mut p.count[gi];
            *c = (*c + 1.0).min(GRID_AVERAGE);
            let old = p.grid[gi];
            p.grid[gi] = old + (est - old) / *c;
        }
        if cnt > 0 {
            let s = (err / cnt as f64) as f32;
            self.noise = if self.noise > 0.0 { 0.9 * self.noise + 0.1 * s } else { s };
        }
        Self::interpolate(&mut p);
        // Loops: carrier from the change of the common phase, window and clock from the slope.
        let ns = g.symbol_len() as f64;
        // Symbols since the last measurement (more across a preamble).
        let gap = self.meas_sym.map_or(1, |m| self.sym.saturating_sub(m).max(1)) as f64;
        self.meas_sym = Some(self.sym);
        if let Some(prev) = self.phase_prev {
            let dphi = (phi - prev + std::f64::consts::PI).rem_euclid(TAU) - std::f64::consts::PI;
            let f = self.nco_freq + 0.3 * dphi / TAU / (ns * gap);
            let at = self.win;
            self.set_nco(f, at);
        }
        self.phase_prev = Some(phi);
        let tau = -slope * FFT as f64 / TAU;
        let total = tau + self.shifts;
        if let Some(prev) = self.timing_prev {
            self.drift_acc += total - prev;
            self.drift_n += gap as u32;
            if self.drift_n >= 16 {
                let rate = self.drift_acc / (self.drift_n as f64 * ns);
                let c = (self.correction * (1.0 + rate)).clamp(1.0 - 2e-3, 1.0 + 2e-3);
                self.correction = c;
                if let Some(r) = self.resampler.as_mut() {
                    r.set_correction(c);
                }
                self.drift_acc = 0.0;
                self.drift_n = 0;
            }
        }
        self.timing_prev = Some(total);
        if tau.abs() >= 0.75 {
            let shift = tau.round();
            self.win = (self.win as i64 + shift as i64) as u64;
            self.shifts += shift;
        }
        self.status.clock_ppm = (self.correction - 1.0) * 1e6;
        self.status.cfo_hz = self.offset_hz(k0);
        // The slice cells, equalised, to the decoding stage.
        let noise = self.noise.max(1e-12);
        let mut cells = Vec::with_capacity(p.slices.len());
        let mut weights = Vec::with_capacity(p.slices.len());
        for (idx, sp) in p.slices.iter().enumerate() {
            let mut a = Vec::with_capacity(sp.payload[d].len());
            let mut aw = Vec::with_capacity(sp.payload[d].len());
            let mut sig_power = 0f64;
            for &off in &sp.payload[d] {
                let k = sp.ds.0 + off as usize;
                let i = k - p.k_lo;
                let hh = p.h[i];
                let pw = hh.norm_sqr();
                // The carrier at 0 Hz of the input is erased.
                if pw > 0.0 && Some(k) != p.dc {
                    a.push(y[i] / hh);
                    aw.push(pw / noise);
                    sig_power += f64::from(pw);
                } else {
                    a.push(Complex32::default());
                    aw.push(0.0);
                }
            }
            if idx == 0 && !sp.payload[d].is_empty() {
                let es = sig_power / sp.payload[d].len() as f64;
                self.status.esn0_db = Some(10.0 * (es / f64::from(noise)).log10());
            }
            cells.push(a);
            weights.push(aw);
        }
        self.send(C2Cells::Symbol { l, cells, weights });
        p.a = y;
        self.path = Some(p);
    }
}

/// PLP_START of PLP `plp` of slice `slice` when it is of type 1.
fn plp_start(l1: &L1Part2, slice: u8, plp: Option<u8>) -> Option<usize> {
    let s = l1.slices.iter().find(|s| s.id == slice)?;
    if s.type2 {
        return None;
    }
    s.plps.iter().find(|p| Some(p.id) == plp).map(|p| usize::from(p.start))
}

/// Whether two L1s describe the same carriers, slices and codings (they may differ in PLP_START and counters).
fn same_shape(a: &L1Part2, b: &L1Part2) -> bool {
    let strip = |l: &L1Part2| {
        let mut l = l.clone();
        l.change_counter = 0;
        for s in &mut l.slices {
            for p in &mut s.plps {
                p.start = 0;
            }
        }
        l
    };
    strip(a) == strip(b)
}

/// The common phase and phase slope (radians per carrier) of a symbol against the channel `h` over its pilots by weighted
/// least squares: `y` and `h` from carrier `k_lo`, `b0` the bin of `k_lo`. The phase is at bin 0.
fn fit_phase(pilots: &[(u32, f32)], y: &[Complex32], h: &[Complex32], b0: f64) -> Option<(f64, f64)> {
    if pilots.len() < 2 {
        return None;
    }
    let z = |j: usize| {
        let (off, v) = pilots[j];
        let i = off as usize;
        y[i] * (h[i] * v).conj()
    };
    let (mut num, mut den) = (0f64, 0f64);
    for j in 1..pilots.len() {
        let d = z(j) * z(j - 1).conj();
        let dk = f64::from(pilots[j].0 - pilots[j - 1].0);
        let wt = f64::from(d.norm());
        num += wt * f64::from(d.im.atan2(d.re)) / dk;
        den += wt;
    }
    let s0 = if den > 0.0 { num / den } else { 0.0 };
    let mut sum = Complex32::default();
    for j in 0..pilots.len() {
        let b = f64::from(pilots[j].0) + b0;
        sum += z(j) * Complex32::from_polar(1.0, (-s0 * b) as f32);
    }
    let c0 = f64::from(sum.im.atan2(sum.re));
    let (mut sw, mut sk, mut sa, mut skk, mut ska) = (0f64, 0f64, 0f64, 0f64, 0f64);
    for j in 0..pilots.len() {
        let b = f64::from(pilots[j].0) + b0;
        let zz = z(j) * Complex32::from_polar(1.0, (-c0 - s0 * b) as f32);
        let (wt, ang) = (f64::from(zz.norm()), f64::from(zz.im.atan2(zz.re)));
        sw += wt;
        sk += wt * b;
        sa += wt * ang;
        skk += wt * b * b;
        ska += wt * b * ang;
    }
    let det = sw * skk - sk * sk;
    if sw <= 0.0 || det.abs() < 1e-12 * sw * sw.max(1.0) {
        return Some((c0, s0));
    }
    let slope = (sw * ska - sk * sa) / det;
    let a = (sa - slope * sk) / sw;
    Some((c0 + a, s0 + slope))
}
