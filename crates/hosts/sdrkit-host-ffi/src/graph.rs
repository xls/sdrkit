//! Graphs that the caller builds from component types: add, set, ports, connect, validate. `sdrkit_graph_start` is with the
//! lifecycle calls.

use crate::error::{stream_type, Error, SDRKIT_STREAM_NONE};
use crate::handle::{insert, run, str_in, Entry, Handle, Host, Out, What};
use crate::value::{count, small, Id, Str, Variant};
use sdrkit_core::SettingId;
use sdrkit_graph::component::KindRule;
use sdrkit_graph::{
    ComponentHandle, ComponentInfo, GraphError, GraphPlan, PortRef, ValidateReport,
};
use sdrkit_host_engine::EngineError;
use std::ffi::c_char;
use std::sync::{Arc, Mutex};

/// One port of a component.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PortInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The name of the port.
    pub name: Str,
    /// True for an output, false for an input.
    pub output: bool,
    /// The `SDRKIT_STREAM_*` type that the port reads or writes, or `SDRKIT_STREAM_NONE` if it is not one fixed type.
    pub r#type: i32,
    /// True for an input that can stay unconnected.
    pub optional: bool,
}

/// The size of the report of `sdrkit_graph_validate`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ReportInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The number of ports that carry a stream, in stream order.
    pub ports: u32,
    /// The number of adapters that the graph inserted.
    pub adapters: u32,
}

/// One port of the report, with its resolved format.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ReportPort {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// `<component>.<port>`, or `input`.
    pub name: Str,
    /// The `SDRKIT_STREAM_*` type.
    pub r#type: i32,
    /// Items per second.
    pub rate_hz: f64,
    /// Samples per audio frame.
    pub channels: u16,
    /// The radio frequency at 0 Hz of the stream.
    pub centre_hz: f64,
}

/// One adapter that the graph inserted.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ReportAdapter {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The name of the adapter in the graph.
    pub name: Str,
    /// Its component type.
    pub r#type: Str,
    /// The port that it reads.
    pub source: Str,
    /// The port that it writes.
    pub output: Str,
    /// Its group delay in seconds.
    pub group_delay_s: f64,
}

/// The graph of a graph handle: the plan that the caller builds, the description of each component, and the last report.
#[derive(Default)]
pub(crate) struct Plan {
    live: GraphPlan,
    /// The description of each component, by component id.
    comps: Vec<ComponentInfo>,
    report: ValidateReport,
}

fn graph_err(e: GraphError) -> Error {
    Error::Engine(EngineError::Graph(e))
}

/// A new plan of the graph of `e`, made again from its description with the registry of the engine. The graph handle keeps
/// its own plan, so it can start again.
pub(crate) fn plan_of(e: &Entry) -> Result<GraphPlan, Error> {
    if e.what != What::Graph {
        return Err(Error::WrongHandle);
    }
    let d = e.state().graph.live.describe();
    d.plan(e.host.engine.registry()).map_err(graph_err)
}

/// Runs `f` on the graph of the graph handle `h`.
fn with_graph(h: Handle, f: impl FnOnce(&Host, &mut Plan) -> Result<(), Error>) -> i32 {
    run(h, |e| {
        if e.what != What::Graph {
            return Err(Error::WrongHandle);
        }
        f(&e.host, &mut e.state().graph)
    })
}

/// The description of component `name` of `p`.
fn comp<'a>(p: &'a Plan, name: &str) -> Result<&'a ComponentInfo, Error> {
    let h = p.live.handle(name).map_err(graph_err)?;
    h.index()
        .and_then(|i| p.comps.get(i))
        .ok_or(Error::NoSuchId)
}

/// The inputs then the outputs of `info`, each with true for an output.
fn ports(info: &ComponentInfo) -> impl Iterator<Item = (&sdrkit_graph::component::Port, bool)> {
    info.inputs
        .iter()
        .map(|p| (p, false))
        .chain(info.outputs.iter().map(|p| (p, true)))
}

