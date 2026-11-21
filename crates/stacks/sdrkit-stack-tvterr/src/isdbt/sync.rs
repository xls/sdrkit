//! ISDB-T synchronisation: the channel finder and width, the resampling to the IFFT rate, mode and guard from the cyclic
//! prefix, the timing, carrier and clock loops, and the TMCC search and decode. These stages close loops on each other sample by
//! sample, so one component (`isdbt.sync`) runs them.
//!
//! Standard: ARIB STD-B31 clause 3.12 to 3.15.

use super::layout::{seg_carriers, seg_table, Band, EdgePilot, Layout, SEG_AT_POS};
use super::tmcc::{decode_soft, Config, Tmcc, SYNC_W0};
use super::{Width, GUARDS};
use crate::band::{resampler, strongest_box, AvgSpectrum, BandFft};
use num_complex::Complex32;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::DspError;
use sdrkit_algo_modem::ofdm::{ClockDrift, FftFramer, LagCorrelator, OfdmLayout, PilotPattern};
use sdrkit_graph::SlicePool;
use std::f64::consts::TAU;
use std::sync::Arc;

/// One demodulated symbol, once the TMCC is known.
#[derive(Clone, Debug, Default)]
pub struct Symbol {
    /// Signal time of the symbol's start, s.
    pub time: f64,
    /// Index in the frame.
    pub frame_pos: usize,
    /// The band's carriers, low to high (corrected for the integer offset).
    pub carriers: Arc<[Complex32]>,
    /// The TMCC in force.
    pub tmcc: Tmcc,
    /// Mode.
    pub mode: u8,
    /// The band the carriers cover.
    pub band: Band,
    /// Changes whenever the configuration (or the lock) changes: the stages after start again.
    pub epoch: u32,
}

/// Which segments a receiver takes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Segments {
    /// The 13 segments of ISDB-T.
    #[default]
    Full,
    /// One segment (the centre one).
    One,
    /// Three segments.
    Three,
    /// One segment first, three once a TMCC of ISDB-TSB announces them and the input rate carries them.
    Auto,
}

/// The synchronisation state, sent at each change and with every frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncState {
    /// 0 searching, 1 mode found, 2 TMCC decoded.
    pub stage: u8,
    /// Width in use, MHz.
    pub width_mhz: f64,
    /// The width setting is automatic.
    pub width_auto: bool,
    /// Mode (0 before it is found).
    pub mode: u8,
    /// Guard fraction.
    pub guard: f64,
    /// Total carrier offset, Hz (the channel's place in the band included).
    pub cfo_hz: f64,
    /// Sample-clock correction, ppm.
    pub clock_ppm: f64,
    /// Integer carrier shift found by the TMCC search.
    pub shift: i32,
    /// Symbol index in the frame (−1 before TMCC).
    pub symbol: i32,
    /// Seconds since the last good TMCC (−1 before one).
    pub since_tmcc_s: f64,
    /// The TMCC, once decoded (as sent: for one segment inside a wider signal its layers B and C are not decoded).
    pub tmcc: Option<Tmcc>,
    /// Segments in use.
    pub segments: u8,
    /// I and Q are swapped.
    pub mirrored: bool,
    /// Where the channel was placed, Hz from the centre of the band.
    pub place_hz: f64,
    /// Log lines since the last state.
    pub log: Vec<String>,
}

struct Place {
    spec: AvgSpectrum,
    ffts: usize,
    tries: usize,
    held: Vec<Complex32>,
    done: bool,
    box_hz: f64,
    guesses: Vec<(f64, bool)>,
    guess: usize,
    since: u64,
    nco: Nco,
}

impl Place {
    fn new(rate: f64) -> Place {
        Place {
            spec: AvgSpectrum::new(BandFft::N1024),
            ffts: 0,
            tries: 0,
            held: Vec::new(),
            done: false,
            box_hz: 0.0,
            guesses: vec![(0.0, false)],
            guess: 0,
            since: 0,
            nco: Nco::new(rate, 0.0),
        }
    }
    fn current(&self) -> (f64, bool) {
        self.guesses[self.guess % self.guesses.len()]
    }
    /// Mirror and mix to 0 Hz, phase continuous.
    fn apply(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        let (hz, mirrored) = self.current();
        let start = out.len();
        if mirrored {
            out.extend(x.iter().map(|v| v.conj()));
        } else {
            out.extend_from_slice(x);
        }
        if hz != 0.0 {
            self.nco.set_freq(-hz);
            self.nco.mix_in_place(&mut out[start..]);
        }
    }
}

struct Track {
    mode: u8,
    band: Band,
    n: usize,
    g: usize,
    p: usize,
    w: usize,
    r: usize,
    t_sym: f64,
    t_ref: f64,
    f: f64,
    theta: f64,
    n_ref: f64,
    corr: LagCorrelator,
    framer: FftFramer,
    buf: Vec<Complex32>,
    spec: Vec<Complex32>,
    prev: Vec<Complex32>,
    have_prev: bool,
    drift: f64,
    drift_syms: usize,
    clock_fit: ClockDrift,
    count: u64,
    // TMCC search.
    tabs: Vec<[Vec<u16>; 2]>,
    rows: Vec<Complex32>,
    nrows: usize,
    since_search: usize,
    fails: usize,
    // Locked.
    locked: Option<Locked>,
}

struct Locked {
    tmcc: Tmcc,
    layout: Layout,
    frame_pos: usize,
    soft: [f32; 204],
    have: [bool; 204],
    bad: u32,
    last_good: f64,
}

