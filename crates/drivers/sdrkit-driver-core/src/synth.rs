//! The test-signal driver: plays a [`Generator`], paced like a file, with a gain model and 8-bit quantisation that give the
//! signal the grain and clipping of a low-cost radio.

use crate::paced::{PacedWriter, Pacing, BENCHMARK, BLOCK, PACE};
use crate::{
    Capabilities, Doing, DriverDescriptor, DriverError, Link, Retuned, RingWriter, Source,
    SourceDriver, Started, StreamFormat, CENTRE, RATE,
};
use sdrkit_algo_sim::chain::GenSetting;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, Generator, Hz, Setting, SettingDecl, SettingId, SettingSet, Station, Value, Values,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

/// The rate of the signal if the settings ask for none. A host that knows the stack of the signal gives the rate of the
/// stack in the settings, so this is the rate of silence and of a signal whose host states no rate.
pub const DEFAULT_RATE_HZ: Hz = 2e6;

/// The test signals that the test-signal driver plays, by stack id. The id is the address without the `synth:` prefix:
/// `<stack>` or `<stack>/<submode>`.
pub trait GeneratorFactory: Send + Sync {
    /// The settings of the test signal of `stack`, which an open radio adds after those of the driver. A setting whose key
    /// the driver has already is left out. Returns `None` if `stack` has no test signal.
    fn settings(&self, stack: &str) -> Option<Vec<SettingDecl>>;
    /// Makes the test signal of `stack` at `rate` Hz with the driver's `values`, which hold [`GenSetting`] and the settings
    /// of [`GeneratorFactory::settings`], and says what it was made from. Returns `None` if it cannot be made: the driver
    /// then plays silence.
    fn make(&self, stack: &str, values: &Values, rate: Hz) -> Option<Made>;
    /// What [`GeneratorFactory::make`] would make the test signal of `stack` from with `values` at `rate`. The driver
    /// makes its signal again only when these differ from those of the signal it plays ([`SignalInputs::makes_same`]),
    /// so a change of another setting leaves the running signal untouched. The default is the rate and the values of
    /// [`GenSetting`] and of [`GeneratorFactory::settings`].
    fn inputs(&self, stack: &str, values: &Values, rate: Hz) -> SignalInputs {
        let own = self.settings(stack).unwrap_or_default();
        SignalInputs {
            rate_hz: rate,
            values: values_where(values, |key| {
                GenSetting::SETTINGS.iter().any(|d| d.key() == key)
                    || own.iter().any(|d| d.field.key == key)
            }),
            station: None,
            tuned_hz: None,
        }
    }
}

/// A test signal and what it was made from.
pub struct Made {
    /// The signal.
    pub gen: Box<dyn Generator>,
    /// What it was made from.
    pub inputs: SignalInputs,
}

/// What a test signal is made from: equal inputs make the same signal.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignalInputs {
    /// The rate, Hz.
    pub rate_hz: Hz,
    /// The values that the generator reads, by key.
    pub values: Vec<(String, Value)>,
    /// The station that the generator is made for, if it reads one.
    pub station: Option<Station>,
    /// The frequency where the session listens, Hz: in the inputs of a made signal, if the generator read it
    /// ([`sdrkit_core::GenConfig::tuned_hz`]); in the inputs of a wanted signal, if a generator could read it.
    pub tuned_hz: Option<Hz>,
}

impl SignalInputs {
    /// True if the signal made from `self` is the signal that `wanted` asks for: the rate, the values and the station are
    /// equal, and so is the frequency where the session listens if the made signal read it.
    pub fn makes_same(&self, wanted: &SignalInputs) -> bool {
        self.rate_hz == wanted.rate_hz
            && self.values == wanted.values
            && self.station == wanted.station
            && (self.tuned_hz.is_none() || self.tuned_hz == wanted.tuned_hz)
    }
}

/// The values of `values` whose key `keep` accepts, by key, in the order of the schema.
pub fn values_where(values: &Values, keep: impl Fn(&str) -> bool) -> Vec<(String, Value)> {
    let decls = values.schema().settings();
    decls
        .iter()
        .enumerate()
        .filter(|(_, d)| keep(&d.field.key))
        .filter_map(|(i, d)| {
            let v = values.value(SettingId(u16::try_from(i).ok()?))?;
            Some((d.field.key.to_string(), v.clone()))
        })
        .collect()
}

/// A function of the stack id, the driver's values and the rate is a factory whose test signals have no settings of
/// their own.
impl<F> GeneratorFactory for F
where
    F: Fn(&str, &Values, Hz) -> Option<Box<dyn Generator>> + Send + Sync,
{
    fn settings(&self, _stack: &str) -> Option<Vec<SettingDecl>> {
        Some(Vec::new())
    }
    fn make(&self, stack: &str, values: &Values, rate: Hz) -> Option<Made> {
        Some(Made {
            gen: self(stack, values, rate)?,
            inputs: self.inputs(stack, values, rate),
        })
    }
}