/// `"<component>.<port>"` of port `r` (an output if `output`), or `"input"` for the input of the graph.
fn end_name(p: &Plan, r: PortRef, output: bool) -> String {
    let Some(i) = r.component.index() else {
        return "input".to_owned();
    };
    let port = p.comps.get(i).and_then(|c| {
        if output {
            c.outputs.get(r.port)
        } else {
            c.inputs.get(r.port)
        }
    });
    match (p.live.ids().nth(i), port) {
        (Some(c), Some(port)) => format!("{c}.{}", port.name),
        (Some(c), None) => c.to_owned(),
        (None, _) => String::new(),
    }
}

/// Makes a new, empty graph of the engine of `handle` and writes its handle to `graph`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_new(handle: Handle, graph: *mut Handle) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(graph) };
    run(handle, |e| {
        out?.put(insert(Entry {
            host: Arc::clone(&e.host),
            what: What::Graph,
            state: Mutex::default(),
            audio_in: None,
        }));
        Ok(())
    })
}

/// Adds a component of the type `component_type` with the name `name` (letters, digits, `_` and `-`) and its default
/// settings. The settings of the component are those of the object `component.<component_type>`. Returns
/// `SDRKIT_E_NO_SUCH_KEY` for an unknown type, `SDRKIT_E_GRAPH` for a bad or used name.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_add(
    graph: Handle,
    name: *const c_char,
    component_type: *const c_char,
) -> i32 {
    // SAFETY: the caller's contract.
    let (name, t) = unsafe { (str_in(name), str_in(component_type)) };
    with_graph(graph, |host, p| {
        let (name, t) = (name?, t?);
        let registry = host.engine.registry();
        let comp = registry.make(t).map_err(graph_err)?;
        let info = comp.info();
        p.live.set_catalog(registry.catalog());
        p.live.add_component(name, comp).map_err(graph_err)?;
        p.comps.push(info);
        Ok(())
    })
}

/// Sets setting `id` of component `component` (its name) to `value` before the graph starts. Returns
/// `SDRKIT_E_NO_SUCH_KEY` for an unknown component, `SDRKIT_E_NO_SUCH_ID` for an unknown setting, or a setting code.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_set(
    graph: Handle,
    component: *const c_char,
    id: Id,
    value: *const Variant,
) -> i32 {
    // SAFETY: the caller's contract.
    let (name, value) = unsafe { (str_in(component), value.as_ref().ok_or(Error::Null)) };
    with_graph(graph, |_, p| {
        let name = name?;
        // SAFETY: the caller's contract on the views of `value`.
        let value = unsafe { value?.to_value()? };
        let key = comp(p, name)?
            .settings
            .setting(SettingId(small(id)?))
            .ok_or(Error::NoSuchId)?
            .field
            .key
            .to_string();
        p.live.set(name, &key, &value).map_err(graph_err)
    })
}

/// Writes the number of ports of component `component` (its name) to `count`: its inputs, then its outputs. Returns
/// `SDRKIT_E_NO_SUCH_KEY` for an unknown component.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_ports(
    graph: Handle,
    component: *const c_char,
    count: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (name, out) = unsafe { (str_in(component), Out::new(count)) };
    with_graph(graph, |_, p| {
        let n = ports(comp(p, name?)?).count();
        out?.put(self::count(n));
        Ok(())
    })
}

/// Fills `port` with port `n` of component `component` (its name). The inputs come first, then the outputs. Returns
/// `SDRKIT_E_NO_SUCH_KEY` for an unknown component, `SDRKIT_E_NO_SUCH_ID` if the port does not exist.
/// The strings of `port` are views that the library owns, valid until the next call on `graph`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_port(
    graph: Handle,
    component: *const c_char,
    n: u32,
    port: *mut PortInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let (name, out) = unsafe { (str_in(component), Out::new(port)) };
    with_graph(graph, |_, p| {
        let info = comp(p, name?)?;
        let (port, output) = usize::try_from(n)
            .ok()
            .and_then(|n| ports(info).nth(n))
            .ok_or(Error::NoSuchId)?;
        out?.put(PortInfo {
            size: 0,
            name: Str::of(&port.name),
            output,
            r#type: match &port.format.kind {
                KindRule::Is(k) => stream_type(*k),
                _ => SDRKIT_STREAM_NONE,
            },
            optional: port.optional.is_some(),
        });
        Ok(())
    })
}

