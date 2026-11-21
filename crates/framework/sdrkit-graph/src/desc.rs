//! Graphs from descriptions: a JSON format listing components and edges, and the registry that turns type names into components.
//!
//! ```json
//! {
//!   "components": [
//!     { "id": "chan",  "type": "dsp.channel", "role": "processor", "settings": { "normal": 10000, "rate": 24000 } },
//!     { "id": "demod", "type": "dsp.am_demod" },
//!     { "id": "audio", "type": "dsp.audio_resampler", "settings": { "rate": 48000 } }
//!   ],
//!   "edges": [["input", "chan.in"], ["chan.out", "demod.in"], ["demod.out", "audio.in"]],
//!   "outputs": ["audio.out"],
//!   "aliases": { "width": "chan.filter" },
//!   "fixed": { "demod.detector": "envelope" },
//!   "status": "demod"
//! }
//! ```
//!
//! A graph that exposes commands under its own names has `"command_aliases"` (see [`crate::GraphBuilder::command_alias`]).
//! A graph with loops and periodic decisions also has `"feedback"` (edges read one pass late, see
//! [`GraphBuilder::feedback`]), `"clock_s"` (see [`crate::GraphBuilder::clock`]) and `"reconfigure_on_retune"` (see
//! [`crate::GraphBuilder::reconfigure_on_retune`]).
//!
//! `role` is optional; when given it must match the component's own. [`Graph::describe`] returns the description of a running graph,
//! with every component's current settings, so a description round-trips. [`Graph::validate`] on the built graph returns the
//! formats of its ports and the adapters it inserts ([`crate::ValidateReport`]).

use crate::adapt::{Adapter, Catalog};
use crate::component::{Component, ComponentInfo, Port, Role};
use crate::graph::{DescPosition, Graph, GraphError, SyntaxError};
use crate::media::FourCc;
use crate::plan::GraphPlan;
use sdrkit_core::Schema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};
use std::collections::BTreeMap;
use std::sync::Arc;

/// One component of a [`Description`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentDesc {
    /// Its id in the graph.
    pub id: String,
    /// Its type name in the [`Registry`].
    #[serde(rename = "type")]
    pub type_name: String,
    /// Its role, validated against the component's own when given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// Settings applied before the graph starts, in the settings JSON form of the component's schema.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub settings: Map<String, Json>,
}

/// A graph as data: components, edges (`["from.port", "to.port"]`, with `"input"` for the graph's `Iq` input), outputs, aliases,
/// fixed settings and the status component.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Description {
    /// The components.
    pub components: Vec<ComponentDesc>,
    /// The edges.
    #[serde(default)]
    pub edges: Vec<(String, String)>,
    /// Feedback edges: the input reads what the output produced in the previous pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feedback: Vec<(String, String)>,
    /// The graph's clock period in seconds, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_s: Option<f64>,
    /// A retune configures the whole graph again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reconfigure_on_retune: bool,
    /// The graph outputs.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Settings exposed under the graph's own names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliases: BTreeMap<String, String>,
    /// Commands exposed under the graph's own names: a name to its target, `{"one": {"component": .., "command": ..}}` or
    /// `{"every": "<command>"}` (see [`GraphBuilder::command_alias`](crate::GraphBuilder::command_alias)).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub command_aliases: BTreeMap<String, crate::CommandTarget>,
    /// Settings fixed by the graph (`"<component>.<key>"`), in the settings JSON form.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fixed: BTreeMap<String, Json>,
    /// The component whose status the graph reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

