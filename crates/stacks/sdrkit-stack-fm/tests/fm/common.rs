//! Helpers shared by the FM tests: a receiver, runs of the generator's signal through it, and the impairments of `sdrkit-algo-sim`.
#![allow(dead_code)]

use sdrkit_algo_sim::clock::ClockError;
use sdrkit_algo_sim::fading::{Doppler, Tap, Tdl};
use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{
    Complex32, Generator, InputInfo, Output, Receiver, SettingError, SettingId, SettingSet,
    StackConfig, Value, ValueKind, Values,
};
use sdrkit_graph::StackGraph;
use sdrkit_stack_fm::{graph, FmGenerator, FmSetting, GenParams, Reading};

/// The receiver the engine runs.
pub type Rx = StackGraph;

/// The stack setting `key` with the value written as text: a number, `1` or `true` for yes, or a choice key.
pub fn value(key: &str, text: &str) -> (SettingId, Value) {
    let schema = FmSetting::schema();
    let id = schema.id(key).unwrap_or_else(|e| panic!("{key}: {e}"));
    let field = &schema.setting(id).expect("declared").field;
    let num = || {
        text.parse::<f64>()
            .unwrap_or_else(|_| panic!("{key}: {text}"))
    };
    let v = match field.kind {
        ValueKind::Bool => Value::Bool(matches!(text, "1" | "true")),
        ValueKind::F64 => Value::F64(num()),
        ValueKind::Hz => Value::Hz(num()),
        ValueKind::I64 => Value::I64(num() as i64),
        ValueKind::Choice => Value::Choice(
            field
                .choice_index(text)
                .unwrap_or_else(|| panic!("{key}: no choice {text}")),
        ),
        _ => Value::Text(text.into()),
    };
    (id, v)
}

/// The stack's default settings with these changed (values written as [`value`] reads them).
pub fn values(settings: &[(&str, &str)]) -> Values {
    let mut values = Values::defaults(&FmSetting::schema());
    for (k, t) in settings {
        let (id, v) = value(k, t);
        values.set(id, &v).unwrap_or_else(|e| panic!("{k}: {e}"));
    }
    values
}

/// Sets stack setting `key` of `rx` through the checked path (the value written as [`value`] reads it).
pub fn set(rx: &mut Rx, key: &str, text: &str) -> Result<(), SettingError> {
    let id = rx.schema().id(key)?;
    let v = match FmSetting::schema().id(key) {
        Ok(_) => value(key, text).1,
        Err(_) => Value::Text(text.into()),
    };
    owner::apply(rx, id, &v)
}

/// A configured receiver at `rate`, tuned to `tuned_hz` (the station at the centre), with these settings.
pub fn receiver(rate: f64, tuned_hz: f64, settings: &[(&str, &str)]) -> Rx {
    let mut rx = graph::receiver(&StackConfig::new(values(settings))).expect("builds");
    rx.configure(&InputInfo {
        rate_hz: rate,
        centre_hz: tuned_hz,
        tuned_hz,
        live: true,
    })
    .unwrap();
    rx
}

/// The FM fields of the last report.
pub fn detail(rx: &Rx) -> Reading {
    Reading::from_results(Owner::results(rx))
}

/// The test station and the carrier-to-noise ratio it is received at (measured in 200 kHz; 100 dB or more is no noise).
#[derive(Clone, Debug)]
pub struct Sig {
    /// The generator's parameters.
    pub p: GenParams,
    /// Carrier to noise, dB in 200 kHz.
    pub cnr_db: f64,
    /// The noise seed.
    pub seed: u64,
}

/// The station `p` at `cnr_db`, noise seed 1.
pub fn sig(p: GenParams, cnr_db: f64) -> Sig {
    Sig { p, cnr_db, seed: 1 }
}

impl Sig {
    /// With another noise seed.
    pub fn seed(mut self, seed: u64) -> Sig {
        self.seed = seed;
        self
    }
    /// The generator with its noise (the generator makes the clean signal; the noise is `sdrkit-algo-sim`'s).
    pub fn source(&self) -> Noisy {
        let g = FmGenerator::new(self.p.clone());
        let power = self.p.gain * self.p.gain;
        let noise =
            (self.cnr_db < 100.0).then(|| awgn(power, self.cnr_db, self.p.rate_hz, self.seed));
        Noisy { g, noise }
    }
}

/// A generator with noise added.
pub struct Noisy {
    /// The generator.
    pub g: FmGenerator,
    noise: Option<Awgn>,
}

impl Noisy {
    /// Fills `x`.
    pub fn generate(&mut self, x: &mut [Complex32]) {
        self.g.generate(x);
        if let Some(n) = &mut self.noise {
            n.process(x);
        }
    }
}

/// Noise at `cnr_db` in 200 kHz against a carrier of mean power `power`, at `rate`.
pub fn awgn(power: f64, cnr_db: f64, rate: f64, seed: u64) -> Awgn {
    let level = NoiseLevel::SnrDb {
        snr_db: cnr_db,
        bandwidth_hz: 200e3,
    };
    Awgn::complex(level, power, rate, seed).expect("valid noise")
}

/// Noise of `sigma` on each of I and Q.
pub fn noise_sigma(sigma: f64, seed: u64) -> Awgn {
    Awgn::with_power(2.0 * sigma * sigma, seed).expect("valid noise")
}

/// The signal and one echo `delay` samples later, `level_db` below it (a steady two-ray channel).
pub fn echo(delay: usize, level_db: f64, rate: f64) -> Tdl {
    let taps = [
        Tap::new(0.0, 0.0, Doppler::Static),
        Tap::new(delay as f64 / rate, level_db, Doppler::Static),
    ];
    Tdl::new(&taps, rate, 1).expect("valid echo")
}

