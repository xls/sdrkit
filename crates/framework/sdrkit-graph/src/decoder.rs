//! Decoders: a plain type that is fed items and writes typed outputs and results, made into a component by [`DecoderComponent`].
//!
//! The component supplies the ports, retuning at the exact sample, the `clear` command, the report throttle, and an optional
//! helper thread for live input.

use crate::component::no_settings;
use crate::component::{Applied, ChannelRule, Component, ComponentInfo, Io, Kept, KindRule, Port};
use crate::context::{RunContext, Ticket};
use crate::graph::GraphError;
use crate::stream::{Buffer, Data, Format, Item, Kind, Packet, TagKind, Timed};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::{field, keys, Field};
use sdrkit_core::{
    Complex32, ConfigError, Event, Published, Record, Results, Schema, SettingError, SettingId,
    StageError, StateBlob, StateError, Value, Values,
};
use std::sync::Arc;

/// Forgets everything that the decoder of a [`DecoderComponent`] decoded so far.
pub const CLEAR: Command =
    Command::new("clear", "Clear").help("Forgets everything decoded so far.");

sdrkit_core::commands! {
    /// The commands that every [`DecoderComponent`] has, before the commands of its decoder.
    pub enum DecoderCommand {
        /// Forgets everything decoded so far ([`Decoder::clear`]).
        Clear = CLEAR,
    }
}

/// An input item a [`Decoder`] can take: samples (`Complex32` for `Iq` or `Symbols`, `f32` for `Real`, `[f32; N]` for audio frames
/// of `N` channels), hard bits (`u8`), soft
/// bits (`i8`), byte packets ([`Packet`]), messages ([`Event`]) or a typed stream (`Timed<T>`).
pub trait DecoderInput: Clone + Send + 'static {
    /// The stream kind it is read from.
    fn kind() -> Kind;
    /// The channel counts it takes. The default takes any.
    fn channels() -> ChannelRule {
        ChannelRule::Any
    }
    /// The items of input `i` this call.
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self];
    /// The item's own signal time in seconds, for an item that carries one (a packet, a message, a typed item). The default
    /// (`None`) is for samples and bits, whose time comes from their rate.
    fn signal_time(&self) -> Option<f64> {
        None
    }
}

impl DecoderInput for Complex32 {
    fn kind() -> Kind {
        Kind::Iq
    }
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self] {
        io.samples(i)
    }
}
impl DecoderInput for f32 {
    fn kind() -> Kind {
        Kind::Real
    }
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self] {
        io.samples(i)
    }
}
impl<const N: usize> DecoderInput for [f32; N] {
    fn kind() -> Kind {
        Kind::Audio
    }
    fn channels() -> ChannelRule {
        ChannelRule::Exact(u16::try_from(N).unwrap_or(u16::MAX))
    }
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self] {
        io.audio(i).frames_of::<N>()
    }
}
impl DecoderInput for u8 {
    fn kind() -> Kind {
        Kind::HardBits
    }
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self] {
        io.hard_bits(i)
    }
}
impl DecoderInput for i8 {
    fn kind() -> Kind {
        Kind::Llr
    }
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self] {
        io.llrs(i)
    }
}
impl DecoderInput for Packet {
    fn kind() -> Kind {
        Kind::Bytes
    }
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self] {
        io.bytes(i)
    }
    fn signal_time(&self) -> Option<f64> {
        Some(self.time)
    }
}
impl DecoderInput for Event {
    fn kind() -> Kind {
        Kind::Messages
    }
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self] {
        io.messages(i)
    }
    fn signal_time(&self) -> Option<f64> {
        Some(self.time)
    }
}
impl<T: Item> DecoderInput for Timed<T> {
    fn kind() -> Kind {
        Kind::typed::<T>()
    }
    fn read<'a>(io: &Io<'a>, i: usize) -> &'a [Self] {
        io.items::<T>(i)
    }
    fn signal_time(&self) -> Option<f64> {
        Some(self.time)
    }
}

/// When a decoder runs on a helper thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Helper {
    /// Always on the decoding thread (the default: decoders that keep up with the signal).
    #[default]
    Never,
    /// On a helper thread when the samples are live; in line for recordings and generators.
    WhenLive {
        /// Items per batch: a batch is delivered once this many have arrived (0: whatever has arrived).
        batch: usize,
        /// The most items waiting while a batch is decoded; beyond it the oldest waiting items are dropped and counted (a live
        /// radio does not wait for the decoder).
        backlog: usize,
    },
}

/// The outputs of a decoder for one call: one buffer per declared output, in order, of the declared kinds. The component moves
/// them into the graph's buffers after the call (no copy).
#[derive(Default)]
pub struct Outbox {
    data: Vec<Data>,
    /// The room each output without a rate gets: the cap that the decoder declares, at most the graph's cap of items per
    /// pass.
    caps: Vec<usize>,
}

