//! Fading analyser (`fading`): what the path does to one steady carrier within 2.8 kHz of the centre over the last minute:
//! the level percentiles against the mean power, the fade depth, the scintillation index S4 (ITU-R P.531), the Rician
//! K-factor by moments (ITU-R P.1057), the rate and the average length of fades under the threshold (Jakes 1974), the RMS
//! Doppler spread and the C/N0; deep or long fades are logged. It measures and decodes nothing. Fading faster than about
//! 200 Hz is cut off and noise adds its own fluctuation. The moment estimator of K and the rules are confirmed by no
//! public document.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{FadingEventKind, FadingEventRow, FadingFields, FADING_EVENTS};
use sdrkit_algo_dsp::carrierfind::{block_len, CarrierFinder, FinderConfig};
use sdrkit_algo_dsp::fadestats::{fade_stats, FadeStats};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "fading";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 8e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 16e3;
/// The pass edge of the channel, Hz.
pub const PASS_HZ: f64 = 3000.0;
/// The stretch dropped at the start, s.
pub const SKIP_S: f64 = 0.2;
/// The carrier search half width, Hz.
pub const SEARCH_HZ: f64 = 2800.0;
/// The noise band half width, Hz.
pub const BAND_HZ: f64 = 3000.0;
/// Bins near the peak left out of the noise.
pub const EXCLUDE_BINS: usize = 400;
/// The peak bin over the noise per bin of a present carrier.
pub const PEAK_RATIO: f64 = 20.0;
/// The line power over the noise of as many bins of a present carrier.
pub const LINE_RATIO: f64 = 3.0;
/// The half width of the line, Hz.
pub const LINE_HZ: f64 = 250.0;
/// The envelope rate is the working rate over `floor(rate / this)`, Hz.
pub const ENVELOPE_HZ: f64 = 1000.0;
/// The pass edge of the envelope filter, Hz.
pub const ENVELOPE_PASS_HZ: f64 = 220.0;
/// The envelope dropped at the start, s.
pub const ENVELOPE_SKIP_S: f64 = 3.0;
/// Seconds between evaluations.
pub const EVALUATE_EVERY_S: f64 = 5.0;
/// Reading rows after the first come at most this often, s.
pub const ROW_EVERY_S: f64 = 30.0;
/// A fade ends this far over the threshold, dB.
pub const FADE_HYSTERESIS_DB: f64 = 2.0;
/// The shares of the level percentiles.
pub const SHARES: [f64; 5] = [0.01, 0.1, 0.5, 0.9, 0.99];

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the fading of one steady carrier: level percentiles, fade depth, S4 (ITU-R P.531), \
the Rician K-factor by moments, fade rate and length, Doppler spread and C/N0; decodes nothing. Fading faster than about \
200 Hz is cut off; noise adds its own fluctuation. The K estimator and the rules are confirmed by no public document.";

/// The statistics window.
pub const WINDOW_S: Setting<field::F64> =
    Setting::new("window_s", "Statistics window", "s", 60.0).range(10.0, 600.0, 0.0);
/// The first reading.
pub const FIRST_S: Setting<field::F64> =
    Setting::new("first_s", "First reading after", "s", 30.0).range(5.0, 600.0, 0.0);
/// The fade threshold.
pub const FADE_DB: Setting<field::F64> =
    Setting::new("fade_db", "Fade threshold under the mean", "dB", 10.0).range(3.0, 30.0, 0.0);
/// The logged depth.
pub const LOG_DEPTH_DB: Setting<field::F64> =
    Setting::new("log_depth_db", "Fade logged deeper than", "dB", 20.0).range(3.0, 40.0, 0.0);
/// The logged length.
pub const LOG_LENGTH_S: Setting<field::F64> =
    Setting::new("log_length_s", "Fade logged longer than", "s", 0.5).range(0.05, 10.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The window.
        WindowS = WINDOW_S,
        /// The first reading.
        FirstS = FIRST_S,
        /// The fade threshold.
        FadeDb = FADE_DB,
        /// The logged depth.
        LogDepthDb = LOG_DEPTH_DB,
        /// The logged length.
        LogLengthS = LOG_LENGTH_S,
    }
}

sdrkit_core::settings! {
    /// The settings of the `fading` stack.
    pub enum FadingSetting {
        /// The window.
        WindowS = WINDOW_S,
        /// The first reading.
        FirstS = FIRST_S,
        /// The fade threshold.
        FadeDb = FADE_DB,
        /// The logged depth.
        LogDepthDb = LOG_DEPTH_DB,
        /// The logged length.
        LogLengthS = LOG_LENGTH_S,
        /// The largest Doppler of the test channel.
        GenFdHz = gen::GEN_FD_HZ,
        /// Its K-factor.
        GenK = gen::GEN_K,
        /// The spans without signal.
        GenOff = gen::GEN_OFF,
    }
}

/// The carrier being followed.
#[derive(Clone, Copy, Debug)]
struct Carrier {
    fc: f64,
    cn0: f64,
}

