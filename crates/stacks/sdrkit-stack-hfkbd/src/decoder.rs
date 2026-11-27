//! What every stack of the family shares: the outputs of a decoder, the receiver graph (the front end, then the decoder), the
//! test signal, the decoded text, the stations heard, and the spot and text events.

use crate::front::HfFront;
use crate::kinds::TextChar;
use crate::typed::{LineEvent, SpotEvent, StationRow, LINE, REMOVE, SPOT, STATIONS};
use sdrkit_core::command::{CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Hertz, Table, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::DeclareSetting;
use sdrkit_core::{
    Complex32, ConfigError, Event, GenConfig, Generator, Hz, Receiver, Setting, SignalInfo,
    StackConfig,
};
use sdrkit_core::{Record, Values};
use sdrkit_graph::{
    Catalog, Format, GraphBuilder, GraphError, Kept, Kind, Outbox, Picture, Port, RateFrom,
    StackGraph, Timed,
};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{ObjectTable, Seen, Stamp};
use sdrkit_util_text::store::{Limits, TextStore};

/// Output number of the decoded characters (`text.chars`).
pub const CHARS: usize = 0;
/// Output number of the events.
pub const EVENTS: usize = 1;
/// Output number of the pictures (the standard `picture` item).
pub const PICTURES: usize = 2;
/// Output number of the decoder's own audio (the CW sidetone), 8 kHz mono.
pub const AUDIO: usize = 3;
/// Output number of the first output a decoder adds after the four above.
pub const EXTRA: usize = 4;

/// The text of the test signal.
pub const GEN_MESSAGE: Setting<field::Text> = Setting::new("gen_message", "Test text", "", "")
    .section(GEN_SECTION)
    .help("The text that the test signal sends. Empty: the stack's own test text.")
    .advanced();
/// The audio centre of a test signal.
pub const GEN_CENTRE_HZ: Setting<field::Hz> =
    Setting::new("gen_centre_hz", "Audio centre", "Hz", 1500.0)
        .range(300.0, 3500.0, 0.0)
        .section(GEN_SECTION)
        .advanced();
/// The section of the test-signal settings of the stacks.
pub const GEN_SECTION: &str = "Test signal";

/// The four outputs every decoder of the family has, in output-number order.
pub fn outputs() -> Vec<Port> {
    vec![
        Port::typed::<TextChar>("chars").rate_from(RateFrom::Fixed(0.0)),
        Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
        Port::typed::<Picture>("pictures").rate_from(RateFrom::Fixed(0.0)),
        Port::audio("audio").rate_from(RateFrom::Fixed(crate::front::AUDIO_RATE)),
    ]
}

/// The formats of the four outputs of [`outputs`].
pub fn formats() -> Vec<Format> {
    vec![
        Format::new(Kind::typed::<TextChar>(), 0.0),
        Format::new(Kind::Messages, 0.0),
        Format::new(Kind::typed::<Picture>(), 0.0),
        Format::new(Kind::Audio, crate::front::AUDIO_RATE),
    ]
}

/// The writes of a decoder of the family into its [`Outbox`].
pub trait Emit {
    /// Adds a decoded character at signal time `time` (seconds) with a confidence of 0 to 100, from signal `signal`.
    fn char(&mut self, time: f64, byte: u8, confidence: u8, signal: u32);
    /// Adds an event.
    fn event(&mut self, e: Event);
    /// Adds a picture at signal time `time`.
    fn picture(&mut self, time: f64, p: Picture);
    /// The audio output: mono samples.
    fn sound(&mut self) -> &mut Vec<f32>;
}

impl Emit for Outbox {
    fn char(&mut self, time: f64, byte: u8, confidence: u8, signal: u32) {
        self.items::<TextChar>(CHARS).push(Timed {
            time,
            value: TextChar {
                byte,
                confidence,
                alt: 0,
                signal,
            },
        });
    }
    fn event(&mut self, e: Event) {
        self.messages(EVENTS).push(e);
    }
    fn picture(&mut self, time: f64, p: Picture) {
        self.items::<Picture>(PICTURES)
            .push(Timed { time, value: p });
    }
    fn sound(&mut self) -> &mut Vec<f32> {
        self.audio(AUDIO)
    }
}

/// Which stream of the front end a decoder reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputPort {
    /// 24 kHz complex, the dial at 0 (PSK, CW, SITOR).
    Wide,
    /// 8 kHz complex sideband, the dial at 0 (every other stack).
    CAudio,
}

