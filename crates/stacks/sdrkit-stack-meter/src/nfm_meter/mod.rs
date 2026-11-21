//! Narrowband FM deviation and CTCSS meter (`nfm_meter`): the peak deviation of the voice band, the CTCSS tone with its
//! nearest standard tone (TIA-603 lists the tones) and its deviation, the carrier offset, and the windows over the narrow and
//! wide channel limits (regional limits, EN 300 086 and national rules: the host sets them). It measures and decodes
//! nothing; DCS is not measured, and it is not a type-approval instrument. The windows and thresholds are confirmed by no
//! public document.

pub mod gen;

use crate::instrument::{Front, Log};
use crate::txtest::{band_gain, band_pass, index_percentile, tx_stack, Ctx, RealSpectrum, TxTest};
use crate::typed::{NfmMeterEventRow, NfmMeterFields, NfmMeterKind, NFM_METER_EVENTS};
use sdrkit_algo_dsp::carrierline::CarrierLine;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use sdrkit_util_ids::tones::ctcss;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "nfm_meter";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 24e3;
/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 24e3;
/// The stretch dropped at the start while the filter fills, s.
pub const SKIP_S: f64 = 0.2;
/// The audio samples of a window.
pub const WINDOW: usize = 1 << 15;
/// The voice band, Hz, and the width of its raised-cosine skirts.
pub const VOICE_BAND: (f64, f64, f64) = (300.0, 3000.0, 50.0);
/// The CTCSS search band, Hz.
pub const CTCSS_BAND: (f64, f64) = (60.0, 260.0);
/// The neighbourhood of the CTCSS tone, Hz.
pub const CTCSS_NEIGHBOURS: (f64, f64) = (40.0, 500.0);
/// The share of the channel power in a run of 5 bins that makes a carrier.
pub const CARRIER_SHARE: f64 = 0.012;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures the voice peak deviation (a percentile of the band-limited discriminator audio), the \
CTCSS tone, its nearest standard tone and its deviation, the carrier offset and the windows over the narrow and wide limits; \
decodes nothing, measures no DCS and is no type-approval instrument. The voice readings need a carrier line. The limits are \
regional settings. The windows and thresholds are confirmed by no public document.";

/// The narrow channel deviation limit.
pub const LIMIT_NARROW_HZ: Setting<field::F64> = Setting::new(
    "limit_narrow_hz",
    "Narrow channel deviation limit",
    "Hz",
    2500.0,
)
.range(1000.0, 10_000.0, 0.0)
.help("A regional limit for 12.5 kHz channels.");
/// The wide channel deviation limit.
pub const LIMIT_WIDE_HZ: Setting<field::F64> = Setting::new(
    "limit_wide_hz",
    "Wide channel deviation limit",
    "Hz",
    5000.0,
)
.range(1000.0, 10_000.0, 0.0)
.help("A regional limit for 25 kHz channels.");
/// The voice peak percentile.
pub const PEAK_PERCENTILE: Setting<field::F64> =
    Setting::new("peak_percentile", "Voice peak percentile", "%", 99.9).range(99.0, 100.0, 0.0);
/// The CTCSS tone over its neighbourhood.
pub const CTCSS_MIN_DB: Setting<field::F64> =
    Setting::new("ctcss_min_db", "CTCSS over its neighbourhood", "dB", 14.8).range(6.0, 30.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The narrow limit.
        LimitNarrowHz = LIMIT_NARROW_HZ,
        /// The wide limit.
        LimitWideHz = LIMIT_WIDE_HZ,
        /// The percentile.
        PeakPercentile = PEAK_PERCENTILE,
        /// The CTCSS test.
        CtcssMinDb = CTCSS_MIN_DB,
    }
}

sdrkit_core::settings! {
    /// The settings of the `nfm_meter` stack.
    pub enum NfmMeterSetting {
        /// The narrow limit.
        LimitNarrowHz = LIMIT_NARROW_HZ,
        /// The wide limit.
        LimitWideHz = LIMIT_WIDE_HZ,
        /// The percentile.
        PeakPercentile = PEAK_PERCENTILE,
        /// The CTCSS test.
        CtcssMinDb = CTCSS_MIN_DB,
        /// The voice scale of the test signal.
        GenVoiceHz = gen::GEN_VOICE_HZ,
        /// The CTCSS tone.
        GenCtcssHz = gen::GEN_CTCSS_HZ,
        /// Its deviation.
        GenCtcssDevHz = gen::GEN_CTCSS_DEV_HZ,
        /// Its end.
        GenCtcssOffS = gen::GEN_CTCSS_OFF_S,
    }
}

