//! The stack test harness: one stack's generator, the standard impairments, its receiver, and what came out.

use crate::Error;
use sdrkit_components::sim::{impaired, with_generator_settings};
use sdrkit_core::settings::SettingKind;
use sdrkit_core::verify::{check_against, json_at, CaseSetting, CheckLine, Tolerance, VectorRun};
use sdrkit_core::{
    Clock, Complex32, Event, Facilities, GenConfig, HostRequest, Hz, InputInfo, Output, Owner,
    Receiver, Results, Setting, StackConfig, StackDescriptor, Station, Value, Values,
};
use serde_json::Value as Json;
use std::path::PathBuf;

/// What a run returned.
#[derive(Debug, Default)]
pub struct Run {
    /// The receiver's results at the end.
    pub results: Results,
    /// The results after each 0.1 s of signal fed: (seconds fed, results).
    pub reports: Vec<(f64, Results)>,
    /// Every sound frame, interleaved.
    pub audio: Vec<f32>,
    /// The audio rate.
    pub audio_rate_hz: Hz,
    /// The samples per frame of `audio` (of its last part, if the count changed).
    pub audio_channels: u16,
    /// Every event.
    pub events: Vec<Event>,
    /// Every host request.
    pub requests: Vec<HostRequest>,
    /// Compressed media packets passed to the host.
    pub media_packets: usize,
    /// Seconds of signal fed.
    pub seconds: f64,
}

impl Run {
    /// The value of the result field `key`. `None` if the results do not declare `key`.
    pub fn field(&self, key: &str) -> Option<&Value> {
        let id = self.results.decl().field_id(key)?;
        self.results.fields().value(id)
    }

    /// The events of the kind `kind`.
    pub fn events_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Event> + 'a {
        self.events.iter().filter(move |e| e.kind() == kind)
    }

    /// This run as a [`VectorRun`] for a stack's own `check_vector` hook. `name` is the recording, `section` its part of
    /// `expected.json`, `expected` the whole file, `dir` the folder that holds them.
    pub fn vector<'a>(
        &'a self,
        name: &'a str,
        section: &'a Json,
        expected: &'a Json,
        dir: &'a std::path::Path,
    ) -> VectorRun<'a> {
        VectorRun {
            name,
            section,
            expected,
            results: &self.results,
            reports: &self.reports,
            audio: &self.audio,
            audio_rate_hz: self.audio_rate_hz,
            audio_channels: self.audio_channels,
            events: &self.events,
            seconds: self.seconds,
            dir,
        }
    }

    /// Compares the result fields named by `paths` with the values at those paths of the `section` of a vector in
    /// `expected.json`, each with its tolerance from the section or the file. The last name of a path is the key of the
    /// field. An expected value is read as the kind of its field. A path that matches no field is "not checked".
    pub fn check_keys(&self, section: &Json, expected: &Json, paths: &[&str]) -> Vec<CheckLine> {
        let decl = self.results.decl();
        paths
            .iter()
            .filter_map(|&p| {
                let exp = json_at(section, p)?;
                let key = p.rsplit('.').next().unwrap_or(p);
                let Some(id) = decl.field_id(key) else {
                    return Some(CheckLine::not_checked(p, "no result field of this key"));
                };
                let field = decl.fields().field(id)?;
                let tol = Tolerance::for_key(section, expected, p);
                let actual = self.results.fields().value(id).filter(|v| !v.is_none());
                Some(check_against(p, exp, field, actual, tol.as_ref()))
            })
            .collect()
    }
}

/// One stack under test. See the [crate documentation](crate).
#[derive(Clone)]
pub struct Harness {
    desc: &'static StackDescriptor,
    submode: Option<String>,
    settings: Vec<CaseSetting>,
    rate_hz: Hz,
    freq_hz: Hz,
    seconds: f64,
    piece: usize,
    seed: u64,
    station: Option<Station>,
    clock: Option<Clock>,
    live: bool,
}

impl std::fmt::Debug for Harness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Harness")
            .field("stack", &self.desc.id)
            .field("settings", &self.settings)
            .finish()
    }
}

impl Harness {
    /// The stack at its defaults: its own rate and frequency, 1 s of signal, pieces of 10 ms, seed 1.
    pub fn new(desc: &'static StackDescriptor) -> Harness {
        Harness {
            desc,
            submode: None,
            settings: Vec::new(),
            rate_hz: desc.tuning.rate_hz,
            freq_hz: desc.tuning.default_hz,
            seconds: 1.0,
            piece: 0,
            seed: 1,
            station: None,
            clock: None,
            live: false,
        }
    }