/// Round I and Q to 8 bits.
pub const QUANTIZE: Setting<field::Bool> = Setting::new("quantize", "8-bit quantisation", "", true)
    .help("Rounds I and Q to 8 bits like the converter of a low-cost radio, with noise of one step and its clipping 12 dB above the nominal level.");
/// Scale the level by the gains below.
pub const GAIN_MODEL: Setting<field::Bool> = Setting::new("gainmodel", "Gain model", "", false)
    .help("Scales the level by the gains below (62 dB in all is the nominal level), to test gain control.");
/// The gain of the low-noise amplifier of the model.
pub const LNA: Setting<field::F64> =
    Setting::new("lna", "LNA gain", "dB", 24.0).range(0.0, 40.0, 8.0);
/// The gain of the variable amplifier of the model.
pub const VGA: Setting<field::F64> =
    Setting::new("vga", "VGA gain", "dB", 24.0).range(0.0, 62.0, 2.0);
/// The amplifier of the model, 14 dB.
pub const AMP: Setting<field::Bool> =
    Setting::new("amp", "Amplifier", "", true).help("Adds 14 dB.");

sdrkit_core::settings! {
    /// The own settings of the test-signal driver. The generator settings ([`GenSetting`]) follow them.
    pub enum SynthSetting {
        /// [`PACE`].
        Pace = PACE,
        /// [`BENCHMARK`].
        Benchmark = BENCHMARK,
        /// [`QUANTIZE`].
        Quantize = QUANTIZE,
        /// [`GAIN_MODEL`].
        GainModel = GAIN_MODEL,
        /// [`LNA`].
        Lna = LNA,
        /// [`VGA`].
        Vga = VGA,
        /// [`AMP`].
        Amp = AMP,
    }
}

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "synth",
    name: "Test signal",
    opens: crate::Opens::Many,
    addresses: &[
        "synth:<stack> (the test signal of the stack)",
        "synth:<stack>/<submode>",
        "synth (silence)",
    ],
    address: &[
        crate::AddressPart::text("stack", "Stack", "", crate::Syntax::Name)
            .required()
            .rows("engine", "stacks")
            .help("The stack whose test signal plays."),
        crate::AddressPart::text("submode", "Submode", "/", crate::Syntax::Name)
            .help("The submode of the stack. Empty for the stack's default."),
    ],
    settings: &[SynthSetting::SETTINGS, GenSetting::SETTINGS],
    commands: &[],
    readings: &[],
    produces: &[StreamFormat {
        kind: Kind::Iq,
        sample: SampleType::Cf32,
        min_rate_hz: 1.0,
        max_rate_hz: 1e9,
        channels: 1,
    }],
    accepts: &[],
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: &[],
    kind_tables: &[],
};

/// The test-signal driver. The address is `synth:<stack>`, `<stack>`, or empty for silence; `<stack>` can name a
/// submode as `<stack>/<submode>`. It plays what its factory makes for the stack.
#[derive(Clone)]
pub struct SynthDriver {
    factory: Arc<dyn GeneratorFactory>,
}

impl SynthDriver {
    /// A driver that makes its generator with `factory` at every open and at every change of what
    /// [`GeneratorFactory::inputs`] reports.
    pub fn new(factory: Arc<dyn GeneratorFactory>) -> Self {
        SynthDriver { factory }
    }
}

/// The scale of the gain model: 10^((G - 62) / 20) with G = LNA + VGA + 14 if the amplifier is on.
pub fn gain_model_scale(lna_db: f64, vga_db: f64, amp: bool) -> f32 {
    let g = lna_db + vga_db + if amp { 14.0 } else { 0.0 };
    10f64.powf((g - 62.0) / 20.0) as f32
}

/// The full scale of the 8-bit converter of the model, as a factor of the nominal level 1.0: 12 dB of headroom, like a
/// radio whose gain is set for a signal with high peaks. A signal with carriers (DVB-T, DAB) at the nominal level has
/// peaks of about 11 dB over its mean; a converter without headroom clips a third of its samples.
pub const FULL_SCALE: f32 = 4.0;

/// Quantises one value like an 8-bit converter with the full scale [`FULL_SCALE`]: 128 steps to the full scale, clipped
/// there. The level of the value stays.
pub fn quantize8(v: f32) -> f32 {
    (v / STEP).clamp(-128.0, 127.0).round() * STEP
}

/// The step of [`quantize8`].
pub const STEP: f32 = FULL_SCALE / 128.0;

/// The noise of the converter of the model: one value, uniform over one step, for each call. A converter without it
/// turns a keyed carrier into keyed copies over the whole band, which no radio shows, because the noise of a real
/// converter is larger than its step.
struct ConverterNoise(u64);

