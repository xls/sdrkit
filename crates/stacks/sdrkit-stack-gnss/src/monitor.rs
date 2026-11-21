//! The interference and spoofing monitor. It looks at the raw input and at the receiver's own solution, one second of signal
//! at a time.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::{Choice, Complex32};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// A narrowband line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub offset_hz: f64,
    pub level_db: f64,
    pub width_bins: usize,
    pub known: bool,
}

sdrkit_core::choice_enum! {
    /// The verdict of the interference monitor.
    pub enum InterferenceStatus {
        /// Nothing found.
        Clear = ("clear", "Clear"),
        /// The noise floor is raised.
        RaisedNoise = ("raised_noise", "Raised noise"),
        /// Interference is found.
        Interference = ("interference", "Interference"),
    }
}

sdrkit_core::choice_enum! {
    /// The level of an event of the interference monitor.
    pub enum EventLevel {
        /// For information.
        Info = ("info", "Info"),
        /// A warning.
        Warning = ("warning", "Warning"),
        /// An alarm.
        Alarm = ("alarm", "Alarm"),
    }
}

/// The kinds of interference that the monitor found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InterferenceKinds {
    /// A tone that sweeps through the band.
    pub swept: bool,
    /// Pulses.
    pub pulsed: bool,
    /// A narrow line that the receiver does not know.
    pub narrowband: bool,
    /// A raised noise floor.
    pub wideband: bool,
}

impl InterferenceKinds {
    /// Whether no kind is set.
    pub fn is_empty(self) -> bool {
        self == InterferenceKinds::default()
    }
    /// Whether every kind of `self` is also in `other`.
    pub fn is_subset(self, other: InterferenceKinds) -> bool {
        (!self.swept || other.swept)
            && (!self.pulsed || other.pulsed)
            && (!self.narrowband || other.narrowband)
            && (!self.wideband || other.wideband)
    }
}

/// An event of the monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorEvent {
    pub signal_s: f64,
    pub unix_s: f64,
    pub level: EventLevel,
    /// What happened, for people.
    pub text: String,
}

/// One history point.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryPoint {
    pub signal_s: f64,
    pub unix_s: f64,
    pub noise_dbfs_hz: f64,
    pub power_dbfs: f64,
    pub cn0_avg: f64,
    pub tracked: f64,
    pub flatness: f64,
    pub status: u8,
    pub spoofing: u8,
    n: u32,
}

/// The monitor's report of the last second.
#[derive(Clone, Debug, PartialEq)]
pub struct InterferenceReport {
    pub learning: bool,
    pub baseline_dbfs_hz: Option<f64>,
    pub noise_dbfs_hz: f64,
    pub power_dbfs: f64,
    pub flatness: f64,
    pub rise_db: f64,
    pub status: InterferenceStatus,
    pub kinds: InterferenceKinds,
    /// Why the monitor says so, for people.
    pub reason: String,
    pub lines: Vec<Line>,
    pub wide_parts: usize,
    pub swept_score: f64,
    pub sweep_mhz_per_us: f64,
    pub swept_bursts: usize,
    pub pulses_per_s: f64,
    pub duty: f64,
    pub pulse_db: f64,
    pub cn0_avg: f64,
    pub cn0_std: f64,
    pub cn0_ref: Option<f64>,
    pub cn0_drop: f64,
    pub spectrum: Vec<f32>,
    pub spoofing: bool,
    pub spoof_reason: String,
    pub equal_cn0: bool,
    pub position_jump: Option<(f64, f64)>,
    pub clock_jump: bool,
    pub far_from_average: bool,
    pub averaging_fixes: usize,
    pub eph_vs_almanac: Option<(String, f64)>,
}

impl Default for InterferenceReport {
    fn default() -> Self {
        InterferenceReport {
            learning: false,
            baseline_dbfs_hz: None,
            noise_dbfs_hz: 0.0,
            power_dbfs: 0.0,
            flatness: 0.0,
            rise_db: 0.0,
            status: InterferenceStatus::Clear,
            kinds: InterferenceKinds::default(),
            reason: String::new(),
            lines: Vec::new(),
            wide_parts: 0,
            swept_score: 0.0,
            sweep_mhz_per_us: 0.0,
            swept_bursts: 0,
            pulses_per_s: 0.0,
            duty: 0.0,
            pulse_db: 0.0,
            cn0_avg: 0.0,
            cn0_std: 0.0,
            cn0_ref: None,
            cn0_drop: 0.0,
            spectrum: Vec::new(),
            spoofing: false,
            spoof_reason: String::new(),
            equal_cn0: false,
            position_jump: None,
            clock_jump: false,
            far_from_average: false,
            averaging_fixes: 0,
            eph_vs_almanac: None,
        }
    }
}