/// How a stack builds its receiver graph.
#[derive(Clone, Copy, Debug)]
pub struct RxOptions {
    /// The stack id.
    pub id: &'static str,
    /// The decoder's input.
    pub input: InputPort,
    /// The stack has the `demod` setting.
    pub demod: bool,
    /// Only USB and LSB (ROS).
    pub sideband_only: bool,
    /// The stack has Listen (every stack but CW, which makes its own audio instead).
    pub listen: bool,
    /// The front end has a span and gives the wide stream only ([`HfFront::with_span`]): CW.
    pub span: bool,
    /// The decoder makes pictures, and its output `pictures` leaves the graph.
    pub pictures: bool,
    /// Outputs of the decoder past the four standard ones that leave the graph ("dec.messages").
    pub outputs: &'static [&'static str],
    /// The stack's sender, if it has one ([`crate::sender::TextSender`]).
    pub sender: Option<fn() -> Box<dyn sdrkit_graph::Component>>,
}

/// The settings of the front end that a stack has: `demod` (the sideband-only form for ROS), `listen`, or `span` alone.
pub fn front_settings(opt: &RxOptions) -> &'static [&'static dyn DeclareSetting] {
    if opt.span {
        return &[&crate::front::SPAN];
    }
    match (opt.demod, opt.sideband_only, opt.listen) {
        (true, true, true) => &[&crate::front::SIDEBAND, &crate::front::LISTEN],
        (true, false, true) => &[&crate::front::DEMOD, &crate::front::LISTEN],
        (true, true, false) => &[&crate::front::SIDEBAND],
        (true, false, false) => &[&crate::front::DEMOD],
        (false, _, true) => &[&crate::front::LISTEN],
        (false, _, false) => &[],
    }
}

/// Builds a stack's graph from its description: the front end, then the decoder `dec`, and the sender into a transmit sink when
/// the stack has one. The decoder's settings keep their own keys, `listen` and `demod` go to the front end, a key of the sender
/// goes to the sender too (one value for both sides), and the stack's settings `cfg` are applied. `send`, `stop` and `tune`
/// go to the sender.
///
/// # Errors
///
/// [`GraphError::NamedSetting`] for a value of `cfg` that a component refuses.
pub fn graph<D>(dec: D, opt: RxOptions, cfg: &StackConfig) -> Result<StackGraph, GraphError>
where
    D: sdrkit_graph::Decoder<Input = Complex32>,
{
    let mut front = HfFront::new();
    if opt.sideband_only {
        front = front.sideband_only();
    }
    if opt.span {
        front = front.with_span();
    }
    let port = match opt.input {
        InputPort::Wide => "front.wide",
        InputPort::CAudio => "front.caudio",
    };
    let schema = dec.settings();
    let own: Vec<&'static str> = dec.commands().iter().map(|c| c.key).collect();
    let mut b = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("front", front)
        .add("dec", sdrkit_graph::DecoderComponent::new(dec))
        .input("front")
        .connect(port, "dec.in")
        .output("dec.events")
        .status_from("dec")
        .clock(0.1);
    if opt.pictures {
        b = b.output("dec.pictures");
    }
    let sender = opt.sender.map(|make| make());
    let sender_keys: Vec<String> = sender
        .as_ref()
        .map(|s| {
            s.info()
                .settings
                .settings()
                .iter()
                .map(|x| x.field.key.to_string())
                .collect()
        })
        .unwrap_or_default();
    // Every sender has these three commands.
    let sender_commands: &[&str] = if sender.is_some() {
        &["send", "stop", "tune"]
    } else {
        &[]
    };
    if let Some(s) = sender {
        b = b
            .add_boxed("sender", s)
            .add("tx", sdrkit_graph::components::TxSink::default())
            .input("sender.in")
            .connect("sender.out", "tx.in")
            .output("sender.events");
    }
    b = if opt.listen {
        b.output("front.listen")
    } else {
        b.output("dec.audio")
    };
    for o in opt.outputs {
        b = b.output(o);
    }
    let mut g = StackGraph::of(crate::typed::shape(opt.id), b.build()?).min_rate(
        crate::front::MIN_RATE,
        "HF keyboard stacks need a sample rate of at least 12 ksps",
    );
    // Every key with the components that take it.
    let mut routes: Vec<(String, Vec<String>)> = Vec::new();
    let mut add = |k: &str, to: String| match routes.iter_mut().find(|r| r.0 == k) {
        Some(r) => r.1.push(to),
        None => routes.push((k.to_string(), vec![to])),
    };
    if opt.listen {
        add("listen", "front.listen".into());
    }
    if opt.demod {
        add("demod", "front.demod".into());
    }
    if opt.span {
        add("span", "front.span".into());
    }
    for s in schema.settings() {
        let k = &s.field.key;
        add(k, format!("dec.{k}"));
    }
    for k in &sender_keys {
        add(k, format!("sender.{k}"));
    }
    for (k, to) in &routes {
        let to: Vec<&str> = to.iter().map(String::as_str).collect();
        g = g.route(k, &to)?;
    }
    for key in own {
        g = g.command(key, sdrkit_graph::CommandTarget::of("dec", key));
    }
    g = g.command("clear", sdrkit_graph::CommandTarget::every("clear"));
    for &key in sender_commands {
        g = g.command(key, sdrkit_graph::CommandTarget::of("sender", key));
    }
    g.with_settings(&cfg.settings).build()
}

