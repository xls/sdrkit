//! Graphs: components wired output to input, checked, configured and run on one thread.

use crate::adapt::{Catalog, InsertedAdapter, ValidateReport};
use crate::command::CommandTarget;
use crate::component::{
    no_settings, Applied, CentreRule, Component, ComponentInfo, Kept, KindRule, Role,
    RuleFault, TagCarry, TagPolicy,
};
use crate::connect::{check_format, check_kind, ComponentHandle, ConnectError, PortRef};
use crate::context::RunContext;
use crate::desc::{ComponentDesc, Description};
use crate::media::{picture_format, Codec, MediaFormat, MediaPacket, MediaSpec, Picture};
use crate::plan::GraphPlan;
use crate::stream::{Buffer, Data, Format, Kind, SampleType, TagKind, Timed};
use crate::tap::{tap, TapReader, TapWriter};
use sdrkit_core::codec::CodecSet;
use sdrkit_core::record::FieldDecl;
use sdrkit_core::results::copy_if_changed;
use sdrkit_core::settings::SchemaError;
use sdrkit_core::{CommandError, CommandId, Commands, Owner, Record};
use sdrkit_core::{
    Complex32, ConfigError, HostRequest, InputInfo, LockState, Output, Published, Receiver,
    Results, ResultsDecl, Schema, SettingError, SettingId, StageError, StateBlob, StateError,
    Station, Value, Values, ValuesError,
};
use sdrkit_core::{
    FieldId, RasterDecl, RasterId, SeriesDecl, SeriesId, Status, TableDecl, TableId,
};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

mod stage;
pub use stage::{Stage, StageProblem, AHEAD_BYTES, MAX_STAGES};
pub(crate) use stage::StageMark;
use stage::{run_node, Deployment, Pass};

/// The id the graph's own `Iq` input is known by in edges and taps (`"input"`).
pub const INPUT: &str = "input";

/// How an output format that a component stated disagrees with the output's declared rule ([`OutputRule`](crate::OutputRule)).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum OutputProblem {
    /// The rate.
    #[error("the rule gives {declared} Hz, configure stated {stated} Hz")]
    Rate {
        /// The rate the rule produces.
        declared: f64,
        /// The rate configure stated.
        stated: f64,
    },
    /// The channel count.
    #[error("the rule gives {declared} channels, configure stated {stated}")]
    Channels {
        /// The count the rule produces.
        declared: u16,
        /// The count configure stated.
        stated: u16,
    },
    /// The sample type.
    #[error("the port declares {declared:?} samples, configure stated {stated:?}")]
    SampleType {
        /// The type the port declares.
        declared: SampleType,
        /// The type configure stated.
        stated: Option<SampleType>,
    },
}

/// One side of a component: its inputs or its outputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The inputs.
    Input,
    /// The outputs.
    Output,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Side::Input => "input",
            Side::Output => "output",
        })
    }
}

/// Why the ports of a component do not fit its role or the rules for ports.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PortProblem {
    /// A generator has inputs.
    #[error("it has inputs")]
    HasInputs,
    /// A sink has outputs.
    #[error("it has outputs")]
    HasOutputs,
    /// A sink has no inputs.
    #[error("it has no inputs")]
    NoInputs,
    /// A generator has no outputs.
    #[error("it has no outputs")]
    NoOutputs,
    /// A processor has no inputs or no outputs.
    #[error("a processor needs inputs and outputs")]
    ProcessorNeedsBoth,
    /// An optional input does not declare one kind.
    #[error("optional input {port} must declare its kind")]
    OptionalKind {
        /// The input.
        port: String,
    },
    /// An optional input is not among the first 64 inputs.
    #[error("optional input {port} is not among the first 64 inputs")]
    OptionalBeyond64 {
        /// The input.
        port: String,
    },
    /// An output follows the kind of an input that is fed back.
    #[error("output {port} follows input {input}, which is fed back")]
    FollowsFeedback {
        /// The output.
        port: String,
        /// The input it follows.
        input: usize,
    },
    /// An output follows the kind of an input that the component does not have.
    #[error("output {port} follows input {input}, which it does not have")]
    FollowsMissing {
        /// The output.
        port: String,
        /// The input it follows.
        input: usize,
    },
    /// An output does not state its kind.
    #[error("output {port} must state its kind")]
    OutputKind {
        /// The output.
        port: String,
    },
    /// The rule of an output reads an input that the component does not have.
    #[error("the rule of output {port} reads input {input}, which it does not have")]
    RuleInput {
        /// The output.
        port: String,
        /// The input the rule reads.
        input: usize,
    },
    /// The rule of an output reads a setting that the component does not declare, or one whose kind or range it cannot use.
    #[error("the rule of output {port} reads setting {setting:?}, which it does not declare with a kind and range it can use")]
    RuleSetting {
        /// The output.
        port: String,
        /// The setting the rule reads.
        setting: SettingId,
    },
}

/// A place in a graph description file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescPosition {
    /// Component number `index` of `components`.
    Component {
        /// Its index.
        index: usize,
    },
    /// Setting `key` of component number `index`.
    ComponentSetting {
        /// The index of the component.
        index: usize,
        /// The key.
        key: String,
    },
    /// Edge number `index` of `edges`.
    Edge {
        /// Its index.
        index: usize,
    },
    /// Edge number `index` of `feedback`.
    Feedback {
        /// Its index.
        index: usize,
    },
    /// Output number `index` of `outputs`.
    Output {
        /// Its index.
        index: usize,
    },
    /// Alias `name` of `aliases`.
    Alias {
        /// The alias.
        name: String,
    },
    /// Command alias `name` of `command_aliases`.
    CommandAlias {
        /// The alias.
        name: String,
    },
    /// Setting `key` of `fixed`.
    Fixed {
        /// The key.
        key: String,
    },
    /// The `status` entry.
    Status,
}

impl std::fmt::Display for DescPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DescPosition::Component { index } => write!(f, "components[{index}]"),
            DescPosition::ComponentSetting { index, key } => {
                write!(f, "components[{index}].settings.{key}")
            }
            DescPosition::Edge { index } => write!(f, "edges[{index}]"),
            DescPosition::Feedback { index } => write!(f, "feedback[{index}]"),
            DescPosition::Output { index } => write!(f, "outputs[{index}]"),
            DescPosition::Alias { name } => write!(f, "aliases.{name}"),
            DescPosition::CommandAlias { name } => write!(f, "command_aliases.{name}"),
            DescPosition::Fixed { key } => write!(f, "fixed.{key}"),
            DescPosition::Status => f.write_str("status"),
        }
    }
}

fn names(v: &[String]) -> String {
    if v.is_empty() {
        "none".into()
    } else {
        v.join(", ")
    }
}

fn specs(v: &[MediaSpec]) -> String {
    v.iter()
        .map(MediaSpec::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Why a graph could not be built or configured. Each error names the components and ports involved.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum GraphError {
    /// Two components share an id.
    #[error("two components have the id \"{0}\"")]
    DuplicateId(String),
    /// An id that cannot be used.
    #[error("\"{0}\" cannot be a component id: use letters, digits, '_' and '-', not \".\", and not \"input\"")]
    BadId(String),
    /// An edge, alias, setting or tap names a component that does not exist.
    #[error("there is no component \"{0}\"")]
    UnknownComponent(String),
    /// A port that the component does not have.
    #[error("component \"{component}\" has no {side} \"{port}\" (its {side}s: {})", names(.available))]
    UnknownPort {
        /// The component.
        component: String,
        /// Inputs or outputs.
        side: Side,
        /// The requested port.
        port: String,
        /// The names of the ports it has.
        available: Vec<String>,
    },
    /// An endpoint without a port, on a component with several.
    #[error("component \"{component}\" has several {side}s ({}): name one as \"{component}.<port>\"", names(.available))]
    AmbiguousPort {
        /// The component.
        component: String,
        /// Inputs or outputs.
        side: Side,
        /// The names of the ports it has.
        available: Vec<String>,
    },
    /// An input left without an edge.
    #[error("input {0} is not connected")]
    Unconnected(String),
    /// A stack does not declare what its interface description holds ([`StackGraph::of`](crate::StackGraph::of)).
    #[error("the stack \"{stack}\" differs from its interface description: {what}")]
    Shape {
        /// The stack id.
        stack: &'static str,
        /// What differs.
        what: String,
    },
    /// A setting of a stack that resolves to no component setting.
    #[error("stack {stack}: the setting \"{key}\" goes to no component")]
    UnroutedSetting {
        /// The stack id.
        stack: &'static str,
        /// The setting's key.
        key: String,
    },
    /// An output format that [`Component::configure`] stated against the output's declared rule.
    #[error("component \"{component}\": output {output}: {problem}")]
    OutputFormat {
        /// The component.
        component: String,
        /// The output port.
        output: String,
        /// What disagrees.
        problem: OutputProblem,
    },
    /// A command alias whose target matches no component command.
    #[error("the command \"{key}\" goes to {target}, which no component of the graph has")]
    UnresolvedCommand {
        /// The alias.
        key: String,
        /// The component command it names.
        target: CommandTarget,
    },
    /// A command to run on every retune that needs an argument.
    #[error("the retune command {target} is refused: {error}")]
    RetuneCommand {
        /// The component command.
        target: CommandTarget,
        /// Why its default arguments are refused.
        error: CommandError,
    },
    /// A command alias that goes to component commands with different arguments or replies.
    #[error("the command \"{key}\" goes to {target}, whose commands take different arguments")]
    CommandAliasArgs {
        /// The alias.
        key: String,
        /// The component commands it names.
        target: CommandTarget,
    },
    /// A connection that its input refuses, found when the output's format is known.
    #[error("cannot connect {from} to {to}: {error}")]
    Connect {
        /// The output.
        from: String,
        /// The input.
        to: String,
        /// Why, with the handles, the port indices and both formats.
        error: Box<ConnectError>,
    },
    /// The edges form a loop.
    #[error("the graph has a cycle through {}", .0.join(", "))]
    Cycle(Vec<String>),
    /// The format of a stream that an input needs is not known when the graph is validated: a sampled stream without a
    /// rate.
    #[error("the format of {port} ({kind}) is not resolved for {input}")]
    UnresolvedFormat {
        /// The output.
        port: String,
        /// Its kind.
        kind: Kind,
        /// The input that needs it.
        input: String,
    },
    /// A component's ports do not fit its role or the rules for ports.
    #[error("component \"{component}\" is a {role:?} but {problem}")]
    Role {
        /// The component.
        component: String,
        /// Its role.
        role: Role,
        /// What is wrong.
        problem: PortProblem,
    },
    /// A graph output of a kind that cannot leave the graph.
    #[error("{port} is {kind}: a graph output must be audio, messages, bytes or a typed stream (tap other kinds instead)")]
    BadOutput {
        /// The output.
        port: String,
        /// Its kind.
        kind: Kind,
    },
    /// A setting that was refused.
    #[error("setting {key}: {error}")]
    Setting {
        /// The key, as the caller supplied it.
        key: String,
        /// Why.
        error: SettingError,
    },
    /// A setting value of a description that does not fit the schema of its component.
    #[error("setting {key}: {error}")]
    SettingValue {
        /// The key.
        key: String,
        /// Why.
        error: ValuesError,
    },
    /// A component refused its input formats.
    #[error("component \"{component}\": {error}")]
    Config {
        /// The component.
        component: String,
        /// Why.
        error: ConfigError,
    },
    /// A component got an input of a kind it does not take. Components give it inside [`ConfigError::Stage`]
    /// ([`GraphError::input_kind`]).
    #[error("input {input} gets {got}, which the component does not take")]
    InputKind {
        /// The input.
        input: usize,
        /// The kind it got.
        got: Kind,
    },
    /// A media input gets a codec or packaging that it does not accept.
    #[error("component \"{component}\": input {input} gets {got}, it accepts {}", specs(.accepts))]
    InputMedia {
        /// The component.
        component: String,
        /// The input.
        input: String,
        /// What it gets.
        got: MediaSpec,
        /// What it accepts.
        accepts: Vec<MediaSpec>,
    },
    /// A component stated another number of output formats than it has outputs.
    #[error("component \"{component}\" stated {stated} output formats for {outputs} outputs")]
    OutputCount {
        /// The component.
        component: String,
        /// The formats it stated.
        stated: usize,
        /// Its outputs.
        outputs: usize,
    },
    /// A component stated an output format of another kind than the port declares.
    #[error("component \"{component}\" stated {stated} for output {output}, declared {declared}")]
    OutputKind {
        /// The component.
        component: String,
        /// The output.
        output: String,
        /// The kind stated.
        stated: Kind,
        /// The kind declared.
        declared: Kind,
    },
    /// A byte output leaves the graph as media of a codec that the host does not decode.
    #[error("component \"{component}\": output {output} leaves the graph as {codec} media, a codec the host does not decode")]
    OutputNotDecoded {
        /// The component.
        component: String,
        /// The output.
        output: String,
        /// The codec.
        codec: Codec,
    },
    /// A component stopped after an internal error, and cannot be configured again.
    #[error("component \"{component}\" stopped after an internal error")]
    Stopped {
        /// The component.
        component: String,
    },
    /// The registry has no component of this type.
    #[error("there is no component type \"{0}\" in the registry")]
    UnknownType(String),
    /// A description file that is not valid JSON of the description form.
    #[error("description, line {line} column {column}: {category}")]
    Syntax {
        /// The line, from 1.
        line: usize,
        /// The column, from 1.
        column: usize,
        /// What is wrong.
        category: SyntaxError,
    },
    /// A component in a description that is not the role the description returns.
    #[error("component \"{component}\" is described as a {described:?}, but it is a {actual:?}")]
    RoleMismatch {
        /// The component.
        component: String,
        /// The role in the description.
        described: Role,
        /// Its own role.
        actual: Role,
    },
    /// The graph's own settings do not make a schema.
    #[error("the graph settings: {0}")]
    Schema(SchemaError),
    /// The adapters of a stack configuration are not an adapter [`Catalog`] of this crate.
    #[error("the adapters given are not an adapter catalog of sdrkit-graph")]
    ForeignAdapters,
    /// A stage boundary that cannot be where the stack marked it.
    #[error("stage \"{stage}\": {problem}")]
    Stage {
        /// The stage that begins at the boundary.
        stage: String,
        /// Why.
        problem: StageProblem,
    },
    /// A stage thread ended in a panic, and its components are gone.
    #[error("a stage thread failed")]
    StageFailed,
    /// An entry of a description that does not fit.
    #[error("description, {at}: {error}")]
    Description {
        /// Where.
        at: DescPosition,
        /// Why.
        error: Box<GraphError>,
    },
}

/// What is wrong with the text of a description file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SyntaxError {
    /// It is not JSON.
    #[error("not JSON")]
    Json,
    /// It is JSON, but not of the description form (a missing or unknown entry, a wrong type).
    #[error("not a graph description")]
    Form,
    /// The text ends too early.
    #[error("the text ends too early")]
    Eof,
}

impl GraphError {
    /// The error a component returns from [`Component::configure`] for input `input` of kind `got`, which it does not take.
    pub fn input_kind(input: usize, got: Kind) -> ConfigError {
        ConfigError::Stage(StageError::new(GraphError::InputKind { input, got }))
    }
    pub(crate) fn at(self, at: DescPosition) -> GraphError {
        GraphError::Description {
            at,
            error: Box::new(self),
        }
    }
}

impl From<GraphError> for ConfigError {
    fn from(e: GraphError) -> Self {
        match e {
            GraphError::Config { error, .. } => error,
            GraphError::Setting { error, .. } => ConfigError::Setting(error),
            e => ConfigError::Stage(StageError::new(e)),
        }
    }
}

impl std::fmt::Display for KindRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KindRule::Is(k) => write!(f, "{k}"),
            KindRule::OneOf(ks) => {
                for (i, k) in ks.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" or ")?;
                    }
                    write!(f, "{k}")?;
                }
                Ok(())
            }
            KindRule::Any => f.write_str("any kind"),
            KindRule::SameAs(n) => write!(f, "the kind of input {n}"),
        }
    }
}

