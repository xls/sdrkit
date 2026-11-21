//! [`StackGraph`]: the receiver a stack returns to the engine when its decoder is a graph.
//!
//! It wraps a [`Graph`] and adds what every stack needs: minimum rates, the stack's results declaration and settings routes,
//! fixed-size passes, the input guard, the station context, host requests and saved state.

use crate::command::CommandTarget;
use crate::context::RunContext;
use crate::graph::{Graph, GraphError, SettingTarget};
use crate::guard::{first_bad, guard, DEFAULT_LIMIT};
use sdrkit_core::codec::CodecSet;
use sdrkit_core::interface::{Fields, StackShape};
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{
    publish::keys, CommandError, CommandId, Commands, Complex32, ConfigError, Facilities,
    InputInfo, LockState, Output, Receiver, Record, Results, ResultsDecl, Schema, SettingError,
    SettingId, StateBlob, StateError, Station, Status, Value, Values,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The version of the saved-state blob.
const STATE_VERSION: u32 = 1;

/// Where a setting of the stack goes.
#[derive(Clone, Debug)]
enum Route {
    /// To these component settings.
    Targets(Vec<SettingTarget>),
    /// To the generator: the receiver accepts it and does nothing.
    Generator,
    /// Nowhere yet: the receiver refuses it.
    None,
}

/// A stack's graph as a receiver.
///
/// # Example
///
/// ```
/// use sdrkit_graph::{components::*, GraphBuilder, StackGraph};
/// use sdrkit_core::{InputInfo, Output, Owner, Receiver, Value};
///
/// let g = GraphBuilder::new()
///     .add("chan", Channel::new(10e3, 24e3).unwrap())
///     .add("demod", AmDemod::default())
///     .add("audio", AudioResampler::new(48e3).unwrap())
///     .input("chan")
///     .connect("chan", "demod")
///     .connect("demod", "audio")
///     .output("audio")
///     .build()
///     .unwrap();
/// let mut rx = StackGraph::builder("am", g)
///     .min_rate(48e3, "AM needs at least 48 kHz of samples")
///     .build()
///     .unwrap();
/// assert!(rx.configure(&InputInfo::new(8e3, 0.0, 0.0)).is_err());
/// assert_eq!(rx.results().status().summary, "AM needs at least 48 kHz of samples");
/// rx.configure(&InputInfo::new(240e3, 1e6, 1e6)).unwrap();
/// let filter = rx.schema().id("chan.filter").unwrap();
/// sdrkit_core::owner::apply(&mut rx, filter, &Value::Choice(0)).unwrap();
/// let mut out = Output::default();
/// rx.feed(&vec![Default::default(); 2400], &mut out);
/// ```
pub struct StackGraph {
    graph: Graph,
    stack: &'static str,
    min_rate_hz: f64,
    refusal: String,
    min_live_hz: f64,
    live_refusal: String,
    ready: bool,
    /// The last configure was refused for its rate.
    refused: bool,
    /// Results while refused, or with the guard's count added.
    own: Results,
    pass: usize,
    held: Vec<Complex32>,
    /// The stack's settings.
    schema: Arc<Schema>,
    /// Where each setting of `schema` goes, by id.
    routes: Vec<Route>,
    /// The component commands to run on every retune, with their arguments.
    on_retune: Vec<(usize, CommandId, Values, bool)>,
    /// What survives [`Receiver::forget`]: the kept keys of the description of the stack.
    kept: Vec<String>,
    default_centre_hz: Option<f64>,
    limit: f32,
    bad_samples: u64,
    /// The description of the stack lists the count of the guard ([`GuardFields`]): the results hold it from the
    /// start, with 0 too.
    guard_shown: bool,
    scratch: Vec<Complex32>,
    /// The present values of `schema`.
    values: Values,
}

impl std::fmt::Debug for StackGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StackGraph")
            .field("stack", &self.stack)
            .field("graph", &self.graph)
            .finish()
    }
}

