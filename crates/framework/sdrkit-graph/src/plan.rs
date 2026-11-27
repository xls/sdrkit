//! A graph built one checked step at a time. Each step is refused at once when it does not fit: an unknown component kind, a
//! setting that its component's schema refuses, or a connection that no conversion can join ([`ConnectError`]).
//! [`GraphPlan::build`] then makes the [`Graph`]. Hosts, the C API and graph descriptions all build graphs through this type.

use crate::adapt::Catalog;
use crate::command::CommandTarget;
use crate::component::{Component, ComponentInfo, KindRule};
use crate::connect::{check_kind, ComponentHandle, ConnectError, PortRef};
use crate::desc::{ComponentDesc, Description, Registry};
use crate::graph::{assemble, port_names, Assembly, Graph, GraphError, Side, INPUT};
use crate::stream::Kind;
use sdrkit_core::{ResultsDecl, SettingError, SettingId, Value, Values};
use std::collections::BTreeMap;
use std::sync::Arc;

struct Planned {
    id: String,
    info: ComponentInfo,
    comp: Box<dyn Component>,
}

/// A graph being built. Every method checks its step and returns an error when the step does not fit, and the plan stays as it
/// was. Formats (rates, centre frequencies) are known only when the graph runs, so the format of each connection is checked when
/// the graph is configured.
///
/// ```
/// use sdrkit_graph::{GraphPlan, PortRef, Registry};
/// use sdrkit_core::Value;
/// let registry = Registry::standard();
/// let mut plan = GraphPlan::new();
/// let chan = plan.add(&registry, "chan", "dsp.channel").unwrap();
/// let demod = plan.add(&registry, "demod", "dsp.am_demod").unwrap();
/// plan.set("chan", "rate", &Value::Hz(24e3)).unwrap();
/// assert!(plan.set("chan", "rate", &Value::Text("fast".into())).is_err());
/// plan.input(PortRef::new(chan, 0)).unwrap();
/// plan.connect(PortRef::new(chan, 0), PortRef::new(demod, 0)).unwrap();
/// let graph = plan.build().unwrap();
/// assert_eq!(graph.order(), ["chan", "demod"]);
/// ```
#[derive(Default)]
pub struct GraphPlan {
    comps: Vec<Planned>,
    edges: Vec<(PortRef, PortRef)>,
    feedback: Vec<(PortRef, PortRef)>,
    outputs: Vec<PortRef>,
    /// Alias name to its target text and its resolved target.
    aliases: BTreeMap<String, (String, usize, SettingId)>,
    command_aliases: BTreeMap<String, CommandTarget>,
    fixed: BTreeMap<String, Value>,
    fixed_targets: Vec<(usize, SettingId)>,
    status: Option<usize>,
    clock_s: Option<f64>,
    reconfigure_on_retune: bool,
    declared: Option<Arc<ResultsDecl>>,
    /// The adapters of the registry that the plan was built with.
    catalog: Option<Arc<Catalog>>,
    marks: Vec<crate::graph::StageMark>,
    first_stage: Option<String>,
    costs: Vec<(String, f64)>,
}

impl std::fmt::Debug for GraphPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphPlan")
            .field(
                "components",
                &self.comps.iter().map(|c| &c.id).collect::<Vec<_>>(),
            )
            .field("edges", &self.edges)
            .finish()
    }
}

impl GraphPlan {
    /// An empty plan.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a component of the registered kind `kind` under `id`, at its default settings, and returns its handle. Errors: `id`
    /// is not a valid id or is taken, the registry has no such kind, or the component's ports do not fit its role.
    pub fn add(
        &mut self,
        registry: &Registry,
        id: &str,
        kind: &str,
    ) -> Result<ComponentHandle, GraphError> {
        self.check_id(id)?;
        let comp = registry.make(kind)?;
        if self.catalog.is_none() {
            self.catalog = Some(Arc::clone(registry.catalog()));
        }
        self.add_component(id, comp)
    }