/// What the receiver reports to the monitor once a second.
#[derive(Clone, Debug, Default)]
pub struct RxView {
    /// C/N0 of the channels at least locked.
    pub cn0: Vec<f64>,
    /// A fix: (signal s, ECEF, horizontal error, speed m/s, receiver clock bias m).
    pub fix: Option<(f64, [f64; 3], f64, f64, f64)>,
    /// Worst ephemeris-against-almanac distance (name, m).
    pub eph_alm: Option<(String, f64)>,
}

const NFFT: usize = 1024;

/// The monitor.
pub struct Monitor {
    fs: f64,
    fft: Fft,
    fft64: Fft,
    win: Vec<f32>,
    win64: Vec<f32>,
    buf: Vec<Complex32>,
    buf64: Vec<Complex32>,
    acc: Vec<f64>,
    frames: u32,
    power: f64,
    nsamp: u64,
    second: u64,
    samples_in_second: u64,
    // swept tone
    prev_peak: Option<i64>,
    prev_step: Option<i64>,
    strong_pairs: u32,
    swept_pairs: u32,
    steps: Vec<f64>,
    // pulses
    win_pow: Vec<f32>,
    sorted: Vec<f32>,
    pulses: u32,
    hot: u64,
    windows: u64,
    pulse_level: f64,
    sweep_up: u32,
    sweep_down: u32,
    multi: u32,
    // state
    floors: Vec<f64>,
    baseline: Option<f64>,
    known: Vec<f64>,
    cn0_window: VecDeque<f64>,
    cn0_ref: Option<f64>,
    last_clear: bool,
    status_hold: (InterferenceStatus, f64, InterferenceKinds),
    pub report: InterferenceReport,
    // spoofing
    last_fix: Option<(f64, [f64; 3], f64)>,
    jump_until: f64,
    clock_until: f64,
    last_clock: Option<(f64, f64)>,
    avg: Option<([f64; 3], usize)>,
    far_since: Option<f64>,
    prev_spoof: bool,
    prev_status: InterferenceStatus,
    prev_kinds: InterferenceKinds,
    history: VecDeque<HistoryPoint>,
    hist_step: u32,
    hist_acc: Option<HistoryPoint>,
    events: VecDeque<MonitorEvent>,
    events_added: u64,
    /// The computer's clock for event times (Unix seconds at sample 0), when known.
    pub clock0: Option<f64>,
    dc: (f32, f32),
}

impl Monitor {
    pub fn new(fs: f64) -> Monitor {
        let hann = |n: usize| Window::Hann.coefficients(n);
        Monitor {
            fs,
            fft: Fft::forward(NFFT),
            fft64: Fft::forward(64),
            win: hann(NFFT),
            win64: hann(64),
            buf: vec![Complex32::new(0.0, 0.0); NFFT],
            buf64: vec![Complex32::new(0.0, 0.0); 64],
            acc: vec![0.0; NFFT],
            frames: 0,
            power: 0.0,
            nsamp: 0,
            second: 0,
            samples_in_second: 0,
            prev_peak: None,
            prev_step: None,
            strong_pairs: 0,
            swept_pairs: 0,
            steps: Vec::new(),
            win_pow: Vec::new(),
            sorted: Vec::new(),
            pulses: 0,
            hot: 0,
            windows: 0,
            pulse_level: 0.0,
            sweep_up: 0,
            sweep_down: 0,
            multi: 0,
            floors: Vec::new(),
            baseline: None,
            known: Vec::new(),
            cn0_window: VecDeque::new(),
            cn0_ref: None,
            last_clear: false,
            status_hold: (
                InterferenceStatus::Clear,
                -1e9,
                InterferenceKinds::default(),
            ),
            report: InterferenceReport::default(),
            last_fix: None,
            jump_until: -1.0,
            clock_until: -1.0,
            last_clock: None,
            avg: None,
            far_since: None,
            prev_spoof: false,
            prev_status: InterferenceStatus::Clear,
            prev_kinds: InterferenceKinds::default(),
            history: VecDeque::new(),
            hist_step: 1,
            hist_acc: None,
            events: VecDeque::new(),
            events_added: 0,
            clock0: None,
            dc: (0.0, 0.0),
        }
    }