    /// With setting `s` of the stack or of the standard `gen_*` impairments at `v`. [`Harness::values`] makes sure that it
    /// fits.
    pub fn set<K: SettingKind>(mut self, s: &Setting<K>, v: K::In<'_>) -> Harness {
        self.settings.push(CaseSetting::new(s, v));
        self
    }
    /// With the settings of a test case or a recorded vector.
    pub fn with_settings(mut self, settings: &[CaseSetting]) -> Harness {
        self.settings.extend_from_slice(settings);
        self
    }
    /// For a submode.
    pub fn submode(mut self, id: &str) -> Harness {
        self.submode = Some(id.into());
        self
    }
    /// At this sample rate.
    pub fn rate(mut self, hz: Hz) -> Harness {
        self.rate_hz = hz;
        self
    }
    /// Tuned to (and centred on) this frequency.
    pub fn freq(mut self, hz: Hz) -> Harness {
        self.freq_hz = hz;
        self
    }
    /// This long a signal.
    pub fn seconds(mut self, s: f64) -> Harness {
        self.seconds = s;
        self
    }
    /// Fed in pieces of this many samples (0: 10 ms).
    pub fn piece(mut self, n: usize) -> Harness {
        self.piece = n;
        self
    }
    /// With this seed for the generator and the impairments.
    pub fn seed(mut self, seed: u64) -> Harness {
        self.seed = seed;
        self
    }
    /// With this station context.
    pub fn station(mut self, s: Station) -> Harness {
        self.station = Some(s);
        self
    }
    /// With this wall clock for the receiver (fixed at 0 without it).
    pub fn clock(mut self, clock: Clock) -> Harness {
        self.clock = Some(clock);
        self
    }
    /// As live samples (helper threads in use) instead of a recording.
    pub fn live(mut self) -> Harness {
        self.live = true;
        self
    }