impl std::fmt::Debug for Outbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(&self.data).finish()
    }
}

fn wrong(o: usize, d: &Data) -> ! {
    panic!("decoder output {o} does not hold this kind of item (it holds {d:?})")
}

impl Outbox {
    /// Empty buffers for outputs of these kinds.
    pub fn new(kinds: &[Kind]) -> Outbox {
        Outbox {
            data: kinds.iter().map(Kind::empty_data).collect(),
            caps: vec![0; kinds.len()],
        }
    }
    /// Number of outputs.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    /// No outputs.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    /// Output `o`, untyped.
    pub fn data(&mut self, o: usize) -> &mut Data {
        &mut self.data[o]
    }
    /// Typed output `o`.
    pub fn items<T: Item>(&mut self, o: usize) -> &mut Vec<Timed<T>> {
        match &mut self.data[o] {
            Data::Items(slot) => {
                if slot.is_none() {
                    *slot = Some(Box::new(Vec::<Timed<T>>::with_capacity(self.caps[o])));
                }
                let b = slot.as_mut().expect("just made");
                let held = b.item_type().rust_name();
                match b.as_any_mut().downcast_mut::<Vec<Timed<T>>>() {
                    Some(v) => v,
                    None => panic!(
                        "decoder output {o} holds {held}, not {}",
                        std::any::type_name::<T>()
                    ),
                }
            }
            d => wrong(o, d),
        }
    }
    /// `Messages` output `o`.
    pub fn messages(&mut self, o: usize) -> &mut Vec<Event> {
        match &mut self.data[o] {
            Data::Messages(v) => v,
            d => wrong(o, d),
        }
    }
    /// `Bytes` output `o`.
    pub fn bytes(&mut self, o: usize) -> &mut Vec<Packet> {
        match &mut self.data[o] {
            Data::Bytes(v) => v,
            d => wrong(o, d),
        }
    }
    /// `Audio` output `o`, interleaved. The component sets its channel count from the output's format when it emits it.
    pub fn audio(&mut self, o: usize) -> &mut Vec<f32> {
        match &mut self.data[o] {
            Data::Audio(a) => &mut a.samples,
            d => wrong(o, d),
        }
    }
    /// `Real` output `o`.
    pub fn real(&mut self, o: usize) -> &mut Vec<f32> {
        match &mut self.data[o] {
            Data::Real(v) => v,
            d => wrong(o, d),
        }
    }
    /// `Iq` or `Symbols` output `o`.
    pub fn complex(&mut self, o: usize) -> &mut Vec<Complex32> {
        match &mut self.data[o] {
            Data::Iq(v) | Data::Symbols(v) => v,
            d => wrong(o, d),
        }
    }
    /// [`Kind::HardBits`] output `o`.
    pub fn hard_bits(&mut self, o: usize) -> &mut Vec<u8> {
        match &mut self.data[o] {
            Data::HardBits(v) => v,
            d => wrong(o, d),
        }
    }
    /// [`Kind::Llr`] output `o`.
    pub fn llrs(&mut self, o: usize) -> &mut Vec<i8> {
        match &mut self.data[o] {
            Data::Llr(v) => v,
            d => wrong(o, d),
        }
    }
    /// Moves everything into `other` (same kinds), leaving this empty.
    pub fn move_into(&mut self, other: &mut Outbox) {
        for (d, s) in other.data.iter_mut().zip(&mut self.data) {
            d.append(s);
        }
    }
}

