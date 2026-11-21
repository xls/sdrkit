//! FM SINAD meter (`sinad`): of an FM carrier modulated with a test tone, SINAD (signal, noise and distortion over noise and
//! distortion) in the audio band with flat weighting, THD of harmonics 2 to 5, THD and noise, the peak deviation, the carrier
//! offset and the crossings of the sensitivity mark (TIA-603 names 12 dB SINAD). It measures the transmitter, the path and this
//! radio together and decodes nothing. Below about 10 dB carrier to noise the FM threshold makes the reading fall faster than
//! theory. The windows and thresholds are confirmed by no public document.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{tx_stack, Ctx, RealSpectrum, TxTest};
use crate::typed::{
    SinadCrossing, SinadCrossingEvent, SinadEventRow, SinadFields, SINAD_CROSSING, SINAD_EVENTS,
};
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "sinad";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 24e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 24e3;
/// The pass edge of the channel filter, Hz.
pub const PASS_HZ: f64 = 8000.0;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.2;
/// The audio samples of a window.
pub const WINDOW: usize = 1 << 15;
/// The highest harmonic frequency measured, Hz.
pub const HARMONIC_TOP_HZ: f64 = 8000.0;
/// A reading row on every this many windows without a crossing.
pub const ROW_EVERY: i64 = 8;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures SINAD in the audio band with flat weighting (no psophometric or C-message filter, no \
de-emphasis), THD of harmonics 2 to 5, THD and noise, the peak deviation and the carrier offset of an FM carrier with a test \
tone; decodes nothing. It measures the transmitter, the path and this radio together. The windows and the tone test are \
confirmed by no public document.";

/// The low edge of the audio band.
pub const BAND_LO_HZ: Setting<field::F64> =
    Setting::new("band_lo_hz", "Audio band, low edge", "Hz", 300.0).range(50.0, 8000.0, 0.0);
/// The high edge of the audio band.
pub const BAND_HI_HZ: Setting<field::F64> =
    Setting::new("band_hi_hz", "Audio band, high edge", "Hz", 3000.0).range(50.0, 8000.0, 0.0);
/// The sensitivity mark.
pub const SINAD_MARK_DB: Setting<field::F64> =
    Setting::new("sinad_mark_db", "Sensitivity mark", "dB", 12.0)
        .range(6.0, 30.0, 0.0)
        .help("The SINAD of the sensitivity figure: 12 dB in TIA-603.");
/// The tone over the band median.
pub const TONE_MIN_DB: Setting<field::F64> =
    Setting::new("tone_min_db", "Tone over the band median", "dB", 20.0).range(10.0, 40.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The low edge.
        BandLoHz = BAND_LO_HZ,
        /// The high edge.
        BandHiHz = BAND_HI_HZ,
        /// The mark.
        SinadMarkDb = SINAD_MARK_DB,
        /// The tone test.
        ToneMinDb = TONE_MIN_DB,
    }
}

sdrkit_core::settings! {
    /// The settings of the `sinad` stack.
    pub enum SinadSetting {
        /// The low edge.
        BandLoHz = BAND_LO_HZ,
        /// The high edge.
        BandHiHz = BAND_HI_HZ,
        /// The mark.
        SinadMarkDb = SINAD_MARK_DB,
        /// The tone test.
        ToneMinDb = TONE_MIN_DB,
        /// The test tone of the test signal.
        GenToneHz = gen::GEN_TONE_HZ,
        /// Its deviation.
        GenDeviationHz = gen::GEN_DEVIATION_HZ,
        /// Its second harmonic.
        GenH2 = gen::GEN_H2,
        /// Its third harmonic.
        GenH3 = gen::GEN_H3,
        /// Later deviations.
        GenDeviationAfter = gen::GEN_DEVIATION_AFTER,
    }
}

/// One window's reading.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reading {
    sinad: f64,
    tone: f64,
    dev: f64,
    thd: f64,
    thdn: f64,
}

/// The measurement of `sinad`.
#[derive(Default)]
pub struct Sinad {
    front: Option<Front>,
    disc: Option<Discriminator>,
    spec: Option<RealSpectrum>,
    audio: Vec<f32>,
    x: Vec<f64>,
    lo: f64,
    hi: f64,
    mark: f64,
    tone_ratio: f64,
    t: f64,
    windows: i64,
    below: bool,
    offset: Option<f64>,
    last: Option<Reading>,
    log: Log<SinadEventRow>,
}

