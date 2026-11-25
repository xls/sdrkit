//! The state of the application, what each key does, and what each tick reads from the library.

use crate::api::{self, CommandDecl, Decl, EventType, Handle, Id, Type, Value};
use crate::entry::{Entry, EntryKey};
use crate::find::Find;
use crate::format;
use crate::known::{self, Regional};
use crate::radio_form::{Outcome, RadioForm};
use crate::results::{Results, Row};
use crate::sound::{Device, Meter, Output, SoundError, Speaker};
use crate::views::{self, StackRow};
use crate::waterfall::Waterfall;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use sdrkit_presets::Preset;
use std::collections::{HashMap, VecDeque};
use std::fmt::Display;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The number of events that the events screen keeps.
const EVENTS_KEPT: usize = 1000;
/// The key of the line of the source list that opens a radio. No address has this form.
const NEW_RADIO: &str = "+";
/// The key of the session setting that holds the frequency that the session listens to.
const TUNED: &str = "tuned_hz";
/// The key of the engine setting that is the transmit switch.
const TRANSMIT_ALLOWED: &str = "transmit_allowed";
/// The key of the session result that counts the bursts on air. A session that cannot transmit does not have it.
const BURSTS_SENT: &str = "session.tx_sent";
/// The key of the event type of a burst, and of its field that holds why the burst did not go on air.
const BURST: &str = "session.transmit";
const BURST_REFUSED: &str = "refused";
/// The key of the session result that holds what stops the session from transmitting now.
pub(crate) const TX_BLOCKED: &str = "session.tx_blocked";
/// The key of the table that a session with an entry line opens on.
const MESSAGES: &str = "messages";
/// The pane of the session and services screens that shows the tables.
pub const TABLES_PANE: usize = 1;
/// The pane of the session screen that is the entry line.
pub const ENTRY_PANE: usize = 2;
/// The object key of the service that scans.
const SCAN: &str = "service.scan";
/// The key of the table of that service that holds what the scan found.
const SCAN_HITS: &str = "hits";

/// The values of the shown settings are read again this often, for a value that the engine changed itself.
const VALUES_EVERY: Duration = Duration::from_millis(500);

/// What follows the opening of a radio.
enum Then {
    /// The radios screen shows the radio.
    Radios,
    /// The stack starts on the radio.
    Start(String),
    /// The form that named the radio closes, and the stacks start on the radio.
    Form,
}

/// A radio that a thread opens: the opening of a network radio waits for the network, so the screen does not make it.
struct Opening {
    address: String,
    then: Then,
    since: Instant,
    done: std::sync::mpsc::Receiver<api::Result<Handle>>,
}

/// True for the test signal of a stack, which opens inside the process.
fn opens_at_once(address: &str) -> bool {
    address.starts_with("synth:")
}

/// A screen of the application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// The stack catalog.
    Stacks,
    /// The driver catalog and the open radios.
    Radios,
    /// The running stack.
    Session,
    /// The settings of the chosen object.
    Settings,
    /// The event types and the events.
    Events,
    /// The commands of the chosen object.
    Commands,
    /// The services of the engine.
    Services,
    /// The load of the running session: its components, stages and connections.
    Load,
}

/// How long the status line shows a rise of the lost samples or the gaps in the warning colour.
pub const LOSS_WARNING: Duration = Duration::from_secs(5);

impl Screen {
    /// Every screen, in the order of the bar.
    pub const ALL: [Screen; 8] = [
        Screen::Stacks,
        Screen::Radios,
        Screen::Session,
        Screen::Settings,
        Screen::Events,
        Screen::Commands,
        Screen::Services,
        Screen::Load,
    ];

    /// Returns the name in the bar.
    pub fn title(self) -> &'static str {
        match self {
            Screen::Stacks => "Stacks",
            Screen::Radios => "Radios",
            Screen::Session => "Session",
            Screen::Settings => "Settings",
            Screen::Events => "Events",
            Screen::Commands => "Commands",
            Screen::Services => "Services",
            Screen::Load => "Load",
        }
    }
}

/// An object of the engine with the copies that the application keeps of it.
pub struct Owner {
    /// The object key, or a name for an object that has no key.
    pub name: String,
    /// The handle.
    pub handle: Handle,
    /// The results as last read.
    pub results: Results,
    /// The error that showed the handle as dead, once a call returned `SDRKIT_E_NO_SUCH_OBJECT`.
    pub dead: Option<api::Error>,
    /// The declarations of the settings, read when first needed.
    pub settings: Vec<Decl>,
    /// The values of the settings, by id.
    pub values: Vec<Value>,
    /// The declarations of the commands, read when first needed.
    pub commands: Vec<CommandDecl>,
    loaded: bool,
}

impl Owner {
    /// Makes the owner of `handle` with the name `name`.
    pub fn new(name: String, handle: Handle) -> Owner {
        Owner {
            name,
            handle,
            results: Results::default(),
            dead: None,
            settings: Vec::new(),
            values: Vec::new(),
            commands: Vec::new(),
            loaded: false,
        }
    }

    /// Notes `e` if it shows that the handle is dead, and returns it.
    fn note(&mut self, e: api::Error) -> api::Error {
        if e.is_dead_handle() {
            self.dead = Some(e.clone());
        }
        e
    }

    /// Reads what changed in the results. A dead owner is not read again.
    ///
    /// # Errors
    ///
    /// The code of the library.
    pub fn poll(&mut self) -> api::Result<()> {
        if self.dead.is_some() {
            return Ok(());
        }
        self.results.poll(&self.handle).map_err(|e| self.note(e))
    }

    /// Reads the declarations of the settings and the commands if they were not read yet.
    ///
    /// # Errors
    ///
    /// The code of the library.
    pub fn load(&mut self) -> api::Result<()> {
        if self.loaded || self.dead.is_some() {
            return Ok(());
        }
        self.settings = self.handle.settings().map_err(|e| self.note(e))?;
        self.values = vec![Value::None; self.settings.len()];
        self.commands = self.handle.commands().map_err(|e| self.note(e))?;
        self.loaded = true;
        Ok(())
    }

    /// Reads the values of the settings again.
    ///
    /// # Errors
    ///
    /// The code of the library.
    pub fn read_values(&mut self) -> api::Result<()> {
        if self.dead.is_some() {
            return Ok(());
        }
        for (id, v) in (0..).zip(self.values.iter_mut()) {
            if let Err(e) = self.handle.get(id, v) {
                if e.is_dead_handle() {
                    self.dead = Some(e.clone());
                }
                return Err(e);
            }
        }
        Ok(())
    }

    /// Takes the waiting events and adds them to `out`.
    ///
    /// # Errors
    ///
    /// The code of the library.
    fn take_events(&mut self, out: &mut VecDeque<EventLine>) -> api::Result<usize> {
        if self.dead.is_some() {
            return Ok(0);
        }
        let n = self.handle.events_take().map_err(|e| self.note(e))?;
        for i in 0..n {
            let info = self.handle.event(i)?;
            let mut fields = vec![Value::None; info.fields as usize];
            for (f, v) in (0..).zip(fields.iter_mut()) {
                self.handle.event_field(i, f, v)?;
            }
            if out.len() >= EVENTS_KEPT {
                out.pop_front();
            }
            out.push_back(EventLine {
                source: self.name.clone(),
                ty: info.ty,
                time: info.time,
                freq_hz: info.freq_hz,
                fields,
            });
        }
        Ok(n as usize)
    }
}

/// One event as the events screen shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct EventLine {
    /// The name of the object that sent it.
    pub source: String,
    /// The id of its event type.
    pub ty: Id,
    /// Seconds since the object started.
    pub time: f64,
    /// The frequency of the signal in Hz.
    pub freq_hz: f64,
    /// The fields, by id.
    pub fields: Vec<Value>,
}

/// A running session.
pub struct Live {
    /// The session object.
    pub session: Owner,
    /// The stack of the session.
    pub stack: Owner,
    /// The stack id.
    pub stack_id: String,
    /// The object key of the radio that the session reads.
    pub radio: String,
    /// The address of that radio.
    pub address: String,
    /// The speaker of the session. It plays once the session sends audio.
    pub speaker: Option<Speaker>,
    /// The speaker did not start: the application does not try again for this session.
    pub sound_failed: bool,
    /// The spectrum history.
    pub waterfall: Waterfall,
    /// The session was stopped and waits to close.
    pub closing: bool,
    /// The id of the session setting that holds the frequency, if the session has it.
    pub tuned: Option<Id>,
    /// The known stations of the region for the service of the stack, from the region data.
    pub known: Option<Results>,
    /// The service that scans, if the engine has it.
    pub scan: Option<Owner>,
    /// The entry line of the session.
    pub entry: Entry,
}

impl Live {
    /// Returns the command of the entry line: the first command of the stack, if its first argument is a required
    /// text that is not a row of a table. Returns `None` for a session that waits to close.
    pub fn entry_command(&self) -> Option<&CommandDecl> {
        let c = self.stack.commands.first().filter(|_| !self.closing)?;
        let a = c.args.first()?;
        let free = a.refers_table.is_empty() || !a.refers_column.is_empty();
        (a.ty == Type::String && a.required && free).then_some(c)
    }

    /// Returns what stops the session from transmitting now, as the engine wrote it.
    pub fn tx_blocked(&self) -> Option<String> {
        let all = [&self.session.results, &self.stack.results];
        let mut texts = all.into_iter().filter_map(|r| field_text(r, TX_BLOCKED));
        texts.find(|t| !t.is_empty())
    }

    /// Starts the speaker of the session, which reads the session through its own handle and waits for its audio.
    ///
    /// # Errors
    ///
    /// [`SoundError`]: the library did not return a handle or the audio of the session, or the speaker did not start.
    /// The application does not try again for this session.
    fn start_sound(
        &mut self,
        engine: &Handle,
        meter: &Arc<Meter>,
        output: &Arc<dyn Output>,
    ) -> Result<(), SoundError> {
        if let Some(e) = self.speaker.as_mut().and_then(Speaker::failed) {
            self.speaker = None;
            self.sound_failed = true;
            return Err(e);
        }
        if self.speaker.is_some() || self.sound_failed || self.session.dead.is_some() {
            return Ok(());
        }
        let own = self.session.handle.key().and_then(|k| engine.find(&k));
        let own = own.map_err(|e| {
            self.sound_failed = true;
            SoundError::Session(e)
        })?;
        self.speaker = Some(Speaker::start(Arc::clone(output), own, Arc::clone(meter)));
        Ok(())
    }
}

/// The object whose settings and commands the settings and commands screens show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The engine.
    Engine,
    /// The open radio of that index.
    Radio(usize),
    /// The running session.
    Session,
    /// The stack of the running session.
    Stack,
    /// The chosen service.
    Service,
}

/// One line of the controls pane of the session screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The frequency that the session listens to.
    Frequency,
    /// Starts a scan of the band of the stack, or stops the scan that runs.
    Scan,
    /// The transmit switch of the engine.
    Transmit,
    /// Command `n` of the stack.
    Command(usize),
    /// Setting `id` of the stack.
    Setting(usize),
}

/// One line of the objects pane of the settings and commands screens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectLine {
    /// The object.
    pub target: Target,
    /// The index of the service in the engine's `services` table, for a service.
    pub service: Option<usize>,
    /// The kind of object: "Engine", "Radio", "Session", "Stack" or "Service".
    pub kind: &'static str,
    /// The name of the object, or empty for the engine.
    pub name: String,
}

/// What a line of typed text is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditFor {
    /// The source of the stacks screen: the test signal, an open radio or a radio to open.
    Source,
    /// Setting `id` of the target.
    Setting(Id),
    /// Argument `n` of the command form.
    Argument(usize),
    /// The frequency that the running session listens to.
    Frequency,
    /// The frequency that the stacks screen starts a stack on.
    StartFrequency,
    /// The country whose rows of the data folder the application uses.
    Region,
    /// A channel of the region data for the running stack: the session tunes to it.
    Channel,
    /// A channel of the region data for the chosen stack of the stacks screen: the stack starts on it.
    StartChannel,
}

/// What the keys do now.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// The keys of the screen.
    Normal,
    /// The keys type a line of text.
    Edit {
        /// What the text is for.
        what: EditFor,
        /// The text so far.
        text: String,
    },
    /// The arrow keys choose an option of a value of type enum, or a row key for a value that refers to a table.
    Pick {
        /// What the option is for: a setting or an argument.
        what: EditFor,
        /// The labels of the options.
        options: Vec<String>,
        /// The row keys that the options stand for, or empty for the options of an enum.
        keys: Vec<String>,
        /// The chosen option.
        sel: usize,
        /// The text typed to search the options.
        find: Find,
    },
    /// The keys type the text that the list of the pane is searched for.
    Find {
        /// The text and the shown match.
        find: Find,
        /// The line that the list showed as selected before the search.
        from: usize,
    },
}

/// The arguments of a command that the user fills.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    /// The index of the command in the owner's commands.
    pub command: usize,
    /// The value of each argument, or `None` for its default.
    pub values: Vec<Option<Value>>,
    /// The chosen argument.
    pub sel: usize,
    /// The command runs as soon as its one argument has a value, and the form closes.
    pub quick: bool,
}

/// The line under the screens: the last result of an action.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Status {
    /// The text.
    pub text: String,
    /// True for an error.
    pub error: bool,
}

/// The options of the command line.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Start {
    /// The address of the radio. `synth` and `synth:` select the test signal of the stack.
    pub source: Option<String>,
    /// The stack to start on it.
    pub stack: Option<String>,
    /// The submode of the stack, or `None` for its default.
    pub submode: Option<String>,
    /// The frequency in Hz to tune the stack to, or `None` for its default.
    pub freq_hz: Option<f64>,
    /// The country (ISO 3166 code) whose rows of the data folder the application uses, or `None` for no such data.
    pub region: Option<String>,
    /// The city of the country, for the known stations.
    pub city: Option<String>,
    /// The data folder, or `None` for none.
    pub data: Option<std::path::PathBuf>,
    /// The settings file of the application, which keeps the chosen region between runs, or `None` to keep nothing.
    pub prefs: Option<std::path::PathBuf>,
}

/// A preset of the region data, and its settings as values of the settings of its stack.
type StartFrom = (Preset, Vec<(Id, Value)>);

