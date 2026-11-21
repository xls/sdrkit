//! Helpers of the tests: receivers from settings, feeding, the rows of declared tables, and impairments from
//! sdrkit-algo-sim.

use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_core::publish::TableDef;
use sdrkit_core::{
    Complex32, InputInfo, Output, Receiver, Record, Results, StackConfig, StackDescriptor, Value,
    ValueKind, Values,
};

/// The stored settings `settings` (key and text) of stack `d`, read by each setting's kind: yes/no as `1`/`0` or
/// `true`/`false`, numbers in their unit, a choice by its key.
pub fn values(d: &StackDescriptor, settings: &[(&str, &str)]) -> Values {
    let schema = (d.settings)();
    let mut v = Values::defaults(&schema);
    for (k, t) in settings {
        let id = schema.id(k).expect("the stack has the setting");
        let decl = schema.setting(id).expect("a declared setting");
        let num = || t.parse::<f64>().expect("a number");
        let value = match decl.field.kind {
            ValueKind::Bool => Value::Bool(matches!(*t, "1" | "true")),
            ValueKind::F64 => Value::F64(num()),
            ValueKind::Hz => Value::Hz(num()),
            ValueKind::Duration => Value::Duration(num()),
            ValueKind::I64 => Value::I64(num() as i64),
            ValueKind::Choice => Value::Choice(decl.field.choice_index(t).expect("a choice key")),
            ValueKind::Text => Value::Text((*t).to_owned()),
            other => panic!("no text form for a setting of kind {other:?}"),
        };
        v.set(id, &value).expect("the setting takes the value");
    }
    v
}

/// The receiver of stack `d` with these stored settings, configured for a recording at `rate` with the radio at `centre`.
pub fn receiver(
    d: &StackDescriptor,
    rate: f64,
    centre: f64,
    tuned: f64,
    settings: &[(&str, &str)],
) -> Box<dyn Receiver> {
    let mut rx = (d.make_receiver)(&StackConfig::new(values(d, settings))).unwrap();
    rx.configure(&InputInfo::new(rate, centre, tuned).recording())
        .expect("the receiver takes the rate");
    rx
}

/// Feeds `x` in pieces of `chunk` samples.
pub fn run(rx: &mut dyn Receiver, x: &[Complex32], chunk: usize) -> Output {
    let mut all = Output::default();
    let mut out = Output::default();
    for c in x.chunks(chunk) {
        out.clear();
        rx.feed(c, &mut out);
        all.events.append(&mut out.events);
        for part in out.audio_runs() {
            all.push_audio(part.samples, part.rate_hz, part.channels);
        }
        all.requests.append(&mut out.requests);
    }
    all
}

/// One row of a declared table.
pub type Row = Record;

/// The rows of table `def` in `r`, newest first.
pub fn rows(r: &Results, def: &TableDef) -> Vec<Row> {
    let Some(t) = r.decl().table_id(def.key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first().map(|(_, rec)| rec.clone()).collect()
}

/// The value of column `key` of a row.
fn cell<'a>(r: &'a Row, key: &str) -> Option<&'a Value> {
    r.value(r.decl().id(key)?)
}

/// The number in `v` (0 when it holds none).
fn number(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x)) => *x,
        Some(Value::I64(x)) => *x as f64,
        _ => 0.0,
    }
}

/// A text cell.
pub fn text<'a>(r: &'a Row, key: &str) -> &'a str {
    match cell(r, key) {
        Some(Value::Text(t)) => t,
        _ => "",
    }
}

/// A number cell (0 when empty).
pub fn num(r: &Row, key: &str) -> f64 {
    number(cell(r, key))
}

/// A whole-number cell (0 when empty).
pub fn int(r: &Row, key: &str) -> i64 {
    match cell(r, key) {
        Some(Value::I64(v)) => *v,
        _ => 0,
    }
}

/// A yes/no cell.
pub fn flag(r: &Row, key: &str) -> bool {
    matches!(cell(r, key), Some(Value::Bool(true)))
}

/// A position cell.
pub fn position(r: &Row, key: &str) -> Option<(f64, f64)> {
    match cell(r, key) {
        Some(Value::Position { lat, lon }) => Some((*lat, *lon)),
        _ => None,
    }
}

/// A number field of the results (0 when absent or empty).
pub fn field(r: &Results, key: &str) -> f64 {
    number(r.decl().field_id(key).and_then(|id| r.fields().value(id)))
}

/// Adds white noise for a signal of mean power `power` at `snr_db` in `bandwidth_hz`.
pub fn noise(
    x: &mut [Complex32],
    power: f64,
    snr_db: f64,
    bandwidth_hz: f64,
    rate: f64,
    seed: u64,
) {
    let mut n = Awgn::complex(
        NoiseLevel::SnrDb {
            snr_db,
            bandwidth_hz,
        },
        power,
        rate,
        seed,
    )
    .unwrap();
    n.process(x);
}

/// Moves the whole signal by `hz`.
pub fn offset(x: &mut [Complex32], rate: f64, hz: f64) {
    sdrkit_algo_sim::offset::FrequencyOffset::new(rate, hz, 0.0)
        .unwrap()
        .process(x);
}