    /// The settings: the stack's settings followed by the generator settings, at their defaults, with the values of
    /// [`Harness::set`]. `gen_seed` is the harness's seed unless a set supplies it.
    ///
    /// # Errors
    ///
    /// [`Error::Schema`] if a stack key is also a generator key. [`Error::Settings`] if a set value is refused or the
    /// settings have no such key.
    pub fn values(&self) -> Result<Values, Error> {
        let schema = with_generator_settings(&(self.desc.settings)()).map_err(Error::Schema)?;
        let mut values = Values::defaults(&schema);
        let seed = i64::try_from(self.seed).unwrap_or(i64::MAX);
        CaseSetting::apply_all(
            &[CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SEED, seed)],
            &mut values,
        )?;
        CaseSetting::apply_all(&self.settings, &mut values)?;
        Ok(values)
    }

    /// The receiver, made with the settings, given the station context.
    ///
    /// # Errors
    ///
    /// What [`Harness::values`] returns, and [`Error::Config`] if the stack refuses to make its receiver.
    pub fn receiver(&self) -> Result<Box<dyn Receiver>, Error> {
        let mut cfg = StackConfig::new(self.values()?);
        cfg.submode = self.submode.clone();
        let mut rx = (self.desc.make_receiver)(&cfg).map_err(Error::Config)?;
        if let Some(c) = &self.clock {
            let facilities = Facilities {
                clock: c.clone(),
                ..Facilities::default()
            };
            rx.set_facilities(&facilities);
        }
        if let Some(s) = &self.station {
            rx.set_station(s);
        }
        Ok(rx)
    }

    /// The test signal: the stack's generator through the standard impairments of the `gen_*` settings.
    ///
    /// # Errors
    ///
    /// [`Error::NoGenerator`] if the stack has no generator, what [`Harness::values`] returns, [`Error::Config`] if the stack
    /// refuses to make its generator, and [`Error::Impair`] if the impairment chain refuses the settings.
    pub fn generate(&self) -> Result<Vec<Complex32>, Error> {
        let make = self.desc.make_generator.ok_or(Error::NoGenerator {
            stack: self.desc.id,
        })?;
        let values = self.values()?;
        let gen = make(
            &GenConfig::new(self.rate_hz, values.clone())
                .with_submode_opt(self.submode.clone())
                .with_seed(self.seed),
        )
        .map_err(Error::Config)?;
        let mut g = impaired(gen, &values).map_err(Error::Impair)?;
        let n = (self.seconds * self.rate_hz).round() as usize;
        let mut x = vec![Complex32::default(); n];
        let got = g.generate(&mut x);
        x.truncate(got);
        Ok(x)
    }

    /// Generates, impairs and feeds the signal.
    ///
    /// # Errors
    ///
    /// What [`Harness::generate`] and [`Harness::feed`] return.
    pub fn run(&self) -> Result<Run, Error> {
        let x = self.generate()?;
        self.feed(&x)
    }

    /// Feeds `x` (at the harness's rate, centred on its frequency) to a new receiver and collects what comes out.
    ///
    /// # Errors
    ///
    /// What [`Harness::values`] returns, and [`Error::Config`] if the receiver refuses the input.
    pub fn feed(&self, x: &[Complex32]) -> Result<Run, Error> {
        self.feed_at(x, self.rate_hz, self.freq_hz)
    }

    /// Feeds the recording `<dir>/<name>` (found by [`crate::read_recording`], at its own rate and centre) to a new
    /// receiver. `Ok(None)` if the recording is not on this machine: the test is then skipped, and the reason printed.
    ///
    /// # Errors
    ///
    /// What [`Harness::feed`] returns.
    pub fn run_vector(&self, dir: &str, name: &str) -> Result<Option<Run>, Error> {
        let Some(r) = crate::read_recording(&format!("{dir}/{name}")) else {
            return Ok(None);
        };
        let centre = if r.centre_hz > 0.0 {
            r.centre_hz
        } else {
            self.freq_hz
        };
        self.feed_at(&r.samples, r.rate_hz, centre).map(Some)
    }

    /// The folder of a stack's vectors (`tests/vectors/<dir>`).
    pub fn vectors(dir: &str) -> PathBuf {
        crate::sigmf::vectors_dir().join(dir)
    }

    fn feed_at(&self, x: &[Complex32], rate: Hz, centre: Hz) -> Result<Run, Error> {
        let mut rx = self.receiver()?;
        let mut input = InputInfo::new(rate, centre, centre);
        if !self.live {
            input = input.recording();
        }
        rx.configure(&input).map_err(Error::Config)?;
        let piece = if self.piece > 0 {
            self.piece
        } else {
            ((rate / 100.0) as usize).max(1)
        };
        let report_every = ((rate / 10.0) as usize).max(1);
        let mut run = Run::default();
        let mut out = Output::default();
        let mut fed = 0usize;
        let mut next_report = report_every;
        let take = |run: &mut Run, out: &mut Output| {
            run.audio.extend_from_slice(out.audio());
            if let Some((rate, channels)) = out.audio_format() {
                run.audio_rate_hz = rate;
                run.audio_channels = channels;
            }
            run.events.append(&mut out.events);
            run.requests.append(&mut out.requests);
            run.media_packets += out.media.len();
        };
        for p in x.chunks(piece) {
            out.clear();
            rx.feed(p, &mut out);
            fed += p.len();
            take(&mut run, &mut out);
            if fed >= next_report {
                next_report += report_every;
                run.reports
                    .push((fed as f64 / rate, Owner::results(rx.as_ref()).clone()));
            }
        }
        // The end of the signal: the receiver decodes what it still holds and merges its results.
        out.clear();
        rx.finish(&mut out);
        take(&mut run, &mut out);
        run.seconds = fed as f64 / rate;
        run.results.clone_from(Owner::results(rx.as_ref()));
        Ok(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::chain;
    use sdrkit_core::publish::{field, keys, Published};
    use sdrkit_core::{
        ConfigError, EventDef, Generator, LockProfile, LockState, Schema, Setting, SettingError,
        SettingId, SettingSet, SignalInfo, Status, Tuning,
    };
    use std::sync::Arc;

    /// A carrier generator (unit power, 1 kHz wide) and a receiver that reports its level and offset from the carrier.
    struct Carrier;
    impl Generator for Carrier {
        fn rate_hz(&self) -> f64 {
            48_000.0
        }
        fn generate(&mut self, out: &mut [Complex32]) -> usize {
            out.fill(Complex32::new(1.0, 0.0));
            out.len()
        }
        fn signal_info(&self) -> Option<SignalInfo> {
            Some(SignalInfo::new(1_000.0))
        }
    }

    const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "", 1.0).range(0.0, 10.0, 0.0);
    const OTHER: Setting<field::F64> = Setting::new("other", "Other", "", 0.0);
    sdrkit_core::settings! {
        /// The settings of the meter.
        enum MeterSetting {
            /// Gain.
            Gain = GAIN,
        }
    }
    static PIECE: EventDef = EventDef::new("meter.piece", "Piece", &[]);

    struct Meter {
        published: Published,
        rate: f64,
        phase: Option<Complex32>,
        acc: Complex32,
        n: u64,
        pow: f64,
    }
    impl Owner for Meter {
        fn schema(&self) -> &Arc<Schema> {
            static S: std::sync::LazyLock<Arc<Schema>> =
                std::sync::LazyLock::new(MeterSetting::schema);
            &S
        }
        fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
            match MeterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
                MeterSetting::Gain => GAIN.read(id, v).map(|_| ()),
            }
        }
        fn results(&self) -> &Results {
            self.published.results()
        }
    }
    impl Receiver for Meter {
        fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
            self.rate = input.rate_hz;
            Ok(())
        }
        fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
            for &s in iq {
                if let Some(p) = self.phase {
                    self.acc += s * p.conj();
                }
                self.phase = Some(s);
                self.pow += s.norm_sqr() as f64;
                self.n += 1;
            }
            let p = &mut self.published;
            p.results_mut().set_status(&Status::new(LockState::Locked));
            p.set(
                &keys::LEVEL_DBFS,
                10.0 * (self.pow / self.n.max(1) as f64).log10(),
            );
            p.set(
                &keys::OFFSET_HZ,
                self.acc.arg() as f64 * self.rate / std::f64::consts::TAU,
            );
            out.events.push(PIECE.event(0.0, 0.0));
        }
    }
    static METER: StackDescriptor = StackDescriptor {
        id: "meter",
        name: "Meter",
        group: "Tools",
        blurb: "Shows the carrier's level and offset.",
        tuning: Tuning::anywhere(100e6, 48e3, 8e3, 1e3),
        submodes: &[],
        settings: MeterSetting::schema,
        make_receiver: |_| {
            Ok(Box::new(Meter {
                published: Published::new(&[&keys::LEVEL_DBFS, &keys::OFFSET_HZ]),
                rate: 0.0,
                phase: None,
                acc: Complex32::default(),
                n: 0,
                pow: 0.0,
            }))
        },
        make_generator: Some(|_| Ok(Box::new(Carrier))),
        lock_profile: LockProfile::DEFAULT,
        signal_types: &[],
        auto_gain: sdrkit_core::AutoGain::Radio,
        verify: None,
        benchmarks: None,
    };

    #[test]
    fn a_run_generates_impairs_feeds_and_collects() {
        let run = Harness::new(&METER)
            .set(&chain::GEN_OFFSET_HZ, 1_000.0)
            .set(&chain::GEN_LEVEL_DBFS, -6.0)
            .seconds(0.5)
            .run()
            .unwrap();
        assert_eq!(run.results.status().lock_state, LockState::Locked);
        let Some(&Value::Hz(off)) = run.field("offset_hz") else {
            panic!("{:?}", run.field("offset_hz"))
        };
        assert!((off - 1_000.0).abs() < 1.0, "{off}");
        let Some(&Value::F64(level)) = run.field("level_dbfs") else {
            panic!("{:?}", run.field("level_dbfs"))
        };
        assert!((level + 6.0).abs() < 0.1, "{level}");
        assert_eq!(run.events_of("meter.piece").count(), 50);
        assert_eq!(run.reports.len(), 5);
        assert!((run.seconds - 0.5).abs() < 1e-9);
        assert_eq!(run.reports[0].1.decl(), run.results.decl());
        // A bad impairment is refused, not replaced.
        assert!(matches!(
            Harness::new(&METER).set(&chain::GEN_LEVEL_DBFS, 99.0).run(),
            Err(Error::Settings(SettingError::OutOfRange { .. }))
        ));
        // A setting that the stack does not have is refused.
        assert!(matches!(
            Harness::new(&METER).set(&OTHER, 1.0).run(),
            Err(Error::Settings(SettingError::Unknown { .. }))
        ));
        // Expectations are compared with their tolerances.
        let section = serde_json::json!({ "level_dbfs": -6.0, "tolerance": { "level_dbfs": 0.5 } });
        let lines = run.check_keys(&section, &serde_json::json!({}), &["level_dbfs"]);
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].outcome,
            sdrkit_core::verify::Outcome::Pass,
            "{:?}",
            lines[0]
        );
        let lines = run.check_keys(
            &serde_json::json!({ "x": { "nope": 1 } }),
            &serde_json::json!({}),
            &["x.nope"],
        );
        assert_eq!(lines[0].outcome, sdrkit_core::verify::Outcome::NotChecked);
    }
}
