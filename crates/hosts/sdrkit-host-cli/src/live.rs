//! Running a stack or a graph on a source: the radio and the session, the live loop with its status line, its changes,
//! its events, its sound and its console, and the spectrum and tap streams.

use crate::error::CliError;
use crate::host::{self, Live, OUTSIDE};
use crate::out::Out;
use crate::reflect;
use crate::sound::{Sound, SoundTo};
use crate::text::{self, Bare};
use sdrkit_core::{Owner, Record, StackDescriptor, Value, Values};
use sdrkit_host_engine::{Engine, Input, OwnerRef, SessionState, TapId, DROPPED, SAMPLES, STATE};
use serde_json::json;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How often the live loop reads the session.
const TICK: Duration = Duration::from_millis(100);

/// Where the samples come from, read from `<driver>:<address>`.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    /// The driver key.
    pub driver: String,
    /// The address the driver opens.
    pub address: String,
    /// A folder of the computer that the storage must show at `outside/`, for a file outside the storage.
    pub mount: Option<PathBuf>,
}

impl Source {
    /// Reads `text`: `synth` (the test signal of stack `stack`), `synth:<stack>`, `file:<path>`, or `<driver>:<address>`.
    pub fn parse(text: &str, stack: &str) -> Source {
        let (driver, rest) = sdrkit_host_engine::split_address(text);
        let mut s = Source {
            driver: driver.to_owned(),
            address: rest.to_owned(),
            mount: None,
        };
        match driver {
            "synth" if rest.is_empty() => s.address = format!("synth:{stack}"),
            "synth" => s.address = format!("synth:{rest}"),
            "file" => {
                let path = PathBuf::from(rest);
                if let (Some(dir), Some(name)) = (path.parent(), path.file_name()) {
                    let dir = if dir.as_os_str().is_empty() {
                        PathBuf::from(".")
                    } else {
                        dir.to_owned()
                    };
                    s.mount = Some(dir);
                    s.address = format!("{OUTSIDE}/{}", name.to_string_lossy());
                }
            }
            _ => {}
        }
        s
    }
}

/// What a live command requires from its radio and its stack.
#[derive(Clone, Debug, Default)]
pub struct Tune {
    /// The frequency, as typed.
    pub freq: Option<String>,
    /// The sample rate, as typed.
    pub rate: Option<String>,
    /// The gain in dB. Without it the radio's automatic gain runs.
    pub gain: Option<f64>,
    /// `key=value` settings of the stack or the radio (`radio.<key>` or `stack.<key>` to say which).
    pub set: Vec<String>,
    /// The variant of the stack.
    pub submode: Option<String>,
}

/// The region data of a live command: the data folder, and the region that `--region` and `--city` name.
#[derive(Debug)]
pub struct Regional {
    /// The open data folder.
    pub data: sdrkit_presets::Data,
    /// The country and the city.
    pub region: sdrkit_presets::Region,
}

/// The descriptor of stack `id`.
///
/// # Errors
///
/// [`CliError::UnknownStack`].
pub fn stack(id: &str) -> Result<&'static StackDescriptor, CliError> {
    sdrkit_host_catalog::all()
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| CliError::UnknownStack { id: id.to_owned() })
}