/// The setup of a [`StackGraph`]. [`StackGraphBuilder::build`] makes sure that every setting and command of the stack
/// resolves to the graph, and returns the receiver.
pub struct StackGraphBuilder {
    rx: StackGraph,
    commands: BTreeMap<String, CommandTarget>,
    /// The names of `commands` in the order of their declaration.
    order: Vec<String>,
    settings: Option<Values>,
    /// The interface description of the stack, if the receiver is made from one.
    shape: Option<StackShape>,
}

impl std::fmt::Debug for StackGraphBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StackGraphBuilder")
            .field("stack", &self.rx.stack)
            .finish()
    }
}

impl StackGraph {
    /// The setup of the receiver of stack `stack` (its id, used in saved state) over `graph`. Its settings are the graph's
    /// own until [`StackGraphBuilder::with_schema`] sets the stack's. Its commands are the graph's own until
    /// [`StackGraphBuilder::command`] names the stack's.
    pub fn builder(stack: &'static str, graph: Graph) -> StackGraphBuilder {
        let schema = Arc::clone(Owner::schema(&graph));
        let mut rx = StackGraph {
            values: Values::defaults(&schema),
            graph,
            stack,
            min_rate_hz: 0.0,
            refusal: String::new(),
            min_live_hz: 0.0,
            live_refusal: String::new(),
            ready: false,
            refused: false,
            own: Results::new(Arc::new(ResultsDecl::new())),
            pass: 0,
            held: Vec::new(),
            schema,
            routes: Vec::new(),
            on_retune: Vec::new(),
            kept: Vec::new(),
            default_centre_hz: None,
            limit: DEFAULT_LIMIT,
            bad_samples: 0,
            guard_shown: false,
            scratch: Vec::new(),
        };
        rx.resolve_routes();
        StackGraphBuilder {
            rx,
            commands: BTreeMap::new(),
            order: Vec::new(),
            settings: None,
            shape: None,
        }
    }
    /// The setup of the receiver of the stack that `shape` describes, over `graph`. The id, the settings and the
    /// declaration of the results come from `shape`. Give each setting that has no component setting of its key a
    /// [`StackGraphBuilder::route`], and each command of `shape` its target with [`StackGraphBuilder::command`].
    /// [`StackGraphBuilder::build`] fails if the receiver does not declare exactly the results and commands of
    /// `shape`, or if no component declares a field, a table, a series or a raster of `shape`.
    ///
    /// # Panics
    ///
    /// If the settings or the results of `shape` declare a key twice.
    pub fn of(shape: &StackShape, graph: Graph) -> StackGraphBuilder {
        let mut b = Self::builder(shape.id, graph)
            .with_schema(Arc::new(shape.schema()))
            .declare(&shape.results_decl());
        b.shape = Some(shape.clone());
        b
    }
    /// Changes the minimum rate (a stack whose need depends on a setting, such as sound-card input). It applies at the next
    /// configure.
    pub fn set_min_rate(&mut self, hz: f64) {
        self.min_rate_hz = hz;
    }
    /// Changes the minimum rate of live input (see [`StackGraphBuilder::min_rate_live`]). It applies at the next configure.
    pub fn set_min_rate_live(&mut self, hz: f64) {
        self.min_live_hz = hz;
    }
    /// Changes the host's audio codec plug-ins (see [`StackGraphBuilder::with_codecs`]).
    pub fn set_codecs(&mut self, codecs: CodecSet) {
        self.graph.set_codecs(codecs);
    }
    fn resolve_routes(&mut self) {
        let g = &self.graph;
        self.routes = self
            .schema
            .settings()
            .iter()
            .map(|s| {
                let key = s.field.key.as_ref();
                if key.starts_with("gen_") {
                    return Route::Generator;
                }
                match g.target(key) {
                    Ok(t) => Route::Targets(vec![t]),
                    Err(_) => Route::None,
                }
            })
            .collect();
    }
}