/// A burst or frame decoder as a plain type: it declares its input kind, outputs and settings, and processes a piece of input.
/// [`DecoderComponent`] runs it as a component. [`Decoder::report`] runs at the graph's clock ticks, or every
/// [`Decoder::report_interval_s`] of signal. With a live input, a decoder can run on a helper thread ([`Decoder::helper`]); with a
/// recording it runs in line.
///
/// # Example
///
/// ```
/// use sdrkit_core::{Complex32, ConfigError, Published, publish::keys};
/// use sdrkit_graph::{Decoder, DecoderComponent, Format, Kind, Outbox, Port, Timed};
///
/// #[derive(Clone, Debug)]
/// struct Count(u64);
/// impl sdrkit_graph::Item for Count { const KIND: &'static str = "demo.count"; }
///
/// /// Counts the samples above a level, and emits the count after each piece.
/// #[derive(Default)]
/// struct Peaks { above: u64, rate: f64 }
/// impl Decoder for Peaks {
///     type Input = Complex32;
///     fn name(&self) -> &'static str { "demo.peaks" }
///     fn outputs(&self) -> Vec<Port> { vec![Port::typed::<Count>("count")] }
///     fn values(&self) -> sdrkit_core::Values { sdrkit_graph::no_values() }
///     fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
///         self.rate = input.rate;
///         Ok(vec![Format::new(Kind::typed::<Count>(), 0.0)])
///     }
///     fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
///         self.above += x.iter().filter(|s| s.norm() > 0.5).count() as u64;
///         out.items::<Count>(0).push(Timed { time: first as f64 / self.rate, value: Count(self.above) });
///     }
///     fn results(&self) -> Published { Published::new(&[&keys::FRAMES_OK]) }
///     fn report(&mut self, r: &mut Published) { r.set(&keys::FRAMES_OK, self.above as i64); }
/// }
/// let c = DecoderComponent::new(Peaks::default());
/// # let _ = c;
/// ```
pub trait Decoder: Send + 'static {
    /// What it is fed.
    type Input: DecoderInput;
    /// The registry name, `"<family>.<name>"`.
    fn name(&self) -> &'static str;
    /// The input kind (by default the input type's own; `Kind::Symbols` for a decoder of complex symbols).
    fn input_kind(&self) -> Kind {
        Self::Input::kind()
    }
    /// The outputs, in order. A family wrapper may add its own after the decoder's. A decoder that only reports has none.
    fn outputs(&self) -> Vec<Port>;
    /// One more input beside `in`, of any kind: for example `Port::spectrum("spectrum", spec)`, so that the graph returns the
    /// decoder the shared spectrum of its signal. `None` (the default) for none.
    fn extra_input(&self) -> Option<Port> {
        None
    }
    /// The items of the extra input for this call, with its format, before [`Decoder::process`] gets the items of `in`.
    /// While a helper thread holds the decoder, the component keeps a copy of the extra items and passes them back when the
    /// decoder comes back, before the batch that holds the items of `in` of the same calls. The default ignores them.
    fn extra(&mut self, format: &Format, input: &Buffer) {
        let _ = (format, input);
    }
    /// Its settings.
    fn settings(&self) -> Arc<Schema> {
        no_settings()
    }
    /// The present value of every setting of [`Decoder::settings`], including the values its constructor received. A decoder
    /// without settings returns [`no_values`](crate::no_values).
    fn values(&self) -> Values;
    /// Applies a new value of setting `id` of [`Decoder::settings`]. The value has passed [`Schema::check`].
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the decoder refuses the value. The default returns [`SettingError::NoSuchId`].
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let _ = value;
        Err(SettingError::NoSuchId { id })
    }
    /// Its own commands (the object commands of its tables, such as `remove`). The component lists them after its own
    /// `clear`, so none of them can have the key `clear`. The default has none.
    fn commands(&self) -> &'static [&'static Command] {
        &[]
    }
    /// Runs its own command `id`, the index in [`Decoder::commands`], with the arguments `args` of that command. While a
    /// helper thread holds the decoder, the component waits for it first.
    ///
    /// # Errors
    ///
    /// [`CommandError`] if the command is refused or fails. The default returns [`CommandError::NoSuchId`].
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let _ = args;
        Err(CommandError::NoSuchId { id })
    }
    /// The most items that output `output` (an output without a rate) returns in one call. The component keeps that much room,
    /// at most the graph's cap of items per pass. `None` (the default) keeps the graph's cap.
    fn output_cap(&self, output: usize) -> Option<usize> {
        let _ = output;
        None
    }
    /// What it publishes. The component adds `dropped` ([`keys::DROPPED`]) if the declaration does not have it. The default
    /// publishes nothing.
    fn results(&self) -> Published {
        Published::new(&[])
    }
    /// The input format is known (start, a change of rate): check it (a rate too low is an error), build the DSP, drop sync, and
    /// return the output formats in [`Decoder::outputs`] order.
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError>;
    /// A retune from the next item on: drop sync and anything half received, keep what was decoded. The default does nothing.
    fn retune(&mut self, input: &Format) {
        let _ = input;
    }
    /// Decodes `x`, whose first item has absolute index `first` in the input stream, appending to `out`.
    fn process(&mut self, x: &[Self::Input], first: u64, out: &mut Outbox);
    /// The end of a clock period of the graph (see [`GraphBuilder::clock`](crate::GraphBuilder::clock)).
    fn tick(&mut self, out: &mut Outbox) {
        let _ = out;
    }
    /// The input ended ([`Component::finish`]): write what is still held and can be decided (a frame whose end is in, a
    /// last burst), appending to `out`. [`Decoder::report`] is called after it. The default does nothing.
    fn finish(&mut self, out: &mut Outbox) {
        let _ = out;
    }
    /// Writes the results. Called at clock ticks, or every [`Decoder::report_interval_s`] of signal, and after `clear`. For an
    /// input without a rate (frames, packets, typed items), the signal time is the newest item's own time, else the graph's
    /// input time. With neither, it is called once for each call that brings items.
    fn report(&mut self, results: &mut Published) {
        let _ = results;
    }
    /// How often to report without a clock, seconds of signal.
    fn report_interval_s(&self) -> f64 {
        0.1
    }
    /// Forgets what was decoded. The `clear` command of the component calls it.
    fn clear(&mut self) {}
    /// The host changed the frequency ([`Component::forget`]): forget what was decoded, except the fields and tables
    /// of `kept`. A decoder that holds kept and other results implements it: it clears the others and tests
    /// [`Kept::table`] and [`Kept::field`] before it clears a kept one. The default calls [`Decoder::clear`] if
    /// `kept` is empty, and else leaves the decoder as it is. [`Decoder::report`] is called after it.
    fn forget(&mut self, kept: &Kept) {
        if kept.is_empty() {
            self.clear();
        }
    }
    /// Whether it runs on a helper thread when live.
    fn helper(&self) -> Helper {
        Helper::Never
    }
    /// Whether the decoder takes its input in looks now: it examines one piece and, on purpose, leaves out what arrives
    /// while it works on it. Items that do not fit the backlog while this is true count as `skipped` ([`LookFields`]) and
    /// not as `dropped`. The component reads it each time the decoder is not on a helper thread. The default is false:
    /// the decoder wants every item.
    fn looks(&self) -> bool {
        false
    }
    /// The run context (live, station, helpers).
    fn attach(&mut self, ctx: &RunContext) {
        let _ = ctx;
    }
    /// What to keep across sessions (see [`Component::save_state`]).
    fn save_state(&self) -> Option<StateBlob> {
        None
    }
    /// Restores it.
    ///
    /// # Errors
    ///
    /// As [`Component::restore_state`]. The default returns [`StateError::NoState`].
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let _ = state;
        Err(StateError::NoState)
    }
}

