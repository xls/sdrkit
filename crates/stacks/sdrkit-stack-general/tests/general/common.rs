//! Helpers of the tests of the general receiver: the settings of the stack, a receiver, a test signal through the impairments of
//! the framework, and runs that keep the audio and the state after each piece.

use sdrkit_algo_sim::chain::{ChainConfig, Impaired, SignalInfo as SimSignal};
use sdrkit_core::publish::keys;
use sdrkit_core::{
    Complex32, Generator, InputInfo, LockState, Output, Owner, Receiver, Results, SettingDecl,
    SettingSet, SignalInfo, StackConfig, Value, ValueKind, Values,
};
use sdrkit_stack_general::typed::{AmFields as Am_, GeneralFields as F};
use sdrkit_stack_general::{
    GenParams, GeneralGenerator, GeneralReceiver, GeneralSetting, CARRIER_0_3_DBFS,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The typed value of `text` for the setting `d`: a number, `0`/`1`/`true`/`false` for a flag, a choice by its key, or text.
pub fn typed(d: &SettingDecl, text: &str) -> Value {
    let key = &d.field.key;
    let num = || {
        text.parse::<f64>()
            .unwrap_or_else(|_| panic!("{key}: {text} is not a number"))
    };
    match d.field.kind {
        ValueKind::Bool => Value::Bool(matches!(text, "1" | "true")),
        ValueKind::F64 => Value::F64(num()),
        ValueKind::Hz => Value::Hz(num()),
        ValueKind::Duration => Value::Duration(num()),
        ValueKind::I64 => Value::I64(num() as i64),
        ValueKind::Choice => Value::Choice(
            d.field
                .choice_index(text)
                .unwrap_or_else(|| panic!("{key}: {text} is not a choice")),
        ),
        _ => Value::Text(text.into()),
    }
}

/// The settings of the stack and of the generator at their defaults, overlaid with `settings` as text (a value that the schema
/// refuses fails the test).
pub fn values(settings: &[(&str, &str)]) -> Values {
    let schema = sdrkit_components::sim::with_generator_settings(&GeneralSetting::schema())
        .expect("one schema");
    let mut v = Values::defaults(&schema);
    for (k, x) in settings {
        let id = schema.id(k).unwrap_or_else(|e| panic!("{e}"));
        let d = schema.setting(id).expect("a setting of the schema");
        v.set(id, &typed(d, x))
            .unwrap_or_else(|e| panic!("{k} = {x}: {e}"));
    }
    v
}

/// Applies one setting of the stack given as text, through the checked path.
pub fn set(
    rx: &mut GeneralReceiver,
    key: &str,
    text: &str,
) -> Result<(), sdrkit_core::SettingError> {
    let schema = Owner::schema(rx).clone();
    let id = schema.id(key)?;
    let v = typed(schema.setting(id).expect("a setting"), text);
    sdrkit_core::owner::apply(rx, id, &v)
}

/// The data folder of the repository.
pub fn data() -> sdrkit_presets::Data {
    sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
}

/// The presets of `general/presets.csv`.
pub fn presets() -> Vec<sdrkit_presets::general::GeneralPreset> {
    data().general_presets().unwrap()
}

/// The text of the setting `presets` with every preset of `general/presets.csv`.
pub fn presets_text() -> String {
    sdrkit_presets::general::presets_text(&presets())
}

/// A receiver at `rate`, the dial at `tuned_hz`, the capture centred `-signal_offset` away (the station is at `signal_offset`
/// in the samples), with `settings` over the defaults. Auto quality is off unless set.
pub fn receiver(rate: f64, signal_offset: f64, settings: &[(&str, &str)]) -> GeneralReceiver {
    let mut s: Vec<(&str, &str)> = vec![("am_auto", "0"), ("am_detector", "envelope")];
    s.extend_from_slice(settings);
    let mut rx = GeneralReceiver::new(&StackConfig::new(values(&s)))
        .expect("the receiver takes the settings");
    let tuned = 145.5e6;
    rx.configure(&InputInfo::new(rate, tuned - signal_offset, tuned).recording())
        .unwrap();
    rx
}

/// The clean generator with a switch for its station, so a test can make sequences such as noise, station, noise.
struct Switched {
    g: GeneralGenerator,
    on: Arc<AtomicBool>,
}

impl Generator for Switched {
    fn rate_hz(&self) -> f64 {
        self.g.rate_hz()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        self.g.signal_info()
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.g.set_station(self.on.load(Ordering::Relaxed));
        self.g.generate(out)
    }
}

/// A test signal: the clean generator of the stack through the impairment chain of the framework.
pub struct Source {
    g: Impaired<Switched>,
    on: Arc<AtomicBool>,
    rate: f64,
}

impl Source {
    /// Fills `out`.
    pub fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.g.generate(out)
    }
    /// The sample rate.
    pub fn rate_hz(&self) -> f64 {
        self.rate
    }
    /// Switches the station on or off (the noise runs on).
    pub fn set_station(&mut self, on: bool) {
        self.on.store(on, Ordering::Relaxed);
    }
}