impl StackGraphBuilder {
    /// Refuses rates below `hz`, with `message` as the reason shown to the user (empty for the standard words).
    pub fn min_rate(mut self, hz: f64, message: &str) -> Self {
        self.rx.min_rate_hz = hz;
        self.rx.refusal = message.into();
        self
    }
    /// Refuses live rates below `hz`, with `message` as the reason shown to the user (empty for the standard words). It applies
    /// only when the input is live ([`InputInfo::live`]). A recording or a generator at a lower rate still runs, down to the
    /// minimum of [`StackGraphBuilder::min_rate`]. Use it for the rate a stack requires from a radio (several channels at once),
    /// when the signal itself decodes at a lower rate.
    ///
    /// ```
    /// use sdrkit_graph::{components::NullSink, GraphBuilder, StackGraph};
    /// use sdrkit_core::{InputInfo, Receiver};
    /// let g = GraphBuilder::new().add("sink", NullSink::default()).input("sink").build().unwrap();
    /// let mut rx = StackGraph::builder("demo", g)
    ///     .min_rate(250e3, "one channel needs 250 ksps")
    ///     .min_rate_live(1e6, "a live radio must give 1 Msps")
    ///     .build()
    ///     .unwrap();
    /// assert!(rx.configure(&InputInfo::new(600e3, 868e6, 868e6)).is_err()); // live
    /// assert!(rx.configure(&InputInfo::new(600e3, 868e6, 868e6).recording()).is_ok());
    /// assert!(rx.configure(&InputInfo::new(200e3, 868e6, 868e6).recording()).is_err());
    /// ```
    pub fn min_rate_live(mut self, hz: f64, message: &str) -> Self {
        self.rx.min_live_hz = hz;
        self.rx.live_refusal = message.into();
        self
    }
    /// Sets the stack's declaration of the graph's results: a declared field, table or series keeps its own key from any
    /// component, not `"<component>.<key>"` (see [`Graph::refresh_results`]).
    pub fn declare(mut self, decl: &ResultsDecl) -> Self {
        self.rx.graph.declare(decl);
        self
    }
    /// Sets the host's audio codec plug-ins for every component, through the run context ([`RunContext::codecs`]).
    pub fn with_codecs(mut self, codecs: CodecSet) -> Self {
        self.rx.graph.set_codecs(codecs);
        self
    }
    /// Feeds the graph in passes of exactly `samples` (the rest waits for the next call). At the end of the stream
    /// ([`Receiver::finish`]) the rest goes as a shorter last pass.
    pub fn fixed_passes(mut self, samples: usize) -> Self {
        self.rx.pass = samples.max(1);
        self.rx.held = Vec::with_capacity(self.rx.pass);
        self
    }
    /// The receiver's settings are `schema`, the stack's own. Each setting goes to the component setting of the same key (an
    /// alias of the graph or `"<component>.<key>"`). A key that starts with `gen_` belongs to the generator and is accepted and
    /// left alone. Give the others a [`StackGraphBuilder::route`].
    pub fn with_schema(mut self, schema: Arc<Schema>) -> Self {
        self.rx.values = Values::defaults(&schema);
        self.rx.schema = schema;
        self.rx.resolve_routes();
        self
    }
    /// Sends the stack's setting `key` to these component settings (aliases or `"<component>.<key>"`). Each gets the value.
    ///
    /// # Errors
    ///
    /// [`GraphError::Setting`] with [`SettingError::Unknown`] if the schema has no setting `key` or the graph has no such
    /// target.
    pub fn route(mut self, key: &str, targets: &[&str]) -> Result<Self, GraphError> {
        let id = self
            .rx
            .schema
            .id(key)
            .map_err(|error| GraphError::Setting {
                key: key.into(),
                error,
            })?;
        let mut ts = Vec::with_capacity(targets.len());
        for t in targets {
            ts.push(
                self.rx
                    .graph
                    .target(t)
                    .map_err(|error| GraphError::Setting {
                        key: (*t).into(),
                        error,
                    })?,
            );
        }
        if let Some(r) = self.rx.routes.get_mut(id.index()) {
            *r = Route::Targets(ts);
        }
        Ok(self)
    }
    /// Declares the stack's command `name` as a command alias of the graph for the component commands of `target`. Once a
    /// stack names one command, its commands are the ones it names, in the order of their declaration: a front end lists
    /// them in that order, so the command a person uses most comes first. [`StackGraphBuilder::build`] resolves the target, as
    /// [`GraphBuilder::command_alias`] does.
    ///
    /// [`GraphBuilder::command_alias`]: crate::GraphBuilder::command_alias
    pub fn command(mut self, name: &str, target: CommandTarget) -> Self {
        if self.commands.insert(name.into(), target).is_none() {
            self.order.push(name.into());
        }
        self
    }
    /// Runs the component commands of `target` with their default arguments on every retune, ordered by the host or
    /// not (`clear` of the tracker: a table of things heard on one frequency).
    ///
    /// The command `clear` does not run in a component of which the description of the stack keeps a field or a
    /// table ([`StackShape::kept`]). Such a component forgets through
    /// [`Component::forget`](crate::Component::forget) with its kept keys, as it does for a change of frequency by
    /// the host: its kept items stay.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnresolvedCommand`] if no component has the command. [`GraphError::RetuneCommand`] if a command needs
    /// an argument.
    pub fn on_retune(mut self, target: CommandTarget) -> Result<Self, GraphError> {
        let targets = self.rx.graph.command_targets_of(&target);
        if targets.is_empty() {
            return Err(GraphError::UnresolvedCommand {
                key: target.command().into(),
                target,
            });
        }
        for (node, cid) in targets {
            let commands = self.rx.graph.node_commands(node);
            let def = &commands.defs()[cid.index()];
            let args = commands.check(cid, &def.new_args()).map_err(|error| {
                GraphError::RetuneCommand {
                    target: target.clone(),
                    error,
                }
            })?;
            self.rx
                .on_retune
                .push((node, cid, args, target.command() == "clear"));
        }
        Ok(self)
    }
    /// The centre (and tuned) frequency to use when the input does not state one (a recording without metadata).
    pub fn default_centre(mut self, hz: f64) -> Self {
        self.rx.default_centre_hz = Some(hz);
        self
    }
    /// The input guard's limit on a sample's magnitude (see [`crate::guard`]).
    pub fn guard_limit(mut self, limit: f32) -> Self {
        self.rx.limit = limit;
        self
    }
    /// Applies the stack's settings (from its `StackConfig`) at build. [`StackGraphBuilder::build`] fails on a refused value.
    pub fn with_settings(mut self, values: &Values) -> Self {
        self.settings = Some(values.clone());
        self
    }
    /// Makes the receiver. It makes sure that every setting of the stack resolves to component settings or to the generator,
    /// and that every command of the stack resolves to a command of the graph.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnroutedSetting`] for the first setting that resolves to no component, [`GraphError::UnresolvedCommand`]
    /// or [`GraphError::CommandAliasArgs`] for the first command that does not resolve, [`GraphError::Setting`] for the first
    /// value of [`StackGraphBuilder::with_settings`] that is refused. [`GraphError::Shape`] if the receiver is made from an
    /// interface description ([`StackGraph::of`]) and its settings, results or commands are not those of the description.
    pub fn build(self) -> Result<StackGraph, GraphError> {
        let StackGraphBuilder {
            mut rx,
            commands,
            order,
            settings,
            shape,
        } = self;
        let unrouted = rx.routes.iter().position(|r| matches!(r, Route::None));
        if let Some(s) = unrouted.and_then(|i| rx.schema.setting(SettingId(i as u16))) {
            return Err(GraphError::UnroutedSetting {
                stack: rx.stack,
                key: s.field.key.to_string(),
            });
        }
        if !commands.is_empty() {
            rx.graph.set_command_aliases(commands, order)?;
        }
        if let Some(values) = settings {
            for (k, d) in values.schema().settings().iter().enumerate() {
                let key = &d.field.key;
                let Ok(id) = rx.schema.id(key) else {
                    continue;
                };
                if let Some(v) = values.value(SettingId(k as u16)) {
                    owner::apply(&mut rx, id, v).map_err(|error| GraphError::Setting {
                        key: key.to_string(),
                        error,
                    })?;
                }
            }
        }
        if let Some(shape) = &shape {
            if let Some(what) = difference(shape, &rx) {
                return Err(GraphError::Shape {
                    stack: rx.stack,
                    what,
                });
            }
            rx.kept = shape.kept_keys().map_err(|key| GraphError::Shape {
                stack: rx.stack,
                what: format!(
                    "the kept item {key} is no table, no result set and no field of a result set of the description"
                ),
            })?;
            rx.guard_shown = shape.receiver_keys().contains(&keys::BAD_SAMPLES.key);
        }
        // Until the first configure, the results are the declaration with the lock state off.
        rx.own = Results::new(Arc::clone(Owner::results(&rx.graph).decl()));
        if rx.guard_shown {
            rx.own.fields_mut().set(&keys::BAD_SAMPLES, 0);
        }
        Ok(rx)
    }
}