/// The selected line on each list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Selection {
    /// The stacks screen.
    pub stack: usize,
    /// The open radios.
    pub radio: usize,
    /// The table that the session or service shows.
    pub table: usize,
    /// The row of that table.
    pub row: usize,
    /// The number of columns of that table scrolled off to the left. The key column stays in place.
    pub column: usize,
    /// The settings screen.
    pub setting: usize,
    /// The event types.
    pub event_type: usize,
    /// The commands screen.
    pub command: usize,
    /// The services screen.
    pub service: usize,
    /// The controls of the session screen.
    pub control: usize,
}

/// The application.
pub struct App {
    /// The engine.
    pub engine: Owner,
    /// The samples lost and the gaps of the live session as last read, and when one of them last rose.
    pub losses: (i64, i64),
    pub losses_rose: Option<Instant>,
    /// The radios that the application opened.
    pub radios: Vec<Owner>,
    /// The session that the application shows and plays.
    pub live: Option<Live>,
    /// The other sessions that run, each on its own radio.
    pub parked: Vec<Live>,
    /// The chosen service.
    pub service: Option<Owner>,
    /// The event types of the catalog of the engine, by id.
    pub event_types: Vec<EventType>,
    /// The events, oldest first.
    pub events: VecDeque<EventLine>,
    /// The screen shown.
    pub screen: Screen,
    /// The object of the settings and commands screens.
    pub target: Target,
    /// The pane of the screen that the keys act on. Pane 0 is the left one: the objects of the settings and commands
    /// screens, the list of the services screen, the controls of the session screen.
    pub pane: usize,
    /// What the keys do.
    pub mode: Mode,
    /// The command form, while one is open.
    pub form: Option<Form>,
    /// The form that opens a radio, while it is open.
    pub radio_form: Option<RadioForm>,
    /// The list of keys is shown.
    pub help: bool,
    /// The reply of the last command: label and text of each field.
    pub reply: Vec<(String, String)>,
    /// The row key chosen on a table with Enter.
    pub chosen_row: Option<String>,
    /// The key of the selected row, once the user moved the selection. The selection follows this row while the
    /// table changes.
    row_key: Option<String>,
    /// The row that each table was left on, by [`App::table_tag`], and the table that each object was left on, by
    /// `tab/` and [`App::view_tag`]. A table that is shown again shows that row.
    rows: HashMap<String, String>,
    /// The line of the settings and of the commands that each object was left on, by the name of the object.
    lines: HashMap<String, (usize, usize)>,
    /// The pane that each screen was left on, in the order of [`Screen::ALL`].
    panes_left: [usize; Screen::ALL.len()],
    /// The status line.
    pub status: Status,
    /// The address that the stacks screen starts a stack on. Empty selects the test signal of the stack.
    pub source: String,
    /// The submode of the stacks that start, or `None` for their default.
    pub submode: Option<String>,
    /// The frequency in Hz to tune the stacks that start to, or `None` for their default.
    pub freq_hz: Option<f64>,
    /// The region data, if a data folder and a country are given and the folder opened.
    pub regional: Option<Regional>,
    /// Why there is no region data, for the status line.
    pub region_note: Option<String>,
    /// The data folder that the regions are read from.
    pub data: Option<std::path::PathBuf>,
    /// The settings of the application that it keeps between runs.
    pub prefs: crate::prefs::Prefs,
    /// The city of the region, for the known stations.
    pub city: Option<String>,
    /// The session screen lists the advanced settings of the stack too.
    pub show_advanced: bool,
    /// The stacks of the catalog.
    pub stacks: Vec<StackRow>,
    /// The selected lines.
    pub sel: Selection,
    /// The level and the mute of the speaker.
    pub meter: Arc<Meter>,
    /// The audio output that a speaker opens: the default device of the system.
    pub output: Arc<dyn Output>,
    /// The application ends after this tick.
    pub quit: bool,
    /// The shown session has not chosen its first table yet: its tables are not read.
    first_table: bool,
    /// The source and the stack that the application was started with, until the first tick opens and starts them.
    launch: Option<(Option<String>, Option<String>)>,
    /// The radio that a thread opens now.
    opening: Option<Opening>,
    /// The screen and the object whose values were last read, and when.
    values_of: Option<(Screen, Target)>,
    values_read: Instant,
    stacks_counter: u64,
    bins: Vec<f32>,
}

impl App {
    /// Makes the application on `engine` and carries out the actions the command line requests.
    ///
    /// # Errors
    ///
    /// The code of the library if the engine cannot be read.
    pub fn new(engine: Handle, start: Start) -> api::Result<App> {
        let event_types = engine.event_types()?;
        let source = start
            .source
            .filter(|s| !matches!(s.as_str(), "synth" | "synth:"));
        let mut app = App {
            engine: Owner::new("engine".into(), engine),
            radios: Vec::new(),
            live: None,
            parked: Vec::new(),
            service: None,
            event_types,
            events: VecDeque::new(),
            screen: Screen::Stacks,
            losses: (0, 0),
            losses_rose: None,
            target: Target::Engine,
            pane: 0,
            mode: Mode::Normal,
            form: None,
            radio_form: None,
            help: false,
            reply: Vec::new(),
            chosen_row: None,
            row_key: None,
            rows: HashMap::new(),
            lines: HashMap::new(),
            panes_left: [0; Screen::ALL.len()],
            status: Status::default(),
            source: source.clone().unwrap_or_default(),
            submode: start.submode.clone(),
            freq_hz: start.freq_hz,
            regional: None,
            region_note: None,
            data: start.data.clone(),
            prefs: crate::prefs::Prefs::load(start.prefs.as_deref()),
            city: start.city.clone(),
            show_advanced: false,
            stacks: Vec::new(),
            sel: Selection::default(),
            meter: Arc::new(Meter::default()),
            output: Arc::new(Device),
            quit: false,
            first_table: false,
            launch: None,
            opening: None,
            values_of: None,
            values_read: Instant::now(),
            stacks_counter: 0,
            bins: Vec::new(),
        };
        app.engine.poll()?;
        app.engine.load()?;
        app.engine.read_values()?;
        app.local_time();
        // `--region` comes first, then the region of the last run.
        let region = start.region.clone().or_else(|| app.prefs.region.clone());
        match (&start.data, &region) {
            (Some(_), Some(region)) => {
                if let Err(e) = app.set_region(region) {
                    app.region_note = Some(e);
                }
            }
            (Some(_), None) => {
                app.region_note = Some(
                    "no region chosen: g chooses the country of the channels and the scan".into(),
                );
            }
            _ => {}
        }
        app.refresh_stacks();
        // The radio is opened and the stack is started at the first tick, after the first screen is drawn: opening a
        // radio on a network takes up to a second, and the screen must not wait for it.
        match (&source, &start.stack) {
            (Some(address), Some(stack)) => app.say(format!("opening {address}, starting {stack}")),
            (None, Some(stack)) => app.say(format!("starting {stack}")),
            (Some(address), None) => app.say(format!("opening {address}")),
            (None, None) => {}
        }
        app.launch = Some((source, start.stack)).filter(|(a, s)| a.is_some() || s.is_some());
        Ok(app)
    }

    /// Opens the radio and starts the stack that the application was started with.
    fn launch(&mut self, source: Option<String>, stack: Option<String>) {
        match (source, stack) {
            (_, Some(stack)) => {
                if let Some(i) = self.stacks.iter().position(|s| s.id == stack) {
                    self.sel.stack = i;
                }
                self.start(&stack);
            }
            (Some(address), None) => {
                self.screen = Screen::Radios;
                if opens_at_once(&address) {
                    if let Err(e) = self.open_radio(&address) {
                        self.fail(e);
                    }
                } else {
                    self.open_radio_on_thread(&address, Then::Radios);
                }
            }
            (None, None) => {}
        }
    }

    /// Shows `text` in the status line.
    pub fn say(&mut self, text: impl Into<String>) {
        self.status = Status {
            text: text.into(),
            error: false,
        };
    }

    /// Shows the error `e` in the status line.
    pub fn fail(&mut self, e: impl Display) {
        self.status = Status {
            text: e.to_string(),
            error: true,
        };
    }

    /// Returns the target object, if it exists.
    pub fn target(&self) -> Option<&Owner> {
        match self.target {
            Target::Engine => Some(&self.engine),
            Target::Radio(i) => self.radios.get(i),
            Target::Session => self.live.as_ref().map(|l| &l.session),
            Target::Stack => self.live.as_ref().map(|l| &l.stack),
            Target::Service => self.service.as_ref(),
        }
    }

    fn target_mut(&mut self) -> Option<&mut Owner> {
        match self.target {
            Target::Engine => Some(&mut self.engine),
            Target::Radio(i) => self.radios.get_mut(i),
            Target::Session => self.live.as_mut().map(|l| &mut l.session),
            Target::Stack => self.live.as_mut().map(|l| &mut l.stack),
            Target::Service => self.service.as_mut(),
        }
    }

    /// Makes `t` the target and reads its declarations.
    fn retarget(&mut self, t: Target) {
        // Each object keeps its own line of the settings and of the commands.
        let left = (self.sel.setting, self.sel.command);
        if let Some(name) = self.target().map(|o| o.name.clone()) {
            self.lines.insert(name, left);
        }
        self.target = t;
        self.form = None;
        let name = self.target().map(|o| o.name.clone());
        let back = name.and_then(|n| self.lines.get(&n).copied());
        (self.sel.setting, self.sel.command) = back.unwrap_or((0, 0));
        let r = self.target_mut().map(Owner::load);
        if let Some(Err(e)) = r {
            self.fail(e);
        }
    }

    /// Returns the objects that have settings and commands: the engine, each open radio, the session and its stack,
    /// each service.
    pub fn objects(&self) -> Vec<ObjectLine> {
        let line = |target, service, kind, name| ObjectLine {
            target,
            service,
            kind,
            name,
        };
        let mut all = vec![line(Target::Engine, None, "Engine", String::new())];
        for (i, r) in self.radios.iter().enumerate() {
            let name = self.radio_address(i).unwrap_or_else(|| r.name.clone());
            all.push(line(Target::Radio(i), None, "Radio", name));
        }
        if let Some(l) = &self.live {
            all.push(line(Target::Session, None, "Session", l.stack_id.clone()));
            all.push(line(Target::Stack, None, "Stack", l.stack_id.clone()));
        }
        for (i, (_, name)) in self.services().into_iter().enumerate() {
            all.push(line(Target::Service, Some(i), "Service", name));
        }
        all
    }

    /// Returns the index in [`App::objects`] of the target.
    pub fn object_index(&self) -> usize {
        let service = (self.target == Target::Service).then_some(self.sel.service);
        self.objects()
            .iter()
            .position(|o| o.target == self.target && o.service == service)
            .unwrap_or(0)
    }

    /// Makes line `i` of [`App::objects`] the target.
    fn choose_object(&mut self, i: usize) {
        let Some(o) = self.objects().into_iter().nth(i) else {
            return;
        };
        if let Some(n) = o.service {
            self.sel.service = n;
            self.load_service();
        }
        self.retarget(o.target);
    }

    /// Returns the number of panes of the shown screen that take keys.
    pub fn panes(&self) -> usize {
        match self.screen {
            Screen::Session if self.entry_command().is_some() => 3,
            Screen::Session | Screen::Settings | Screen::Commands | Screen::Services => 2,
            _ => 1,
        }
    }

    /// Returns the command of the entry line of the shown session, if its stack has one.
    pub fn entry_command(&self) -> Option<&CommandDecl> {
        self.live.as_ref()?.entry_command()
    }

    /// Returns true if the keys go into the entry line.
    pub fn on_entry(&self) -> bool {
        self.screen == Screen::Session && self.pane == ENTRY_PANE && self.entry_command().is_some()
    }

    /// Returns the text arguments of the entry command that the shown table has a column for: the argument and the id
    /// of the column. The text of the line itself is not among them.
    pub fn entry_columns(&self) -> Vec<(&Decl, usize)> {
        let (Some(cmd), Screen::Session) = (self.entry_command(), self.screen) else {
            return Vec::new();
        };
        let tables = self.shown_tables();
        let Some(&(r, t)) = tables.get(self.sel.table) else {
            return Vec::new();
        };
        let columns = &r.decl.tables[t].columns;
        let args = cmd.args.iter().skip(1).filter(|a| a.ty == Type::String);
        args.filter_map(|a| Some((a, columns.iter().position(|c| c.key == a.key)?)))
            .collect()
    }

    /// Returns why a text of the entry line cannot go on air now, with what to do about it: the session cannot
    /// transmit, the transmit switch of the engine is off, or the engine reports what blocks the transmitter.
    pub fn entry_blocked(&self) -> Option<String> {
        let live = self.live.as_ref()?;
        let session = &live.session.results;
        if session.counter > 0 && field_text(session, BURSTS_SENT).is_none() {
            return Some("This session cannot transmit: its radio has no transmitter".to_owned());
        }
        let e = &self.engine;
        let switch = self.engine_setting(TRANSMIT_ALLOWED);
        if let Some(i) = switch.filter(|i| e.values.get(*i) == Some(&Value::Bool(false))) {
            let label = &e.settings[i].label;
            return Some(format!(
                "Transmit is off: switch on \"{label}\" in the controls"
            ));
        }
        live.tx_blocked()
    }

    /// Opens the session on its first table, once the tables of the stack are read: a session with an entry line
    /// opens on the table of the messages, if the stack has it.
    fn choose_first_table(&mut self) {
        let Some(l) = self.live.as_ref().filter(|_| self.first_table) else {
            return;
        };
        let r = &l.stack.results;
        if r.decl.tables.is_empty() {
            return;
        }
        self.first_table = false;
        if let Some(t) = r.table_id(MESSAGES).filter(|_| l.entry_command().is_some()) {
            self.sel.table = t;
        }
    }

    /// Builds the stacks list again if the engine's results changed.
    fn refresh_stacks(&mut self) {
        if self.stacks_counter != self.engine.results.counter {
            self.stacks_counter = self.engine.results.counter;
            self.stacks = views::stacks(&self.engine.results);
        }
    }