/// The measurement of `fading`.
#[derive(Default)]
pub struct Fading {
    front: Option<Front>,
    rate: f64,
    n: usize,
    finder: Option<CarrierFinder>,
    fft: Option<WindowedFft>,
    block: Vec<Complex32>,
    carrier: Option<Carrier>,
    nco: Option<Nco>,
    env_front: Option<Front>,
    env_rate: f64,
    env: VecDeque<Complex32>,
    mixed: Vec<Complex32>,
    window: f64,
    first: f64,
    u: f64,
    log_depth: f64,
    log_length: f64,
    t: f64,
    next_eval: f64,
    stats: Option<FadeStats>,
    last_row: Option<f64>,
    fade: Option<(f64, f64)>,
    log: Log<FadingEventRow>,
}

fn row(t: f64, kind: FadingEventKind) -> FadingEventRow {
    FadingEventRow {
        time_s: Seconds(t),
        kind,
        depth_db: None,
        length_ms: None,
        k_db: None,
        p10_db: None,
        fade_rate_hz: None,
        fade_length_ms: None,
        doppler_hz: None,
        cn0_dbhz: None,
    }
}

/// The K-factor in dB; none for K = 0.
fn k_db(k: f64) -> Option<f64> {
    (k > 0.0).then(|| 10.0 * k.log10())
}

impl Fading {
    fn lose(&mut self, c: &mut Ctx) {
        if self.carrier.take().is_some() {
            self.log.push(row(self.t, FadingEventKind::Lost));
        }
        self.env.clear();
        self.stats = None;
        self.fade = None;
        self.nco = None;
        c.tally.energy(self.t);
    }

    fn block(&mut self, c: &mut Ctx) {
        let (Some(finder), Some(fft)) = (self.finder.as_mut(), self.fft.as_mut()) else {
            return;
        };
        let Some(look) = finder.look(&self.block) else {
            return;
        };
        let n = self.n as i64;
        let b = self.rate / self.n as f64;
        let half = (LINE_HZ / b).floor() as i64;
        let p = fft.run_power(&self.block);
        let (mut q, mut m1) = (0.0, 0.0);
        for k in look.bin - half..=look.bin + half {
            let v = f64::from(p[k.rem_euclid(n) as usize]);
            q += v;
            m1 += v * k as f64 * b;
        }
        let noise = look.noise_per_bin;
        if !(look.present && q > LINE_RATIO * noise * (2 * half + 1) as f64) {
            self.lose(c);
            return;
        }
        let est = m1 / q.max(1e-30);
        let fc = match self.carrier {
            None => {
                self.log.push(row(self.t, FadingEventKind::Found));
                est
            }
            Some(k) => k.fc + 0.5 * (est - k.fc),
        };
        self.carrier = Some(Carrier {
            fc,
            cn0: look.cn0_dbhz,
        });
        if self.nco.is_none() {
            self.nco = Some(Nco::new(self.rate, -fc));
            self.env_front = Some(Front::new(
                self.rate,
                ENVELOPE_HZ,
                ENVELOPE_PASS_HZ,
                ENVELOPE_SKIP_S,
            ));
        }
        if let Some(o) = self.nco.as_mut() {
            o.set_freq(-fc);
        }
        c.tally.good(self.t);
    }

    fn envelope(&mut self, x: &[Complex32], c: &mut Ctx) {
        let (Some(nco), Some(front)) = (self.nco.as_mut(), self.env_front.as_mut()) else {
            return;
        };
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        nco.mix_in_place(&mut self.mixed);
        self.env_rate = front.rate();
        let keep = (self.window * self.env_rate) as usize;
        let mean = self.stats.as_ref().map(|s| s.mean);
        let thr = -10.0 * self.u.log10();
        let mut fades = Vec::new();
        for &e in front.process(&self.mixed) {
            self.env.push_back(e);
            if self.env.len() > keep {
                self.env.pop_front();
            }
            let Some(m) = mean else {
                continue;
            };
            let l = 10.0 * (f64::from(e.norm_sqr()) / m).max(1e-30).log10();
            match self.fade.as_mut() {
                None if l < -thr => self.fade = Some((0.0, l)),
                Some(f) if l > -thr + FADE_HYSTERESIS_DB => {
                    fades.push(*f);
                    self.fade = None;
                }
                Some(f) => {
                    f.0 += 1.0 / self.env_rate;
                    f.1 = f.1.min(l);
                }
                None => {}
            }
        }
        for (len, depth) in fades {
            if depth < -self.log_depth || len > self.log_length {
                let mut r = row(self.t, FadingEventKind::Fade);
                r.depth_db = Some(depth);
                r.length_ms = Some(1e3 * len);
                self.log.push(r);
            }
        }
        if self.t >= self.next_eval && self.env.len() as f64 >= self.first * self.env_rate {
            self.next_eval = self.t + EVALUATE_EVERY_S;
            let env: Vec<Complex32> = self.env.iter().copied().collect();
            self.stats = fade_stats(&env, self.env_rate, ENVELOPE_PASS_HZ, self.u, &SHARES);
            if let Some(s) = &self.stats {
                c.tally.good(self.t);
                if self.last_row.is_none_or(|t| self.t - t >= ROW_EVERY_S) {
                    self.last_row = Some(self.t);
                    let mut r = row(self.t, FadingEventKind::Reading);
                    r.k_db = k_db(s.k);
                    r.p10_db = Some(s.levels_db[1]);
                    r.fade_rate_hz = Some(s.crossing_rate_hz);
                    r.fade_length_ms = s.fade_length_s.map(|l| 1e3 * l);
                    r.doppler_hz = s.doppler_rms_hz;
                    r.cn0_dbhz = self.carrier.map(|k| k.cn0);
                    self.log.push(r);
                }
            }
        }
    }
}