/// What `rx` declares that `shape` does not hold, or the reverse, and what `shape` holds that no component declares.
/// `None` if there is no difference.
fn difference(shape: &StackShape, rx: &StackGraph) -> Option<String> {
    let results = Owner::results(&rx.graph).decl();
    if let Some(what) = shape.difference(&rx.schema, results, Owner::commands(&rx.graph)) {
        return Some(what);
    }
    // The receiver writes the count of its guard itself, and no other field.
    let own = shape.receiver_keys();
    if let Some(key) = own.iter().find(|k| **k != keys::BAD_SAMPLES.key) {
        return Some(format!(
            "the results (the receiver does not write the field {key} of receiver_fields)"
        ));
    }
    for c in &shape.components {
        if let Some(key) = c
            .shared
            .iter()
            .find(|k| !rx.graph.component_declares(c.component, k, false))
        {
            return Some(format!(
                "the results (the component {} does not publish the shared field {key})",
                c.component
            ));
        }
    }
    for (table, writers) in &shape.table_writers {
        if let Some(w) = writers
            .iter()
            .find(|w| !rx.graph.component_declares(w, table, true))
        {
            return Some(format!(
                "the results (the component {w} does not publish the table {table})"
            ));
        }
    }
    let unpublished: Vec<String> = rx
        .graph
        .undeclared(results)
        .into_iter()
        .filter(|k| !own.contains(&k.as_str()))
        .collect();
    (!unpublished.is_empty())
        .then(|| format!("the results (described and published by no component: {unpublished:?})"))
}