/// The result of a helper-thread job.
struct Done<D: Decoder> {
    dec: D,
    input: Vec<D::Input>,
    out: Outbox,
    results: Published,
}

/// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a
/// decoder component names it after the result sets of the decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Fields)]
pub struct DecoderFields {
    /// Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
    #[field(shared = keys::DROPPED)]
    pub dropped: Option<i64>,
}

/// Items that a decoder left out between looks ([`Decoder::looks`]).
pub const SKIPPED: Field<field::I64> = Field::new("skipped", "Not examined between looks", "");

/// The result set of a decoder that takes its input in looks ([`Decoder::looks`]). The decoder declares the field in
/// [`Decoder::results`], and the description of its stack lists the set; the component writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Fields)]
pub struct LookFields {
    /// Items left out between looks, on purpose. It holds a value while the decoder runs on a helper thread.
    #[field(shared = SKIPPED)]
    pub skipped: Option<i64>,
}

/// A [`Decoder`] as a component: input `in`, the decoder's outputs, the decoder's settings and the `clear` command.
pub struct DecoderComponent<D: Decoder> {
    dec: Option<D>,
    results: Published,
    out: Outbox,
    format: Option<Format>,
    kinds: Vec<Kind>,
    ctx: RunContext,
    since: f64,
    rate: f64,
    // Helper-thread state.
    job: Option<Ticket<Done<D>>>,
    /// The items that wait for the decoder, oldest first: a queue, so that dropping the oldest moves nothing.
    waiting: std::collections::VecDeque<D::Input>,
    spare: Vec<D::Input>,
    waiting_first: u64,
    queued: Vec<(SettingId, Value)>,
    pending_retune: Option<Format>,
    pending_clear: bool,
    /// A forget that waits for the decoder to come back from a helper thread.
    pending_forget: Option<Kept>,
    dropped: u64,
    /// `dropped` is in the results (the decoder runs on a helper thread).
    dropped_shown: bool,
    /// Items left out while the decoder took its input in looks.
    skipped: u64,
    /// [`Decoder::looks`], read when the decoder was last here.
    looks: bool,
    /// The declaration of the decoder holds `skipped` ([`LookFields`]).
    skipped_declared: bool,
    /// The report interval in seconds of signal, read at configure (the decoder is away while a batch runs).
    interval: f64,
    /// The newest item time at the last report, for an input without a rate.
    last_report_t: f64,
    /// A report is due and waits for the next batch (helper thread, input without a rate).
    report_pending: bool,
    // For `info` while the decoder is away.
    name: &'static str,
    outputs: Vec<Port>,
    /// The decoder's extra input ([`Decoder::extra_input`]).
    extra: Option<Port>,
    /// The extra items that came while a helper thread held the decoder, and their format.
    extra_held: Option<(Format, Buffer)>,
    /// `clear`, then the decoder's own commands.
    commands: Commands,
    /// The cap of each output that the decoder declares ([`Decoder::output_cap`]).
    declared_caps: Vec<Option<usize>>,
    input_kind: Kind,
    schema: Arc<Schema>,
    /// The present values of `schema`. Kept here while the decoder is away.
    values: Values,
    helper: Helper,
}