impl ConverterNoise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * STEP
    }
}

#[derive(Clone, Copy, Debug)]
struct Options {
    pacing: Pacing,
    quantize: bool,
    scale: Option<f32>,
}

impl Options {
    fn from(v: &Values) -> Options {
        let pacing = Pacing::of(v);
        let scale = v
            .get(&GAIN_MODEL)
            .then(|| gain_model_scale(v.get(&LNA), v.get(&VGA), v.get(&AMP)));
        Options {
            pacing,
            quantize: v.get(&QUANTIZE),
            scale,
        }
    }
}

fn rate_of(v: &Values) -> Hz {
    match v.get(&RATE) {
        r if r > 0.0 => r,
        _ => DEFAULT_RATE_HZ,
    }
}

/// The stack id of a test-signal address: empty for silence.
fn stack_of(address: &str) -> &str {
    address.strip_prefix("synth:").unwrap_or(address)
}

/// True if a radio of the kind has a setting with the key `key`.
fn kind_has(key: &str) -> bool {
    crate::RadioSetting::SETTINGS
        .iter()
        .chain(DESCRIPTOR.settings.iter().flat_map(|g| g.iter()))
        .any(|d| d.key() == key)
}

impl SourceDriver for SynthDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }
    /// Opens the test signal of the stack that `address` names. The first signal is made from `values`; with the settings
    /// of [`SourceDriver::settings_at`] in them, it is the signal that they ask for.
    ///
    /// # Errors
    ///
    /// [`DriverError::Address`] if the factory has no test signal for the stack.
    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let stack = stack_of(address);
        let settings = self.settings_at(address)?;
        let caps = Capabilities {
            name: "Test signal".into(),
            min_freq_hz: 0.0,
            max_freq_hz: 1e12,
            ..Default::default()
        };
        let inputs = inputs_of(&self.factory, stack, values);
        Ok(Box::new(SynthSource {
            caps,
            factory: self.factory.clone(),
            stack: stack.into(),
            settings,
            wanted: Arc::new(Wanted {
                now: Mutex::new(Want {
                    values: values.clone(),
                    inputs,
                    version: 0,
                    taken: 0,
                    ended: false,
                }),
                taken: Condvar::new(),
                cut: AtomicBool::new(false),
            }),
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }))
    }

    /// The settings of the test signal of the stack ([`GeneratorFactory::settings`]) whose key the kind does not have.
    ///
    /// # Errors
    ///
    /// [`DriverError::Address`] if the factory has no test signal for the stack.
    fn settings_at(&self, address: &str) -> Result<Vec<SettingDecl>, DriverError> {
        let all = match stack_of(address) {
            "" => Vec::new(),
            s => self.factory.settings(s).ok_or(DriverError::Address)?,
        };
        Ok(all.into_iter().filter(|d| !kind_has(&d.field.key)).collect())
    }
}

/// An open test signal.
pub struct SynthSource {
    caps: Capabilities,
    factory: Arc<dyn GeneratorFactory>,
    /// The stack id that the address named.
    stack: String,
    /// The settings of the stack's test signal.
    settings: Vec<SettingDecl>,
    /// The settings that the signal plays.
    wanted: Arc<Wanted>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// The settings that a test signal plays, shared by the source and its thread.
struct Wanted {
    now: Mutex<Want>,
    /// Signalled when the thread takes a version and when it ends.
    taken: Condvar,
    /// Set with a version that makes the signal again, and by a stop: the thread drops the block that it is writing.
    cut: AtomicBool,
}

/// The settings of a test signal and how far its thread has taken them.
struct Want {
    values: Values,
    /// The inputs of the signal that plays, once the thread has made it for `values`; until then, those that `values`
    /// ask for.
    inputs: SignalInputs,
    /// Moves at every change of `values`.
    version: u64,
    /// The last version that the thread took. From then on it writes no sample made before that version.
    taken: u64,
    /// The thread has ended.
    ended: bool,
}

/// Marks the thread of a test signal as ended when it returns or panics, so that no apply waits for it.
struct Ended<'a>(&'a Wanted);

impl Drop for Ended<'_> {
    fn drop(&mut self) {
        lock(&self.0.now).ended = true;
        self.0.taken.notify_all();
    }
}

/// The inputs of the signal of `stack` with `values`; for silence, the rate alone.
fn inputs_of(factory: &Arc<dyn GeneratorFactory>, stack: &str, values: &Values) -> SignalInputs {
    let rate = rate_of(values);
    match stack {
        "" => SignalInputs {
            rate_hz: rate,
            ..SignalInputs::default()
        },
        s => factory.inputs(s, values, rate),
    }
}