    /// Reads what changed: the results, the spectrum, the sound and the events. Nothing here waits for a session.
    pub fn tick(&mut self) {
        if let Some((source, stack)) = self.launch.take() {
            self.launch(source, stack);
        }
        self.tick_opening();
        if let Err(e) = self.engine.poll() {
            self.fail(e);
        }
        self.refresh_stacks();
        for r in &mut self.radios {
            // A failed read marks a dead radio; the radios screen shows it.
            let _ = r.poll();
        }
        if let Some(s) = &mut self.service {
            let _ = s.poll();
        }
        self.tick_live();
        self.note_losses();
        self.choose_first_table();
        if self.pane >= self.panes() {
            self.pane = 0;
        }
        for l in &mut self.parked {
            // A failed read marks a dead session; the application ends it when its radio closes.
            let _ = l.session.poll();
        }
        self.follow_row();
        let mut events = std::mem::take(&mut self.events);
        let mut first_error = None;
        let owners = std::iter::once(&mut self.engine)
            .chain(self.radios.iter_mut())
            .chain(self.live.iter_mut().map(|l| &mut l.session))
            .chain(self.parked.iter_mut().map(|l| &mut l.session))
            .chain(self.service.iter_mut());
        let mut taken = 0;
        for o in owners {
            match o.take_events(&mut events) {
                Ok(n) => taken += n,
                Err(e) => {
                    first_error.get_or_insert(e);
                }
            }
        }
        self.events = events;
        if let Some(why) = self.refused_burst(taken) {
            self.fail(format!("the burst did not go on air: {why}"));
        }
        if let Some(e) = first_error.filter(|e| !e.is_dead_handle()) {
            self.fail(e);
        }
        self.read_shown_values();
    }

    /// Notes when the samples lost or the gaps of the live session rose, for the warning of the status line.
    fn note_losses(&mut self) {
        let now = self.live.as_ref().map_or((0, 0), |l| {
            let r = &l.session.results;
            let int = |key| match crate::views::field(r, key) {
                Some(api::Value::I64(n)) => *n,
                _ => 0,
            };
            (int("session.dropped"), int("session.gaps"))
        });
        if now.0 > self.losses.0 || now.1 > self.losses.1 {
            self.losses_rose = Some(Instant::now());
        }
        self.losses = now;
    }

    /// True while the samples lost or the gaps rose in the last [`LOSS_WARNING`].
    pub fn losses_rising(&self) -> bool {
        self.losses_rose.is_some_and(|t| t.elapsed() < LOSS_WARNING)
    }

    /// Reads the values of the settings that the screen shows: when the screen or its object changes, and every
    /// [`VALUES_EVERY`] for a value that the engine changed itself. A write reads its object again at once.
    fn read_shown_values(&mut self) {
        let shown = (self.screen, self.target);
        let due = self.values_of != Some(shown) || self.values_read.elapsed() >= VALUES_EVERY;
        if !due {
            return;
        }
        self.values_of = Some(shown);
        self.values_read = Instant::now();
        if self.screen == Screen::Settings {
            if let Some(Err(e)) = self.target_mut().map(Owner::read_values) {
                self.fail(e);
            }
        }
        if self.screen == Screen::Session {
            if let Some(l) = self.live.as_mut().filter(|l| !l.closing) {
                // The controls show the values. A failed read marks a dead object; the head of the screen shows it.
                let _ = l.session.read_values();
                let _ = l.stack.read_values();
            }
        }
    }

    /// Returns why the newest burst among the last `taken` events did not go on air, or `None` if no burst among them
    /// was refused.
    fn refused_burst(&self, taken: usize) -> Option<String> {
        let ty = self.event_types.iter().position(|t| t.key == BURST)?;
        let mut fields = self.event_types[ty].fields.iter();
        let field = fields.position(|d| d.key == BURST_REFUSED)?;
        let new = self.events.iter().rev().take(taken);
        new.filter(|e| e.ty as usize == ty)
            .find_map(|e| match e.fields.get(field) {
                Some(Value::String(why)) => Some(why.clone()),
                _ => None,
            })
    }

    /// Reads the shown session: results and spectrum, starts its speaker, and closes it once it has stopped.
    fn tick_live(&mut self) {
        let Some(live) = &mut self.live else {
            return;
        };
        if live.closing {
            match live.session.handle.session_close() {
                Ok(()) => {
                    self.live = None;
                    self.session_gone();
                    self.say("the session is closed");
                }
                Err(e) if e.code == crate::sys::SDRKIT_E_STILL_RUNNING => {}
                Err(e) => {
                    self.live = None;
                    self.session_gone();
                    self.fail(e);
                }
            }
            return;
        }
        let mut error = None;
        if let Err(e) = live.session.poll() {
            error = Some(e);
        }
        if let Err(e) = live.stack.poll() {
            error.get_or_insert(e);
        }
        if let Some(scan) = &mut live.scan {
            let _ = scan.poll();
        }
        if live.session.dead.is_none() {
            match live.session.handle.spectrum_read(&mut self.bins) {
                Ok(Some(info)) => live
                    .waterfall
                    .push(&self.bins, info.span_hz, info.centre_hz),
                Ok(None) => {}
                Err(e) => {
                    error.get_or_insert(e);
                }
            }
        }
        let sound = live.start_sound(&self.engine.handle, &self.meter, &self.output);
        if let Some(e) = error {
            self.fail(e);
        } else if let Err(e) = sound {
            self.fail(e);
        }
    }

    /// Opens the radio at `address` with the driver's defaults and returns its index. The engine tunes it when a stack
    /// starts on it.
    fn open_radio(&mut self, address: &str) -> crate::api::Result<usize> {
        let h = self.engine.handle.radio_open(address, &[])?;
        Ok(self.adopt_radio(address, h))
    }

    /// Lists the radio that `h` opened at `address` and returns its index.
    fn adopt_radio(&mut self, address: &str, h: Handle) -> usize {
        let name = h.key().unwrap_or_else(|_| address.to_owned());
        self.radios.push(Owner::new(name, h));
        // The engine's `radios` table has the row of the radio after this read.
        let _ = self.engine.poll();
        self.say(format!("opened {address}"));
        self.radios.len() - 1
    }

    /// Opens the radio at `address` on a thread of its own, with the driver's defaults. The status line shows the
    /// progress, and [`App::tick`] carries out `then` once the radio is open. One radio opens at a time.
    fn open_radio_on_thread(&mut self, address: &str, then: Then) {
        if let Some(o) = &self.opening {
            let text = format!("{} still opens: wait for it", o.address);
            self.fail(text);
            return;
        }
        // The thread works through an engine handle of its own.
        let engine = self.engine.handle.key().and_then(|k| self.engine.handle.find(&k));
        let engine = match engine {
            Ok(h) => h,
            Err(e) => return self.opening_failed(&then, e),
        };
        let (tx, done) = std::sync::mpsc::channel();
        let at = address.to_owned();
        let spawned = std::thread::Builder::new()
            .name("sdrkit-tui-open".into())
            .spawn(move || {
                let _ = tx.send(engine.radio_open(&at, &[]));
            });
        if let Err(e) = spawned {
            self.fail(format!("no thread to open {address}: {e}"));
            return;
        }
        self.say(format!("opening {address}"));
        self.opening = Some(Opening {
            address: address.to_owned(),
            then,
            since: Instant::now(),
            done,
        });
    }

