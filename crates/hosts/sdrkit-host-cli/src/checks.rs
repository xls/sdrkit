//! The tests an operator runs: `verify` (each stack's test signal through the engine), `bench` (the benchmarks of every
//! stack) and `vectors` (the recordings that the tests read).

use crate::error::CliError;
use crate::live;
use crate::out::Out;
use crate::reflect;
use crate::text;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::verify::{CaseSetting, CheckLine, Outcome, VerifyCase, VerifyOptions};
use sdrkit_core::{LockState, Owner, StackDescriptor, Value, Values};
use sdrkit_driver_core::synth::QUANTIZE;
use sdrkit_driver_core::{CENTRE, RATE_GIVEN};
use sdrkit_host_engine::{Engine, EngineError, Input, OwnerRef, RadioId, SessionId, SAMPLES};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::time::{Duration, Instant};

/// One line of a test table.
struct Check {
    stack: &'static str,
    name: String,
    outcome: Outcome,
    detail: String,
}

fn check(stack: &'static str, name: String, outcome: Outcome, detail: impl Into<String>) -> Check {
    Check {
        stack,
        name,
        outcome,
        detail: detail.into(),
    }
}

/// Prints the checks as a table or JSON lines and counts the failures.
fn report(checks: &[Check], out: &mut Out) -> Result<usize, CliError> {
    let failed = checks.iter().filter(|c| c.outcome == Outcome::Fail).count();
    if out.json {
        for c in checks {
            out.json(&json!({ "stack": c.stack, "check": c.name, "outcome": c.outcome, "detail": c.detail }))?;
        }
    } else {
        let rows: Vec<Vec<String>> = checks
            .iter()
            .map(|c| {
                let o = match c.outcome {
                    Outcome::Pass => "pass",
                    Outcome::Fail => "FAIL",
                    Outcome::NotChecked => "not checked",
                };
                vec![
                    c.stack.to_owned(),
                    c.name.clone(),
                    o.to_owned(),
                    c.detail.clone(),
                ]
            })
            .collect();
        let mut s = String::new();
        text::table(
            &["Stack", "Check", "Outcome", "Detail"].map(String::from),
            &rows,
            &mut s,
        );
        let _ = std::fmt::Write::write_fmt(
            &mut s,
            format_args!("{} checks, {failed} failed\n", checks.len()),
        );
        out.text(&s)?;
    }
    Ok(failed)
}

/// What `verify` requires from each case.
#[derive(Clone, Copy, Debug)]
pub struct VerifyAsk {
    /// The length of the test signal, seconds.
    pub seconds: f64,
    /// No offset and no noise.
    pub clean: bool,
    /// The SNR of the noise, dB in the stack's bandwidth.
    pub snr_db: f64,
}

/// The cases of a stack: its own, or one case of its test signal at its defaults.
fn cases(d: &StackDescriptor, o: &VerifyOptions) -> Vec<VerifyCase> {
    if let Some(h) = d.verify {
        let own = (h.cases)(o);
        if !own.is_empty() {
            return own;
        }
    }
    let name = if o.clean {
        "test signal"
    } else {
        "test signal, impaired"
    };
    vec![VerifyCase::new(
        name,
        o,
        (0.01 * d.tuning.bandwidth_hz).min(1000.0).round(),
    )]
}

/// The rank of a lock state, from off to locked.
fn rank(l: LockState) -> u8 {
    match l {
        LockState::Off => 0,
        LockState::Searching => 1,
        LockState::Tracking => 2,
        LockState::Locked => 3,
    }
}

/// True if the results hold a value in a field or a row in a table.
fn has_content(o: &dyn Owner) -> bool {
    let r = o.results();
    r.fields()
        .iter()
        .any(|(d, v)| !v.is_none() && !d.key.starts_with("session."))
        || (0..r.decl().tables().len()).any(|i| {
            u16::try_from(i)
                .ok()
                .and_then(|i| r.table(sdrkit_core::TableId(i)))
                .is_some_and(|t| !t.is_empty())
        })
}