impl SynthSource {
    fn started(&self) -> Started {
        let w = lock(&self.wanted.now);
        Started {
            rate_hz: rate_of(&w.values),
            centre_hz: w.values.get(&CENTRE),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Source for SynthSource {
    fn capabilities(&self) -> &Capabilities {
        &self.caps
    }

    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        if self.thread.is_some() {
            return Err(DriverError::NotNow {
                state: Doing::Running,
            });
        }
        self.stop.store(false, Ordering::Release);
        self.wanted.cut.store(false, Ordering::Release);
        lock(&self.wanted.now).ended = false;
        let (wanted, stop, factory, stack) = (
            self.wanted.clone(),
            self.stop.clone(),
            self.factory.clone(),
            self.stack.clone(),
        );
        let t = std::thread::Builder::new()
            .name("sdrkit-synth".into())
            .spawn(move || play(sink, &wanted, &stop, &factory, &stack))
            .map_err(|_| DriverError::Thread)?;
        self.thread = Some(t);
        Ok(self.started())
    }

    /// Takes `values`. A change of the inputs of the signal ([`GeneratorFactory::inputs`]) makes it again: the thread of
    /// a running source drops the block that it is writing, and the call returns once the thread has taken the change.
    /// After the call the source writes no sample of the old signal, at any rate and pace; the samples that the ring
    /// held before the call stay there. Another change leaves the running signal as it is and returns at once.
    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        let inputs = inputs_of(&self.factory, &self.stack, values);
        let mut w = lock(&self.wanted.now);
        let remake = !w.inputs.makes_same(&inputs);
        w.values = values.clone();
        if remake {
            w.inputs = inputs;
        }
        w.version += 1;
        if remake && self.thread.is_some() {
            self.wanted.cut.store(true, Ordering::Release);
            let version = w.version;
            w = self
                .wanted
                .taken
                .wait_while(w, |w| w.taken < version && !w.ended)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        drop(w);
        Ok(Retuned::Live(self.started()))
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.wanted.cut.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    fn sample_type(&self) -> SampleType {
        SampleType::Cf32
    }

    fn settings(&self) -> Vec<SettingDecl> {
        self.settings.clone()
    }

    fn link(&self) -> Link {
        if self.thread.is_some() {
            Link::Up
        } else {
            Link::Idle
        }
    }
}

impl Drop for SynthSource {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The thread of a test signal: makes the signal of the last version of [`Wanted`] and writes it into `sink` until
/// `stop`. It takes a new version before each block, and at once when [`Wanted::cut`] ends the block it is writing.
fn play(
    sink: RingWriter,
    wanted: &Wanted,
    stop: &AtomicBool,
    factory: &Arc<dyn GeneratorFactory>,
    stack: &str,
) {
    let _ended = Ended(wanted);
    let mut version = u64::MAX;
    let mut gen: Option<Box<dyn Generator>> = None;
    let mut made: Option<SignalInputs> = None;
    let mut opts = Options {
        pacing: Pacing::RealTime(1.0),
        quantize: true,
        scale: None,
    };
    let mut out = PacedWriter::new(sink, DEFAULT_RATE_HZ, opts.pacing);
    let mut block = vec![Complex32::default(); BLOCK];
    let mut noise = ConverterNoise(0x9E37_79B9_7F4A_7C15);
    loop {
        let changed = {
            let mut w = lock(&wanted.now);
            let cut = wanted.cut.swap(false, Ordering::AcqRel);
            (w.version != version).then(|| {
                w.taken = w.version;
                (w.values.clone(), w.inputs.clone(), w.version, cut)
            })
        };
        if stop.load(Ordering::Acquire) {
            return;
        }
        if let Some((values, inputs, v, cut)) = changed {
            wanted.taken.notify_all();
            version = v;
            let rate = rate_of(&values);
            let next = Options::from(&values);
            // Only a change of the inputs, or a dropped block, makes the signal again; the running one keeps its phase and
            // its time.
            let remake = cut || !made.as_ref().is_some_and(|m| m.makes_same(&inputs));
            if remake {
                let m = match stack {
                    "" => None,
                    s => factory.make(s, &values, rate),
                };
                let inputs = m.as_ref().map_or(inputs, |m| m.inputs.clone());
                gen = m.map(|m| m.gen);
                // The signal now plays for every later version that asks for the same signal.
                let mut w = lock(&wanted.now);
                if inputs.makes_same(&w.inputs) {
                    w.inputs = inputs.clone();
                }
                made = Some(inputs);
            }
            if remake || next.pacing != opts.pacing {
                out.reanchor(rate, next.pacing);
            }
            opts = next;
        }
        let n = gen.as_mut().map_or(0, |g| g.generate(&mut block));
        // A finished or missing signal goes on as silence.
        block[n.min(BLOCK)..].fill(Complex32::default());
        if let Some(s) = opts.scale {
            block.iter_mut().for_each(|c| *c *= s);
        }
        if opts.quantize {
            block
                .iter_mut()
                .for_each(|c| {
                    *c = Complex32::new(quantize8(c.re + noise.next()), quantize8(c.im + noise.next()));
                });
        }
        // A cut that is not a stop drops the rest of the block; the next turn takes the new version.
        if !out.push_iq(&block, &wanted.cut)
            && (stop.load(Ordering::Acquire) || !wanted.cut.load(Ordering::Acquire))
        {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ring;
    use crate::testing::values;
    use sdrkit_core::{Schema, Value};
    use std::time::{Duration, Instant};

    /// The period of the ramp, which does not divide [`BLOCK`]: a ramp made again shows as a step.
    const PERIOD: u64 = 251;

    /// A ramp generator: sample i has I = (i mod [`PERIOD`]) / 512.
    struct Ramp {
        i: u64,
        rate: Hz,
        len: Option<u64>,
    }
    impl Generator for Ramp {
        fn rate_hz(&self) -> Hz {
            self.rate
        }
        fn generate(&mut self, out: &mut [Complex32]) -> usize {
            let mut n = 0;
            for o in out.iter_mut() {
                if self.len.is_some_and(|l| self.i >= l) {
                    break;
                }
                *o = Complex32::new((self.i % PERIOD) as f32 / 512.0, 0.7);
                self.i += 1;
                n += 1;
            }
            n
        }
    }

    /// A factory of one stack, `ramp`, with one setting, `ramp_len`.
    struct Ramps {
        len: Option<u64>,
        seen: Mutex<Vec<String>>,
    }

    const RAMP_LEN: Setting<field::F64> = Setting::new("ramp_len", "Length", "", 0.0);

    impl GeneratorFactory for Ramps {
        fn settings(&self, stack: &str) -> Option<Vec<SettingDecl>> {
            (stack == "ramp" || stack.starts_with("ramp/"))
                .then(|| Schema::of(&[&RAMP_LEN, &LNA]).settings().to_vec())
        }
        fn make(&self, stack: &str, values: &Values, rate: Hz) -> Option<Made> {
            lock(&self.seen).push(stack.to_owned());
            let set =
                values
                    .schema()
                    .id(RAMP_LEN.key())
                    .ok()
                    .and_then(|id| match values.value(id) {
                        Some(sdrkit_core::Value::F64(x)) if *x > 0.0 => Some(*x as u64),
                        _ => None,
                    });
            Some(Made {
                gen: Box::new(Ramp {
                    i: 0,
                    rate,
                    len: set.or(self.len),
                }),
                inputs: self.inputs(stack, values, rate),
            })
        }
    }

    fn ramps(len: Option<u64>) -> Arc<Ramps> {
        Arc::new(Ramps {
            len,
            seen: Mutex::default(),
        })
    }

    fn driver(len: Option<u64>) -> SynthDriver {
        SynthDriver::new(ramps(len))
    }

    fn tune(d: &SynthDriver, rate: Hz, flags: &[(&str, bool)]) -> Values {
        let mut set = vec![("rate", Value::Hz(rate))];
        set.extend(flags.iter().map(|(k, b)| (*k, Value::Bool(*b))));
        values(d, &set)
    }

    fn cf32(n: usize) -> (RingWriter, crate::RingReader) {
        ring(n, SampleType::Cf32)
    }

    #[test]
    fn quantisation_and_the_gain_model() {
        assert_eq!(quantize8(1.0), 1.0);
        assert_eq!(quantize8(5.0), 127.0 / 32.0);
        assert_eq!(quantize8(-5.0), -4.0);
        assert_eq!(quantize8(0.3), 10.0 / 32.0);
        assert!((gain_model_scale(24.0, 24.0, true) - 1.0).abs() < 1e-6);
        assert!((gain_model_scale(24.0, 24.0, false) - 0.1995).abs() < 1e-3);
    }

    #[test]
    fn plays_in_real_time_by_default() {
        let d = driver(None);
        let mut src = d.open("ramp", &tune(&d, 500e3, &[])).unwrap();
        let (w, mut r) = cf32(1 << 20);
        let s = src.start(w).unwrap();
        assert_eq!(s.rate_hz, 500e3);
        let t = Instant::now();
        std::thread::sleep(Duration::from_millis(400));
        src.stop();
        let got = r.available() as f64;
        let expect = t.elapsed().as_secs_f64() * 500e3;
        assert!(
            got > expect * 0.6 && got < expect * 1.3,
            "{got} vs {expect}"
        );
        let mut buf = vec![Complex32::default(); 4];
        r.read_iq(&mut buf);
        // Rounded to the step, with the noise of the converter of at most half a step.
        let want = Complex32::new(1.0 / 512.0, 0.7);
        for (got, want) in [(buf[1].re, want.re), (buf[1].im, want.im)] {
            assert!((got - want).abs() <= STEP, "{got} vs {want}");
            assert_eq!(got, quantize8(got));
        }
    }

    #[test]
    fn benchmark_runs_as_fast_as_the_ring_allows_and_stops_at_once() {
        let d = driver(None);
        let mut src = d
            .open(
                "ramp",
                &tune(&d, 1e3, &[("benchmark", true), ("quantize", false)]),
            )
            .unwrap();
        let (w, mut r) = cf32(1 << 22);
        src.start(w).unwrap();
        let mut total = 0usize;
        let mut buf = vec![Complex32::default(); 1 << 16];
        let t = Instant::now();
        let mut ok = true;
        let mut idx = 0u64;
        while t.elapsed() < Duration::from_millis(300) {
            let n = r.read_iq(&mut buf);
            for c in &buf[..n] {
                ok &= c.re == (idx % PERIOD) as f32 / 512.0;
                idx += 1;
            }
            total += n;
        }
        let t = Instant::now();
        src.stop();
        assert!(t.elapsed() < Duration::from_millis(200));
        assert!(ok, "samples out of order or lost");
        assert!(total > 100_000, "{total}"); // far faster than 1 ksps
        assert!(r.available() <= crate::paced::BENCHMARK_QUEUE);
    }

    #[test]
    fn a_finished_signal_or_no_generator_produces_silence() {
        let d = driver(Some(10));
        let mut src = d
            .open("ramp", &tune(&d, 1e6, &[("benchmark", true)]))
            .unwrap();
        let (w, mut r) = cf32(1 << 18);
        src.start(w).unwrap();
        r.wait(BLOCK * 2, Duration::from_secs(2));
        src.stop();
        let mut buf = vec![Complex32::default(); BLOCK * 2];
        r.read_iq(&mut buf);
        assert!(buf[9].im != 0.0 && buf[10..].iter().all(|c| *c == Complex32::default()));

        let d = driver(None);
        let mut src = d.open("", &tune(&d, 0.0, &[("benchmark", true)])).unwrap();
        let (w, mut r) = cf32(1 << 16);
        assert_eq!(src.start(w).unwrap().rate_hz, DEFAULT_RATE_HZ);
        r.wait(100, Duration::from_secs(2));
        src.stop();
        let n = r.read_iq(&mut buf);
        assert!(n > 0 && buf[..n].iter().all(|c| *c == Complex32::default()));
    }

    #[test]
    fn the_factory_gets_the_stack_id_of_the_address() {
        let f = ramps(None);
        let d = SynthDriver::new(f.clone());
        let v = tune(&d, 1e6, &[("benchmark", true)]);
        for address in ["synth:ramp", "ramp/fast", "synth:", ""] {
            let mut src = d.open(address, &v).unwrap();
            let (w, r) = cf32(1 << 16);
            src.start(w).unwrap();
            r.wait(1, Duration::from_millis(500));
            src.stop();
        }
        assert_eq!(*lock(&f.seen), ["ramp", "ramp/fast"]);
        assert!(matches!(d.open("fm", &v), Err(DriverError::Address)));
        assert!(matches!(d.open("synth:fm", &v), Err(DriverError::Address)));
    }

    #[test]
    fn the_source_has_the_settings_of_the_stack_and_the_generator_gets_them() {
        let d = driver(None);
        let v = values(
            &d,
            &[
                ("rate", Value::Hz(1e6)),
                ("benchmark", Value::Bool(true)),
                ("quantize", Value::Bool(false)),
            ],
        );
        let mut src = d.open("ramp", &v).unwrap();
        // The driver has `lna` already: only `ramp_len` is added.
        let keys: Vec<_> = src.settings().iter().map(|s| s.field.key.clone()).collect();
        assert_eq!(keys, [RAMP_LEN.key()]);
        let mut decls = v.schema().settings().to_vec();
        decls.extend(src.settings());
        let mut all = Values::defaults(&Arc::new(Schema::new(decls).unwrap()));
        for i in 0..v.schema().len() {
            let id = sdrkit_core::SettingId(i as u16);
            all.set(id, v.value(id).unwrap()).unwrap();
        }
        let len = all.schema().id(RAMP_LEN.key()).unwrap();
        all.set(len, &Value::F64(8000.0)).unwrap();
        src.apply(&all).unwrap();
        let (w, mut r) = cf32(1 << 16);
        src.start(w).unwrap();
        r.wait(9000, Duration::from_secs(2));
        src.stop();
        let mut buf = vec![Complex32::default(); 9000];
        r.read_iq(&mut buf);
        assert_eq!(buf[3], Complex32::new(3.0 / 512.0, 0.7));
        assert!(buf[7999].im != 0.0 && buf[8000] == Complex32::default());
    }

    /// A setting of the test signal given in the open call of the kind is in force for the first sample, and the signal
    /// is made once.
    #[test]
    fn a_setting_of_the_signal_given_at_open_is_in_force_for_the_first_sample() {
        use sdrkit_core::Owner as _;
        let f = ramps(None);
        let k = crate::testing::kind(SynthDriver::new(f.clone()));
        let schema = k.schema_at("ramp").unwrap();
        assert_eq!(schema.len(), k.schema().len() + 1);
        let mut v = Values::defaults(&schema);
        let set = |v: &mut Values, key: &str, x: Value| v.set(schema.id(key).unwrap(), &x).unwrap();
        set(&mut v, "rate", Value::Hz(1e6));
        set(&mut v, "benchmark", Value::Bool(true));
        set(&mut v, "quantize", Value::Bool(false));
        set(&mut v, RAMP_LEN.key(), Value::F64(8000.0));
        let mut radio = k.open("ramp", &v).unwrap();
        assert_eq!(radio.values().unwrap().get(&RAMP_LEN), 8000.0);
        let (w, mut r) = cf32(1 << 16);
        radio.start(w).unwrap();
        r.wait(9000, Duration::from_secs(2));
        radio.stop();
        let mut buf = vec![Complex32::default(); 9000];
        r.read_iq(&mut buf);
        assert!(buf[7999].im != 0.0 && buf[8000] == Complex32::default());
        assert_eq!(lock(&f.seen).len(), 1);
    }

    /// The first `n` samples of the ramp in benchmark pacing with quantisation, with the values `change` applied once
    /// `at` samples were read, and the number of generators made.
    fn stream(n: usize, at: usize, change: &[(&str, Value)]) -> (Vec<Complex32>, usize) {
        let f = ramps(None);
        let d = SynthDriver::new(f.clone());
        let mut set = vec![("rate", Value::Hz(1e6)), ("benchmark", Value::Bool(true))];
        let mut src = d.open("ramp", &values(&d, &set)).unwrap();
        // A small ring keeps the signal at most two blocks ahead of the reader, so the change falls inside the stream.
        let (w, mut r) = cf32(BLOCK * 2);
        src.start(w).unwrap();
        let mut got = vec![Complex32::default(); n];
        let (mut have, mut applied) = (0, change.is_empty());
        let t = Instant::now();
        while have < n && t.elapsed() < Duration::from_secs(5) {
            if have >= at && !applied {
                set.extend(change.iter().cloned());
                src.apply(&values(&d, &set)).unwrap();
                applied = true;
            }
            have += r.read_iq(&mut got[have..]);
        }
        src.stop();
        assert_eq!(have, n);
        let made = lock(&f.seen).len();
        (got, made)
    }

    #[test]
    fn a_change_that_the_generator_does_not_read_leaves_the_signal_as_it_runs() {
        let n = BLOCK * 8;
        let (plain, made) = stream(n, 0, &[]);
        assert_eq!(made, 1);
        // `vga` and `amp` are settings of the driver that the ramp does not read; without the gain model they change
        // nothing.
        let (changed, made) = stream(
            n,
            BLOCK,
            &[("vga", Value::F64(10.0)), ("amp", Value::Bool(false))],
        );
        assert!(plain == changed, "the stream changed");
        assert_eq!(made, 1, "the generator was made again");
        // `lna` is a setting that the ramp declares: a change makes it again.
        let (_, made) = stream(n, BLOCK, &[("lna", Value::F64(10.0))]);
        assert_eq!(made, 2);
    }

    #[test]
    fn a_retune_rebuilds_the_generator() {
        let d = driver(None);
        let mut src = d
            .open(
                "ramp",
                &tune(&d, 1e6, &[("benchmark", true), ("quantize", false)]),
            )
            .unwrap();
        let (w, mut r) = cf32(1 << 18);
        src.start(w).unwrap();
        r.wait(1000, Duration::from_secs(2));
        let Retuned::Live(s) = src
            .apply(&tune(
                &d,
                2e6,
                &[
                    ("benchmark", true),
                    ("quantize", false),
                    ("gainmodel", true),
                    ("amp", false),
                ],
            ))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(s.rate_hz, 2e6);
        // Drain until the scaled ramp shows (I of sample 1 is 1/512 x 0.1995).
        let mut buf = vec![Complex32::default(); 1 << 16];
        let t = Instant::now();
        let mut seen = false;
        while t.elapsed() < Duration::from_secs(2) && !seen {
            let n = r.read_iq(&mut buf);
            seen = buf[..n].iter().any(|c| (c.im - 0.7 * 0.19953).abs() < 1e-4);
        }
        src.stop();
        assert!(seen);
    }

    #[test]
    fn at_a_low_rate_the_samples_after_a_remake_are_of_the_new_signal() {
        let d = driver(None);
        let base = [("rate", Value::Hz(48e3)), ("quantize", Value::Bool(false))];
        let mut src = d.open("ramp", &values(&d, &base)).unwrap();
        // A ring shorter than a block: the thread waits for room with the rest of its block, which lasts 683 ms.
        let (w, mut r) = cf32(4096);
        src.start(w).unwrap();
        assert_eq!(r.wait(4096, Duration::from_secs(2)), 4096);
        std::thread::sleep(Duration::from_millis(20));
        let mut set = base.to_vec();
        set.push(("lna", Value::F64(10.0)));
        let t = Instant::now();
        src.apply(&values(&d, &set)).unwrap();
        let took = t.elapsed();
        // What the ring held at the call is of the old signal; the next sample starts the new ramp.
        let mut old = vec![Complex32::default(); r.available()];
        assert_eq!(r.read_iq(&mut old), old.len());
        assert!(r.wait(2, Duration::from_secs(2)) >= 2);
        let mut next = [Complex32::default(); 2];
        r.read_iq(&mut next);
        src.stop();
        assert_eq!([next[0].re, next[1].re], [0.0, 1.0 / 512.0], "old samples after the call");
        assert!(took < Duration::from_millis(100), "{took:?}");
    }

    #[test]
    fn at_a_low_rate_a_remake_ends_the_wait_for_the_time_of_a_block() {
        let d = driver(None);
        let base = [("rate", Value::Hz(48e3)), ("quantize", Value::Bool(false))];
        let mut src = d.open("ramp", &values(&d, &base)).unwrap();
        // A ring larger than a block: the thread has written its block and waits 683 ms for its time.
        let (w, mut r) = cf32(BLOCK * 4);
        let stats = w.stats();
        src.start(w).unwrap();
        assert_eq!(r.wait(BLOCK, Duration::from_secs(2)), BLOCK);
        std::thread::sleep(Duration::from_millis(20));
        let mut set = base.to_vec();
        set.push(("lna", Value::F64(10.0)));
        let t = Instant::now();
        src.apply(&values(&d, &set)).unwrap();
        let took = t.elapsed();
        let at = usize::try_from(stats.written()).unwrap();
        assert!(r.wait(at + 2, Duration::from_secs(2)) >= at + 2);
        let mut buf = vec![Complex32::default(); at + 2];
        assert_eq!(r.read_iq(&mut buf), at + 2);
        src.stop();
        assert_eq!([buf[at].re, buf[at + 1].re], [0.0, 1.0 / 512.0], "old samples after the call");
        assert!(took < Duration::from_millis(100), "{took:?}");
    }

    /// A factory of a ramp whose make waits for [`Gated::go`], whose wanted inputs hold the radio centre as the tuned
    /// frequency, and whose made signal does not read it.
    #[derive(Default)]
    struct Gated {
        go: Mutex<bool>,
        opened: Condvar,
        made: Mutex<usize>,
    }

    impl GeneratorFactory for Gated {
        fn settings(&self, _: &str) -> Option<Vec<SettingDecl>> {
            Some(Vec::new())
        }
        fn make(&self, stack: &str, values: &Values, rate: Hz) -> Option<Made> {
            let go = lock(&self.go);
            drop(self.opened.wait_while(go, |go| !*go).unwrap_or_else(std::sync::PoisonError::into_inner));
            *lock(&self.made) += 1;
            Some(Made {
                gen: Box::new(Ramp { i: 0, rate, len: None }),
                inputs: SignalInputs {
                    tuned_hz: None,
                    ..self.inputs(stack, values, rate)
                },
            })
        }
        fn inputs(&self, _: &str, values: &Values, rate: Hz) -> SignalInputs {
            SignalInputs {
                rate_hz: rate,
                tuned_hz: Some(values.get(&CENTRE)),
                ..SignalInputs::default()
            }
        }
    }

    /// A signal that does not read the tuned frequency is not made again by a retune, also when an equal change came
    /// while it was being made.
    #[test]
    fn a_retune_leaves_a_signal_that_does_not_read_the_tuned_frequency() {
        let f = Arc::new(Gated::default());
        let d = SynthDriver::new(f.clone());
        let at = |hz: f64| {
            values(
                &d,
                &[
                    ("rate", Value::Hz(1e6)),
                    ("centre_hz", Value::Hz(hz)),
                    ("benchmark", Value::Bool(true)),
                ],
            )
        };
        let mut src = d.open("ramp", &at(100e6)).unwrap();
        let (w, _r) = cf32(1 << 16);
        src.start(w).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        src.apply(&at(100e6)).unwrap();
        *lock(&f.go) = true;
        f.opened.notify_all();
        std::thread::sleep(Duration::from_millis(100));
        src.apply(&at(101e6)).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        src.stop();
        assert_eq!(*lock(&f.made), 1);
    }
}
