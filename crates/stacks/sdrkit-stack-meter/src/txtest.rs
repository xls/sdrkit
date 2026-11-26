//! The frame of the transmitter test stacks: one decoder component that holds the settings, the reset command, the tally of
//! results, the input level and the events, around a [`TxTest`] that measures. It also holds the parts that several of these
//! stacks use: the one-window spectrum of a real signal, the frequency-domain band-pass with raised-cosine skirts, the
//! percentile of a list by index and the FM modulator of a test signal.

use crate::instrument::{InputLevel, Tally};
use crate::typed::WatchFields;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Complex32, ConfigError, Event, Record, Schema, SettingError, SettingId, StackConfig, Status,
    Value, Values,
};
use sdrkit_graph::{
    Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Kept, Kind,
    Outbox, Port, RateFrom, StackGraph,
};
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

/// Rows kept in a table of a transmitter test.
pub const ROWS_KEPT: usize = 500;

/// What a measurement shares with its frame.
#[derive(Debug, Default)]
pub struct Ctx {
    /// The good, bad and energy readings.
    pub tally: Tally,
    /// The events of this call.
    pub events: Vec<Event>,
    /// The tuned frequency; `None` when the input does not state it.
    pub tuned_hz: Option<f64>,
}

impl Ctx {
    /// The tuned frequency for an event, 0 when unknown.
    pub fn dial(&self) -> f64 {
        self.tuned_hz.unwrap_or(0.0)
    }
}

/// A transmitter test: the measurement inside a [`Frame`].
pub trait TxTest: Default + Send + 'static {
    /// The stack id.
    const ID: &'static str;
    /// The registry name of the component.
    const NAME: &'static str;
    /// The least input rate, Hz.
    const MIN_RATE_HZ: f64;
    /// What the stack measures, for its status.
    const NOTE: &'static str;
    /// The settings of the component.
    fn schema() -> Arc<Schema>;
    /// Starts at the working rate of input `rate_hz` with the settings `v`: drops every tracker, average, floor and
    /// measurement in progress, keeps the tables and the signal clock. Runs at the start, after a change of a setting, a
    /// reset, a retune and a clear.
    fn start(&mut self, rate_hz: f64, v: &Values);
    /// Takes input samples.
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx);
    /// Empties the tables, the histories and the counters.
    fn clear(&mut self);
    /// The signal time, s.
    fn now(&self) -> f64;
    /// Writes the result fields and the tables; returns the headline.
    fn publish(&mut self, p: &mut Published, c: &Ctx) -> String;
    /// The host changed the frequency: empties what `kept` does not hold. The default empties everything.
    fn forget(&mut self, kept: &Kept) {
        let _ = kept;
        self.clear();
    }
}

/// The `reset` command: the trackers, averages and floors start again; the tables stay.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Drops the trackers, averages and floors, as after a retune; the tables stay.");
static COMMANDS: [&Command; 1] = [&RESET];

/// The decoder of a transmitter test.
pub struct Frame<T: TxTest> {
    inner: T,
    values: Values,
    rate: f64,
    ctx: Ctx,
    level: InputLevel,
    status: Status,
    started: bool,
}

impl<T: TxTest> Default for Frame<T> {
    fn default() -> Self {
        Frame {
            inner: T::default(),
            values: Values::defaults(&T::schema()),
            rate: 0.0,
            ctx: Ctx::default(),
            level: InputLevel::default(),
            status: Status::default(),
            started: false,
        }
    }
}