struct Pending {
    id: String,
    comp: Box<dyn Component>,
}

/// Builds a [`Graph`] in code. Every method chains; all checks happen in [`build`](Self::build).
///
/// Endpoints are `"<component>.<port>"`, or just `"<component>"` when it has one port on that side. The graph's `Iq` input is
/// [`INPUT`].
///
/// ```
/// use sdrkit_graph::{components::*, GraphBuilder};
/// let graph = GraphBuilder::new()
///     .add("chan", Channel::new(10e3, 24e3).unwrap())
///     .add("demod", AmDemod::default())
///     .add("audio", AudioResampler::new(48e3).unwrap())
///     .input("chan")
///     .connect("chan", "demod")
///     .connect("demod", "audio")
///     .output("audio")
///     .alias("width", "chan.filter")
///     .build()
///     .unwrap();
/// # let _ = graph;
/// ```
#[derive(Default)]
pub struct GraphBuilder {
    comps: Vec<Pending>,
    edges: Vec<(String, String)>,
    feedback: Vec<(String, String)>,
    outputs: Vec<String>,
    settings: Vec<(String, Value)>,
    aliases: BTreeMap<String, String>,
    command_aliases: BTreeMap<String, CommandTarget>,
    fixed: BTreeMap<String, Value>,
    status: Option<String>,
    clock_s: Option<f64>,
    reconfigure_on_retune: bool,
    declared: Option<Arc<ResultsDecl>>,
    catalog: Option<Arc<Catalog>>,
    own: Option<Published>,
    stages: Vec<(String, String, Stage)>,
    first_stage: Option<String>,
    costs: Vec<(String, f64)>,
}

impl GraphBuilder {
    /// Marks the edge from `from` to `to` as a stage boundary: the stage `stage` begins at the input of `to`. The host may
    /// run the components from there on another thread. The output of the graph is the same either way. No feedback edge
    /// may join two stages, and no component after a boundary may read the input of the graph.
    pub fn stage(mut self, from: &str, to: &str, stage: Stage) -> Self {
        self.stages.push((from.to_owned(), to.to_owned(), stage));
        self
    }

    /// Names the first stage of the graph. The default is `front`.
    pub fn first_stage(mut self, name: &str) -> Self {
        self.first_stage = Some(name.to_owned());
        self
    }

    /// Sets the recorded cost of each stage from the cost file `table` of the family, for `stack` in `variant`.
    pub fn costs(mut self, table: &sdrkit_core::CostTable, stack: &str, variant: &str) -> Self {
        self.costs = table
            .rows()
            .iter()
            .filter(|r| r.stack == stack && r.variant == variant)
            .map(|r| (r.stage.clone(), r.units))
            .collect();
        self
    }

    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds a component under `id`.
    pub fn add(self, id: &str, comp: impl Component + 'static) -> Self {
        self.add_boxed(id, Box::new(comp))
    }
    /// Adds a boxed component under `id`.
    pub fn add_boxed(mut self, id: &str, comp: Box<dyn Component>) -> Self {
        self.comps.push(Pending {
            id: id.into(),
            comp,
        });
        self
    }
    /// Connects an output to an input.
    pub fn connect(mut self, from: &str, to: &str) -> Self {
        self.edges.push((from.into(), to.into()));
        self
    }
    /// Connects an output to an input of an earlier component, closing a loop: the input sees what the output produced in the
    /// **previous** pass (the edge is not part of the run order, so it makes no cycle). Use it for slow control from a later
    /// component to an earlier one (a measured SNR choosing a demodulator's path); with a [`clock`](Self::clock), what is decided at
    /// the end of one period acts from the start of the next, at the same sample whatever the cuts. A feedback input is never worked
    /// on in place, and its format is the one its producer had at the previous configure.
    pub fn feedback(mut self, from: &str, to: &str) -> Self {
        self.feedback.push((from.into(), to.into()));
        self
    }
    /// Sets a clock of `seconds` on the graph: passes are cut at every multiple of `round(seconds x input rate)` input samples, counted
    /// from the configure (and from each retune), and the pass that ends on one is marked ([`Io::tick`]). Components take their
    /// periodic decisions (reports, path choices) at a tick, so they happen at the same sample whatever the cuts. The merged results
    /// are refreshed after each tick.
    pub fn clock(mut self, seconds: f64) -> Self {
        self.clock_s = (seconds > 0.0).then_some(seconds);
        self
    }
    /// Makes [`Receiver::retuned`] configure the whole graph again (every component starts fresh, the results are refreshed at once)
    /// instead of passing a `Retuned` tag down the graph. For a stack whose results belong to one frequency.
    pub fn reconfigure_on_retune(mut self) -> Self {
        self.reconfigure_on_retune = true;
        self
    }
    /// Feeds the graph's `Iq` input to `to` (call it again to feed more inputs).
    pub fn input(self, to: &str) -> Self {
        self.connect(INPUT, to)
    }
    /// Makes an output a graph output: audio goes to `Output::audio`, messages to `Output::events`, bytes to `Output::media`, typed
    /// items (that convert to JSON) to `Output::events`.
    pub fn output(mut self, from: &str) -> Self {
        self.outputs.push(from.into());
        self
    }
    /// Applies setting `key` before the graph starts. The key is an alias or `"<component>.<key>"`, resolved at
    /// [`build`](Self::build).
    pub fn set(mut self, key: &str, value: Value) -> Self {
        self.settings.push((key.into(), value));
        self
    }
    /// Exposes `"<component>.<key>"` under `name` (`"width"` for `"chan.filter"`). The alias resolves to the component and its
    /// setting id at [`build`](Self::build). When a graph has aliases, its [`schema`](Graph::schema) holds only them.
    pub fn alias(mut self, name: &str, target: &str) -> Self {
        self.aliases.insert(name.into(), target.into());
        self
    }
    /// Exposes the component commands of `target` under `name`. The alias resolves at [`build`](Self::build). When a graph
    /// has command aliases, its [`commands`](Owner::commands) hold only them.
    pub fn command_alias(mut self, name: &str, target: CommandTarget) -> Self {
        self.command_aliases.insert(name.into(), target);
        self
    }
    /// Sets `"<component>.<key>"` and refuses later changes to it.
    pub fn fix(mut self, key: &str, value: Value) -> Self {
        self.fixed.insert(key.into(), value);
        self
    }
    /// Takes the graph's status (lock state, SNR, level, offset), log, detail and plain field names from this component.
    pub fn status_from(mut self, id: &str) -> Self {
        self.status = Some(id.into());
        self
    }
    /// Sets results of the graph's own (a stack's presets, channels and other tables that no component makes). They are
    /// merged under their own keys before the components' results, so they exist before the graph is configured and stay
    /// as they are written ([`Graph::own_results_mut`]).
    pub fn publish(mut self, own: Published) -> Self {
        self.own = Some(own);
        self
    }
    /// Uses the adapter catalog `catalog` (usually [`Registry::catalog`](crate::Registry::catalog) of the host's registry) in
    /// place of the standard one, so the adapters that other crates register serve this graph.
    pub fn catalog(mut self, catalog: &Arc<Catalog>) -> Self {
        self.catalog = Some(Arc::clone(catalog));
        self
    }
    /// Sets the stack's declaration of the graph's results. A field, table or series that the declaration names is merged
    /// under its own key from whichever component publishes it, not under `"<component>.<key>"`. See [`Graph::declare`].
    pub fn declare(mut self, decl: &ResultsDecl) -> Self {
        self.declared = Some(Arc::new(decl.clone()));
        self
    }

    /// Builds the graph through a [`GraphPlan`]: names resolve to handles there, and each step is checked as the plan checks it.
    /// Then the plan checks what needs the whole graph (see [`GraphPlan::build`]).
    ///
    /// # Errors
    ///
    /// The first refused step, as a [`GraphError`]. A refused connection is [`GraphError::Connect`] with the endpoint names.
    pub fn build(self) -> Result<Graph, GraphError> {
        let mut p = GraphPlan::new();
        if let Some(c) = &self.catalog {
            p.set_catalog(c);
        }
        for c in self.comps {
            p.add_component(&c.id, c.comp)?;
        }
        let edges = self.edges.iter().map(|e| (e, false));
        for ((from, to), back) in edges.chain(self.feedback.iter().map(|e| (e, true))) {
            let (f, t) = (p.output_port(from)?, p.input_port(to)?);
            let done = if back {
                p.feedback(f, t)
            } else {
                p.connect(f, t)
            };
            done.map_err(|error| GraphError::Connect {
                from: from.clone(),
                to: to.clone(),
                error: Box::new(error),
            })?;
        }
        if let Some(c) = self.clock_s {
            p.clock(c);
        }
        if self.reconfigure_on_retune {
            p.reconfigure_on_retune();
        }
        for o in &self.outputs {
            let f = p.output_port(o)?;
            p.output(f).map_err(|error| GraphError::Connect {
                from: o.clone(),
                to: String::new(),
                error: Box::new(error),
            })?;
        }
        for (name, target) in &self.aliases {
            p.alias(name, target)?;
        }
        for (name, target) in &self.command_aliases {
            p.command_alias(name, target.clone())?;
        }
        for (key, v) in &self.fixed {
            p.fix(key, v)?;
        }
        for (key, v) in &self.settings {
            p.set_key(key, v)?;
        }
        if let Some(s) = &self.status {
            p.status_from(s)?;
        }
        if let Some(d) = self.declared {
            p.declare(d);
        }
        for (from, to, stage) in self.stages {
            // A mark names two components; a port after the id is allowed and not needed.
            let id = |end: &str| end.split('.').next().unwrap_or(end).to_owned();
            let (f, t) = (p.handle(&id(&from))?, p.handle(&id(&to))?);
            p.stage(f, t, stage);
        }
        if let Some(name) = &self.first_stage {
            p.first_stage(name);
        }
        p.costs(self.costs);
        let mut g = p.build()?;
        if let Some(own) = self.own {
            g.own = Some(own);
            g.rebuild_merge();
            g.refresh_results();
        }
        Ok(g)
    }
}

/// What a [`GraphPlan`] returns to make a [`Graph`]: every name is resolved to a component index and a setting id.
pub(crate) struct Assembly {
    pub catalog: Arc<Catalog>,
    /// The components, in the order they were added: id, declaration, component.
    pub comps: Vec<(String, ComponentInfo, Box<dyn Component>)>,
    pub edges: Vec<(PortRef, PortRef)>,
    pub feedback: Vec<(PortRef, PortRef)>,
    pub outputs: Vec<PortRef>,
    /// Alias name to its target text, for the description.
    pub aliases: BTreeMap<String, String>,
    /// The target of each alias, in the order of `aliases`.
    pub alias_targets: Vec<(usize, SettingId)>,
    /// Command alias name to its target text.
    pub command_aliases: BTreeMap<String, CommandTarget>,
    /// `"<component>.<key>"` to its fixed value, for the description.
    pub fixed: BTreeMap<String, Value>,
    pub fixed_targets: Vec<(usize, SettingId)>,
    pub status: Option<usize>,
    pub clock_s: Option<f64>,
    pub reconfigure_on_retune: bool,
    pub declared: Option<Arc<ResultsDecl>>,
    /// The marked stage boundaries, the name of the first stage and the recorded cost of each stage by name.
    pub marks: Vec<StageMark>,
    pub first_stage: Option<String>,
    pub costs: Vec<(String, f64)>,
}