/// The ISDB-T synchronisation.
pub struct IsdbtSync {
    width_setting: Width,
    segments: Segments,
    band: Band,
    tried_three: bool,
    rate: f64,
    width: f64,
    widths_failed: usize,
    place: Place,
    rs: FractionalResampler<Complex32>,
    clock: f64,
    mixed: Vec<Complex32>,
    x: Vec<Complex32>,
    x0: u64,
    consumed: u64,
    origin: f64,
    last_win: Option<(u8, usize)>,
    trk: Option<Track>,
    epoch: u32,
    state: SyncState,
    dirty: bool,
    prefer_mirror: bool,
    pool: SlicePool<Complex32>,
}

const CAP: usize = 2 * 204 + 16 + 100;
const NEED: usize = 2 * 204 + 16;

impl IsdbtSync {
    /// For an input `rate`, the `width` setting, the segments to take and the upper-edge pilot rule of a narrow band.
    ///
    /// # Errors
    ///
    /// [`DspError`] if no resampler serves `rate`.
    pub fn new(
        rate: f64,
        width_setting: Width,
        segments: Segments,
        edge: EdgePilot,
    ) -> Result<IsdbtSync, DspError> {
        let auto = width_setting == Width::Auto;
        let ns = match segments {
            Segments::Full => 13,
            Segments::Three => 3,
            Segments::One | Segments::Auto => 1,
        };
        let band = Band::of(ns, edge);
        let width = match width_setting {
            Width::W7 => 7.0,
            Width::W8 => 8.0,
            Width::Auto if ns == 13 && rate >= 0.95 * 8e6 => 8.0,
            Width::Auto | Width::W6 => 6.0,
        };
        let mut s = IsdbtSync {
            width_setting,
            segments,
            band,
            tried_three: false,
            rate,
            width,
            widths_failed: 0,
            place: Place::new(rate),
            rs: Self::resampler(rate, width, band)?,
            clock: 1.0,
            mixed: Vec::new(),
            x: Vec::new(),
            x0: 0,
            consumed: 0,
            origin: 0.0,
            last_win: None,
            trk: None,
            epoch: 0,
            state: SyncState {
                width_mhz: width,
                width_auto: auto,
                segments: ns as u8,
                symbol: -1,
                since_tmcc_s: -1.0,
                ..Default::default()
            },
            dirty: true,
            prefer_mirror: false,
            pool: SlicePool::default(),
        };
        s.state.width_mhz = s.width;
        Ok(s)
    }
    fn resampler(
        rate: f64,
        width: f64,
        band: Band,
    ) -> Result<FractionalResampler<Complex32>, DspError> {
        // The 13 segments fill 0.686 of the IFFT band: flat to its edge, cut at the lower Nyquist frequency. A narrow band is
        // flat to a little past its own edge.
        let fs = band.ifft_rate(width);
        let pass = if band.ns == 13 {
            0.35 * fs
        } else {
            0.515 * band.occupied_hz(width)
        };
        resampler(rate, fs, pass, 0.5 * fs.min(rate))
    }
    /// The IFFT rate in use.
    pub fn fs(&self) -> f64 {
        self.band.ifft_rate(self.width)
    }
    /// The band in use.
    pub fn band(&self) -> Band {
        self.band
    }
    /// Whether the input rate carries a band of `ns` segments of a raster `w` MHz wide.
    fn fits(&self, ns: usize, w: f64) -> bool {
        fits(self.rate, ns, w)
    }
    /// The width in use, MHz.
    pub fn width(&self) -> f64 {
        self.width
    }
    /// The state now.
    pub fn state(&self) -> &SyncState {
        &self.state
    }
    /// Tries the mirrored orientation first (the one found before a retune).
    pub fn prefer_mirrored(&mut self, yes: bool) {
        self.prefer_mirror = yes;
    }
    /// Forgets the log lines already read.
    pub fn clear_log(&mut self) {
        self.state.log.clear();
    }
    /// Whether the state changed since the last call (and clears the flag).
    pub fn take_dirty(&mut self) -> bool {
        std::mem::replace(&mut self.dirty, false)
    }
    fn log(&mut self, line: String) {
        self.state.log.push(line);
        self.dirty = true;
    }
    /// Drops the lock, the mode and the TMCC: the signal is another one. The search starts again.
    pub fn forget(&mut self) {
        self.restart();
        self.dirty = true;
    }
    /// Starts the chain after the place again (a new guess or width).
    fn restart(&mut self) {
        // The widths tried are those the input rate carries, so the resampler of the first width stays only if the rate
        // cannot make the new one.
        if let Ok(rs) = Self::resampler(self.rate, self.width, self.band) {
            self.rs = rs;
        }
        self.rs.set_correction(self.clock);
        self.x.clear();
        self.x0 = 0;
        self.origin = self.consumed as f64 / self.rate;
        self.last_win = None;
        if self.trk.as_ref().is_some_and(|t| t.locked.is_some()) {
            self.log("TMCC lost".into());
        }
        self.trk = None;
        self.place.since = 0;
        self.epoch += 1;
        let st = &mut self.state;
        st.stage = 0;
        st.mode = 0;
        st.symbol = -1;
        st.tmcc = None;
        st.shift = 0;
        st.width_mhz = self.width;
        st.segments = self.band.ns as u8;
        let (hz, m) = self.place.current();
        st.place_hz = hz;
        st.mirrored = m;
        st.cfo_hz = hz;
        self.dirty = true;
    }
    fn next_guess(&mut self) {
        self.place.guess += 1;
        let wrapped = self.place.guess.is_multiple_of(self.place.guesses.len());
        if wrapped && self.segments == Segments::Auto && self.band.ns == 3 {
            // Three segments failed at every place: back to one.
            self.band = Band::of(1, self.band.edge);
            self.log("three segments not found: back to one segment".into());
            self.place.guess = 0;
        } else if wrapped && self.width_setting == Width::Auto {
            // Every guess failed at this width: the next width in the order 6, 8, 7, 6 that the rate can carry.
            self.widths_failed += 1;
            let order = [6.0, 8.0, 7.0];
            let i = order.iter().position(|&w| w == self.width).unwrap_or(0);
            let mut nw = order[(i + 1) % 3];
            if !self.fits(self.band.ns, nw) {
                nw = 6.0;
            }
            self.width = nw;
            self.place.guess = 0;
        }
        self.restart();
    }

