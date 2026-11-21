//! Beacon propagation monitor (`beacon_monitor`): one narrow beacon is followed and its path classed from three readings
//! per spectrum: the level of the line over the noise, the Doppler spread of the line (aurora smears a CW line over tens
//! to hundreds of Hz) and the swing of the level (sporadic-E and tropospheric fading). A change of class that holds for two
//! decisions is logged. It reads the signal and forecasts nothing; a keyed or drifting transmitter reads as something
//! else. The rules are confirmed by no public document.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{
    BeaconMonitorChangeEvent, BeaconMonitorEventRow, BeaconMonitorFields, BeaconPath,
    BEACON_MONITOR_CHANGE, BEACON_MONITOR_EVENTS,
};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::stats::centred_mean;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Choice, Complex32, Schema, Setting, SettingSet, Values};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "beacon_monitor";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 8e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 2400.0;
/// The pass edge of the channel, Hz.
pub const PASS_HZ: f64 = 1000.0;
/// The stretch dropped at the start, s.
pub const SKIP_S: f64 = 0.2;
/// The points of a spectrum.
pub const FFT_N: usize = 2048;
/// The line is searched again within this many bins.
pub const FOLLOW_BINS: i64 = 40;
/// The strongest bin of the spectrum takes over when this many times as strong.
pub const TAKE_OVER: f64 = 4.0;
/// The half width of the moving mean of the spread, bins.
pub const SMOOTH_HALF: usize = 4;
/// A smoothed bin over this many times the noise counts in the spread.
pub const SPREAD_RATIO: f64 = 2.2;
/// The spread stops after this many bins in a row under it.
pub const SPREAD_STOP: usize = 12;
/// The spread reaches at most this far from the line, Hz.
pub const SPREAD_HZ: f64 = 300.0;
/// A Gaussian full width at half height in standard deviations.
pub const FWHM: f64 = 2.355;
/// The swing of the level is taken over the present readings of this span, s: longer than the period of a slow fade
/// (about 8 s at 0.12 Hz), so a decision near a turning point of the fade still sees the whole swing.
pub const SWING_S: f64 = 12.0;
/// A line reads at most this far over the noise, dB: without noise the floor is the rounding of the spectrum, whose
/// scatter would read as a swing.
pub const LEVEL_TOP_DB: f64 = 100.0;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures one beacon: the level of its line over the noise, the Doppler spread of the line and \
the swing of its level, and classes the path steady, fading, aurora-like or none; forecasts nothing. A keyed or drifting \
transmitter reads as something else. The rules are confirmed by no public document.";

/// The level of a present line.
pub const PRESENT_DB: Setting<field::F64> =
    Setting::new("present_db", "Line over the noise", "dB", 10.0).range(3.0, 30.0, 0.0);
/// The aurora spread.
pub const AURORA_HZ: Setting<field::F64> =
    Setting::new("aurora_hz", "Aurora-like above", "Hz", 40.0).range(10.0, 200.0, 0.0);
/// The fading swing.
pub const FADING_DB: Setting<field::F64> =
    Setting::new("fading_db", "Fading swing above", "dB", 6.0).range(2.0, 20.0, 0.0);
/// The spectra of a decision.
pub const SPECTRA: Setting<field::I64> =
    Setting::new("spectra", "Spectra per decision", "", 5).range(3.0, 20.0, 1.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The presence level.
        PresentDb = PRESENT_DB,
        /// The aurora spread.
        AuroraHz = AURORA_HZ,
        /// The fading swing.
        FadingDb = FADING_DB,
        /// The spectra of a decision.
        Spectra = SPECTRA,
    }
}

sdrkit_core::settings! {
    /// The settings of the `beacon_monitor` stack.
    pub enum BeaconMonitorSetting {
        /// The presence level.
        PresentDb = PRESENT_DB,
        /// The aurora spread.
        AuroraHz = AURORA_HZ,
        /// The fading swing.
        FadingDb = FADING_DB,
        /// The spectra of a decision.
        Spectra = SPECTRA,
        /// The fading rate of the test signal.
        GenFadeHz = gen::GEN_FADE_HZ,
        /// Its fading depth.
        GenFadeDepthDb = gen::GEN_FADE_DEPTH_DB,
        /// Its aurora spread.
        GenSpreadHz = gen::GEN_SPREAD_HZ,
        /// The spans without beacon.
        GenOff = gen::GEN_OFF,
    }
}

/// The reading of one spectrum.
#[derive(Clone, Copy, Debug)]
struct Reading {
    level: f64,
    spread: Option<f64>,
    offset: f64,
}