/// Makes the graph of a plan. It checks what needs the whole graph: every required input connected (an optional input left
/// open reads an empty stream), no cycle, the kinds that depend on other connections, and the kinds of the graph outputs.
pub(crate) fn assemble(a: Assembly) -> Result<Graph, GraphError> {
    let Assembly {
        catalog,
        comps,
        edges,
        feedback,
        outputs,
        aliases,
        alias_targets,
        command_aliases,
        fixed,
        fixed_targets,
        status,
        clock_s,
        reconfigure_on_retune,
        declared,
        marks,
        first_stage,
        costs,
    } = a;
    let mut nodes: Vec<Node> = Vec::with_capacity(comps.len());
    let mut by_id: BTreeMap<String, usize> = BTreeMap::new();
    for (id, info, comp) in comps {
        by_id.insert(id.clone(), nodes.len());
        nodes.push(Node {
            inputs: vec![usize::MAX; info.inputs.len()],
            feedback: vec![false; info.inputs.len()],
            unconnected: 0,
            outputs: Vec::new(),
            kinds: Vec::new(),
            carries: Vec::new(),
            scratch: Vec::with_capacity(info.outputs.len()),
            meter: Arc::default(),
            in_count: Vec::new(),
            id,
            values: comp.values(),
            info,
            comp,
        });
    }

    // Streams: 0 is the graph's input, then one per output port.
    let mut names = vec![INPUT.to_string()];
    for n in nodes.iter_mut() {
        for port in &n.info.outputs {
            n.outputs.push(names.len());
            names.push(format!("{}.{}", n.id, port.name));
        }
    }
    let mut producer: Vec<Option<usize>> = vec![None];
    for (i, n) in nodes.iter().enumerate() {
        producer.extend(n.outputs.iter().map(|_| Some(i)));
    }
    let stream_of = |nodes: &[Node], r: PortRef| match r.component.index() {
        None => 0,
        Some(i) => nodes[i].outputs[r.port],
    };

    // Edges: the plan has checked each one.
    let mut consumers = vec![0usize; names.len()];
    let mut edge_names = Vec::with_capacity(edges.len());
    let mut feedback_names = Vec::with_capacity(feedback.len());
    let mut sources = Vec::with_capacity(edges.len() + feedback.len());
    let n_edges = edges.len();
    for (k, &(from, to)) in edges.iter().chain(&feedback).enumerate() {
        let is_feedback = k >= n_edges;
        let s = stream_of(&nodes, from);
        let dst = to.component.0 as usize;
        let to_name = format!("{}.{}", nodes[dst].id, nodes[dst].info.inputs[to.port].name);
        nodes[dst].inputs[to.port] = s;
        nodes[dst].feedback[to.port] = is_feedback;
        sources.push((dst, to.port, from));
        consumers[s] += 1;
        if is_feedback {
            feedback_names.push((names[s].clone(), to_name));
        } else {
            edge_names.push((names[s].clone(), to_name));
        }
    }
    // An optional input left unconnected reads a stream of its own that no component writes: it stays empty, with the
    // format of the port's declared kind.
    let mut open_kinds: Vec<(usize, Kind)> = Vec::new();
    for n in nodes.iter_mut() {
        for p in 0..n.inputs.len() {
            if n.inputs[p] != usize::MAX {
                continue;
            }
            let port = &n.info.inputs[p];
            let name = format!("{}.{}", n.id, port.name);
            if port.optional.is_none() {
                return Err(GraphError::Unconnected(name));
            }
            let kind = match &port.format.kind {
                KindRule::Is(k) => *k,
                KindRule::OneOf(ks) if !ks.is_empty() => ks[0],
                KindRule::OneOf(_) | KindRule::Any | KindRule::SameAs(_) => {
                    return Err(GraphError::Role {
                        component: n.id.clone(),
                        role: n.info.role,
                        problem: PortProblem::OptionalKind {
                            port: port.name.clone(),
                        },
                    })
                }
            };
            if p >= 64 {
                return Err(GraphError::Role {
                    component: n.id.clone(),
                    role: n.info.role,
                    problem: PortProblem::OptionalBeyond64 {
                        port: port.name.clone(),
                    },
                });
            }
            n.inputs[p] = names.len();
            n.unconnected |= 1 << p;
            open_kinds.push((names.len(), kind));
            names.push(format!("{name} (unconnected)"));
            producer.push(None);
            consumers.push(0);
        }
    }

    // Topological order (Kahn); what is left over lies on a cycle.
    let mut indeg: Vec<usize> = nodes
        .iter()
        .map(|n| {
            n.inputs
                .iter()
                .zip(&n.feedback)
                .filter(|&(&s, &fb)| producer[s].is_some() && !fb)
                .count()
        })
        .collect();
    let mut order = Vec::with_capacity(nodes.len());
    let mut ready: Vec<usize> = (0..nodes.len()).filter(|&i| indeg[i] == 0).rev().collect();
    while let Some(i) = ready.pop() {
        order.push(i);
        for (j, n) in nodes.iter().enumerate() {
            for (&s, &fb) in n.inputs.iter().zip(&n.feedback) {
                if producer[s] == Some(i) && !fb {
                    indeg[j] -= 1;
                    if indeg[j] == 0 {
                        ready.insert(0, j);
                    }
                }
            }
        }
    }
    if order.len() < nodes.len() {
        let stuck = (0..nodes.len())
            .filter(|i| !order.contains(i))
            .map(|i| nodes[i].id.clone())
            .collect();
        return Err(GraphError::Cycle(stuck));
    }

    // Kinds, in order.
    let mut kinds: Vec<Option<Kind>> = vec![None; names.len()];
    kinds[0] = Some(Kind::Iq);
    for (s, k) in open_kinds {
        kinds[s] = Some(k);
    }
    for &i in &order {
        let n = &nodes[i];
        let mut in_kinds = Vec::with_capacity(n.inputs.len());
        for (p, &s) in n.inputs.iter().enumerate() {
            // A fed-back input is checked below, once every kind is known.
            in_kinds.push(if n.feedback[p] {
                Kind::Real
            } else {
                kinds[s].expect("producers come first")
            });
        }
        let mut outs = Vec::with_capacity(n.outputs.len());
        for (o, port) in n.info.outputs.iter().enumerate() {
            let k = match &port.format.kind {
                KindRule::Is(k) => *k,
                KindRule::SameAs(i) if n.feedback.get(*i) == Some(&true) => {
                    return Err(GraphError::Role {
                        component: n.id.clone(),
                        role: n.info.role,
                        problem: PortProblem::FollowsFeedback {
                            port: port.name.clone(),
                            input: *i,
                        },
                    })
                }
                KindRule::SameAs(i) => {
                    in_kinds.get(*i).copied().ok_or_else(|| GraphError::Role {
                        component: n.id.clone(),
                        role: n.info.role,
                        problem: PortProblem::FollowsMissing {
                            port: port.name.clone(),
                            input: *i,
                        },
                    })?
                }
                KindRule::OneOf(_) | KindRule::Any => {
                    return Err(GraphError::Role {
                        component: n.id.clone(),
                        role: n.info.role,
                        problem: PortProblem::OutputKind {
                            port: port.name.clone(),
                        },
                    })
                }
            };
            kinds[n.outputs[o]] = Some(k);
            outs.push(k);
        }
        nodes[i].kinds = outs;
    }
    let kinds: Vec<Kind> = kinds
        .into_iter()
        .map(|k| k.expect("every stream is the input or the output of a component"))
        .collect();
    // The kinds that depended on connections made later, with the error that connect returns.
    for &(dst, port, from) in &sources {
        let n = &nodes[dst];
        let s = n.inputs[port];
        let to = PortRef::new(ComponentHandle(dst as u32), port);
        let port_decl = from
            .component
            .index()
            .map(|i| &nodes[i].info.outputs[from.port].format);
        let accepts = &n.info.inputs[port].format;
        check_kind(&catalog, from, to, kinds[s], port_decl, accepts).map_err(|error| {
            GraphError::Connect {
                from: names[s].clone(),
                to: format!("{}.{}", n.id, n.info.inputs[port].name),
                error: Box::new(error),
            }
        })?;
    }

    let mut out_ids = Vec::with_capacity(outputs.len());
    for &o in &outputs {
        let s = stream_of(&nodes, o);
        match &kinds[s] {
            Kind::Audio | Kind::Messages | Kind::Bytes | Kind::Item(_) => {
                out_ids.push(s);
                consumers[s] += 1;
            }
            k => {
                return Err(GraphError::BadOutput {
                    port: names[s].clone(),
                    kind: *k,
                })
            }
        }
    }

    let n_nodes = nodes.len();
    let streams = kinds.iter().map(Buffer::new).collect();
    let formats = kinds.iter().map(|k| Format::new(*k, 0.0)).collect();
    let media = vec![None; kinds.len()];
    let merge_order = status
        .into_iter()
        .chain(order.iter().copied().filter(|&i| Some(i) != status))
        .collect();
    let user_nodes = nodes.len();
    let user_streams = names.len();
    let wiring = nodes.iter().map(|n| n.inputs.clone()).collect();
    let user_order = order.clone();
    let active = vec![true; nodes.len()];
    let mut g = Graph {
        catalog,
        user_nodes,
        user_streams,
        wiring,
        user_order,
        report: ValidateReport::default(),
        active,
        nodes,
        by_id,
        order,
        streams,
        formats,
        media,
        names,
        edges: edge_names,
        feedback: feedback_names,
        clock_s,
        clock_len: 0,
        clock_pos: 0,
        reconfigure_on_retune,
        outputs: out_ids,
        aliases: BTreeMap::new(),
        fixed: BTreeMap::new(),
        fixed_targets: Vec::new(),
        values: Values::defaults(&no_settings()),
        schema: no_settings(),
        targets: Vec::new(),
        commands: Commands::default(),
        command_targets: Vec::new(),
        command_aliases,
        command_order: Vec::new(),
        status,
        taps: Vec::new(),
        results: Results::new(Arc::new(ResultsDecl::new())),
        merge: Merge::default(),
        configured: false,
        retune: None,
        consumers,
        max_chunk: 65_536,
        max_items: 4096,
        dropped_items: 0,
        since_merge: f64::INFINITY,
        key_scratch: String::new(),
        log_sig: 0,
        written: Vec::new(),
        tables_given: Vec::new(),
        merge_order,
        ctx: RunContext::default(),
        media_audio: Default::default(),
        declared: declared.unwrap_or_default(),
        reformats: Vec::with_capacity(n_nodes),
        live_taps: 0,
        stopped: None,
        problems: std::collections::VecDeque::new(),
        tuned_unfollowed: false,
        own: None,
        marks,
        first_stage: first_stage.unwrap_or_else(|| "front".to_owned()),
        costs,
        stage_of: Vec::new(),
        stages: Vec::new(),
        stage_set: sdrkit_core::StageSetId::default(),
        deploy: None,
        backlog: Output::default(),
        backlog_full: false,
        reformat_here: false,
        gaps: Vec::new(),
        metering: true,
        signal_s: 0.0,
        flow: Vec::new(),
        flow_src: Vec::new(),
        passes: 0,
        validations: 0,
    };
    g.assign_stages()?;
    g.fixed_targets = fixed_targets
        .into_iter()
        .map(|(node, id)| SettingTarget { node, id })
        .collect();
    g.fixed = fixed;
    g.aliases = aliases;
    let alias_targets = alias_targets
        .into_iter()
        .map(|(node, id)| SettingTarget { node, id })
        .collect();
    g.build_schema(alias_targets)?;
    // The results declaration exists from the start, made from the components' declarations.
    g.rebuild_merge();
    Ok(g)
}

/// How `stated`, the format that configure returned for output `port`, disagrees with the port's rule, which returned `derived`.
fn disagreement(
    port: &crate::component::Port,
    derived: (Option<f64>, Option<u16>),
    stated: &Format,
) -> Option<OutputProblem> {
    let (rate, channels) = derived;
    if let Some(declared) = rate.filter(|&r| (r - stated.rate).abs() > 1e-9 * r.abs().max(1.0)) {
        return Some(OutputProblem::Rate {
            declared,
            stated: stated.rate,
        });
    }
    if let Some(declared) = channels.filter(|&c| c != stated.channels) {
        return Some(OutputProblem::Channels {
            declared,
            stated: stated.channels,
        });
    }
    match port.format.sample_type {
        Some(declared) if stated.sample_type != Some(declared) => Some(OutputProblem::SampleType {
            declared,
            stated: stated.sample_type,
        }),
        _ => None,
    }
}

pub(crate) fn check_role(id: &str, info: &ComponentInfo) -> Result<(), GraphError> {
    let problem = match info.role {
        Role::Generator if !info.inputs.is_empty() => PortProblem::HasInputs,
        Role::Sink if !info.outputs.is_empty() => PortProblem::HasOutputs,
        Role::Sink if info.inputs.is_empty() => PortProblem::NoInputs,
        Role::Processor if info.inputs.is_empty() || info.outputs.is_empty() => {
            PortProblem::ProcessorNeedsBoth
        }
        Role::Generator if info.outputs.is_empty() => PortProblem::NoOutputs,
        _ => match info.outputs.iter().find_map(|p| {
            p.output
                .check(info.inputs.len(), &info.settings)
                .err()
                .map(|f| (p.name.clone(), f))
        }) {
            None => return Ok(()),
            Some((port, RuleFault::Input(input))) => PortProblem::RuleInput { port, input },
            Some((port, RuleFault::Setting(setting))) => PortProblem::RuleSetting { port, setting },
        },
    };
    Err(GraphError::Role {
        component: id.into(),
        role: info.role,
        problem,
    })
}

pub(crate) fn port_names(ports: &[crate::component::Port]) -> Vec<String> {
    ports.iter().map(|p| p.name.clone()).collect()
}

/// `"<prefix>.<k>"`, or `k` without a prefix, built in a reused string.
fn keyed<'a>(key: &'a mut String, prefix: &str, k: &str) -> &'a str {
    key.clear();
    if !prefix.is_empty() {
        key.push_str(prefix);
        key.push('.');
    }
    key.push_str(k);
    key
}

/// The component and output port that produce stream `s` (not the graph input).
fn port_of(nodes: &[Node], s: usize) -> (usize, usize) {
    for (i, n) in nodes.iter().enumerate() {
        if let Some(o) = n.outputs.iter().position(|&x| x == s) {
            return (i, o);
        }
    }
    unreachable!("every stream but the input has a producer")
}

/// The rate of the graph's own input (stream 0).
fn formats_rate(formats: &[Format]) -> f64 {
    formats.first().map_or(0.0, |f| f.rate)
}

fn split(end: &str) -> (&str, Option<&str>) {
    match end.split_once('.') {
        Some((c, p)) => (c, Some(p)),
        None => (end, None),
    }
}

fn resolve_output(
    nodes: &[Node],
    by_id: &BTreeMap<String, usize>,
    end: &str,
) -> Result<usize, GraphError> {
    let (c, p) = split(end);
    if c == INPUT {
        return match p {
            None | Some("out") => Ok(0),
            Some(p) => Err(GraphError::UnknownPort {
                component: INPUT.into(),
                side: Side::Output,
                port: p.into(),
                available: vec!["out".into()],
            }),
        };
    }
    let i = *by_id
        .get(c)
        .ok_or_else(|| GraphError::UnknownComponent(c.into()))?;
    let n = &nodes[i];
    let ports = &n.info.outputs;
    let o = match p {
        Some(p) => {
            ports
                .iter()
                .position(|q| q.name == p)
                .ok_or_else(|| GraphError::UnknownPort {
                    component: c.into(),
                    side: Side::Output,
                    port: p.into(),
                    available: port_names(ports),
                })?
        }
        None if ports.len() == 1 => 0,
        None if ports.is_empty() => {
            return Err(GraphError::UnknownPort {
                component: c.into(),
                side: Side::Output,
                port: String::new(),
                available: Vec::new(),
            })
        }
        None => {
            return Err(GraphError::AmbiguousPort {
                component: c.into(),
                side: Side::Output,
                available: port_names(ports),
            })
        }
    };
    Ok(n.outputs[o])
}