    /// Shows the progress of the radio that opens and, once it is open, carries out what follows.
    fn tick_opening(&mut self) {
        use std::sync::mpsc::TryRecvError;
        let Some(o) = &self.opening else {
            return;
        };
        let result = match o.done.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => {
                let text = format!("opening {}: {} s", o.address, o.since.elapsed().as_secs());
                self.say(text);
                return;
            }
            Err(TryRecvError::Disconnected) => {
                let text = format!("the thread that opened {} ended", o.address);
                self.opening = None;
                self.fail(text);
                return;
            }
        };
        let Some(Opening { address, then, .. }) = self.opening.take() else {
            return;
        };
        let i = match result {
            Ok(h) => self.adopt_radio(&address, h),
            Err(e) => return self.opening_failed(&then, e),
        };
        match then {
            Then::Radios => {}
            Then::Start(stack) => self.start_on(&stack, i, address),
            Then::Form => self.radio_from_form(i, address),
        }
    }

    /// Shows why a radio did not open: in the form that named it, else in the status line.
    fn opening_failed(&mut self, then: &Then, e: api::Error) {
        match (then, &mut self.radio_form) {
            (Then::Form, Some(form)) => form.error = e.to_string(),
            _ => self.fail(e),
        }
    }

    /// Makes radio `i`, which the form opened at `address`, the source of the stacks, and closes the form.
    fn radio_from_form(&mut self, i: usize, address: String) {
        self.sel.radio = i;
        self.radio_form = None;
        self.source = self.radio_address(i).unwrap_or(address);
        let name = &self.radios[i].name;
        let text = format!("opened {name} ({}): the stacks start on it", self.source);
        self.say(text);
    }

    /// Returns the address that opens radio `i`, `<driver>:<address>`, read from the engine's `radios` table.
    pub fn radio_address(&self, i: usize) -> Option<String> {
        let r = &self.engine.results;
        let t = r.table_id("radios")?;
        let key = &self.radios.get(i)?.name;
        let row = r.tables[t].rows.iter().find(|row| &row.key == key)?;
        let text = |column| match row.cells.get(r.column_id(t, column)?)? {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        };
        Some(format!("{}:{}", text("driver")?, text("address")?))
    }

    /// Returns the text that shows the source of the stacks: the test signal, or the radio and its address.
    pub fn source_text(&self) -> String {
        if self.source.is_empty() {
            return "the test signal of the stack".to_owned();
        }
        match self.open_radio_at(&self.source) {
            Some(i) => format!("{} ({})", self.radios[i].name, self.source),
            None => format!("{} (opens when a stack starts)", self.source),
        }
    }

    /// Returns the stack of the session that reads the radio of key `radio`, and true if the application shows that
    /// session.
    pub fn session_on(&self, radio: &str) -> Option<(&str, bool)> {
        let shown = self.live.iter().map(|l| (l, true));
        let other = self.parked.iter().map(|l| (l, false));
        let mut all = shown.chain(other);
        let (l, shown) = all.find(|(l, _)| l.radio == radio)?;
        Some((l.stack_id.as_str(), shown))
    }

    /// Returns the index of the open radio of address `address`.
    fn open_radio_at(&self, address: &str) -> Option<usize> {
        (0..self.radios.len()).find(|&i| {
            let open = self.radio_address(i);
            self.radios[i].dead.is_none() && open.is_some_and(|a| same_address(&a, address))
        })
    }

    /// Starts stack `stack` on the source radio, or on its test signal if the source is empty, and shows the new
    /// session. A session that reads the same radio ends first. A stack that starts on its test signal ends each
    /// session on a test signal first. A session on another radio keeps running.
    pub fn start(&mut self, stack: &str) {
        let address = if self.source.is_empty() {
            self.close_test_radios();
            format!("synth:{stack}")
        } else {
            self.source.clone()
        };
        let r = match self.open_radio_at(&address) {
            Some(r) => r,
            None if opens_at_once(&address) => match self.open_radio(&address) {
                Ok(r) => r,
                Err(e) => {
                    self.fail(e);
                    return;
                }
            },
            None => {
                // The stack starts when the radio is open.
                self.open_radio_on_thread(&address, Then::Start(stack.to_owned()));
                return;
            }
        };
        self.start_on(stack, r, address);
    }

    /// Starts stack `stack` on the open radio `r`, which was opened at `address`, and shows the new session.
    fn start_on(&mut self, stack: &str, r: usize, address: String) {
        let radio = self.radios[r].name.clone();
        self.end_sessions_on(&radio);
        // Region data that cannot be used does not stop the start: the status line says why it was left out.
        let (preset, refused) = match self.preset(stack) {
            Ok(p) => (p, None),
            Err(e) => (None, Some(e)),
        };
        let (freq_hz, settings) = match &preset {
            Some((p, settings)) => (Some(p.frequency_hz), settings.as_slice()),
            None => (self.freq_hz, &[][..]),
        };
        let started = self.engine.handle.stack_start(
            stack,
            self.submode.as_deref(),
            freq_hz,
            settings,
            &self.radios[r].handle,
        );
        let session = match started {
            Ok(s) => s,
            Err(e) => {
                self.fail(e);
                return;
            }
        };
        let stack_handle = match session.stack() {
            Ok(h) => h,
            Err(e) => {
                self.fail(e);
                return;
            }
        };
        let name = session.key().unwrap_or_else(|_| "session".into());
        let tuned = session.setting_find(TUNED).ok();
        let scan = self.engine.handle.find(SCAN).ok();
        let spectrum = session
            .setting_find("spectrum")
            .and_then(|id| session.set(id, &Value::Bool(true)));
        self.park();
        self.live = Some(Live {
            stack: Owner::new(format!("stack {stack} of {name}"), stack_handle),
            session: Owner::new(name, session),
            stack_id: stack.to_owned(),
            radio: radio.clone(),
            address: address.clone(),
            speaker: None,
            sound_failed: false,
            waterfall: Waterfall::default(),
            closing: false,
            tuned,
            known: None,
            scan: scan.map(|h| Owner::new(SCAN.to_owned(), h)),
            entry: Entry::default(),
        });
        if let Some(l) = &mut self.live {
            let owners = [&mut l.session, &mut l.stack];
            for o in owners.into_iter().chain(l.scan.as_mut()) {
                // An object that cannot be read has no lines in the controls.
                let _ = o.load().and_then(|()| o.read_values());
            }
        }
        let known = self
            .regional
            .as_ref()
            .map_or(Ok(None), |r| r.known_stations(stack));
        let mut said = format!("started {stack} on {radio} ({address})");
        if let (Some((p, _)), Some(r)) = (&preset, &self.regional) {
            said.push_str(&format!(
                " at {} MHz from the region data of {}",
                p.frequency_hz / 1e6,
                r.region.of_stack(stack)
            ));
        }
        if let Some(e) = refused {
            said.push_str(&format!("; {e}: started without the region data"));
        }
        if let Some(note) = &self.region_note {
            said.push_str(&format!("; {note}"));
        }
        self.reset_session_view();
        self.screen = Screen::Session;
        match (spectrum, known) {
            (Ok(()), Ok(known)) => {
                if let Some(l) = &mut self.live {
                    l.known = known;
                }
                self.say(said);
            }
            (Err(e), _) => self.fail(e),
            (_, Err(e)) => self.fail(e),
        }
    }

    /// Makes the country `code` the region: opens the data folder, and gives the engine the channels of a scan and the
    /// frequency entries of the country. The region data stays as it was if the folder or the code cannot be used.
    ///
    /// # Errors
    ///
    /// A sentence: no data folder, a folder that does not open, or a code that the data has no plans for.
    pub fn set_region(&mut self, code: &str) -> Result<(), String> {
        let Some(folder) = self.data.clone() else {
            return Err("no data folder: --data names it".into());
        };
        let r = Regional::open(&folder, code, self.city.as_deref())?;
        let country = Some(r.region.country().to_owned());
        if self.prefs.region != country {
            self.prefs.region = country;
            if let Err(e) = self.prefs.save() {
                self.fail(format!("the region is not kept for the next run: {e}"));
            }
        }
        self.region_note = self
            .engine
            .handle
            .load_presets(&folder, r.region.country())
            .err()
            .map(|e| e.message);
        self.regional = Some(r);
        Ok(())
    }

    /// Opens the list of the countries that the data folder has plans for.
    fn pick_region(&mut self) {
        let Some(folder) = self.data.clone() else {
            self.fail("no data folder: --data names it");
            return;
        };
        let regions = match known::regions(&folder) {
            Ok(r) => r,
            Err(e) => {
                self.fail(e);
                return;
            }
        };
        let now = self.regional.as_ref().map(|r| r.region.country());
        let sel = regions.iter().position(|r| Some(r.code.as_str()) == now);
        self.mode = Mode::Pick {
            what: EditFor::Region,
            options: regions.iter().map(|r| r.line()).collect(),
            keys: regions.into_iter().map(|r| r.code).collect(),
            sel: sel.unwrap_or(0),
            find: Find::default(),
        };
    }

    /// Opens the list of the channels that the region data has for `stack`, each with its label, its frequency and
    /// its width. The chosen channel goes to `what`.
    fn pick_channel(&mut self, stack: &str, what: EditFor) {
        let Some(r) = &self.regional else {
            self.fail("no region chosen: g chooses the country of the channels");
            return;
        };
        let channels = match r.channels(stack) {
            Ok(c) => c,
            Err(e) => {
                self.fail(e);
                return;
            }
        };
        if channels.is_empty() {
            let region = r.region.of_stack(stack).to_string();
            self.fail(format!("the data of {region} has no channels for {stack}"));
            return;
        }
        let now = match what {
            EditFor::Channel => self.tuned_hz(),
            _ => self.freq_hz,
        };
        let near = |hz: f64| now.map_or(f64::MAX, |n| (n - hz).abs());
        let sel = (0..channels.len())
            .min_by(|&a, &b| near(channels[a].1).total_cmp(&near(channels[b].1)))
            .unwrap_or(0);
        self.mode = Mode::Pick {
            what,
            options: channels.iter().map(|c| c.0.clone()).collect(),
            keys: channels.iter().map(|c| c.1.to_string()).collect(),
            sel,
            find: Find::default(),
        };
    }

    /// The frequency that the shown session listens to.
    fn tuned_hz(&self) -> Option<f64> {
        let l = self.live.as_ref()?;
        match l.session.values.get(l.tuned? as usize)? {
            Value::Hz(hz) => Some(*hz),
            _ => None,
        }
    }

    /// Gives the running stack the settings that the region data has for it. The frequency of the session stays.
    fn apply_region_to_session(&mut self) -> Result<(), String> {
        let Some(stack) = self.live.as_ref().map(|l| l.stack_id.clone()) else {
            return Ok(());
        };
        let preset = self.preset(&stack)?;
        let known = self
            .regional
            .as_ref()
            .map_or(Ok(None), |r| r.known_stations(&stack))?;
        let Some(l) = &mut self.live else {
            return Ok(());
        };
        l.known = known;
        for (id, v) in preset.iter().flat_map(|(_, s)| s) {
            l.stack.handle.set(*id, v).map_err(|e| e.message)?;
        }
        let _ = l.stack.read_values();
        Ok(())
    }

    /// The preset that the region data has for `stack`, with its settings as values of the stack's settings. `None`
    /// without region data, or if the data has nothing for the stack in the region.
    fn preset(&self, stack: &str) -> Result<Option<StartFrom>, String> {
        let Some(r) = &self.regional else {
            return Ok(None);
        };
        let preset = r
            .preset(stack, self.freq_hz, self.submode.as_deref())
            .map_err(|e| e.to_string())?;
        let Some(p) = preset else {
            return Ok(None);
        };
        let settings = r.settings(&self.engine.handle, stack, self.freq_hz, self.submode.as_deref())?;
        Ok(Some((p, settings)))
    }

    /// Moves the keys of the session screen to its first control and its first table.
    fn reset_session_view(&mut self) {
        self.first_table = true;
        self.pane = 0;
        self.sel.control = 0;
        self.sel.table = 0;
        self.sel.row = 0;
        self.row_key = None;
        self.sel.column = 0;
    }

    /// Stops the shown session. It closes on a later tick, once it has stopped.
    fn stop(&mut self) {
        let Some(live) = &mut self.live else {
            return;
        };
        live.speaker = None;
        match live.session.handle.session_stop() {
            Ok(()) => {
                live.closing = true;
                self.say("stopping the session");
            }
            Err(e) => self.fail(e),
        }
    }

    /// Stops and closes the shown session, waiting up to five seconds for it to stop, then shows the next session
    /// that runs.
    pub fn stop_now(&mut self) {
        let Some(mut live) = self.live.take() else {
            return;
        };
        if let Err(e) = end_session(&mut live) {
            self.fail(e);
        }
        self.session_gone();
    }

    /// Shows the next session that runs, after the shown session closed. The settings and commands screens leave the
    /// objects of the closed session.
    fn session_gone(&mut self) {
        if matches!(self.target, Target::Session | Target::Stack) {
            self.target = Target::Engine;
        }
        if self.live.is_none() && !self.parked.is_empty() {
            self.live = Some(self.parked.remove(0));
            self.reset_session_view();
        }
    }

    /// Moves the shown session to the sessions that run without a screen. Its speaker stops. A session that waits to
    /// close closes first.
    fn park(&mut self) {
        if self.live.as_ref().is_some_and(|l| l.closing) {
            self.stop_now();
        }
        if let Some(mut l) = self.live.take() {
            l.speaker = None;
            self.parked.push(l);
        }
    }

    /// Shows the session `next`, which the caller removed from the parked sessions. The speaker moves to it.
    fn show_session(&mut self, next: Live) {
        self.park();
        let text = format!(
            "showing {} on {} ({})",
            next.stack_id, next.radio, next.address
        );
        self.live = Some(next);
        self.reset_session_view();
        if matches!(self.target, Target::Session | Target::Stack) {
            self.retarget(self.target);
        }
        self.say(text);
    }

    /// Shows the next session that runs.
    fn next_session(&mut self) {
        if self.parked.is_empty() {
            self.say("no other session runs");
            return;
        }
        let next = self.parked.remove(0);
        self.show_session(next);
    }

    /// Shows the session that reads the selected radio.
    fn show_session_of_radio(&mut self) {
        let Some(radio) = self.radios.get(self.sel.radio).map(|r| r.name.clone()) else {
            return;
        };
        if let Some(i) = self.parked.iter().position(|l| l.radio == radio) {
            let next = self.parked.remove(i);
            self.show_session(next);
        } else if !self.live.as_ref().is_some_and(|l| l.radio == radio) {
            self.say(format!("no session reads {radio}"));
            return;
        }
        self.show(Screen::Session);
    }

    /// Stops and closes each session that reads the radio of key `radio`, waiting up to five seconds for each to
    /// stop.
    fn end_sessions_on(&mut self, radio: &str) {
        let mut i = 0;
        while i < self.parked.len() {
            if self.parked[i].radio == radio {
                let mut l = self.parked.remove(i);
                if let Err(e) = end_session(&mut l) {
                    self.fail(e);
                }
            } else {
                i += 1;
            }
        }
        if self.live.as_ref().is_some_and(|l| l.radio == radio) {
            self.stop_now();
        }
    }

    /// Ends each session on a test signal and closes each radio that sends a test signal.
    fn close_test_radios(&mut self) {
        let mut i = 0;
        while i < self.radios.len() {
            let test = self.radio_address(i);
            if test.is_some_and(|a| a.starts_with("synth:")) {
                let key = self.radios[i].name.clone();
                self.end_sessions_on(&key);
                let _ = self.radios[i].handle.radio_close();
                self.forget_radio(i);
            } else {
                i += 1;
            }
        }
    }

    /// Removes radio `i` from the open radios. The stacks start on the test signal again if the radio was their
    /// source.
    fn forget_radio(&mut self, i: usize) {
        if self
            .radio_address(i)
            .is_some_and(|a| same_address(&a, &self.source))
        {
            self.source.clear();
        }
        self.radios.remove(i);
        if self.sel.radio > i {
            self.sel.radio -= 1;
        }
        self.sel.radio = self.sel.radio.min(self.radios.len().saturating_sub(1));
        self.target = match self.target {
            Target::Radio(j) if j == i => Target::Engine,
            Target::Radio(j) if j > i => Target::Radio(j - 1),
            t => t,
        };
    }

    /// Makes the selected radio the source of the stacks.
    fn use_radio(&mut self) {
        let i = self.sel.radio;
        let Some(r) = self.radios.get(i) else {
            return;
        };
        let name = r.name.clone();
        match self.radio_address(i).filter(|_| r.dead.is_none()) {
            Some(a) => {
                self.say(format!("the stacks start on {name} ({a})"));
                self.source = a;
            }
            None => self.fail(format!("{name} is closed: c removes it from the list")),
        }
    }

    /// Carries out key `k`.
    pub fn key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.help {
            self.help = false;
            return;
        }
        if self.radio_form.is_some() {
            self.key_radio_form(k.code);
            return;
        }
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Edit { what, mut text } => match k.code {
                KeyCode::Enter => {
                    self.status.error = false;
                    self.commit_text(what, &text);
                    // A refused value stays in the box, with the reason, to be corrected.
                    if self.status.error && matches!(self.mode, Mode::Normal) {
                        self.mode = Mode::Edit { what, text };
                    }
                }
                KeyCode::Esc => self.status.error = false,
                KeyCode::Backspace => {
                    text.pop();
                    self.mode = Mode::Edit { what, text };
                }
                KeyCode::Char(c) => {
                    text.push(c);
                    self.mode = Mode::Edit { what, text };
                }
                _ => self.mode = Mode::Edit { what, text },
            },
            Mode::Pick {
                what,
                options,
                keys,
                mut sel,
                mut find,
            } => match k.code {
                KeyCode::Enter => match keys.get(sel) {
                    Some(key) => self.commit_value(what, Value::String(key.clone())),
                    None => self.commit_value(what, Value::Enum(sel as u32)),
                },
                KeyCode::Esc => {}
                code => {
                    if Find::takes(code) {
                        sel = find.key(code, &options).unwrap_or(sel);
                    } else {
                        find.clear();
                        move_sel(&mut sel, options.len(), code);
                    }
                    self.mode = Mode::Pick {
                        what,
                        options,
                        keys,
                        sel,
                        find,
                    };
                }
            },
            Mode::Find { mut find, from } => match k.code {
                KeyCode::Enter => {}
                KeyCode::Esc => self.find_select(from),
                code if Find::takes(code) => {
                    let line = find.key(code, &self.find_lines());
                    self.find_select(line.unwrap_or(from));
                    self.mode = Mode::Find { find, from };
                }
                code => self.key_normal(code),
            },
            Mode::Normal => self.key_normal(k.code),
        }
        if self.mode == Mode::Normal && self.form.as_ref().is_some_and(|f| f.quick) {
            self.form = None;
        }
    }

    /// Shows screen `s`.
    fn show(&mut self, s: Screen) {
        let objects = |s| matches!(s, Screen::Settings | Screen::Commands);
        let index = |s: Screen| Screen::ALL.iter().position(|x| *x == s).unwrap_or(0);
        // Each screen keeps its pane, and the session and each service keep their table and its row.
        self.panes_left[index(self.screen)] = self.pane;
        self.keep_view();
        if !(objects(s) && objects(self.screen)) {
            self.pane = self.panes_left[index(s)];
        }
        self.screen = s;
        self.restore_view();
        if objects(s) {
            if self.target().is_none() {
                self.target = Target::Engine;
            }
            self.retarget(self.target);
        }
        if s == Screen::Services {
            self.load_service();
        }
    }

    /// Moves the keys to the next pane of the screen, or to the previous one for a negative `step`. After the last
    /// pane comes the first one. A screen with one pane keeps the keys where they are. The entry line of the session
    /// screen comes before the tables, as on the screen.
    fn next_pane(&mut self, step: isize) {
        let order: &[usize] = match self.panes() {
            3 => &[0, ENTRY_PANE, TABLES_PANE],
            2 => &[0, 1],
            _ => &[0],
        };
        let at = order.iter().position(|p| *p == self.pane).unwrap_or(0);
        let n = order.len() as isize;
        self.pane = order[(at as isize + step).rem_euclid(n) as usize];
    }

    /// Carries out key `code` in the entry line.
    fn key_entry(&mut self, code: KeyCode) {
        if matches!(code, KeyCode::Tab | KeyCode::BackTab) {
            self.next_pane(if code == KeyCode::Tab { 1 } else { -1 });
            return;
        }
        let Some(live) = &mut self.live else {
            return;
        };
        match live.entry.key(code) {
            EntryKey::Stay => {}
            EntryKey::Leave => self.pane = TABLES_PANE,
            EntryKey::Run => self.run_entry(),
        }
    }

    /// Runs the entry command with the text of the line and the chosen values; every other argument has its default.
    /// A command that ran clears the line. A refused text stays in the line with the message of the engine.
    fn run_entry(&mut self) {
        let Some(live) = &mut self.live else {
            return;
        };
        let Some(cmd) = live.entry_command() else {
            return;
        };
        let entry = &live.entry;
        if entry.text.trim().is_empty() {
            let text = format!("{}: type it first", cmd.args[0].label);
            live.entry.refused(text);
            return;
        }
        let mut args = vec![(0, Value::String(entry.text.clone()))];
        for (id, a) in (0..).zip(&cmd.args).skip(1) {
            if let Some(v) = entry.chosen(&a.key) {
                args.push((id, Value::String(v.to_owned())));
            }
        }
        let said = entry_done(cmd, entry);
        let r = live.stack.handle.invoke(cmd.id, &args).map(|_| ());
        let r = r.map_err(|e| live.stack.note(e));
        let blocked = self.entry_blocked();
        self.entry_ran(said, r, blocked);
    }

    /// Notes the result `r` of the entry command. A command that ran clears the line, and the status line shows
    /// `said`. A refused text stays in the line with the message of the engine. A text that the command took while
    /// `blocked` tells why nothing goes on air stays too, with that reason, and so does a text that the engine refused
    /// because the transmitter is off.
    pub fn entry_ran(&mut self, said: String, r: api::Result<()>, blocked: Option<String>) {
        let Some(live) = &mut self.live else {
            return;
        };
        match (r, blocked) {
            (Ok(()), None) => {
                live.entry.done();
                self.say(said);
            }
            (Ok(()), Some(why)) => live.entry.refused(format!("Not on air. {why}")),
            (Err(e), Some(why)) if e.code == crate::sys::SDRKIT_E_TRANSMIT_OFF => {
                live.entry.refused(format!("Not on air. {why}"));
            }
            (Err(e), _) if e.message.is_empty() => live.entry.refused(e.to_string()),
            (Err(e), _) => live.entry.refused(e.message),
        }
    }

    /// Makes the values of the selected row the chosen values of the entry line, for each argument that
    /// [`App::entry_columns`] returns. If the row holds the chosen values already, the arguments have no chosen value
    /// after the call. Returns false if the shown table has no such column or the row has no text in it.
    fn choose_for_entry(&mut self) -> bool {
        let tables = self.shown_tables();
        let row = tables
            .get(self.sel.table)
            .and_then(|(r, t)| r.tables[*t].rows.get(self.sel.row));
        let Some(row) = row else {
            return false;
        };
        let found = self.entry_columns().into_iter().filter_map(|(a, c)| {
            let text = match row.cells.get(c) {
                Some(Value::String(s)) if !s.is_empty() => s.clone(),
                _ => return None,
            };
            Some((a.key.clone(), a.label.to_lowercase(), text))
        });
        let found: Vec<_> = found.collect();
        let Some(live) = self.live.as_mut().filter(|_| !found.is_empty()) else {
            return false;
        };
        let mut said = Vec::new();
        for (key, label, text) in found {
            if live.entry.choose(&key, &text) {
                said.push(format!("{label}: {text}"));
            } else {
                said.push(format!("{label}: not chosen, the command uses its default"));
            }
        }
        self.say(said.join("  "));
        true
    }

    /// Carries out key `code` in the form that opens a radio.
    fn key_radio_form(&mut self, code: KeyCode) {
        let Some(form) = &mut self.radio_form else {
            return;
        };
        match form.key(code, &self.engine.handle, &self.engine.results) {
            Outcome::Stay => {}
            Outcome::Close => self.radio_form = None,
            Outcome::Open(address) if opens_at_once(&address) => match self.open_radio(&address) {
                Ok(i) => self.radio_from_form(i, address),
                Err(e) => self.opening_failed(&Then::Form, e),
            },
            Outcome::Open(address) => self.open_radio_on_thread(&address, Then::Form),
        }
    }

    /// Shows the list of sources of the stacks screen: the test signal, each open radio, a radio to open.
    fn pick_source(&mut self) {
        let mut options = vec!["The test signal of the stack".to_owned()];
        let mut keys = vec![String::new()];
        for i in 0..self.radios.len() {
            if self.radios[i].dead.is_some() {
                continue;
            }
            if let Some(a) = self.radio_address(i) {
                options.push(format!("{}  {a}", self.radios[i].name));
                keys.push(a);
            }
        }
        options.push("Open a radio…".to_owned());
        keys.push(NEW_RADIO.to_owned());
        let sel = keys.iter().position(|k| *k == self.source).unwrap_or(0);
        self.mode = Mode::Pick {
            what: EditFor::Source,
            options,
            keys,
            sel,
            find: Find::default(),
        };
    }

    fn key_normal(&mut self, code: KeyCode) {
        if self.on_entry() {
            self.key_entry(code);
            return;
        }
        if let KeyCode::Char(c @ '1'..='8') = code {
            self.show(Screen::ALL[c as usize - '1' as usize]);
            return;
        }
        match code {
            KeyCode::Tab | KeyCode::BackTab if self.form.is_none() => {
                self.next_pane(if code == KeyCode::Tab { 1 } else { -1 });
                return;
            }
            KeyCode::Char('?') if self.form.is_none() => {
                self.help = true;
                return;
            }
            KeyCode::Char('q') if self.form.is_none() => {
                self.quit = true;
                return;
            }
            KeyCode::Char('/') if self.form.is_none() => {
                self.mode = Mode::Find {
                    find: Find::default(),
                    from: self.find_line(),
                };
                return;
            }
            KeyCode::Char('m') if self.form.is_none() => {
                let on = !self.meter.muted();
                self.meter.set_muted(on);
                self.say(if on { "sound muted" } else { "sound on" });
                return;
            }
            _ => {}
        }
        match self.screen {
            Screen::Stacks => match code {
                KeyCode::Char('a') => self.pick_source(),
                KeyCode::Char('g') => self.pick_region(),
                KeyCode::Char('c') => {
                    if let Some(id) = self.stacks.get(self.sel.stack).map(|s| s.id.clone()) {
                        self.pick_channel(&id, EditFor::StartChannel);
                    }
                }
                KeyCode::Char('f') => {
                    self.mode = Mode::Edit {
                        what: EditFor::StartFrequency,
                        text: String::new(),
                    };
                }
                KeyCode::Enter => {
                    if let Some(id) = self.stacks.get(self.sel.stack).map(|s| s.id.clone()) {
                        self.start(&id);
                    }
                }
                c => move_sel(&mut self.sel.stack, self.stacks.len(), c),
            },
            Screen::Radios => match code {
                KeyCode::Char('a') => {
                    self.radio_form = Some(RadioForm::new(&self.engine.results));
                }
                KeyCode::Char('c') => self.close_radio(),
                KeyCode::Char('u') => self.use_radio(),
                KeyCode::Char('s') => self.show_session_of_radio(),
                KeyCode::Enter if self.sel.radio < self.radios.len() => {
                    self.target = Target::Radio(self.sel.radio);
                    self.show(Screen::Settings);
                    self.pane = 1;
                }
                c => move_sel(&mut self.sel.radio, self.radios.len(), c),
            },
            Screen::Session => match code {
                KeyCode::Char('s') => self.stop(),
                KeyCode::Char('n') => self.next_session(),
                KeyCode::Char('f') => self.edit_frequency(),
                KeyCode::Char('g') => self.pick_region(),
                KeyCode::Char('c') => {
                    if let Some(id) = self.live.as_ref().map(|l| l.stack_id.clone()) {
                        self.pick_channel(&id, EditFor::Channel);
                    }
                }
                KeyCode::Char('i') if self.entry_command().is_some() => self.pane = ENTRY_PANE,
                c if table_key(c, self.pane == TABLES_PANE).is_some() => self.key_table(c),
                KeyCode::Char('a') => {
                    self.show_advanced = !self.show_advanced;
                    self.sel.control = 0;
                    self.say(if self.show_advanced {
                        "every setting of the stack is shown"
                    } else {
                        "the advanced settings are hidden"
                    });
                }
                KeyCode::Enter if self.pane == 0 => self.act_on_control(),
                c if self.pane == 0 => {
                    let n = self.controls().len();
                    move_sel(&mut self.sel.control, n, c);
                }
                KeyCode::Enter => self.act_on_row(),
                c => self.move_row(c),
            },
            Screen::Services if table_key(code, self.pane == 1).is_some() => self.key_table(code),
            Screen::Services if self.pane == 0 => match code {
                KeyCode::Right => self.pane = 1,
                KeyCode::Enter if self.service.is_some() => {
                    self.target = Target::Service;
                    self.show(Screen::Settings);
                    self.pane = 1;
                }
                c => {
                    let n = self.services().len();
                    move_sel(&mut self.sel.service, n, c);
                    self.load_service();
                }
            },
            Screen::Services => match code {
                KeyCode::Enter => self.choose_row(),
                c => self.move_row(c),
            },
            Screen::Settings | Screen::Commands if self.pane == 0 => match code {
                KeyCode::Right | KeyCode::Enter => self.pane = 1,
                c => {
                    let mut i = self.object_index();
                    move_sel(&mut i, self.objects().len(), c);
                    self.choose_object(i);
                }
            },
            Screen::Settings => match code {
                KeyCode::Left => self.pane = 0,
                KeyCode::Enter => self.edit_setting(),
                c => {
                    let n = self.target().map_or(0, |o| o.settings.len());
                    move_sel(&mut self.sel.setting, n, c);
                }
            },
            Screen::Events => match code {
                KeyCode::Char('c') => self.events.clear(),
                c => move_sel(&mut self.sel.event_type, self.event_types.len(), c),
            },
            Screen::Commands => self.key_commands(code),
            // The load screen shows the published load and has no keys of its own.
            Screen::Load => {}
        }
    }

    fn key_commands(&mut self, code: KeyCode) {
        let commands = self.target().map_or(0, |o| o.commands.len());
        let Some(form) = &mut self.form else {
            match code {
                KeyCode::Left => self.pane = 0,
                KeyCode::Enter if self.sel.command < commands => {
                    let n = self
                        .target()
                        .map_or(0, |o| o.commands[self.sel.command].args.len());
                    self.form = Some(Form {
                        command: self.sel.command,
                        values: vec![None; n],
                        sel: 0,
                        quick: false,
                    });
                }
                c => move_sel(&mut self.sel.command, commands, c),
            }
            return;
        };
        let n = form.values.len();
        match code {
            KeyCode::Esc => self.form = None,
            KeyCode::Char('r') => self.run_command(),
            KeyCode::Char('d') if form.sel < n => form.values[form.sel] = None,
            KeyCode::Char('p') if form.sel < n => match &self.chosen_row {
                Some(key) => form.values[form.sel] = Some(Value::String(key.clone())),
                None => self.say("choose a row first: Enter on a row of a table"),
            },
            KeyCode::Enter if form.sel < n => self.edit_argument(),
            c => move_sel(&mut form.sel, n, c),
        }
    }

    /// Returns the declaration of the value that `what` edits.
    pub fn edit_decl(&self, what: EditFor) -> Option<&Decl> {
        match what {
            EditFor::Source
            | EditFor::StartFrequency
            | EditFor::Region
            | EditFor::Channel
            | EditFor::StartChannel => None,
            EditFor::Setting(id) => self.target()?.settings.get(id as usize),
            EditFor::Argument(n) => self.argument(n),
            EditFor::Frequency => {
                let l = self.live.as_ref()?;
                l.session.settings.get(l.tuned? as usize)
            }
        }
    }

    /// Returns the value that `what` edits as it is now, as text.
    pub fn edit_now(&self, what: EditFor) -> Option<String> {
        match what {
            EditFor::Source | EditFor::Region | EditFor::Channel | EditFor::StartChannel => None,
            EditFor::Setting(id) => {
                let o = self.target()?;
                let (d, v) = (o.settings.get(id as usize)?, o.values.get(id as usize)?);
                Some(format::value_text(v, d))
            }
            EditFor::Argument(n) => {
                let v = self.form.as_ref()?.values.get(n)?.as_ref()?;
                Some(format::value_text(v, self.argument(n)?))
            }
            EditFor::Frequency => Some(self.control_text(Control::Frequency).1),
            EditFor::StartFrequency => Some(match self.freq_hz {
                Some(hz) => {
                    let mut s = String::new();
                    format::hz(&mut s, hz);
                    s
                }
                None => "the frequency of each stack".to_owned(),
            }),
        }
    }

    /// Returns the index of the engine setting with `key`.
    fn engine_setting(&self, key: &str) -> Option<usize> {
        self.engine.settings.iter().position(|d| d.key == key)
    }

    /// Shows every time in local time. An engine whose time zone is still its default (UTC) takes the zone of this
    /// computer.
    fn local_time(&mut self) {
        let Some(i) = self.engine_setting("time_zone") else {
            return;
        };
        let now = match self.engine.values.get(i) {
            Some(Value::I64(m)) => *m,
            _ => 0,
        };
        let zone = if now == 0 { format::system_zone() } else { now };
        if zone != now && self.engine.handle.set(i as Id, &Value::I64(zone)).is_ok() {
            let _ = self.engine.read_values();
        }
        format::set_zone(zone);
    }

    /// Returns the label and the value of the engine setting with `key`, or empty texts if the engine does not have it.
    fn engine_text(&self, key: &str) -> (String, String) {
        let e = &self.engine;
        let shown = self.engine_setting(key).and_then(|i| {
            let d = e.settings.get(i)?;
            Some((d.label.clone(), format::value_text(e.values.get(i)?, d)))
        });
        shown.unwrap_or_default()
    }

    /// Opens the editor of the engine setting with `key`.
    fn edit_engine_setting(&mut self, key: &str) {
        if let Some(i) = self.engine_setting(key) {
            self.retarget(Target::Engine);
            self.sel.setting = i;
            self.edit_setting();
        }
    }

    /// Returns the lines of the controls pane of the session screen: the frequency, the scan, the transmit
    /// switch if the session can transmit, then the commands and the settings of the stack.
    pub fn controls(&self) -> Vec<Control> {
        let Some(l) = &self.live else {
            return Vec::new();
        };
        let mut all = Vec::new();
        if l.tuned.is_some() {
            all.push(Control::Frequency);
        }
        if l.scan.as_ref().is_some_and(|s| !s.commands.is_empty()) {
            all.push(Control::Scan);
        }
        if field_text(&l.session.results, BURSTS_SENT).is_some()
            && self.engine_setting(TRANSMIT_ALLOWED).is_some()
        {
            all.push(Control::Transmit);
        }
        all.extend((0..l.stack.commands.len()).map(Control::Command));
        let settings = l.stack.settings.iter().enumerate();
        all.extend(
            settings
                .filter(|(_, d)| !d.fixed && (self.show_advanced || !d.advanced))
                .map(|(i, _)| Control::Setting(i)),
        );
        all
    }

    /// Returns the name and the value of control `c`, as the controls pane shows them.
    pub fn control_text(&self, c: Control) -> (String, String) {
        let Some(l) = &self.live else {
            return (String::new(), String::new());
        };
        match c {
            Control::Frequency => {
                let id = l.tuned.map_or(0, |id| id as usize);
                let mut s = String::new();
                match (l.session.settings.get(id), l.session.values.get(id)) {
                    // A session that follows the centre of its input listens to the centre of the spectrum.
                    (_, Some(Value::Hz(hz))) if *hz == 0.0 && l.waterfall.span_hz > 0.0 => {
                        format::hz(&mut s, l.waterfall.centre_hz);
                    }
                    (Some(d), Some(v)) => format::value(&mut s, v, d),
                    _ => {}
                }
                ("Frequency".to_owned(), s)
            }
            Control::Scan => {
                let r = l.scan.as_ref().map(|s| &s.results);
                let runs = r.is_some_and(scan_runs);
                let mut s = r.and_then(|r| field_text(r, "state")).unwrap_or_default();
                if let Some(found) = r.and_then(|r| field_text(r, "found")) {
                    if runs || s == "Done" {
                        s.push_str(", found ");
                        s.push_str(&found);
                    }
                }
                let name = if runs {
                    "Stop the scan"
                } else {
                    "Scan the whole band"
                };
                (name.to_owned(), s)
            }
            Control::Transmit => self.engine_text(TRANSMIT_ALLOWED),
            Control::Command(n) => {
                let c = l.stack.commands.get(n);
                let name = c.map(|c| c.label.clone()).unwrap_or_default();
                let more = c.is_some_and(|c| !c.args.is_empty());
                (name, if more { "…" } else { "" }.to_owned())
            }
            Control::Setting(i) => match (l.stack.settings.get(i), l.stack.values.get(i)) {
                (Some(d), Some(v)) => {
                    // Two settings of a stack can have one label in two sections: the section tells them apart.
                    let twice = l.stack.settings.iter().filter(|o| !o.fixed);
                    let twice = twice.filter(|o| o.label == d.label).count() > 1;
                    let name = if twice && !d.section.is_empty() {
                        format!("{}: {}", d.section, d.label)
                    } else {
                        d.label.clone()
                    };
                    (name, format::value_text(v, d))
                }
                _ => (String::new(), String::new()),
            },
        }
    }

    /// Carries out the selected line of the controls pane.
    fn act_on_control(&mut self) {
        let Some(c) = self.controls().get(self.sel.control).copied() else {
            return;
        };
        match c {
            Control::Frequency => self.edit_frequency(),
            Control::Scan => self.scan(),
            Control::Transmit => self.edit_engine_setting(TRANSMIT_ALLOWED),
            Control::Command(n) => self.quick_command(n),
            Control::Setting(id) => {
                self.retarget(Target::Stack);
                self.sel.setting = id;
                self.edit_setting();
            }
        }
    }

    /// Opens the line that takes a new frequency for the session. The line starts empty.
    fn edit_frequency(&mut self) {
        if self.live.as_ref().is_some_and(|l| l.tuned.is_some()) {
            self.mode = Mode::Edit {
                what: EditFor::Frequency,
                text: String::new(),
            };
        }
    }

    /// Sets the frequency that the session listens to. A scan that runs stops first: it sets the frequency itself.
    fn tune(&mut self, hz: f64) {
        let scan = self.live.as_ref().and_then(|l| l.scan.as_ref());
        if scan.is_some_and(|s| scan_runs(&s.results)) {
            if let Err(e) = self.scan_command("stop") {
                self.fail(e);
                return;
            }
        }
        let Some(l) = &mut self.live else {
            return;
        };
        let Some(id) = l.tuned else {
            return;
        };
        let r = l.session.handle.set(id, &Value::Hz(hz));
        let _ = l.session.read_values();
        match r {
            Ok(()) => {
                let mut s = "tuned to ".to_owned();
                format::hz(&mut s, hz);
                self.say(s);
            }
            Err(e) => self.fail(e),
        }
    }

    /// Starts a scan of the band of the stack on the session, or stops the scan that runs.
    pub fn scan(&mut self) {
        let scan = self.live.as_ref().and_then(|l| l.scan.as_ref());
        let Some(runs) = scan.map(|s| scan_runs(&s.results)) else {
            return;
        };
        match self.scan_command(if runs { "stop" } else { "start" }) {
            Ok(()) if runs => self.say("the scan is stopped"),
            Ok(()) => {
                let said = self.scan_said();
                self.say(said);
            }
            Err(e) => self.fail(e),
        }
    }

    /// What the status line says when a scan starts: the channels of the region data, or the raster of the stack.
    fn scan_said(&self) -> String {
        let stack = self.live.as_ref().map_or("", |l| l.stack_id.as_str());
        let channels = self
            .regional
            .as_ref()
            .map(|r| (r, r.scan_channels(stack)));
        match channels {
            Some((r, n)) if n > 0 => format!(
                "scanning the {n} channels of {stack} in {}: the table of the hits shows them",
                r.region.of_stack(stack)
            ),
            Some((r, _)) => format!(
                "the data of {} has no channels for {stack}: scanning the range of the stack in steps of its width",
                r.region.of_stack(stack)
            ),
            None => "no region chosen (g): scanning the range of the stack in steps of its width"
                .to_owned(),
        }
    }

    /// Runs the command `key` of the scan service for the session. Does nothing if the engine has no such service or
    /// command.
    fn scan_command(&self, key: &str) -> api::Result<()> {
        let Some(l) = &self.live else {
            return Ok(());
        };
        let Some(scan) = &l.scan else {
            return Ok(());
        };
        let Some(cmd) = scan.commands.iter().find(|c| c.key == key) else {
            return Ok(());
        };
        let mut args = Vec::new();
        for (id, d) in (0..).zip(&cmd.args) {
            match d.key.as_str() {
                "session" => args.push((id, Value::String(l.session.name.clone()))),
                "stack" => args.push((id, Value::String(l.stack_id.clone()))),
                _ => {}
            }
        }
        scan.handle.invoke(cmd.id, &args).map(|_| ())
    }

    /// Runs command `n` of the stack. A command with one argument asks for it first; a command with more arguments
    /// opens its form on the commands screen.
    fn quick_command(&mut self, n: usize) {
        self.retarget(Target::Stack);
        let args = self.target().and_then(|o| o.commands.get(n));
        let Some(args) = args.map(|c| c.args.len()) else {
            return;
        };
        self.sel.command = n;
        self.form = Some(Form {
            command: n,
            values: vec![None; args],
            sel: 0,
            quick: args <= 1,
        });
        match args {
            0 => {
                self.run_command();
                self.form = None;
            }
            1 => self.edit_argument(),
            _ => {
                self.screen = Screen::Commands;
                self.pane = 1;
            }
        }
    }

    /// Acts on the selected row of the session's table. If the row holds a value for an argument of the entry line,
    /// the entry line takes it. Else, if one command of the stack has a row of this table as its only argument, the
    /// command runs with the row. Else, if the row has a frequency, the session tunes to it. Else the row becomes the
    /// chosen row.
    fn act_on_row(&mut self) {
        if self.choose_for_entry() {
            return;
        }
        let tables = self.shown_tables();
        let Some(&(r, t)) = tables.get(self.sel.table) else {
            return;
        };
        let Some(row) = r.tables[t].rows.get(self.sel.row) else {
            return;
        };
        let decl = &r.decl.tables[t];
        let stack = self.live.as_ref().map(|l| &l.stack);
        let command = stack.filter(|o| std::ptr::eq(r, &o.results)).and_then(|o| {
            let mut takes_row = o.commands.iter().enumerate().filter(|(_, c)| {
                let [a] = c.args.as_slice() else {
                    return false;
                };
                // An argument with a value column takes free values, and its command is not run from a row.
                a.refers_object.is_empty()
                    && a.refers_table == decl.key
                    && a.refers_column.is_empty()
            });
            let (n, _) = takes_row.next()?;
            takes_row.next().is_none().then_some(n)
        });
        let tuned = decl
            .columns
            .iter()
            .position(|c| c.ty == Type::Hz && tunes(&c.key))
            .and_then(|c| match row.cells.get(c) {
                Some(Value::Hz(hz)) if *hz > 0.0 => Some(*hz),
                _ => None,
            });
        let key = row.key.clone();
        match (command, tuned) {
            (Some(n), _) => {
                self.retarget(Target::Stack);
                self.form = Some(Form {
                    command: n,
                    values: vec![Some(Value::String(key))],
                    sel: 0,
                    quick: true,
                });
                self.run_command();
                self.form = None;
            }
            (None, Some(hz)) => self.tune(hz),
            (None, None) => self.choose_row(),
        }
    }

    /// Returns the declaration of argument `n` of the open form's command.
    fn argument(&self, n: usize) -> Option<&Decl> {
        let form = self.form.as_ref()?;
        self.target()?.commands.get(form.command)?.args.get(n)
    }

    fn edit_argument(&mut self) {
        let Some(form) = &self.form else {
            return;
        };
        let n = form.sel;
        let current = form.values[n].clone();
        let Some(d) = self.argument(n).cloned() else {
            return;
        };
        let value = current.unwrap_or_else(|| d.default.clone());
        // An argument with a value column is typed: its value need not be in the table.
        let table = if d.refers_column.is_empty() {
            d.refers_table.as_str()
        } else {
            ""
        };
        match self.rows_of(&d.refers_object, table) {
            Some(rows) if rows.keys.is_empty() => {
                self.fail(format!("{}: no rows yet", rows.table));
            }
            Some(rows) => {
                let sel = rows
                    .keys
                    .iter()
                    .position(|k| matches!(&value, Value::String(v) if v == k))
                    .unwrap_or(0);
                self.mode = Mode::Pick {
                    what: EditFor::Argument(n),
                    options: rows.lines,
                    keys: rows.keys,
                    sel,
                    find: Find::default(),
                };
            }
            None => self.mode = edit_mode(EditFor::Argument(n), &d, &value),
        }
    }

    /// Returns the rows of table `table` of the object `object` (empty for the object of the commands). Returns `None`
    /// if `table` is empty or the application holds no results of that object.
    fn rows_of(&self, object: &str, table: &str) -> Option<RowList> {
        if table.is_empty() {
            return None;
        }
        let owner = if object.is_empty() {
            self.target()?
        } else {
            let live = self.live.iter();
            let live = live.flat_map(|l| [Some(&l.session), Some(&l.stack), l.scan.as_ref()]);
            std::iter::once(&self.engine)
                .chain(&self.radios)
                .chain(live.flatten())
                .chain(&self.service)
                .find(|o| o.name == object)?
        };
        let r = &owner.results;
        let t = r.table_id(table)?;
        // The name of a row is the column that the table states, else its `label` or `name` column, else its first
        // text column.
        let columns = &r.decl.tables[t].columns;
        let named = r.decl.tables[t].label_column.or_else(|| {
            columns
                .iter()
                .position(|c| c.key == "label" || c.key == "name")
        });
        let name = named.or_else(|| columns.iter().position(|c| c.ty == Type::String));
        let rows = &r.tables[t].rows;
        let lines = rows
            .iter()
            .map(|row| match name.and_then(|c| row.cells.get(c)) {
                Some(Value::String(n)) if !n.is_empty() && *n != row.key => {
                    format!("{n}  ({})", row.key)
                }
                _ => row.key.clone(),
            });
        Some(RowList {
            table: r.decl.tables[t].label.clone(),
            keys: rows.iter().map(|row| row.key.clone()).collect(),
            lines: lines.collect(),
        })
    }

    fn edit_setting(&mut self) {
        let id = self.sel.setting;
        let Some(o) = self.target() else {
            return;
        };
        let (Some(d), Some(v)) = (o.settings.get(id), o.values.get(id)) else {
            return;
        };
        let what = EditFor::Setting(id as Id);
        if let Value::Bool(b) = v {
            let flipped = Value::Bool(!b);
            self.commit_value(what, flipped);
            return;
        }
        self.mode = edit_mode(what, d, v);
    }

    /// Carries out the line `text` typed for `what`. An empty line for a number changes nothing.
    fn commit_text(&mut self, what: EditFor, text: &str) {
        let empty = text.trim().is_empty();
        match what {
            EditFor::Source | EditFor::Region | EditFor::Channel | EditFor::StartChannel => {}
            EditFor::Setting(_) | EditFor::Argument(_) => {
                let Some(d) = self.edit_decl(what).cloned() else {
                    return;
                };
                if empty && format::typed_anew(&d) {
                    return;
                }
                match format::parse(text, &d) {
                    Ok(v) => self.commit_value(what, v),
                    Err(e) => self.fail(format!("{}: {e}", d.label)),
                }
            }
            EditFor::Frequency if empty => {}
            EditFor::Frequency => match format::parse_frequency(text) {
                Ok(hz) => self.tune(hz),
                Err(e) => self.fail(format!("Frequency: {e}")),
            },
            EditFor::StartFrequency if empty => {
                self.freq_hz = None;
                self.say("a stack starts on its own frequency");
            }
            EditFor::StartFrequency => match format::parse_frequency(text) {
                Ok(hz) => {
                    self.freq_hz = Some(hz);
                    let mut s = "a stack starts on ".to_owned();
                    format::hz(&mut s, hz);
                    self.say(s);
                }
                Err(e) => self.fail(format!("Frequency: {e}")),
            },
        }
    }

    /// Writes `v` to the setting or the argument `what`.
    fn commit_value(&mut self, what: EditFor, v: Value) {
        match what {
            EditFor::Setting(id) => {
                let Some(o) = self.target_mut() else {
                    return;
                };
                let r = o.handle.set(id, &v);
                let label = o
                    .settings
                    .get(id as usize)
                    .map(|d| d.label.clone())
                    .unwrap_or_default();
                match r {
                    Ok(()) => {
                        let _ = o.read_values();
                        self.say(format!("{label} set"));
                    }
                    Err(e) => self.fail(e),
                }
            }
            EditFor::Argument(n) => {
                if let Some(slot) = self.form.as_mut().and_then(|f| f.values.get_mut(n)) {
                    *slot = Some(v);
                }
                if self.form.as_ref().is_some_and(|f| f.quick) {
                    self.run_command();
                }
            }
            EditFor::Frequency | EditFor::StartFrequency => {}
            EditFor::Region => {
                let Value::String(code) = v else {
                    return;
                };
                let done = self
                    .set_region(&code)
                    .and_then(|()| self.apply_region_to_session());
                match (done, &self.region_note) {
                    (Err(e), _) => self.fail(e),
                    (Ok(()), Some(note)) => self.fail(note.clone()),
                    (Ok(()), None) => self.say(format!(
                        "region {code}: the stacks, the channel list (c) and the scan use its data"
                    )),
                }
            }
            EditFor::Channel | EditFor::StartChannel => {
                let Some(hz) = (match &v {
                    Value::String(t) => t.parse::<f64>().ok(),
                    _ => None,
                }) else {
                    return;
                };
                if what == EditFor::Channel {
                    self.tune(hz);
                } else {
                    self.freq_hz = Some(hz);
                    let mut s = "a stack starts on ".to_owned();
                    format::hz(&mut s, hz);
                    self.say(s);
                }
            }
            EditFor::Source => match v {
                Value::String(key) if key == NEW_RADIO => {
                    self.radio_form = Some(RadioForm::new(&self.engine.results));
                }
                Value::String(address) => {
                    if address.is_empty() {
                        self.say("source: the test signal of the stack");
                    } else {
                        self.say(format!("source: {address}"));
                    }
                    self.source = address;
                }
                _ => {}
            },
        }
    }

    /// Runs the command of the open form with the arguments that the user set.
    fn run_command(&mut self) {
        let Some(form) = self.form.clone() else {
            return;
        };
        let args: Vec<(Id, Value)> = (0..)
            .zip(&form.values)
            .filter_map(|(id, v)| v.clone().map(|v| (id, v)))
            .collect();
        let Some(o) = self.target() else {
            return;
        };
        let Some(cmd) = o.commands.get(form.command) else {
            return;
        };
        let (key, decls) = (cmd.label.clone(), cmd.reply.clone());
        match o.handle.invoke(cmd.id, &args) {
            Ok(reply) => {
                self.reply = decls
                    .iter()
                    .zip(&reply)
                    .map(|(d, v)| (d.label.clone(), format::value_text(v, d)))
                    .collect();
                self.say(format!("{key}: done"));
            }
            Err(e) => {
                self.reply.clear();
                self.fail(e);
            }
        }
    }

    /// Closes the selected radio and removes it from the list. A session that reads the radio ends first.
    fn close_radio(&mut self) {
        let i = self.sel.radio;
        let Some(name) = self.radios.get(i).map(|r| r.name.clone()) else {
            return;
        };
        self.end_sessions_on(&name);
        match self.radios[i].handle.radio_close() {
            Ok(()) => {
                self.forget_radio(i);
                self.say(format!("closed {name}"));
            }
            Err(e) if e.is_dead_handle() => {
                self.forget_radio(i);
                self.say(format!("removed {name}: it was closed"));
            }
            Err(e) => self.fail(e),
        }
    }

    /// Returns the rows of the table that the session or the services screen shows.
    fn shown_rows(&self) -> &[Row] {
        let tables = self.shown_tables();
        tables
            .get(self.sel.table)
            .map_or(&[][..], |(r, t)| &r.tables[*t].rows)
    }

    /// Returns the text of each line of the list that the keys act on, for the search. The first word of a line is the
    /// id or the key of its item where it has one.
    pub fn find_lines(&self) -> Vec<String> {
        let decls = |d: &[Decl]| -> Vec<String> {
            let line = |d: &Decl| format!("{} {} {}", d.label, d.key, d.section);
            d.iter().map(line).collect()
        };
        match (self.screen, self.pane) {
            (Screen::Stacks, _) => {
                let line = |s: &StackRow| format!("{} {} {}", s.id, s.name, s.group);
                self.stacks.iter().map(line).collect()
            }
            (Screen::Radios, _) => (0..self.radios.len())
                .map(|i| {
                    let address = self.radio_address(i).unwrap_or_default();
                    format!("{} {address}", self.radios[i].name)
                })
                .collect(),
            (Screen::Session, 0) => {
                let controls = self.controls().into_iter();
                controls.map(|c| self.control_text(c).0).collect()
            }
            (Screen::Services, 0) => {
                let services = self.services().into_iter();
                services
                    .map(|(key, name)| format!("{name} {key}"))
                    .collect()
            }
            (Screen::Session | Screen::Services, _) => {
                let tables = self.shown_tables();
                let Some((r, t)) = tables.get(self.sel.table) else {
                    return Vec::new();
                };
                let columns = &r.decl.tables[*t].columns;
                let line =
                    |row: &Row| format!("{} {}", row.key, views::cells(row, columns).join(" "));
                r.tables[*t].rows.iter().map(line).collect()
            }
            (Screen::Settings | Screen::Commands, 0) => {
                let objects = self.objects().into_iter();
                objects.map(|o| format!("{} {}", o.kind, o.name)).collect()
            }
            (Screen::Settings, _) => self.target().map_or_else(Vec::new, |o| decls(&o.settings)),
            (Screen::Commands, _) => self.target().map_or_else(Vec::new, |o| {
                let line = |c: &CommandDecl| format!("{} {}", c.label, c.key);
                o.commands.iter().map(line).collect()
            }),
            (Screen::Events, _) => {
                let line = |e: &EventType| format!("{} {}", e.label, e.key);
                self.event_types.iter().map(line).collect()
            }
            (Screen::Load, _) => Vec::new(),
        }
    }

    /// Returns the selected line of the list that the keys act on.
    fn find_line(&self) -> usize {
        match (self.screen, self.pane) {
            (Screen::Stacks, _) => self.sel.stack,
            (Screen::Radios, _) => self.sel.radio,
            (Screen::Session, 0) => self.sel.control,
            (Screen::Services, 0) => self.sel.service,
            (Screen::Session | Screen::Services, _) => self.sel.row,
            (Screen::Settings | Screen::Commands, 0) => self.object_index(),
            (Screen::Settings, _) => self.sel.setting,
            (Screen::Commands, _) => self.sel.command,
            (Screen::Events, _) => self.sel.event_type,
            (Screen::Load, _) => 0,
        }
    }

    /// Selects line `i` of the list that the keys act on.
    fn find_select(&mut self, i: usize) {
        match (self.screen, self.pane) {
            (Screen::Stacks, _) => self.sel.stack = i,
            (Screen::Radios, _) => self.sel.radio = i,
            (Screen::Session, 0) => self.sel.control = i,
            (Screen::Services, 0) => {
                self.sel.service = i;
                self.load_service();
            }
            (Screen::Session | Screen::Services, _) => {
                let key = self.shown_rows().get(i).map(|row| row.key.clone());
                self.sel.row = i;
                self.row_key = key;
            }
            (Screen::Settings | Screen::Commands, 0) => self.choose_object(i),
            (Screen::Settings, _) => self.sel.setting = i,
            (Screen::Commands, _) => self.sel.command = i,
            (Screen::Events, _) => self.sel.event_type = i,
            (Screen::Load, _) => {}
        }
    }

    /// Returns the tables that the session or the services screen shows, each as the results that hold it and its id
    /// there. The session screen shows the tables of the stack, then the hits of the scan, then the known stations of
    /// the region.
    pub fn shown_tables(&self) -> Vec<(&Results, usize)> {
        let mut all = Vec::new();
        if self.screen == Screen::Services {
            if let Some(s) = &self.service {
                all.extend((0..s.results.tables.len()).map(|t| (&s.results, t)));
            }
        } else if let Some(l) = &self.live {
            let r = &l.stack.results;
            all.extend((0..r.tables.len()).map(|t| (r, t)));
            let hits = l.scan.as_ref().map(|s| &s.results);
            all.extend(hits.and_then(|r| Some((r, r.table_id(SCAN_HITS)?))));
            all.extend(l.known.as_ref().map(|r| (r, 0)));
        }
        all
    }

    /// Moves the selected row of the shown table by the key `code` and keeps the key of the row.
    fn move_row(&mut self, code: KeyCode) {
        let tables = self.shown_tables();
        let rows = tables
            .get(self.sel.table)
            .map_or(&[][..], |(r, t)| &r.tables[*t].rows);
        let mut i = self.sel.row;
        move_sel(&mut i, rows.len(), code);
        let key = rows.get(i).map(|row| row.key.clone());
        self.sel.row = i;
        self.row_key = key;
    }

    /// Moves the selection to the row that the user set it on, after the tables were read. A row that the table lost
    /// leaves the selection on its line.
    fn follow_row(&mut self) {
        let Some(key) = &self.row_key else {
            // No row was chosen in this table yet: its row in use shows once the table has it.
            self.place_row();
            return;
        };
        let tables = self.shown_tables();
        let rows = tables
            .get(self.sel.table)
            .map_or(&[][..], |(r, t)| &r.tables[*t].rows);
        let at = rows.iter().position(|row| &row.key == key);
        let line = self.sel.row.min(rows.len().saturating_sub(1));
        let fallback = rows.get(line).map(|row| row.key.clone());
        match at {
            Some(i) => self.sel.row = i,
            None => {
                self.sel.row = line;
                self.row_key = fallback;
            }
        }
    }

    /// Carries out key `code` on the tables of the screen, if [`table_key`] gives it a meaning.
    fn key_table(&mut self, code: KeyCode) {
        match table_key(code, self.pane == 1) {
            Some(TableKey::Table(step)) => self.next_table(step),
            Some(TableKey::Columns(step)) => self.scroll_columns(step),
            None => {}
        }
    }

    /// Scrolls the columns of the shown table by `step` columns; a negative `step` scrolls to the left.
    fn scroll_columns(&mut self, step: isize) {
        let tables = self.shown_tables();
        let shown = tables.get(self.sel.table);
        let n = shown.map_or(0, |(r, t)| r.decl.tables[*t].columns.len());
        let last = n.saturating_sub(1);
        self.sel.column = self.sel.column.saturating_add_signed(step).min(last);
    }

    /// Shows the next table of the screen, or the previous one for a negative `step`. After the last table comes
    /// the first one.
    fn next_table(&mut self, step: isize) {
        let n = self.shown_tables().len();
        if n > 0 {
            let to = (self.sel.table as isize + step).rem_euclid(n as isize) as usize;
            self.open_table(to);
        }
    }

    /// A name for table `n` of the screen that no other table of the application has: the screen, the service and the
    /// key of the table.
    fn table_tag(&self, n: usize) -> Option<String> {
        let tables = self.shown_tables();
        let &(r, t) = tables.get(n)?;
        let owner = match (&self.service, self.screen) {
            (Some(s), Screen::Services) => s.name.as_str(),
            _ => "session",
        };
        Some(format!("{owner}/{}", r.decl.tables[t].key))
    }

    /// A name for the object whose tables the screen shows: the service on the services screen, else the session.
    fn view_tag(&self) -> String {
        match (&self.service, self.screen) {
            (Some(s), Screen::Services) => format!("tab/{}", s.name),
            _ => "tab/session".to_owned(),
        }
    }

    /// Remembers the shown table of the object on the screen and its selected row.
    fn keep_view(&mut self) {
        self.keep_row();
        let key = {
            let tables = self.shown_tables();
            tables
                .get(self.sel.table)
                .map(|(r, t)| r.decl.tables[*t].key.clone())
        };
        if let Some(key) = key {
            self.rows.insert(self.view_tag(), key);
        }
    }

    /// Shows the table that the object on the screen was left on, on the row it was left on. An object that was not
    /// shown before shows its first table.
    fn restore_view(&mut self) {
        let left = self.rows.get(&self.view_tag()).cloned();
        let n = {
            let tables = self.shown_tables();
            left.and_then(|k| tables.iter().position(|(r, t)| r.decl.tables[*t].key == k))
        };
        self.sel.table = n.unwrap_or(0);
        self.sel.column = 0;
        self.sel.row = 0;
        self.row_key = None;
        self.place_row();
    }

    /// Remembers the selected row of the shown table.
    fn keep_row(&mut self) {
        if let (Some(tag), Some(key)) = (self.table_tag(self.sel.table), self.row_key.clone()) {
            self.rows.insert(tag, key);
        }
    }

    /// Shows table `n` of the screen on the row it was left on, or on its row in use ([`App::row_in_use`]), or on its
    /// first row.
    fn open_table(&mut self, n: usize) {
        self.keep_row();
        self.first_table = false;
        self.sel.table = n;
        self.sel.column = 0;
        self.sel.row = 0;
        self.row_key = None;
        self.place_row();
    }

    /// Puts the selection of the shown table on the row it was left on, else on its row in use. Without either the
    /// selection stays.
    fn place_row(&mut self) {
        let n = self.sel.table;
        let left = self.table_tag(n).and_then(|t| self.rows.get(&t).cloned());
        let at = {
            let tables = self.shown_tables();
            let rows = tables.get(n).map_or(&[][..], |(r, t)| &r.tables[*t].rows);
            let find = |key: &String| rows.iter().position(|row| &row.key == key);
            left.as_ref()
                .and_then(|k| Some((find(k)?, k.clone())))
                .or_else(|| {
                    let k = self.row_in_use(n)?;
                    Some((find(&k)?, k))
                })
        };
        if let Some((i, key)) = at {
            self.sel.row = i;
            self.row_key = Some(key);
        }
    }

    /// The choices in use of the stack of the shown session: for each setting with choices or text, the key of the chosen
    /// choice or the text, then the text of the result field with the key of the setting (the choice in force when the
    /// setting is automatic, which has no row of its own).
    fn choices_in_use(&self) -> Vec<String> {
        let Some(l) = &self.live else {
            return Vec::new();
        };
        let (o, r) = (&l.stack, &l.stack.results);
        let mut out = Vec::new();
        for (i, d) in o.settings.iter().enumerate() {
            match o.values.get(i) {
                Some(Value::Enum(v)) if d.ty == Type::Enum => {
                    out.extend(d.options.get(*v as usize).map(|c| c.key.clone()));
                }
                Some(Value::String(text)) if d.ty == Type::String && !text.is_empty() => {
                    out.push(text.clone());
                }
                _ => continue,
            }
            let field = r.decl.fields.iter().position(|f| f.key == d.key);
            if let Some(Value::String(text)) = field.and_then(|f| r.fields.get(f)) {
                if !text.is_empty() {
                    out.push(text.clone());
                }
            }
        }
        out
    }

    /// The key of the row in use of table `n` of the session screen: the row of a choice in use of the stack (the channel
    /// plan), or the first row that belongs to one (a channel of that plan: its key starts with the choice and `/`).
    pub fn row_in_use(&self, n: usize) -> Option<String> {
        let tables = self.shown_tables();
        let &(r, t) = tables.get(n)?;
        let l = self.live.as_ref()?;
        if !std::ptr::eq(r, &l.stack.results) {
            return None;
        }
        let rows = &r.tables[t].rows;
        let choices = self.choices_in_use();
        choices
            .iter()
            .find_map(|c| rows.iter().find(|row| &row.key == c))
            .or_else(|| {
                choices.iter().find_map(|c| {
                    let part = format!("{c}/");
                    rows.iter().find(|row| row.key.starts_with(&part))
                })
            })
            .map(|row| row.key.clone())
    }

    /// The words that the frame of the shown table carries about its row in use, if the table lists choices and one of
    /// them is in use.
    pub fn table_note(&self) -> Option<String> {
        let key = self.row_in_use(self.sel.table)?;
        (!key.contains('/')).then(|| format!("in use: {key}"))
    }

    /// Acts on the selected row of the services screen: a row of a table of choices sets the setting, a row that other
    /// rows belong to (a plan and its channels) goes to them, and any other row is kept for a command argument.
    fn choose_row(&mut self) {
        let tables = self.shown_tables();
        let Some(&(r, t)) = tables.get(self.sel.table) else {
            return;
        };
        let Some(key) = r.tables[t]
            .rows
            .get(self.sel.row)
            .map(|row| row.key.clone())
        else {
            return;
        };
        let table = r.decl.tables[t].key.clone();
        // The rows of a table `<setting>_choices` are the choices of that setting of the engine: the row sets it.
        let setting = table
            .strip_suffix("_choices")
            .and_then(|k| self.engine_setting(k));
        // The rows of another table of the object with keys `<this key>/...` belong to this row: go to the first one.
        let part = format!("{key}/");
        let parts = tables.iter().enumerate().find_map(|(n, &(o, u))| {
            let at = o.tables[u]
                .rows
                .iter()
                .position(|x| x.key.starts_with(&part))?;
            (n != self.sel.table).then(|| (n, at, o.tables[u].rows[at].key.clone()))
        });
        // A table whose every row key is a choice of one setting of the stack lists those choices; so does a table with a
        // row whose key is the text of the result field with the key of a text setting, the value in force (the channel
        // plans of a television stack). The row sets the setting.
        let of_stack =
            self.live
                .as_ref()
                .filter(|l| std::ptr::eq(r, &l.stack.results) && !r.tables[t].rows.is_empty())
                .and_then(|l| {
                    let rows = &r.tables[t].rows;
                    let in_force = |key: &str| {
                        let f = r.decl.fields.iter().position(|f| f.key == key)?;
                        match r.fields.get(f) {
                            Some(Value::String(text)) => Some(text.clone()),
                            _ => None,
                        }
                    };
                    l.stack.settings.iter().position(|d| {
                        !d.fixed
                            && match d.ty {
                                Type::Enum => rows
                                    .iter()
                                    .all(|x| d.options.iter().any(|o| o.key == x.key)),
                                Type::String => in_force(&d.key)
                                    .is_some_and(|v| rows.iter().any(|x| x.key == v)),
                                _ => false,
                            }
                    })
                });
        if let Some(i) = setting {
            self.set_engine_choice(i, &key);
            return;
        }
        if let Some(i) = of_stack {
            self.set_stack_choice(i, &key);
        }
        if let Some((n, at, first)) = parts {
            self.keep_row();
            self.first_table = false;
            self.sel.table = n;
            self.sel.row = at;
            self.sel.column = 0;
            self.row_key = Some(first);
        } else if of_stack.is_none() {
            self.say(format!(
                "chosen row {key}: p pastes it into a command argument"
            ));
            self.chosen_row = Some(key);
        }
    }

    /// Sets the setting `i` of the stack of the shown session to the choice `text` and says what it is now.
    fn set_stack_choice(&mut self, i: usize, text: &str) {
        let Some(l) = self.live.as_mut() else {
            return;
        };
        let d = &l.stack.settings[i];
        let label = d.label.clone();
        let set = format::parse(text, d)
            .map_err(|e| format!("{label} {text}: {e}"))
            .and_then(|v| {
                let set = l.stack.handle.set(i as Id, &v);
                set.map_err(|e| format!("{label} {text}: {e}"))
            });
        match set {
            Ok(()) => {
                let _ = l.stack.read_values();
                let now = l
                    .stack
                    .values
                    .get(i)
                    .map(|v| format::value_text(v, &l.stack.settings[i]))
                    .unwrap_or_default();
                // The rows that belong to another choice are not the place to return to.
                let part = format!("{text}/");
                self.rows
                    .retain(|_, key| !key.contains('/') || key.starts_with(&part));
                self.say(format!("{label}: {now}"));
            }
            Err(e) => self.fail(e),
        }
    }

    /// Sets the setting `i` of the engine to the choice `text` and says what it is now.
    fn set_engine_choice(&mut self, i: usize, text: &str) {
        let label = self.engine.settings[i].label.clone();
        let set = format::parse(text, &self.engine.settings[i])
            .map_err(|e| format!("{label} {text}: {e}"))
            .and_then(|v| {
                let set = self.engine.handle.set(i as Id, &v);
                set.map_err(|e| format!("{label} {text}: {e}"))
            });
        match set {
            Ok(()) => {
                let _ = self.engine.read_values();
                let now = self
                    .engine
                    .values
                    .get(i)
                    .map(|v| format::value_text(v, &self.engine.settings[i]))
                    .unwrap_or_default();
                // The rows that belong to another choice are not the place to return to.
                let part = format!("{text}/");
                self.rows
                    .retain(|_, key| !key.contains('/') || key.starts_with(&part));
                self.say(format!("{label}: {now}"));
            }
            Err(e) => self.fail(e),
        }
    }

    /// Returns the keys and names of the services, from the engine's `services` table.
    pub fn services(&self) -> Vec<(String, String)> {
        let r = &self.engine.results;
        let Some(t) = r.table_id("services") else {
            return Vec::new();
        };
        let name = r.column_id(t, "name");
        r.tables[t]
            .rows
            .iter()
            .map(|row| {
                let n = match name.and_then(|c| row.cells.get(c)) {
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                };
                (row.key.clone(), n)
            })
            .collect()
    }

    /// Reads the selected service, unless the application holds it already. The row key of the `services` table is the
    /// object key of the service.
    fn load_service(&mut self) {
        let Some((key, _)) = self.services().into_iter().nth(self.sel.service) else {
            return;
        };
        if self.service.as_ref().is_some_and(|o| o.name == key) {
            return;
        }
        self.keep_view();
        self.sel.table = 0;
        self.sel.row = 0;
        self.row_key = None;
        self.sel.column = 0;
        match self.engine.handle.find(&key) {
            Ok(h) => {
                let mut o = Owner::new(key, h);
                if let Err(e) = o.poll() {
                    self.fail(e);
                }
                self.service = Some(o);
                self.restore_view();
                if self.target == Target::Service {
                    self.retarget(Target::Service);
                }
            }
            Err(e) => {
                self.service = None;
                self.fail(format!("{key}: {e}"));
            }
        }
    }

    /// Ends every session and frees every handle. The engine stops last.
    pub fn shutdown(&mut self) {
        while self.live.is_some() {
            self.stop_now();
        }
        self.service = None;
        for r in &self.radios {
            let _ = r.handle.radio_close();
        }
        self.radios.clear();
    }
}