impl Description {
    /// Parses JSON. The parse is strict: an unknown entry is refused.
    ///
    /// # Errors
    ///
    /// [`GraphError::Syntax`] with the line and column.
    pub fn from_json(json: &str) -> Result<Description, GraphError> {
        serde_json::from_str(json).map_err(|e| GraphError::Syntax {
            line: e.line(),
            column: e.column(),
            category: match e.classify() {
                serde_json::error::Category::Data => SyntaxError::Form,
                serde_json::error::Category::Eof => SyntaxError::Eof,
                _ => SyntaxError::Json,
            },
        })
    }
    /// As JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a description serialises")
    }
    /// Builds the graph with the components of `registry`, through the same checked steps as a host ([`GraphPlan`]).
    ///
    /// # Errors
    ///
    /// [`GraphError::Description`] with the position of the entry that does not fit and the error of the step.
    pub fn build(&self, registry: &Registry) -> Result<Graph, GraphError> {
        self.plan(registry)?.build()
    }

    /// The plan of the graph with the components of `registry`, before [`GraphPlan::build`].
    ///
    /// # Errors
    ///
    /// As [`Description::build`], except the checks of [`GraphPlan::build`].
    pub fn plan(&self, registry: &Registry) -> Result<GraphPlan, GraphError> {
        let mut p = GraphPlan::new();
        for (index, c) in self.components.iter().enumerate() {
            let at = || DescPosition::Component { index };
            p.add(registry, &c.id, &c.type_name)
                .map_err(|e| e.at(at()))?;
            if let Some(role) = c.role {
                let own = registry
                    .descriptor(&c.type_name)
                    .map_err(|e| e.at(at()))?
                    .role;
                if own != role {
                    return Err(GraphError::RoleMismatch {
                        component: c.id.clone(),
                        described: role,
                        actual: own,
                    }
                    .at(at()));
                }
            }
            for (k, v) in &c.settings {
                if !self.fixed.contains_key(&format!("{}.{k}", c.id)) {
                    p.set_json(&c.id, k, v).map_err(|e| {
                        e.at(DescPosition::ComponentSetting {
                            index,
                            key: k.clone(),
                        })
                    })?;
                }
            }
        }
        let edges = self.edges.iter().map(|e| (e, false));
        let feedback = self.feedback.iter().map(|e| (e, true));
        for (index, ((from, to), back)) in edges.chain(feedback).enumerate() {
            let at = if back {
                DescPosition::Feedback {
                    index: index - self.edges.len(),
                }
            } else {
                DescPosition::Edge { index }
            };
            let f = p.output_port(from).map_err(|e| e.at(at.clone()))?;
            let t = p.input_port(to).map_err(|e| e.at(at.clone()))?;
            let done = if back {
                p.feedback(f, t)
            } else {
                p.connect(f, t)
            };
            done.map_err(|error| {
                GraphError::Connect {
                    from: from.clone(),
                    to: to.clone(),
                    error: Box::new(error),
                }
                .at(at)
            })?;
        }
        if let Some(c) = self.clock_s {
            p.clock(c);
        }
        if self.reconfigure_on_retune {
            p.reconfigure_on_retune();
        }
        for (index, o) in self.outputs.iter().enumerate() {
            let at = || DescPosition::Output { index };
            let f = p.output_port(o).map_err(|e| e.at(at()))?;
            p.output(f).map_err(|error| {
                GraphError::Connect {
                    from: o.clone(),
                    to: String::new(),
                    error: Box::new(error),
                }
                .at(at())
            })?;
        }
        for (k, v) in &self.aliases {
            p.alias(k, v)
                .map_err(|e| e.at(DescPosition::Alias { name: k.clone() }))?;
        }
        for (k, v) in &self.command_aliases {
            p.command_alias(k, v.clone())
                .map_err(|e| e.at(DescPosition::CommandAlias { name: k.clone() }))?;
        }
        for (k, v) in &self.fixed {
            p.fix_json(k, v)
                .map_err(|e| e.at(DescPosition::Fixed { key: k.clone() }))?;
        }
        if let Some(s) = &self.status {
            p.status_from(s).map_err(|e| e.at(DescPosition::Status))?;
        }
        Ok(p)
    }
}

/// Makes a component of one type.
pub type MakeComponent = Box<dyn Fn() -> Box<dyn Component> + Send + Sync>;

/// Component types by name. The standard library and each stack add theirs; a description is built from it.
#[derive(Default)]
pub struct Registry {
    makers: BTreeMap<String, (MakeComponent, ComponentDescriptor)>,
    decoders: Vec<MediaDecoder>,
    catalog: Arc<Catalog>,
}