impl TxTest for Fading {
    const ID: &'static str = ID;
    const NAME: &'static str = "fading.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let front = Front::new(rate_hz, WORK_HZ, PASS_HZ, SKIP_S);
        self.rate = front.rate();
        self.n = block_len(self.rate, 1.0);
        self.finder = CarrierFinder::new(
            self.n,
            self.rate,
            FinderConfig::new(SEARCH_HZ, BAND_HZ, EXCLUDE_BINS, PEAK_RATIO),
        )
        .ok();
        self.fft = WindowedFft::new(self.n, self.n, Window::Hann).ok();
        self.front = Some(front);
        self.block.clear();
        self.window = v.get(&WINDOW_S);
        self.first = v.get(&FIRST_S).min(self.window);
        self.u = 10f64.powf(-v.get(&FADE_DB) / 10.0);
        self.log_depth = v.get(&LOG_DEPTH_DB);
        self.log_length = v.get(&LOG_LENGTH_S);
        self.carrier = None;
        self.nco = None;
        self.env.clear();
        self.stats = None;
        self.fade = None;
        self.t = self.t.max(SKIP_S);
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let Some(mut front) = self.front.take() else {
            return;
        };
        let y = front.process(x).to_vec();
        self.front = Some(front);
        let mut rest = &y[..];
        while !rest.is_empty() {
            let take = (self.n - self.block.len()).min(rest.len());
            let (a, b) = rest.split_at(take);
            self.block.extend_from_slice(a);
            self.t += take as f64 / self.rate;
            if self.carrier.is_some() {
                self.envelope(a, c);
            }
            rest = b;
            if self.block.len() == self.n {
                self.block(c);
                self.block.clear();
            }
        }
    }
    fn clear(&mut self) {
        self.stats = None;
        self.last_row = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&FADING_EVENTS, p);
        let s = self.stats.as_ref();
        let lv = |k: usize| s.map(|s| s.levels_db[k]);
        p.put(&FadingFields {
            k_db: s.and_then(|s| k_db(s.k)),
            k: s.map(|s| s.k),
            rayleigh: s.map(|s| s.k == 0.0),
            p1_db: lv(0),
            p10_db: lv(1),
            p50_db: lv(2),
            p90_db: lv(3),
            p99_db: lv(4),
            fade_depth_db: s.map(|s| s.levels_db[2] - s.levels_db[0]),
            s4: s.map(|s| s.s4),
            fade_rate_hz: s.map(|s| s.crossing_rate_hz),
            fade_length_ms: s.and_then(|s| s.fade_length_s).map(|l| 1e3 * l),
            doppler_hz: s.and_then(|s| s.doppler_rms_hz),
            cn0_dbhz: self.carrier.map(|k| k.cn0),
            carrier_offset_hz: self.carrier.map(|k| Hertz(k.fc)),
            window_s: self
                .carrier
                .map(|_| Seconds(self.env.len() as f64 / self.env_rate.max(1.0))),
        });
        match (self.carrier, s) {
            (Some(k), Some(s)) => format!(
                "carrier at {:.1} Hz: S4 {:.2}, K {}, {:.2} fades/s",
                k.fc,
                s.s4,
                k_db(s.k).map_or("Rayleigh".to_string(), |d| format!("{d:.1} dB")),
                s.crossing_rate_hz
            ),
            (Some(k), None) => format!("carrier at {:.1} Hz; collecting the envelope", k.fc),
            _ => "no carrier".to_string(),
        }
    }
}

const KEYS: [&str; 5] = [
    "window_s",
    "first_s",
    "fade_db",
    "log_depth_db",
    "log_length_s",
];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("a carrier in Rayleigh fading at 5 Hz", o, 0.0);
    c.seconds = o.seconds.max(40.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let s4 = crate::instrument::number(r, FadingFields::S4.key);
        vec![CheckLine::new(
            FadingFields::S4.key,
            (s4 - 1.0).abs() <= 0.2,
            format!("{s4:.3}, 1 for Rayleigh fading"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: Fading,
    settings: FadingSetting,
    keys: KEYS,
    name: "Fading analyser",
    blurb: "Of one carrier: level percentiles, fade depth, S4, Rician K, fade rate and length, Doppler spread and C/N0.",
    tuning: (0.1e6, 6e9, 48e3, 6e3, true),
    rate_text: "The fading analyser needs at least 8 ksps",
    generator: |cfg| gen::FadingGenerator::new(cfg.rate_hz, &cfg.settings, cfg.seed),
    verify: verify_cases
}
