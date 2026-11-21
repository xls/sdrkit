//! The stages of a stack and its load figures. A stack declares where its receive chain may be divided and what each part
//! costs. The engine reads the declaration before a session starts, gives a stage a thread of its own when the stack needs
//! more than one core, and reads the measured load while the session runs. A stack never starts a thread.

/// One stage of a stack: a run of components that the engine may place on a thread of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct StageDecl {
    /// The name of the stage, unique in its stack.
    pub name: String,
    /// The usual cost of the stage in calibration units per second of signal (`sdrkit_graph::calibrate`), from the recorded
    /// cost file of the family. `None` when no cost was recorded. Divided by the units that one core of a device runs
    /// per second, it is the share of a core that the stage needs on that device.
    pub cost: Option<f64>,
    /// The most bytes of units that may wait before the stage when it has a thread of its own. This is the read-ahead
    /// bound of the stage.
    pub ahead_bytes: usize,
}

/// The load of one stage, as counters since the session was configured. `clone_from` keeps the buffer of the name.
#[derive(Debug, Default, PartialEq)]
pub struct StageLoad {
    /// The name of the stage.
    pub name: String,
    /// Nanoseconds that the components of the stage worked.
    pub busy_ns: u64,
    /// Seconds of input signal that the stage has worked on.
    pub signal_s: f64,
    /// The stage has a thread of its own now.
    pub cut: bool,
    /// The number of the thread that the stage runs on: 0 is the session thread.
    pub thread: u32,
    /// The bytes that wait before the stage now, and the bound.
    pub queue_bytes: usize,
    pub queue_cap_bytes: usize,
    /// The units that wait before the stage now, and the most that waited.
    pub queue_units: usize,
    pub queue_units_max: usize,
    /// The times that the stage before found the queue full and waited.
    pub behind: u64,
}

impl Clone for StageLoad {
    fn clone(&self) -> Self {
        let mut c = StageLoad::default();
        c.clone_from(self);
        c
    }
    fn clone_from(&mut self, s: &Self) {
        self.name.clone_from(&s.name);
        self.busy_ns = s.busy_ns;
        self.signal_s = s.signal_s;
        self.cut = s.cut;
        self.thread = s.thread;
        self.queue_bytes = s.queue_bytes;
        self.queue_cap_bytes = s.queue_cap_bytes;
        self.queue_units = s.queue_units;
        self.queue_units_max = s.queue_units_max;
        self.behind = s.behind;
    }
}

impl Clone for ComponentLoad {
    fn clone(&self) -> Self {
        let mut c = ComponentLoad::default();
        c.clone_from(self);
        c
    }
    fn clone_from(&mut self, s: &Self) {
        self.component.clone_from(&s.component);
        self.section = s.section;
        self.stage = s.stage;
        self.busy_ns = s.busy_ns;
        self.worst_ns = s.worst_ns;
        self.calls = s.calls;
        self.items_in = s.items_in;
        self.items_out = s.items_out;
    }
}

impl StageLoad {
    /// The fill of the queue before the stage, from 0 to 1.
    pub fn queue_fill(&self) -> f64 {
        if self.queue_cap_bytes == 0 {
            0.0
        } else {
            (self.queue_bytes as f64 / self.queue_cap_bytes as f64).min(1.0)
        }
    }
}

/// The work of one component, or of one named section inside a component, as counters since the session was configured.
/// `clone_from` keeps the buffer of the id.
#[derive(Debug, Default, PartialEq)]
pub struct ComponentLoad {
    /// The id of the component in its graph.
    pub component: String,
    /// The name of the section. Empty for the row of the whole component.
    pub section: &'static str,
    /// The index of the stage that the component belongs to.
    pub stage: usize,
    /// Nanoseconds of work.
    pub busy_ns: u64,
    /// The longest single call, nanoseconds.
    pub worst_ns: u64,
    /// The calls.
    pub calls: u64,
    /// The items read from the inputs and written to the outputs. 0 for a section.
    pub items_in: u64,
    pub items_out: u64,
}