/// Runs command `key` of the session with the arguments of a case.
fn run_command(o: &mut dyn Owner, key: &str, args: &[CaseSetting]) -> Result<(), CliError> {
    let id = o
        .commands()
        .id(key)
        .map_err(|_| EngineError::no_command(key))?;
    let def = o
        .commands()
        .def(id)
        .ok_or_else(|| EngineError::no_command(key))?;
    let mut rec = def.new_args();
    for a in args {
        let bad = || CliError::UnknownSetting {
            owner: format!("the command `{key}`"),
            key: a.key.to_owned(),
        };
        let f = rec.decl().id(a.key).ok_or_else(bad)?;
        rec.set_value(f, &a.value).map_err(|_| bad())?;
    }
    EngineError::invoke(o, id, &rec)?;
    Ok(())
}

/// What a case run saw while its signal went in.
#[derive(Default)]
struct Seen {
    /// The seconds of signal that the session read, each sample counted at the rate of the input when it was read.
    signal_s: f64,
    best: LockState,
    first_lock_s: Option<f64>,
    sound_s: f64,
    loud_s: f64,
    /// The commands of the case that failed, with their errors.
    refused: Vec<String>,
}

/// The rate of the samples that `radio` gives now, or `fallback` while the radio states none.
fn input_rate(engine: &Engine, radio: RadioId, fallback: f64) -> Result<f64, CliError> {
    let given = engine.owner(OwnerRef::Radio(radio), |o| {
        o.results().fields().get(&RATE_GIVEN).unwrap_or(0.0)
    })?;
    Ok(if given > 0.0 { given } else { fallback })
}