impl<T: TxTest> Decoder for Frame<T> {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        T::NAME
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        T::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.values.set(id, value)?;
        if self.rate > 0.0 {
            self.inner.start(self.rate, &self.values);
        }
        Ok(())
    }
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            if self.rate > 0.0 {
                self.inner.start(self.rate, &self.values);
            }
            Ok(None)
        } else {
            Err(CommandError::NoSuchId { id })
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(crate::typed::shape(T::ID).results_decl()))
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        if !input.rate.is_finite() || input.rate < T::MIN_RATE_HZ * 0.999 {
            return Err(ConfigError::RateTooLow {
                need: T::MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.ctx.tuned_hz = (input.tuned_hz > 0.0).then_some(input.tuned_hz);
        self.started = false;
        self.inner = T::default();
        self.clear();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.started = true;
        self.level.push(x);
        self.inner.feed(x, &mut self.ctx);
        if !self.ctx.events.is_empty() {
            out.messages(0).append(&mut self.ctx.events);
        }
    }
    fn report(&mut self, p: &mut Published) {
        let headline = if self.started {
            self.inner.publish(p, &self.ctx)
        } else {
            let _ = self.inner.publish(p, &self.ctx);
            "waiting for samples".to_string()
        };
        let t = self.inner.now();
        let level = self.level.read();
        let st = &mut self.status;
        st.lock_state = self.ctx.tally.lock(t);
        st.level_dbfs = level.map(|l| l as f32);
        st.offset_hz = None;
        st.snr_db = None;
        st.summary = headline;
        p.put(&WatchFields {
            state: self.ctx.tally.state(t),
            good: i64::try_from(self.ctx.tally.good).unwrap_or(i64::MAX),
            bad: i64::try_from(self.ctx.tally.bad).unwrap_or(i64::MAX),
            level_dbfs: level,
            note: T::NOTE,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        0.25
    }
    fn retune(&mut self, input: &Format) {
        self.ctx.tuned_hz = (input.tuned_hz > 0.0).then_some(input.tuned_hz);
        if self.rate > 0.0 {
            self.inner.start(self.rate, &self.values);
        }
    }
    fn clear(&mut self) {
        self.inner.clear();
        if self.rate > 0.0 {
            self.inner.start(self.rate, &self.values);
        }
        self.ctx.tally = Tally::default();
        self.ctx.events.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        self.inner.forget(kept);
        if self.rate > 0.0 {
            self.inner.start(self.rate, &self.values);
        }
        self.ctx.tally = Tally::default();
        self.ctx.events.clear();
    }
}

/// The receiver of a transmitter test: one decoder `meter` on the input, the settings `keys` routed to it.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver<T: TxTest>(
    cfg: &StackConfig,
    keys: &[&str],
    rate_text: &str,
) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("meter", DecoderComponent::new(Frame::<T>::default()))
        .input("meter")
        .output("meter.events")
        .status_from("meter")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut g =
        StackGraph::of(crate::typed::shape(T::ID), graph).min_rate(T::MIN_RATE_HZ, rate_text);
    for key in keys {
        g = g.route(key, &[&format!("meter.{key}")])?;
    }
    g.command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("meter", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the component of `T` to a registry.
pub fn register<T: TxTest>(r: &mut sdrkit_graph::Registry) {
    r.add(T::NAME, || {
        Box::new(DecoderComponent::new(Frame::<T>::default()))
    });
}

/// The value at index `floor(p x n)` (clamped to the last) of `v` sorted; `None` for an empty list.
pub fn index_percentile(v: &mut [f64], p: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let k = ((p * v.len() as f64).floor() as usize).min(v.len() - 1);
    let (_, x, _) = v.select_nth_unstable_by(k, f64::total_cmp);
    Some(*x)
}

/// The power spectrum of one window of a real signal: four-term Blackman-Harris over `n` samples, the mean removed first,
/// bins `0 .. n / 2` scaled so that they sum to the mean square (`p_k = |X_k|^2 x 2 / (n sum w^2)`).
#[derive(Debug)]
pub struct RealSpectrum {
    fft: Fft,
    window: Vec<f32>,
    norm: f64,
    buf: Vec<Complex32>,
    /// The bins from 0 Hz.
    pub p: Vec<f64>,
    /// The width of a bin, Hz.
    pub bin_hz: f64,
}

impl RealSpectrum {
    /// A spectrum of `n` samples at `rate_hz`.
    pub fn new(n: usize, rate_hz: f64) -> RealSpectrum {
        let window = Window::BlackmanHarris4.coefficients(n);
        let w2: f64 = window.iter().map(|&w| f64::from(w) * f64::from(w)).sum();
        RealSpectrum {
            fft: Fft::forward(n),
            window,
            norm: 2.0 / (n as f64 * w2),
            buf: vec![Complex32::default(); n],
            p: vec![0.0; n / 2],
            bin_hz: rate_hz / n as f64,
        }
    }

    /// The length of a window.
    pub fn len(&self) -> usize {
        self.window.len()
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Computes the bins of `x` (its first `n` values) after removing `mean`.
    pub fn compute(&mut self, x: &[f64], mean: f64) {
        for ((b, &v), &w) in self.buf.iter_mut().zip(x).zip(&self.window) {
            *b = Complex32::new(((v - mean) * f64::from(w)) as f32, 0.0);
        }
        self.fft.process(&mut self.buf);
        for (p, b) in self.p.iter_mut().zip(&self.buf) {
            *p = f64::from(b.norm_sqr()) * self.norm;
        }
    }

    /// The sum of the 7 bins `k - 3 ..= k + 3` (cut at the ends).
    pub fn sum7(&self, k: usize) -> f64 {
        let last = self.p.len() - 1;
        self.p[k.saturating_sub(3).min(last)..=(k + 3).min(last)]
            .iter()
            .sum()
    }

    /// The refined frequency of the peak bin `k` by the log parabola, Hz.
    pub fn refined_hz(&self, k: usize) -> f64 {
        if k == 0 || k + 1 >= self.p.len() {
            return k as f64 * self.bin_hz;
        }
        let f = sdrkit_algo_dsp::peak::log_parabolic(self.p[k - 1], self.p[k], self.p[k + 1]);
        (k as f64 + f.offset) * self.bin_hz
    }
}

/// The gain of a band-pass from `lo` to `hi` Hz with raised-cosine skirts `skirt` Hz wide outside the band, at `|f|`.
pub fn band_gain(f: f64, lo: f64, hi: f64, skirt: f64) -> f64 {
    let a = f.abs();
    if a >= lo && a <= hi {
        1.0
    } else if a >= lo - skirt && a < lo {
        0.5 - 0.5 * (PI * (a - (lo - skirt)) / skirt).cos()
    } else if a > hi && a <= hi + skirt {
        0.5 + 0.5 * (PI * (a - hi) / skirt).cos()
    } else {
        0.0
    }
}

/// The frequency-domain band-pass of a real stretch `x` at `rate_hz` (no window function): the FFT of `x`, the gain `gain(|f|)`,
/// back to time; the real part.
pub fn band_pass(x: &[f64], rate_hz: f64, gain: impl Fn(f64) -> f64) -> Vec<f64> {
    let n = x.len();
    let mut buf: Vec<Complex32> = x.iter().map(|&v| Complex32::new(v as f32, 0.0)).collect();
    Fft::forward(n).process(&mut buf);
    for (k, b) in buf.iter_mut().enumerate() {
        let j = if k <= n / 2 {
            k as f64
        } else {
            k as f64 - n as f64
        };
        *b *= gain(j * rate_hz / n as f64) as f32;
    }
    Fft::inverse(n).process(&mut buf);
    buf.iter().map(|b| f64::from(b.re) / n as f64).collect()
}

/// The analytic signal of a real stretch `x` at `rate_hz` through the gain `gain(f)` (no window function): the FFT of `x`,
/// twice the bins of positive frequency `f` times the gain, zero elsewhere, back to time. With a band gain its real part is
/// [`band_pass`] of the same gain; its magnitude and phase are the envelope and the phase of the band.
pub fn analytic(x: &[f64], rate_hz: f64, gain: impl Fn(f64) -> f64) -> Vec<Complex32> {
    let n = x.len();
    let mut buf: Vec<Complex32> = x.iter().map(|&v| Complex32::new(v as f32, 0.0)).collect();
    Fft::forward(n).process(&mut buf);
    for (k, b) in buf.iter_mut().enumerate() {
        *b *= if k < n / 2 {
            (2.0 * gain(k as f64 * rate_hz / n as f64)) as f32
        } else {
            0.0
        };
    }
    Fft::inverse(n).process(&mut buf);
    let s = 1.0 / n as f32;
    buf.iter().map(|b| b * s).collect()
}

/// An FM modulator of a test signal: a carrier of amplitude 1 whose instantaneous frequency is given per sample in Hz.
#[derive(Clone, Debug, Default)]
pub struct FmTones {
    phase: f64,
}

impl FmTones {
    /// The next sample at instantaneous frequency `f_hz` at `rate_hz`, of amplitude `a`.
    pub fn next(&mut self, f_hz: f64, rate_hz: f64, a: f64) -> Complex32 {
        self.phase = (self.phase + TAU * f_hz / rate_hz).rem_euclid(TAU);
        Complex32::from_polar(a as f32, self.phase as f32)
    }
}

/// Writes the receiver, the registration, the generator hook, the verify hooks and the descriptor of a transmitter test
/// stack whose module holds `ID` and `MIN_RATE_HZ`.
macro_rules! tx_stack {
    (meter: $m:ty, settings: $s:ty, keys: $keys:expr, name: $name:expr, blurb: $blurb:expr,
     tuning: ($min:expr, $max:expr, $rate:expr, $width:expr, $centre:expr), rate_text: $rt:expr,
     generator: |$cfg:ident| $gen:expr, verify: $verify:path) => {
        /// The receiver: the measuring component on the input.
        ///
        /// # Errors
        ///
        /// [`sdrkit_graph::GraphError`] if a setting of `cfg` is refused.
        pub fn receiver(
            cfg: &sdrkit_core::StackConfig,
        ) -> Result<sdrkit_graph::StackGraph, sdrkit_graph::GraphError> {
            $crate::txtest::receiver::<$m>(cfg, &$keys, $rt)
        }

        /// Adds the stack's components to a registry.
        pub fn register(r: &mut sdrkit_graph::Registry) {
            $crate::txtest::register::<$m>(r);
        }

        fn make_receiver(
            cfg: &sdrkit_core::StackConfig,
        ) -> Result<Box<dyn sdrkit_core::Receiver>, sdrkit_core::ConfigError> {
            Ok(Box::new(receiver(cfg)?))
        }

        fn make_generator(
            $cfg: &sdrkit_core::GenConfig,
        ) -> Result<Box<dyn sdrkit_core::Generator>, sdrkit_core::ConfigError> {
            Ok(Box::new($gen))
        }

        /// The engine's checks of the test signal.
        pub static VERIFY: sdrkit_core::verify::VerifyHooks = sdrkit_core::verify::VerifyHooks {
            cases: $verify,
            vector_settings: None,
            check_vector: None,
        };

        /// The descriptor of the stack.
        pub static DESCRIPTOR: sdrkit_core::StackDescriptor = sdrkit_core::StackDescriptor {
            id: ID,
            name: $name,
            group: "Measurement",
            blurb: $blurb,
            tuning: sdrkit_core::Tuning {
                kind: Some(sdrkit_core::TuningKind::Free),
                min_hz: $min,
                max_hz: $max,
                carrier_at_centre: $centre,
                ..sdrkit_core::Tuning::anywhere(0.0, $rate, MIN_RATE_HZ, $width)
            },
            submodes: &[],
            settings: <$s as sdrkit_core::SettingSet>::schema,
            make_receiver,
            make_generator: Some(make_generator),
            lock_profile: sdrkit_core::LockProfile::DEFAULT,
            signal_types: &[],
            auto_gain: sdrkit_core::AutoGain::Hold,
            verify: Some(&VERIFY),
            benchmarks: Some($crate::bench::benchmarks),
        };
    };
}
pub(crate) use tx_stack;