/// Moves what the streams `outputs` hold into `out`. Coded audio that no component reads (`consumed` is false for its
/// stream) is also decoded into `out`'s audio when `codecs` has a plug-in for it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn collect_outputs(
    outputs: &[usize],
    formats: &[Format],
    streams: &[Buffer],
    media: &[Option<Arc<MediaFormat>>],
    consumed: impl Fn(usize) -> bool,
    media_audio: &mut crate::codec::MediaAudio,
    codecs: &Arc<CodecSet>,
    out: &mut Output,
) {
    for &s in outputs {
        let f = &formats[s];
        let decode = !consumed(s);
        match &streams[s].data {
            Data::Audio(a) => out.push_audio(&a.samples, f.rate, f.channels),
            Data::Messages(v) => out.events.extend(v.iter().cloned()),
            Data::Bytes(p) => {
                if let Some(m) = &media[s] {
                    if let Some(kind) = m.codec.stream_kind() {
                        // The bytes and the format are shared with the host, not copied.
                        for pk in p {
                            if decode {
                                media_audio
                                    .decode(codecs, m, pk.bytes(), out);
                            }
                            out.push_media(kind, pk.clone(), m);
                        }
                    }
                }
            }
            Data::Items(Some(items)) if items.item_type().is::<MediaPacket>() => {
                if let Some(v) = items.as_any().downcast_ref::<Vec<Timed<MediaPacket>>>() {
                    // Each packet goes with its own format: the bytes and the format are shared, not copied.
                    for m in v {
                        if let Some(kind) = m.value.stream_kind() {
                            if decode {
                                let v = &m.value;
                                media_audio.decode(
                                    codecs,
                                    &v.format,
                                    v.packet.bytes(),
                                    out,
                                );
                            }
                            out.push_media(kind, m.value.packet.clone(), &m.value.format);
                        }
                    }
                }
            }
            Data::Items(Some(items)) if items.item_type().is::<Picture>() => {
                if let Some(v) = items.as_any().downcast_ref::<Vec<Timed<Picture>>>() {
                    // The pixels are shared with the host, not copied.
                    // The number of a picture is the number that its component gave it, with the stream above it:
                    // two outputs of pictures never share a number.
                    let port = s as u64;
                    for p in v {
                        let f = picture_format(&p.value);
                        let packet = sdrkit_core::Packet::new(p.value.data.clone(), p.time);
                        let whole = p.value.id == Picture::WHOLE;
                        let tag = sdrkit_core::PictureTag {
                            id: (port << 32) | u64::from(p.value.id),
                            state: if whole {
                                sdrkit_core::PictureState::Finished
                            } else {
                                p.value.state
                            },
                            name: p.value.name.as_deref().map(Arc::from),
                        };
                        out.push_picture(packet, &Arc::new(f), tag);
                    }
                }
            }
            Data::Items(Some(items)) if items.item_type().is::<HostRequest>() => {
                if let Some(v) = items.as_any().downcast_ref::<Vec<Timed<HostRequest>>>() {
                    out.requests.extend(v.iter().map(|t| t.value.clone()));
                }
            }
            Data::Items(Some(items)) => {
                for i in 0..items.len() {
                    if let Some(e) = items.event(i, f.tuned_hz) {
                        out.events.push(e);
                    }
                }
            }
            _ => {}
        }
    }
}

struct Node {
    id: String,
    info: ComponentInfo,
    comp: Box<dyn Component>,
    /// The present values of the component's settings, read again after each change.
    values: Values,
    /// Stream ids of the input ports.
    inputs: Vec<usize>,
    /// Which inputs are fed back (they read the previous pass).
    feedback: Vec<bool>,
    /// Bit `i` set: input `i` is optional and left unconnected (it reads an empty stream no component writes).
    unconnected: u64,
    /// Stream ids of the output ports.
    outputs: Vec<usize>,
    /// The resolved kinds of the outputs.
    kinds: Vec<Kind>,
    /// One per (input, output) pair, for [`TagPolicy::Auto`].
    carries: Vec<TagCarry>,
    scratch: Vec<Buffer>,
    /// The work counters of the component.
    meter: Arc<crate::meter::Counters>,
    /// The counters of each input port.
    in_count: Vec<Arc<crate::meter::InputCount>>,
}

/// Components wired into a graph, run on one thread in topological order with no locks.
///
/// A graph implements [`Receiver`]: its `Iq` input is fed by [`Receiver::feed`], its audio, message, byte and typed outputs land in
/// [`Output`], and the results of its components are merged into one [`Results`].
///
/// Results are merged after every call. The status component (see [`GraphBuilder::status_from`]) returns the status, the detail and
/// its fields, tables, series and points under their own names. The fields, tables and series that the stack declared
/// ([`GraphBuilder::declare`]) keep their own names from any component. Every other result of a component appears as
/// `"<component>.<name>"`, and so does a result whose key the stack declared in this form. Log lines of other components are prefixed `"<component>: "`.
pub struct Graph {
    /// The conversions that validate inserts.
    catalog: Arc<Catalog>,
    /// The components of the plan come first in `nodes`, the inserted adapters after them.
    user_nodes: usize,
    /// The streams of the plan come first in `streams`, those of the inserted adapters after them.
    user_streams: usize,
    /// The input streams of each component of the plan, as the plan wired them.
    wiring: Vec<Vec<usize>>,
    /// The run order of the components of the plan.
    user_order: Vec<usize>,
    /// What the last validate resolved and inserted.
    report: ValidateReport,
    /// Per node: it has a live consumer, so it runs.
    active: Vec<bool>,
    nodes: Vec<Node>,
    by_id: BTreeMap<String, usize>,
    order: Vec<usize>,
    streams: Vec<Buffer>,
    formats: Vec<Format>,
    /// Each stream's media format, shared with the packets passed to the host.
    media: Vec<Option<Arc<MediaFormat>>>,
    names: Vec<String>,
    edges: Vec<(String, String)>,
    feedback: Vec<(String, String)>,
    clock_s: Option<f64>,
    /// Input samples per clock period (0: no clock).
    clock_len: u64,
    /// Input samples into the current period.
    clock_pos: u64,
    reconfigure_on_retune: bool,
    outputs: Vec<usize>,
    aliases: BTreeMap<String, String>,
    fixed: BTreeMap<String, Value>,
    /// The settings that `fixed` names, resolved.
    fixed_targets: Vec<SettingTarget>,
    /// The graph's own settings: the aliases, or every component setting.
    schema: Arc<Schema>,
    /// The component setting of each setting of `schema`, by id.
    targets: Vec<SettingTarget>,
    /// The present values of `schema`.
    values: Values,
    /// Every component command, as `"<component>.<key>"`.
    commands: Commands,
    /// The component and its command id of each command of `commands`, by id.
    /// The component commands of each command of `commands`, by id.
    command_targets: Vec<Vec<(usize, CommandId)>>,
    /// Command alias name to its target (see [`GraphBuilder::command_alias`]).
    command_aliases: BTreeMap<String, CommandTarget>,
    /// The command aliases that come first in the command list, in this order. The others follow in the order of their names.
    command_order: Vec<String>,
    status: Option<usize>,
    taps: Vec<(usize, TapWriter)>,
    results: Results,
    /// How the components' results map into `results`.
    merge: Merge,
    /// Per field of `results`: written in this merge.
    written: Vec<bool>,
    /// Per table of `results`: given by a component in this merge.
    tables_given: Vec<bool>,
    /// The node indices in merge order: the status component first, then the others in run order.
    merge_order: Vec<usize>,
    configured: bool,
    retune: Option<(f64, f64)>,
    /// Edges (and graph outputs) reading each stream.
    consumers: Vec<usize>,
    max_chunk: usize,
    max_items: usize,
    dropped_items: u64,
    since_merge: f64,
    key_scratch: String,
    log_sig: usize,
    ctx: RunContext,
    /// The decoders of the coded audio that leaves the graph.
    media_audio: crate::codec::MediaAudio,
    /// What the stack declared: merged without the component prefix.
    declared: Arc<ResultsDecl>,
    /// Components that requested a new configure in this pass ([`Io::request_reformat`]).
    reformats: Vec<usize>,
    /// The live taps when demand was last worked out.
    live_taps: usize,
    /// Why the graph stopped while it ran (a component refused the configure it requested).
    stopped: Option<GraphError>,
    /// The problems that the host supplied the graph ([`Owner::problem`]), oldest first. The log keeps them after the lines of
    /// the components.
    problems: std::collections::VecDeque<String>,
    /// An input that needs the tuned frequency reads the graph input with no shift: a retune off the centre validates again.
    tuned_unfollowed: bool,
    /// The graph's own results ([`GraphBuilder::publish`]).
    own: Option<Published>,
    /// The stage boundaries that the stack marked.
    marks: Vec<StageMark>,
    first_stage: String,
    /// The recorded cost of each stage, by name.
    costs: Vec<(String, f64)>,
    /// The stage of each component, and the stages in run order.
    stage_of: Vec<usize>,
    stages: Vec<sdrkit_core::StageDecl>,
    /// The identity of `stages`: a new one each time they change.
    stage_set: sdrkit_core::StageSetId,
    /// The present placement of cuts, if the host made one.
    deploy: Option<Deployment>,
    /// Output of passes that were finished outside [`Graph::process`], for the next call.
    backlog: Output,
    backlog_full: bool,
    /// A component of this thread requested a new configure while other components were away.
    reformat_here: bool,
    /// Missing samples that the input has not shown yet: the index of the first sample after the gap, and the count.
    gaps: Vec<(u64, u64)>,
    /// The components are timed and their items counted.
    metering: bool,
    /// Seconds of input that the graph has taken since it was configured.
    signal_s: f64,
    /// The counters of each stream, and the writer of each stream: the component and its output port.
    flow: Vec<Arc<crate::meter::StreamCount>>,
    flow_src: Vec<(usize, usize)>,
    /// The passes since the graph was made.
    passes: u64,
    /// The number of validations.
    validations: u64,
}

/// A setting of one component of a graph: the component and the setting's id in the component's schema. Resolved once from an
/// alias or a `"<component>.<key>"` key ([`Graph::target`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SettingTarget {
    node: usize,
    id: SettingId,
}

impl SettingTarget {
    /// The id of the setting in its component's schema.
    pub fn id(&self) -> SettingId {
        self.id
    }
}

/// Where the results of one component go in the graph's results.
#[derive(Debug, Default)]
struct NodeMap {
    /// The declaration the map was made from.
    from: Option<Arc<ResultsDecl>>,
    fields: Vec<FieldId>,
    tables: Vec<TableId>,
    series: Vec<SeriesId>,
    rasters: Vec<RasterId>,
}

/// The kind of a declared item of the results.
#[derive(Clone, Copy)]
enum Part {
    Field,
    Table,
    Series,
    Raster,
}

/// The declarations of the graph's results while the merge is made.
struct DeclLists {
    fields: Vec<FieldDecl>,
    tables: Vec<TableDecl>,
    series: Vec<SeriesDecl>,
    rasters: Vec<RasterDecl>,
}

impl DeclLists {
    fn lens(&self) -> (usize, usize, usize, usize) {
        (
            self.fields.len(),
            self.tables.len(),
            self.series.len(),
            self.rasters.len(),
        )
    }
    /// Maps the keys of `rd` to the graph's results, adding what is new. `key_of` maps a key, from the kind of its
    /// item, the key and whether the stack declared it (among the first `declared` of each list).
    fn map(
        &mut self,
        rd: &Arc<ResultsDecl>,
        declared: (usize, usize, usize, usize),
        key_of: impl Fn(Part, &str, bool) -> String,
    ) -> NodeMap {
        let mut m = NodeMap {
            from: Some(Arc::clone(rd)),
            ..NodeMap::default()
        };
        for f in rd.fields().fields() {
            let key = key_of(
                Part::Field,
                &f.key,
                self.fields[..declared.0].iter().any(|g| g.key == f.key),
            );
            let at = self
                .fields
                .iter()
                .position(|g| g.key == key)
                .unwrap_or_else(|| {
                    let mut g = f.clone();
                    g.key = Cow::Owned(key);
                    self.fields.push(g);
                    self.fields.len() - 1
                });
            m.fields.push(FieldId(at as u16));
        }
        for t in rd.tables() {
            let key = key_of(
                Part::Table,
                &t.key,
                self.tables[..declared.1].iter().any(|g| g.key == t.key),
            );
            let at = self
                .tables
                .iter()
                .position(|g| g.key == key)
                .unwrap_or_else(|| {
                    let mut g = t.clone();
                    g.key = Cow::Owned(key);
                    self.tables.push(g);
                    self.tables.len() - 1
                });
            m.tables.push(TableId(at as u16));
        }
        for x in rd.series_decls() {
            let key = key_of(
                Part::Series,
                &x.key,
                self.series[..declared.2].iter().any(|g| g.key == x.key),
            );
            let at = self
                .series
                .iter()
                .position(|g| g.key == key)
                .unwrap_or_else(|| {
                    let mut g = x.clone();
                    g.key = key;
                    self.series.push(g);
                    self.series.len() - 1
                });
            m.series.push(SeriesId(at as u16));
        }
        for x in rd.rasters() {
            let key = key_of(
                Part::Raster,
                &x.key,
                self.rasters[..declared.3].iter().any(|g| g.key == x.key),
            );
            let at = self
                .rasters
                .iter()
                .position(|g| g.key == key)
                .unwrap_or_else(|| {
                    let mut g = x.clone();
                    g.key = key;
                    self.rasters.push(g);
                    self.rasters.len() - 1
                });
            m.rasters.push(RasterId(at as u16));
        }
        m
    }
}

/// How the components' results map into the graph's results.
#[derive(Debug, Default)]
struct Merge {
    nodes: Vec<NodeMap>,
    /// The map of the graph's own results ([`GraphBuilder::publish`]).
    own: NodeMap,
}

impl std::fmt::Debug for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graph")
            .field(
                "components",
                &self
                    .order
                    .iter()
                    .map(|&i| &self.nodes[i].id)
                    .collect::<Vec<_>>(),
            )
            .field("edges", &self.edges)
            .field("feedback", &self.feedback)
            .finish()
    }
}

impl Graph {
    /// Builds a graph from its JSON description, with the components of `registry`.
    pub fn from_json(json: &str, registry: &crate::Registry) -> Result<Graph, GraphError> {
        Description::from_json(json)?.build(registry)
    }

    /// Configures every component in topological order for an `Iq` input of this format. Call it at the start and after any change
    /// of rate.
    pub fn configure_input(&mut self, input: Format) -> Result<(), GraphError> {
        self.validate(input)?;
        self.configured = true;
        self.clock_len = self
            .clock_s
            .map_or(0, |c| ((c * self.formats[0].rate).round() as u64).max(1));
        self.clock_pos = 0;
        self.retune = None;
        self.tell_frequency();
        // The results of the fresh components, at once.
        self.refresh_results();
        Ok(())
    }