/// Watches session `id`, fed by `radio`, until it has read `case.seconds` of signal: runs the case's commands at their
/// times, and watches the lock state and the sound. The samples of each poll count at the radio's rate at that poll, or
/// at `stack_rate` while the radio states no rate.
fn watch(
    engine: &Engine,
    id: SessionId,
    radio: RadioId,
    case: &VerifyCase,
    stack_rate: f64,
) -> Result<Seen, CliError> {
    let mut seen = Seen::default();
    let mut commands: Vec<_> = case.commands.iter().collect();
    let mut audio = vec![0.0f32; 1 << 14];
    let s = OwnerRef::Session(id);
    let limit = Duration::from_secs_f64(3.0 * case.seconds + 10.0);
    let t0 = Instant::now();
    let mut counted = 0i64;
    loop {
        let done = engine.owner(s, |o| o.results().fields().get(&SAMPLES).unwrap_or(0))?;
        let rate = input_rate(engine, radio, stack_rate)?;
        if rate > 0.0 && done > counted {
            seen.signal_s += (done - counted) as f64 / rate;
            counted = done;
        }
        let t = seen.signal_s;
        commands.retain(|c| {
            if c.at_s > t {
                return true;
            }
            if let Err(e) = engine
                .owner(s, |o| run_command(o, c.key, &c.args))
                .map_err(CliError::from)
                .and_then(|r| r)
            {
                seen.refused.push(format!("{}: {e}", c.key));
            }
            false
        });
        let lock_state = engine.owner(s, |o| o.results().status().lock_state)?;
        if rank(lock_state) > rank(seen.best) {
            seen.best = lock_state;
        }
        if seen.first_lock_s.is_none() && rank(lock_state) >= 2 {
            seen.first_lock_s = Some(t);
        }
        while let Some(r) = engine.audio(id, &mut audio)? {
            let n = r.frames * usize::from(r.channels);
            let secs = r.frames as f64 / r.rate_hz.max(1.0);
            let rms = (audio[..n].iter().map(|v| v * v).sum::<f32>() / n.max(1) as f32).sqrt();
            seen.sound_s += secs;
            if rms > 1e-3 {
                seen.loud_s += secs;
            }
        }
        if t >= case.seconds || t0.elapsed() > limit {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(seen)
}

/// Opens the test-signal radio of stack `d` for `case`, which a session of the stack with `stack_values` will read at
/// `freq_hz`: the stack's test signal with the case's settings, its offset and its noise, in real time and not
/// quantised. Every setting, of the driver kind and of the stack's test signal ([`Engine::radio_schema`]), with the centre
/// that the session will ask for, is set in the open call. The rate stays on Automatic unless the case states one, so
/// the engine takes the rate of each stack that the case starts on the radio.
fn open_signal(
    engine: &Engine,
    d: &StackDescriptor,
    case: &VerifyCase,
    stack_values: &Values,
    freq_hz: f64,
) -> Result<RadioId, CliError> {
    use sdrkit_core::Operator as _;
    let mut settings = vec![CaseSetting::new(&QUANTIZE, false)];
    settings.extend(case.settings.iter().cloned());
    if case.offset_hz != 0.0 {
        settings.push(CaseSetting::new(&GEN_OFFSET_HZ, case.offset_hz));
    }
    if let Some(snr) = case.snr_db {
        settings.push(CaseSetting::new(&GEN_NOISE, NoiseKind::Snr));
        settings.push(CaseSetting::new(&GEN_SNR_DB, snr));
    }
    let mut operator = sdrkit_core::TableOperator::of_stack(d, case.submode.as_deref());
    operator.set_rate(d.rate_hz(stack_values));
    operator.set_frequency(freq_hz);
    if let Some(rx) = operator.demand().receive {
        if rx.tuned_hz > 0.0 {
            settings.push(CaseSetting::new(&CENTRE, rx.tuned_hz + rx.tune_offset_hz));
        }
    }
    let address = match &case.submode {
        Some(m) => format!("{}/{m}", d.id),
        None => d.id.to_owned(),
    };
    let schema = engine.radio_schema("synth", &address)?;
    let mut values = Values::defaults(&schema);
    CaseSetting::apply_all(&settings, &mut values)
        .map_err(|e| CliError::from(EngineError::setting(&schema, e)))?;
    Ok(engine.open_radio("synth", &address, &values)?)
}

/// Runs one case of stack `d` through the engine, on the test-signal radio, and returns its checks.
fn run_case(
    engine: &Engine,
    d: &'static StackDescriptor,
    case: &VerifyCase,
) -> Result<Vec<Check>, CliError> {
    let f = |what: &str| format!("{}: {what}", case.name);
    let mut checks = Vec::new();
    let freq = case.freq_hz.unwrap_or(d.tuning.default_hz);
    let mut values = Values::defaults(&(d.settings)());
    let schema = std::sync::Arc::clone(values.schema());
    if let Err(e) = CaseSetting::apply_all(&case.settings, &mut values) {
        let e = CliError::from(EngineError::setting(&schema, e));
        checks.push(check(d.id, f("starts"), Outcome::Fail, e.to_string()));
        return Ok(checks);
    }
    let radio = match open_signal(engine, d, case, &values, freq) {
        Ok(r) => r,
        Err(e) => {
            checks.push(check(d.id, f("test signal"), Outcome::Fail, e.to_string()));
            return Ok(checks);
        }
    };
    let started = engine.start_stack(
        d.id,
        case.submode.as_deref(),
        &values,
        Input::Radio(radio),
        Some(freq),
    );
    let id = match started {
        Ok(id) => id,
        Err(e) => {
            let _ = engine.close_radio(radio);
            checks.push(check(d.id, f("starts"), Outcome::Fail, e.to_string()));
            return Ok(checks);
        }
    };
    checks.push(check(d.id, f("starts"), Outcome::Pass, ""));
    let seen = watch(engine, id, radio, case, d.tuning.rate_hz);
    let s = OwnerRef::Session(id);
    let after = engine.owner(s, |o| {
        let r = o.results();
        let dropped = r.fields().get(&sdrkit_host_engine::DROPPED).unwrap_or(0);
        let own: Vec<CheckLine> = case.check.map(|c| c(r)).unwrap_or_default();
        (has_content(o), dropped, own)
    });
    let ended = live::end(
        engine,
        &crate::host::Live {
            radio: Some(radio),
            session: id,
            stack: Some(d.id),
        },
    );
    let seen = seen?;
    let (content, dropped, own) = after?;
    ended?;
    let lock_state = format!("{:?}", seen.best).to_lowercase();
    checks.push(match (seen.first_lock_s, case.expect_lock) {
        (Some(t), _) => check(
            d.id,
            f("locks"),
            Outcome::Pass,
            format!("{lock_state} after {t:.1} s of signal"),
        ),
        (None, true) => check(
            d.id,
            f("locks"),
            Outcome::Fail,
            format!("only reached {lock_state}"),
        ),
        (None, false) => check(d.id, f("locks"), Outcome::NotChecked, lock_state),
    });
    checks.push(if content {
        check(d.id, f("results"), Outcome::Pass, "")
    } else {
        check(
            d.id,
            f("results"),
            Outcome::Fail,
            "the results stayed empty",
        )
    });
    let loud = seen.loud_s > 0.1;
    checks.push(match (case.expect_audio, loud) {
        (Some(false), true) => check(
            d.id,
            f("audio"),
            Outcome::Fail,
            format!("{:.1} s of sound from a stack without any", seen.loud_s),
        ),
        (_, true) => check(
            d.id,
            f("audio"),
            Outcome::Pass,
            format!("{:.1} s of sound", seen.loud_s),
        ),
        (Some(true), false) => check(
            d.id,
            f("audio"),
            Outcome::Fail,
            format!("no sound ({:.1} s of silence)", seen.sound_s),
        ),
        (_, false) => check(d.id, f("audio"), Outcome::NotChecked, "no sound"),
    });
    checks.push(if dropped == 0 {
        check(d.id, f("no sample loss"), Outcome::Pass, "")
    } else {
        check(
            d.id,
            f("no sample loss"),
            Outcome::Fail,
            format!("{dropped} samples dropped"),
        )
    });
    for r in &seen.refused {
        checks.push(check(d.id, f("command"), Outcome::Fail, r.clone()));
    }
    for c in own {
        checks.push(check(d.id, f(&c.key), c.outcome, c.detail));
    }
    Ok(checks)
}

/// `verify [stack...]`: runs each case of each stack (every stack with a test signal if none is named) through the
/// engine and prints a table of the checks.
///
/// # Errors
///
/// [`CliError::UnknownStack`], [`CliError::ChecksFailed`] if a check failed, or the error of the engine.
pub fn verify(
    engine: &Engine,
    ids: &[String],
    ask: VerifyAsk,
    out: &mut Out,
) -> Result<(), CliError> {
    let stacks: Vec<&'static StackDescriptor> = if ids.is_empty() {
        sdrkit_host_catalog::all()
            .into_iter()
            .filter(|d| d.make_generator.is_some())
            .collect()
    } else {
        ids.iter()
            .map(|i| live::stack(i))
            .collect::<Result<_, _>>()?
    };
    let o = VerifyOptions {
        seconds: ask.seconds,
        clean: ask.clean,
        snr_db: ask.snr_db,
    };
    let mut checks = Vec::new();
    for d in stacks {
        if d.make_generator.is_none() {
            checks.push(check(
                d.id,
                "test signal".into(),
                Outcome::NotChecked,
                "the stack has no test signal",
            ));
            continue;
        }
        for case in cases(d, &o) {
            checks.extend(run_case(engine, d, &case)?);
        }
    }
    let failed = report(&checks, out)?;
    if failed > 0 {
        return Err(CliError::ChecksFailed {
            failed,
            total: checks.len(),
        });
    }
    Ok(())
}

/// `bench`: runs the benchmarks whose name or stack id contains `filter` (all without one) through the engine's
/// `benchmark.run` command, each for about `seconds`, and prints each result as it comes. Then it prints the load tables
/// of each case that ran a graph (components by cost, connections, stages) and the status of each case. A case fails
/// when a thread that a session would give its stack needs more than one core at the rate of the stack.
///
/// # Errors
///
/// [`CliError::ChecksFailed`] if a case failed, and the error of the engine.
pub fn bench(
    engine: &Engine,
    filter: Option<&str>,
    seconds: f64,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    let only = filter.unwrap_or("");
    let expected: usize = sdrkit_host_catalog::all()
        .iter()
        .filter_map(|d| Some((d.id, d.benchmarks?)))
        .map(|(id, b)| {
            b().iter()
                .filter(|b| b.name.contains(only) || id.contains(only))
                .count()
        })
        .sum();
    let table = sdrkit_host_engine::BENCHMARKS.key;
    let mut since = engine.owner(OwnerRef::Engine, |o| {
        let r = o.results();
        r.decl()
            .table_id(table)
            .and_then(|t| r.table(t))
            .map_or(0, sdrkit_core::Table::change_counter)
    })?;
    reflect::call(
        engine,
        None,
        "engine",
        Some("benchmark.run"),
        &[format!("seconds={seconds}"), format!("only={only}")],
        &mut Out::new(&mut std::io::sink(), &mut std::io::sink(), false, false),
    )?;
    let mut done = 0;
    let mut names: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut headings = Vec::new();
    let t0 = Instant::now();
    let limit = Duration::from_secs_f64(seconds.max(0.1) * (expected as f64) * 4.0 + 30.0);
    while done < expected && !stop() && t0.elapsed() < limit {
        std::thread::sleep(Duration::from_millis(200));
        let json = out.json;
        let Some(t) = engine.owner(OwnerRef::Engine, |o| {
            reflect::table_text(o, table, since, json)
        })?
        else {
            break;
        };
        since = t.counter;
        done += t.rows.len().max(t.json.len());
        headings.clone_from(&t.headings);
        if json {
            for r in &t.json {
                if let Some(k) = r.get(0).and_then(|k| k.as_str()) {
                    names.push(k.to_owned());
                }
                out.json(r)?;
            }
        } else {
            for r in t.rows {
                out.line(&r.join("  "))?;
                names.push(r[0].clone());
                rows.push(r);
            }
        }
    }
    bench_loads(engine, &names, out)?;
    if !out.json && !rows.is_empty() {
        let mut s = String::from("\n");
        text::table(&headings, &rows, &mut s);
        out.text(&s)?;
    }
    // The status of each case, read again so that JSON output is checked too.
    let failed = bench_failures(engine, &names)?;
    for (name, why) in &failed {
        out.line(&format!("{name}: {why}"))?;
    }
    if failed.is_empty() {
        return Ok(());
    }
    out.line(&format!(
        "{} of {} benchmarks FAILED: a thread needs more than one core at the rate of its stack",
        failed.len(),
        names.len()
    ))?;
    Err(CliError::ChecksFailed {
        failed: failed.len(),
        total: names.len(),
    })
}

/// The cases of `names` whose status in the table of benchmarks is a failure, with the status.
fn bench_failures(engine: &Engine, names: &[String]) -> Result<Vec<(String, String)>, CliError> {
    let table = sdrkit_host_engine::BENCHMARKS.key;
    Ok(engine.owner(OwnerRef::Engine, |o| {
        let r = o.results();
        let Some(t) = r.decl().table_id(table).and_then(|t| r.table(t)) else {
            return Vec::new();
        };
        let Some(col) = t.columns().id(sdrkit_host_engine::BENCH_STATUS.key) else {
            return Vec::new();
        };
        names
            .iter()
            .filter_map(|n| match t.row(n)?.value(col)? {
                Value::Text(s) if s.starts_with("FAIL") => Some((n.clone(), s.to_string())),
                _ => None,
            })
            .collect()
    })?)
}

/// Prints the load tables of each case of `names`: the components with the largest cost first, the connections and the
/// stages, each in run order.
fn bench_loads(engine: &Engine, names: &[String], out: &mut Out) -> Result<(), CliError> {
    let json = out.json;
    let tables = engine.owner(OwnerRef::Engine, |o| {
        [
            "benchmark.components",
            "benchmark.connections",
            "benchmark.stages",
        ]
        .map(|key| (key, reflect::table_text(o, key, 0, json)))
    })?;
    for name in names {
        let prefix = format!("{name}/");
        for (key, t) in &tables {
            let Some(t) = t else { continue };
            if json {
                for r in t.json.iter().rev() {
                    if r.get(0)
                        .and_then(|k| k.as_str())
                        .is_some_and(|k| k.starts_with(&prefix))
                    {
                        out.json(&json!({ "table": key, "row": r }))?;
                    }
                }
                continue;
            }
            // The rows come newest first: back to run order. The column of the benchmark is left out.
            let mut rows: Vec<Vec<String>> = t
                .rows
                .iter()
                .rev()
                .filter(|r| r.first().is_some_and(|k| k.starts_with(&prefix)))
                .map(|r| {
                    let mut r = r.clone();
                    r[0] = r[0][prefix.len()..].to_owned();
                    r.remove(1);
                    r
                })
                .collect();
            if rows.is_empty() {
                continue;
            }
            let mut headings = t.headings.clone();
            headings.remove(1);
            if *key == "benchmark.components" {
                if let Some(c) = headings.iter().position(|h| h == "Cost") {
                    let cost = |r: &Vec<String>| r[c].parse::<f64>().unwrap_or(0.0);
                    rows.sort_by(|a, b| cost(b).total_cmp(&cost(a)));
                }
            }
            let mut s = format!("\n{name}: {key}\n");
            text::table(&headings, &rows, &mut s);
            out.text(&s)?;
        }
    }
    Ok(())
}

/// One recording of the manifest of the test vectors.
#[derive(Debug, Deserialize)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
    #[serde(default)]
    regenerate: bool,
}