    /// Adds a component made in code under `id`, and returns its handle. Errors as for [`GraphPlan::add`].
    pub fn add_component(
        &mut self,
        id: &str,
        comp: Box<dyn Component>,
    ) -> Result<ComponentHandle, GraphError> {
        self.check_id(id)?;
        let info = comp.info();
        crate::graph::check_role(id, &info)?;
        self.comps.push(Planned {
            id: id.into(),
            info,
            comp,
        });
        Ok(ComponentHandle((self.comps.len() - 1) as u32))
    }

    fn check_id(&self, id: &str) -> Result<(), GraphError> {
        if id.is_empty()
            || id == INPUT
            || !id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(GraphError::BadId(id.into()));
        }
        if self.comps.iter().any(|c| c.id == id) {
            return Err(GraphError::DuplicateId(id.into()));
        }
        Ok(())
    }

    fn find(&self, id: &str) -> Result<usize, GraphError> {
        self.comps
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| GraphError::UnknownComponent(id.into()))
    }

    fn setting_id(&self, i: usize, full: &str, key: &str) -> Result<SettingId, GraphError> {
        self.comps[i]
            .info
            .settings
            .id(key)
            .map_err(|_| GraphError::Setting {
                key: full.into(),
                error: SettingError::Unknown { key: full.into() },
            })
    }

    /// Sets the setting `key` of component `id`. The value must pass the component's schema ([`sdrkit_core::Schema::check`]):
    /// the right kind, inside the range, one of the choices. Nothing is clamped. The component must also take it. A refused value
    /// changes nothing.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnknownComponent`], [`GraphError::Setting`] for an unknown key, or [`GraphError::NamedSetting`] for a
    /// fixed setting or a refused value.
    pub fn set(&mut self, id: &str, key: &str, value: &Value) -> Result<(), GraphError> {
        let i = self.find(id)?;
        let full = format!("{id}.{key}");
        let sid = self.setting_id(i, &full, key)?;
        let c = &mut self.comps[i];
        let refuse = |error: SettingError, schema: &sdrkit_core::Schema| {
            GraphError::Setting {
                key: full.clone(),
                error,
            }
            .named(schema)
        };
        if self.fixed_targets.contains(&(i, sid)) {
            return Err(refuse(SettingError::Fixed { id: sid }, &c.info.settings));
        }
        c.info
            .settings
            .check(sid, value)
            .map_err(|e| refuse(e, &c.info.settings))?;
        c.comp
            .apply(sid, value)
            .map_err(|e| refuse(e, &c.info.settings))?;
        Ok(())
    }

    /// Sets the setting that `key` names: an alias of the plan, or `"<component>.<key>"`.
    ///
    /// # Errors
    ///
    /// As [`GraphPlan::set`].
    pub(crate) fn set_key(&mut self, key: &str, value: &Value) -> Result<(), GraphError> {
        let target = self.aliases.get(key).map_or(key, |(t, _, _)| t.as_str());
        let (id, k) = target.split_once('.').ok_or_else(|| GraphError::Setting {
            key: key.into(),
            error: SettingError::Unknown { key: key.into() },
        })?;
        let (id, k) = (id.to_owned(), k.to_owned());
        self.set(&id, &k, value)
    }

    /// The value that the JSON form `json` of setting `key` of component `i` stands for.
    fn json_value(
        &self,
        i: usize,
        full: &str,
        key: &str,
        json: &serde_json::Value,
    ) -> Result<Value, GraphError> {
        let sid = self.setting_id(i, full, key)?;
        let mut v = Values::defaults(&self.comps[i].info.settings);
        let mut one = serde_json::Map::new();
        one.insert(key.into(), json.clone());
        v.update_from_json(&serde_json::Value::Object(one))
            .map_err(|error| {
                GraphError::SettingValue {
                    key: full.into(),
                    error,
                }
                .named(&self.comps[i].info.settings)
            })?;
        Ok(v.value(sid)
            .expect("the schema declares the setting")
            .clone())
    }

    /// Sets the setting `key` of component `id` from its JSON form (the settings JSON form of the component's schema), as a
    /// graph description returns it.
    ///
    /// # Errors
    ///
    /// As [`GraphPlan::set`], and [`GraphError::SettingValue`] for JSON that does not fit the setting.
    pub(crate) fn set_json(
        &mut self,
        id: &str,
        key: &str,
        json: &serde_json::Value,
    ) -> Result<(), GraphError> {
        let i = self.find(id)?;
        let v = self.json_value(i, &format!("{id}.{key}"), key, json)?;
        self.set(id, key, &v)
    }

    /// Sets `"<component>.<key>"` like [`GraphPlan::set`] and refuses every later change to it.
    ///
    /// # Errors
    ///
    /// As [`GraphPlan::set`].
    pub fn fix(&mut self, key: &str, value: &Value) -> Result<(), GraphError> {
        let (id, k) = key.split_once('.').ok_or_else(|| GraphError::Setting {
            key: key.into(),
            error: SettingError::Unknown { key: key.into() },
        })?;
        self.set(id, k, value)?;
        let i = self.find(id)?;
        let sid = self.setting_id(i, key, k)?;
        self.fixed_targets.push((i, sid));
        self.fixed.insert(key.into(), value.clone());
        Ok(())
    }

    /// Fixes `"<component>.<key>"` from its JSON form, as a graph description writes it.
    ///
    /// # Errors
    ///
    /// As [`GraphPlan::set_json`].
    pub(crate) fn fix_json(
        &mut self,
        key: &str,
        json: &serde_json::Value,
    ) -> Result<(), GraphError> {
        let bad = || GraphError::Setting {
            key: key.into(),
            error: SettingError::Unknown { key: key.into() },
        };
        let (id, k) = key.split_once('.').ok_or_else(bad)?;
        let i = self.find(id)?;
        let v = self.json_value(i, key, k, json)?;
        self.fix(key, &v)
    }

    /// The handle of component `id`. This is the one lookup by id. Do it once and keep the handle.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnknownComponent`].
    pub fn handle(&self, id: &str) -> Result<ComponentHandle, GraphError> {
        self.find(id).map(|i| ComponentHandle(i as u32))
    }

    /// The output that `end` names: `"<component>.<port>"`, `"<component>"` when it has one output, or [`INPUT`]. This is the
    /// one lookup by port name. Do it once and keep the port.
    ///
    /// # Errors
    ///
    /// [`GraphError::UnknownComponent`], [`GraphError::UnknownPort`] or [`GraphError::AmbiguousPort`].
    pub fn output_port(&self, end: &str) -> Result<PortRef, GraphError> {
        let (c, p) = split(end);
        if c == INPUT {
            return match p {
                None | Some("out") => Ok(PortRef::INPUT),
                Some(p) => Err(GraphError::UnknownPort {
                    component: INPUT.into(),
                    side: Side::Output,
                    port: p.into(),
                    available: vec!["out".into()],
                }),
            };
        }
        let i = self.find(c)?;
        let o = port_index(c, Side::Output, &self.comps[i].info.outputs, p)?;
        Ok(PortRef::new(ComponentHandle(i as u32), o))
    }

    /// The input that `end` names: `"<component>.<port>"`, or `"<component>"` when it has one input, or one required input
    /// beside optional ones. Do it once and keep the port.
    ///
    /// # Errors
    ///
    /// As [`GraphPlan::output_port`].
    pub fn input_port(&self, end: &str) -> Result<PortRef, GraphError> {
        let (c, p) = split(end);
        let i = self.find(c)?;
        let ports = &self.comps[i].info.inputs;
        let k = match p {
            Some(_) => port_index(c, Side::Input, ports, p)?,
            None => {
                let mut required = (0..ports.len()).filter(|&k| ports[k].optional.is_none());
                match (ports.len(), required.next(), required.next()) {
                    (1, _, _) => 0,
                    (_, Some(k), None) => k,
                    _ => port_index(c, Side::Input, ports, None)?,
                }
            }
        };
        Ok(PortRef::new(ComponentHandle(i as u32), k))
    }

    /// The component of `h`, or the error for a handle the plan does not have.
    fn planned(&self, h: ComponentHandle) -> Result<&Planned, ConnectError> {
        h.index()
            .and_then(|i| self.comps.get(i))
            .ok_or(ConnectError::NoSuchComponent { component: h })
    }

    /// The output port of `r`, or the error for a port that does not exist. `None` for the graph's input.
    fn out_port(&self, r: PortRef) -> Result<Option<&crate::component::Port>, ConnectError> {
        if r.component == ComponentHandle::INPUT {
            return if r.port == 0 {
                Ok(None)
            } else {
                Err(ConnectError::NoSuchPort {
                    component: r.component,
                    side: Side::Output,
                    port: r.port,
                })
            };
        }
        let c = self.planned(r.component)?;
        c.info
            .outputs
            .get(r.port)
            .map(Some)
            .ok_or(ConnectError::NoSuchPort {
                component: r.component,
                side: Side::Output,
                port: r.port,
            })
    }

    /// The kind an output carries, when it is known now: a fixed kind, or the kind of the input it follows once that input is
    /// connected. `None` when it depends on a connection not made yet.
    fn output_kind(&self, from: PortRef, depth: usize) -> Option<Kind> {
        let port = match self.out_port(from) {
            Ok(Some(port)) => port,
            Ok(None) => return Some(Kind::Iq),
            Err(_) => return None,
        };
        if depth > self.comps.len() {
            return None;
        }
        match &port.format.kind {
            KindRule::Is(k) => Some(*k),
            KindRule::SameAs(n) => {
                let to = PortRef::new(from.component, *n);
                let (src, _) = self.edges.iter().find(|(_, t)| *t == to)?;
                self.output_kind(*src, depth + 1)
            }
            _ => None,
        }
    }

    fn link(&mut self, from: PortRef, to: PortRef, feedback: bool) -> Result<(), ConnectError> {
        let out = self.out_port(from)?;
        let c = self.planned(to.component)?;
        let input = c.info.inputs.get(to.port).ok_or(ConnectError::NoSuchPort {
            component: to.component,
            side: Side::Input,
            port: to.port,
        })?;
        if self
            .edges
            .iter()
            .chain(&self.feedback)
            .any(|(_, t)| *t == to)
        {
            return Err(ConnectError::PortTaken { to });
        }
        if feedback && from.component == ComponentHandle::INPUT {
            return Err(ConnectError::InputAsFeedback { to });
        }
        if let Some(k) = self.output_kind(from, 0) {
            check_kind(
                &self.catalog(),
                from,
                to,
                k,
                out.map(|p| &p.format),
                &input.format,
            )?;
        }
        let list = if feedback {
            &mut self.feedback
        } else {
            &mut self.edges
        };
        list.push((from, to));
        Ok(())
    }

    /// Connects output `from` to input `to`.
    ///
    /// # Errors
    ///
    /// [`ConnectError::NoSuchComponent`] or [`ConnectError::NoSuchPort`] for a port that does not exist,
    /// [`ConnectError::PortTaken`] for an input that is connected already, [`ConnectError::NoConversion`] when no conversion
    /// can join the kinds, [`ConnectError::ItemTypeMismatch`] and [`ConnectError::CodecMismatch`]. When the output's kind
    /// depends on a connection not made yet, the kind is checked by [`GraphPlan::build`]. The rate, the channels and the
    /// sample type are checked when the graph is configured.
    pub fn connect(&mut self, from: PortRef, to: PortRef) -> Result<(), ConnectError> {
        self.link(from, to, false)
    }

    /// Feeds the graph's own `Iq` input to `to`. The same as `connect(PortRef::INPUT, to)`.
    ///
    /// # Errors
    ///
    /// As [`GraphPlan::connect`].
    pub fn input(&mut self, to: PortRef) -> Result<(), ConnectError> {
        self.link(PortRef::INPUT, to, false)
    }

    /// Connects an output to an input of an earlier component, read one pass late (see [`GraphBuilder::feedback`](crate::GraphBuilder::feedback)).
    ///
    /// # Errors
    ///
    /// As [`GraphPlan::connect`], and [`ConnectError::InputAsFeedback`] for the graph's own input.
    pub fn feedback(&mut self, from: PortRef, to: PortRef) -> Result<(), ConnectError> {
        self.link(from, to, true)
    }

    /// Makes an output a graph output (see [`GraphBuilder::output`](crate::GraphBuilder::output)).
    ///
    /// # Errors
    ///
    /// [`ConnectError::NoSuchComponent`] or [`ConnectError::NoSuchPort`].
    pub fn output(&mut self, from: PortRef) -> Result<(), ConnectError> {
        self.out_port(from)?;
        if !self.outputs.contains(&from) {
            self.outputs.push(from);
        }
        Ok(())
    }

    /// Exposes `"<component>.<key>"` under `name`. Errors: no such component or setting.
    pub fn alias(&mut self, name: &str, target: &str) -> Result<(), GraphError> {
        let bad = || GraphError::Setting {
            key: name.into(),
            error: SettingError::Unknown { key: target.into() },
        };
        let (c, k) = target.split_once('.').ok_or_else(bad)?;
        let i = self.find(c)?;
        let sid = self.comps[i].info.settings.id(k).map_err(|_| bad())?;
        self.aliases.insert(name.into(), (target.into(), i, sid));
        Ok(())
    }

    /// Exposes a component command under `name` (see [`GraphBuilder::command_alias`](crate::GraphBuilder::command_alias)).
    ///
    /// # Errors
    ///
    /// [`GraphError::UnresolvedCommand`] if no component has the command that `target` names.
    pub fn command_alias(&mut self, name: &str, target: CommandTarget) -> Result<(), GraphError> {
        let (component, key) = (target.component(), target.command());
        let found = self
            .comps
            .iter()
            .any(|c| component.is_none_or(|id| c.id == id) && c.comp.commands().id(key).is_ok());
        if !found {
            return Err(GraphError::UnresolvedCommand {
                key: name.into(),
                target,
            });
        }
        self.command_aliases.insert(name.into(), target);
        Ok(())
    }

    /// Takes the graph's status from component `id`. Errors: no such component.
    pub fn status_from(&mut self, id: &str) -> Result<(), GraphError> {
        self.status = Some(self.find(id)?);
        Ok(())
    }

    /// Sets the stack's declaration of the graph's results (see [`GraphBuilder::declare`](crate::GraphBuilder::declare)).
    pub fn declare(&mut self, decl: Arc<ResultsDecl>) {
        self.declared = Some(decl);
    }

    /// Sets a clock of `seconds` on the graph (see [`GraphBuilder::clock`](crate::GraphBuilder::clock)).
    pub fn clock(&mut self, seconds: f64) {
        self.clock_s = (seconds > 0.0).then_some(seconds);
    }

    /// A retune configures the whole graph again (see [`GraphBuilder::reconfigure_on_retune`](crate::GraphBuilder::reconfigure_on_retune)).
    pub fn reconfigure_on_retune(&mut self) {
        self.reconfigure_on_retune = true;
    }

    /// Uses the adapter catalog `catalog` for connect and validate, in place of the catalog of the registry of the first
    /// [`GraphPlan::add`].
    pub fn set_catalog(&mut self, catalog: &Arc<Catalog>) {
        self.catalog = Some(Arc::clone(catalog));
    }

    /// The adapters that connect and validate use: the one given to [`GraphPlan::set_catalog`], those of the registry of the
    /// first [`GraphPlan::add`], or the standard catalog.
    fn catalog(&self) -> Arc<Catalog> {
        self.catalog.clone().unwrap_or_else(Catalog::standard_arc)
    }

    /// The component ids, in the order they were added.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.comps.iter().map(|c| c.id.as_str())
    }

    /// The plan as a description, which [`Description::plan`] turns back into the same plan with the registry that made its
    /// components. The declaration of [`GraphPlan::declare`] is not in it.
    pub fn describe(&self) -> Description {
        let out_name = |r: &PortRef| match r.component.index() {
            None => INPUT.to_owned(),
            Some(i) => format!(
                "{}.{}",
                self.comps[i].id, self.comps[i].info.outputs[r.port].name
            ),
        };
        let in_name = |r: &PortRef| match r.component.index() {
            None => INPUT.to_owned(),
            Some(i) => format!(
                "{}.{}",
                self.comps[i].id, self.comps[i].info.inputs[r.port].name
            ),
        };
        let edges = |list: &[(PortRef, PortRef)]| {
            list.iter()
                .map(|(f, t)| (out_name(f), in_name(t)))
                .collect::<Vec<_>>()
        };
        let components = self
            .comps
            .iter()
            .map(|c| ComponentDesc {
                id: c.id.clone(),
                type_name: c.info.name.clone(),
                role: Some(c.info.role),
                settings: match c.comp.values().to_json() {
                    serde_json::Value::Object(m) => m,
                    _ => serde_json::Map::new(),
                },
            })
            .collect();
        let fixed = self
            .fixed
            .iter()
            .filter_map(|(k, v)| {
                let (id, key) = k.split_once('.')?;
                let schema = &self.comps[self.find(id).ok()?].info.settings;
                let mut one = Values::defaults(schema);
                one.set(schema.id(key).ok()?, v).ok()?;
                Some((k.clone(), one.to_json().get(key)?.clone()))
            })
            .collect();
        Description {
            components,
            edges: edges(&self.edges),
            feedback: edges(&self.feedback),
            clock_s: self.clock_s,
            reconfigure_on_retune: self.reconfigure_on_retune,
            outputs: self.outputs.iter().map(out_name).collect(),
            aliases: self
                .aliases
                .iter()
                .map(|(k, (target, _, _))| (k.clone(), target.clone()))
                .collect(),
            command_aliases: self.command_aliases.clone(),
            fixed,
            status: self.status.map(|i| self.comps[i].id.clone()),
        }
    }

    /// Marks the edge from component `from` to component `to` as a stage boundary: `stage` begins at the input of `to`
    /// ([`GraphBuilder::stage`](crate::GraphBuilder::stage)).
    pub fn stage(&mut self, from: ComponentHandle, to: ComponentHandle, stage: crate::Stage) {
        self.marks.push(crate::graph::StageMark {
            from: from.index().unwrap_or(usize::MAX),
            to: to.index().unwrap_or(usize::MAX),
            stage,
        });
    }

    /// Names the first stage of the graph.
    pub fn first_stage(&mut self, name: &str) {
        self.first_stage = Some(name.to_owned());
    }

    /// Sets the recorded cost of each stage, by the name of the stage.
    pub fn costs(&mut self, costs: Vec<(String, f64)>) {
        self.costs = costs;
    }

    /// Makes the graph. It checks what needs the whole graph: every required input connected, no cycle, the kinds that depend
    /// on other connections, and the kinds of the graph outputs. Errors name the components and ports involved.
    pub fn build(self) -> Result<Graph, GraphError> {
        let catalog = self.catalog();
        assemble(Assembly {
            catalog,
            comps: self
                .comps
                .into_iter()
                .map(|c| (c.id, c.info, c.comp))
                .collect(),
            edges: self.edges,
            feedback: self.feedback,
            outputs: self.outputs,
            alias_targets: self.aliases.values().map(|&(_, i, id)| (i, id)).collect(),
            aliases: self
                .aliases
                .into_iter()
                .map(|(k, (t, _, _))| (k, t))
                .collect(),
            command_aliases: self.command_aliases,
            fixed: self.fixed,
            fixed_targets: self.fixed_targets,
            status: self.status,
            clock_s: self.clock_s,
            reconfigure_on_retune: self.reconfigure_on_retune,
            declared: self.declared,
            marks: self.marks,
            first_stage: self.first_stage,
            costs: self.costs,
        })
    }
}