/// The traffic of one connection of a receive chain: an output port of one component to an input port of the next, as
/// counters since the session was configured. The fill and the waits are sampled. `clone_from` keeps the buffers of
/// the texts.
#[derive(Debug, Default, PartialEq)]
pub struct ConnectionLoad {
    /// The component that writes, and its output port.
    pub from: String,
    pub output: String,
    /// The component that reads, and its input port.
    pub to: String,
    pub input: String,
    /// The stream format, as text.
    pub format: String,
    /// The items and their bytes that crossed.
    pub items: u64,
    pub bytes: u64,
    /// The bytes that the buffer or queue holds now, the sum and count of the samples of that fill, and the most.
    pub held_bytes: u64,
    pub held_sum: u64,
    pub held_samples: u64,
    pub held_max: u64,
    /// The bound of the buffer or queue in bytes. 0 when it has no fixed bound.
    pub capacity_bytes: u64,
    /// The sampled waits of an item, from the end of the call that wrote it to the start of the call that read it.
    pub wait_ns_sum: u64,
    pub wait_samples: u64,
    pub wait_worst_ns: u64,
    /// The times that the writer found the buffer or queue full, and that the reader found it empty.
    pub full: u64,
    pub empty: u64,
    /// The connection joins two stages, and the stages run on different threads now.
    pub boundary: bool,
    pub cut: bool,
    /// The index of the stage of the reader.
    pub stage: usize,
}

impl Clone for ConnectionLoad {
    fn clone(&self) -> Self {
        let mut c = ConnectionLoad::default();
        c.clone_from(self);
        c
    }
    fn clone_from(&mut self, s: &Self) {
        self.from.clone_from(&s.from);
        self.output.clone_from(&s.output);
        self.to.clone_from(&s.to);
        self.input.clone_from(&s.input);
        self.format.clone_from(&s.format);
        (self.items, self.bytes) = (s.items, s.bytes);
        (
            self.held_bytes,
            self.held_sum,
            self.held_samples,
            self.held_max,
        ) = (s.held_bytes, s.held_sum, s.held_samples, s.held_max);
        self.capacity_bytes = s.capacity_bytes;
        (self.wait_ns_sum, self.wait_samples, self.wait_worst_ns) =
            (s.wait_ns_sum, s.wait_samples, s.wait_worst_ns);
        (self.full, self.empty, self.boundary, self.cut, self.stage) =
            (s.full, s.empty, s.boundary, s.cut, s.stage);
    }
}

/// The body of one stage thread. The engine calls [`StageRun::run`] on a thread that it owns. The call returns when the
/// stack ends the placement that made it.
pub trait StageRun: Send {
    /// Runs the stage until its placement ends.
    fn run(self: Box<Self>);
    /// The index of the first stage that this thread runs.
    fn stage(&self) -> usize;
}

/// The identity of one declaration of stages. Each [`StageSetId::fresh`] is unique in the process, so two receive chains
/// never share one. The default stands for no stages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StageSetId(u64);