    /// Analyses one piece of raw input (`dc`: the front end's DC estimate). Returns true when a second ended (call
    /// [`Monitor::second`] then).
    pub fn piece(&mut self, x: &[Complex32], dc: (f64, f64)) -> bool {
        self.dc = (dc.0 as f32, dc.1 as f32);
        let d = Complex32::new(self.dc.0, self.dc.1);
        // Averaged spectrum over the first 4096 samples.
        let nfr = (x.len().min(4096)) / NFFT;
        let wsum: f64 = self.win.iter().map(|w| (*w as f64).powi(2)).sum();
        for f in 0..nfr {
            for i in 0..NFFT {
                self.buf[i] = (x[f * NFFT + i] - d) * self.win[i];
            }
            self.fft.process(&mut self.buf);
            for i in 0..NFFT {
                self.acc[i] += self.buf[i].norm_sqr() as f64 / (wsum * NFFT as f64);
            }
            self.frames += 1;
        }
        for s in x {
            self.power += (s - d).norm_sqr() as f64;
        }
        // Swept tone: 64-point spectra.
        for blk in x.as_chunks::<64>().0 {
            for i in 0..64 {
                self.buf64[i] = (blk[i] - d) * self.win64[i];
            }
            self.fft64.process(&mut self.buf64);
            // Peak bin and the mean of the others, read from the spectrum in place (no buffer per spectrum).
            let (mut k, mut pk, mut total) = (0usize, 0f32, 0f32);
            for (i, c) in self.buf64.iter().enumerate() {
                let v = c.norm_sqr();
                total += v;
                if v > pk {
                    (k, pk) = (i, v);
                }
            }
            let rest = (total - pk) / 63.0;
            if pk > 10.0 * rest {
                if let Some(pp) = self.prev_peak {
                    let mut step = k as i64 - pp;
                    if step > 32 {
                        step -= 64;
                    } else if step < -32 {
                        step += 64;
                    }
                    self.strong_pairs += 1;
                    if let Some(ps) = self.prev_step {
                        if ps != 0 && step.signum() == ps.signum() && (step - ps).abs() <= 2 {
                            self.swept_pairs += 1;
                            self.steps.push(step as f64);
                        }
                    }
                    self.prev_step = Some(step);
                }
                self.prev_peak = Some(k as i64);
            } else {
                self.prev_peak = None;
                self.prev_step = None;
            }
        }
        // Pulses: 16-sample windows over the piece.
        self.win_pow.clear();
        for w in x.as_chunks::<16>().0 {
            self.win_pow
                .push(w.iter().map(|s| (s - d).norm_sqr()).sum::<f32>() / 16.0);
        }
        // The median window power, in a buffer kept from piece to piece.
        self.sorted.clear();
        self.sorted.extend_from_slice(&self.win_pow);
        let mid = self.sorted.len() / 2;
        let med = if self.sorted.is_empty() {
            0.0
        } else {
            *self
                .sorted
                .select_nth_unstable_by(mid, |a, b| a.total_cmp(b))
                .1
        }
        .max(1e-12);
        let mut run = 0usize;
        let n = self.win_pow.len();
        for i in 0..=n {
            let hot = i < n && self.win_pow[i] > 8.0 * med;
            if hot {
                run += 1;
                self.hot += 1;
                self.pulse_level += 10.0 * (self.win_pow[i] / med).log10() as f64;
            } else if run > 0 {
                self.pulses += 1;
                if run >= 2 {
                    self.multi += 1;
                    let f = |j: usize| -> f64 {
                        let w = &x[j * 16..j * 16 + 16];
                        let mut acc = Complex32::new(0.0, 0.0);
                        for k in 1..16 {
                            acc += w[k] * w[k - 1].conj();
                        }
                        acc.arg() as f64 / TAU
                    };
                    let df = f(i - 1) - f(i - run);
                    if df > 0.01 {
                        self.sweep_up += 1;
                    } else if df < -0.01 {
                        self.sweep_down += 1;
                    }
                }
                run = 0;
            }
        }
        self.windows += n as u64;
        self.nsamp += x.len() as u64;
        self.samples_in_second += x.len() as u64;
        if self.samples_in_second as f64 >= self.fs {
            self.samples_in_second -= self.fs as u64;
            return true;
        }
        false
    }