    /// Resolves the format of every port for an `Iq` input of format `input`, in run order, and inserts the adapters of the
    /// catalog where a produced format does not fit an input. Each derivation (a source port, an adapter and its settings) is
    /// made once, and every input that needs it reads the same stream. It configures every component, because configure
    /// states the formats that a port's rule leaves to it. [`Graph::configure_input`] runs it. It returns the report, which
    /// [`Graph::report`] also keeps.
    ///
    /// # Errors
    ///
    /// [`GraphError::Connect`] with the mismatch when no chain of at most three adapters joins an output to an input,
    /// [`GraphError::UnresolvedFormat`] for a sampled stream without a rate, and the configure errors of the components.
    pub fn validate(&mut self, input: Format) -> Result<&ValidateReport, GraphError> {
        self.home();
        self.validations += 1;
        self.signal_s = 0.0;
        for n in &mut self.nodes {
            n.meter = Arc::default();
        }
        self.flow.clear();
        self.strip();
        self.formats[0] = Format {
            kind: Kind::Iq,
            ..input
        };
        for (_, t) in self.taps.iter().filter(|(s, _)| *s == 0) {
            t.reformat(&self.formats[0]);
        }
        self.configured = false;
        self.stopped = None;
        self.tuned_unfollowed = false;
        self.report = ValidateReport::default();
        let user_order = self.user_order.clone();
        let mut order = Vec::with_capacity(user_order.len());
        // Each derivation made so far: its source stream, its catalog entry, its settings, its stream and its report entry.
        let mut derived: Vec<(usize, usize, Values, usize, usize)> = Vec::new();
        for &i in &user_order {
            for p in 0..self.nodes[i].inputs.len() {
                let n = &self.nodes[i];
                if n.feedback[p] || (p < 64 && n.unconnected & (1 << p) != 0) {
                    continue;
                }
                let s = n.inputs[p];
                let want = &n.info.inputs[p].format;
                let f = &self.formats[s];
                if want.accepts_format(f) {
                    // The graph input at the tuned frequency needs no shift now, but a retune off the centre will.
                    self.tuned_unfollowed |= s == 0 && want.centre == Some(CentreRule::Tuned);
                    continue;
                }
                let input_name = format!("{}.{}", n.id, n.info.inputs[p].name);
                if f.kind.is_sampled()
                    && !(f.rate.is_finite() && f.rate > 0.0)
                    && want.rate != crate::RateRule::Any
                {
                    return Err(GraphError::UnresolvedFormat {
                        port: self.names[s].clone(),
                        kind: f.kind,
                        input: input_name,
                    });
                }
                let Some(chain) = self.catalog.chain(f, want) else {
                    return Err(self.mismatch(s, i, p));
                };
                let mut cur = s;
                let mut last = None;
                for step in chain {
                    if let Some(d) = derived
                        .iter()
                        .find(|d| d.0 == cur && d.1 == step.adapter && d.2 == step.values)
                    {
                        cur = d.3;
                        last = Some(d.4);
                        continue;
                    }
                    let (adapter, values) = (step.adapter, step.values.clone());
                    let out = self.insert(cur, step, &mut order, (s, i, p))?;
                    derived.push((cur, adapter, values, out, self.report.adapters.len() - 1));
                    last = Some(self.report.adapters.len() - 1);
                    cur = out;
                }
                self.nodes[i].inputs[p] = cur;
                if let Some(k) = last {
                    self.report.adapters[k].consumers.push(input_name);
                }
            }
            order.push(i);
            self.configure_node(i)?;
        }
        self.order = order;
        let mut consumers = vec![0; self.streams.len()];
        for n in &self.nodes {
            for (p, &s) in n.inputs.iter().enumerate() {
                if p >= 64 || n.unconnected & (1 << p) == 0 {
                    consumers[s] += 1;
                }
            }
        }
        for &s in &self.outputs {
            consumers[s] += 1;
        }
        self.consumers = consumers;
        self.report.ports = self
            .names
            .iter()
            .cloned()
            .zip(self.formats.iter().cloned())
            .collect();
        self.update_demand();
        self.assign_stages()?;
        Ok(&self.report)
    }

    /// What the last [`Graph::validate`] resolved and inserted. Empty before the first.
    pub fn report(&self) -> &ValidateReport {
        &self.report
    }

    /// Removes the adapters that the last validate inserted and wires the plan's components as the plan did.
    fn strip(&mut self) {
        self.nodes.truncate(self.user_nodes);
        self.consumers.truncate(self.user_streams);
        self.streams.truncate(self.user_streams);
        self.formats.truncate(self.user_streams);
        self.media.truncate(self.user_streams);
        self.names.truncate(self.user_streams);
        for (n, w) in self.nodes.iter_mut().zip(&self.wiring) {
            n.inputs.clone_from(w);
        }
        let user_streams = self.user_streams;
        self.taps.retain(|(s, _)| *s < user_streams);
        self.order.clone_from(&self.user_order);
        self.active.truncate(self.user_nodes);
    }

    /// Inserts the adapter of `step` on stream `source`, configures it, appends it to `order` and returns its output stream.
    /// `consumer` (the stream, the component and the input that need the chain) names the error if the adapter refuses.
    fn insert(
        &mut self,
        source: usize,
        step: crate::adapt::Step,
        order: &mut Vec<usize>,
        consumer: (usize, usize, usize),
    ) -> Result<usize, GraphError> {
        let a = &self.catalog.adapters()[step.adapter];
        let (kind, class, to) = (a.kind, a.class, a.to);
        let Some((mut comp, _)) = crate::adapt::configured(a, &step.values, &self.formats[source])
        else {
            let (s, i, p) = consumer;
            return Err(self.mismatch(s, i, p));
        };
        comp.attach(&self.ctx);
        let info = comp.info();
        // The id holds no dot, so a `"<component>.<key>"` lookup never splits it.
        let short = kind.rsplit('.').next().unwrap_or(kind);
        let id = format!("~{short}{}", self.report.adapters.len() + 1);
        let node = self.nodes.len();
        // An adapter's optional inputs after the first stay open: each reads an empty stream of its own.
        let mut inputs = vec![source];
        let mut unconnected = 0u64;
        for (p, port) in info.inputs.iter().enumerate().skip(1).take(63) {
            let KindRule::Is(k) = port.format.kind else {
                continue;
            };
            unconnected |= 1 << p;
            inputs.push(self.streams.len());
            self.streams.push(Buffer::new(&k));
            self.formats.push(Format::new(k, 0.0));
            self.media.push(None);
            self.names.push(format!("{id}.{} (unconnected)", port.name));
            self.consumers.push(0);
        }
        let n_inputs = inputs.len();
        let out = self.streams.len();
        let name = format!(
            "{id}.{}",
            info.outputs.first().map_or("out", |p| p.name.as_str())
        );
        self.nodes.push(Node {
            inputs,
            feedback: vec![false; n_inputs],
            unconnected,
            outputs: vec![out],
            kinds: vec![to],
            carries: Vec::new(),
            scratch: Vec::with_capacity(1),
            meter: Arc::default(),
            in_count: Vec::new(),
            id: id.clone(),
            values: comp.values(),
            info,
            comp,
        });
        self.streams.push(Buffer::new(&to));
        self.formats.push(Format::new(to, 0.0));
        self.media.push(None);
        self.names.push(name.clone());
        self.consumers.push(0);
        self.active.push(true);
        self.configure_node(node)?;
        let latency = self.nodes[node].comp.latency();
        order.push(node);
        self.report.adapters.push(InsertedAdapter {
            id,
            kind,
            class,
            source: self.names[source].clone(),
            output: name,
            consumers: Vec::new(),
            group_delay_s: latency.group_delay_s,
            first_valid: latency.first_valid,
        });
        Ok(out)
    }

    /// The error for input `p` of node `i`, which no chain of adapters joins to stream `s`.
    fn mismatch(&self, s: usize, i: usize, p: usize) -> GraphError {
        let n = &self.nodes[i];
        let from = if s == 0 {
            PortRef::INPUT
        } else {
            let (c, o) = port_of(&self.nodes, s);
            PortRef::new(ComponentHandle(c as u32), o)
        };
        let to = PortRef::new(ComponentHandle(i as u32), p);
        let f = &self.formats[s];
        let want = &n.info.inputs[p].format;
        let error = check_kind(&self.catalog, from, to, f.kind, None, want)
            .and_then(|()| {
                if want.kind.accepts(&f.kind) {
                    check_format(from, to, f, want)
                } else {
                    Ok(())
                }
            })
            .err();
        let error = match (error, &f.media) {
            (Some(e), _) => e,
            (None, Some(m)) if !want.media.is_empty() => {
                return GraphError::InputMedia {
                    component: n.id.clone(),
                    input: n.info.inputs[p].name.clone(),
                    got: MediaSpec::new(m.codec, m.packaging),
                    accepts: want.media.clone(),
                }
            }
            (None, _) => ConnectError::NoConversion {
                from,
                to,
                produces: f.kind,
                accepts: Box::new(want.clone()),
            },
        };
        GraphError::Connect {
            from: self.names[s].clone(),
            to: format!("{}.{}", n.id, n.info.inputs[p].name),
            error: Box::new(error),
        }
    }

    /// Marks the components that run: those with a live consumer downstream. A graph output, a tap, a sink and a component
    /// that publishes results are live consumers.
    fn update_demand(&mut self) {
        self.home();
        let mut live = vec![false; self.streams.len()];
        for &s in &self.outputs {
            live[s] = true;
        }
        for (s, t) in &self.taps {
            if !t.closed() {
                live[*s] = true;
            }
        }
        self.active = vec![false; self.nodes.len()];
        // A fed-back input reads a component later in the run order: repeat until nothing changes.
        let mut changed = true;
        while changed {
            changed = false;
            for &i in self.order.iter().rev() {
                let n = &self.nodes[i];
                let on = n.info.role == Role::Sink
                    || n.comp.results().is_some()
                    || n.outputs.iter().any(|&s| live[s]);
                if on && !self.active[i] {
                    self.active[i] = true;
                    changed = true;
                    for &s in &n.inputs {
                        live[s] = true;
                    }
                }
            }
        }
        self.live_taps = self.taps();
    }

    /// Passes every component the input's centre and tuned frequency ([`Component::retuned`]).
    fn tell_frequency(&mut self) {
        self.home();
        let (c, t) = (self.formats[0].centre_hz, self.formats[0].tuned_hz);
        for n in &mut self.nodes {
            n.comp.retuned(c, t);
        }
    }

    /// The clock period in input samples (0 without a clock; see [`GraphBuilder::clock`]).
    pub fn clock_samples(&self) -> u64 {
        self.clock_len
    }

    fn configure_node(&mut self, i: usize) -> Result<bool, GraphError> {
        let ins: Vec<Format> = self.nodes[i]
            .inputs
            .iter()
            .map(|&s| self.formats[s].clone())
            .collect();
        let to_node = &self.nodes[i];
        for (p, f) in ins.iter().enumerate() {
            if p < 64 && to_node.unconnected & (1 << p) != 0 {
                continue;
            }
            let src = to_node.inputs[p];
            let from = if src == 0 {
                PortRef::INPUT
            } else {
                let (c, o) = port_of(&self.nodes, src);
                PortRef::new(ComponentHandle(c as u32), o)
            };
            let to = PortRef::new(ComponentHandle(i as u32), p);
            let port = &to_node.info.inputs[p];
            check_format(from, to, f, &port.format).map_err(|error| GraphError::Connect {
                from: self.names[src].clone(),
                to: format!("{}.{}", to_node.id, port.name),
                error: Box::new(error),
            })?;
        }
        let n = &mut self.nodes[i];
        for (p, f) in ins.iter().enumerate() {
            let accepts = &n.info.inputs[p].format.media;
            if let (false, Some(m)) = (accepts.is_empty(), &f.media) {
                if !accepts.iter().any(|a| m.matches(a)) {
                    return Err(GraphError::InputMedia {
                        component: n.id.clone(),
                        input: n.info.inputs[p].name.clone(),
                        got: MediaSpec::new(m.codec, m.packaging),
                        accepts: accepts.clone(),
                    });
                }
            }
        }
        let rules: Vec<(Option<f64>, Option<u16>)> = n
            .info
            .outputs
            .iter()
            .map(|p| p.output.derive(&ins, &n.values))
            .collect();
        let outs = n.comp.configure(&ins).map_err(|error| GraphError::Config {
            component: n.id.clone(),
            error,
        })?;
        if outs.len() != n.outputs.len() {
            return Err(GraphError::OutputCount {
                component: n.id.clone(),
                stated: outs.len(),
                outputs: n.outputs.len(),
            });
        }
        let mut changed = false;
        // The delay of the path: the longest of the inputs, and the component's own.
        let delay =
            ins.iter().map(|f| f.delay_s).fold(0.0, f64::max) + n.comp.latency().group_delay_s;
        for (o, mut f) in outs.into_iter().enumerate() {
            f.delay_s = delay;
            let s = n.outputs[o];
            if f.kind != n.kinds[o] {
                return Err(GraphError::OutputKind {
                    component: n.id.clone(),
                    output: n.info.outputs[o].name.clone(),
                    stated: f.kind,
                    declared: n.kinds[o],
                });
            }
            if let Some(problem) = disagreement(&n.info.outputs[o], rules[o], &f) {
                return Err(GraphError::OutputFormat {
                    component: n.id.clone(),
                    output: n.info.outputs[o].name.clone(),
                    problem,
                });
            }
            // A byte output that states a media format must state one the host decodes. A byte output without a media format
            // (a transport stream) is legal: it stays in the graph, for a tap or `Graph::buffer`.
            if f.kind == Kind::Bytes && self.outputs.contains(&s) {
                if let Some(m) = &f.media {
                    if m.codec.stream_kind().is_none() {
                        return Err(GraphError::OutputNotDecoded {
                            component: n.id.clone(),
                            output: n.info.outputs[o].name.clone(),
                            codec: m.codec,
                        });
                    }
                }
            }
            if f != self.formats[s] {
                changed = true;
                self.media[s] = f.media.clone().map(Arc::new);
                for (ts, t) in &self.taps {
                    if *ts == s {
                        t.reformat(&f);
                    }
                }
            }
            self.formats[s] = f;
        }
        n.carries.clear();
        if n.info.tags == TagPolicy::Auto {
            for &si in &n.inputs {
                for &so in &n.outputs {
                    n.carries
                        .push(TagCarry::new(self.formats[si].rate, self.formats[so].rate));
                }
            }
        }
        Ok(changed)
    }

    /// Configures `start` again and every later component whose input formats changed.
    fn reconfigure_from(&mut self, start: usize) -> Result<(), GraphError> {
        let pos = self
            .order
            .iter()
            .position(|&i| i == start)
            .expect("every component is in the run order");
        let mut dirty = vec![false; self.streams.len()];
        for k in pos..self.order.len() {
            let i = self.order[k];
            if i == start || self.nodes[i].inputs.iter().any(|&s| dirty[s]) {
                let before: Vec<Format> = self.nodes[i]
                    .outputs
                    .iter()
                    .map(|&s| self.formats[s].clone())
                    .collect();
                self.configure_node(i)?;
                for (o, &s) in self.nodes[i].outputs.iter().enumerate() {
                    dirty[s] = self.formats[s] != before[o];
                }
            }
        }
        Ok(())
    }