/// The manifest of the test vectors.
#[derive(Debug, Deserialize)]
struct Manifest {
    recordings: Vec<Entry>,
}

/// The SHA-256 of a file, as lower-case hex.
fn sha256_file(p: &Path) -> Result<String, CliError> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let mut h = sdrkit_algo_crypto::hash::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize_reset()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// `vectors list` or `vectors verify`: the state of each recording of the manifest whose path starts with `filter`. With
/// `sums`, it also compares each checksum.
///
/// # Errors
///
/// [`CliError::Io`] or [`CliError::Json`] if the manifest does not read, [`CliError::ChecksFailed`] when `sums` finds a
/// recording missing or wrong.
pub fn vectors(filter: Option<&str>, sums: bool, out: &mut Out) -> Result<(), CliError> {
    let vectors = sdrkit_dev_test::sigmf::vectors_dir();
    let m: Manifest = serde_json::from_slice(&std::fs::read(vectors.join("manifest.json"))?)?;
    let dir = sdrkit_dev_test::recordings_dir();
    let mut rows = Vec::new();
    let mut bad = 0;
    let picked: Vec<&Entry> = m
        .recordings
        .iter()
        .filter(|e| filter.is_none_or(|f| e.path.starts_with(f)))
        .collect();
    for e in &picked {
        let p = dir.join(&e.path);
        let state = match std::fs::metadata(&p) {
            Err(_) => "missing".to_owned(),
            Ok(md) if md.len() != e.size => {
                format!("wrong size ({} bytes, not {})", md.len(), e.size)
            }
            Ok(_) if !sums => "present".to_owned(),
            Ok(_) if sha256_file(&p)? == e.sha256 => "checksum good".to_owned(),
            Ok(_) => "checksum mismatch".to_owned(),
        };
        if !matches!(state.as_str(), "present" | "checksum good") {
            bad += 1;
        }
        if out.json {
            out.json(&json!({ "path": e.path, "size": e.size, "state": state, "regenerable": e.regenerate }))?;
        }
        rows.push(vec![
            e.path.clone(),
            e.size.to_string(),
            state,
            if e.regenerate { "yes" } else { "no" }.to_owned(),
        ]);
    }
    if !out.json {
        let mut s = format!("Recordings folder: {}\n", dir.display());
        text::table(
            &["Path", "Bytes", "State", "Regenerable"].map(String::from),
            &rows,
            &mut s,
        );
        out.text(&s)?;
    }
    if sums && bad > 0 {
        return Err(CliError::ChecksFailed {
            failed: bad,
            total: picked.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::{GenConfig, Generator, MakeGenerator, MemStorage};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, OnceLock};

    /// The makes of the test signal of each counted stack, and the generator that each count wraps.
    static MAKES: [AtomicU64; 2] = [AtomicU64::new(0), AtomicU64::new(0)];
    static MAKERS: [OnceLock<MakeGenerator>; 2] = [OnceLock::new(), OnceLock::new()];

    fn counted<const N: usize>(
        cfg: &GenConfig,
    ) -> Result<Box<dyn Generator>, sdrkit_core::ConfigError> {
        MAKES[N].fetch_add(1, Ordering::SeqCst);
        (MAKERS[N].get().expect("a counted generator"))(cfg)
    }

    /// Stack `id` with its generator counted in [`MAKES`] at `N`.
    fn count<const N: usize>(id: &str) -> &'static StackDescriptor {
        let d = live::stack(id).unwrap();
        let _ = MAKERS[N].set(d.make_generator.unwrap());
        Box::leak(Box::new(StackDescriptor {
            id: d.id,
            name: d.name,
            group: d.group,
            blurb: d.blurb,
            tuning: d.tuning.clone(),
            submodes: d.submodes,
            settings: d.settings,
            make_receiver: d.make_receiver,
            make_generator: Some(counted::<N>),
            lock_profile: d.lock_profile,
            signal_types: d.signal_types,
            auto_gain: d.auto_gain,
            verify: d.verify,
            benchmarks: d.benchmarks,
        }))
    }

    /// Each case of the verify tool for fm and loran makes the test signal once: the open call sets every setting of
    /// the signal, and the start of the session changes none of them.
    #[test]
    fn a_case_makes_its_test_signal_once() {
        let counted = [count::<0>("fm"), count::<1>("loran")];
        let facilities = sdrkit_core::Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: sdrkit_host_engine::system_clock(),
            random: sdrkit_host_engine::system_random(),
            codecs: Arc::new(sdrkit_host_catalog::codecs()),
        };
        let components = sdrkit_host_catalog::registry(&facilities);
        let stacks = sdrkit_host_catalog::all()
            .into_iter()
            .map(|d| *counted.iter().find(|c| c.id == d.id).unwrap_or(&d))
            .collect();
        let engine = Engine::new(sdrkit_host_engine::Setup {
            facilities,
            transport: Arc::new(sdrkit_host_engine::StdNet),
            stacks,
            families: sdrkit_host_catalog::families(),
            components,
        })
        .unwrap();
        let o = VerifyOptions {
            seconds: 1.0,
            clean: true,
            snr_db: 30.0,
        };
        let mut seen = Vec::new();
        for (n, d) in counted.iter().enumerate() {
            for case in cases(d, &o) {
                let before = MAKES[n].load(Ordering::SeqCst);
                run_case(&engine, d, &case).unwrap();
                seen.push((d.id, case.name.clone(), MAKES[n].load(Ordering::SeqCst) - before));
            }
        }
        assert!(seen.iter().all(|(_, _, n)| *n == 1), "makes per case: {seen:?}");
    }

    /// A case reads its full length of signal at the rate that the radio gives, not at the stack's rate: here a session
    /// of another stack reads the radio first, so the radio keeps the rate of that stack.
    #[test]
    fn a_case_reads_its_length_at_the_input_rate() {
        let engine = crate::host::engine(std::sync::Arc::new(MemStorage::new())).unwrap();
        let d = live::stack("ptt_log").unwrap();
        assert_eq!(d.tuning.default_hz, 0.0);
        let o = VerifyOptions {
            seconds: 1.0,
            clean: true,
            snr_db: 30.0,
        };
        let case = VerifyCase::new("test signal", &o, 0.0);
        let values = Values::defaults(&(d.settings)());
        let radio = open_signal(&engine, d, &case, &values, 0.0).unwrap();
        let first = live::stack("fm").unwrap();
        assert_ne!(first.tuning.rate_hz, d.tuning.rate_hz);
        let held = Values::defaults(&(first.settings)());
        let holder = engine
            .start_stack(first.id, None, &held, Input::Radio(radio), Some(0.0))
            .unwrap();
        let id = engine
            .start_stack(d.id, None, &values, Input::Radio(radio), Some(0.0))
            .unwrap();
        let t0 = Instant::now();
        let seen = watch(&engine, id, radio, &case, d.tuning.rate_hz);
        let wall_s = t0.elapsed().as_secs_f64();
        let samples = engine
            .owner(OwnerRef::Session(id), |o| {
                o.results().fields().get(&SAMPLES).unwrap_or(0)
            })
            .unwrap();
        let rate = engine
            .owner(OwnerRef::Radio(radio), |o| {
                o.results().fields().get(&RATE_GIVEN).unwrap_or(0.0)
            })
            .unwrap();
        engine.stop_session(holder).unwrap();
        live::end(
            &engine,
            &crate::host::Live {
                radio: Some(radio),
                session: id,
                stack: Some(d.id),
            },
        )
        .unwrap();
        let seen = seen.unwrap();
        assert!(rate > 0.0 && rate != d.tuning.rate_hz, "radio rate {rate}");
        let read_s = samples as f64 / rate;
        assert!(
            read_s >= case.seconds,
            "read {read_s:.3} s of a {} s case",
            case.seconds
        );
        assert!(
            seen.signal_s >= case.seconds,
            "counted {:.3} s",
            seen.signal_s
        );
        // The test signal runs in real time: half the case's length bounds the radio's first buffer.
        assert!(
            wall_s >= 0.5 * case.seconds,
            "a {} s case ran {wall_s:.3} s",
            case.seconds
        );
    }
}