impl<D: Decoder> DecoderComponent<D> {
    /// Wraps a decoder.
    pub fn new(dec: D) -> Self {
        let schema = dec.settings();
        let values = dec.values();
        let mut decl = dec.results().results().decl().as_ref().clone();
        if decl.field_id(keys::DROPPED.key).is_none() {
            decl = decl.with_fields(&[&keys::DROPPED]);
        }
        let skipped_declared = decl.field_id(SKIPPED.key).is_some();
        DecoderComponent {
            name: dec.name(),
            outputs: dec.outputs(),
            extra: dec.extra_input(),
            extra_held: None,
            declared_caps: Vec::new(),
            commands: Commands::of(
                &DecoderCommand::COMMANDS
                    .iter()
                    .chain(dec.commands())
                    .copied()
                    .collect::<Vec<_>>(),
            ),
            input_kind: dec.input_kind(),
            helper: dec.helper(),
            schema,
            values,
            dec: Some(dec),
            results: Published::from_decl(Arc::new(decl)),
            out: Outbox::default(),
            format: None,
            kinds: Vec::new(),
            ctx: RunContext::default(),
            since: 0.0,
            rate: 0.0,
            job: None,
            waiting: std::collections::VecDeque::new(),
            spare: Vec::new(),
            waiting_first: 0,
            queued: Vec::new(),
            pending_retune: None,
            pending_clear: false,
            pending_forget: None,
            dropped: 0,
            dropped_shown: false,
            skipped: 0,
            looks: false,
            skipped_declared,
            interval: 0.1,
            last_report_t: f64::NEG_INFINITY,
            report_pending: false,
        }
    }
    /// The decoder (`None` while a helper thread holds it).
    pub fn decoder(&self) -> Option<&D> {
        self.dec.as_ref()
    }
    /// The decoder, to change (`None` while a helper thread holds it).
    pub fn decoder_mut(&mut self) -> Option<&mut D> {
        self.dec.as_mut()
    }
    /// Input items dropped because the helper thread could not keep up.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }
    /// Input items left out while the decoder took its input in looks ([`Decoder::looks`]).
    pub fn skipped(&self) -> u64 {
        self.skipped
    }
    /// Whether a batch is being decoded on a helper thread.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    fn threaded(&self) -> Option<(usize, usize)> {
        // Read once at configure: the decoder is away while a batch runs.
        match self.helper {
            Helper::WhenLive { batch, backlog } if self.ctx.live => {
                Some((batch, backlog.max(batch).max(1)))
            }
            _ => None,
        }
    }

    /// Whether a report is due after this call: at a tick, or when `interval` seconds of signal have passed since the last
    /// one. Signal time comes from the input rate, else from the newest item's own time, else from the graph's input (`pass_s`).
    /// With none of them, each call that brings items reports.
    fn report_due(&mut self, x: &[D::Input], tick: bool, pass_s: f64) -> bool {
        if tick {
            self.since = 0.0;
            if let Some(t) = x.last().and_then(DecoderInput::signal_time) {
                self.last_report_t = t;
            }
            return true;
        }
        if self.rate > 0.0 {
            self.since += x.len() as f64 / self.rate;
            if self.since >= self.interval {
                self.since = 0.0;
                return true;
            }
            return false;
        }
        if let Some(t) = x.last().and_then(DecoderInput::signal_time) {
            // A time that goes back (a new recording) starts the count again. Item times are computed, so a nanosecond of
            // rounding is allowed.
            if t - self.last_report_t >= self.interval - 1e-9 || t < self.last_report_t {
                self.last_report_t = t;
                return true;
            }
            return false;
        }
        if pass_s > 0.0 {
            self.since += pass_s;
            if self.since >= self.interval && !x.is_empty() {
                self.since = 0.0;
                return true;
            }
            return false;
        }
        !x.is_empty()
    }

    /// Writes the counts of dropped and skipped items into the results while the decoder runs on a helper thread, and takes
    /// them out when it does not.
    fn show_dropped(&mut self) {
        let shown = self.threaded().is_some();
        if !shown && !self.dropped_shown {
            return;
        }
        self.results.put(&DecoderFields {
            dropped: shown.then_some(self.dropped as i64),
        });
        if self.skipped_declared {
            self.results.put(&LookFields {
                skipped: shown.then_some(self.skipped as i64),
            });
        }
        self.dropped_shown = shown;
    }

    /// Runs `x` in line, splitting it at retunes, and reports when due.
    fn run_inline(
        &mut self,
        x: &[D::Input],
        first: u64,
        retunes: &[(usize, f64, f64)],
        tick: bool,
        pass_s: f64,
    ) {
        let due = self.report_due(x, tick, pass_s);
        let Some(dec) = self.dec.as_mut() else { return };
        let mut done = 0;
        for &(at, c, t) in retunes {
            dec.process(&x[done..at], first + done as u64, &mut self.out);
            done = at;
            if let Some(f) = &mut self.format {
                f.centre_hz = c;
                f.tuned_hz = t;
                dec.retune(f);
            }
            self.results.results_mut().lost_signal();
        }
        dec.process(&x[done..], first + done as u64, &mut self.out);
        if tick {
            dec.tick(&mut self.out);
        }
        if due {
            dec.report(&mut self.results);
        }
    }

    /// Applies what waited for the decoder to come back from a helper thread.
    fn apply_queued(&mut self) {
        let Some(dec) = self.dec.as_mut() else { return };
        if let Some((f, b)) = self.extra_held.as_mut() {
            if !b.is_empty() {
                dec.extra(f, b);
                b.advance();
            }
        }
        let queued = !self.queued.is_empty();
        for (id, v) in self.queued.drain(..) {
            // A value the decoder refuses leaves its setting as it was: the values below show that.
            let _ = dec.apply(id, &v);
        }
        if queued {
            self.values = dec.values();
        }
        if let Some(f) = self.pending_retune.take() {
            dec.retune(&f);
            self.results.results_mut().lost_signal();
        }
        if std::mem::take(&mut self.pending_clear) {
            dec.clear();
            dec.report(&mut self.results);
        }
        if let Some(kept) = self.pending_forget.take() {
            dec.forget(&kept);
            dec.report(&mut self.results);
        }
        self.show_dropped();
    }

    fn poll(&mut self) {
        let Some(t) = self.job.as_mut() else { return };
        let Some(r) = t.try_take() else { return };
        self.job = None;
        self.take_done(r);
    }
    /// Waits for the batch on the helper thread, if one is out, and takes its outputs and results.
    fn wait_job(&mut self) {
        if let Some(t) = self.job.take() {
            self.take_done(t.wait());
        }
    }
    fn take_done(&mut self, r: Result<Done<D>, crate::context::JobPanicked>) {
        match r {
            Ok(mut d) => {
                d.out.move_into(&mut self.out);
                self.results.results_mut().clone_from(d.results.results());
                d.input.clear();
                self.spare = d.input;
                self.dec = Some(d.dec);
                self.apply_queued();
            }
            Err(_) => {
                // The decoder panicked on the helper: it is gone. Say so; the component stays, producing nothing.
                let summary = GraphError::Stopped {
                    component: self.name.into(),
                }
                .to_string();
                let stopped =
                    sdrkit_core::Status::new(sdrkit_core::LockState::Off).with_summary(&summary);
                self.results.results_mut().set_status(&stopped);
            }
        }
    }

    fn start_batch(&mut self, batch: usize) {
        if self.job.is_some() || self.dec.is_none() {
            return;
        }
        let n = if batch == 0 {
            self.waiting.len()
        } else {
            batch
        };
        if n == 0 || self.waiting.len() < n {
            return;
        }
        let mut input = std::mem::take(&mut self.spare);
        input.clear();
        input.extend(self.waiting.drain(..n));
        let first = self.waiting_first;
        self.waiting_first += n as u64;
        let mut dec = self.dec.take().expect("checked above");
        let mut out = Outbox::new(&self.kinds);
        out.caps.clone_from(&self.out.caps);
        // The component keeps showing its results while the batch runs; the job reports into a copy.
        let mut results = self.results.clone();
        // A rated input reports after every batch. An input without a rate reports when a report fell due since the last batch.
        let report = self.rate > 0.0 || std::mem::take(&mut self.report_pending);
        self.job = Some(self.ctx.pool.spawn(move || {
            dec.process(&input, first, &mut out);
            if report {
                dec.report(&mut results);
            }
            Done {
                dec,
                input,
                out,
                results,
            }
        }));
    }
}