    /// Feeds input samples. Symbols decoded (once the TMCC is known) go to `out`.
    pub fn feed(&mut self, input: &[Complex32], out: &mut Vec<Symbol>) {
        if !self.place.done {
            self.place.held.extend_from_slice(input);
            let need = 48 * 1024;
            while !self.place.done && self.place.held.len() >= (self.place.ffts + 1) * 1024 {
                let b = self.place.ffts;
                self.place
                    .spec
                    .add_all(&self.place.held[b * 1024..(b + 1) * 1024]);
                self.place.ffts += 1;
                if self.place.ffts * 1024 >= need {
                    self.finish_place();
                }
            }
            if !self.place.done {
                return;
            }
            let held = std::mem::take(&mut self.place.held);
            self.run(&held, out);
            return;
        }
        self.run(input, out);
    }

    fn finish_place(&mut self) {
        let p = self.place.spec.mean();
        let (rate, ns) = (self.rate, self.band.ns);
        let bins = move |w: f64| (occupied(ns, w) / rate * 1024.0).round() as usize;
        let n = p.len();
        let six = strongest_box(&p, bins(6.0).min(n));
        let mut ok = six.is_some_and(|b| b.2 || bins(6.0) >= n);
        if self.width_setting == Width::Auto {
            if let Some((_, m6, _)) = six {
                let mut best = 6.0;
                for w in [7.0, 8.0] {
                    if !fits(rate, ns, w) || bins(w) > n {
                        continue;
                    }
                    if let Some((_, m, edge_ok)) = strongest_box(&p, bins(w)) {
                        if m >= 0.94 * m6 && (edge_ok || bins(w) >= n - 2) {
                            best = w;
                        }
                    }
                }
                self.width = best;
            }
        }
        let wb = bins(self.width).min(n);
        let bx = strongest_box(&p, wb);
        if let Some(b) = bx {
            ok = b.2 || wb + 4 >= n;
            let centre = if ns == 13 {
                b.0 as f64 + wb as f64 / 2.0
            } else {
                // The centre segment of a narrow band is the centre of the widest signal around it: a 3-segment box as strong
                // as the 1-segment one, else the middle of the 1-segment boxes that are nearly as strong as the best.
                let w3 = (occupied(3, self.width) / rate * 1024.0).round() as usize;
                match (w3 <= n).then(|| strongest_box(&p, w3)).flatten() {
                    Some((s3, m3, true)) if ns == 1 && m3 >= 0.94 * b.1 => {
                        ok = true;
                        s3 as f64 + w3 as f64 / 2.0
                    }
                    _ => plateau_centre(&p, wb),
                }
            };
            self.place.box_hz = (centre - n as f64 / 2.0) * self.rate / n as f64;
        }
        self.place.tries += 1;
        if !ok && self.place.tries < 16 {
            // Silence or noise at the start: measure again on later samples.
            self.place.spec.clear();
            let drop = self.place.ffts * 1024;
            self.place.held.drain(..drop.min(self.place.held.len()));
            self.place.ffts = 0;
            return;
        }
        let c = self.place.box_hz;
        self.place.guesses = if !ok || c.abs() < 40e3 {
            vec![(0.0, false), (0.0, true)]
        } else {
            vec![(c, false), (0.0, false), (-c, true), (0.0, true)]
        };
        if self.prefer_mirror {
            // The orientation found before a retune goes first.
            self.place.guesses.sort_by_key(|g| !g.1);
        }
        self.place.guess = 0;
        self.place.done = true;
        self.restart();
    }

    /// Input goes through in stretches aligned on the input count, and every decision that drops samples (a restart) is taken at
    /// a stretch's end, so the result does not depend on how the input is cut.
    fn run(&mut self, input: &[Complex32], out: &mut Vec<Symbol>) {
        const B: u64 = 4096;
        let mut i = 0;
        while i < input.len() {
            let n = ((B - self.consumed % B) as usize).min(input.len() - i);
            let chunk = &input[i..i + n];
            i += n;
            self.consumed += n as u64;
            self.mixed.clear();
            let mut mixed = std::mem::take(&mut self.mixed);
            self.place.apply(chunk, &mut mixed);
            self.place.since += n as u64;
            self.rs.process(&mixed, &mut self.x);
            self.mixed = mixed;
            if self.consumed.is_multiple_of(B) {
                self.stretch_end(out);
            }
        }
    }

    fn stretch_end(&mut self, out: &mut Vec<Symbol>) {
        loop {
            let progressed = if self.trk.is_none() {
                self.acquire()
            } else {
                self.track(out)
            };
            if !progressed {
                break;
            }
        }
        // A guess is given up after 3 s of input without a TMCC lock.
        let locked = self.trk.as_ref().is_some_and(|t| t.locked.is_some());
        if !locked && self.place.since as f64 > 3.0 * self.rate {
            self.next_guess();
        }
        if let Some(t) = &self.trk {
            if let Some(l) = &t.locked {
                let now = self.origin + t.t_sym / self.fs();
                self.state.since_tmcc_s = now - l.last_good;
            }
        }
    }