/// The measurement of `beacon_monitor`.
#[derive(Default)]
pub struct BeaconMonitor {
    front: Option<Front>,
    rate: f64,
    fft: Option<WindowedFft>,
    floor: Option<BinMedian>,
    buf: Vec<Complex32>,
    signed: Vec<f64>,
    smooth: Vec<f64>,
    present_db: f64,
    aurora: f64,
    fading: f64,
    spectra: usize,
    line: Option<i64>,
    last: Option<Reading>,
    decision: Vec<Option<Reading>>,
    /// The times and levels of the present readings of the last [`SWING_S`].
    levels: VecDeque<(f64, f64)>,
    swing: Option<f64>,
    class: Option<BeaconPath>,
    pending: Option<BeaconPath>,
    changes: i64,
    t: f64,
    log: Log<BeaconMonitorEventRow>,
}

impl BeaconMonitor {
    fn spectrum(&mut self, c: &mut Ctx) {
        let (Some(fft), Some(floor)) = (self.fft.as_mut(), self.floor.as_mut()) else {
            return;
        };
        let p = fft.run_power(&self.buf);
        let n = FFT_N as i64;
        self.signed.clear();
        self.signed
            .extend((0..n).map(|i| f64::from(p[((i + n / 2) % n) as usize])));
        let noise = f64::from(floor.estimate(p.iter().copied()).unwrap_or(0.0)).max(1e-30);
        let s = &self.signed;
        let strongest = |lo: i64, hi: i64| {
            (lo.max(0)..=hi.min(n - 1)).max_by(|&a, &b| s[a as usize].total_cmp(&s[b as usize]))
        };
        let whole = strongest(0, n - 1).unwrap_or(n / 2);
        let line = match self.line {
            Some(l) => {
                let near = strongest(l - FOLLOW_BINS, l + FOLLOW_BINS).unwrap_or(l);
                if s[whole as usize] > TAKE_OVER * s[near as usize] {
                    whole
                } else {
                    near
                }
            }
            None => whole,
        };
        self.line = Some(line);
        let bin = self.rate / FFT_N as f64;
        let level = (10.0 * (s[line as usize] / noise).log10()).min(LEVEL_TOP_DB);
        let reading = (level >= self.present_db).then(|| {
            centred_mean(&self.signed, SMOOTH_HALF, &mut self.smooth);
            let reach = (SPREAD_HZ / bin).floor() as i64;
            let (mut w, mut m1, mut m2) = (0.0, 0.0, 0.0);
            let mut add = |k: i64, q: f64| {
                let d = (k - line) as f64;
                w += q - noise;
                m1 += (q - noise) * d;
                m2 += (q - noise) * d * d;
            };
            for dir in [-1i64, 1] {
                let mut quiet = 0;
                for j in usize::from(dir == 1)..=reach as usize {
                    let k = line + dir * j as i64;
                    if !(0..n).contains(&k) {
                        break;
                    }
                    let q = self.smooth[k as usize];
                    if q > SPREAD_RATIO * noise {
                        quiet = 0;
                        add(k, q);
                    } else {
                        quiet += 1;
                        if quiet >= SPREAD_STOP {
                            break;
                        }
                    }
                }
            }
            let spread = (w > 0.0).then(|| {
                let mean = m1 / w;
                FWHM * (m2 / w - mean * mean).max(0.0).sqrt() * bin
            });
            Reading {
                level,
                spread,
                offset: (line - n / 2) as f64 * bin,
            }
        });
        if let Some(r) = reading {
            c.tally.energy(self.t);
            self.levels.push_back((self.t, r.level));
        }
        while self
            .levels
            .front()
            .is_some_and(|&(t, _)| t < self.t - SWING_S)
        {
            self.levels.pop_front();
        }
        self.last = reading;
        self.decision.push(reading);
        if self.decision.len() >= self.spectra {
            self.decide(c);
        }
    }