/// Connects the output `from` to the input `to`. Each is `"<component>.<port>"`, or `"<component>"` for a component with
/// one port on that side. `from` is `"input"` for the input of the graph. Returns `SDRKIT_E_NO_SUCH_KEY` for an unknown
/// component or port, or `SDRKIT_E_CONNECT` with the components, the ports and the stream types in the error detail.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_connect(
    graph: Handle,
    from: *const c_char,
    to: *const c_char,
) -> i32 {
    // SAFETY: the caller's contract.
    let (from, to) = unsafe { (str_in(from), str_in(to)) };
    with_graph(graph, |_, p| {
        let f = p.live.output_port(from?).map_err(graph_err)?;
        let t = p.live.input_port(to?).map_err(graph_err)?;
        let linked = if f.component == ComponentHandle::INPUT {
            p.live.input(t)
        } else {
            p.live.connect(f, t)
        };
        linked.map_err(|error| {
            graph_err(GraphError::Connect {
                from: end_name(p, f, true),
                to: end_name(p, t, false),
                error: Box::new(error),
            })
        })
    })
}

/// Builds the graph for complex samples at `rate_hz` around `centre_hz`, resolves the format of every port, inserts the
/// adapters that it needs, and fills `report` with the size of the report. Returns `SDRKIT_E_GRAPH`, `SDRKIT_E_CONNECT` or
/// `SDRKIT_E_CONFIG` if the graph does not build.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_validate(
    graph: Handle,
    rate_hz: f64,
    centre_hz: f64,
    report: *mut ReportInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(report) };
    with_graph(graph, |host, p| {
        let out = out?;
        p.report = host.engine.validate_graph(&p.live, rate_hz, centre_hz)?;
        out.put(ReportInfo {
            size: 0,
            ports: count(p.report.ports.len()),
            adapters: count(p.report.adapters.len()),
        });
        Ok(())
    })
}

/// Fills `port` with port `n` of the last report. Returns `SDRKIT_E_NO_SUCH_ID` if the port does not exist.
/// The strings of `port` are views that the library owns, valid until the next call on `graph`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_report_port(
    graph: Handle,
    n: u32,
    port: *mut ReportPort,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(port) };
    with_graph(graph, |_, p| {
        let out = out?;
        let (name, f) = usize::try_from(n)
            .ok()
            .and_then(|i| p.report.ports.get(i))
            .ok_or(Error::NoSuchId)?;
        out.put(ReportPort {
            size: 0,
            name: Str::of(name),
            r#type: stream_type(f.kind),
            rate_hz: f.rate,
            channels: f.channels,
            centre_hz: f.centre_hz,
        });
        Ok(())
    })
}

/// Fills `adapter` with adapter `n` of the last report. Returns `SDRKIT_E_NO_SUCH_ID` if the adapter does not exist.
/// The strings of `adapter` are views that the library owns, valid until the next call on `graph`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_report_adapter(
    graph: Handle,
    n: u32,
    adapter: *mut ReportAdapter,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(adapter) };
    with_graph(graph, |_, p| {
        let out = out?;
        let a = usize::try_from(n)
            .ok()
            .and_then(|i| p.report.adapters.get(i))
            .ok_or(Error::NoSuchId)?;
        out.put(ReportAdapter {
            size: 0,
            name: Str::of(&a.id),
            r#type: Str::of(a.kind),
            source: Str::of(&a.source),
            output: Str::of(&a.output),
            group_delay_s: a.group_delay_s,
        });
        Ok(())
    })
}

crate::handle::sized_structs!(PortInfo, ReportInfo, ReportPort, ReportAdapter);