    /// One acquisition attempt on exactly the first `wait` samples held. False when more samples are needed.
    fn acquire(&mut self) -> bool {
        // Eight symbols of the longest mode and guard, plus one, to fold every candidate over the last 8 symbols.
        let wait = 10 * (8192 + 2048);
        if self.x.len() < wait {
            return false;
        }
        let band = self.band;
        let found = search(&self.x[..wait], band);
        match found {
            Some((mode, gi, frac, pk)) if self.last_win == Some((mode, gi)) => {
                let n = band.fft_size(mode);
                let g = (n as f64 * GUARDS[gi]) as usize;
                let per = n + g;
                // Guard starts sit at `pk` modulo the period. The first one far enough in to search around it.
                let w = (g / 2).clamp(4, 32);
                let mut t = pk as f64;
                while t < (w + 2) as f64 {
                    t += per as f64;
                }
                let spacing = self.fs() / n as f64;
                let r = ((60e3 / spacing).ceil() as usize).clamp(8, 80);
                let nc = band.carriers(mode);
                let p0 = band.first_pos();
                let tabs: Vec<[Vec<u16>; 2]> = (0..band.ns)
                    .map(|pos| {
                        let s = SEG_AT_POS[p0 + pos];
                        [
                            seg_table(mode, s, false).tmcc,
                            seg_table(mode, s, true).tmcc,
                        ]
                    })
                    .collect();
                self.trk = Some(Track {
                    mode,
                    band,
                    n,
                    g,
                    p: per,
                    w,
                    r,
                    t_sym: self.x0 as f64 + t,
                    t_ref: self.x0 as f64 + t,
                    f: frac,
                    theta: 0.0,
                    n_ref: self.x0 as f64 + t,
                    corr: LagCorrelator::new(n, g, 1.0, 1.0)
                        .expect("a prefix of at least one sample"),
                    framer: search_framer(nc, n, g, r),
                    buf: vec![Complex32::new(0.0, 0.0); per + 2 * w + 2],
                    spec: vec![Complex32::new(0.0, 0.0); nc + 2 * r],
                    prev: vec![Complex32::new(0.0, 0.0); nc + 2 * r],
                    have_prev: false,
                    drift: 0.0,
                    drift_syms: 0,
                    clock_fit: ClockDrift::new(1.0).expect("no forgetting is valid"),
                    count: 0,
                    tabs,
                    rows: vec![Complex32::new(0.0, 0.0); CAP * 2 * band.ns * (2 * r + 1)],
                    nrows: 0,
                    since_search: 0,
                    fails: 0,
                    locked: None,
                });
                let st = &mut self.state;
                st.stage = 1;
                st.mode = mode;
                st.guard = GUARDS[gi];
                st.cfo_hz = self.place.current().0 + frac * spacing;
                self.log(format!(
                    "{} found: mode {mode} ({:.0} kHz carrier spacing), guard interval {}, CFO {:+.1} Hz",
                    if band.ns == 13 { "ISDB-T".to_string() } else { format!("{} segment{}", band.ns, if band.ns == 1 { "" } else { "s" }) },
                    spacing / 1e3,
                    super::Guard::of(GUARDS[gi]).text(),
                    frac * spacing
                ));
                true
            }
            other => {
                self.last_win = other.map(|f| (f.0, f.1));
                // Retry on later samples: keep half the wait.
                let cut = wait - wait / 2;
                self.x.drain(..cut);
                self.x0 += cut as u64;
                true
            }
        }
    }

    /// One symbol of tracking. False when more samples are needed.
    fn track(&mut self, out: &mut Vec<Symbol>) -> bool {
        let fs = self.fs();
        let t = self
            .trk
            .as_mut()
            .expect("a tracking step runs only after an acquisition");
        let (n, g, p, w) = (t.n, t.g, t.p, t.w);
        let t0 = t.t_sym.floor() as i64;
        let first = t0 - w as i64 - 1;
        if first < self.x0 as i64 {
            // The grid fell behind the samples held: reacquire.
            self.trk = None;
            self.restart();
            return true;
        }
        let first = first as u64;
        let need = (first - self.x0) as usize + p + 2 * w + 2;
        if self.x.len() < need {
            return false;
        }
        // Derotate the stretch by the carrier offset, phase continuous in absolute samples.
        let a = (first - self.x0) as usize;
        let wrad = -TAU * t.f / n as f64;
        let ph0 = t.theta + wrad * (first as f64 - t.n_ref);
        let mut nco = Nco::new(TAU, wrad);
        nco.set_phase_cycles(ph0 / TAU);
        let len = t.buf.len();
        nco.mix(&self.x[a..a + len], &mut t.buf);
        // Cyclic-prefix correlation around the expected guard start (buf index w + 1 is t0). The correlator's window ends `n + g -
        // 1` samples after the guard start it stands for, and its correlation is that of the prefix with the symbol's end.
        let base = w + 1;
        let lo = base - w;
        let mut best = (f32::NEG_INFINITY, 0i64, Complex32::new(0.0, 0.0));
        let mut mags = [0f32; 3];
        let mut prevm = [0f32; 2];
        t.corr.reset();
        for &v in &t.buf[lo..lo + g + n - 1] {
            t.corr.push(v);
        }
        for dk in 0..=2 * w {
            let (_, pc) = t.corr.push(t.buf[lo + dk + g + n - 1]);
            let m = pc.norm();
            if m > best.0 {
                best = (m, dk as i64 - w as i64, pc);
                mags = [prevm[1], m, 0.0];
            } else if dk as i64 - w as i64 == best.1 + 1 {
                mags[2] = m;
            }
            prevm = [prevm[1], m];
        }
        let mut e = best.1 as f64 - (t.t_sym - t0 as f64);
        let den = mags[0] - 2.0 * mags[1] + mags[2];
        if best.1.unsigned_abs() < w as u64 && den.abs() > 1e-12 {
            e += (0.5 * (mags[0] - mags[2]) / den).clamp(-0.5, 0.5) as f64;
        }
        let eps = t.corr.freq_of(best.2) * n as f64;
        // FFT window a little inside the guard. Whole-sample steps of the window against a fixed grid are taken out of the carriers
        // (the slow drift the grid does not follow is left to the equaliser's slope tracking: a correction of the timing
        // estimate's fractional jitter puts that jitter on the carriers).
        t.framer.set_timing(t.t_ref - t0 as f64);
        t.t_ref += p as f64;
        t.framer
            .demodulate(&t.buf, base + g, 0, &mut t.spec)
            .expect("the stretch holds the symbol and the band");
        let time = self.origin + t.t_sym / fs;
        // Loops: timing (gain 0.08), carrier (gain 0.15, phase continuous), clock (a line through the timing of at least 200 symbols).
        let step = 0.08 * e.clamp(-(w as f64), w as f64);
        t.t_sym += p as f64 + step;
        // The clock is measured once the timing loop has settled.
        if t.count >= 64 {
            t.drift += step;
            t.clock_fit.push((t.drift_syms * p) as f64, t.drift);
            t.drift_syms += 1;
        }
        let next_ref = t.t_sym.floor() - w as f64 - 1.0;
        let new_theta = t.theta + wrad * (next_ref - t.n_ref);
        t.theta = new_theta.rem_euclid(TAU);
        t.n_ref = next_ref;
        if eps.abs() < 0.5 {
            t.f += 0.15 * eps;
        }
        t.count += 1;
        let mut clock_change = None;
        if t.drift_syms >= 200 && t.drift.abs() > 8.0 {
            let err = (t.clock_fit.ppm().unwrap_or(0.0) * 1e-6).clamp(-200e-6, 200e-6);
            clock_change = Some(1.0 + 0.8 * err);
            t.drift = 0.0;
            t.drift_syms = 0;
            t.clock_fit.reset();
        }
        // Drop what no later symbol needs.
        let keep_from = (t.t_sym.floor() as i64 - w as i64 - 2).max(self.x0 as i64) as u64;
        let cut = (keep_from - self.x0) as usize;
        if cut > 0 {
            self.x.drain(..cut.min(self.x.len()));
            self.x0 += cut as u64;
        }
        if let Some(f) = clock_change {
            // Symbols coming out long (the window drifting late): the resampler takes input faster.
            self.clock *= f;
            self.rs.set_correction(self.clock);
            self.state.clock_ppm = (self.clock - 1.0) * 1e6;
        }
        let spacing = fs / n as f64;
        let t = self
            .trk
            .as_mut()
            .expect("a tracking step runs only after an acquisition");
        self.state.cfo_hz = self.place.current().0 + t.f * spacing;
        if t.locked.is_none() {
            self.search_step();
        } else {
            self.locked_step(time, out);
        }
        if let Some(t) = &mut self.trk {
            std::mem::swap(&mut t.prev, &mut t.spec);
            t.have_prev = true;
        }
        true
    }