fn split(end: &str) -> (&str, Option<&str>) {
    match end.split_once('.') {
        Some((c, p)) => (c, Some(p)),
        None => (end, None),
    }
}

fn port_index(
    component: &str,
    side: Side,
    ports: &[crate::component::Port],
    port: Option<&str>,
) -> Result<usize, GraphError> {
    let available = || port_names(ports);
    match port {
        Some(p) => ports
            .iter()
            .position(|q| q.name == p)
            .ok_or_else(|| GraphError::UnknownPort {
                component: component.into(),
                side,
                port: p.into(),
                available: available(),
            }),
        None if ports.len() == 1 => Ok(0),
        None if ports.is_empty() => Err(GraphError::UnknownPort {
            component: component.into(),
            side,
            port: String::new(),
            available: available(),
        }),
        None => Err(GraphError::AmbiguousPort {
            component: component.into(),
            side,
            available: available(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_step_is_checked_at_once() {
        let r = Registry::standard();
        let mut p = GraphPlan::new();
        assert!(matches!(
            p.add(&r, "x", "no_such_kind"),
            Err(GraphError::UnknownType(_))
        ));
        p.add(&r, "chan", "dsp.channel").unwrap();
        assert!(matches!(
            p.add(&r, "chan", "dsp.channel"),
            Err(GraphError::DuplicateId(_))
        ));
        assert!(matches!(
            p.add(&r, "a.b", "channel"),
            Err(GraphError::BadId(_))
        ));
        assert!(matches!(
            p.set("chan", "no_such_key", &Value::Hz(1.0)),
            Err(GraphError::Setting { .. })
        ));
        assert!(matches!(
            p.set("chan", "rate", &Value::Text("fast".into())),
            Err(GraphError::NamedSetting { .. })
        ));
        // The text form of a number is a wrong kind, and a value out of range is refused, not clamped.
        assert!(p.set("chan", "rate", &Value::Text("24000".into())).is_err());
        assert!(p.set("chan", "rate", &Value::Hz(-5.0)).is_err());
        let demod = p.add(&r, "demod", "dsp.am_demod").unwrap();
        let audio = p.add(&r, "audio", "dsp.audio_resampler").unwrap();
        let chan = p.handle("chan").unwrap();
        let (chan_in, chan_out) = (PortRef::new(chan, 0), PortRef::new(chan, 0));
        p.input(chan_in).unwrap();
        // The resampler takes audio or real samples. IQ to real is a demodulator, never a conversion.
        assert!(matches!(
            p.connect(chan_out, PortRef::new(audio, 0)),
            Err(ConnectError::NoConversion { .. })
        ));
        p.connect(chan_out, PortRef::new(demod, 0)).unwrap();
        assert_eq!(
            p.connect(PortRef::INPUT, chan_in),
            Err(ConnectError::PortTaken { to: chan_in })
        );
        assert!(matches!(
            p.output_port("chan.nope"),
            Err(GraphError::UnknownPort { .. })
        ));
        p.connect(PortRef::new(demod, 0), PortRef::new(audio, 0))
            .unwrap();
        p.output(PortRef::new(audio, 0)).unwrap();
        p.alias("width", "chan.filter").unwrap();
        assert!(p.alias("x", "chan.nope").is_err());
        let g = p.build().unwrap();
        assert_eq!(g.order(), ["chan", "demod", "audio"]);
    }

    #[test]
    fn a_missing_input_is_found_at_build() {
        let r = Registry::standard();
        let mut p = GraphPlan::new();
        p.add(&r, "demod", "dsp.am_demod").unwrap();
        assert!(matches!(p.build(), Err(GraphError::Unconnected(_))));
    }

    #[test]
    fn a_plan_described_makes_the_same_plan_again() {
        let r = Registry::standard();
        let mut p = GraphPlan::new();
        let chan = p.add(&r, "chan", "dsp.channel").unwrap();
        let demod = p.add(&r, "demod", "dsp.am_demod").unwrap();
        p.set("chan", "rate", &Value::Hz(24e3)).unwrap();
        p.input(PortRef::new(chan, 0)).unwrap();
        p.connect(PortRef::new(chan, 0), PortRef::new(demod, 0))
            .unwrap();
        p.alias("width", "chan.filter").unwrap();
        p.fix("chan.offset", &Value::Hz(1e3)).unwrap();
        let d = p.describe();
        assert_eq!(d.edges[0].0, "input");
        assert_eq!(d.plan(&r).unwrap().describe(), d);
        assert_eq!(p.build().unwrap().describe(), d);
    }
}