/// Writes `value` into setting `key` of `values`.
fn put(values: &mut Values, key: &str, value: &Value) -> Result<bool, CliError> {
    match values.schema().id(key) {
        Ok(id) => {
            sdrkit_host_engine::EngineError::set(values, id, value)?;
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

/// Reads `key=value` into the values of the stack or of the radio, by the setting's kind. Returns the key if the stack
/// took it.
fn assign<'a>(
    stack: &mut Values,
    radio: &mut Values,
    text: &'a str,
    who: &str,
) -> Result<Option<&'a str>, CliError> {
    let (k, v) = text::key_value(text)?;
    let (target, key, of_stack) = match (k.strip_prefix("radio."), k.strip_prefix("stack.")) {
        (Some(key), _) => (Some(&mut *radio), key, false),
        (_, Some(key)) => (Some(&mut *stack), key, true),
        _ if stack.schema().id(k).is_ok() => (Some(&mut *stack), k, true),
        _ if radio.schema().id(k).is_ok() => (Some(&mut *radio), k, false),
        _ => (None, k, false),
    };
    let unknown = || CliError::UnknownSetting {
        owner: who.to_owned(),
        key: key.to_owned(),
    };
    let target = target.ok_or_else(unknown)?;
    let id = target.schema().id(key).map_err(|_| unknown())?;
    let decl = target
        .schema()
        .setting(id)
        .map(|s| s.field.clone())
        .ok_or_else(unknown)?;
    let value = text::parse_value(&decl, v)?;
    put(target, key, &value)?;
    Ok(of_stack.then_some(key))
}

/// Opens the radio of `source` and starts stack `desc` on it with the settings of `tune`. The engine tunes the session to
/// the frequency of `tune` or to the stack's default. The radio opens with the rate in `tune`; without one the rate stays
/// on Automatic and follows the stack.
///
/// With `regional`, the stack starts from the preset that the region data has for it: the settings and the frequency of
/// the region, each replaced by a setting or a frequency of `tune`. A note on `out` names the frequency. A stack that
/// the region data has nothing for starts as without it, and so does a stack that does not know a value of the data,
/// with a note.
///
/// # Errors
///
/// [`CliError::UnknownDriver`], [`CliError::UnknownSetting`], [`CliError::BadNumber`], [`CliError::Preset`] for region
/// data that cannot be read, the errors of the engine.
pub fn start(
    engine: &Engine,
    source: &Source,
    desc: &'static StackDescriptor,
    tune: &Tune,
    regional: Option<&Regional>,
    out: &mut Out,
) -> Result<Live, CliError> {
    let kind = engine
        .find(&format!("driver.{}", source.driver))
        .map_err(|_| CliError::UnknownDriver {
            key: source.driver.clone(),
        })?;
    let mut radio = host::defaults(engine, kind)?;
    let mut values = Values::defaults(&(desc.settings)());
    let freq = tune
        .freq
        .as_deref()
        .map(|f| text::parse_hz(f, Bare::Frequency))
        .transpose()?;
    let rate = tune
        .rate
        .as_deref()
        .map(|r| text::parse_hz(r, Bare::Hz))
        .transpose()?;
    if let Some(g) = tune.gain {
        let who = format!("the driver `{}`", source.driver);
        let missing = || CliError::UnknownSetting {
            owner: who.clone(),
            key: "gain".into(),
        };
        if !put(&mut radio, "gain", &Value::F64(g))? {
            return Err(missing());
        }
        put(&mut radio, "agc", &Value::Bool(false))?;
    }
    // A rate that the user gives is a rate that the host sets: the radio opens with it, and a stack that needs more
    // refuses the start. A source with a mount keeps the rate of its file.
    if let (Some(r), None) = (rate, &source.mount) {
        if !put(&mut radio, sdrkit_driver_core::RATE.field.key, &Value::Hz(r))? {
            return Err(CliError::UnknownSetting {
                owner: format!("the driver `{}`", source.driver),
                key: sdrkit_driver_core::RATE.field.key.into(),
            });
        }
    }
    let who = format!("the stack `{}` or the driver `{}`", desc.id, source.driver);
    // A setting that only the open radio has (those of its transmitter) is set after the open.
    let mut late = Vec::new();
    let mut own = Vec::new();
    for s in &tune.set {
        match assign(&mut values, &mut radio, s, &who) {
            Err(CliError::UnknownSetting { .. }) => late.push(s.as_str()),
            r => own.extend(r?),
        }
    }
    // A recording keeps its own frequency.
    let preset = match regional.filter(|_| source.driver != "file") {
        Some(r) => {
            let options = sdrkit_presets::Options {
                frequency_hz: freq,
                submode: tune.submode.clone(),
                settings: own
                    .iter()
                    .filter_map(|key| {
                        let id = values.schema().id(key).ok()?;
                        Some(((*key).to_owned(), values.value(id)?.clone()))
                    })
                    .collect(),
            };
            match r.data.preset(desc.id, &r.region, &options) {
                Ok(p) => Some(p),
                Err(sdrkit_presets::PresetError::NoData { .. }) => None,
                Err(e) => return Err(sdrkit_presets::StartError::from(e).into()),
            }
        }
        None => None,
    };
    if let (Some(p), Some(r)) = (&preset, regional) {
        let mut note = format!(
            "region data {}: {} MHz",
            r.region.of_stack(desc.id),
            p.frequency_hz / 1e6
        );
        if let Some(w) = p.width_hz {
            note.push_str(&format!(", {} MHz wide", w / 1e6));
        }
        if let Some(s) = &p.station {
            note.push_str(&format!(", station {}", s.network));
        }
        out.note(&note)?;
    }
    let radio_id = engine
        .open_radio(&source.driver, &source.address, &radio)
        .map_err(|e| CliError::of_open(engine, &source.driver, e))?;
    for s in late {
        let (k, v) = text::key_value(s)?;
        let key = k.strip_prefix("radio.").unwrap_or(k);
        if let Err(e) = engine.set_text(OwnerRef::Radio(radio_id), key, v) {
            let _ = engine.close_radio(radio_id);
            return Err(e.into());
        }
    }
    let plain = || {
        engine
            .start_stack(
                desc.id,
                tune.submode.as_deref(),
                &values,
                Input::Radio(radio_id),
                freq,
            )
            .map_err(CliError::from)
    };
    let started = match &preset {
        Some(p) => match sdrkit_presets::start_preset(engine, radio_id, p) {
            // The data has a value that this stack does not know: nothing started yet.
            Err(e @ sdrkit_presets::StartError::Setting { .. }) => out
                .note(&format!("{e}: started without the region data"))
                .and_then(|()| plain()),
            r => r.map_err(CliError::from),
        },
        None => plain(),
    };
    let session = match started {
        Ok(s) => s,
        Err(e) => {
            let _ = engine.close_radio(radio_id);
            return Err(e);
        }
    };
    Ok(Live {
        radio: Some(radio_id),
        session,
        stack: Some(desc.id),
    })
}

/// Stops and closes the session and the radio of `live`.
///
/// # Errors
///
/// What the engine returns.
pub fn end(engine: &Engine, live: &Live) -> Result<(), CliError> {
    engine.stop_session(live.session)?;
    let t0 = Instant::now();
    loop {
        match engine.close_session(live.session) {
            Err(sdrkit_host_engine::EngineError::StillRunning)
                if t0.elapsed() < Duration::from_secs(5) =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            r => break r?,
        }
    }
    if let Some(r) = live.radio {
        engine.close_radio(r)?;
    }
    Ok(())
}

/// How the live loop ends and where its sound goes.
#[derive(Clone, Debug)]
pub struct Watch {
    /// Stop after this many seconds.
    pub seconds: Option<f64>,
    /// Where the sound goes.
    pub sound: SoundTo,
    /// The source is a recording: the loop ends when the session ends at the end of the recording, and the status says
    /// how fast the stack plays it.
    pub to_the_end: bool,
    /// Print every changed field and table row. Without it, only the status line, the log and the console output.
    pub verbose: bool,
    /// The audio that the session gets as its microphone while it asks for host audio.
    pub tx_audio: Option<Arc<crate::txaudio::TxAudio>>,
    /// The folder that the pictures of the stack are saved in, and the seconds of signal between two saved video frames.
    pub pictures: Option<(PathBuf, f64)>,
}

/// What the live loop remembers between two reads of the session: the change counters it printed.
#[derive(Default)]
struct Seen {
    results: u64,
    fields: u64,
    tables: Vec<u64>,
    last_fields: Option<Record>,
    field_texts: Vec<String>,
    last_log: Option<String>,
    status: String,
}

/// What one read of the session returned.
struct Changes {
    status: String,
    status_json: serde_json::Value,
    lines: Vec<String>,
    json: Vec<serde_json::Value>,
    state: Option<SessionState>,
    /// The samples that the session took.
    samples: u64,
}

/// Reads what changed in the session's results since `seen`, as text lines or JSON values.
fn changes(o: &dyn Owner, seen: &mut Seen, json: bool, verbose: bool, now: f64) -> Changes {
    let r = o.results();
    let st = r.status();
    let mut status = format!("{:?}", st.lock_state);
    if let Some(v) = st.snr_db {
        let _ = write!(status, "  SNR {v:.1} dB");
    }
    if let Some(v) = st.level_dbfs {
        let _ = write!(status, "  level {v:.1} dBFS");
    }
    if let Some(v) = st.offset_hz {
        let _ = write!(status, "  offset {}", text::hz(v));
    }
    let samples = r.fields().get(&SAMPLES).unwrap_or(0);
    let taken = u64::try_from(samples).unwrap_or(0);
    let dropped = r.fields().get(&DROPPED).unwrap_or(0);
    let _ = write!(status, "  samples {samples}  lost {dropped}");
    if !st.summary.is_empty() {
        let _ = write!(status, "  {}", st.summary);
    }
    let mut c = Changes {
        status,
        status_json: json!({ "status": st, "samples": samples, "lost": dropped }),
        lines: Vec::new(),
        json: Vec::new(),
        state: r.fields().get(&STATE),
        samples: taken,
    };
    if r.change_counter() == seen.results {
        return c;
    }
    if r.fields_counter() > seen.fields {
        let mut changed = serde_json::Map::new();
        for (i, (d, v)) in r.fields().iter().enumerate() {
            let id = sdrkit_core::FieldId(u16::try_from(i).unwrap_or(u16::MAX));
            let before = seen.last_fields.as_ref().and_then(|l| l.value(id));
            if v.is_none() || before == Some(v) || d.key.starts_with("session.") {
                continue;
            }
            if json {
                changed.insert(
                    d.key.to_string(),
                    serde_json::to_value(v).unwrap_or_default(),
                );
                continue;
            }
            let t = text::cell(d, v, now);
            if seen.field_texts.len() <= i {
                seen.field_texts.resize(i + 1, String::new());
            }
            if seen.field_texts[i] != t {
                if verbose {
                    c.lines.push(format!("{}: {t}", d.label));
                }
                seen.field_texts[i] = t;
            }
        }
        if !changed.is_empty() {
            c.json.push(json!({ "fields": changed }));
        }
        match &mut seen.last_fields {
            Some(l) => l.clone_from(r.fields()),
            None => seen.last_fields = Some(r.fields().clone()),
        }
        seen.fields = r.fields_counter();
    }
    let decl = r.decl();
    seen.tables.resize(decl.tables().len(), 0);
    let changed: Vec<_> = r.tables_changed_since(seen.results).collect();
    for id in changed {
        let i = usize::from(id.0);
        let (Some(t), Some(since)) = (decl.table(id), seen.tables.get(i).copied()) else {
            continue;
        };
        let Some(tt) = reflect::table_text(o, &t.key, since, json) else {
            continue;
        };
        if json {
            if !tt.json.is_empty() {
                c.json.push(json!({ "table": t.key, "rows": tt.json }));
            }
        } else if verbose {
            for row in &tt.rows {
                let mut l = format!("{} {}:", t.label, row[0]);
                for (h, v) in tt
                    .headings
                    .iter()
                    .zip(row)
                    .skip(1)
                    .filter(|(_, v)| !v.is_empty())
                {
                    let _ = write!(l, " {h} {v},");
                }
                c.lines.push(l.trim_end_matches(',').to_owned());
            }
        }
        seen.tables[i] = tt.counter;
    }
    let log = r.log_lines();
    let from = seen
        .last_log
        .as_ref()
        .and_then(|last| log.iter().rposition(|l| l == last))
        .map_or(0, |p| p + 1);
    for l in log.iter().skip(from) {
        if json {
            c.json.push(json!({ "log": l }));
        } else {
            c.lines.push(format!("| {l}"));
        }
    }
    if let Some(l) = log.back() {
        seen.last_log = Some(l.clone());
    }
    seen.results = r.change_counter();
    c
}

/// The rate of the samples of radio `r`, 0 if it has none.
fn radio_rate(engine: &Engine, r: sdrkit_host_engine::RadioId) -> f64 {
    engine
        .owner(OwnerRef::Radio(r), |o| {
            o.results().fields().get(&sdrkit_driver_core::RATE_GIVEN)
        })
        .ok()
        .flatten()
        .unwrap_or(0.0)
}

/// How fast a stack plays a recording: `samples` at `rate_hz` taken in `elapsed_s`. `None` near real time, and in the
/// first two seconds.
fn speed_note(samples: u64, rate_hz: f64, elapsed_s: f64) -> Option<String> {
    if rate_hz <= 0.0 || elapsed_s < 2.0 {
        return None;
    }
    let x = samples as f64 / rate_hz / elapsed_s;
    ((x - 1.0).abs() > 0.05).then(|| format!("the stack runs at {x:.1} times real time on this file"))
}

/// The words of a console line: split at white space, except inside double quotes, which are removed
/// (`text="CQ DE VK2ABC"` is one word).
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut any) = (false, false);
    for c in line.chars() {
        match c {
            '"' => (quoted, any) = (!quoted, true),
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut word));
                    any = false;
                }
            }
            c => {
                word.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(word);
    }
    out
}