/// A CTCSS reading.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tone {
    hz: f64,
    standard: f64,
    dev: f64,
}

/// The measurement of `nfm_meter`.
#[derive(Default)]
pub struct NfmMeter {
    front: Option<Front>,
    disc: Option<Discriminator>,
    spec: Option<RealSpectrum>,
    line: Option<CarrierLine>,
    audio: Vec<f32>,
    x: Vec<f64>,
    rate: f64,
    narrow: f64,
    wide: f64,
    pct: f64,
    ctcss_ratio: f64,
    t: f64,
    windows: i64,
    over_narrow: i64,
    over_wide: i64,
    peak_max: Option<f64>,
    voice_peak: Option<f64>,
    offset: Option<f64>,
    tone: Option<Tone>,
    last_standard: Option<Option<f64>>,
    log: Log<NfmMeterEventRow>,
}

impl NfmMeter {
    fn ctcss(&self, spec: &RealSpectrum) -> Option<Tone> {
        let b = spec.bin_hz;
        let (a, z) = (
            (CTCSS_BAND.0 / b).ceil() as usize,
            (CTCSS_BAND.1 / b).floor() as usize,
        );
        let t = (a..=z).max_by(|&i, &j| spec.p[i].total_cmp(&spec.p[j]))?;
        let mut nb: Vec<f64> = ((CTCSS_NEIGHBOURS.0 / b).floor() as usize
            ..=(CTCSS_NEIGHBOURS.1 / b).floor() as usize)
            .step_by(3)
            .filter(|&k| k.abs_diff(t) > 12)
            .map(|k| spec.p[k])
            .collect();
        let m = nb.len() / 2;
        let (_, med, _) = nb.select_nth_unstable_by(m, f64::total_cmp);
        if spec.p[t] <= self.ctcss_ratio * *med {
            return None;
        }
        let hz = spec.refined_hz(t);
        let standard = f64::from(ctcss::nearest(hz as f32, f32::INFINITY)?.hz);
        Some(Tone {
            hz,
            standard,
            dev: (2.0 * spec.sum7(t)).sqrt(),
        })
    }

    fn row(&mut self, kind: NfmMeterKind) {
        let tone = self.tone;
        self.log.push(NfmMeterEventRow {
            time_s: Seconds(self.t),
            kind,
            ctcss_standard_hz: tone.map(|t| Hertz(t.standard)),
            ctcss_hz: tone.map(|t| Hertz(t.hz)),
            ctcss_deviation_hz: tone.map(|t| Hertz(t.dev)),
            voice_peak_khz: self.voice_peak.map(|v| v / 1000.0),
        });
    }

    fn window(&mut self, c: &mut Ctx) {
        let Some(mut spec) = self.spec.take() else {
            return;
        };
        let n = WINDOW;
        let mean = self.x[..n].iter().sum::<f64>() / n as f64;
        self.t += n as f64 / self.rate;
        self.windows += 1;
        spec.compute(&self.x[..n], mean);
        self.tone = self.ctcss(&spec);
        self.spec = Some(spec);
        let carrier = self.line.as_ref().is_some_and(|l| l.present(CARRIER_SHARE));
        self.offset = carrier.then_some(mean);
        self.voice_peak = carrier
            .then(|| {
                let centred: Vec<f64> = self.x[..n].iter().map(|v| v - mean).collect();
                let (lo, hi, sk) = VOICE_BAND;
                let mut v: Vec<f64> = band_pass(&centred, self.rate, |f| band_gain(f, lo, hi, sk))
                    .iter()
                    .map(|y| y.abs())
                    .collect();
                index_percentile(&mut v, self.pct / 100.0)
            })
            .flatten();
        let standard = self.tone.map(|t| (t.standard * 10.0).round() / 10.0);
        if self.last_standard.is_some_and(|s| s != standard)
            || (self.last_standard.is_none() && standard.is_some())
        {
            self.row(if standard.is_some() {
                NfmMeterKind::Tone
            } else {
                NfmMeterKind::ToneGone
            });
        }
        self.last_standard = Some(standard);
        if self.tone.is_some() || carrier {
            c.tally.good(self.t);
        } else {
            c.tally.energy(self.t);
        }
        if let Some(v) = self.voice_peak {
            self.peak_max = Some(self.peak_max.map_or(v, |m: f64| m.max(v)));
            if v > self.wide {
                self.over_wide += 1;
                self.over_narrow += 1;
                self.row(NfmMeterKind::OverWide);
            } else if v > self.narrow {
                self.over_narrow += 1;
                if self.windows % 2 == 0 {
                    self.row(NfmMeterKind::OverNarrow);
                }
            }
        }
    }
}