/// A carrier offset.
pub fn offset(rate: f64, hz: f64) -> FrequencyOffset {
    FrequencyOffset::new(rate, hz, 0.0).expect("valid offset")
}

/// A sample-clock error.
pub fn clock(ppm: f64) -> ClockError {
    ClockError::new(ppm).expect("valid clock error")
}

/// An ADC of `bits` bits that clips at `full_scale`.
pub fn quantise(x: &mut [Complex32], bits: u32, full_scale: f32) {
    let q = sdrkit_algo_sim::front::Quantiser::new(bits).expect("valid ADC");
    sdrkit_algo_sim::front::scale(x, 1.0 / full_scale);
    q.process(x);
    sdrkit_algo_sim::front::scale(x, full_scale);
}

/// The results of a run.
pub struct Run {
    /// Interleaved 48 kHz stereo.
    pub audio: Vec<f32>,
    /// The fields after every 0.1 s of signal.
    pub reports: Vec<Reading>,
    /// The last fields.
    pub last: Reading,
}

impl Run {
    /// Left and right of the last `seconds` of audio.
    pub fn tail(&self, seconds: f64) -> (Vec<f32>, Vec<f32>) {
        let frames = (seconds * 48_000.0) as usize;
        let n = self.audio.len() / 2;
        let from = n.saturating_sub(frames);
        sdrkit_dev_test::measure::deinterleave(&self.audio[2 * from..])
    }

    /// The first report time (s) at which `f` holds, if any.
    pub fn first(&self, f: impl Fn(&Reading) -> bool) -> Option<f64> {
        self.reports
            .iter()
            .position(f)
            .map(|i| (i + 1) as f64 * 0.1)
    }
}

/// Feeds `seconds` of signal from `source` (which fills a block) through `rx`, 0.1 s at a time, recording each report.
pub fn run_with(
    rx: &mut Rx,
    rate: f64,
    seconds: f64,
    mut source: impl FnMut(&mut Vec<Complex32>, usize),
) -> Run {
    let block = (rate / 10.0).round() as usize;
    let blocks = (seconds * 10.0).round() as usize;
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut reports = Vec::new();
    let mut buf = Vec::with_capacity(block);
    for _ in 0..blocks {
        buf.clear();
        source(&mut buf, block);
        out.clear();
        rx.feed(&buf, &mut out);
        stereo(&out, &mut audio);
        reports.push(detail(rx));
    }
    let last = detail(rx);
    Run {
        audio,
        reports,
        last,
    }
}

/// Appends the sound of `out` to `audio` as interleaved stereo: a mono frame is written as the same sample twice.
pub fn stereo(out: &Output, audio: &mut Vec<f32>) {
    for p in out.audio_runs() {
        if p.channels == 1 {
            audio.extend(p.samples.iter().flat_map(|&s| [s, s]));
        } else {
            audio.extend_from_slice(p.samples);
        }
    }
}

/// The station's signal, optionally changed by `impair`, through `rx`.
pub fn run(rx: &mut Rx, s: Sig, seconds: f64, mut impair: impl FnMut(&mut Vec<Complex32>)) -> Run {
    let rate = s.p.rate_hz;
    let mut g = s.source();
    let mut pending: Vec<Complex32> = Vec::new();
    run_with(rx, rate, seconds, move |buf, n| {
        while pending.len() < n {
            let mut x = vec![Complex32::default(); n];
            g.generate(&mut x);
            impair(&mut x);
            pending.extend_from_slice(&x);
        }
        buf.extend(pending.drain(..n));
    })
}

/// The RDS contents the default generator sends.
pub fn assert_rds_contents(d: &Reading) {
    assert_eq!(d.station, "SDRKIT  ", "PS");
    assert_eq!(d.text, "sdrkit FM test signal", "RT");
    assert!(d.text_complete, "RT complete");
    assert_eq!(d.pi, 0x4A21, "PI");
    assert_eq!(d.programme, "Pop Music", "PTY");
    assert!(d.traffic && !d.ta && d.music, "TP/TA/MS");
}

/// "RDS good" of the impairment tests: sync, at least 90 % usable blocks, enough groups, and the RDS contents.
pub fn assert_rds_good(d: &Reading, seconds: f64) {
    assert!(d.rds, "RDS sync");
    assert!(d.rds_ok >= 90.0, "block quality {}", d.rds_ok);
    let need = 0.9 * (seconds - 1.5) * 11.42;
    assert!(
        d.groups as f64 >= need,
        "{} groups, need {need:.0}",
        d.groups
    );
    assert_rds_contents(d);
}

/// Tone amplitude of a channel.
pub fn amp(x: &[f32], hz: f64) -> f64 {
    sdrkit_dev_test::measure::tone_amplitude(x, 48_000.0, hz)
}

/// The band settings that the repository's data folder has for the FM band `id`, as setting keys and texts.
pub fn band_settings(id: &str) -> Vec<(String, String)> {
    let data = sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
    let band = data.fm_band(id).unwrap_or_else(|| panic!("no FM band {id}"));
    band.settings()
        .into_iter()
        .map(|(k, v)| match v {
            Value::Hz(hz) => (k, hz.to_string()),
            other => panic!("{k}: {other:?}"),
        })
        .collect()
}

/// `settings` as the pairs that [`receiver`] takes, after `first`.
pub fn pairs<'a>(first: &[(&'a str, &'a str)], settings: &'a [(String, String)]) -> Vec<(&'a str, &'a str)> {
    first
        .iter()
        .copied()
        .chain(settings.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .collect()
}