/// A stack's receiver as [`graph`] builds it.
///
/// # Errors
///
/// [`ConfigError`] for the errors of [`graph`].
pub fn receiver<D>(
    dec: D,
    opt: RxOptions,
    cfg: &StackConfig,
) -> Result<Box<dyn Receiver>, ConfigError>
where
    D: sdrkit_graph::Decoder<Input = Complex32>,
{
    Ok(Box::new(graph(dec, opt, cfg)?))
}

/// A modulator: writes a stack's clean signal as complex samples with the dial at 0 Hz (an audio tone of f Hz is at +f Hz).
pub trait Modulator: Send {
    /// Fills `out`. The signal never ends: the text repeats.
    fn fill(&mut self, out: &mut [Complex32]);
}

/// The test signal of a stack: 0.3 s of silence, then the modulator. It declares what the signal is, so the framework can add
/// noise as an SNR or C/N0.
pub struct HfGenerator {
    rate: f64,
    m: Box<dyn Modulator>,
    lead: usize,
    info: SignalInfo,
}

impl HfGenerator {
    /// A generator at `rate` for the modulator `m`, whose clean signal is `info`.
    pub fn new(rate: f64, m: Box<dyn Modulator>, info: SignalInfo) -> Self {
        HfGenerator {
            rate,
            m,
            lead: (0.3 * rate) as usize,
            info,
        }
    }
    /// The same, boxed.
    pub fn boxed(rate: f64, m: impl Modulator + 'static, info: SignalInfo) -> Box<dyn Generator> {
        Box::new(Self::new(rate, Box::new(m), info))
    }
}

impl Generator for HfGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let quiet = self.lead.min(out.len());
        out[..quiet].fill(Complex32::default());
        self.lead -= quiet;
        self.m.fill(&mut out[quiet..]);
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(self.info)
    }
    /// The modulator writes the dial at 0 Hz of the samples.
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The tune offset of the family: the radio is tuned this far above the dial, Hz.
pub const TUNE_OFFSET_HZ: f64 = 20e3;

/// Where a test signal at `rate` puts the dial, Hz from the centre of its samples: [`TUNE_OFFSET_HZ`] below the centre, where a
/// receiver of the stack looks, when the rate holds the offset and the audio band; else at the centre.
pub fn test_dial(rate: f64) -> f64 {
    if rate / 2.0 >= TUNE_OFFSET_HZ + 4000.0 {
        -TUNE_OFFSET_HZ
    } else {
        0.0
    }
}

/// A modulator moved so its dial lies at `dial` Hz of the samples.
struct Placed<M: Modulator> {
    m: M,
    dial: f64,
    rate: f64,
    n: u64,
}

impl<M: Modulator> Modulator for Placed<M> {
    fn fill(&mut self, out: &mut [Complex32]) {
        self.m.fill(out);
        if self.dial != 0.0 {
            for o in out.iter_mut() {
                let p = std::f64::consts::TAU * (self.dial * self.n as f64 / self.rate).fract();
                *o *= Complex32::new(p.cos() as f32, p.sin() as f32);
                self.n += 1;
            }
        }
    }
}

/// A test signal whose station lies at `dial` Hz of the samples.
struct PlacedGenerator {
    g: HfGenerator,
    dial: f64,
}

impl Generator for PlacedGenerator {
    fn rate_hz(&self) -> Hz {
        self.g.rate_hz()
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.g.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        self.g.signal_info()
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        self.dial
    }
}