    fn search_step(&mut self) {
        let fits3 = self.fits(3, self.width);
        let t = self
            .trk
            .as_mut()
            .expect("a tracking step runs only after an acquisition");
        if !t.have_prev {
            return;
        }
        let ls = seg_carriers(t.mode);
        let r = t.r as i64;
        let ns = 2 * t.r + 1;
        let nseg = t.band.ns;
        let row = 2 * nseg * ns;
        if t.nrows == CAP {
            t.rows.copy_within(row.., 0);
            t.nrows -= 1;
        }
        let at = t.nrows * row;
        for pos in 0..nseg {
            for ty in 0..2 {
                for si in 0..ns {
                    let sh = si as i64 - r;
                    let mut acc = Complex32::new(0.0, 0.0);
                    for &k in &t.tabs[pos][ty] {
                        let ci = (r + (pos * ls + k as usize) as i64 + sh) as usize;
                        acc += t.spec[ci] * t.prev[ci].conj();
                    }
                    t.rows[at + (pos * 2 + ty) * ns + si] = acc;
                }
            }
        }
        t.nrows += 1;
        t.since_search += 1;
        if t.nrows < NEED || t.since_search < 100 {
            return;
        }
        t.since_search = 0;
        let auto = self.segments == Segments::Auto;
        let found = tmcc_search(t).filter(|f| {
            // A TMCC that does not fit the band is taken only to change over to three segments.
            f.3.current.within(t.band.ns).is_some()
                || (auto && t.band.ns == 1 && f.3.system == 1 && f.3.current.total() == 3)
        });
        match found {
            Some((sh, n0, odd, tmcc)) => {
                let band = t.band;
                if auto
                    && band.ns == 1
                    && !self.tried_three
                    && tmcc.system == 1
                    && tmcc.current.total() == 3
                    && fits3
                {
                    // An ISDB-TSB signal of three segments that the rate carries: change over once.
                    self.tried_three = true;
                    self.band = Band::of(3, band.edge);
                    self.log("TMCC of three segments: changing over to three segments".into());
                    self.restart();
                    return;
                }
                let last = t.nrows - 1;
                let into = last - n0;
                let frame_pos = into % 204;
                let _odd_now = odd ^ ((into / 204) % 2 == 1);
                t.f += sh as f64;
                let eff = effective(&tmcc, band.ns);
                let layout = Layout::new(t.mode, &eff.current, band);
                let now = self.origin + t.t_sym / self.band.ifft_rate(self.width);
                t.locked = Some(Locked {
                    tmcc: eff,
                    layout,
                    frame_pos,
                    soft: [0.0; 204],
                    have: [false; 204],
                    bad: 0,
                    last_good: now,
                });
                t.have_prev = false;
                t.nrows = 0;
                self.epoch += 1;
                let st = &mut self.state;
                st.stage = 2;
                st.shift = sh as i32;
                st.tmcc = Some(tmcc);
                st.symbol = frame_pos as i32;
                st.since_tmcc_s = 0.0;
                let line = format!("TMCC decoded: {}", layers_text(&tmcc.current));
                self.log(line);
            }
            None => {
                // Drop the oldest 100 symbols. After 8 failed searches, back to acquisition and the next guess.
                let row = 2 * t.band.ns * (2 * t.r + 1);
                t.rows.copy_within(100 * row..t.nrows * row, 0);
                t.nrows -= 100;
                t.fails += 1;
                if t.fails >= 8 {
                    self.next_guess();
                }
            }
        }
    }