    /// Ends a second: measures, judges, updates the report.
    pub fn second(&mut self, view: &RxView, unix: f64) {
        self.second += 1;
        let t = self.second as f64;
        let n = NFFT;
        let mut spec: Vec<f64> = (0..n)
            .map(|i| self.acc[(i + n / 2) % n] / self.frames.max(1) as f64)
            .collect();
        let bin_hz = self.fs / n as f64;
        let dens: Vec<f64> = spec.iter().map(|p| p * n as f64 / self.fs).collect();
        let edge = (0.08 * n as f64) as usize;
        let usable: Vec<usize> = (edge..n - edge)
            .filter(|&i| (i as i64 - (n / 2) as i64).abs() > 2)
            .collect();
        let mut u: Vec<f64> = usable.iter().map(|&i| dens[i]).collect();
        u.sort_by(|a, b| a.total_cmp(b));
        let floor_lin = u[u.len() / 2].max(1e-30);
        let floor = 10.0 * floor_lin.log10();
        let gm = (usable.iter().map(|&i| dens[i].max(1e-30).ln()).sum::<f64>()
            / usable.len() as f64)
            .exp();
        let am = usable.iter().map(|&i| dens[i]).sum::<f64>() / usable.len() as f64;
        let flatness = gm / am.max(1e-30);
        let power = 10.0 * (self.power / self.nsamp.max(1) as f64).max(1e-30).log10();
        // Lines.
        let mut lines = Vec::new();
        let mut wide = 0;
        let over: Vec<bool> = (0..n)
            .map(|i| usable.contains(&i) && 10.0 * (dens[i] / floor_lin).log10() > 8.0)
            .collect();
        let mut i = 0;
        while i < n {
            if !over[i] {
                i += 1;
                continue;
            }
            let start = i;
            let mut end = i;
            let mut j = i + 1;
            while j < n && (over[j] || (j + 1 < n && over[j + 1])) {
                if over[j] {
                    end = j;
                }
                j += 1;
            }
            let width = end - start + 1;
            if width > 8 {
                wide += 1;
            } else {
                let (pk, pv) =
                    (start..=end).fold(
                        (start, 0.0),
                        |a, k| if dens[k] > a.1 { (k, dens[k]) } else { a },
                    );
                let off = (pk as f64 - (n / 2) as f64) * bin_hz;
                let lvl = 10.0 * (pv / floor_lin).log10();
                let known = self
                    .known
                    .iter()
                    .any(|&k| ((k - off) / bin_hz).abs() <= 2.5);
                lines.push(Line {
                    offset_hz: off,
                    level_db: lvl,
                    width_bins: width,
                    known,
                });
            }
            i = j;
        }
        lines.sort_by(|a, b| b.level_db.total_cmp(&a.level_db));
        lines.truncate(16);
        // Swept tone.
        let swept_score = if self.strong_pairs >= 30 {
            self.swept_pairs as f64 / self.strong_pairs as f64
        } else {
            0.0
        };
        let mut st = std::mem::take(&mut self.steps);
        st.sort_by(|a, b| a.total_cmp(b));
        let med_step = st.get(st.len() / 2).copied().unwrap_or(0.0);
        let sweep_rate = (med_step * self.fs / 64.0) / (64.0 / self.fs * 1e6) / 1e6;
        let swept_bursts = self.multi >= 10
            && (self.sweep_up.max(self.sweep_down) as f64) >= 0.8 * self.multi as f64;
        let pps = self.pulses as f64;
        let duty = self.hot as f64 / self.windows.max(1) as f64;
        let pdb = if self.hot > 0 {
            self.pulse_level / self.hot as f64
        } else {
            0.0
        };
        // Satellites.
        let cn0_avg = if view.cn0.is_empty() {
            0.0
        } else {
            view.cn0.iter().sum::<f64>() / view.cn0.len() as f64
        };
        let cn0_std = if view.cn0.len() > 1 {
            (view.cn0.iter().map(|c| (c - cn0_avg).powi(2)).sum::<f64>() / view.cn0.len() as f64)
                .sqrt()
        } else {
            0.0
        };
        self.cn0_window.push_back(cn0_avg);
        while self.cn0_window.len() > 3 {
            self.cn0_window.pop_front();
        }
        let cn0_s = self.cn0_window.iter().sum::<f64>() / self.cn0_window.len() as f64;
        // Learning and judging.
        let learning = t <= 5.0;
        let mut kinds = InterferenceKinds::default();
        let mut causes: Vec<String> = Vec::new();
        let mut alarm = false;
        let mut raised = false;
        let mut rise = 0.0;
        if learning {
            self.floors.push(floor);
            for l in &lines {
                if l.level_db < 20.0 {
                    self.known.push(l.offset_hz);
                }
            }
            if t == 5.0 {
                let mut f = self.floors.clone();
                f.sort_by(|a, b| a.total_cmp(b));
                let b = f[f.len() / 2];
                self.baseline = Some(b);
                self.event(
                    t,
                    unix,
                    EventLevel::Info,
                    format!("baseline learned: {b:.1} dBFS/Hz"),
                );
            }
        } else if let Some(base) = self.baseline {
            rise = floor - base;
            let chirp = swept_score >= 0.4 || swept_bursts;
            if chirp {
                kinds.swept = true;
                alarm = true;
                causes.push(if swept_bursts {
                    format!(
                        "swept tone through the band, {} bursts a second",
                        self.multi
                    )
                } else {
                    format!("swept tone, {:.3} MHz/\u{b5}s", sweep_rate.abs())
                });
            }
            let pulsed = !swept_bursts && self.pulses >= 10 && pps >= 20.0 && duty < 0.5;
            if pulsed {
                kinds.pulsed = true;
                alarm = true;
                causes.push(format!(
                    "pulses, {:.0} a second, {:.1} dB above the band",
                    pps, pdb
                ));
            }
            let unknown: Vec<&Line> = lines
                .iter()
                .filter(|l| !l.known && l.level_db >= 10.0)
                .collect();
            if !unknown.is_empty() && !chirp {
                kinds.narrowband = true;
                alarm = true;
                let l = unknown[0];
                let mut c = format!(
                    "narrowband line at {:+.0} kHz, {:.0} dB above the floor",
                    l.offset_hz / 1e3,
                    l.level_db
                );
                if unknown.len() > 1 {
                    c.push_str(&format!(" ({} lines)", unknown.len()));
                }
                causes.push(c);
            }
            if rise >= 2.0 && !chirp && !pulsed {
                kinds.wideband = true;
                raised = true;
                if rise >= 6.0 {
                    alarm = true;
                }
                causes.push(format!(
                    "noise floor up {:.1} dB, {}",
                    rise,
                    if flatness > 0.7 {
                        "flat across the band"
                    } else {
                        "part of the band"
                    }
                ));
            }
            if let Some(r) = self.cn0_ref {
                let drop = r - cn0_s;
                if drop >= 4.0 && rise >= 1.0 {
                    alarm = true;
                    causes.push(format!("C/N0 down {drop:.1} dB"));
                }
            }
            // Baseline follows a falling floor and slowly a rising one while clear.
            let clear = kinds.is_empty() && !alarm;
            if floor < base {
                self.baseline = Some(base + 0.2 * (floor - base));
            } else if clear {
                self.baseline = Some(base + 0.01);
            }
            if clear && view.cn0.len() >= 3 {
                match self.cn0_ref {
                    None if self.last_clear => self.cn0_ref = Some(cn0_s),
                    Some(r) => self.cn0_ref = Some(r + (cn0_s - r) / 30.0),
                    _ => {}
                }
            }
            self.last_clear = clear;
        }
        let mut status = if alarm {
            InterferenceStatus::Interference
        } else if raised {
            InterferenceStatus::RaisedNoise
        } else {
            InterferenceStatus::Clear
        };
        // A status holds 3 s.
        if status != InterferenceStatus::Clear {
            self.status_hold = (status, t, kinds);
        } else if t - self.status_hold.1 < 3.0 {
            status = self.status_hold.0;
            kinds = self.status_hold.2;
        }
        if status != self.prev_status
            || (status != InterferenceStatus::Clear && !kinds.is_subset(self.prev_kinds))
        {
            let level = match status {
                InterferenceStatus::Interference => EventLevel::Alarm,
                InterferenceStatus::RaisedNoise => EventLevel::Warning,
                InterferenceStatus::Clear => EventLevel::Info,
            };
            let text = if causes.is_empty() {
                format!(
                    "status {}",
                    InterferenceStatus::CHOICES[usize::from(status.index())].key
                )
            } else {
                causes.join("; ")
            };
            self.event(t, unix, level, text);
        }
        self.prev_status = status;
        self.prev_kinds = kinds;
        // Spoofing.
        let equal_cn0 = view.cn0.len() >= 5 && cn0_std < 1.0;
        let mut jump = None;
        let mut clock_jump = false;
        let mut far = false;
        if let Some((ts, p, herr, speed, clk)) = view.fix {
            if let Some((t0, p0, _)) = self.last_fix {
                let dt = ts - t0;
                let d = crate::geo::dist(p, p0);
                if dt < 10.0 && d > 100.0 + 3.0 * speed * dt + 5.0 * herr {
                    self.jump_until = t + 60.0;
                    self.report.position_jump = Some((d, dt));
                    jump = Some((d, dt));
                    self.event(
                        t,
                        unix,
                        EventLevel::Alarm,
                        format!("position jumped {d:.0} m in {dt:.0} s"),
                    );
                }
            }
            if let Some((t0, c0)) = self.last_clock {
                let dc = clk - c0;
                let ms = C_MS;
                let near_ms = (dc / ms - (dc / ms).round()).abs() * ms < 1000.0;
                let _ = t0;
                if dc.abs() > 3000.0 && !near_ms {
                    self.clock_until = t + 60.0;
                    clock_jump = true;
                }
            }
            self.last_clock = Some((ts, clk));
            self.last_fix = Some((ts, p, herr));
            let jumping = t < self.jump_until;
            match &mut self.avg {
                None => self.avg = Some((p, 1)),
                Some((a, n)) => {
                    let d = crate::geo::dist(p, *a);
                    if *n >= 30 && d > 300.0 + 5.0 * herr {
                        far = true;
                        let since = *self.far_since.get_or_insert(t);
                        if t - since > 600.0 {
                            self.avg = Some((p, 1));
                            self.far_since = None;
                            self.event(
                                t,
                                unix,
                                EventLevel::Info,
                                "a new long-term average position starts".into(),
                            );
                        }
                    } else {
                        self.far_since = None;
                        if !jumping {
                            *n += 1;
                            for k in 0..3 {
                                a[k] += (p[k] - a[k]) / *n as f64;
                            }
                        }
                    }
                }
            }
        }
        let pos_jump = t < self.jump_until;
        let clock_j = clock_jump || t < self.clock_until;
        let eph_alm = view.eph_alm.clone().filter(|e| e.1 > 100e3);
        let mut sp = Vec::new();
        if equal_cn0 {
            sp.push("equal C/N0".to_string());
        }
        if pos_jump {
            sp.push("position jump".into());
        }
        if clock_j {
            sp.push("clock jump".into());
        }
        if far {
            sp.push("far from the long-term average".into());
        }
        if let Some((n, d)) = &eph_alm {
            sp.push(format!(
                "ephemeris against almanac ({n}, {:.0} km)",
                d / 1e3
            ));
        }
        let spoofing = !sp.is_empty();
        if spoofing != self.prev_spoof {
            self.event(
                t,
                unix,
                if spoofing {
                    EventLevel::Alarm
                } else {
                    EventLevel::Info
                },
                if spoofing {
                    format!("spoofing indicators: {}", sp.join("; "))
                } else {
                    "spoofing indicators clear".into()
                },
            );
        }
        self.prev_spoof = spoofing;
        // History.
        let status_n = sdrkit_core::Choice::index(status) as u8;
        let p = HistoryPoint {
            signal_s: t,
            unix_s: unix,
            noise_dbfs_hz: floor,
            power_dbfs: power,
            cn0_avg,
            tracked: view.cn0.len() as f64,
            flatness,
            status: status_n,
            spoofing: spoofing as u8,
            n: 1,
        };
        let acc = match self.hist_acc.take() {
            None => p,
            Some(mut a) => {
                let k = a.n as f64;
                a.noise_dbfs_hz = (a.noise_dbfs_hz * k + p.noise_dbfs_hz) / (k + 1.0);
                a.power_dbfs = (a.power_dbfs * k + p.power_dbfs) / (k + 1.0);
                a.cn0_avg = (a.cn0_avg * k + p.cn0_avg) / (k + 1.0);
                a.flatness = (a.flatness * k + p.flatness) / (k + 1.0);
                a.tracked = a.tracked.max(p.tracked);
                a.status = a.status.max(p.status);
                a.spoofing = a.spoofing.max(p.spoofing);
                a.n += 1;
                a
            }
        };
        if acc.n >= self.hist_step {
            self.history.push_back(acc);
            if self.history.len() >= 900 {
                let old: Vec<HistoryPoint> = self.history.drain(..).collect();
                for pair in old.chunks(2) {
                    let mut a = pair[0].clone();
                    if let Some(b) = pair.get(1) {
                        a.noise_dbfs_hz = 0.5 * (a.noise_dbfs_hz + b.noise_dbfs_hz);
                        a.power_dbfs = 0.5 * (a.power_dbfs + b.power_dbfs);
                        a.cn0_avg = 0.5 * (a.cn0_avg + b.cn0_avg);
                        a.flatness = 0.5 * (a.flatness + b.flatness);
                        a.tracked = a.tracked.max(b.tracked);
                        a.status = a.status.max(b.status);
                        a.spoofing = a.spoofing.max(b.spoofing);
                        a.signal_s = b.signal_s;
                    }
                    self.history.push_back(a);
                }
                self.hist_step *= 2;
            }
        } else {
            self.hist_acc = Some(acc);
        }
        for v in spec.iter_mut() {
            *v = 10.0 * (*v * n as f64 / self.fs).max(1e-30).log10();
        }
        self.report = InterferenceReport {
            learning,
            baseline_dbfs_hz: self.baseline,
            noise_dbfs_hz: floor,
            power_dbfs: power,
            flatness,
            rise_db: rise,
            status,
            kinds,
            reason: causes.join("; "),
            lines,
            wide_parts: wide,
            swept_score,
            sweep_mhz_per_us: sweep_rate.abs(),
            swept_bursts: if swept_bursts { self.multi as usize } else { 0 },
            pulses_per_s: pps,
            duty,
            pulse_db: pdb,
            cn0_avg: cn0_s,
            cn0_std,
            cn0_ref: self.cn0_ref,
            cn0_drop: self.cn0_ref.map(|r| r - cn0_s).unwrap_or(0.0),
            spectrum: spec.iter().map(|v| *v as f32).collect(),
            spoofing,
            spoof_reason: sp.join("; "),
            equal_cn0,
            position_jump: if pos_jump {
                self.report.position_jump.or(jump)
            } else {
                None
            },
            clock_jump: clock_j,
            far_from_average: far,
            averaging_fixes: self.avg.map(|a| a.1).unwrap_or(0),
            eph_vs_almanac: eph_alm,
        };
        // Reset the second's accumulators.
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.frames = 0;
        self.power = 0.0;
        self.nsamp = 0;
        self.strong_pairs = 0;
        self.swept_pairs = 0;
        self.pulses = 0;
        self.hot = 0;
        self.windows = 0;
        self.pulse_level = 0.0;
        self.sweep_up = 0;
        self.sweep_down = 0;
        self.multi = 0;
        let _ = dens;
    }