/// The rows of a table that a command argument refers to.
struct RowList {
    /// The name of the table for people.
    table: String,
    /// The row keys.
    keys: Vec<String>,
    /// A line for each row: its name and its key.
    lines: Vec<String>,
}

/// Returns true if `a` and `b` are the same address: the same driver kind before the first `:` and the same text after
/// it. A `//` after the `:` and spaces around the address do not count.
fn same_address(a: &str, b: &str) -> bool {
    let parts = |s: &'_ str| -> (String, String) {
        let s = s.trim();
        let (driver, rest) = s.split_once(':').unwrap_or((s, ""));
        let rest = rest.strip_prefix("//").unwrap_or(rest);
        (driver.to_owned(), rest.to_owned())
    };
    parts(a) == parts(b)
}

/// Stops `live` and closes its session, waiting up to five seconds for it to stop. A session that the engine closed
/// already is no error.
fn end_session(live: &mut Live) -> api::Result<()> {
    live.speaker = None;
    let gone = |r: api::Result<()>| match r {
        Err(e) if e.is_dead_handle() => Ok(()),
        r => r,
    };
    if !live.closing {
        gone(live.session.handle.session_stop())?;
        live.closing = true;
    }
    let t0 = Instant::now();
    loop {
        match live.session.handle.session_close() {
            Err(e)
                if e.code == crate::sys::SDRKIT_E_STILL_RUNNING
                    && t0.elapsed() < Duration::from_secs(5) =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            r => return gone(r),
        }
    }
}