/// What a host reads about a component kind before it makes one: its kind key, its role, its ports with their format rules,
/// and its settings schema.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentDescriptor {
    /// The kind key (`"channel"`).
    pub kind: String,
    /// What it is for.
    pub role: Role,
    /// The inputs, in order. A connection names an input by its index here.
    pub inputs: Vec<Port>,
    /// The outputs, in order.
    pub outputs: Vec<Port>,
    /// The settings it accepts.
    pub settings: Arc<Schema>,
}

impl ComponentDescriptor {
    /// The descriptor of a component that declares `info`.
    pub fn from_info(info: ComponentInfo) -> Self {
        ComponentDescriptor {
            kind: info.name,
            role: info.role,
            inputs: info.inputs,
            outputs: info.outputs,
            settings: info.settings,
        }
    }
}

/// A decoder type, as [`Registry::decoder_for`] chooses among them.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaDecoder {
    /// The component type name.
    pub name: String,
    /// The codec codes it decodes.
    pub codes: Vec<FourCc>,
    /// It uses the platform's hardware (chosen before software ones).
    pub hardware: bool,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.makers.keys()).finish()
    }
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }
    /// A registry holding the standard components ([`crate::components`]).
    pub fn standard() -> Self {
        let mut r = Self::new();
        crate::components::register(&mut r);
        r
    }
    /// Adds (or replaces) a type. The name must be the one the component's `info().name` returns, so a graph describes itself
    /// with it. It makes one component now to read its descriptor.
    pub fn add(
        &mut self,
        name: &str,
        make: impl Fn() -> Box<dyn Component> + Send + Sync + 'static,
    ) -> &mut Self {
        let mut d = ComponentDescriptor::from_info(make().info());
        d.kind = name.into();
        self.makers.insert(name.into(), (Box::new(make), d));
        self
    }
    /// Adds a decoder type: a component that decodes the codecs with these codes. A host adds its hardware decoders
    /// (`hardware: true`); portable software decoders are added with `false`.
    pub fn add_decoder(
        &mut self,
        name: &str,
        codes: &[FourCc],
        hardware: bool,
        make: impl Fn() -> Box<dyn Component> + Send + Sync + 'static,
    ) -> &mut Self {
        self.add(name, make);
        self.decoders.retain(|d| d.name != name);
        self.decoders.push(MediaDecoder {
            name: name.into(),
            codes: codes.to_vec(),
            hardware,
        });
        self
    }
    /// Adds a conversion to the adapter catalog, and its component kind when the registry does not have it.
    pub fn add_adapter(&mut self, adapter: Adapter) -> &mut Self {
        if !self.makers.contains_key(adapter.kind) {
            self.add(adapter.kind, adapter.make);
        }
        Arc::make_mut(&mut self.catalog).add(adapter);
        self
    }
    /// The adapter catalog: the conversions a graph built from this registry inserts.
    pub fn catalog(&self) -> &Arc<Catalog> {
        &self.catalog
    }
    /// The decoder type for a codec code: a hardware one if there is one, else a software one, the first added of each.
    pub fn decoder_for(&self, code: FourCc) -> Option<&MediaDecoder> {
        let mut fits = self.decoders.iter().filter(|d| d.codes.contains(&code));
        let first = fits.clone().find(|d| d.hardware);
        first.or_else(|| fits.next())
    }
    /// Makes a component of type `name`, at its defaults.
    pub fn make(&self, name: &str) -> Result<Box<dyn Component>, GraphError> {
        self.makers
            .get(name)
            .map(|(m, _)| m())
            .ok_or_else(|| GraphError::UnknownType(name.into()))
    }
    /// The descriptor of type `name`. No component is made.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnknownType`].
    pub fn descriptor(&self, name: &str) -> Result<&ComponentDescriptor, GraphError> {
        self.makers
            .get(name)
            .map(|(_, d)| d)
            .ok_or_else(|| GraphError::UnknownType(name.into()))
    }
    /// The descriptors of every type, in name order. No component is made.
    pub fn descriptors(&self) -> impl Iterator<Item = &ComponentDescriptor> {
        self.makers.values().map(|(_, d)| d)
    }
    /// The type names, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.makers.keys().map(|k| k.as_str())
    }
}