    fn locked_step(&mut self, time: f64, out: &mut Vec<Symbol>) {
        let t = self
            .trk
            .as_mut()
            .expect("a tracking step runs only after an acquisition");
        let r = t.r;
        let nc = t.band.carriers(t.mode);
        let band = t.band;
        let l = t
            .locked
            .as_mut()
            .expect("a locked step runs only while the TMCC lock is held");
        l.frame_pos = (l.frame_pos + 1) % 204;
        let fp = l.frame_pos;
        if t.have_prev {
            // The TMCC bit of this symbol: differential over every TMCC carrier, the common phase step (from the squared products
            // of the TMCC, AC and continual carriers) taken out.
            let lay = &l.layout;
            let mut z2 = Complex32::new(0.0, 0.0);
            let mut sum = Complex32::new(0.0, 0.0);
            for s in 0..lay.ns() {
                let tab = &lay.tables[s];
                for &k in tab.tmcc.iter().chain(&tab.ac1).chain(&tab.ac2) {
                    let c = r + lay.band(s, k as usize);
                    let d = t.spec[c] * t.prev[c].conj();
                    z2 += d * d / (d.norm() + 1e-20);
                }
                if lay.diff[s] {
                    let c = r + lay.band(s, 0);
                    let d = t.spec[c] * t.prev[c].conj();
                    z2 += d * d / (d.norm() + 1e-20);
                }
                for &k in &tab.tmcc {
                    let c = r + lay.band(s, k as usize);
                    sum += t.spec[c] * t.prev[c].conj();
                }
            }
            let phi = z2.arg() / 2.0;
            let soft = -(sum * Complex32::new(phi.cos(), -phi.sin())).re;
            l.soft[fp] = soft;
            l.have[fp] = true;
        }
        let mut emit_tmcc = None;
        if fp == 203 {
            // The frame's word: sync correlation, then the code.
            let w: [f32; 16] = std::array::from_fn(|i| {
                if (SYNC_W0 >> (15 - i)) & 1 == 1 {
                    1.0
                } else {
                    -1.0
                }
            });
            let (mut c, mut a) = (0f32, 0f32);
            for i in 0..16 {
                c += l.soft[1 + i] * w[i];
                a += l.soft[1 + i].abs();
            }
            let complete = l.have[1..].iter().all(|&h| h);
            let mut good = false;
            if complete && a > 0.0 && c.abs() / a >= 0.4 {
                let soft: [f32; 184] = std::array::from_fn(|i| -l.soft[20 + i]);
                if let Some(bits) = decode_soft(&soft) {
                    let tm = Tmcc::from_info(&bits[..102]);
                    if tm.current.within(band.ns).is_some() {
                        good = true;
                        emit_tmcc = Some(tm);
                    }
                }
            }
            l.have = [false; 204];
            if good {
                l.bad = 0;
                l.last_good = time;
            } else {
                l.bad += 1;
            }
        }
        let lost = t.locked.as_ref().is_some_and(|l| l.bad >= 4);
        if lost {
            self.trk = None;
            self.log("TMCC lost".into());
            self.restart();
            return;
        }
        let t = self
            .trk
            .as_mut()
            .expect("a tracking step runs only after an acquisition");
        // The symbol goes out under the frame's own parameters. What the frame's TMCC says applies from the next frame.
        let l = t
            .locked
            .as_ref()
            .expect("a locked step runs only while the TMCC lock is held");
        out.push(Symbol {
            time,
            frame_pos: fp,
            carriers: self
                .pool
                .exact(nc, |v| v.copy_from_slice(&t.spec[r..r + nc])),
            tmcc: l.tmcc,
            mode: t.mode,
            band,
            epoch: self.epoch,
        });
        self.state.symbol = fp as i32;
        if fp == 203 {
            self.dirty = true;
        }
        let l = t
            .locked
            .as_mut()
            .expect("a locked step runs only while the TMCC lock is held");
        let mut changed = false;
        if let Some(tm) = emit_tmcc {
            // At countdown 0 the next frame takes the announced parameters.
            let mut sent = tm;
            if tm.countdown == 0 && tm.next.within(band.ns).is_some() && tm.next != tm.current {
                sent.current = tm.next;
                sent.countdown = 15;
            }
            let apply = effective(&sent, band.ns);
            if apply != l.tmcc {
                if apply.current != l.tmcc.current {
                    l.layout = Layout::new(t.mode, &apply.current, band);
                    changed = true;
                }
                l.tmcc = apply;
                self.state.tmcc = Some(sent);
                self.dirty = true;
            }
        }
        if changed {
            self.epoch += 1;
            let line = format!(
                "TMCC decoded: {}",
                layers_text(&apply_text(&self.state.tmcc))
            );
            self.log(line);
        }
    }
}

/// Whether an input `rate` carries a band of `ns` segments of a raster `w` MHz wide: 13 segments need 0.95 of the raster's
/// width, a narrow band 1.05 times its occupied width.
fn fits(rate: f64, ns: usize, w: f64) -> bool {
    if ns == 13 {
        rate >= 0.95 * 6e6 * w / 6.0
    } else {
        rate >= 1.05 * Band::of(ns, EdgePilot::Sequence).occupied_hz(w)
    }
}

/// The centre (in bins) of the boxes of `w` bins whose power is at least 0.97 of the strongest: the middle of a flat signal wider
/// than the box.
fn plateau_centre(p: &[f64], w: usize) -> f64 {
    let n = p.len();
    if w == 0 || w > n {
        return n as f64 / 2.0;
    }
    let mut csum = vec![0.0; n + 1];
    for i in 0..n {
        csum[i + 1] = csum[i] + p[i];
    }
    let sums: Vec<f64> = (0..=n - w).map(|s| csum[s + w] - csum[s]).collect();
    let m = sums.iter().copied().fold(0.0, f64::max);
    let first = sums.iter().position(|&v| v >= 0.97 * m).unwrap_or(0);
    let last = sums.iter().rposition(|&v| v >= 0.97 * m).unwrap_or(first);
    (first + last) as f64 / 2.0 + w as f64 / 2.0
}