/// Returns the text of result field `key` of `r`, or `None` if `r` has no such field or the field has no value.
pub(crate) fn field_text(r: &Results, key: &str) -> Option<String> {
    let i = r.decl.fields.iter().position(|d| d.key == key)?;
    let v = r.fields.get(i).filter(|v| **v != Value::None)?;
    Some(format::value_text(v, &r.decl.fields[i]))
}

/// Returns the status line for the entry command `cmd` that ran with the line `entry`: the label of the command, the
/// text and the chosen values.
fn entry_done(cmd: &CommandDecl, entry: &Entry) -> String {
    let mut s = format!("{}: done: \"{}\"", cmd.label, entry.text);
    for a in cmd.args.iter().skip(1) {
        if let Some(v) = entry.chosen(&a.key) {
            s.push_str(&format!(" · {}: {v}", a.label.to_lowercase()));
        }
    }
    s
}

/// Returns true if the results `r` of the service that scans show a scan that runs.
fn scan_runs(r: &Results) -> bool {
    let Some(i) = r.decl.fields.iter().position(|d| d.key == "state") else {
        return false;
    };
    let Some(Value::Enum(n)) = r.fields.get(i) else {
        return false;
    };
    let state = r.decl.fields[i].options.get(*n as usize);
    state.is_some_and(|o| !matches!(o.key.as_str(), "idle" | "done"))
}