/// Runs one console line of `run`: `call <owner> <command> [key=value...]`, `set [owner] key=value...`,
/// `show [owner] [table]`, `events <owner>`.
fn console(engine: &Engine, live: &Live, line: &str, out: &mut Out) -> Result<(), CliError> {
    let words = words(line);
    let never = || false;
    match words.first().map(String::as_str) {
        None => Ok(()),
        Some("call") => match words.get(1) {
            Some(owner) => reflect::call(engine, Some(live), owner, words.get(2).map(String::as_str), words.get(3..).unwrap_or(&[]), out),
            None => out.note("call <owner> <command> [key=value...]"),
        },
        Some("set") => {
            let (name, rest) = match words.get(1) {
                Some(w) if !w.contains('=') => (w.as_str(), &words[2..]),
                _ => ("session", &words[1..]),
            };
            let r = host::owner(engine, name, Some(live))?;
            reflect::apply(engine, r, name, rest)
        }
        Some("show") => {
            let (owner, table) = match (words.get(1), words.get(2)) {
                (Some(o), Some(t)) => (o.as_str(), Some(t.as_str())),
                (Some(t), None) if host::owner(engine, t, Some(live)).is_err() => ("session", Some(t.as_str())),
                (Some(o), None) => (o.as_str(), None),
                (None, _) => ("session", None),
            };
            reflect::show(engine, Some(live), owner, table, 0, None, out)
        }
        Some("events") => {
            let name = words.get(1).map_or("session", String::as_str);
            reflect::events(engine, Some(live), name, false, out, &never)
        }
        Some(_) => out.note(
            "type: call <owner> <command> [key=value...] | set [owner] key=value... | show [owner] [table] | events [owner]",
        ),
    }
}