impl Sinad {
    fn window(&mut self, c: &mut Ctx) {
        let Some(spec) = self.spec.as_mut() else {
            return;
        };
        let n = WINDOW;
        let mean = self.x[..n].iter().sum::<f64>() / n as f64;
        spec.compute(&self.x[..n], mean);
        self.t += n as f64 / (spec.bin_hz * n as f64);
        self.windows += 1;
        self.offset = Some(mean);
        let b = spec.bin_hz;
        let j1 = (self.lo / b).ceil() as usize;
        let j2 = ((self.hi / b).floor() as usize).min(spec.p.len() - 1);
        if j2 <= j1 + 8 {
            self.last = None;
            return;
        }
        let mut band = spec.p[j1..j2].to_vec();
        let m = band.len() / 2;
        let (_, med, _) = band.select_nth_unstable_by(m, f64::total_cmp);
        let med = *med;
        let t = (j1..j2)
            .max_by(|&a, &b| spec.p[a].total_cmp(&spec.p[b]))
            .unwrap_or(j1);
        if spec.p[t] < self.tone_ratio * med {
            self.last = None;
            c.tally.energy(self.t);
            return;
        }
        let s = spec.sum7(t);
        let total: f64 = spec.p[j1..=j2].iter().sum();
        let tone = spec.refined_hz(t);
        let mut h = 0.0;
        for k in 2..=5 {
            let f = k as f64 * tone;
            let j = (f / b).round() as usize;
            if f > HARMONIC_TOP_HZ || j + 3 >= spec.p.len() {
                break;
            }
            h += spec.sum7(j);
        }
        let rest = (total - s).max(1e-30);
        let r = Reading {
            sinad: 10.0 * (total / rest).log10(),
            tone,
            dev: (2.0 * s).sqrt(),
            thd: 100.0 * (h / s).sqrt(),
            thdn: 100.0 * (rest / s).sqrt(),
        };
        self.last = Some(r);
        c.tally.good(self.t);
        let below = r.sinad < self.mark;
        let crossing = match (self.below, below) {
            (true, false) => SinadCrossing::Rose,
            (false, true) => SinadCrossing::Fell,
            _ => SinadCrossing::NoCrossing,
        };
        self.below = below;
        if crossing != SinadCrossing::NoCrossing || self.windows % ROW_EVERY == 0 {
            let row = SinadEventRow {
                time_s: Seconds(self.t),
                sinad_db: r.sinad,
                crossing,
                tone_hz: Hertz(r.tone),
                deviation_khz: r.dev / 1000.0,
                thd_pct: r.thd,
            };
            if crossing != SinadCrossing::NoCrossing {
                c.events.push(SINAD_CROSSING.event(
                    self.t,
                    c.dial(),
                    &SinadCrossingEvent {
                        time_s: row.time_s,
                        sinad_db: row.sinad_db,
                        crossing,
                        tone_hz: row.tone_hz,
                        deviation_khz: row.deviation_khz,
                        thd_pct: row.thd_pct,
                    },
                ));
            }
            self.log.push(row);
        }
    }
}

impl TxTest for Sinad {
    const ID: &'static str = ID;
    const NAME: &'static str = "sinad.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let front = Front::new(rate_hz, WORK_HZ, PASS_HZ, SKIP_S);
        let r = front.rate();
        self.disc = Discriminator::new(r, 1.0).ok();
        self.spec = Some(RealSpectrum::new(WINDOW, r));
        self.front = Some(front);
        self.lo = v.get(&BAND_LO_HZ);
        self.hi = v.get(&BAND_HI_HZ).max(self.lo);
        self.mark = v.get(&SINAD_MARK_DB);
        self.tone_ratio = 10f64.powf(v.get(&TONE_MIN_DB) / 10.0);
        self.x.clear();
        self.t = self.t.max(SKIP_S);
        self.last = None;
        self.offset = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let (Some(front), Some(disc)) = (self.front.as_mut(), self.disc.as_mut()) else {
            return;
        };
        self.audio.clear();
        disc.process(front.process(x), &mut self.audio);
        self.x.extend(self.audio.iter().map(|&a| f64::from(a)));
        while self.x.len() >= WINDOW {
            self.window(c);
            self.x.drain(..WINDOW);
        }
    }
    fn clear(&mut self) {
        self.windows = 0;
        self.below = true;
        self.last = None;
        self.offset = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&SINAD_EVENTS, p);
        let r = self.last;
        p.put(&SinadFields {
            sinad_db: r.map(|r| r.sinad),
            below_12db: r.map(|r| r.sinad < self.mark),
            tone_hz: r.map(|r| Hertz(r.tone)),
            deviation_khz: r.map(|r| r.dev / 1000.0),
            thd_pct: r.map(|r| r.thd),
            thdn_pct: r.map(|r| r.thdn),
            carrier_offset_hz: r.and(self.offset).map(Hertz),
            windows: self.windows,
        });
        match r {
            Some(r) => format!(
                "SINAD {:.1} dB, tone {:.1} Hz at {:.2} kHz, THD {:.2} %",
                r.sinad,
                r.tone,
                r.dev / 1000.0,
                r.thd
            ),
            None => "no test tone".to_string(),
        }
    }
}

const KEYS: [&str; 4] = ["band_lo_hz", "band_hi_hz", "sinad_mark_db", "tone_min_db"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("a 1 kHz tone at 3 kHz deviation", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let d = crate::instrument::number(r, SinadFields::DEVIATION_KHZ.key);
        vec![CheckLine::new(
            SinadFields::DEVIATION_KHZ.key,
            (d - 3.0).abs() < 0.1,
            format!("{d:.3} kHz, 3 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: Sinad,
    settings: SinadSetting,
    keys: KEYS,
    name: "FM SINAD meter",
    blurb: "SINAD, THD, THD and noise, peak deviation and carrier offset of an FM carrier with a test tone; the 12 dB crossing.",
    tuning: (26e6, 6e9, 48e3, 16e3, true),
    rate_text: "The SINAD meter needs at least 24 ksps",
    generator: |cfg| gen::SinadGenerator::new(cfg.rate_hz, &cfg.settings),
    verify: verify_cases
}