    fn decide(&mut self, c: &mut Ctx) {
        let present: Vec<Reading> = self.decision.drain(..).flatten().collect();
        let mut spreads: Vec<f64> = present.iter().filter_map(|r| r.spread).collect();
        let (new, swing, spread) = if present.len() >= 3 {
            let hi = self.levels.iter().map(|l| l.1).fold(f64::MIN, f64::max);
            let lo = self.levels.iter().map(|l| l.1).fold(f64::MAX, f64::min);
            spreads.sort_by(f64::total_cmp);
            let med = spreads.get(spreads.len() / 2).copied().unwrap_or(0.0);
            let class = if med > self.aurora {
                BeaconPath::AuroraLike
            } else if hi - lo > self.fading {
                BeaconPath::Fading
            } else {
                BeaconPath::Steady
            };
            (class, Some(hi - lo), Some(med))
        } else {
            (BeaconPath::None, None, None)
        };
        self.swing = swing;
        let class = self.class.unwrap_or(BeaconPath::None);
        if new == class {
            self.pending = None;
        } else if self.pending == Some(new) {
            self.pending = None;
            let from = class;
            self.class = Some(new);
            self.changes += 1;
            let e = BeaconMonitorChangeEvent {
                time_s: Seconds(self.t),
                from_class: from,
                to_class: new,
                level_db: present.last().map(|r| r.level),
                spread_hz: spread,
                swing_db: swing,
            };
            c.events
                .push(BEACON_MONITOR_CHANGE.event(self.t, c.dial(), &e));
            self.log.push(BeaconMonitorEventRow {
                time_s: e.time_s,
                from_class: from,
                to_class: new,
                level_db: e.level_db,
                spread_hz: spread,
                swing_db: swing,
            });
            if new == BeaconPath::None {
                c.tally.bad(self.t);
            } else {
                c.tally.good(self.t);
            }
        } else {
            self.pending = Some(new);
        }
        if class != BeaconPath::None && new == class {
            c.tally.good(self.t);
        }
    }
}

impl TxTest for BeaconMonitor {
    const ID: &'static str = ID;
    const NAME: &'static str = "beacon_monitor.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let front = Front::new(rate_hz, WORK_HZ, PASS_HZ, SKIP_S);
        self.rate = front.rate();
        self.front = Some(front);
        self.fft = WindowedFft::new(FFT_N, FFT_N, Window::BlackmanHarris4)
            .ok()
            .map(|mut f| {
                Window::BlackmanHarris4.fill_symmetric(f.window_mut());
                f
            });
        self.floor = BinMedian::new(1).ok();
        self.buf.clear();
        self.present_db = v.get(&PRESENT_DB);
        self.aurora = v.get(&AURORA_HZ);
        self.fading = v.get(&FADING_DB);
        self.spectra = v.get(&SPECTRA).max(3) as usize;
        self.line = None;
        self.last = None;
        self.decision.clear();
        self.levels.clear();
        self.swing = None;
        self.class = None;
        self.pending = None;
        self.t = self.t.max(SKIP_S);
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let Some(mut front) = self.front.take() else {
            return;
        };
        let y = front.process(x).to_vec();
        self.front = Some(front);
        for z in y {
            self.buf.push(z);
            self.t += 1.0 / self.rate;
            if self.buf.len() == FFT_N {
                self.spectrum(c);
                self.buf.clear();
            }
        }
    }
    fn clear(&mut self) {
        self.changes = 0;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&BEACON_MONITOR_EVENTS, p);
        let r = self.last;
        p.put(&BeaconMonitorFields {
            class: self.class.unwrap_or(BeaconPath::None),
            level_db: r.map(|r| r.level),
            spread_hz: r.and_then(|r| r.spread),
            swing_db: self.swing,
            line_offset_hz: r.map(|r| Hertz(r.offset)),
            changes: self.changes,
        });
        match r {
            Some(r) => format!(
                "{}: line at {:.1} Hz, {:.1} dB over the noise",
                self.class.unwrap_or(BeaconPath::None).label(),
                r.offset,
                r.level
            ),
            None => format!(
                "{}: no line",
                self.class.unwrap_or(BeaconPath::None).label()
            ),
        }
    }
}

const KEYS: [&str; 4] = ["present_db", "aurora_hz", "fading_db", "spectra"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("a steady beacon", o, 0.0);
    c.seconds = o.seconds.max(30.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        use sdrkit_core::interface::ReadFields as _;
        let n = crate::instrument::number(r, BeaconMonitorFields::CHANGES.key);
        let class = BeaconMonitorFields::read(r.fields()).map(|f| f.class);
        vec![
            CheckLine::new(
                BeaconMonitorFields::CHANGES.key,
                n >= 1.0,
                format!("{n} class changes, at least 1 expected"),
            ),
            CheckLine::new(
                "class",
                class == Some(BeaconPath::Steady),
                format!("{class:?}, steady expected"),
            ),
        ]
    });
    vec![c]
}

tx_stack! {
    meter: BeaconMonitor,
    settings: BeaconMonitorSetting,
    keys: KEYS,
    name: "Beacon propagation monitor",
    blurb: "One beacon: its level over the noise, the Doppler spread of its line and its level swing; the path steady, fading, aurora-like or none.",
    tuning: (28e6, 450e6, 250e3, 4e3, true),
    rate_text: "The beacon monitor needs at least 8 ksps",
    generator: |cfg| gen::BeaconMonitorGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