/// Returns the editor that edits `v` of declaration `d`: a list of options for an enum, else a line of text. The line
/// for a number starts empty; the line for a text starts with the text.
fn edit_mode(what: EditFor, d: &Decl, v: &Value) -> Mode {
    if d.ty == Type::Enum && !d.options.is_empty() {
        let sel = match v {
            Value::Enum(i) => *i as usize,
            _ => 0,
        };
        return Mode::Pick {
            what,
            options: d.options.iter().map(|o| o.label.clone()).collect(),
            keys: Vec::new(),
            sel,
            find: Find::default(),
        };
    }
    let text = if format::typed_anew(d) {
        String::new()
    } else {
        format::edit_text(v, d)
    };
    Mode::Edit { what, text }
}

/// What a key does to the tables of the session and services screens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableKey {
    /// Shows the table this number of tabs to the right; a negative number is to the left.
    Table(isize),
    /// Scrolls the columns of the shown table by this number; a negative number is to the left.
    Columns(isize),
}

/// Returns true if a column of frequencies with `key` holds the frequency of a station or a channel: Enter on its row
/// tunes there. Offsets, widths, rates and Doppler shifts are frequencies of another sort.
fn tunes(key: &str) -> bool {
    key.starts_with("freq")
        || matches!(
            key,
            "centre" | "centre_hz" | "dial" | "channel_hz" | "vision"
        )
}