impl TxTest for NfmMeter {
    const ID: &'static str = ID;
    const NAME: &'static str = "nfm_meter.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        let d = (rate_hz / WORK_HZ).floor().max(1.0);
        let front = Front::new(rate_hz, WORK_HZ, 8000f64.min(0.45 * rate_hz / d), SKIP_S);
        self.rate = front.rate();
        self.disc = Discriminator::new(self.rate, 1.0).ok();
        self.spec = Some(RealSpectrum::new(WINDOW, self.rate));
        self.line = Some(CarrierLine::new(
            CarrierLine::block_len(self.rate, 0.4, 256, 32768),
            5,
        ));
        self.front = Some(front);
        self.narrow = v.get(&LIMIT_NARROW_HZ);
        self.wide = v.get(&LIMIT_WIDE_HZ);
        self.pct = v.get(&PEAK_PERCENTILE);
        self.ctcss_ratio = 10f64.powf(v.get(&CTCSS_MIN_DB) / 10.0);
        self.x.clear();
        self.t = self.t.max(SKIP_S);
        self.tone = None;
        self.voice_peak = None;
        self.offset = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        let (Some(front), Some(disc), Some(line)) =
            (self.front.as_mut(), self.disc.as_mut(), self.line.as_mut())
        else {
            return;
        };
        let z = front.process(x);
        line.push(z);
        self.audio.clear();
        disc.process(z, &mut self.audio);
        self.x.extend(self.audio.iter().map(|&a| f64::from(a)));
        while self.x.len() >= WINDOW {
            self.window(c);
            self.x.drain(..WINDOW);
        }
    }
    fn clear(&mut self) {
        self.windows = 0;
        self.over_narrow = 0;
        self.over_wide = 0;
        self.peak_max = None;
        self.voice_peak = None;
        self.tone = None;
        self.offset = None;
        self.last_standard = None;
        self.log.clear();
    }
    fn now(&self) -> f64 {
        self.t
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&NFM_METER_EVENTS, p);
        let t = self.tone;
        p.put(&NfmMeterFields {
            voice_peak_khz: self.voice_peak.map(|v| v / 1000.0),
            voice_peak_max_khz: self.peak_max.map(|v| v / 1000.0),
            over_narrow: self.over_narrow,
            over_wide: self.over_wide,
            windows: self.windows,
            ctcss_hz: t.map(|t| Hertz(t.hz)),
            ctcss_standard_hz: t.map(|t| Hertz(t.standard)),
            ctcss_error_hz: t.map(|t| Hertz(t.hz - t.standard)),
            ctcss_deviation_hz: t.map(|t| Hertz(t.dev)),
            ctcss_share_narrow_pct: t.map(|t| 100.0 * t.dev / self.narrow),
            ctcss_share_wide_pct: t.map(|t| 100.0 * t.dev / self.wide),
            carrier_offset_hz: self.offset.map(Hertz),
        });
        let voice = self.voice_peak.map_or("no carrier".to_string(), |v| {
            format!("voice peak {:.2} kHz", v / 1000.0)
        });
        match t {
            Some(t) => format!(
                "{voice}, CTCSS {:.2} Hz ({:.1}) at {:.0} Hz",
                t.hz, t.standard, t.dev
            ),
            None => format!("{voice}, no CTCSS"),
        }
    }
}

const KEYS: [&str; 4] = [
    "limit_narrow_hz",
    "limit_wide_hz",
    "peak_percentile",
    "ctcss_min_db",
];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("voice and a 100 Hz CTCSS tone", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = crate::instrument::number(r, NfmMeterFields::CTCSS_STANDARD_HZ.key);
        vec![CheckLine::new(
            NfmMeterFields::CTCSS_STANDARD_HZ.key,
            f == 100.0,
            format!("{f} Hz, 100 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: NfmMeter,
    settings: NfmMeterSetting,
    keys: KEYS,
    name: "Narrowband FM deviation and CTCSS meter",
    blurb: "Voice peak deviation, the CTCSS tone and its deviation, the carrier offset and the windows over the deviation limits.",
    tuning: (26e6, 6e9, 48e3, 16e3, true),
    rate_text: "The NFM meter needs at least 24 ksps",
    generator: |cfg| gen::NfmGenerator::new(cfg.rate_hz, &cfg.settings),
    verify: verify_cases
}