/// The test signal of modulator `m` (which writes the dial at 0 Hz) at `rate`, with the dial moved to [`test_dial`]: where a
/// receiver of the stack looks, so the signal source needs no tune offset of its own.
pub fn placed(rate: f64, m: impl Modulator + 'static, info: SignalInfo) -> Box<dyn Generator> {
    let dial = test_dial(rate);
    Box::new(PlacedGenerator {
        g: HfGenerator::new(
            rate,
            Box::new(Placed {
                m,
                dial,
                rate,
                n: 0,
            }),
            info,
        ),
        dial,
    })
}

/// The text of the test signal: the [`GEN_MESSAGE`] setting, else `default`. The stack must declare [`GEN_MESSAGE`].
pub fn message(cfg: &GenConfig, default: &str) -> String {
    match cfg.settings.get(&GEN_MESSAGE) {
        s if !s.is_empty() => s.to_string(),
        _ => default.to_string(),
    }
}

/// A text store that keeps the newest `chars` characters and `lines` chat lines.
pub fn store(chars: usize, lines: usize) -> TextStore {
    TextStore::new(Limits {
        chars,
        lines: lines.max(1),
        ..Limits::default()
    })
}

/// Writes the text of `store` into `buf`, reusing its memory.
pub fn text_into(store: &TextStore, buf: &mut String) {
    buf.clear();
    buf.extend(store.text());
}

/// The newest callsign of a store.
pub fn last_call(store: &TextStore) -> Option<&str> {
    store.callsigns().last()
}

/// A station heard: the row of [`STATIONS`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Station {
    /// The callsign.
    pub call: String,
    /// Where it was last heard, Hz.
    pub freq_hz: f64,
    /// Its SNR when last heard, dB.
    pub snr_db: f64,
}

impl Shaped for Station {
    type Row = StationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&StationRow {
            callsign: &self.call,
            freq_hz: Hertz(self.freq_hz),
            snr_db: self.snr_db,
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// What a text stack keeps of what it heard: the stations, the log lines to publish, and the text shown.
#[derive(Clone, Debug)]
pub struct Heard {
    /// The stack's name for the spot and text events ("RTTY").
    stack: &'static str,
    not_calls: &'static [&'static str],
    stations: ObjectTable<String, Station>,
    /// Log lines waiting to be published.
    pub logs: Vec<String>,
    text: String,
    shown: Option<u64>,
    now: f64,
}

impl Heard {
    /// Nothing heard yet, by the stack named `stack`.
    pub fn new(stack: &'static str) -> Self {
        Heard {
            stack,
            not_calls: &[],
            stations: ObjectTable::new(STATIONS.cap),
            logs: Vec::new(),
            text: String::new(),
            shown: None,
            now: 0.0,
        }
    }

    /// The same, where the words `not_calls` look like callsigns but are not stations ("ALLCALL").
    pub fn with_not_calls(mut self, not_calls: &'static [&'static str]) -> Self {
        self.not_calls = not_calls;
        self
    }

    /// Names the stack in the spot and text events from now on ("BPSK63").
    pub fn set_stack(&mut self, stack: &'static str) {
        self.stack = stack;
    }

    /// Returns a decoded byte to a text store and the outputs. A new callsign becomes a spot event, a station and a log line. A
    /// finished line becomes a text event.
    #[allow(clippy::too_many_arguments)]
    pub fn char(
        &mut self,
        store: &mut TextStore,
        out: &mut Outbox,
        byte: u8,
        time: f64,
        confidence: u8,
        signal: u32,
        freq_hz: f64,
        snr_db: f32,
    ) {
        out.char(time, byte, confidence, signal);
        let open = !store.current().text.is_empty();
        let start = store.current().start_s;
        let call = store
            .push_byte(byte, time, freq_hz, signal)
            .filter(|c| !self.not_calls.contains(c))
            .map(str::to_owned);
        if open && (store.current().text.is_empty() || store.current().start_s != start) {
            if let Some(line) = store.lines().last() {
                out.event(self.line_event(&line.text, time, line.freq_hz, snr_db));
            }
        }
        if let Some(call) = call {
            self.spot(out, &call, time, freq_hz, snr_db);
        }
    }

