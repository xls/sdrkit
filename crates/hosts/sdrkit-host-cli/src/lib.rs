//! `sdrkit`, the console app of the engine: it lists the catalogs, reads and commands every owner through its
//! declarations, runs stacks on any source with their sound, and tests the stacks.

#![warn(missing_docs)]

pub mod checks;
pub mod error;
pub mod host;
pub mod live;
pub mod out;
pub mod reflect;
pub mod sound;
pub mod text;
pub mod txaudio;

pub use error::CliError;

use clap::{Args, Parser, Subcommand};
use host::Mounted;
use live::{Regional, Source, Tune, Watch};
use out::Out;
use sdrkit_core::Storage;
use sdrkit_host_engine::{DirStorage, Engine, EngineError, OwnerRef};
use sound::SoundTo;
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;

/// The command line.
#[derive(Debug, Parser)]
#[command(
    name = "sdrkit",
    version,
    about = "Run, read, command and test the stacks of sdrkit from a terminal."
)]
pub struct Cli {
    /// Print one JSON value per line instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// The folder where the engine keeps its files and settings (default: ~/.sdrkit).
    #[arg(long, global = true)]
    storage: Option<PathBuf>,
    /// The country (ISO 3166 code: AU, US, DE ...) whose rows of the data folder the tool uses. A code that the data
    /// has no plans for is refused, with the list of the regions. The tool reads the rows and sets plain values: the settings and the frequency of a stack, the channels of a scan, the frequency
    /// entries. The engine has no region, and the country is not kept for the next run.
    #[arg(long, global = true)]
    region: Option<String>,
    /// The city of the country. A stack then starts on the strongest known station of the city.
    #[arg(long, global = true)]
    city: Option<String>,
    /// The data folder: plans, known stations, cities, frequency entries. Without it, the first of these that has plans:
    /// `data` in the storage folder, `~/.sdrkit/data`, `data` or `share/sdrkit/data` in a folder above the program,
    /// `data` in this folder.
    /// With `--region <country>`, a stack starts from the preset that this data has for it.
    #[arg(long, global = true, value_name = "FOLDER")]
    data: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

/// Where the samples come from and how a stack is tuned.
#[derive(Debug, Args, Clone)]
struct TuneArgs {
    /// The source, as <driver>:<address>: synth (the stack's test signal), file:<path>, rtl_tcp:<host:port>,
    /// iiod:<host>, or any driver that `sdrkit drivers` lists.
    #[arg(long, default_value = "synth")]
    source: String,
    /// The frequency: 919.875M, 919.875MHz, 919875000, 100.3 (a number below 100000 is MHz). Default: the stack's own.
    #[arg(long)]
    freq: Option<String>,
    /// The sample rate: 2.4M, 250k, 2400000. Default: the stack's own.
    #[arg(long)]
    rate: Option<String>,
    /// The gain in dB. Default: the radio's automatic gain.
    #[arg(long)]
    gain: Option<f64>,
    /// A setting of the stack or the radio, as key=value (radio.<key> or stack.<key> to say which). Repeat it.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    set: Vec<String>,
    /// The variant of the stack.
    #[arg(long)]
    submode: Option<String>,
    /// Stop after this many seconds.
    #[arg(long)]
    seconds: Option<f64>,
    /// Print every changed result field and table row as it changes. Without it, `run` prints the status line, the
    /// log and the output of console commands.
    #[arg(long)]
    verbose: bool,
}

impl TuneArgs {
    fn tune(&self, more: &[String]) -> Tune {
        let mut set = self.set.clone();
        set.extend(more.iter().cloned());
        Tune {
            freq: self.freq.clone(),
            rate: self.rate.clone(),
            gain: self.gain,
            set,
            submode: self.submode.clone(),
        }
    }
}

/// The settings commands.
#[derive(Debug, Subcommand)]
enum SettingsAction {
    /// Write the settings of every object to a file as saved settings.
    Export {
        /// The file.
        file: PathBuf,
    },
    /// Apply a file of saved settings (all or nothing) and keep the settings for the next run.
    Import {
        /// The file.
        file: PathBuf,
    },
    /// Print the JSON Schema of the settings of an owner.
    Schema {
        /// The owner.
        owner: String,
    },
}

/// The vectors commands.
#[derive(Debug, Subcommand)]
enum VectorsAction {
    /// Show each recording of the manifest: present, missing or of the wrong size.
    List {
        /// Only the paths that start with this text (a family: dvbt).
        filter: Option<String>,
    },
    /// Compare the size and the SHA-256 of each recording. Fails if one is missing or wrong.
    Verify {
        /// Only the paths that start with this text.
        filter: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List the stacks: id, name, group and what they receive.
    Stacks,
    /// List the radio drivers.
    Drivers,
    /// List the services.
    Services,
    /// Show the settings of an owner (key, label, kind, range, choices, value, help), and set them with key=value.
    #[command(args_conflicts_with_subcommands = true)]
    Settings {
        #[command(subcommand)]
        action: Option<SettingsAction>,
        /// The owner: engine, service:<key>, kind:stack:<id>, kind:driver:<key>, kind:component:<kind>.
        owner: Option<String>,
        /// Values to set, as key=value.
        values: Vec<String>,
    },
    /// Show the results of an owner, or one of its tables.
    Show {
        /// The owner.
        owner: String,
        /// The table.
        table: Option<String>,
        /// Only what changed after this change counter.
        #[arg(long, default_value_t = 0)]
        since: u64,
        /// Keep printing the changes until Ctrl-C.
        #[arg(long)]
        follow: bool,
    },
    /// Run a command of an owner with key=value arguments and print its reply. Without a command, list the commands.
    Call {
        /// The owner.
        owner: String,
        /// The command key.
        command: Option<String>,
        /// The arguments, as key=value.
        args: Vec<String>,
    },
    /// Print the events of an owner.
    Events {
        /// The owner.
        owner: String,
        /// Keep printing the events until Ctrl-C.
        #[arg(long)]
        follow: bool,
    },
    /// Run a stack on a source: a live status line, its results as they change, its events and its sound. Type
    /// `call`, `set`, `show` or `events` lines while it runs. Ctrl-C stops it.
    Run {
        /// The stack id.
        stack: String,
        /// Settings of the stack or the radio, as key=value.
        values: Vec<String>,
        #[command(flatten)]
        tune: TuneArgs,
        /// Where the sound goes: speaker, none, or a file name that ends in .wav. Default: speaker, none with --json.
        #[arg(long)]
        audio: Option<SoundTo>,
        /// A WAV file to transmit as the microphone: it plays from its start each time the session asks for host
        /// audio, such as after `call session ptt on=true`.
        #[arg(long, value_name = "FILE")]
        tx_audio: Option<PathBuf>,
    },
    /// Play a recording (SigMF, WAV or raw IQ) through a stack, to its end.
    Play {
        /// The recording.
        file: PathBuf,
        /// The stack id.
        stack: String,
        /// Settings of the stack or the radio, as key=value.
        values: Vec<String>,
        #[command(flatten)]
        tune: TuneArgs,
        /// Where the sound goes: speaker, none, or a .wav file.
        #[arg(long)]
        audio: Option<SoundTo>,
    },
    /// Record the samples of a source into the storage with the record service, while a stack runs on it.
    Record {
        /// The stack id.
        stack: String,
        #[command(flatten)]
        tune: TuneArgs,
    },
    /// Show what a graph resolves: each port with its format and each adapter it inserts.
    Graph {
        /// A stack id, or a graph description file (JSON).
        what: String,
        /// The sample rate of the graph's input. Default: the stack's own, 2M for a file.
        #[arg(long)]
        rate: Option<String>,
    },
    /// Print the level of a stream inside the graph of a session.
    Tap {
        /// The stack id.
        stack: String,
        /// The output port, as <component>.<port>, or input.
        port: String,
        #[command(flatten)]
        tune: TuneArgs,
    },
    /// Draw the spectrum of the source as a waterfall, while a stack runs on it.
    Spectrum {
        /// The stack id.
        stack: String,
        #[command(flatten)]
        tune: TuneArgs,
        /// Characters per line.
        #[arg(long, default_value_t = 100)]
        width: usize,
    },
    /// Run a stack on a source and show where its processor time goes: the components and their sections by share of
    /// one core, then the stages with their queues, then the session.
    Profile {
        /// The stack id.
        stack: String,
        #[command(flatten)]
        tune: TuneArgs,
    },
    /// Run the benchmarks of every stack and show the processor time and the speed against real time.
    Bench {
        /// Only the benchmarks whose name or stack contains this text.
        filter: Option<String>,
        /// Seconds for each benchmark.
        #[arg(long, default_value_t = 1.0)]
        seconds: f64,
    },
    /// Run each stack's test signal through the engine with an offset and noise, and show a table of the checks.
    Verify {
        /// The stacks (default: every stack with a test signal).
        stacks: Vec<String>,
        /// Seconds of test signal for each case.
        #[arg(long, default_value_t = 5.0)]
        seconds: f64,
        /// No offset and no noise.
        #[arg(long)]
        clean: bool,
        /// The SNR of the noise, dB in the stack's bandwidth.
        #[arg(long, default_value_t = 20.0)]
        snr: f64,
    },
    /// The test recordings of the manifest.
    Vectors {
        #[command(subcommand)]
        action: VectorsAction,
    },
    /// Show the station (position, height, callsign, locator) that every stack gets, and change it.
    Station {
        /// The position, LAT,LON in degrees.
        #[arg(long)]
        position: Option<String>,
        /// The antenna height, metres.
        #[arg(long)]
        height: Option<String>,
        /// The callsign.
        #[arg(long)]
        callsign: Option<String>,
        /// The Maidenhead locator.
        #[arg(long)]
        locator: Option<String>,
        /// Keep the station for the next run.
        #[arg(long)]
        save: bool,
    },
    /// Write a C header with the key of every setting, field, table, column, event and command of every owner kind.
    Declarations {
        /// Write the C header (the only form now).
        #[arg(long = "c-header", required = true)]
        c_header: bool,
        /// The file to write (default: the output).
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// The streams of a command, and the console lines that `run` reads.
pub struct Io<'a> {
    /// Data.
    pub out: &'a mut dyn Write,
    /// Notes, errors and the status line.
    pub err: &'a mut dyn Write,
    /// The error stream is a terminal.
    pub redraw: bool,
    /// Console lines for `run`.
    pub lines: Option<Receiver<String>>,
}

/// The columns of the `stacks` table that the `stacks` command shows.
const STACK_COLUMNS: &[&str] = &["name", "group", "blurb", "default_hz"];

static STOP: AtomicBool = AtomicBool::new(false);

/// Asks every live command to stop, as Ctrl-C does.
pub fn request_stop() {
    STOP.store(true, Ordering::SeqCst);
}

fn stopping() -> bool {
    STOP.load(Ordering::SeqCst)
}

/// The storage folder: the one given, else `~/.sdrkit`.
fn storage_folder(given: Option<PathBuf>) -> PathBuf {
    given.unwrap_or_else(|| {
        std::env::var_os("HOME")
            .map_or_else(|| PathBuf::from("."), PathBuf::from)
            .join(".sdrkit")
    })
}

/// Runs the command line `args` (the program name first) and returns the exit code: 0, 1 if a check or a call failed, 2
/// for bad usage. `storage` replaces the storage folder (a test supplies a `MemStorage`).
pub fn run(
    args: impl IntoIterator<Item = OsString>,
    storage: Option<Arc<dyn Storage>>,
    io: Io,
) -> u8 {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            let _ = write!(io.err, "{}", e.render().ansi());
            return u8::try_from(e.exit_code()).unwrap_or(2);
        }
    };
    let Io {
        out,
        err,
        redraw,
        lines,
    } = io;
    let mut engine_slot = None;
    let r = command(
        cli,
        storage,
        &mut engine_slot,
        &mut Out::new(out, err, false, redraw),
        lines.as_ref(),
    );
    match r {
        Ok(()) => 0,
        Err(e) => {
            let mut sink = std::io::sink();
            let mut o = Out::new(&mut sink, err, false, false);
            let _ = o.note(&format!("error: {e}"));
            if let Some(engine) = &engine_slot {
                let _ = hint(engine, &e, &mut o);
            }
            e.exit_code()
        }
    }
}

/// Prints what a user can type in place of an unknown stack, owner or driver.
fn hint(engine: &Engine, e: &CliError, o: &mut Out) -> Result<(), CliError> {
    match e {
        CliError::UnknownStack { .. } | CliError::Engine(EngineError::NoSuchStack { .. }) => {
            o.note("The stacks, by group:")?;
            let mut s = Vec::new();
            reflect::catalog(
                engine,
                sdrkit_host_engine::STACKS.key,
                Some("group"),
                STACK_COLUMNS,
                &mut Out::new(&mut s, &mut std::io::sink(), false, false),
            )?;
            o.note(&String::from_utf8_lossy(&s))
        }
        CliError::UnknownDriver { .. } | CliError::Engine(EngineError::NoSuchDriver { .. }) => {
            o.note("The drivers:")?;
            let mut s = Vec::new();
            reflect::catalog(
                engine,
                sdrkit_host_engine::DRIVERS.key,
                None,
                &[],
                &mut Out::new(&mut s, &mut std::io::sink(), false, false),
            )?;
            o.note(&String::from_utf8_lossy(&s))
        }
        CliError::UnknownOwner { .. } => o.note(&format!(
            "The owners: {}",
            host::owner_names(engine).join(", ")
        )),
        _ => Ok(()),
    }
}

/// Makes the engine on `storage`, or on the storage folder, with a folder of the computer shown for a file source.
fn make_engine(
    storage: Option<Arc<dyn Storage>>,
    folder: Option<PathBuf>,
    mount: Option<&PathBuf>,
    slot: &mut Option<Arc<Engine>>,
) -> Result<(Arc<Engine>, Arc<dyn Storage>), CliError> {
    let main: Arc<dyn Storage> = match storage {
        Some(s) => s,
        None => Arc::new(DirStorage::new(storage_folder(folder))?),
    };
    let st: Arc<dyn Storage> = match mount {
        Some(m) => Arc::new(Mounted::new(Arc::clone(&main), m)?),
        None => Arc::clone(&main),
    };
    let engine = host::engine(st)?;
    *slot = Some(Arc::clone(&engine));
    Ok((engine, main))
}

/// The data of the country `region` for a command, read from the data folder `folder`. The engine gets the plain
/// values that the folder has for the country: the channels of a scan from the plans that the country uses, and the
/// frequency entries. `None` without a region, or if `folder` is not given and the tool does not run on the storage
/// folder. A folder that does not exist is reported with a note, and gives `None`.
///
/// # Errors
///
/// [`CliError::Region`] if `region` is not a country code, [`CliError::UnknownRegion`] with the list of the regions if
/// the data has no plans for it, [`CliError::Preset`] for a file that cannot be used.
fn regional(
    engine: &Engine,
    folder: Option<PathBuf>,
    region: Option<&str>,
    city: Option<&str>,
    out: &mut Out,
) -> Result<Option<Regional>, CliError> {
    let Some(region) = region else {
        return Ok(None);
    };
    let region = sdrkit_presets::Region::new(region)?;
    let Some(folder) = folder else {
        return Ok(None);
    };
    let data_error = |e| {
        CliError::from(sdrkit_presets::StartError::from(
            sdrkit_presets::PresetError::from(e),
        ))
    };
    match sdrkit_presets::Data::open(&folder) {
        Ok(data) => {
            // The region is one of the list that the data gives.
            let region = data
                .region(region.country())
                .map_err(|list| CliError::UnknownRegion {
                    region: region.country().to_owned(),
                    list,
                })?;
            let plans = data.plan_ids(region.country());
            let plans: Vec<&str> = plans.iter().map(String::as_str).collect();
            data.load_scan_channels(engine, &plans);
            match data.load_frequency_entries(engine) {
                Ok(_) | Err(sdrkit_presets::DataError::Missing { .. }) => {}
                Err(e) => return Err(data_error(e)),
            }
            Ok(Some(Regional {
                data,
                region: region.with_city(city.unwrap_or("")),
            }))
        }
        Err(sdrkit_presets::DataError::Missing { path }) => {
            out.note(&format!("no region data loaded: {}", path.display()))?;
            Ok(None)
        }
        Err(e) => Err(data_error(e)),
    }
}

fn sound_of(json: bool, given: Option<SoundTo>) -> SoundTo {
    given.unwrap_or(if json { SoundTo::Off } else { SoundTo::Speaker })
}

fn command(
    cli: Cli,
    storage: Option<Arc<dyn Storage>>,
    slot: &mut Option<Arc<Engine>>,
    out: &mut Out,
    lines: Option<&Receiver<String>>,
) -> Result<(), CliError> {
    out.json = cli.json;
    let json = cli.json;
    let region = cli.region.as_deref();
    // A storage that is not a folder (a test) has a data folder only if one is given.
    let data = match &storage {
        Some(_) => cli.data,
        None => Some(sdrkit_presets::data_folder(
            cli.data.as_deref(),
            Some(&storage_folder(cli.storage.clone())),
        )),
    };
    let city = cli.city.as_deref();
    let folder = cli.storage;
    let stop = stopping;
    let source_of = |tune: &TuneArgs, stack: &str| Source::parse(&tune.source, stack);
    match cli.command {
        Command::Vectors { action } => match action {
            VectorsAction::List { filter } => checks::vectors(filter.as_deref(), false, out),
            VectorsAction::Verify { filter } => checks::vectors(filter.as_deref(), true, out),
        },
        Command::Run {
            stack,
            values,
            tune,
            audio,
            tx_audio,
        } => {
            let tx_audio = tx_audio
                .map(|p| txaudio::TxAudio::read(&p).map(Arc::new))
                .transpose()?;
            let source = source_of(&tune, &stack);
            let (engine, _) = make_engine(storage, folder, source.mount.as_ref(), slot)?;
            let desc = live::stack(&stack)?;
            let regional = regional(&engine, data, region, city, out)?;
            let tune_of = tune.tune(&values);
            let live = live::start(&engine, &source, desc, &tune_of, regional.as_ref(), out)?;
            let w = Watch {
                seconds: tune.seconds,
                sound: sound_of(json, audio),
                to_the_end: source.mount.is_some(),
                verbose: tune.verbose,
                tx_audio,
            };
            let r = live::watch(&engine, &live, &w, out, lines, &stop);
            let e = live::end(&engine, &live);
            r.and(e)
        }
        Command::Play {
            file,
            stack,
            values,
            mut tune,
            audio,
        } => {
            tune.source = format!("file:{}", file.display());
            let source = source_of(&tune, &stack);
            let (engine, _) = make_engine(storage, folder, source.mount.as_ref(), slot)?;
            let desc = live::stack(&stack)?;
            // A recording keeps its own frequency: no preset of the region.
            let live = live::start(&engine, &source, desc, &tune.tune(&values), None, out)?;
            let w = Watch {
                seconds: tune.seconds,
                sound: sound_of(json, audio),
                to_the_end: true,
                verbose: tune.verbose,
                tx_audio: None,
            };
            let r = live::watch(&engine, &live, &w, out, lines, &stop);
            let e = live::end(&engine, &live);
            r.and(e)
        }
        Command::Record { stack, tune } => {
            let source = source_of(&tune, &stack);
            let (engine, _) = make_engine(storage, folder, source.mount.as_ref(), slot)?;
            let desc = live::stack(&stack)?;
            let regional = regional(&engine, data, region, city, out)?;
            let tune_of = tune.tune(&[]);
            let live = live::start(&engine, &source, desc, &tune_of, regional.as_ref(), out)?;
            let seconds = tune.seconds.unwrap_or(10.0);
            let args = [
                format!(
                    "session={}",
                    engine
                        .key_of(sdrkit_host_engine::OwnerRef::Session(live.session))
                        .unwrap_or_default()
                ),
                format!("seconds={seconds}"),
            ];
            let r = reflect::call(
                &engine,
                Some(&live),
                "service:record",
                Some("iq.start"),
                &args,
                out,
            )
            .and_then(|()| {
                let w = Watch {
                    seconds: Some(seconds + 1.0),
                    sound: SoundTo::Off,
                    to_the_end: false,
                    verbose: false,
                    tx_audio: None,
                };
                live::watch(
                    &engine,
                    &live,
                    &w,
                    &mut Out::new(&mut std::io::sink(), &mut std::io::sink(), false, false),
                    lines,
                    &stop,
                )?;
                reflect::show(
                    &engine,
                    Some(&live),
                    "service:record",
                    Some("recordings"),
                    0,
                    None,
                    out,
                )
            });
            let e = live::end(&engine, &live);
            r.and(e)
        }
        Command::Tap { stack, port, tune } => {
            let source = source_of(&tune, &stack);
            let (engine, _) = make_engine(storage, folder, source.mount.as_ref(), slot)?;
            let desc = live::stack(&stack)?;
            let regional = regional(&engine, data, region, city, out)?;
            let tune_of = tune.tune(&[]);
            let live = live::start(&engine, &source, desc, &tune_of, regional.as_ref(), out)?;
            let r = live::tap(&engine, &live, &port, tune.seconds, out, &stop);
            let e = live::end(&engine, &live);
            r.and(e)
        }
        Command::Spectrum { stack, tune, width } => {
            let source = source_of(&tune, &stack);
            let (engine, _) = make_engine(storage, folder, source.mount.as_ref(), slot)?;
            let desc = live::stack(&stack)?;
            let regional = regional(&engine, data, region, city, out)?;
            let tune_of = tune.tune(&[]);
            let live = live::start(&engine, &source, desc, &tune_of, regional.as_ref(), out)?;
            let r = live::spectrum(&engine, &live, width, tune.seconds, out, &stop);
            let e = live::end(&engine, &live);
            r.and(e)
        }
        Command::Profile { stack, tune } => {
            let source = source_of(&tune, &stack);
            let (engine, _) = make_engine(storage, folder, source.mount.as_ref(), slot)?;
            let desc = live::stack(&stack)?;
            let regional = regional(&engine, data, region, city, out)?;
            let tune_of = tune.tune(&[]);
            let live = live::start(&engine, &source, desc, &tune_of, regional.as_ref(), out)?;
            let r = live::profile(&engine, &live, tune.seconds, out, &stop);
            let e = live::end(&engine, &live);
            r.and(e)
        }
        command => {
            let (engine, main) = make_engine(storage, folder, None, slot)?;
            regional(&engine, data, region, city, out)?;
            fixed(command, &engine, &*main, out, &stop)
        }
    }
}

/// The commands that need no source.
fn fixed(
    command: Command,
    engine: &Engine,
    storage: &dyn Storage,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    match command {
        Command::Stacks => reflect::catalog(
            engine,
            sdrkit_host_engine::STACKS.key,
            Some("group"),
            STACK_COLUMNS,
            out,
        ),
        Command::Drivers => {
            reflect::catalog(engine, sdrkit_host_engine::DRIVERS.key, None, &[], out)
        }
        Command::Services => {
            reflect::catalog(engine, sdrkit_host_engine::SERVICES.key, None, &[], out)
        }
        Command::Settings {
            action,
            owner,
            values,
        } => match (action, owner) {
            (Some(SettingsAction::Export { file }), _) => {
                std::fs::write(&file, engine.export_settings()?)?;
                out.line("exported")
            }
            (Some(SettingsAction::Import { file }), _) => {
                engine.import_settings(&std::fs::read(file)?)?;
                host::save_settings(engine, storage)?;
                out.line("imported")
            }
            (Some(SettingsAction::Schema { owner }), _) => {
                let r = host::owner(engine, &owner, None)?;
                out.text(&format!("{}\n", engine.settings_schema(r)?))
            }
            (None, Some(owner)) => reflect::settings(engine, storage, None, &owner, &values, out),
            (None, None) => reflect::settings(engine, storage, None, "engine", &values, out),
        },
        Command::Show {
            owner,
            table,
            since,
            follow,
        } => reflect::show(
            engine,
            None,
            &owner,
            table.as_deref(),
            since,
            follow.then_some(stop),
            out,
        ),
        Command::Call {
            owner,
            command,
            args,
        } => reflect::call(engine, None, &owner, command.as_deref(), &args, out),
        Command::Events { owner, follow } => {
            reflect::events(engine, None, &owner, follow, out, stop)
        }
        Command::Graph { what, rate } => graph(engine, &what, rate.as_deref(), out),
        Command::Bench { filter, seconds } => {
            checks::bench(engine, filter.as_deref(), seconds, out, stop)
        }
        Command::Verify {
            stacks,
            seconds,
            clean,
            snr,
        } => checks::verify(
            engine,
            &stacks,
            checks::VerifyAsk {
                seconds,
                clean,
                snr_db: snr,
            },
            out,
        ),
        Command::Station {
            position,
            height,
            callsign,
            locator,
            save,
        } => {
            let mut values = Vec::new();
            for (k, v) in [
                ("position", position),
                ("height_m", height),
                ("callsign", callsign),
                ("locator", locator),
            ] {
                if let Some(v) = v {
                    values.push(format!("{k}={v}"));
                }
            }
            reflect::apply(engine, OwnerRef::Engine, "engine", &values)?;
            if save {
                host::save_settings(engine, storage)?;
            }
            reflect::settings(engine, storage, None, "engine", &[], out)
        }
        Command::Declarations { out: file, .. } => {
            let h = reflect::c_header(engine, out)?;
            match file {
                Some(f) => Ok(std::fs::write(f, h)?),
                None => out.text(&h),
            }
        }
        Command::Vectors { .. }
        | Command::Run { .. }
        | Command::Play { .. }
        | Command::Record { .. }
        | Command::Tap { .. }
        | Command::Profile { .. }
        | Command::Spectrum { .. } => Ok(()),
    }
}

/// `graph <stack|file>`: the graph of the stack `what`, or the graph description in the file `what`, resolved by the
/// engine for an IQ input at `rate` (the stack's own rate, or 2 MHz for a file). Prints the ports and the adapters as a
/// tree.
fn graph(engine: &Engine, what: &str, rate: Option<&str>, out: &mut Out) -> Result<(), CliError> {
    let path = std::path::Path::new(what);
    let (desc, own_rate) = if path.is_file() {
        let d = sdrkit_graph::Description::from_json(&std::fs::read_to_string(path)?)?;
        (d, 2e6)
    } else {
        let stack = live::stack(what)?;
        (engine.stack_graph(stack.id)?, stack.tuning.rate_hz)
    };
    let rate = match rate {
        Some(r) => text::parse_hz(r, text::Bare::Hz)?,
        None => own_rate,
    };
    let plan = desc.plan(engine.registry())?;
    let report = engine.validate_graph(&plan, rate, 0.0)?;
    if out.json {
        let ports: Vec<_> = report
            .ports
            .iter()
            .map(|(p, f)| serde_json::json!({ "port": p, "kind": format!("{:?}", f.kind), "rate_hz": f.rate, "channels": f.channels }))
            .collect();
        let adapters: Vec<_> = report
            .adapters
            .iter()
            .map(|a| serde_json::json!({ "id": a.id, "kind": a.kind, "reads": a.source, "gives": a.output, "consumers": a.consumers }))
            .collect();
        return out.json(&serde_json::json!({ "ports": ports, "adapters": adapters }));
    }
    let mut s = String::from("Ports\n");
    let n = report.ports.len();
    for (i, (p, f)) in report.ports.iter().enumerate() {
        let branch = if i + 1 == n { "└─" } else { "├─" };
        s.push_str(&format!(
            "{branch} {p}: {:?}, {}, {} channel(s)\n",
            f.kind,
            text::hz(f.rate),
            f.channels
        ));
    }
    s.push_str("Adapters\n");
    let n = report.adapters.len();
    for (i, a) in report.adapters.iter().enumerate() {
        let branch = if i + 1 == n { "└─" } else { "├─" };
        s.push_str(&format!(
            "{branch} {} ({}): {} -> {}",
            a.id, a.kind, a.source, a.output
        ));
        if !a.consumers.is_empty() {
            s.push_str(&format!(", read by {}", a.consumers.join(", ")));
        }
        s.push('\n');
    }
    out.text(&s)
}