/// The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with
/// [`StackShape::receiver_fields`], at its place among the result sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Fields)]
pub struct GuardFields {
    /// Input samples replaced because they were not finite or far beyond full scale.
    #[field(shared = keys::BAD_SAMPLES)]
    pub bad_samples: i64,
}

impl StackGraph {
    /// The graph.
    pub fn graph(&self) -> &Graph {
        &self.graph
    }
    /// The graph, to tap or inspect.
    pub fn graph_mut(&mut self) -> &mut Graph {
        &mut self.graph
    }
    /// Whether the receiver accepted its rate.
    pub fn ready(&self) -> bool {
        self.ready
    }
    /// Input samples fixed by the guard since the start.
    pub fn bad_samples(&self) -> u64 {
        self.bad_samples
    }
    /// Sets the graph's run context, from the host (its helper threads); the station and `live` are kept up to date by the
    /// receiver.
    pub fn set_context(&mut self, ctx: RunContext) {
        self.graph.set_context(ctx);
    }

    fn fix_input(&self, input: &InputInfo) -> InputInfo {
        let mut i = *input;
        if let Some(c) = self.default_centre_hz {
            if i.centre_hz == 0.0 || !i.centre_hz.is_finite() {
                i.centre_hz = c;
            }
            if i.tuned_hz == 0.0 || !i.tuned_hz.is_finite() {
                i.tuned_hz = i.centre_hz;
            }
        }
        i
    }