/// A test signal at `rate` for `demod` with these generator settings. The carrier is 0.3 of full scale unless `gen_level_dbfs`
/// says otherwise, and a `gen_snr_db` (SNR in 12.5 kHz) turns the noise on.
pub fn generator(rate: f64, demod: &str, settings: &[(&str, &str)]) -> Source {
    let level = CARRIER_0_3_DBFS.to_string();
    let mut s = vec![("demod", demod), ("gen_level_dbfs", level.as_str())];
    if settings.iter().any(|(k, _)| *k == "gen_snr_db") {
        s.push(("gen_noise", "snr"));
    }
    s.push(("gen_seed", "42"));
    s.extend_from_slice(settings);
    let v = values(&s);
    let on = Arc::new(AtomicBool::new(true));
    let g = Switched {
        g: GeneralGenerator::new(GenParams::from_values(rate, &v)),
        on: on.clone(),
    };
    let info = g.signal_info().expect("the generator declares its signal");
    let sig = SimSignal::new(rate)
        .with_power(info.power)
        .with_bandwidth(info.bandwidth_hz);
    let cfg = ChainConfig::from_values(&v, &sig).expect("valid test signal settings");
    Source {
        g: Impaired::new(g, &cfg, sig).expect("the chain builds"),
        on,
        rate,
    }
}

/// What the AM processor reports, read from the results.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Am {
    /// The processor runs.
    pub active: bool,
    /// The loop is locked.
    pub locked: bool,
    /// Hz from the tuned frequency.
    pub carrier_offset_hz: f64,
    /// The status line.
    pub line: String,
}

/// The state of the receiver, read from its typed results after a piece.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Detail {
    /// 0 no input yet, 1 squelch closed, 2 squelch open.
    pub state: u8,
    /// The filter width (Hz).
    pub width_hz: f64,
    /// The demodulation rate (Hz).
    pub demod_rate_hz: f64,
    /// The channel relative to the dial (Hz).
    pub channel_offset_hz: f64,
    /// The listening frequency (Hz).
    pub listening_hz: f64,
    /// The passband relative to the channel (Hz).
    pub filter_lo_hz: f64,
    /// See `filter_lo_hz`.
    pub filter_hi_hz: f64,
    /// The low edge of the passband (Hz).
    pub filter_lo_abs_hz: f64,
    /// The level (dBFS, -200 before the first block).
    pub level_dbfs: f64,
    /// The noise (dBFS, -200 until measured).
    pub noise_dbfs: f64,
    /// The noise is measured.
    pub noise_valid: bool,
    /// The SNR (dB, 0 unless measured).
    pub snr_db: f64,
    /// The squelch threshold (dBFS, -200 for none).
    pub squelch_threshold_dbfs: f64,
    /// The squelch is open.
    pub squelch_open: bool,
    /// The FM offset (Hz, 0 unless FM and open).
    pub freq_offset_hz: f64,
    /// 10 ms blocks open.
    pub blocks_open: u64,
    /// 10 ms blocks closed.
    pub blocks_closed: u64,
    /// The CTCSS tone found (Hz).
    pub ctcss_hz: Option<f64>,
    /// The AM processor.
    pub am: Am,
    /// The summary line.
    pub summary: String,
}