/// Returns what key `code` does to the tables of the session and services screens, or `None` for a key with
/// another use. `[` and `]` choose the table from every pane of the screen. `on_tables` tells that the keys act on
/// the tables pane: there Left and Right choose the table too, and `,` and `.` scroll its columns.
pub fn table_key(code: KeyCode, on_tables: bool) -> Option<TableKey> {
    match code {
        KeyCode::Char('[') => Some(TableKey::Table(-1)),
        KeyCode::Char(']') => Some(TableKey::Table(1)),
        KeyCode::Left if on_tables => Some(TableKey::Table(-1)),
        KeyCode::Right if on_tables => Some(TableKey::Table(1)),
        KeyCode::Char(',') if on_tables => Some(TableKey::Columns(-1)),
        KeyCode::Char('.') if on_tables => Some(TableKey::Columns(1)),
        _ => None,
    }
}

/// Moves the selection `sel` of a list of `n` lines by the arrow, page and home keys.
pub(crate) fn move_sel(sel: &mut usize, n: usize, code: KeyCode) {
    let last = n.saturating_sub(1);
    *sel = match code {
        KeyCode::Up => sel.saturating_sub(1),
        KeyCode::Down => (*sel + 1).min(last),
        KeyCode::PageUp => sel.saturating_sub(10),
        KeyCode::PageDown => (*sel + 10).min(last),
        KeyCode::Home => 0,
        KeyCode::End => last,
        _ => (*sel).min(last),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send() -> CommandDecl {
        let arg = |key: &str, label: &str| Decl {
            key: key.to_owned(),
            label: label.to_owned(),
            ty: Type::String,
            ..Decl::default()
        };
        CommandDecl {
            label: "Send a text".to_owned(),
            args: vec![
                arg("text", "Text"),
                arg("channel", "Channel"),
                arg("to", "To"),
            ],
            ..CommandDecl::default()
        }
    }

    #[test]
    fn the_status_of_a_command_that_ran_names_the_command_the_text_and_the_chosen_values() {
        let mut e = Entry {
            text: "hello".to_owned(),
            ..Entry::default()
        };
        assert_eq!(entry_done(&send(), &e), "Send a text: done: \"hello\"");
        e.choose("channel", "LongFast");
        assert_eq!(
            entry_done(&send(), &e),
            "Send a text: done: \"hello\" · channel: LongFast"
        );
    }

    #[test]
    fn the_brackets_choose_the_table_from_every_pane() {
        for on_tables in [false, true] {
            assert_eq!(
                table_key(KeyCode::Char('['), on_tables),
                Some(TableKey::Table(-1))
            );
            assert_eq!(
                table_key(KeyCode::Char(']'), on_tables),
                Some(TableKey::Table(1))
            );
        }
    }

    #[test]
    fn the_arrows_choose_the_table_on_the_tables_pane_only() {
        assert_eq!(table_key(KeyCode::Left, true), Some(TableKey::Table(-1)));
        assert_eq!(table_key(KeyCode::Right, true), Some(TableKey::Table(1)));
        assert_eq!(table_key(KeyCode::Left, false), None);
        assert_eq!(table_key(KeyCode::Right, false), None);
    }

    #[test]
    fn comma_and_full_stop_scroll_the_columns_on_the_tables_pane_only() {
        assert_eq!(
            table_key(KeyCode::Char(','), true),
            Some(TableKey::Columns(-1))
        );
        assert_eq!(
            table_key(KeyCode::Char('.'), true),
            Some(TableKey::Columns(1))
        );
        assert_eq!(table_key(KeyCode::Char(','), false), None);
        assert_eq!(table_key(KeyCode::Char('.'), false), None);
    }

    #[test]
    fn the_letter_t_and_the_other_keys_do_nothing_to_the_tables() {
        for code in [
            KeyCode::Char('t'),
            KeyCode::Char('T'),
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Enter,
            KeyCode::Tab,
        ] {
            assert_eq!(table_key(code, true), None);
            assert_eq!(table_key(code, false), None);
        }
    }
}