/// Resamples the signal as a sample clock `ppm` fast does.
pub fn clock(x: &[Complex32], ppm: f64) -> Vec<Complex32> {
    let mut y = Vec::new();
    sdrkit_algo_sim::clock::ClockError::new(ppm)
        .unwrap()
        .process(x, &mut y);
    y
}

/// An ADC of `bits` bits clipping at `full_scale`.
pub fn quantise(x: &mut [Complex32], bits: u32, full_scale: f32) {
    for v in x.iter_mut() {
        *v /= full_scale;
    }
    sdrkit_algo_sim::front::Quantiser::new(bits)
        .unwrap()
        .process(x);
    for v in x.iter_mut() {
        *v *= full_scale;
    }
}

/// Adds an echo `delay` samples late at `db` relative to the signal.
pub fn echo(x: &mut [Complex32], delay: usize, db: f64) {
    let g = 10f64.powf(db / 20.0) as f32;
    for i in (delay..x.len()).rev() {
        let d = x[i - delay] * g;
        x[i] += d;
    }
}

/// The impairments of a test: the standard chain of sdrkit-algo-sim, at the generator's own level.
pub fn chain(
    g: &dyn sdrkit_core::Generator,
    mut cfg: sdrkit_algo_sim::chain::ChainConfig,
) -> sdrkit_algo_sim::chain::Chain {
    let info = g.signal_info().expect("the generator states its signal");
    let mut sig = sdrkit_algo_sim::chain::SignalInfo::new(g.rate_hz())
        .with_power(info.power)
        .with_bandwidth(info.bandwidth_hz);
    if let Some(s) = info.symbol_rate_hz {
        sig = sig.with_symbol_rate(s);
    }
    cfg.level_dbfs = 10.0 * info.power.log10();
    sdrkit_algo_sim::chain::Chain::new(&cfg, &sig).expect("a valid chain")
}

/// `seconds` of the generator through `chain`, fed to `rx` in pieces of `piece` samples.
pub fn stream(
    g: &mut dyn sdrkit_core::Generator,
    chain: &mut sdrkit_algo_sim::chain::Chain,
    rx: &mut dyn Receiver,
    seconds: f64,
    piece: usize,
) {
    let total = (seconds * g.rate_hz()) as usize;
    let mut buf = vec![Complex32::default(); piece];
    let mut y = Vec::with_capacity(2 * piece);
    let mut out = Output::default();
    let mut done = 0;
    while done < total {
        let n = piece.min(total - done);
        g.generate(&mut buf[..n]);
        y.clear();
        chain.process(&buf[..n], &mut y);
        out.clear();
        rx.feed(&y, &mut out);
        done += n;
    }
}

/// Runs the stack command `key` with its default arguments, through the checked path.
pub fn command(rx: &mut dyn Receiver, key: &str) {
    let id = rx.commands().id(key).expect("the stack has the command");
    let args = rx
        .commands()
        .def(id)
        .expect("a declared command")
        .new_args();
    sdrkit_core::owner::invoke(rx, id, &args).expect("the command runs");
}

/// Applies the stack setting `key` through the checked path.
pub fn set(rx: &mut dyn Receiver, key: &str, v: &Value) -> Result<(), sdrkit_core::SettingError> {
    let id = rx.schema().id(key)?;
    sdrkit_core::owner::apply(rx, id, v)
}

/// The frequencies (Hz) of the set `set` of the stack `stack` of the repository's channel file.
pub fn channel_set(stack: &str, set: &str) -> Vec<f64> {
    use sdrkit_presets::avdata;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data")
        .join(avdata::DATALINK_CHANNELS);
    let rows = avdata::read_channels(&path).expect("the channel file loads");
    let v = avdata::set_hz(&rows, stack, set);
    assert!(!v.is_empty(), "{stack} {set}");
    v
}

/// The text of the setting `channels` for these frequencies (Hz).
pub fn channels_text(hz: &[f64]) -> String {
    hz.iter().map(|f| format!("{:.6}", f / 1e6)).collect::<Vec<_>>().join(" ")
}

/// The results of the receiver of `d` with its default settings on `secs` of its default test signal at the default
/// tuning, at `rate`.
pub fn default_run(d: &sdrkit_core::StackDescriptor, rate: f64, secs: f64) -> Box<dyn sdrkit_core::Receiver> {
    default_run_at(d, rate, secs, d.tuning.default_hz)
}

/// As [`default_run`], with the receiver tuned to `f` Hz and the samples centred there.
pub fn default_run_at(
    d: &sdrkit_core::StackDescriptor,
    rate: f64,
    secs: f64,
    f: f64,
) -> Box<dyn sdrkit_core::Receiver> {
    let cfg = sdrkit_core::GenConfig::new(rate, sdrkit_core::Values::defaults(&(d.settings)())).with_seed(1);
    let make = d.make_generator.expect("the stack has a test signal");
    let mut g = make(&cfg).expect("the test signal builds");
    let mut x = vec![sdrkit_core::Complex32::default(); (rate * secs) as usize];
    g.generate(&mut x);
    let mut rx = receiver(d, rate, f, f, &[]);
    run(rx.as_mut(), &x, 65_536);
    rx
}