impl<D: Decoder> DecoderComponent<D> {
    /// Moves the decoded items into the component's outputs. An output without a rate gets the outbox's buffer whole when it
    /// is empty, and the outbox keeps room for the output's cap ([`Decoder::output_cap`]), so a decoder allocates nothing
    /// once both buffers have that room.
    fn hand_over(&mut self, io: &mut Io) {
        let max = io.max_items();
        for o in 0..self.out.len() {
            let cap = self
                .declared_caps
                .get(o)
                .copied()
                .flatten()
                .map_or(max, |c| c.min(max));
            self.out.caps[o] = cap;
            let src = self.out.data(o);
            if let Data::Audio(a) = src {
                a.channels = io.output_format(o).channels;
            }
            let dst = &mut io.output(o).data;
            let rated = matches!(
                dst,
                Data::Iq(_)
                    | Data::Real(_)
                    | Data::Audio(_)
                    | Data::Symbols(_)
                    | Data::HardBits(_)
                    | Data::Llr(_)
                    | Data::Spectrum(_)
            );
            if rated || !dst.is_empty() {
                dst.append(src);
                continue;
            }
            std::mem::swap(dst, src);
            match self.out.data(o) {
                Data::Bytes(v) => v.reserve(cap.saturating_sub(v.len())),
                Data::Messages(v) => v.reserve(cap.saturating_sub(v.len())),
                Data::Items(Some(v)) => v.reserve_total(cap),
                _ => {}
            }
        }
    }
}

