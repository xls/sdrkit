//! What the crate registers beside its descriptors: one verification case per stack (its test signal off the tuned frequency)
//! and the benchmarks of the hot paths. The checks read the typed results.

use sdrkit_core::interface::Column;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    BenchResult, Benchmark, Complex32, GenConfig, InputInfo, Output, Results, StackConfig,
    StackDescriptor, Value, Values,
};

pub(crate) static DRM: VerifyHooks = VerifyHooks {
    cases: drm_cases,
    vector_settings: None,
    check_vector: None,
};

pub(crate) static HAMDRM: VerifyHooks = VerifyHooks {
    cases: hamdrm_cases,
    vector_settings: None,
    check_vector: None,
};

pub(crate) static HDRADIO: VerifyHooks = VerifyHooks {
    cases: hd_cases,
    vector_settings: None,
    check_vector: None,
};

pub(crate) static CDR: VerifyHooks = VerifyHooks {
    cases: cdr_cases,
    vector_settings: None,
    check_vector: None,
};

pub(crate) static CQUAM: VerifyHooks = VerifyHooks {
    cases: cquam_cases,
    vector_settings: None,
    check_vector: None,
};

/// The test signal 300 Hz off: the FAC decodes.
fn drm_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 300.0);
    c.seconds = c.seconds.max(8.0);
    c.check = Some(|r| at_least(r, crate::drm::results::DrmFields::FAC_OK.key, 5.0));
    vec![c]
}

/// Without the format file: the stack says that the HamDRM format is not loaded. It sends no signal, so nothing locks.
fn hamdrm_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("no format", o, 50.0);
    c.seconds = c.seconds.max(2.0);
    c.expect_lock = false;
    c.check = Some(|r| {
        let t = text(r, &crate::hamdrm::HamdrmFields::FORMAT);
        vec![CheckLine::new(
            "format",
            t.starts_with("not loaded"),
            format!("format: {t}"),
        )]
    });
    vec![c]
}

/// The FM test signal 1 kHz off: P1 transfer frames decode.
fn hd_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 1e3);
    c.seconds = c.seconds.max(6.0);
    c.check = Some(|r| at_least(r, crate::hd::results::HdradioFields::P1_OK.key, 1.0));
    vec![c]
}

/// The test signal 1 kHz off, without the GY/T 268.1 tables: the beacon is found and the receiver says that the tables are not
/// loaded.
fn cdr_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, no tables", o, 1e3);
    c.seconds = c.seconds.max(2.0);
    c.check = Some(|r| {
        let mut v = at_least(r, crate::cdr::results::CdrFields::SUBFRAMES.key, 4.0);
        let t = text(r, &crate::cdr::results::CdrFields::TABLES);
        v.push(CheckLine::new(
            "tables",
            t.starts_with("not loaded"),
            format!("tables: {t}"),
        ));
        v
    });
    vec![c]
}

/// The stereo test signal: the pilot is seen and the receiver is in stereo.
fn cquam_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(4.0);
    c.expect_audio = Some(true);
    c.check = Some(|r| at_least(r, crate::typed::CquamFields::PILOT_CHECKS_OK.key, 1.0));
    vec![c]
}

/// The text of result field `c`, empty when it has none.
fn text<'a>(r: &'a Results, c: &Column<field::Text>) -> &'a str {
    r.fields().at(c).unwrap_or("")
}

/// The number of result field `key` at least `min`.
fn at_least(r: &Results, key: &'static str, min: f64) -> Vec<CheckLine> {
    let v = r
        .decl()
        .field_id(key)
        .and_then(|id| r.fields().value(id))
        .and_then(|v| match v {
            Value::I64(x) => Some(*x as f64),
            Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => Some(*x),
            _ => None,
        })
        .unwrap_or(f64::NAN);
    vec![CheckLine::new(
        key,
        v >= min,
        format!("{key} {v}, at least {min}"),
    )]
}

/// The benchmarks of this crate: each receiver on its own test signal (items are input samples).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "broadcast.cquam_250k",
            run: |s| bench_mode(&crate::CQUAM, 250e3, 0.0, s),
        },
        Benchmark {
            name: "broadcast.hdradio_fm_744k",
            run: |s| bench_mode(&crate::HDRADIO, crate::hd::l1::FM_RATE, 100e6, s),
        },
        Benchmark {
            name: "broadcast.drm_48k",
            run: |s| bench_mode(&crate::DRM, 48e3, 0.0, s),
        },
    ]
}

/// One second of a stack's own test signal through its receiver at `rate`, tuned to `tuned` (above 30 MHz: the FM band
/// of HD Radio).
fn bench_mode(d: &StackDescriptor, rate: f64, tuned: f64, seconds: f64) -> BenchResult {
    let values = Values::defaults(&(d.settings)());
    let cfg = GenConfig::new(rate, values.clone()).with_seed(1);
    let mut g = d.make_generator.and_then(|m| m(&cfg).ok());
    let n = rate as usize;
    let mut x = vec![Complex32::default(); n];
    if let Some(g) = &mut g {
        g.generate(&mut x);
    }
    let Ok(mut rx) = (d.make_receiver)(&StackConfig::new(values)) else {
        return BenchResult::default();
    };
    let input = InputInfo::new(rate, tuned, tuned).recording();
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, 1.0, || {
        let _ = rx.configure(&input);
        for c in x.chunks(16_384) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}