/// The width of the occupied band in Hz, for the channel finder.
fn occupied(ns: usize, w: f64) -> f64 {
    if ns == 13 {
        5.572e6 * w / 6.0
    } else {
        Band::of(ns, EdgePilot::Sequence).occupied_hz(w)
    }
}

fn apply_text(t: &Option<Tmcc>) -> Config {
    t.map(|t| t.current).unwrap_or_default()
}

/// The TMCC as a receiver of `ns` segments decodes it: the current and next configurations restricted to the band
/// ([`Config::within`]). The caller has checked that the current one fits.
fn effective(t: &Tmcc, ns: usize) -> Tmcc {
    let current = t.current.within(ns).unwrap_or(t.current);
    Tmcc {
        current,
        next: t.next.within(ns).unwrap_or(current),
        ..*t
    }
}

/// "A: 1 segment QPSK 2/3, B: 12 segments 64QAM 3/4 (with a one-segment layer)".
pub fn layers_text(c: &Config) -> String {
    let mut parts = Vec::new();
    for (i, l) in c.layers.iter().enumerate() {
        if l.used() {
            parts.push(format!(
                "{}: {} segment{} {} {}",
                ["A", "B", "C"][i],
                l.segments,
                if l.segments == 1 { "" } else { "s" },
                l.modulation_name(),
                l.rate_name()
            ));
        }
    }
    let mut s = parts.join(", ");
    if c.partial {
        s.push_str(" (with a one-segment layer)");
    }
    s
}

/// Mode, guard index, fractional offset (spacings) and the guard-start position (modulo the period, from the start of `x`), from
/// the correlation of the cyclic prefix with the end of the symbol. The correlation is folded over 8 symbol periods for 13
/// segments and up to 32 for a narrow band, whose guards are short.
pub fn search(x: &[Complex32], band: Band) -> Option<(u8, usize, f64, usize)> {
    let mut best: Option<(f64, u8, usize, f64, usize)> = None;
    for mode in 1..=3u8 {
        let n = band.fft_size(mode);
        for (gi, &g) in GUARDS.iter().enumerate() {
            let gl = (n as f64 * g) as usize;
            let per = n + gl;
            let syms = if band.ns == 13 {
                8usize
            } else {
                ((x.len().saturating_sub(n + gl)) / per).min(32)
            };
            if syms < 8 || x.len() < per * syms + n + gl {
                continue;
            }
            let span = per * syms;
            let start = x.len() - span - n - gl;
            // The prefix correlation normalised per window, folded over the symbol period. Index `i` is a guard start: the window's
            // newest sample is then `start + i + n + gl - 1`.
            let Ok(mut lc) = LagCorrelator::new(n, gl, 1.0, 1.0) else {
                continue;
            };
            let mut fold_c = vec![Complex32::new(0.0, 0.0); per];
            for (q, &v) in x[start..start + span + n + gl - 1].iter().enumerate() {
                let (m, p) = lc.push(v);
                if q + 1 >= n + gl {
                    let i = q + 1 - n - gl;
                    let pn = p.norm();
                    if pn > 0.0 {
                        fold_c[i % per] += p * (m / pn / syms as f32);
                    }
                }
            }
            let mut pk = 0usize;
            let mut v = 0f64;
            let mut mean = 0f64;
            for i in 0..per {
                let val = fold_c[i].norm() as f64;
                mean += val;
                if val > v {
                    v = val;
                    pk = i;
                }
            }
            mean /= per as f64;
            let ratio = v / mean.max(1e-12);
            // Noise alone folds to about 1 / sqrt(8 G) with peaks of a few times that: the threshold follows it.
            let floor = 6.0 / ((syms * gl) as f64).sqrt();
            if v > 0.12f64.max(floor) && ratio > 3.0 {
                let score = v * ratio.min(12.0) / 12.0;
                if best.as_ref().is_none_or(|b| score > b.0) {
                    let frac = fold_c[pk].arg() as f64 / TAU;
                    best = Some((score, mode, gi, frac, (start + pk) % per));
                }
            }
        }
    }
    best.map(|b| (b.1, b.2, b.3, b.4))
}

/// The TMCC search over the rows kept: integer shift, frame start, parity and the decoded TMCC.
fn tmcc_search(t: &Track) -> Option<(i64, usize, bool, Tmcc)> {
    let ns = 2 * t.r + 1;
    let nseg = t.band.ns;
    let w2 = 2 * nseg;
    let row = w2 * ns;
    let rows = t.nrows;
    let w: [f32; 16] = std::array::from_fn(|i| {
        if (SYNC_W0 >> (15 - i)) & 1 == 1 {
            1.0
        } else {
            -1.0
        }
    });
    let mut soft = vec![0f32; rows * w2];
    let mut cands: Vec<(f32, i64, usize)> = Vec::new();
    for si in 0..ns {
        let sh = si as i64 - t.r as i64;
        let phi = TAU * sh as f64 * t.g as f64 / t.n as f64;
        let rot = Complex32::new(phi.cos() as f32, -phi.sin() as f32);
        for k in 0..rows {
            for pt in 0..w2 {
                soft[k * w2 + pt] = -(t.rows[k * row + pt * ns + si] * rot).re;
            }
        }
        let mut best_here = (0f32, 0usize);
        for n0 in 0..=rows - 204 {
            let (mut ct, mut at) = (0f32, 0f32);
            for pos in 0..nseg {
                let mut pick = (0f32, 0f32, -1f32);
                for ty in 0..2 {
                    let (mut c, mut a) = (0f32, 0f32);
                    for i in 0..16 {
                        let v = soft[(n0 + 1 + i) * w2 + pos * 2 + ty];
                        c += v * w[i];
                        a += v.abs();
                    }
                    let q = if a > 0.0 { c.abs() / a } else { 0.0 };
                    if q > pick.2 {
                        pick = (c, a, q);
                    }
                }
                ct += pick.0;
                at += pick.1;
            }
            let q = if at > 0.0 { ct.abs() / at } else { 0.0 };
            if q > best_here.0 {
                best_here = (q, n0);
            }
        }
        if best_here.0 >= 0.55 {
            cands.push((best_here.0, sh, best_here.1));
        }
    }
    cands.sort_by(|a, b| b.0.total_cmp(&a.0));
    for &(_, sh, n0) in cands.iter().take(3) {
        if let Some((odd, tm)) = tmcc_decode_at(t, sh, n0, &w) {
            return Some((sh, n0, odd, tm));
        }
    }
    None
}