impl<D: Decoder> Component for DecoderComponent<D> {
    /// A decoder without outputs (one that only reports) is a sink.
    fn info(&self) -> ComponentInfo {
        let mut input = vec![Port::new("in", self.input_kind).channels(D::Input::channels())];
        input.extend(self.extra.clone());
        let info = if self.outputs.is_empty() {
            ComponentInfo::sink(self.name, input)
        } else {
            ComponentInfo::processor(self.name, input, self.outputs.clone())
        };
        info.with_settings(Arc::clone(&self.schema)).manual_tags()
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A batch still out holds the decoder: wait for it to come back. Its outputs belong to the old format and are dropped
        // with the outbox below.
        self.wait_job();
        self.waiting.clear();
        let f = inputs[0].clone();
        let Some(dec) = self.dec.as_mut() else {
            return Err(ConfigError::Stage(StageError::new(GraphError::Stopped {
                component: self.name.into(),
            })));
        };
        let outs = dec.configure(&f)?;
        self.helper = dec.helper();
        if outs.len() != self.outputs.len() {
            return Err(ConfigError::Stage(StageError::new(
                GraphError::OutputCount {
                    component: self.name.into(),
                    stated: outs.len(),
                    outputs: self.outputs.len(),
                },
            )));
        }
        let kinds: Vec<Kind> = self
            .outputs
            .iter()
            .zip(&outs)
            .map(|(p, f)| match &p.format.kind {
                KindRule::Is(k) => *k,
                KindRule::OneOf(_) | KindRule::Any | KindRule::SameAs(_) => f.kind,
            })
            .collect();
        self.out = Outbox::new(&kinds);
        self.declared_caps = (0..kinds.len()).map(|o| dec.output_cap(o)).collect();
        self.kinds = kinds;
        self.rate = f.rate;
        self.since = 0.0;
        self.interval = dec.report_interval_s();
        self.last_report_t = f64::NEG_INFINITY;
        self.report_pending = false;
        self.format = Some(f);
        self.show_dropped();
        Ok(outs)
    }