    /// Runs `iq` through the graph: every component in order, then the graph outputs into `out` (appended), the taps, and the
    /// merged results. Input longer than [`Graph::max_chunk`] is run in pieces of at most that, so no buffer grows beyond what
    /// that many input samples make (components give the same output however the input is cut).
    pub fn process(&mut self, iq: &[Complex32], out: &mut Output) {
        if !self.configured {
            return;
        }
        if self.flow.len() != self.streams.len() {
            self.home();
            self.reset_flow();
        }
        if self.deploy.is_some() {
            if self.deploy.as_ref().is_some_and(|d| d.away() && d.attention()) {
                self.home();
            }
            self.scatter();
        }
        self.take_backlog(out);
        if iq.is_empty() {
            // An empty call still runs the components (a helper thread's results come back) and merges what they hold.
            self.run_once(iq, false, false, out);
            self.refresh_results();
            return;
        }
        let mut rest = iq;
        while !rest.is_empty() {
            let mut n = rest.len().min(self.max_chunk);
            if self.clock_len > 0 {
                n = n.min((self.clock_len - self.clock_pos) as usize);
            }
            let (piece, later) = rest.split_at(n);
            rest = later;
            let mut tick = false;
            if self.clock_len > 0 {
                self.clock_pos += n as u64;
                if self.clock_pos == self.clock_len {
                    self.clock_pos = 0;
                    tick = true;
                }
            }
            self.run_once(piece, tick, false, out);
            if tick {
                self.refresh_results();
            }
        }
        self.since_merge += iq.len() as f64 / self.formats[0].rate.max(1.0);
        if self.since_merge >= Self::MERGE_INTERVAL_S {
            self.refresh_results();
        }
    }

    /// Merged results are refreshed at most this often, in signal time (about the engine's snapshot rate).
    pub const MERGE_INTERVAL_S: f64 = 1.0 / 60.0;

    /// Ends the stream: one last pass in run order where every component gets [`Component::finish`] instead of
    /// [`Component::process`]. The pass is also a clock tick ([`Io::tick`]). Each component reads what the components before it returned in this pass, decodes what it still
    /// holds, and can wait for its helper threads. The outputs go to `out` (appended) and the taps, and the results are merged
    /// at once. A later [`Graph::process`] continues as a new piece of signal.
    ///
    /// ```
    /// use sdrkit_graph::{components::NullSink, Format, GraphBuilder};
    /// use sdrkit_core::Output;
    /// let mut g = GraphBuilder::new().add("sink", NullSink::default()).input("sink").build().unwrap();
    /// g.configure_input(Format::iq(48e3, 0.0, 0.0)).unwrap();
    /// let mut out = Output::default();
    /// g.process(&[Default::default(); 480], &mut out);
    /// g.finish(&mut out);
    /// ```
    pub fn finish(&mut self, out: &mut Output) {
        self.home();
        self.take_backlog(out);
        if !self.configured {
            return;
        }
        // The end of the stream also ends a clock period that has begun, so components that publish at a tick publish now. A
        // period that has just ended had its tick already.
        let tick = self.clock_len == 0 || self.clock_pos > 0;
        self.run_once(&[], tick, true, out);
        self.clock_pos = 0;
        self.refresh_results();
    }

    /// The most input samples run in one pass.
    pub fn max_chunk(&self) -> usize {
        self.max_chunk
    }
    /// Sets the most input samples run in one pass (at least 1).
    pub fn set_max_chunk(&mut self, n: usize) {
        self.max_chunk = n.max(1);
    }
    /// The most items of a stream without a rate (bytes, messages, typed items) one component may produce in one pass. Beyond it the
    /// newest are dropped, counted ([`Graph::dropped_items`]) and marked by a `Discontinuity` tag.
    pub fn set_max_items(&mut self, n: usize) {
        self.max_items = n.max(1);
    }
    /// Items dropped by the cap of [`Graph::set_max_items`].
    pub fn dropped_items(&self) -> u64 {
        self.dropped_items
    }

    /// Bytes of memory the graph's own buffers hold now (stream buffers and taps), not counting the components' internal state.
    pub fn memory_bytes(&self) -> usize {
        self.streams.iter().map(Buffer::bytes).sum::<usize>()
            + self
                .nodes
                .iter()
                .map(|n| n.scratch.capacity() * std::mem::size_of::<Buffer>())
                .sum::<usize>()
            + self.taps.iter().map(|(_, t)| t.bytes()).sum::<usize>()
    }

    /// Notes that `lost` samples are missing after the next `ahead` input samples. The first sample after the gap carries
    /// a `Discontinuity` tag.
    pub fn discontinuity_after(&mut self, ahead: usize, lost: u64) {
        if self.gaps.len() < 256 {
            self.gaps.push((self.streams[0].end() + ahead as u64, lost));
        }
    }

    /// Moves the output of passes that were finished outside [`Graph::process`] into `out`.
    fn take_backlog(&mut self, out: &mut Output) {
        if self.backlog_full {
            self.backlog_full = false;
            out.append(&mut self.backlog);
        }
    }

    /// One pass of every component in run order. With `finish`, each component gets [`Component::finish`].
    fn run_once(&mut self, iq: &[Complex32], tick: bool, finish: bool, out: &mut Output) {
        // The samples stay in the caller's slice: the input buffer holds only their index and tags.
        let input = &mut self.streams[0];
        input.advance();
        input.moved = iq.len();
        if let Some((centre_hz, tuned_hz)) = self.retune.take() {
            input.tag_here(
                0,
                TagKind::Retuned {
                    centre_hz,
                    tuned_hz,
                },
            );
        }
        if !self.gaps.is_empty() {
            let (first, end) = (input.first(), input.first() + iq.len() as u64);
            self.gaps.retain(|&(at, lost)| {
                if at >= end && !finish {
                    return true;
                }
                input.push_tag(at.clamp(first, end), TagKind::Discontinuity { lost });
                false
            });
        }
        let pass_s = if formats_rate(&self.formats) > 0.0 {
            iq.len() as f64 / formats_rate(&self.formats)
        } else {
            0.0
        };
        self.signal_s += pass_s;
        self.passes += 1;
        let away = self.deploy.as_ref().is_some_and(Deployment::away);
        {
            let Graph {
                nodes,
                order,
                streams,
                formats,
                consumers,
                taps,
                outputs,
                max_items,
                dropped_items,
                reformats,
                active,
                metering,
                deploy,
                flow,
                passes,
                ..
            } = self;
            let mut p = Pass {
                streams,
                formats,
                consumers,
                outputs,
                taps,
                max_items: *max_items,
                dropped_items,
                reformats,
                tick,
                finish,
                iq,
                pass_s,
                metering: *metering,
                flow,
                seq: *passes,
            };
            for &i in order.iter() {
                if active[i] && (!away || deploy.as_ref().is_some_and(|d| d.here(i))) {
                    run_node(i, &mut nodes[i], &mut p);
                }
            }
        }
        if away {
            self.after_home_pass(tick, finish, pass_s, out);
            if self.reformat_here {
                // A component of this thread requested a new configure: it is done before its next pass.
                self.reformat_here = false;
                self.home();
                self.take_backlog(out);
            }
            return;
        }
        self.collect(out);
        self.taps.retain(|(_, t)| !t.closed());
        if self.taps.len() != self.live_taps {
            self.update_demand();
        }
        for (s, t) in &mut self.taps {
            if *s == 0 {
                t.push_input(&self.streams[0], iq);
            } else {
                t.push(&self.streams[*s]);
            }
        }
        if !self.reformats.is_empty() {
            self.apply_reformats();
        }
    }

    /// Configures again the components that requested it in this pass, in run order, and every later component whose input
    /// format changes. A refusal stops the graph and puts the reason in its status.
    fn apply_reformats(&mut self) {
        self.home();
        let mut asked = std::mem::take(&mut self.reformats);
        for &i in &asked {
            if self.reconfigure_from(i).is_err() {
                // The new formats need other adapters: validate the whole graph again.
                let input = self.formats[0].clone();
                if let Err(e) = self.configure_input(input) {
                    self.configured = false;
                    self.stopped = Some(e);
                }
                break;
            }
        }
        asked.clear();
        self.reformats = asked;
        self.refresh_results();
    }

    /// Moves what the graph's outputs hold into `out`. Coded audio that no component of the graph reads is also decoded into
    /// `out`'s audio when the run context has a plug-in for it.
    fn collect(&mut self, out: &mut Output) {
        let nodes = &self.nodes;
        collect_outputs(
            &self.outputs,
            &self.formats,
            &self.streams,
            &self.media,
            |s| nodes.iter().any(|n| n.inputs.contains(&s)),
            &mut self.media_audio,
            &self.ctx.codecs,
            out,
        );
    }