impl Detail {
    /// The state in `r`.
    pub fn of(r: &Results) -> Detail {
        let f = r.fields();
        let level = f.get(&keys::LEVEL_DBFS);
        let noise = f.get(&keys::NOISE_DBFS);
        Detail {
            state: match r.status().lock_state {
                LockState::Locked => 2,
                LockState::Tracking => 1,
                _ => 0,
            },
            width_hz: f.at(&F::WIDTH_HZ).unwrap_or(0.0),
            demod_rate_hz: f.at(&F::DEMOD_RATE_HZ).unwrap_or(0.0),
            channel_offset_hz: f.at(&F::CHANNEL_OFFSET_HZ).unwrap_or(0.0),
            listening_hz: f.at(&F::LISTENING_HZ).unwrap_or(0.0),
            filter_lo_hz: f.at(&F::FILTER_LO_HZ).unwrap_or(0.0),
            filter_hi_hz: f.at(&F::FILTER_HI_HZ).unwrap_or(0.0),
            filter_lo_abs_hz: f.at(&F::PASSBAND_LO_HZ).unwrap_or(0.0),
            level_dbfs: level.unwrap_or(-200.0),
            noise_dbfs: noise.unwrap_or(-200.0),
            noise_valid: noise.is_some(),
            snr_db: f.get(&keys::SNR_DB).unwrap_or(0.0),
            squelch_threshold_dbfs: f.at(&F::SQUELCH_THRESHOLD_DBFS).unwrap_or(-200.0),
            squelch_open: f.at(&F::SQUELCH_OPEN).unwrap_or(false),
            freq_offset_hz: f.get(&keys::OFFSET_HZ).unwrap_or(0.0),
            blocks_open: f.at(&F::BLOCKS_OPEN).unwrap_or(0) as u64,
            blocks_closed: f.at(&F::BLOCKS_CLOSED).unwrap_or(0) as u64,
            ctcss_hz: f.at(&F::CTCSS_HZ),
            am: Am {
                active: f.at(&Am_::AM_ACTIVE).unwrap_or(false),
                locked: f.at(&Am_::AM_LOCKED).unwrap_or(false),
                carrier_offset_hz: f.at(&Am_::AM_CARRIER_OFFSET_HZ).unwrap_or(0.0),
                line: f.at(&Am_::AM_STATUS).unwrap_or("").to_owned(),
            },
            summary: r.status().summary.clone(),
        }
    }
}

/// The results of a run.
#[derive(Default)]
pub struct Run {
    /// Mono audio (the first channel) at 48 kHz.
    pub audio: Vec<f32>,
    /// The state after each piece.
    pub reports: Vec<Detail>,
    /// Events.
    pub events: Vec<sdrkit_core::Event>,
}

impl Run {
    /// The last `seconds` of audio.
    pub fn tail(&self, seconds: f64) -> &[f32] {
        let n = (seconds * 48_000.0) as usize;
        &self.audio[self.audio.len().saturating_sub(n)..]
    }
    /// The last state.
    pub fn last(&self) -> &Detail {
        self.reports.last().expect("a report")
    }
}

/// The first channel of the interleaved audio of `out`.
pub fn first_channel(out: &Output) -> impl Iterator<Item = f32> + '_ {
    out.audio_runs()
        .flat_map(|p| p.samples.iter().step_by(usize::from(p.channels.max(1))))
        .copied()
}

/// Feeds `seconds` from `source` in pieces of `block`, keeping the audio and the state after each piece.
pub fn run(
    rx: &mut GeneralReceiver,
    seconds: f64,
    rate: f64,
    block: usize,
    mut source: impl FnMut(&mut [Complex32]),
    r: &mut Run,
) {
    let total = (seconds * rate).round() as usize;
    let mut buf = vec![Complex32::default(); block];
    let mut out = Output::default();
    let mut done = 0;
    while done < total {
        let n = block.min(total - done);
        source(&mut buf[..n]);
        out.clear();
        rx.feed(&buf[..n], &mut out);
        r.audio.extend(first_channel(&out));
        r.events.append(&mut out.events);
        r.reports.push(Detail::of(Owner::results(rx)));
        done += n;
    }
}

/// Runs a test signal through a receiver.
pub fn run_gen(rx: &mut GeneralReceiver, g: &mut Source, seconds: f64, block: usize) -> Run {
    let rate = g.rate_hz();
    let mut r = Run::default();
    run(
        rx,
        seconds,
        rate,
        block,
        |b| {
            g.generate(b);
        },
        &mut r,
    );
    r
}

/// The peak amplitude of a tone (single-bin correlation).
pub fn tone(x: &[f32], f: f64) -> f64 {
    sdrkit_dev_test::measure::tone_amplitude(x, 48_000.0, f)
}

/// The RMS.
pub fn rms(x: &[f32]) -> f64 {
    sdrkit_dev_test::measure::rms(x)
}