impl StageSetId {
    /// An identity that no other call in the process returned. Never equal to the default.
    pub fn fresh() -> StageSetId {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        StageSetId(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// A receive chain that the engine may divide into stages. [`Receiver::staged`](crate::Receiver::staged) returns it.
pub trait Staged {
    /// The stages in run order. One entry for a receive chain that has no stage boundary.
    fn stages(&self) -> Vec<StageDecl>;
    /// The identity of what [`Staged::stages`] returns now. It changes when the names, the count or the declared cost of
    /// the stages change, and at no other time. The engine compares it at each look at the load: a change ends the
    /// placement and places the new stages at once by their declared cost. Before the stages change, their placement
    /// ends: the work that waits at the cuts is finished and every stage thread returns.
    fn stage_set(&self) -> StageSetId;
    /// The boundaries that are cut now: bit `i` is the boundary before stage `i + 1`.
    fn cuts(&self) -> u32;
    /// Cuts at the boundaries of `mask` and at no other. Work that waits at a boundary is finished first, so no unit is
    /// lost and the output is the same for every mask. Returns one body per cut, in stage order; the engine runs each on
    /// a thread. Bodies of an earlier placement return.
    fn cut(&mut self, mask: u32) -> Vec<Box<dyn StageRun>>;
    /// Takes every component back to the calling thread and finishes the work that waits at the cuts. The next block
    /// that is fed places the stages again. Call it before a read that needs every component, such as a saved state.
    fn gather(&mut self);
    /// Moves the output that the stage threads have finished since the last block into `out`. It does not wait.
    fn poll(&mut self, out: &mut crate::Output);
    /// [`Staged::gather`], and moves the output of the finished work into `out`.
    fn drain(&mut self, out: &mut crate::Output);
    /// The load of each stage, in run order. A row that `out` holds already is written in place.
    fn stage_loads(&self, out: &mut Vec<StageLoad>);
    /// The work of each component in run order, each followed by its sections.
    fn component_loads(&self, out: &mut Vec<ComponentLoad>);
    /// The traffic of each connection between two components, in the run order of the readers. A row that `out` holds
    /// already is written in place. None by default.
    fn connection_loads(&self, out: &mut Vec<ConnectionLoad>) {
        out.clear();
    }
}

/// The recorded costs of the stacks of one family. The text has one line per stage:
/// `<stack> <variant> <stage> <calibration units per second of signal>`. A line that starts with `#` is a comment. The variant is the submode
/// or the setting that changes the cost, or `-`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CostTable {
    rows: Vec<CostRow>,
}

/// One line of a [`CostTable`].
#[derive(Clone, Debug, PartialEq)]
pub struct CostRow {
    pub stack: String,
    pub variant: String,
    pub stage: String,
    /// The calibration units per second of signal.
    pub units: f64,
}

impl CostTable {
    /// Reads the text of a cost file. A line that does not fit is left out.
    pub fn parse(text: &str) -> CostTable {
        let rows = text
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .filter_map(|l| {
                let mut w = l.split_whitespace();
                let (stack, variant, stage, units) = (w.next()?, w.next()?, w.next()?, w.next()?);
                Some(CostRow {
                    stack: stack.to_owned(),
                    variant: variant.to_owned(),
                    stage: stage.to_owned(),
                    units: units
                        .parse()
                        .ok()
                        .filter(|s: &f64| s.is_finite() && *s >= 0.0)?,
                })
            })
            .collect();
        CostTable { rows }
    }

    /// A table of `rows`.
    pub fn from_rows(rows: Vec<CostRow>) -> CostTable {
        CostTable { rows }
    }

    /// The rows.
    pub fn rows(&self) -> &[CostRow] {
        &self.rows
    }

    /// The recorded cost of `stage` of `stack` in `variant`, in calibration units per second of signal.
    pub fn units(&self, stack: &str, variant: &str, stage: &str) -> Option<f64> {
        self.rows
            .iter()
            .find(|r| r.stack == stack && r.variant == variant && r.stage == stage)
            .map(|r| r.units)
    }

    /// The text of the table, as [`CostTable::parse`] reads it.
    pub fn render(&self) -> String {
        let mut s = String::from("# stack variant stage calibration-units-per-second-of-signal (written by the cost test, do not edit)\n");
        for r in &self.rows {
            s.push_str(&format!(
                "{} {} {} {:.1}\n",
                r.stack, r.variant, r.stage, r.units
            ));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cost_table_reads_what_it_writes() {
        let t = CostTable::parse("# c\nfm - front 312.5\nfm - audio 10.0\nbad line\n");
        assert_eq!(t.rows().len(), 2);
        assert_eq!(t.units("fm", "-", "front"), Some(312.5));
        assert_eq!(t.units("fm", "-", "none"), None);
        assert_eq!(CostTable::parse(&t.render()), t);
    }
}