    /// Whether a component's results declaration is not the one the merge was made from.
    fn merge_is_stale(&self) -> bool {
        let own = self.own.as_ref().map(|p| p.results().decl());
        let own_stale = match (own, &self.merge.own.from) {
            (None, None) => false,
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            _ => true,
        };
        own_stale
            || self.merge.nodes.len() != self.nodes.len()
            || self.nodes.iter().zip(&self.merge.nodes).any(|(n, m)| {
                let now = n.comp.results().map(Results::decl);
                match (now, &m.from) {
                    (None, None) => false,
                    (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                    _ => true,
                }
            })
    }

    /// Makes the graph's results declaration from the stack's declaration and the components' declarations, and the map from
    /// each component's ids to the graph's ids.
    fn rebuild_merge(&mut self) {
        let d = &self.declared;
        let mut lists = DeclLists {
            fields: d.fields().fields().to_vec(),
            tables: d.tables().to_vec(),
            series: d.series_decls().to_vec(),
            rasters: d.rasters().to_vec(),
        };
        let declared = lists.lens();
        let mut own = NodeMap::default();
        if let Some(p) = &self.own {
            own = lists.map(p.results().decl(), declared, |_, k, _| k.to_owned());
        }
        let mut maps: Vec<NodeMap> = (0..self.nodes.len()).map(|_| NodeMap::default()).collect();
        for &i in &self.merge_order {
            let n = &self.nodes[i];
            let Some(res) = n.comp.results() else {
                continue;
            };
            let main = self.status == Some(i);
            maps[i] = lists.map(res.decl(), declared, |part, k, declared| {
                // A key that the stack declares for this component by name goes there, before a key of the stack
                // with the same name. A field and a table of a component can have one key, so only an item of
                // the same kind counts.
                let named = format!("{}.{k}", n.id);
                let by_name = match part {
                    Part::Field => d.fields().id(&named).is_some(),
                    Part::Table => d.table_id(&named).is_some(),
                    Part::Series => d.series_id(&named).is_some(),
                    Part::Raster => d.rasters().iter().any(|r| r.key == named),
                };
                if !by_name && (declared || main) {
                    k.to_owned()
                } else {
                    named
                }
            });
        }
        let decl = ResultsDecl::from_parts(
            lists.fields,
            lists.tables,
            lists.series,
            lists.rasters,
            d.points_label(),
            d.events().to_vec(),
        )
        .expect("each key is added once");
        let mut results = Results::new(Arc::new(decl));
        results.set_log_cap(self.results.log_cap());
        self.results = results;
        self.merge = Merge { nodes: maps, own };
        self.log_sig = 0;
    }

    /// The fields, tables, series and rasters of `decl` that neither a component nor the graph itself declares: each as
    /// its key, with `table`, `series` or `raster` and a space before the key of one.
    pub fn undeclared(&self, decl: &ResultsDecl) -> Vec<String> {
        let mut held: Vec<&ResultsDecl> = Vec::new();
        if let Some(p) = &self.own {
            held.push(p.results().decl());
        }
        held.extend(
            self.nodes
                .iter()
                .filter_map(|n| n.comp.results())
                .map(|r| &**r.decl()),
        );
        // The declaration of the component that `key` names with `<component>.<key>`, and the key in it.
        let of_component = |key: &str| -> Option<(&ResultsDecl, String)> {
            self.nodes.iter().find_map(|n| {
                let own = key.strip_prefix(n.id.as_str())?.strip_prefix('.')?;
                Some((&**n.comp.results()?.decl(), own.to_owned()))
            })
        };
        let mut out = Vec::new();
        for f in decl.fields().fields() {
            if !held.iter().any(|d| d.fields().id(&f.key).is_some())
                && !of_component(&f.key).is_some_and(|(d, k)| d.fields().id(&k).is_some())
            {
                out.push(f.key.to_string());
            }
        }
        for t in decl.tables() {
            if !held.iter().any(|d| d.table_id(&t.key).is_some())
                && !of_component(&t.key).is_some_and(|(d, k)| d.table_id(&k).is_some())
            {
                out.push(format!("table {}", t.key));
            }
        }
        for s in decl.series_decls() {
            if !held.iter().any(|d| d.series_id(&s.key).is_some())
                && !of_component(&s.key).is_some_and(|(d, k)| d.series_id(&k).is_some())
            {
                out.push(format!("series {}", s.key));
            }
        }
        for r in decl.rasters() {
            if !held
                .iter()
                .any(|d| d.rasters().iter().any(|x| x.key == r.key))
            {
                out.push(format!("raster {}", r.key));
            }
        }
        out
    }

    /// The graph's own results ([`GraphBuilder::publish`]), to write. A change shows at the next merge.
    pub fn own_results_mut(&mut self) -> Option<&mut Published> {
        self.own.as_mut()
    }

    /// Merges the components' results into the graph's now (it happens on its own every [`Graph::MERGE_INTERVAL_S`] of signal).
    ///
    /// The rule for keys: the status component's fields, tables, series, rasters and points keep their own keys. A field, table,
    /// series or raster that the stack declared ([`Graph::declare`]) keeps its own key from whichever component publishes it.
    /// Every other key of a component is merged as `"<component>.<key>"`. When two components publish one declared key, the
    /// status component supplies it if it holds a value, else the first component in run order that holds one.
    pub fn refresh_results(&mut self) {
        self.since_merge = 0.0;
        self.take_snapshots();
        if self.merge_is_stale() {
            self.rebuild_merge();
        }
        let mut key = std::mem::take(&mut self.key_scratch);
        let mut written = std::mem::take(&mut self.written);
        let mut tables_given = std::mem::take(&mut self.tables_given);
        let r = &mut self.results;
        written.clear();
        written.resize(r.fields().decl().len(), false);
        tables_given.clear();
        tables_given.resize(r.decl().tables().len(), false);
        let mut np = 0;
        let mut log_sig = 0usize;
        // The graph's own results come first: their tables are given on every merge, so their rows stay.
        let own = self.own.as_ref().map(|p| (p.results(), &self.merge.own));
        let nodes = self.merge_order.iter().filter_map(|&i| {
            let n = &self.nodes[i];
            Some((n.comp.results()?, &self.merge.nodes[i], i))
        });
        let sources = own
            .map(|(res, m)| (res, m, usize::MAX))
            .into_iter()
            .chain(nodes);
        for (res, m, i) in sources {
            let main = self.status == Some(i);
            let id = self.nodes.get(i).map_or("", |n| n.id.as_str());
            for (k, gid) in m.fields.iter().enumerate() {
                let v = res.fields().value(FieldId(k as u16));
                if let Some(v) = v.filter(|v| !v.is_none()) {
                    let w = &mut written[gid.index()];
                    if !*w {
                        *w = true;
                        if r.fields_mut().set_value(*gid, v).is_err() {
                            // A stack that declares a field of another kind than the component publishes.
                            let key = &r.fields().decl().fields()[gid.index()].key;
                            sdrkit_core::record::refuse_write(
                                key,
                                "a value of another kind than declared",
                            );
                        }
                    }
                }
            }
            for (k, gid) in m.tables.iter().enumerate() {
                let given = &mut tables_given[usize::from(gid.0)];
                let (Some(src), false) = (res.table(TableId(k as u16)), *given) else {
                    continue;
                };
                if let (false, Some(mut dst)) = (src.is_empty(), r.table_mut(*gid)) {
                    dst.copy_from(src);
                    *given = true;
                }
            }
            for (k, gid) in m.series.iter().enumerate() {
                if let Some(src) = res.series(SeriesId(k as u16)) {
                    r.update_series(*gid, |d| {
                        // Room for what the source holds room for, so the copy does not grow while the source fills.
                        let room = src.points.capacity().saturating_sub(d.points.len());
                        d.points.reserve(room);
                        copy_if_changed(d, src)
                    });
                }
            }
            for (k, gid) in m.rasters.iter().enumerate() {
                if let Some(src) = res.raster(RasterId(k as u16)) {
                    r.update_raster(*gid, |d| copy_if_changed(d, src));
                }
            }
            for (k, v) in res.points() {
                let point = keyed(&mut key, if main { "" } else { id }, k);
                r.set_point(point, v);
                // The row of the point is in a table of the component: state it by the table of the graph.
                let row = v.row.as_ref().and_then(|l| {
                    let table = m.tables.get(usize::from(l.table.0))?;
                    Some((*table, l.key.as_str()))
                });
                r.link_point(point, row);
            }
            np += res.points().len();
            let log = res.log_lines();
            log_sig = log_sig
                .wrapping_mul(31)
                .wrapping_add(log.len())
                .wrapping_add(log.back().map_or(0, |l| l.as_ptr() as usize));
            if main {
                r.set_status(res.status());
            }
        }
        for (k, given) in tables_given.iter().enumerate() {
            if let (false, Some(mut t)) = (*given, r.table_mut(TableId(k as u16))) {
                t.clear();
            }
        }
        for (k, w) in written.iter().enumerate() {
            if !*w {
                // No value fits every field.
                let _ = r.fields_mut().set_value(FieldId(k as u16), &Value::None);
            }
        }
        self.key_scratch = key;
        self.written = written;
        self.tables_given = tables_given;
        // A point that no component publishes any more: start again from what they publish now.
        if r.points().len() > np {
            r.clear_points();
            return self.refresh_results();
        }
        if self.status.is_none() {
            r.set_status(&Status::default());
        }
        if let Some(why) = &self.stopped {
            r.set_status(&Status::new(LockState::Off).with_summary(&why.to_string()));
        }
        if let Some(n) = self.media_audio.notice() {
            if !r.status().summary.contains(n) {
                let mut s = r.status().clone();
                if !s.summary.is_empty() {
                    s.summary.push_str("; ");
                }
                s.summary.push_str(n);
                r.set_status(&s);
            }
        }
        if log_sig != self.log_sig {
            self.log_sig = log_sig;
            self.rebuild_log();
        }
    }

    fn rebuild_log(&mut self) {
        let r = &mut self.results;
        r.clear_log();
        for &i in &self.order {
            let n = &self.nodes[i];
            let Some(res) = n.comp.results() else {
                continue;
            };
            let main = self.status == Some(i);
            for line in res.log_lines() {
                r.log(if main {
                    line.clone()
                } else {
                    format!("{}: {line}", n.id)
                });
            }
        }
        for line in &self.problems {
            r.log(line.as_str());
        }
    }

    /// The setting `"<component>.<key>"`.
    fn component_target(&self, key: &str) -> Result<SettingTarget, SettingError> {
        let unknown = || SettingError::Unknown { key: key.into() };
        let (c, k) = key.split_once('.').ok_or_else(unknown)?;
        let node = *self.by_id.get(c).ok_or_else(unknown)?;
        let id = self.nodes[node]
            .info
            .settings
            .id(k)
            .map_err(|_| unknown())?;
        Ok(SettingTarget { node, id })
    }

    /// The component setting that `key` names: an alias, or `"<component>.<key>"`. This is the one lookup by text. Do it once and
    /// keep the target.
    ///
    /// # Errors
    ///
    /// [`SettingError::Unknown`] if no alias and no component setting has this key.
    pub fn target(&self, key: &str) -> Result<SettingTarget, SettingError> {
        match self.aliases.get(key) {
            Some(t) => self.component_target(t),
            None => self.component_target(key),
        }
    }

    /// The id of the component that holds setting `t`.
    pub fn target_component(&self, t: SettingTarget) -> &str {
        &self.nodes[t.node].id
    }

    /// Makes the graph's own schema: one setting per alias, or every component setting as `"<component>.<key>"` when the graph
    /// has no alias. A fixed setting is marked fixed.
    fn build_schema(&mut self, alias_targets: Vec<SettingTarget>) -> Result<(), GraphError> {
        let named: Vec<(String, SettingTarget)> = if alias_targets.is_empty() {
            let mut all = Vec::new();
            for &i in &self.order {
                let n = &self.nodes[i];
                for (k, s) in n.info.settings.settings().iter().enumerate() {
                    let t = SettingTarget {
                        node: i,
                        id: SettingId(k as u16),
                    };
                    all.push((format!("{}.{}", n.id, s.field.key), t));
                }
            }
            all
        } else {
            self.aliases.keys().cloned().zip(alias_targets).collect()
        };
        let mut decls = Vec::with_capacity(named.len());
        let mut targets = Vec::with_capacity(named.len());
        for (name, t) in named {
            let n = &self.nodes[t.node];
            if let Some(s) = n.info.settings.setting(t.id) {
                let mut s = s.clone();
                s.field.key = Cow::Owned(name);
                s.fixed |= self.fixed_targets.contains(&t);
                if let Some(v) = n.comp.values().value(t.id) {
                    s.default = v.clone();
                }
                decls.push(s);
                targets.push(t);
            }
        }
        self.schema = Arc::new(Schema::new(decls).map_err(GraphError::Schema)?);
        self.values = Values::defaults(&self.schema);
        self.targets = targets;
        self.build_commands()
    }

    /// The component commands that `target` names. Empty if there is none.
    pub(crate) fn command_targets_of(&self, target: &CommandTarget) -> Vec<(usize, CommandId)> {
        let (component, key) = (target.component(), target.command());
        self.order
            .iter()
            .filter(|&&i| component.is_none_or(|c| self.nodes[i].id == c))
            .filter_map(|&i| Some((i, self.nodes[i].comp.commands().id(key).ok()?)))
            .collect()
    }

    /// Makes the graph's command list: one command per command alias, or every component command as
    /// `"<component>.<command>"` when the graph has no command alias.
    fn build_commands(&mut self) -> Result<(), GraphError> {
        let mut defs = Vec::new();
        let mut targets = Vec::new();
        if self.command_aliases.is_empty() {
            for &i in &self.order {
                let n = &self.nodes[i];
                for (k, d) in n.comp.commands().defs().iter().enumerate() {
                    let mut d = d.clone();
                    d.key = Cow::Owned(format!("{}.{}", n.id, d.key));
                    defs.push(d);
                    targets.push(vec![(i, CommandId(k as u16))]);
                }
            }
        }
        let first = self
            .command_order
            .iter()
            .filter_map(|n| self.command_aliases.get_key_value(n));
        let rest = self
            .command_aliases
            .iter()
            .filter(|(n, _)| !self.command_order.contains(n));
        for (name, target) in first.chain(rest) {
            let ts = self.command_targets_of(target);
            let unresolved = || GraphError::UnresolvedCommand {
                key: name.clone(),
                target: target.clone(),
            };
            let &(first, cid) = ts.first().ok_or_else(unresolved)?;
            let def_of =
                |&(i, c): &(usize, CommandId)| &self.nodes[i].comp.commands().defs()[c.index()];
            let mut d = def_of(&(first, cid)).clone();
            if ts
                .iter()
                .any(|t| *def_of(t).args != *d.args || *def_of(t).reply != *d.reply)
            {
                return Err(GraphError::CommandAliasArgs {
                    key: name.clone(),
                    target: target.clone(),
                });
            }
            d.key = Cow::Owned(name.clone());
            defs.push(d);
            targets.push(ts);
        }
        self.commands = Commands::new(defs).map_err(|e| GraphError::Schema(e.into()))?;
        self.command_targets = targets;
        Ok(())
    }

    /// Sets these command aliases on the graph (name to target, as [`GraphBuilder::command_alias`] takes them) in place of
    /// the ones it has. The command list has the aliases of `order` first, in that order.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnresolvedCommand`] or [`GraphError::CommandAliasArgs`]. The graph keeps its commands.
    pub(crate) fn set_command_aliases(
        &mut self,
        aliases: BTreeMap<String, CommandTarget>,
        order: Vec<String>,
    ) -> Result<(), GraphError> {
        let old = std::mem::replace(&mut self.command_aliases, aliases);
        let old_order = std::mem::replace(&mut self.command_order, order);
        let r = self.build_commands();
        if r.is_err() {
            self.command_aliases = old;
            self.command_order = old_order;
        }
        r
    }

    /// The keys of `kept` that are fields and tables of the component at `node`, by the keys that the component
    /// declares. `kept` holds the keys that the stack keeps, as the stack shows them: a field key, or `table` and a
    /// space before a table key.
    fn kept_of(&self, node: usize, kept: &[String]) -> Kept {
        let n = &self.nodes[node];
        let Some(r) = n.comp.results() else {
            return Kept::default();
        };
        let d = &self.declared;
        // The key that the stack shows a key of the component under.
        let is_kept = |key: &str, table: bool| {
            let named = format!("{}.{key}", n.id);
            let by_name = if table {
                d.table_id(&named).is_some()
            } else {
                d.fields().id(&named).is_some()
            };
            let shown = if by_name { named.as_str() } else { key };
            let word = if table { "table " } else { "" };
            kept.iter().any(|k| *k == format!("{word}{shown}"))
        };
        let decl = r.decl();
        let fields: Vec<&str> = decl
            .fields()
            .fields()
            .iter()
            .filter(|f| is_kept(&f.key, false))
            .map(|f| f.key.as_ref())
            .collect();
        let tables: Vec<&str> = decl
            .tables()
            .iter()
            .filter(|t| is_kept(&t.key, true))
            .map(|t| t.key.as_ref())
            .collect();
        Kept::of(&fields, &tables)
    }

    /// Tells each component of the plan that the host changed the frequency ([`Component::forget`]). `kept` holds
    /// the keys that the stack keeps, as the stack shows them. Each component gets the kept keys of its own fields
    /// and tables.
    pub(crate) fn forget(&mut self, kept: &[String]) {
        self.home();
        for node in 0..self.user_nodes {
            let own = self.kept_of(node, kept);
            self.nodes[node].comp.forget(&own);
        }
    }

    /// Tells the component at `node` to forget what `kept` does not hold ([`Component::forget`]), if the stack
    /// keeps a field or a table of it. Returns false, and does nothing, for a component of which nothing is kept.
    pub(crate) fn forget_kept(&mut self, node: usize, kept: &[String]) -> bool {
        self.home();
        let own = self.kept_of(node, kept);
        if own.is_empty() {
            return false;
        }
        self.nodes[node].comp.forget(&own);
        true
    }

    /// True if the component `id` declares the field `key`, or the table `key` if `table` is set.
    pub(crate) fn component_declares(&self, id: &str, key: &str, table: bool) -> bool {
        self.nodes
            .iter()
            .find(|n| n.id == id)
            .and_then(|n| n.comp.results())
            .is_some_and(|r| {
                if table {
                    r.decl().table_id(key).is_some()
                } else {
                    r.decl().fields().id(key).is_some()
                }
            })
    }

    /// The commands of component `node`.
    pub(crate) fn node_commands(&self, node: usize) -> &Commands {
        self.nodes[node].comp.commands()
    }

    /// Runs command `cid` of component `node` with `args`, which the caller has checked.
    pub(crate) fn invoke_component(
        &mut self,
        node: usize,
        cid: CommandId,
        args: &Values,
    ) -> Result<Option<Record>, CommandError> {
        self.home();
        self.nodes[node].comp.invoke(cid, args)
    }

    /// Checks `value` against the component's schema, applies it, and configures the graph again from the component if the
    /// value changes its output formats.
    fn apply_raw(&mut self, t: SettingTarget, value: &Value) -> Result<(), SettingError> {
        self.home();
        let n = &mut self.nodes[t.node];
        n.info.settings.check(t.id, value)?;
        let applied = n.comp.apply(t.id, value)?;
        n.values = n.comp.values();
        if let Some(g) = self.targets.iter().position(|x| *x == t) {
            // The graph's setting is a copy of the component's, whose schema accepted the value.
            let _ = self.values.set(SettingId(g as u16), value);
        }
        if applied == Applied::Reformat && self.configured {
            self.reconfigure_from(t.node)
                .map_err(|e| SettingError::Config {
                    id: t.id,
                    source: Box::new(ConfigError::from(e)),
                })?;
        }
        Ok(())
    }

    /// Changes a component setting. A setting that changes a rate configures again the components it affects. The rest of the
    /// graph keeps running.
    ///
    /// # Errors
    ///
    /// [`SettingError::Fixed`] for a fixed setting. Any refusal of the component's schema or of the component, with the
    /// component's setting id. Nothing changes.
    pub fn apply_to(&mut self, t: SettingTarget, value: &Value) -> Result<(), SettingError> {
        if self.fixed_targets.contains(&t) {
            return Err(SettingError::Fixed { id: t.id });
        }
        self.apply_raw(t, value)
    }

    /// The component of `handle` as an owner. `None` if the graph has no such component.
    pub fn owner(&mut self, handle: ComponentHandle) -> Option<ComponentOwner<'_>> {
        let node = handle.index().filter(|&i| i < self.nodes.len())?;
        Some(ComponentOwner { graph: self, node })
    }

    /// Taps an output (`"<component>.<port>"`, or [`INPUT`]) into a ring of `chunks` buffers, read from any thread. Adding a tap
    /// does not disturb the running graph; dropping the reader removes it.
    pub fn tap(&mut self, port: &str, chunks: usize) -> Result<TapReader, GraphError> {
        self.home();
        let s = resolve_output(&self.nodes, &self.by_id, port)?;
        let (w, r) = tap(self.formats[s].clone(), chunks);
        self.taps.push((s, w));
        self.update_demand();
        Ok(r)
    }

    /// Number of live taps.
    pub fn taps(&self) -> usize {
        self.taps.iter().filter(|(_, t)| !t.closed()).count()
    }

    fn stream(&self, port: &str) -> Option<usize> {
        resolve_output(&self.nodes, &self.by_id, port).ok()
    }

    /// The format of an output, as configured.
    pub fn format(&self, port: &str) -> Option<&Format> {
        self.stream(port).map(|s| &self.formats[s])
    }