fn tmcc_decode_at(t: &Track, sh: i64, n0: usize, w: &[f32; 16]) -> Option<(bool, Tmcc)> {
    let ns = 2 * t.r + 1;
    let nseg = t.band.ns;
    let row = 2 * nseg * ns;
    let si = (sh + t.r as i64) as usize;
    let phi = TAU * sh as f64 * t.g as f64 / t.n as f64;
    let rot = Complex32::new(phi.cos() as f32, -phi.sin() as f32);
    let soft =
        |k: usize, pos: usize, ty: usize| -(t.rows[k * row + (pos * 2 + ty) * ns + si] * rot).re;
    // Parity from the pooled sync word, then each position's type by its sync word and its own type bits.
    let mut ct = 0f32;
    for pos in 0..nseg {
        let mut pick = (0f32, -1f32);
        for ty in 0..2 {
            let (mut c, mut a) = (0f32, 0f32);
            for i in 0..16 {
                let v = soft(n0 + 1 + i, pos, ty);
                c += v * w[i];
                a += v.abs();
            }
            let q = if a > 0.0 { c.abs() / a } else { 0.0 };
            if q > pick.1 {
                pick = (c, q);
            }
        }
        ct += pick.0;
    }
    let odd = ct < 0.0;
    let sgn = if odd { -1.0 } else { 1.0 };
    let mut types = [0usize; 13];
    let mut pooled = [0f32; 204];
    for pos in 0..nseg {
        let mut best = (f32::NEG_INFINITY, 0usize);
        for ty in 0..2 {
            // Normalised by the bits' total magnitude: a differential segment's TMCC carriers include the one carrier of a
            // coherent segment at the same position (ARIB STD-B31 tables 3-14 and 3-15), so raw sums favour the type with more
            // carriers.
            let (mut c, mut a) = (0f32, 0f32);
            for i in 0..16 {
                let v = soft(n0 + 1 + i, pos, ty);
                c += sgn * v * w[i];
                a += v.abs();
            }
            let tb: f32 = (17..20).map(|i| soft(n0 + i, pos, ty)).sum();
            a += (17..20).map(|i| soft(n0 + i, pos, ty).abs()).sum::<f32>();
            let score = if a > 0.0 {
                (c + if ty == 1 { tb } else { -tb }) / a
            } else {
                0.0
            };
            if score > best.0 {
                best = (score, ty);
            }
        }
        types[pos] = best.1;
        let tb: f32 = (17..20).map(|i| soft(n0 + i, pos, best.1)).sum();
        // Condition 2: the type bits match the type assumed (positions too weak to say are let through).
        let a: f32 = (17..20).map(|i| soft(n0 + i, pos, best.1).abs()).sum();
        if a > 0.0 && (if best.1 == 1 { tb } else { -tb }) < -0.3 * a {
            return None;
        }
        for (i, p) in pooled.iter_mut().enumerate().skip(1) {
            *p += soft(n0 + i, pos, best.1);
        }
    }
    let s184: Vec<f32> = pooled[20..204].iter().map(|v| -v).collect();
    let bits = decode_soft(&s184)?;
    let tm = Tmcc::from_info(&bits[..102]);
    // A valid configuration that does not fit a band of one segment still gives that segment's type (layer A holds segment 0).
    let eff = tm
        .current
        .within(nseg)
        .or_else(|| (nseg == 1 && tm.current.valid()).then_some(tm.current))?;
    // Condition 5: every used segment agrees, pooled. The bits sent are those of the TMCC as sent, the segment types those of
    // the band.
    let lay = Layout::new(t.mode, &eff, t.band);
    let p0 = t.band.first_pos();
    let (mut c5, mut a5) = (0f32, 0f32);
    for pos in 0..nseg {
        let s = SEG_AT_POS[p0 + pos];
        let ty = lay.diff[s] as usize;
        let expect = tm.frame_bits(odd, ty == 1);
        for i in 1..204 {
            let v = soft(n0 + i, pos, ty);
            c5 += v * if expect[i] { 1.0 } else { -1.0 };
            a5 += v.abs();
        }
    }
    let _ = types;
    if a5 <= 0.0 || c5 / a5 <= 0.3 {
        return None;
    }
    Some((odd, tm))
}

/// A framer for the band's `nc` carriers and `r` carriers either side (room for the integer offset the TMCC search finds), the
/// window `min(g / 4, 24)` samples inside the guard.
fn search_framer(nc: usize, n: usize, g: usize, r: usize) -> FftFramer {
    let half = ((nc - 1) / 2 + r) as i32;
    let layout = OfdmLayout::new(n, &[g], -half, half, &[], &[], &PilotPattern::none())
        .expect("the band and the search margin fit the FFT");
    FftFramer::new(&layout, (g / 4).min(24)).expect("the backoff is inside the guard")
}