    fn run(&mut self, x: &[Complex32], out: &mut Output) {
        if self.pass == 0 || x.is_empty() {
            self.graph.feed(x, out);
            return;
        }
        let mut rest = x;
        if !self.held.is_empty() {
            let take = (self.pass - self.held.len()).min(rest.len());
            self.held.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.held.len() == self.pass {
                self.graph.feed(&self.held, out);
                self.held.clear();
            }
        }
        while rest.len() >= self.pass {
            self.graph.feed(&rest[..self.pass], out);
            rest = &rest[self.pass..];
        }
        self.held.extend_from_slice(rest);
    }

    /// Copies the graph's results into `own` and adds the guard's count.
    fn refresh_own(&mut self) {
        if !(self.ready && (self.bad_samples > 0 || self.guard_shown)) {
            return;
        }
        let src = Owner::results(&self.graph);
        let stale = self.own.decl().field_id(keys::BAD_SAMPLES.key).is_none()
            || self.own.fields().decl().len() < src.fields().decl().len()
            || self.own.decl().tables().len() != src.decl().tables().len()
            || self.own.decl().series_decls().len() != src.decl().series_decls().len()
            || self.own.decl().rasters().len() != src.decl().rasters().len()
            || self.own.decl().fields().fields()[..src.fields().decl().len()]
                != *src.fields().decl().fields();
        if stale {
            let mut decl = src.decl().as_ref().clone();
            if decl.field_id(keys::BAD_SAMPLES.key).is_none() {
                decl = decl.with_fields(&[&keys::BAD_SAMPLES]);
            }
            self.own = Results::new(Arc::new(decl));
        }
        self.own.copy_from(src);
        self.own
            .fields_mut()
            .set(&keys::BAD_SAMPLES, self.bad_samples as i64);
    }

    /// Merges the components' results now. They are merged on their own as the signal is fed, after an empty `feed`, and at
    /// [`Receiver::finish`].
    pub fn refresh_results(&mut self) {
        self.graph.refresh_results();
        self.refresh_own();
    }
}

impl Owner for StackGraph {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let r = self.apply_inner(id, value);
        if r.is_ok() {
            // The checked path validated the value against this schema.
            let _ = self.values.set(id, value);
            if self.ready {
                // A setting can change what is shown: merge now rather than at the next piece of signal.
                self.graph.refresh_results();
                self.refresh_own();
            }
        }
        r
    }

    fn results_decl(&self) -> &Arc<ResultsDecl> {
        Owner::results(self).decl()
    }

    fn results(&self) -> &Results {
        if !self.ready || self.bad_samples > 0 || self.guard_shown {
            &self.own
        } else {
            Owner::results(&self.graph)
        }
    }

    fn commands(&self) -> &Commands {
        Owner::commands(&self.graph)
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let r = Owner::invoke(&mut self.graph, id, args);
        if r.is_ok() && self.ready {
            self.graph.refresh_results();
            self.refresh_own();
        }
        r
    }

    fn problem(&mut self, text: &str) {
        self.graph.problem(text);
        if self.ready {
            self.refresh_own();
        } else {
            self.own.problem(text);
        }
    }
}