/// The live loop of `run` and `play`: prints the status line, the changed fields and table rows, the log and the events,
/// moves the sound, and runs the console lines, until `stop` returns true, the time is up or the session ends.
///
/// # Errors
///
/// What the engine and the sound output give. A console line that fails prints its error and the loop goes on.
pub fn watch(
    engine: &Arc<Engine>,
    live: &Live,
    w: &Watch,
    out: &mut Out,
    lines: Option<&Receiver<String>>,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    let t0 = Instant::now();
    let mut seen = Seen::default();
    let mut sound = Sound::new(&w.sound);
    // The WAV file that a change of the format of the sound opened, for the status line.
    let mut sound_file: Option<String> = None;
    let mut buf = vec![0.0f32; 1 << 14];
    let session = OwnerRef::Session(live.session);
    let mut saver = w
        .pictures
        .as_ref()
        .map(|(folder, every)| {
            crate::pictures::SavePictures::new(folder.clone(), *every, live.stack.unwrap_or("graph"))
        })
        .transpose()?;
    let rate = live.radio.map_or(0.0, |r| radio_rate(engine, r));
    let mut started: Option<Instant> = None;
    // Stops when the loop ends.
    let _feeder = w
        .tx_audio
        .as_ref()
        .map(|a| crate::txaudio::Feeder::start(engine, live.session, Arc::clone(a)))
        .transpose()?;
    loop {
        let json = out.json;
        while let Some(line) = lines.and_then(|l| l.try_recv().ok()) {
            if !json {
                out.line("")?;
            }
            if let Err(e) = console(engine, live, &line, out) {
                out.note(&format!("error: {e}"))?;
            }
            if !json {
                out.line("")?;
            }
        }
        let now = reflect::now();
        let c = engine.owner(session, |o| changes(o, &mut seen, json, w.verbose, now))?;
        for l in &c.lines {
            out.line(l)?;
        }
        for v in &c.json {
            out.json(v)?;
        }
        reflect::print_events(engine, session, "session", out)?;
        if let Some(s) = saver.as_mut() {
            s.pump(engine, live.session, out)?;
        }
        if let Some(p) = sound.pump(engine, live.session, &mut buf)? {
            let note = format!("the sound changed its format: now {}", p.display());
            if json {
                out.note(&note)?;
            }
            sound_file = Some(note);
        }
        let mut status = match &sound_file {
            Some(note) => format!("{}  ({note})", c.status),
            None => c.status,
        };
        if w.to_the_end && c.samples > 0 {
            let t0 = *started.get_or_insert_with(Instant::now);
            if let Some(note) = speed_note(c.samples, rate, t0.elapsed().as_secs_f64()) {
                let _ = write!(status, "  {note}");
            }
        }
        if status != seen.status {
            if json {
                out.json(&c.status_json)?;
            } else {
                out.status(&status)?;
            }
            seen.status = status;
        }
        let over = w.seconds.is_some_and(|s| t0.elapsed().as_secs_f64() >= s);
        // At the end of a recording the session ends by itself once its stack has taken every sample.
        let ended = matches!(c.state, Some(SessionState::Failed | SessionState::Stopped));
        if ended {
            // The last events and pictures reach the host after one more pass of the engine.
            engine.settle(Duration::from_secs(5));
            let c = engine.owner(session, |o| changes(o, &mut seen, json, w.verbose, now))?;
            for l in &c.lines {
                out.line(l)?;
            }
            for v in &c.json {
                out.json(v)?;
            }
            reflect::print_events(engine, session, "session", out)?;
            if let Some(s) = saver.as_mut() {
                s.pump(engine, live.session, out)?;
            }
            sound.pump(engine, live.session, &mut buf)?;
        }
        if over || ended || stop() {
            break;
        }
        std::thread::sleep(TICK);
    }
    out.done()?;
    sound.finish()
}

