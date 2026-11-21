//! `sdrkit-tui`: the terminal application.

use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use sdrkit_host_tui::api::{self, Handle};
use sdrkit_host_tui::app::{App, Start};
use sdrkit_host_tui::ui;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// The time between two reads of the library: 15 a second.
const TICK: Duration = Duration::from_millis(66);

/// A terminal application for sdrkit: stacks, radios, sessions, settings, events, commands and services.
#[derive(Parser, Debug)]
#[command(name = "sdrkit-tui", version)]
struct Args {
    /// The address of the radio, `<driver>:<address>`. The Radios screen lists the address forms of each driver.
    #[arg(long)]
    source: Option<String>,
    /// The stack to start on the source. Without a source, the stack runs on its own test signal.
    #[arg(long)]
    stack: Option<String>,
    /// The submode of the stack. Without it, the stack runs its default submode.
    #[arg(long)]
    submode: Option<String>,
    /// The frequency in MHz to tune the stack to. Without it, the stack runs at its default frequency.
    #[arg(long, value_name = "MHZ")]
    freq: Option<f64>,
    /// The country (ISO 3166 code: AU, US, DE ...) whose rows of the data folder the application uses: the preset of a
    /// stack, the known stations, the channels of a scan and the frequency entries. The key g chooses it from the list
    /// of the data. The engine has no region.
    #[arg(long)]
    region: Option<String>,
    /// The city of the country. A stack then starts on the strongest known station of the city.
    #[arg(long)]
    city: Option<String>,
    /// The folder where the engine keeps its settings and files. Without it, the engine keeps them in memory.
    #[arg(long)]
    storage: Option<PathBuf>,
    /// The data folder: plans, known stations, cities, frequency entries. Without it, the first of these that has plans:
    /// `data` in the storage folder, `~/.sdrkit/data`, `data` or `share/sdrkit/data` in a folder above the program,
    /// `data` in this folder.
    #[arg(long, value_name = "FOLDER")]
    data: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let engine = match &args.storage {
        Some(folder) => Handle::engine_in(folder),
        None => Handle::engine_in_memory(),
    };
    let data = sdrkit_presets::data_folder(args.data.as_deref(), args.storage.as_deref());
    // The settings file of the application: in the storage folder, else in `~/.sdrkit`.
    let prefs = args.storage.clone().unwrap_or_else(|| {
        std::env::var_os("HOME")
            .map_or_else(|| PathBuf::from("."), PathBuf::from)
            .join(".sdrkit")
    });
    let start = Start {
        source: args.source,
        stack: args.stack,
        submode: args.submode,
        freq_hz: args.freq.map(|mhz| mhz * 1e6),
        region: args.region,
        city: args.city,
        data: Some(data),
        prefs: Some(prefs.join("tui.conf")),
    };
    let app = engine.and_then(|e| App::new(e, start));
    let mut app = match app {
        Ok(a) => a,
        Err(e) => {
            eprintln!("sdrkit-tui: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut terminal = ratatui::init();
    // The terminal is in raw mode, so Ctrl-C arrives as a key. A signal from outside (kill, a closed window) must
    // still leave the terminal usable.
    let handler = ctrlc::set_handler(|| {
        ratatui::restore();
        std::process::exit(130);
    });
    if let Err(e) = handler {
        app.fail(format!("no signal handler: {e}"));
    }
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    app.shutdown();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sdrkit-tui: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Draws, reads keys and ticks until the user quits.
fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let mut next = Instant::now();
    if app.status.text.is_empty() {
        if let Ok(v) = api::version() {
            app.say(format!("sdrkit {v}"));
        }
    }
    while !app.quit {
        terminal.draw(|f| ui::draw(f, app))?;
        let wait = next.saturating_duration_since(Instant::now());
        if event::poll(wait)? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    app.key(k);
                }
            }
        }
        if Instant::now() >= next {
            app.tick();
            next = Instant::now() + TICK;
        }
    }
    Ok(())
}