impl Receiver for StackGraph {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        let input = self.fix_input(input);
        self.held.clear();
        // The live minimum applies when it is the higher one and the samples come from a radio.
        let live_rule = input.live && self.min_live_hz > self.min_rate_hz;
        let need = if live_rule {
            self.min_live_hz
        } else {
            self.min_rate_hz
        };
        if !(input.rate_hz.is_finite() && input.rate_hz >= need && input.rate_hz > 0.0) {
            self.ready = false;
            let got = if input.rate_hz.is_finite() {
                input.rate_hz
            } else {
                0.0
            };
            let e = ConfigError::RateTooLow { need, got };
            let message = if live_rule && input.rate_hz >= self.min_rate_hz {
                &self.live_refusal
            } else {
                &self.refusal
            };
            self.refused = true;
            self.own = Results::new(Arc::clone(Owner::results(&self.graph).decl()));
            let summary = if message.is_empty() {
                e.to_string()
            } else {
                message.clone()
            };
            self.own
                .set_status(&Status::new(LockState::Off).with_summary(&summary));
            return Err(e);
        }
        if self.pass > 0 {
            let chunk = self.graph.max_chunk().max(self.pass);
            self.graph.set_max_chunk(chunk);
        }
        self.refused = false;
        self.graph.configure(&input)?;
        self.ready = true;
        Ok(())
    }

    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        if !self.ready {
            return;
        }
        match first_bad(iq, self.limit) {
            None => self.run(iq, out),
            Some(_) => {
                let mut x = std::mem::take(&mut self.scratch);
                x.clear();
                x.extend_from_slice(iq);
                self.bad_samples += guard(&mut x, self.limit) as u64;
                self.run(&x, out);
                self.scratch = x;
            }
        }
        self.refresh_own();
    }

    /// Runs the samples [`StackGraphBuilder::fixed_passes`] still holds as a shorter last pass, then [`Graph::finish`].
    fn finish(&mut self, out: &mut Output) {
        if !self.ready {
            return;
        }
        if !self.held.is_empty() {
            let held = std::mem::take(&mut self.held);
            self.graph.feed(&held, out);
            self.held = held;
            self.held.clear();
        }
        self.graph.finish(out);
        self.refresh_own();
    }

    fn retuned(&mut self, input: &InputInfo) {
        let input = self.fix_input(input);
        for (node, cid, args, clears) in &self.on_retune {
            if !(*clears && self.graph.forget_kept(*node, &self.kept)) {
                // A retune has no error path. A command that fails leaves its component as it was.
                let _ = self.graph.invoke_component(*node, *cid, args);
            }
        }
        self.held.clear();
        if self.ready {
            self.graph.retuned(&input);
        }
        self.refresh_own();
    }

    /// Tells each component to forget what the description of the stack does not keep ([`Component::forget`]).
    fn forget(&mut self) {
        self.graph.forget(&self.kept);
        self.refresh_results();
    }

    fn set_station(&mut self, station: &Station) {
        let mut ctx = self.graph.context().clone();
        ctx.station = Arc::new(station.clone());
        self.graph.set_context(ctx);
    }

    fn transmits(&self) -> bool {
        self.graph.transmits()
    }

    fn set_transmit(&mut self, on: bool) {
        self.graph.set_transmit(on);
    }

    fn set_facilities(&mut self, facilities: &Facilities) {
        let mut ctx = self.graph.context().clone();
        ctx.storage = facilities.storage.clone();
        ctx.clock = facilities.clock.clone();
        ctx.random = facilities.random.clone();
        ctx.codecs = facilities.codecs.clone();
        self.graph.set_context(ctx);
    }

    fn save_state(&self) -> Option<StateBlob> {
        let inner = self.graph.save_state()?;
        Some(StateBlob::new(
            self.stack,
            STATE_VERSION,
            serde_json::to_vec(&inner).expect("a state blob serialises"),
        ))
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let bytes = state.open(self.stack, STATE_VERSION)?;
        let inner: StateBlob = serde_json::from_slice(bytes).map_err(|_| StateError::Damaged)?;
        let r = self.graph.restore_state(&inner);
        self.refresh_own();
        r
    }

    fn discontinuity(&mut self, lost: u64) {
        self.graph.discontinuity_after(self.held.len(), lost);
    }

    fn staged(&mut self) -> Option<&mut dyn sdrkit_core::Staged> {
        Some(&mut self.graph)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(&mut self.graph)
    }
}

impl StackGraph {
    fn apply_inner(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match self.routes.get(id.index()) {
            Some(Route::Generator) => Ok(()),
            Some(Route::Targets(ts)) => {
                let mut first = Ok(());
                for &t in ts {
                    if let Err(e) = self.graph.apply_to(t, value) {
                        let inner = t.id();
                        if first.is_ok() {
                            first = Err(e.map_ids(|x| if x == inner { id } else { x }));
                        }
                    }
                }
                first
            }
            Some(Route::None) => Err(SettingError::Unsupported { id }),
            None => Err(SettingError::NoSuchId { id }),
        }
    }
}