/// Waits until the record service has no recording that runs, then prints the line that ended the last one: its length
/// and the samples that were missing. A recording runs until it holds its seconds of samples, however long the stack
/// takes for them.
///
/// # Errors
///
/// What the engine returns.
pub fn wait_recording(engine: &Engine, stop: &dyn Fn() -> bool, out: &mut Out) -> Result<(), CliError> {
    let service = engine.find("service.record")?;
    let running = |o: &mut dyn Owner| {
        let r = o.results();
        r.decl()
            .table_id("running")
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.rows_newest_first().count())
    };
    let t0 = Instant::now();
    let mut seen = false;
    loop {
        let n = engine.owner(service, running)?;
        seen |= n > 0;
        if n == 0 && (seen || t0.elapsed() > Duration::from_secs(5)) || stop() {
            break;
        }
        std::thread::sleep(TICK);
    }
    engine.settle(Duration::from_secs(5));
    let line = engine.owner(service, |o| {
        o.results()
            .log_lines()
            .iter()
            .rev()
            .find(|l| l.starts_with("recorded "))
            .cloned()
    })?;
    if let Some(l) = line {
        out.note(&l)?;
    }
    Ok(())
}

/// Turns on the session's spectrum and draws each frame as one line of a waterfall, `width` characters wide, until
/// `stop` returns true or the time is up.
///
/// # Errors
///
/// What the engine returns.
pub fn spectrum(
    engine: &Engine,
    live: &Live,
    width: usize,
    seconds: Option<f64>,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    const SHADES: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
    let s = OwnerRef::Session(live.session);
    reflect::apply(engine, s, "session", &["spectrum=yes".to_owned()])?;
    let t0 = Instant::now();
    let mut bins = Vec::new();
    let mut header = false;
    while !stop() && seconds.is_none_or(|x| t0.elapsed().as_secs_f64() < x) {
        let Some(f) = engine.spectrum(live.session, &mut bins)? else {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        };
        if out.json {
            out.json(&json!({ "centre_hz": f.centre_hz, "span_hz": f.span_hz, "bins": bins }))?;
            continue;
        }
        if !header {
            let lo = text::hz(f.centre_hz - f.span_hz / 2.0);
            let hi = text::hz(f.centre_hz + f.span_hz / 2.0);
            let pad = width.saturating_sub(lo.len() + hi.len());
            out.line(&format!("{lo}{}{hi}", " ".repeat(pad)))?;
            header = true;
        }
        let w = width.max(1);
        let per = bins.len().div_ceil(w).max(1);
        let cols: Vec<f32> = bins
            .chunks(per)
            .map(|c| c.iter().copied().fold(f32::MIN, f32::max))
            .collect();
        let floor = cols.iter().copied().fold(f32::MAX, f32::min);
        let top = cols.iter().copied().fold(f32::MIN, f32::max);
        let span = (top - floor).max(1e-6);
        let line: String = cols
            .iter()
            .map(|v| {
                let x = ((v - floor) / span * (SHADES.len() - 1) as f32).round();
                SHADES[(x.max(0.0) as usize).min(SHADES.len() - 1)]
            })
            .collect();
        out.line(&line)?;
    }
    Ok(())
}