    /// The buffer an output holds after the last call (for tests and inspection).
    pub fn buffer(&self, port: &str) -> Option<&Buffer> {
        self.stream(port).map(|s| &self.streams[s])
    }

    /// The results a component publishes itself.
    pub fn component_results(&self, id: &str) -> Option<&Results> {
        self.by_id
            .get(id)
            .and_then(|&i| self.nodes[i].comp.results())
    }

    /// The component `id`, between two passes. The graph runs on the thread that owns it, so this access needs no lock. Use
    /// [`Graph::apply_to`] for a setting, so that a change of rate reconfigures the graph. After a change through this access, call
    /// [`Graph::refresh_results`] to show it at once.
    pub fn component(&self, id: &str) -> Option<&dyn Component> {
        self.by_id.get(id).map(|&i| self.nodes[i].comp.as_ref())
    }

    /// The component `id`, to change between two passes (see [`Graph::component`]).
    pub fn component_mut(&mut self, id: &str) -> Option<&mut dyn Component> {
        self.home();
        let i = *self.by_id.get(id)?;
        Some(self.nodes[i].comp.as_mut())
    }

    /// The component `id` as its own type `C`. `None` when there is no such component, when it is of another type, or when
    /// it does not open itself ([`Component::any_ref`]).
    ///
    /// ```
    /// use std::any::Any;
    /// use sdrkit_core::{Complex32, ConfigError};
    /// use sdrkit_graph::{Component, ComponentInfo, Format, GraphBuilder, Io, Port};
    ///
    /// /// Multiplies by a gain, and opens itself to the graph's owner.
    /// struct Gain(f32);
    /// impl Component for Gain {
    ///     fn info(&self) -> ComponentInfo {
    ///         ComponentInfo::processor("gain", vec![Port::iq("in")], vec![Port::iq("out")])
    ///     }
    ///     fn configure(&mut self, i: &[Format]) -> Result<Vec<Format>, ConfigError> {
    ///         Ok(vec![i[0].clone()])
    ///     }
    ///     fn process(&mut self, io: &mut Io) {
    ///         let g = self.0;
    ///         for s in io.pass_through::<Complex32>(0, 0) {
    ///             *s *= g;
    ///         }
    ///     }
    ///     fn values(&self) -> sdrkit_core::Values { sdrkit_graph::no_values() }
    ///     fn any_ref(&self) -> Option<&dyn Any> { Some(self) }
    ///     fn any_mut(&mut self) -> Option<&mut dyn Any> { Some(self) }
    /// }
    /// let mut g = GraphBuilder::new().add("gain", Gain(1.0)).input("gain").build().unwrap();
    /// g.component_as_mut::<Gain>("gain").unwrap().0 = 2.0;
    /// assert_eq!(g.component_as::<Gain>("gain").unwrap().0, 2.0);
    /// ```
    pub fn component_as<C: Component + 'static>(&self, id: &str) -> Option<&C> {
        self.component(id)?.any_ref()?.downcast_ref::<C>()
    }

    /// The component `id` as its own type `C`, to change between two passes (see [`Graph::component_as`]).
    pub fn component_as_mut<C: Component + 'static>(&mut self, id: &str) -> Option<&mut C> {
        self.component_mut(id)?.any_mut()?.downcast_mut::<C>()
    }

    /// Sets the stack's declaration of the graph's results (see [`Graph::refresh_results`] for the rule), and merges the
    /// results again under it. Points are not declared by key, so they keep the component prefix.
    pub fn declare(&mut self, decl: &ResultsDecl) {
        self.declared = Arc::new(decl.clone());
        self.rebuild_merge();
        self.refresh_results();
    }

    /// Sets the host's audio codec plug-ins for every component, through the run context ([`RunContext::codecs`]).
    pub fn set_codecs(&mut self, codecs: CodecSet) {
        let mut ctx = self.ctx.clone();
        ctx.codecs = Arc::new(codecs);
        self.set_context(ctx);
    }

    /// The stream kinds that the graph gives to the host, each once: the kind of each graph output, and audio for a host
    /// audio sink (`host.out`).
    pub fn host_kinds(&self) -> Vec<Kind> {
        let mut out: Vec<Kind> = Vec::new();
        for n in &self.nodes {
            let given = n
                .outputs
                .iter()
                .zip(&n.kinds)
                .filter(|(s, _)| self.outputs.contains(s))
                .map(|(_, k)| *k);
            let sink = (n.info.name == "host.out").then_some(Kind::Audio);
            for k in given.chain(sink) {
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
        out
    }

    /// Component ids in the order they run.
    pub fn order(&self) -> Vec<&str> {
        self.order
            .iter()
            .map(|&i| self.nodes[i].id.as_str())
            .collect()
    }

    /// Sets the run context for every component (live or recorded, the station, the helper threads). The graph keeps it, updates
    /// `live` at each configure from the input, and offers it again then.
    pub fn set_context(&mut self, ctx: RunContext) {
        self.home();
        self.ctx = ctx;
        for n in &mut self.nodes {
            n.comp.attach(&self.ctx);
        }
    }

    /// The run context.
    pub fn context(&self) -> &RunContext {
        &self.ctx
    }

    /// Passes every component a new station context (see [`Graph::set_context`]).
    pub fn set_station(&mut self, station: &Station) {
        let mut ctx = self.ctx.clone();
        ctx.station = Arc::new(station.clone());
        self.set_context(ctx);
    }

    /// True if the graph has a transmit sink (`tx.sink`).
    pub fn transmits(&self) -> bool {
        self.nodes.iter().any(|n| n.info.name == "tx.sink")
    }

    /// Passes every component the state of the transmitter (see [`RunContext::transmit`]). A call that changes
    /// nothing does nothing.
    pub fn set_transmit(&mut self, on: bool) {
        if self.ctx.transmit != on {
            let mut ctx = self.ctx.clone();
            ctx.transmit = on;
            self.set_context(ctx);
        }
    }

    /// Turns the work counters of the components on or off. On by default: each call of a component is timed with the
    /// monotonic clock (two clock reads) and its items are counted. [`Staged::component_loads`](sdrkit_core::Staged) reads
    /// the counters.
    pub fn set_metering(&mut self, on: bool) {
        self.home();
        self.metering = on;
    }

    /// What the components want kept across sessions, in one blob. `None` if no component keeps anything.
    pub fn save_state(&self) -> Option<StateBlob> {
        let parts: Vec<(&str, StateBlob)> = self
            .order
            .iter()
            .filter_map(|&i| {
                let n = &self.nodes[i];
                n.comp.save_state().map(|b| (n.id.as_str(), b))
            })
            .collect();
        if parts.is_empty() {
            return None;
        }
        Some(StateBlob::new(
            STATE_OWNER,
            STATE_VERSION,
            serde_json::to_vec(&parts).expect("a list of state blobs serialises"),
        ))
    }

    /// Restores what [`Graph::save_state`] returned: each component its own part. Every part is tried. A part for a component that
    /// is not in the graph is skipped.
    ///
    /// # Errors
    ///
    /// [`StateError`] if the blob is not a graph state, or the first refusal of a component.
    pub fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.home();
        let bytes = state.open(STATE_OWNER, STATE_VERSION)?;
        let parts: Vec<(String, StateBlob)> =
            serde_json::from_slice(bytes).map_err(|_| StateError::Damaged)?;
        let mut first = Ok(());
        for (id, b) in &parts {
            if let Some(&i) = self.by_id.get(id) {
                if let Err(e) = self.nodes[i].comp.restore_state(b) {
                    if first.is_ok() {
                        first = Err(e);
                    }
                }
            }
        }
        self.refresh_results();
        first
    }

    /// The graph as a description, which [`Description::build`] turns back into the same graph.
    pub fn describe(&self) -> Description {
        let components = self.nodes[..self.user_nodes]
            .iter()
            .map(|n| {
                let settings = n.comp.values().to_json();
                ComponentDesc {
                    id: n.id.clone(),
                    type_name: n.info.name.clone(),
                    role: Some(n.info.role),
                    settings: match settings {
                        serde_json::Value::Object(m) => m,
                        _ => unreachable!("the values of a schema are an object"),
                    },
                }
            })
            .collect();
        let fixed = self
            .fixed
            .iter()
            .map(|(k, v)| {
                let t = self
                    .component_target(k)
                    .expect("the build step resolved every fixed setting");
                let settings = &self.nodes[t.node].info.settings;
                let mut one = Values::defaults(settings);
                one.set(t.id, v)
                    .expect("the build step checked every fixed value");
                let key = settings.settings()[t.id.index()].field.key.as_ref();
                let json = one.to_json();
                let value = json
                    .get(key)
                    .expect("the values hold every key of the schema");
                (k.clone(), value.clone())
            })
            .collect();
        Description {
            components,
            edges: self.edges.clone(),
            feedback: self.feedback.clone(),
            clock_s: self.clock_s,
            reconfigure_on_retune: self.reconfigure_on_retune,
            outputs: self
                .outputs
                .iter()
                .map(|&s| self.names[s].clone())
                .collect(),
            aliases: self.aliases.clone(),
            command_aliases: self.command_aliases.clone(),
            fixed,
            status: self.status.map(|i| self.nodes[i].id.clone()),
        }
    }
}

const STATE_OWNER: &str = "graph";
const STATE_VERSION: u32 = 1;

/// The graph's settings are the aliases (under their own names) when the graph has any, else every component setting as
/// `"<component>.<key>"`. A fixed setting is marked fixed. Its commands are every component command as
/// `"<component>.<key>"`.
impl Owner for Graph {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }

    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }

    /// Changes setting `id`. A setting that changes a rate configures again the components it affects. The rest of the graph
    /// keeps running. Errors are those of [`Graph::apply_to`], with the graph's setting id.
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let t = *self
            .targets
            .get(id.index())
            .ok_or(SettingError::NoSuchId { id })?;
        let inner = t.id;
        self.apply_raw(t, value)
            .map_err(|e| e.map_ids(|x| if x == inner { id } else { x }))
    }

    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.decl()
    }

    fn results(&self) -> &Results {
        &self.results
    }

    fn commands(&self) -> &Commands {
        &self.commands
    }

    /// Runs command `id` on each component it goes to, in graph order. The first error stops it. The reply is the one of
    /// the last component.
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        self.home();
        let targets = self
            .command_targets
            .get(id.index())
            .ok_or(CommandError::NoSuchId { id })?;
        let mut reply = None;
        for &(node, cid) in targets {
            reply = self.nodes[node].comp.invoke(cid, args)?;
        }
        Ok(reply)
    }

    /// Shows `text` in the status until the next merge, and in the log after the lines of the components.
    fn problem(&mut self, text: &str) {
        if self.problems.len() == self.results.log_cap() {
            self.problems.pop_front();
        }
        self.problems.push_back(text.to_owned());
        self.rebuild_log();
        self.results.update_status(|s| {
            text.clone_into(&mut s.summary);
            true
        });
    }
}

/// One component of a graph as an owner: its settings, results and commands ([`Graph::owner`]).
pub struct ComponentOwner<'a> {
    graph: &'a mut Graph,
    node: usize,
}

impl std::fmt::Debug for ComponentOwner<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentOwner")
            .field("id", &self.graph.nodes[self.node].id)
            .finish()
    }
}

impl Owner for ComponentOwner<'_> {
    fn schema(&self) -> &Arc<Schema> {
        &self.graph.nodes[self.node].info.settings
    }

    fn values(&self) -> Option<&Values> {
        Some(&self.graph.nodes[self.node].values)
    }

    /// Changes setting `id` of the component, as [`Graph::apply_to`] does.
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let t = SettingTarget {
            node: self.node,
            id,
        };
        self.graph.apply_to(t, value)
    }

    fn results_decl(&self) -> &Arc<ResultsDecl> {
        Owner::results(self).decl()
    }

    fn results(&self) -> &Results {
        static NONE: LazyLock<Results> = LazyLock::new(Results::default);
        self.graph.nodes[self.node].comp.results().unwrap_or(&NONE)
    }

    fn commands(&self) -> &Commands {
        self.graph.nodes[self.node].comp.commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        self.graph.home();
        self.graph.nodes[self.node].comp.invoke(id, args)
    }
}

impl Receiver for Graph {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        if self.ctx.live != input.live || !self.configured {
            let mut ctx = self.ctx.clone();
            ctx.live = input.live;
            self.set_context(ctx);
        }
        let f = Format::iq(input.rate_hz, input.centre_hz, input.tuned_hz);
        self.configure_input(f).map_err(ConfigError::from)
    }

    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        self.process(iq, out)
    }

    fn finish(&mut self, out: &mut Output) {
        Graph::finish(self, out)
    }

    fn discontinuity(&mut self, lost: u64) {
        self.discontinuity_after(0, lost);
    }

    fn staged(&mut self) -> Option<&mut dyn sdrkit_core::Staged> {
        Some(self)
    }

    /// A new centre or tuned frequency. Without [`GraphBuilder::reconfigure_on_retune`] (and at the same rate) a `Retuned` tag goes
    /// down the graph from the first sample of the next call, and the components keep their state; with it, or at a new rate, the
    /// whole graph is configured again. The clock restarts either way.
    fn retuned(&mut self, input: &InputInfo) {
        self.home();
        // An input that needs a fixed centre needs a new shift: validate again. A shift that follows the tuned frequency moves
        // at the retune's sample by itself.
        let fixed = self.nodes[..self.user_nodes].iter().any(|n| {
            n.info.inputs.iter().any(|p| {
                matches!(
                    p.format.centre,
                    Some(CentreRule::Fixed(_) | CentreRule::MayLeave(_))
                )
            })
        });
        let centred = fixed || (self.tuned_unfollowed && input.tuned_hz != input.centre_hz);
        if !self.configured
            || self.reconfigure_on_retune
            || centred
            || input.rate_hz != self.formats[0].rate
        {
            let f = Format::iq(input.rate_hz, input.centre_hz, input.tuned_hz);
            if let Err(e) = self.configure_input(f) {
                // The graph cannot run at the new frequency: it stops, and its status says why.
                self.configured = false;
                self.stopped = Some(e);
                self.refresh_results();
            }
            return;
        }
        self.formats[0].centre_hz = input.centre_hz;
        self.formats[0].tuned_hz = input.tuned_hz;
        self.retune = Some((input.centre_hz, input.tuned_hz));
        self.clock_pos = 0;
        self.tell_frequency();
    }

    fn set_station(&mut self, station: &Station) {
        Graph::set_station(self, station)
    }

    fn transmits(&self) -> bool {
        Graph::transmits(self)
    }

    fn set_transmit(&mut self, on: bool) {
        Graph::set_transmit(self, on)
    }

    fn save_state(&self) -> Option<StateBlob> {
        Graph::save_state(self)
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        Graph::restore_state(self, state)
    }

    /// The graph itself, so a host holding a `Box<dyn Receiver>` can tap its ports ([`Graph::tap`]).
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