    fn line_event(&self, text: &str, time: f64, freq_hz: f64, snr_db: f32) -> Event {
        LINE.event(
            time,
            freq_hz,
            &LineEvent {
                source: sender(text),
                text,
                snr_db: f64::from(snr_db),
                mode: self.stack,
                family: Family::Message,
            },
        )
    }

    /// A callsign heard: a spot event, its station, and a log line.
    pub fn spot(&mut self, out: &mut Outbox, call: &str, time: f64, freq_hz: f64, snr_db: f32) {
        self.logs.push(format!("{time:.1} s: callsign {call}"));
        let snr = f64::from(snr_db);
        if let Ok(u) = self
            .stations
            .upsert(call, Stamp::at(time), Station::default)
        {
            *u.row = Station {
                call: call.to_owned(),
                freq_hz,
                snr_db: snr,
            };
        }
        self.now = self.now.max(time);
        out.event(SPOT.event(
            time,
            freq_hz,
            &SpotEvent {
                callsign: call,
                source: call,
                snr_db: snr,
                freq_hz: Hertz(freq_hz),
                mode: self.stack,
                family: Family::Message,
            },
        ));
    }

    /// Takes the text of `store` when it changed. [`Heard::text`] then returns it.
    pub fn update(&mut self, store: &TextStore) {
        if self.shown != Some(store.total) {
            self.shown = Some(store.total);
            text_into(store, &mut self.text);
        }
    }

    /// The text that [`Heard::update`] took last.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Publishes the waiting log lines and the stations.
    pub fn report(&mut self, p: &mut Published) {
        flush_logs(&mut self.logs, p);
        shaped::rows(&self.stations, &STATIONS, self.now, p);
    }

    /// Forgets the stations and the text shown.
    pub fn clear(&mut self) {
        self.stations.clear();
        self.text.clear();
        self.shown = None;
    }

    /// Forgets what a change of frequency does not keep: the stations unless `kept` holds their table.
    pub fn forget(&mut self, kept: &Kept) {
        if !kept.table(STATIONS.key) {
            self.stations.clear();
        }
        self.shown = None;
    }

    /// Runs command `id` of [`HeardCommand`] with the arguments `args`. `p` holds the declaration of the decoder's results.
    ///
    /// # Errors
    ///
    /// [`CommandError::NoSuchRow`] if `remove` names a station that is not in the table. The errors of the arguments.
    pub fn invoke(
        &mut self,
        id: CommandId,
        args: &Values,
        p: &Published,
    ) -> Result<Option<Record>, CommandError> {
        match HeardCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            HeardCommand::Remove => {
                let a: crate::typed::StationArgs = REMOVE.args(args)?;
                let row = a.row;
                if self.stations.remove(row).is_none() {
                    return Err(p.no_such_row(&STATIONS, row));
                }
            }
        }
        Ok(None)
    }
}

sdrkit_core::commands! {
    /// The commands of a decoder that keeps a [`Heard`].
    pub enum HeardCommand {
        /// Forgets one station.
        Remove = REMOVE.command,
    }
}

/// The callsign after "DE" in a line of text, if there is one.
fn sender(text: &str) -> Option<&str> {
    let mut words = text.split_whitespace();
    while let Some(w) = words.next() {
        if w.eq_ignore_ascii_case("de") {
            return words
                .next()
                .filter(|c| sdrkit_util_text::callsign::is_callsign(c));
        }
    }
    None
}

/// Writes `args` into `s`, reusing its memory.
pub fn put_text(s: &mut String, args: std::fmt::Arguments<'_>) {
    use std::fmt::Write;
    s.clear();
    // Writing into a String cannot fail.
    let _ = s.write_fmt(args);
}

/// Removes the rows of table `t` whose key `keep` refuses. `scratch` keeps the keys to remove between calls.
pub fn retain_rows<S: sdrkit_core::interface::Fields + 'static>(
    p: &mut Published,
    t: &Table<S>,
    scratch: &mut Vec<String>,
    keep: impl Fn(&str) -> bool,
) {
    scratch.clear();
    let r = p.results();
    if let Some(table) = r.decl().table_id(t.key).and_then(|id| r.table(id)) {
        for (k, _) in table.rows_newest_first() {
            if !keep(k) {
                scratch.push(k.to_owned());
            }
        }
    }
    for k in scratch.iter() {
        p.remove_row(t, k);
    }
}

/// Moves the waiting log lines into the results.
pub fn flush_logs(logs: &mut Vec<String>, p: &mut Published) {
    for l in logs.drain(..) {
        p.results_mut().log(l);
    }
}