/// Lets the session run for `seconds` (5 without it), or until `stop` returns true, then prints its load: the table
/// `session.components` with the largest share first, under it the table `session.connections` with the queue of the
/// session first, the table `session.stages` in run order, and the totals of the session.
///
/// # Errors
///
/// What the engine returns.
pub fn profile(
    engine: &Engine,
    live: &Live,
    seconds: Option<f64>,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    let t0 = Instant::now();
    let seconds = seconds.unwrap_or(5.0);
    while !stop() && t0.elapsed().as_secs_f64() < seconds {
        std::thread::sleep(Duration::from_millis(50));
    }
    let s = OwnerRef::Session(live.session);
    let as_json = out.json;
    let tables = engine.owner(s, |o| {
        let table = |key| reflect::table_text(o, key, 0, as_json);
        let f = o.results().fields();
        let field = |key: &str| {
            let id = o.results().decl().field_id(key)?;
            match f.value(id)? {
                Value::F64(x) => Some(format!("{x:.3}")),
                Value::I64(n) => Some(n.to_string()),
                Value::Text(t) => Some(t.to_string()),
                _ => None,
            }
        };
        let totals: Vec<(&str, Option<String>)> = [
            "session.samples",
            "session.share",
            "session.core_units",
            "session.cores_needed",
            "session.cores_declared",
            "session.cores_free",
            "session.threads",
            "session.dropped",
            "session.gaps",
            "session.stage_behind",
            "session.load_problem",
        ]
        .into_iter()
        .map(|k| (k, field(k)))
        .collect();
        (
            table("session.components"),
            table("session.connections"),
            table("session.stages"),
            totals,
        )
    })?;
    let (components, connections, stages, totals) = tables;
    if out.json {
        let rows = |t: Option<reflect::TableText>| t.map(|t| t.json).unwrap_or_default();
        let totals: serde_json::Map<String, serde_json::Value> = totals
            .into_iter()
            .filter_map(|(k, v)| Some((k.to_owned(), json!(v?))))
            .collect();
        return out.json(&json!({
            "components": rows(components),
            "connections": rows(connections),
            "stages": rows(stages),
            "session": totals,
        }));
    }
    let mut print = |t: Option<reflect::TableText>, by_share: bool| -> Result<(), CliError> {
        let Some(mut t) = t else { return Ok(()) };
        // The rows come newest first: back to run order, or the largest share first.
        t.rows.reverse();
        if let Some(c) = t
            .headings
            .iter()
            .position(|h| h == "Share")
            .filter(|_| by_share)
        {
            let share = |r: &Vec<String>| r[c].parse::<f64>().unwrap_or(0.0);
            t.rows.sort_by(|a, b| share(b).total_cmp(&share(a)));
        }
        let mut text = String::new();
        text::table(&t.headings, &t.rows, &mut text);
        out.line(&text)
    };
    print(components, true)?;
    print(connections, false)?;
    print(stages, false)?;
    for (key, value) in totals {
        if let Some(v) = value {
            out.line(&format!("{key}: {v}"))?;
        }
    }
    Ok(())
}