    fn event(&mut self, t: f64, unix: f64, level: EventLevel, text: String) {
        self.events.push_back(MonitorEvent {
            signal_s: t,
            unix_s: unix,
            level,
            text,
        });
        self.events_added += 1;
        while self.events.len() > 200 {
            self.events.pop_front();
        }
    }

    /// The newest events (at most 200), oldest first, each with its number since the start.
    pub fn events(&self) -> impl Iterator<Item = (u64, &MonitorEvent)> {
        let first = self.events_added - self.events.len() as u64;
        (first..).zip(self.events.iter())
    }

    /// The number of events since the start.
    pub fn events_added(&self) -> u64 {
        self.events_added
    }

    /// The history of the noise floor, one point for each period, oldest first.
    pub fn history(&self) -> impl Iterator<Item = &HistoryPoint> {
        self.history.iter()
    }
}

/// One millisecond of light, m.
const C_MS: f64 = 299_792.458;

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, s: &mut u64, sigma: f32) -> Vec<Complex32> {
        let mut g = || {
            *s ^= *s << 13;
            *s ^= *s >> 7;
            *s ^= *s << 17;
            let u1 = ((*s >> 11) as f64 / (1u64 << 53) as f64).max(1e-300);
            *s ^= *s << 13;
            *s ^= *s >> 7;
            *s ^= *s << 17;
            let u2 = (*s >> 11) as f64 / (1u64 << 53) as f64;
            ((-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()) as f32
        };
        (0..n)
            .map(|_| Complex32::new(g() * sigma, g() * sigma))
            .collect()
    }

    fn run(m: &mut Monitor, secs: usize, s: &mut u64, mut add: impl FnMut(&mut [Complex32], u64)) {
        let fs = 4e6;
        let piece = 16_384;
        let mut idx = 0u64;
        let per = (secs as f64 * fs) as usize / piece;
        for _ in 0..per {
            let mut x = noise(piece, s, 0.14);
            add(&mut x, idx);
            idx += piece as u64;
            if m.piece(&x, (0.0, 0.0)) {
                m.second(&RxView::default(), 1.7e9 + m.second as f64);
            }
        }
    }

    #[test]
    fn clear_then_a_line() {
        let mut m = Monitor::new(4e6);
        let mut s = 7u64;
        run(&mut m, 6, &mut s, |_, _| {});
        assert_eq!(m.report.status, InterferenceStatus::Clear);
        assert!(m.report.rise_db.abs() < 0.5);
        let f = 700e3 / 4e6;
        run(&mut m, 3, &mut s, |x, i0| {
            for (k, v) in x.iter_mut().enumerate() {
                *v += Complex32::from_polar(0.1, (TAU * f * (i0 + k as u64) as f64) as f32);
            }
        });
        assert_eq!(m.report.status, InterferenceStatus::Interference);
        assert!(m.report.kinds.narrowband);
        let l = &m.report.lines[0];
        assert!((l.offset_hz - 700e3).abs() < 8e3, "{l:?}");
        assert!(l.level_db > 20.0);
        assert!(!m.report.kinds.swept);
        assert!(m.history().count() > 0);
    }

    #[test]
    fn chirp_pulses_noise() {
        let fs = 4e6;
        let mut m = Monitor::new(fs);
        let mut s = 9u64;
        run(&mut m, 6, &mut s, |_, _| {});
        // 3.6 MHz in 100 us, repeating.
        run(&mut m, 2, &mut s, |x, i0| {
            for (k, v) in x.iter_mut().enumerate() {
                let t = ((i0 + k as u64) as f64 / fs) % 100e-6;
                let f0 = -1.8e6;
                let rate = 3.6e6 / 100e-6;
                let ph = TAU * (f0 * t + 0.5 * rate * t * t);
                *v += Complex32::from_polar(0.5, ph as f32);
            }
        });
        assert!(m.report.kinds.swept, "{:?}", m.report);
        assert!(
            (m.report.sweep_mhz_per_us - 0.036).abs() < 0.25 * 0.036,
            "{}",
            m.report.sweep_mhz_per_us
        );
        let mut m = Monitor::new(fs);
        run(&mut m, 6, &mut s, |_, _| {});
        run(&mut m, 2, &mut s, |x, i0| {
            for (k, v) in x.iter_mut().enumerate() {
                let n = i0 + k as u64;
                if n % 4000 < 40 {
                    *v += Complex32::new(1.0, 0.0);
                }
            }
        });
        assert!(m.report.kinds.pulsed, "{:?}", m.report.kinds);
        assert!((m.report.pulses_per_s - 1000.0).abs() < 150.0);
        let mut m = Monitor::new(fs);
        run(&mut m, 6, &mut s, |_, _| {});
        let mut s2 = 99u64;
        run(&mut m, 2, &mut s, |x, _| {
            let extra = noise(x.len(), &mut s2, 0.14 * 1.45);
            for (v, e) in x.iter_mut().zip(extra) {
                *v += e;
            }
        });
        assert_eq!(
            m.report.kinds,
            InterferenceKinds {
                wideband: true,
                ..Default::default()
            }
        );
        assert!((m.report.rise_db - 4.8).abs() < 1.0, "{}", m.report.rise_db);
        assert!(m.report.flatness > 0.9);
    }

    #[test]
    fn spoofing_indicators() {
        let mut m = Monitor::new(4e6);
        let p0 = [3_000_000.0, 4_000_000.0, 3_500_000.0];
        for k in 0..61 {
            m.second(
                &RxView {
                    cn0: vec![40.0, 44.0, 38.0],
                    fix: Some((k as f64, p0, 3.0, 0.0, 10.0)),
                    eph_alm: None,
                },
                1.7e9 + k as f64,
            );
            assert!(!m.report.spoofing, "{k}");
        }
        let p1 = [p0[0] + 2000.0, p0[1], p0[2]];
        m.second(
            &RxView {
                cn0: vec![40.0, 44.0, 38.0],
                fix: Some((61.0, p1, 3.0, 0.0, 10.0)),
                eph_alm: None,
            },
            1.7e9 + 61.0,
        );
        assert!(m.report.spoofing);
        let (d, _) = m.report.position_jump.unwrap();
        assert!((d - 2000.0).abs() < 20.0);
        assert!(m.report.far_from_average);
        assert!(!m.report.clock_jump && !m.report.equal_cn0);
        assert!(m.events().any(|(_, e)| e.text.contains("position jumped")));
        let mut m = Monitor::new(4e6);
        m.second(
            &RxView {
                cn0: vec![40.0, 40.2, 40.1, 39.9, 40.0],
                fix: None,
                eph_alm: None,
            },
            0.0,
        );
        assert!(m.report.equal_cn0 && m.report.spoofing);
    }
}