    fn process(&mut self, io: &mut Io) {
        if self.extra.is_some() && io.is_connected(1) {
            let (f, items) = (io.input_format(1), io.input(1));
            match self.dec.as_mut() {
                Some(dec) => dec.extra(f, items),
                None if !items.is_empty() => match self.extra_held.as_mut() {
                    Some((held_f, held)) if !held.is_empty() && held_f == f => {
                        held.data.extend_copy(&items.data);
                    }
                    Some((held_f, held)) => {
                        held_f.clone_from(f);
                        held.copy_from(items);
                    }
                    None => {
                        let mut held = Buffer::new(&f.kind);
                        held.copy_from(items);
                        self.extra_held = Some((f.clone(), held));
                    }
                },
                None => {}
            }
        }
        let input = io.input(0);
        let first = input.first();
        let x = D::Input::read(io, 0);
        let tick = io.tick();
        let pass_s = io.pass_seconds();
        // Retunes, at their item.
        let mut retunes: [(usize, f64, f64); 4] = [(0, 0.0, 0.0); 4];
        let mut nr = 0;
        for t in input.tags() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                if nr < retunes.len() {
                    retunes[nr] = (
                        ((t.at.saturating_sub(first)) as usize).min(x.len()),
                        centre_hz,
                        tuned_hz,
                    );
                    nr += 1;
                }
            }
        }
        match self.threaded() {
            None => self.run_inline(x, first, &retunes[..nr], tick, pass_s),
            Some((batch, backlog)) => {
                if self.rate == 0.0 && self.report_due(x, tick, pass_s) {
                    self.report_pending = true;
                }
                self.poll();
                if nr > 0 {
                    // A retune: what waits belongs to the old frequency.
                    let (_, c, t) = retunes[nr - 1];
                    self.waiting.clear();
                    if let Some(mut f) = self.format.clone() {
                        f.centre_hz = c;
                        f.tuned_hz = t;
                        match self.dec.as_mut() {
                            Some(d) => {
                                d.retune(&f);
                                self.results.results_mut().lost_signal();
                            }
                            None => self.pending_retune = Some(f.clone()),
                        }
                        self.format = Some(f);
                    }
                    self.waiting_first = first + retunes[nr - 1].0 as u64;
                    self.waiting.extend(x[retunes[nr - 1].0..].iter().cloned());
                } else {
                    if self.waiting.is_empty() {
                        self.waiting_first = first;
                    }
                    self.waiting.extend(x.iter().cloned());
                }
                if let Some(d) = &self.dec {
                    self.looks = d.looks();
                }
                if self.waiting.len() > backlog {
                    let over = self.waiting.len() - backlog;
                    self.waiting.drain(..over);
                    self.waiting_first += over as u64;
                    // Between looks the decoder leaves the items out on purpose; else it was late.
                    if self.looks {
                        self.skipped += over as u64;
                    } else {
                        self.dropped += over as u64;
                    }
                }
                self.start_batch(batch);
                self.show_dropped();
            }
        }
        self.hand_over(io);
    }

    /// Consumes the last input, waits for the helper thread, decodes what waits for it in line, then calls
    /// [`Decoder::finish`] and [`Decoder::report`].
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        self.wait_job();
        if let Some(dec) = self.dec.as_mut() {
            if !self.waiting.is_empty() {
                dec.process(self.waiting.make_contiguous(), self.waiting_first, &mut self.out);
                self.waiting_first += self.waiting.len() as u64;
                self.waiting.clear();
            }
            dec.finish(&mut self.out);
            dec.report(&mut self.results);
            self.report_pending = false;
            self.since = 0.0;
        }
        self.show_dropped();
        self.hand_over(io);
    }

    fn commands(&self) -> &Commands {
        &self.commands
    }

    /// The decoder forgets what `kept` does not hold ([`Decoder::forget`]) and reports at once. While a helper thread
    /// holds the decoder, it runs when the decoder comes back.
    fn forget(&mut self, kept: &Kept) {
        match self.dec.as_mut() {
            Some(d) => {
                d.forget(kept);
                if kept.is_empty() {
                    self.results.results_mut().clear_log();
                }
                d.report(&mut self.results);
                self.show_dropped();
            }
            None => self.pending_forget = Some(kept.clone()),
        }
    }

    /// Runs `clear`: the decoder forgets what it decoded ([`Decoder::clear`]) and reports at once. While a helper thread holds
    /// the decoder, it runs when the decoder comes back. Runs a command of the decoder ([`Decoder::invoke`]) after the batch
    /// on a helper thread ends, and reports at once.
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        if DecoderCommand::from_id(id) == Some(DecoderCommand::Clear) {
            match self.dec.as_mut() {
                Some(d) => {
                    d.clear();
                    self.results.results_mut().clear_log();
                    d.report(&mut self.results);
                    self.show_dropped();
                }
                None => self.pending_clear = true,
            }
            return Ok(None);
        }
        if id.index() >= self.commands.len() {
            return Err(CommandError::NoSuchId { id });
        }
        self.wait_job();
        let Some(d) = self.dec.as_mut() else {
            return Err(CommandError::Failed {
                source: StageError::new(GraphError::Stopped {
                    component: self.name.into(),
                }),
            });
        };
        let own = CommandId(id.0 - DecoderCommand::COMMANDS.len() as u16);
        let reply = d.invoke(own, args)?;
        d.report(&mut self.results);
        self.show_dropped();
        Ok(reply)
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match self.dec.as_mut() {
            Some(d) => d.apply(id, value)?,
            None => {
                if self.schema.setting(id).is_none() {
                    return Err(SettingError::NoSuchId { id });
                }
                self.queued.push((id, value.clone()));
            }
        }
        self.values.set(id, value)?;
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        self.values.clone()
    }

    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.ctx = ctx.clone();
        if let Some(d) = self.dec.as_mut() {
            d.attach(ctx);
        }
    }

    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.dec.as_ref().and_then(D::save_state)
    }

    /// Waits for a batch on a helper thread, then passes the state to the decoder.
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.wait_job();
        let Some(d) = self.dec.as_mut() else {
            return Err(StateError::NoState);
        };
        d.restore_state(state)?;
        d.report(&mut self.results);
        self.show_dropped();
        Ok(())
    }
}