/// Opens a tap on `port` of the session's graph and prints the level of each piece, until `stop` returns true or the time
/// is up.
///
/// # Errors
///
/// [`CliError::Engine`] with `NoGraph` if the session does not run a graph, what the engine returns.
pub fn tap(
    engine: &Engine,
    live: &Live,
    port: &str,
    seconds: Option<f64>,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    let tap: TapId = engine.open_tap(live.session, port, 16)?;
    let t0 = Instant::now();
    let mut buf = vec![0.0f32; 1 << 15];
    while !stop() && seconds.is_none_or(|x| t0.elapsed().as_secs_f64() < x) {
        let Some(r) = engine.read_tap(live.session, tap, &mut buf)? else {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        };
        let v = &buf[..r.values];
        let power = v.iter().map(|x| x * x).sum::<f32>() / (v.len().max(1) as f32);
        let db = 10.0 * power.max(1e-20).log10();
        if out.json {
            out.json(&json!({ "port": port, "values": r.values, "level_db": db, "rate_hz": r.rate_hz, "dropped": r.dropped }))?;
        } else {
            out.line(&format!(
                "{port}: {:?}, {} values at {}, level {db:.1} dB, dropped {}",
                r.kind,
                r.values,
                text::hz(r.rate_hz),
                r.dropped
            ))?;
        }
    }
    engine.close_tap(live.session, tap)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::words;

    #[test]
    fn a_quoted_value_is_one_word() {
        assert_eq!(
            words(r#"call session send text="CQ CQ DE VK2ABC"  x=1"#),
            ["call", "session", "send", "text=CQ CQ DE VK2ABC", "x=1"]
        );
        assert_eq!(words(r#"set a="""#), ["set", "a="]);
        assert!(words("   ").is_empty());
    }
}
