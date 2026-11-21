//! The threads of a session. The engine reads the stages that a stack declares and their recorded cost, gives a stage of
//! a heavy stack a thread of its own, measures the load while the session runs, and cuts at one more boundary or merges
//! two stages when the load asks for it. A stack starts no thread.

use crate::feed::QueueMeter;
use sdrkit_core::{
    ComponentLoad, ConnectionLoad, Receiver, StageDecl, StageLoad, StageRun, StageSetId, Staged,
};
use sdrkit_graph::Pool;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The share of one core that the stages of one thread may cost together when the engine places them.
const FILL: f64 = 0.8;
/// A stack whose declared cost is below this stays on one thread.
const HEAVY: f64 = 0.7;
/// The time between two looks at the load.
const LOOK: Duration = Duration::from_millis(250);
/// A thread is hot when it works this share of the time, or when the queue before it is this full.
const HOT_SHARE: f64 = 0.85;
const HOT_FILL: f64 = 0.75;
/// A thread that stays hot this long gets a cut, seconds.
const HOT_S: f64 = 0.5;
/// Two neighbouring threads that together stay below this share for [`CALM_S`] seconds become one.
const CALM_SHARE: f64 = 0.4;
const CALM_S: f64 = 5.0;
/// The least time between two changes of the placement.
const SETTLE: Duration = Duration::from_millis(1000);

/// The processor cores of the engine: how many the sessions may use, the pool for loops inside a component, and what
/// the running sessions declared.
pub(crate) struct Cores {
    /// The setting `cores_max`. 0 takes the performance cores of the machine.
    max: AtomicUsize,
    pub(crate) pool: Pool,
    /// The declared cost of the running sessions, in thousandths of a core.
    declared: AtomicU64,
    /// The priority of the stage threads and of the pool, as the host set it ([`Priority`]).
    priority: Arc<AtomicUsize>,
}

/// How the engine ranks its threads against the other work of the machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Priority {
    /// A stage thread of a session with sound or pictures answers the user at once; pool threads are one class lower.
    #[default]
    High,
    /// One class lower for every thread.
    Normal,
    /// Background work.
    Low,
}

impl Cores {
    pub(crate) fn new() -> Cores {
        let machine = machine_cores();
        let priority = Arc::new(AtomicUsize::new(0));
        let p = Arc::clone(&priority);
        // A pool thread runs one class below a stage thread.
        let pool = Pool::with_start(machine.saturating_sub(1).max(1), move || {
            platform::set_class(priority_of(p.load(Ordering::Relaxed)), true, false);
        });
        pool.set_workers(machine.saturating_sub(1));
        Cores {
            max: AtomicUsize::new(0),
            pool,
            declared: AtomicU64::new(0),
            priority,
        }
    }

    /// The cores that the sessions may use.
    pub(crate) fn max(&self) -> usize {
        match self.max.load(Ordering::Relaxed) {
            0 => machine_cores(),
            n => n,
        }
    }

    /// Sets the setting `cores_max` and the size of the pool that follows from it.
    pub(crate) fn set_max(&self, max: usize) {
        self.max.store(max, Ordering::Relaxed);
        self.pool.set_workers(self.max().saturating_sub(1));
    }

    /// Sets the priority of the threads that start from now on.
    #[allow(dead_code)]
    pub(crate) fn set_priority(&self, p: Priority) {
        self.priority.store(p as usize, Ordering::Relaxed);
    }

    fn priority(&self) -> Priority {
        priority_of(self.priority.load(Ordering::Relaxed))
    }

    /// The cores that the other sessions leave, by what they declared.
    fn free(&self, own: f64) -> f64 {
        let all = self.declared.load(Ordering::Relaxed) as f64 / 1000.0;
        (self.max() as f64 - (all - own).max(0.0)).max(0.0)
    }
}

fn priority_of(n: usize) -> Priority {
    match n {
        0 => Priority::High,
        1 => Priority::Normal,
        _ => Priority::Low,
    }
}

/// What this device delivers, measured once for the process with the calibration workload
/// ([`sdrkit_graph::calibrate`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Device {
    /// The calibration units per second of one fast core, with the instruction set of this build.
    pub fast_units: f64,
    /// The same of one slow core. Equal to `fast_units` where the platform has one class of cores, or does not tell.
    pub slow_units: f64,
    /// The fast cores.
    pub fast_cores: usize,
    /// The slow cores. 0 where the platform has one class of cores, or does not tell.
    pub slow_cores: usize,
}

static DEVICE: std::sync::OnceLock<Device> = std::sync::OnceLock::new();

/// The time that the calibration of one class of cores takes.
const CALIBRATION: Duration = Duration::from_millis(15);

/// The calibration of this device. The first call measures for about 30 ms on the calling thread; the engine makes it
/// on a thread of its own when it starts. Later calls return the same numbers.
pub fn device() -> Device {
    *DEVICE.get_or_init(|| {
        let own = std::thread::Builder::new()
            .name("sdrkit-calibrate".into())
            .spawn(|| {
                platform::set_class(Priority::High, true, true);
                let fast = sdrkit_graph::calibrate::units_per_second(CALIBRATION);
                let slow = platform::slow_cores().map(|_| {
                    // The background class runs on the slow cores.
                    platform::set_background();
                    sdrkit_graph::calibrate::units_per_second(CALIBRATION)
                });
                (fast, slow)
            });
        let (fast, slow) = own
            .ok()
            .and_then(|t| t.join().ok())
            .unwrap_or_else(|| (sdrkit_graph::calibrate::units_per_second(CALIBRATION), None));
        Device {
            fast_units: fast,
            slow_units: slow.map_or(fast, |s| s.min(fast)),
            fast_cores: machine_cores(),
            slow_cores: platform::slow_cores().unwrap_or(0),
        }
    })
}

/// The calibration, if it has finished.
pub(crate) fn device_if_ready() -> Option<Device> {
    DEVICE.get().copied()
}

/// What a stack needs on this device, by its recorded cost.
#[derive(Clone, Debug, PartialEq)]
pub struct StackNeed {
    /// The stages in run order: the name, and the recorded cost in calibration units per second of signal. `None` for a
    /// stage with no recorded cost.
    pub stages: Vec<(String, Option<f64>)>,
    /// The recorded cost of all stages, in calibration units per second of signal.
    pub units: f64,
    /// The fast cores of this device that the stack needs: `units` divided by [`Device::fast_units`].
    pub cores: f64,
    /// The cores that the running sessions leave of the setting `cores_max`.
    pub cores_free: f64,
    /// The threads that the engine would give the stack now.
    pub threads: u32,
    /// The stack keeps up with its signal on this device: its largest stage fits one core, and the stack fits the
    /// free cores.
    pub real_time: bool,
    /// The calibration of this device.
    pub device: Device,
}

impl Cores {
    /// What the stages `stages` need on the device `device`.
    pub(crate) fn need(&self, stages: &[StageDecl], device: Device) -> StackNeed {
        let costs: Vec<f64> = stages
            .iter()
            .map(|s| s.cost.unwrap_or(0.0) / device.fast_units)
            .collect();
        let cores: f64 = costs.iter().sum();
        let free = self.free(0.0);
        let mask = if cores >= HEAVY {
            plan(&costs, (free.floor() as usize).max(1))
        } else {
            0
        };
        let threads = mask.count_ones() + 1;
        // The work of the fullest thread under this plan.
        let (mut fullest, mut acc) = (0f64, 0.0);
        for (i, c) in costs.iter().enumerate() {
            if i > 0 && mask & (1 << (i - 1)) != 0 {
                acc = 0.0;
            }
            acc += c;
            fullest = fullest.max(acc);
        }
        StackNeed {
            stages: stages.iter().map(|s| (s.name.clone(), s.cost)).collect(),
            units: cores * device.fast_units,
            cores,
            cores_free: free,
            threads,
            real_time: fullest <= 1.0 && cores <= free,
            device,
        }
    }
}

/// The performance cores of the machine.
pub(crate) fn machine_cores() -> usize {
    platform::performance_cores()
        .or_else(|| std::thread::available_parallelism().ok().map(usize::from))
        .unwrap_or(1)
        .max(1)
}

#[cfg(target_vendor = "apple")]
mod platform {
    use super::Priority;

    pub(super) fn performance_cores() -> Option<usize> {
        let mut n: libc::c_int = 0;
        let mut len = std::mem::size_of::<libc::c_int>();
        // SAFETY: the name is a C string, and `n` and `len` are valid for the size that `len` states.
        let r = unsafe {
            libc::sysctlbyname(
                c"hw.perflevel0.logicalcpu".as_ptr(),
                std::ptr::from_mut(&mut n).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        (r == 0 && n > 0).then_some(n as usize)
    }

    /// The efficiency cores, if the machine has them.
    pub(super) fn slow_cores() -> Option<usize> {
        let mut n: libc::c_int = 0;
        let mut len = std::mem::size_of::<libc::c_int>();
        // SAFETY: the name is a C string, and `n` and `len` are valid for the size that `len` states.
        let r = unsafe {
            libc::sysctlbyname(
                c"hw.perflevel1.logicalcpu".as_ptr(),
                std::ptr::from_mut(&mut n).cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        (r == 0 && n > 0).then_some(n as usize)
    }

    /// Puts the calling thread into the background class, which the system runs on the efficiency cores.
    pub(super) fn set_background() {
        // SAFETY: the call changes the class of the calling thread only.
        unsafe {
            libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_BACKGROUND, 0);
        }
    }

    /// Sets the class of the calling thread. `stage` is a stage thread of a session; else a pool thread.
    pub(super) fn set_class(p: Priority, interactive: bool, stage: bool) {
        use libc::qos_class_t::{
            QOS_CLASS_DEFAULT, QOS_CLASS_USER_INITIATED, QOS_CLASS_USER_INTERACTIVE,
            QOS_CLASS_UTILITY,
        };
        let class = match (p, stage, interactive) {
            (Priority::High, true, true) => QOS_CLASS_USER_INTERACTIVE,
            (Priority::High, _, _) | (Priority::Normal, true, true) => QOS_CLASS_USER_INITIATED,
            (Priority::Normal, _, _) => QOS_CLASS_DEFAULT,
            (Priority::Low, _, _) => QOS_CLASS_UTILITY,
        };
        // SAFETY: the call changes the class of the calling thread only.
        unsafe {
            libc::pthread_set_qos_class_self_np(class, 0);
        }
    }
}

#[cfg(not(target_vendor = "apple"))]
mod platform {
    use super::Priority;

    pub(super) fn performance_cores() -> Option<usize> {
        None
    }

    pub(super) fn slow_cores() -> Option<usize> {
        None
    }

    /// Other platforms keep the priority of the thread that started the engine.
    pub(super) fn set_class(_: Priority, _: bool, _: bool) {}

    pub(super) fn set_background() {}
}

/// One row of the table of components: a component, or a section inside one.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct ComponentRow {
    pub(crate) load: ComponentLoad,
    pub(crate) stage: String,
    pub(crate) thread: u32,
    /// The share of one core over the last second and since the start.
    pub(crate) share: f64,
    pub(crate) share_total: f64,
    /// The work since the start in calibration units per second of signal.
    pub(crate) units: f64,
}

/// One row of the table of stages.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct StageRow {
    pub(crate) load: StageLoad,
    pub(crate) declared: Option<f64>,
    /// The share of one core over the last second.
    pub(crate) share: f64,
    /// The cores that the stage needs at the rate of the stack, over the last second.
    pub(crate) cost: f64,
    /// The same in calibration units per second of signal, and the recorded cost in that unit.
    pub(crate) units: f64,
    pub(crate) declared_units: Option<f64>,
}

/// One row of the table of connections. The first row of a session is its queue.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct ConnectionRow {
    pub(crate) load: ConnectionLoad,
    /// The stage of the reader, by name.
    pub(crate) stage: String,
    /// Items and bytes per second over the last second.
    pub(crate) items_per_s: f64,
    pub(crate) bytes_per_s: f64,
    /// Input samples that the session lost at this place.
    pub(crate) lost: u64,
}

impl Clone for ConnectionRow {
    fn clone(&self) -> Self {
        let mut c = ConnectionRow::default();
        c.clone_from(self);
        c
    }
    fn clone_from(&mut self, s: &Self) {
        self.load.clone_from(&s.load);
        self.stage.clone_from(&s.stage);
        (self.items_per_s, self.bytes_per_s, self.lost) = (s.items_per_s, s.bytes_per_s, s.lost);
    }
}

impl Clone for ComponentRow {
    fn clone(&self) -> Self {
        let mut c = ComponentRow::default();
        c.clone_from(self);
        c
    }
    fn clone_from(&mut self, s: &Self) {
        self.load.clone_from(&s.load);
        self.stage.clone_from(&s.stage);
        (self.thread, self.share, self.share_total, self.units) =
            (s.thread, s.share, s.share_total, s.units);
    }
}

impl Clone for StageRow {
    fn clone(&self) -> Self {
        let mut c = StageRow::default();
        c.clone_from(self);
        c
    }
    fn clone_from(&mut self, s: &Self) {
        self.load.clone_from(&s.load);
        (
            self.declared,
            self.share,
            self.cost,
            self.units,
            self.declared_units,
        ) = (s.declared, s.share, s.cost, s.units, s.declared_units);
    }
}

impl Load {
    /// Copies `s` and keeps the buffers of the rows.
    pub(crate) fn copy_from(&mut self, s: &Load) {
        self.components.clone_from(&s.components);
        self.stages.clone_from(&s.stages);
        self.connections.clone_from(&s.connections);
        let (components, stages, connections) = (
            std::mem::take(&mut self.components),
            std::mem::take(&mut self.stages),
            std::mem::take(&mut self.connections),
        );
        *self = Load {
            components,
            stages,
            connections,
            ..*s
        };
    }
}

/// The load of a session, as the session thread last measured it.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Load {
    /// Moves on with each measurement.
    pub(crate) seq: u64,
    pub(crate) components: Vec<ComponentRow>,
    pub(crate) stages: Vec<StageRow>,
    pub(crate) connections: Vec<ConnectionRow>,
    /// The samples missing in the blocks that the session took: gaps that the driver marked.
    pub(crate) gap_samples: u64,
    /// The share of one core that the stack takes now, all threads together.
    pub(crate) share: f64,
    /// The cores that the stack needs at its rate.
    pub(crate) need: f64,
    /// The cores that the stack declared, and the cores that were free for it at its start.
    pub(crate) declared: f64,
    pub(crate) free: f64,
    pub(crate) threads: u32,
    /// The times that a stage found the queue after it full.
    pub(crate) behind: u64,
    /// The places where samples are missing that the stack was told of.
    pub(crate) gaps: u64,
    /// The declared cost is above the free cores.
    pub(crate) short: bool,
    /// The calibration units per second of one fast core of this device.
    pub(crate) core_units: f64,
}

/// The placement of one session's stages on threads, and its measurements. The session thread owns it.
pub(crate) struct Placement {
    cores: Arc<Cores>,
    stages: Vec<StageDecl>,
    /// The identity of `stages`.
    set: StageSetId,
    mask: u32,
    threads: Vec<JoinHandle<()>>,
    /// The session has sound or pictures.
    interactive: bool,
    started: Instant,
    looked: Instant,
    changed: Instant,
    /// The counters at the last look, and at the look a second ago.
    last: Vec<StageLoad>,
    now: Vec<StageLoad>,
    comps: Vec<ComponentLoad>,
    second: Vec<ComponentLoad>,
    second_stages: Vec<StageLoad>,
    second_at: Instant,
    /// Seconds that each thread has been hot, and that each cut has been calm.
    hot: Vec<f64>,
    calm: Vec<f64>,
    dropped: u64,
    shares: Vec<f64>,
    pub(crate) gaps: u64,
    pub(crate) load: Load,
    declared_milli: u64,
    /// The calibration units per second of one fast core of this device.
    core_units: f64,
    /// The stages were placed by their recorded cost.
    planned: bool,
    /// The queue of the session, if it reads blocks.
    pub(crate) queue: Option<Arc<QueueMeter>>,
    conns: Vec<ConnectionLoad>,
    graph_conns: Vec<ConnectionLoad>,
    /// The items and bytes of each connection a second ago.
    second_conns: Vec<(u64, u64)>,
    /// Input samples lost: at the queue, in gaps of the blocks, and while the queue before stage `i` was full.
    lost_queue: u64,
    gap_samples: u64,
    lost_stage: Vec<u64>,
    /// The samples that [`Placement::lose`] has placed.
    attributed: u64,
    probe: Vec<StageLoad>,
    behind_seen: Vec<u64>,
}

impl Placement {
    pub(crate) fn new(cores: Arc<Cores>) -> Placement {
        let now = Instant::now();
        Placement {
            cores,
            stages: Vec::new(),
            set: StageSetId::default(),
            mask: 0,
            threads: Vec::new(),
            interactive: true,
            started: now,
            looked: now,
            changed: now,
            last: Vec::new(),
            now: Vec::new(),
            comps: Vec::new(),
            second: Vec::new(),
            second_stages: Vec::new(),
            second_at: now,
            hot: Vec::new(),
            calm: Vec::new(),
            dropped: 0,
            shares: Vec::new(),
            gaps: 0,
            load: Load::default(),
            declared_milli: 0,
            core_units: 0.0,
            planned: false,
            queue: None,
            conns: Vec::new(),
            graph_conns: Vec::new(),
            second_conns: Vec::new(),
            lost_queue: 0,
            gap_samples: 0,
            lost_stage: Vec::new(),
            attributed: 0,
            probe: Vec::new(),
            behind_seen: Vec::new(),
        }
    }

    /// Counts where the session lost input: `lost` samples that its queue could not take, and `gaps` samples that the
    /// driver marked as missing. A loss while the queue before a stage was full, or its writer waited, counts at the
    /// deepest such stage: that stage held the session up.
    pub(crate) fn lose(&mut self, rx: &mut dyn Receiver, lost: u64, gaps: u64) {
        self.gap_samples += gaps;
        if lost == 0 {
            return;
        }
        self.attributed += lost;
        let mut at = None;
        if let Some(st) = rx.staged().filter(|_| self.mask != 0) {
            st.stage_loads(&mut self.probe);
            self.behind_seen.resize(self.probe.len(), 0);
            for (s, l) in self.probe.iter().enumerate() {
                if !l.cut {
                    continue;
                }
                let waited = std::mem::replace(&mut self.behind_seen[s], l.behind) != l.behind;
                if waited || (l.queue_cap_bytes > 0 && l.queue_bytes >= l.queue_cap_bytes) {
                    at = Some(s);
                }
            }
        }
        match at {
            Some(s) => {
                if self.lost_stage.len() <= s {
                    self.lost_stage.resize(s + 1, 0);
                }
                self.lost_stage[s] += lost;
            }
            None => self.lost_queue += lost,
        }
    }

    /// Reads what `rx` declares and places its stages: a stack whose declared cost is light stays on one thread, and a
    /// heavy one gets cuts so that no thread has more than [`FILL`] of a core, as far as cores are free. The session
    /// starts in any case; [`Load::short`] says that the declared cost is above the free cores.
    pub(crate) fn start(&mut self, rx: &mut dyn Receiver) {
        self.end(rx);
        let now = Instant::now();
        (self.started, self.looked, self.changed, self.second_at) = (now, now, now, now);
        self.last.clear();
        self.second.clear();
        self.second_stages.clear();
        self.load = Load::default();
        self.planned = false;
        self.plan_now(rx);
    }

    /// Places the stages by their recorded cost, once the calibration of the device is there. The session thread does
    /// not wait for the calibration: until then the stack runs on one thread.
    fn plan_now(&mut self, rx: &mut dyn Receiver) {
        let device = device_if_ready();
        let Some(st) = rx.staged() else {
            self.stages.clear();
            self.set = StageSetId::default();
            if let Some(d) = device {
                (self.planned, self.core_units) = (true, d.fast_units);
            }
            return;
        };
        self.set = st.stage_set();
        self.stages = st.stages();
        let Some(device) = device else {
            return;
        };
        self.planned = true;
        // The recorded cost in calibration units, as cores of this device.
        let core_units = device.fast_units;
        let costs: Vec<f64> = self
            .stages
            .iter()
            .map(|s| s.cost.unwrap_or(0.0) / core_units)
            .collect();
        let declared: f64 = costs.iter().sum();
        self.core_units = core_units;
        self.declared_milli = (declared * 1000.0) as u64;
        self.cores
            .declared
            .fetch_add(self.declared_milli, Ordering::Relaxed);
        let free = self.cores.free(declared);
        self.load.declared = declared;
        self.load.free = free;
        self.load.short = declared > free;
        let mask = if declared >= HEAVY {
            plan(&costs, (free.floor() as usize).max(1))
        } else {
            0
        };
        self.place(rx, mask);
    }

    /// Ends the placement: every stage comes back to the session thread, and the stage threads end.
    pub(crate) fn end(&mut self, rx: &mut dyn Receiver) {
        if let Some(st) = rx.staged() {
            drop(st.cut(0));
        }
        self.join();
        self.mask = 0;
        self.cores
            .declared
            .fetch_sub(std::mem::take(&mut self.declared_milli), Ordering::Relaxed);
    }

    /// Measures the load once more, for the last copy of a session that ends. `dropped` is the count of samples that the
    /// session lost before its stack: those after the last block count at the queue.
    pub(crate) fn publish_last(&mut self, rx: &mut dyn Receiver, dropped: u64) {
        self.lost_queue += dropped.saturating_sub(self.attributed);
        self.attributed = self.attributed.max(dropped);
        match rx.staged() {
            Some(st) => {
                st.stage_loads(&mut self.last);
                self.publish(Some(st));
            }
            None => self.publish(None),
        }
    }

    /// Waits for the stage threads of a placement that has ended.
    pub(crate) fn join(&mut self) {
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }

    /// Cuts `rx` at the boundaries of `mask` and starts one thread per cut.
    fn place(&mut self, rx: &mut dyn Receiver, mask: u32) {
        let Some(st) = rx.staged() else { return };
        let bodies = st.cut(mask);
        // The threads of the placement before have their components no more and return.
        self.join();
        let mut failed = false;
        for body in bodies {
            let (p, interactive) = (self.cores.priority(), self.interactive);
            let made = std::thread::Builder::new()
                .name(format!("sdrkit-stage-{}", body.stage()))
                .spawn(move || {
                    platform::set_class(p, interactive, true);
                    run_body(body);
                });
            match made {
                Ok(t) => self.threads.push(t),
                Err(_) => failed = true,
            }
        }
        if failed {
            // Without a thread for every cut the stack runs whole on the session thread.
            drop(st.cut(0));
            self.join();
        }
        self.mask = st.cuts();
        self.changed = Instant::now();
        self.hot.clear();
        self.calm.clear();
    }

    /// Measures the load, at most every [`LOOK`], and changes the placement when the load asks for it. `dropped` is the
    /// count of samples that the session lost before its stack.
    ///
    /// A stack whose stages changed since the last look ([`Staged::stage_set`]), or whose cuts are no longer those of the
    /// placement, has ended that placement itself ([`Receiver::staged`]). The look then joins the stage threads, places
    /// the new stages by their declared cost without waiting for [`SETTLE`], starts the measurements again from the
    /// counters of the new stages, and publishes their names and declared cost at once. The comparison reads one
    /// integer and the mask per look; it allocates nothing.
    pub(crate) fn look(&mut self, rx: &mut dyn Receiver, dropped: u64) {
        let dt = self.looked.elapsed();
        if dt < LOOK {
            return;
        }
        if !self.planned {
            self.plan_now(rx);
        }
        let Some(st) = rx.staged() else {
            if self.set != StageSetId::default() {
                // The stack has no stages any more.
                self.restage(rx);
            }
            // A stack that is no graph has no stages: its queue still has a row.
            if self.second_at.elapsed() >= Duration::from_millis(950) || self.load.seq == 0 {
                self.looked = Instant::now();
                self.now.clear();
                self.last.clear();
                self.publish(None);
            }
            return;
        };
        if st.stage_set() != self.set || st.cuts() != self.mask {
            self.restage(rx);
            let Some(st) = rx.staged() else { return };
            self.looked = Instant::now();
            self.dropped = dropped;
            // The counters of the new stages are the base of the next measurement; the queue keeps its own.
            st.stage_loads(&mut self.last);
            st.component_loads(&mut self.second);
            self.second_stages.clone_from(&self.last);
            st.connection_loads(&mut self.graph_conns);
            let first = usize::from(self.queue.is_some());
            self.second_conns
                .resize(first + self.graph_conns.len(), (0, 0));
            for (b, c) in self.second_conns[first..].iter_mut().zip(&self.graph_conns) {
                *b = (c.items, c.bytes);
            }
            self.publish(Some(st));
            return;
        }
        self.looked = Instant::now();
        st.stage_loads(&mut self.now);
        let lost = dropped > std::mem::replace(&mut self.dropped, dropped);
        let dt_s = dt.as_secs_f64();
        let share = |a: &StageLoad, b: Option<&StageLoad>, s: f64| {
            (a.busy_ns.saturating_sub(b.map_or(0, |b| b.busy_ns))) as f64 / 1e9 / s.max(1e-3)
        };
        let mut shares = std::mem::take(&mut self.shares);
        shares.clear();
        shares.extend(
            self.now
                .iter()
                .enumerate()
                .map(|(i, l)| share(l, self.last.get(i), dt_s)),
        );
        let groups = self.mask.count_ones() as usize + 1;
        self.hot.resize(groups, 0.0);
        self.calm.resize(groups.saturating_sub(1), 0.0);
        let mut group_share = [0.0; 33];
        let mut group_fill = [0.0; 33];
        let groups = groups.min(group_share.len());
        for (i, l) in self.now.iter().enumerate() {
            let g = (l.thread as usize).min(groups - 1);
            group_share[g] += shares[i];
            if l.cut {
                group_fill[g] = l.queue_fill();
            }
        }
        let mut split = None;
        for g in 0..groups {
            let hot = group_share[g] >= HOT_SHARE || group_fill[g] >= HOT_FILL || (g == 0 && lost);
            self.hot[g] = if hot { self.hot[g] + dt_s } else { 0.0 };
            if self.hot[g] >= HOT_S && split.is_none() {
                split = self.split_of(g, &shares);
            }
        }
        for g in 0..groups.saturating_sub(1) {
            let calm = group_share[g] + group_share[g + 1] < CALM_SHARE;
            self.calm[g] = if calm { self.calm[g] + dt_s } else { 0.0 };
        }
        self.shares = shares;
        std::mem::swap(&mut self.last, &mut self.now);
        if self.second_at.elapsed() >= Duration::from_millis(950) || self.load.seq == 0 {
            self.publish(Some(st));
        }
        if self.changed.elapsed() < SETTLE {
            return;
        }
        if let Some(bit) = split {
            if groups < self.cores.max() {
                self.place(rx, self.mask | bit);
            }
        } else if let Some(g) = self.calm.iter().position(|&c| c >= CALM_S) {
            // The cut before thread `g + 1` goes.
            let bit = (0..32)
                .filter(|b| self.mask & (1 << b) != 0)
                .nth(g)
                .map_or(0, |b| 1u32 << b);
            self.place(rx, self.mask & !bit);
        }
    }

    /// Ends the placement of stages that the stack left and places those it has now by their declared cost. The loss
    /// counted before a stage of the old set moves to the queue, so the rows still add up to the loss of the session.
    fn restage(&mut self, rx: &mut dyn Receiver) {
        self.end(rx);
        self.lost_queue += self.lost_stage.drain(..).sum::<u64>();
        self.behind_seen.clear();
        self.hot.clear();
        self.calm.clear();
        self.planned = false;
        self.plan_now(rx);
    }

    /// The boundary inside thread `g` that divides its measured work most evenly, as a bit of the mask.
    fn split_of(&self, g: usize, shares: &[f64]) -> Option<u32> {
        let stages: Vec<usize> = (0..self.now.len().min(shares.len()))
            .filter(|&i| self.now[i].thread as usize == g)
            .collect();
        let total: f64 = stages.iter().map(|&i| shares[i]).sum();
        let mut best: Option<(f64, usize)> = None;
        let mut left = 0.0;
        for w in stages.windows(2) {
            left += shares[w[0]];
            let worst = left.max(total - left);
            if best.is_none_or(|(b, _)| worst < b) {
                best = Some((worst, w[1]));
            }
        }
        best.map(|(_, s)| 1u32 << (s - 1))
    }

    /// Makes the rows of the load tables from the counters now and those of a second ago.
    fn publish(&mut self, st: Option<&mut dyn Staged>) {
        let window = self.second_at.elapsed().as_secs_f64().max(1e-3);
        let total = self.started.elapsed().as_secs_f64().max(1e-3);
        match st {
            Some(st) => {
                st.component_loads(&mut self.comps);
                st.connection_loads(&mut self.graph_conns);
            }
            None => {
                self.comps.clear();
                self.graph_conns.clear();
            }
        }
        let first = usize::from(self.queue.is_some());
        self.conns
            .resize_with(first + self.graph_conns.len(), ConnectionLoad::default);
        if let Some(q) = &self.queue {
            queue_row(q, &mut self.conns[0]);
        }
        for (d, c) in self.conns[first..].iter_mut().zip(&self.graph_conns) {
            d.clone_from(c);
        }
        let stages = &self.last;
        let before = |c: &ComponentLoad| {
            self.second
                .iter()
                .find(|b| b.component == c.component && b.section == c.section)
                .map_or(0, |b| b.busy_ns)
        };
        let load = &mut self.load;
        load.seq += 1;
        load.components
            .resize_with(self.comps.len(), ComponentRow::default);
        for (row, c) in load.components.iter_mut().zip(&self.comps) {
            let stage = stages.get(c.stage);
            row.share = c.busy_ns.saturating_sub(before(c)) as f64 / 1e9 / window;
            row.share_total = c.busy_ns as f64 / 1e9 / total;
            row.units = self.core_units * c.busy_ns as f64
                / 1e9
                / stage.map_or(0.0, |s| s.signal_s).max(1e-9);
            match stage {
                Some(s) => row.stage.clone_from(&s.name),
                None => row.stage.clear(),
            }
            row.thread = stage.map_or(0, |s| s.thread);
            row.load.clone_from(c);
        }
        load.stages.resize_with(stages.len(), StageRow::default);
        (load.share, load.need, load.behind) = (0.0, 0.0, 0);
        for (i, s) in stages.iter().enumerate() {
            let b = self.second_stages.get(i);
            let busy = s.busy_ns.saturating_sub(b.map_or(0, |b| b.busy_ns)) as f64 / 1e9;
            let signal = s.signal_s - b.map_or(0.0, |b| b.signal_s);
            let cost = if signal > 0.0 { busy / signal } else { 0.0 };
            load.share += busy / window;
            load.need += cost;
            load.behind += s.behind;
            let row = &mut load.stages[i];
            row.declared_units = self.stages.get(i).and_then(|d| d.cost);
            row.declared = row.declared_units.map(|u| u / self.core_units.max(1e-9));
            (row.units, row.share, row.cost) = (cost * self.core_units, busy / window, cost);
            row.load.clone_from(s);
        }
        load.core_units = self.core_units;
        load.threads = self.mask.count_ones() + 1;
        load.gaps = self.gaps;
        load.gap_samples = self.gap_samples;
        self.publish_connections(window);
        self.second.clone_from(&self.comps);
        self.second_stages.clone_from(&self.last);
        self.second_at = Instant::now();
    }
}

impl Placement {
    /// Makes the rows of the table of connections, with the rates over the last `window` seconds and the samples lost
    /// at each place.
    fn publish_connections(&mut self, window: f64) {
        let rows = &mut self.load.connections;
        rows.resize_with(self.conns.len(), ConnectionRow::default);
        if self.second_conns.len() != self.conns.len() {
            self.second_conns.clear();
            self.second_conns.resize(self.conns.len(), (0, 0));
        }
        let first = usize::from(self.queue.is_some());
        for (k, ((row, c), before)) in rows
            .iter_mut()
            .zip(&self.conns)
            .zip(&mut self.second_conns)
            .enumerate()
        {
            let (items, bytes) = (
                c.items.saturating_sub(before.0),
                c.bytes.saturating_sub(before.1),
            );
            row.items_per_s = items as f64 / window;
            row.bytes_per_s = bytes as f64 / window;
            *before = (c.items, c.bytes);
            row.load.clone_from(c);
            match self.last.get(c.stage).filter(|_| k >= first) {
                Some(s) => row.stage.clone_from(&s.name),
                None => row.stage.clear(),
            }
            row.lost = 0;
        }
        if first == 1 {
            rows[0].lost = self.lost_queue + self.gap_samples;
        }
        // The loss before a stage counts at the first connection that crosses into it, so the rows add up to the
        // loss of the session.
        for (s, &lost) in self.lost_stage.iter().enumerate().filter(|(_, l)| **l > 0) {
            let skip = first;
            let at = rows
                .iter()
                .skip(skip)
                .position(|r| r.load.stage == s && r.load.cut)
                .or_else(|| {
                    rows.iter()
                        .skip(skip)
                        .position(|r| r.load.stage == s && r.load.boundary)
                })
                .map(|k| k + skip);
            if let Some(r) = rows.get_mut(at.unwrap_or(0)) {
                r.lost += lost;
            }
        }
    }
}

/// The threads that the engine would give a stack by the load that a benchmark measured.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BenchPlan {
    /// The seconds of signal of the run.
    pub(crate) signal_s: f64,
    /// The cores that each stage needs at the rate of the stack, and the thread of each stage.
    pub(crate) stage_cores: Vec<f64>,
    pub(crate) stage_thread: Vec<u32>,
    pub(crate) threads: u32,
    /// The cores that the fullest thread needs, and its number.
    pub(crate) fullest: f64,
    pub(crate) fullest_thread: u32,
}

/// Places the stages of a benchmark run on at most `cores` threads, as a session would by these costs.
pub(crate) fn bench_plan(stages: &[StageLoad], cores: usize) -> BenchPlan {
    let signal_s = stages.first().map_or(0.0, |s| s.signal_s);
    let stage_cores: Vec<f64> = stages
        .iter()
        .map(|s| {
            if signal_s > 0.0 {
                s.busy_ns as f64 / 1e9 / signal_s
            } else {
                0.0
            }
        })
        .collect();
    let total: f64 = stage_cores.iter().sum();
    let mask = if total >= HEAVY {
        plan(&stage_cores, cores.max(1))
    } else {
        0
    };
    let mut p = BenchPlan {
        signal_s,
        threads: mask.count_ones() + 1,
        ..BenchPlan::default()
    };
    let (mut thread, mut acc) = (0u32, 0.0);
    for (i, c) in stage_cores.iter().enumerate() {
        if i > 0 && mask & (1 << (i - 1)) != 0 {
            (thread, acc) = (thread + 1, 0.0);
        }
        acc += c;
        if acc > p.fullest {
            (p.fullest, p.fullest_thread) = (acc, thread);
        }
        p.stage_thread.push(thread);
    }
    p.stage_cores = stage_cores;
    p
}

/// The row of the queue of a session: from the driver or the host feed to the stack.
fn queue_row(q: &QueueMeter, row: &mut ConnectionLoad) {
    let load = |a: &AtomicU64| a.load(Ordering::Relaxed);
    const IQ: u64 = std::mem::size_of::<sdrkit_core::Complex32>() as u64;
    for (dst, text) in [
        (&mut row.from, "input"),
        (&mut row.output, ""),
        (&mut row.to, "stack"),
        (&mut row.input, ""),
        (&mut row.format, "iq"),
    ] {
        if dst != text {
            text.clone_into(dst);
        }
    }
    let taken = load(&q.taken);
    (row.items, row.bytes) = (taken, taken * IQ);
    row.held_bytes = load(&q.sent).saturating_sub(taken) * IQ;
    (row.held_sum, row.held_samples, row.held_max) = (
        load(&q.held_sum) * IQ,
        load(&q.held_n),
        load(&q.held_max) * IQ,
    );
    row.capacity_bytes = load(&q.blocks) * load(&q.block_len) * IQ;
    (row.wait_ns_sum, row.wait_samples, row.wait_worst_ns) =
        (load(&q.wait_sum), load(&q.wait_n), load(&q.wait_worst));
    (row.full, row.empty) = (load(&q.full), load(&q.empty));
    (row.boundary, row.cut, row.stage) = (false, true, 0);
}

/// Runs the body of a stage thread. A panic of a component ends the thread; the stack then stops and says so.
fn run_body(body: Box<dyn StageRun>) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body.run()));
}

/// The cuts for stages with these costs in cores of this device on at most `threads` threads: a new thread starts where the stages of
/// the present one would cost more than [`FILL`] of a core together.
pub(crate) fn plan(stages: &[f64], threads: usize) -> u32 {
    let total: f64 = stages.iter().sum();
    let mut fill = FILL;
    loop {
        let (mut mask, mut acc, mut used) = (0u32, 0.0, 1);
        for (i, &c) in stages.iter().enumerate() {
            if i > 0 && acc > 0.0 && acc + c > fill {
                mask |= 1 << (i - 1);
                used += 1;
                acc = 0.0;
            }
            acc += c;
        }
        if used <= threads.max(1) || fill > total {
            return if used <= threads.max(1) { mask } else { 0 };
        }
        // Fewer threads than the stages ask for: each thread takes more.
        fill *= 1.25;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plan_fills_each_thread_to_four_fifths_of_a_core() {
        assert_eq!(plan(&[0.5, 0.5, 0.5], 8), 0b11);
        assert_eq!(plan(&[0.3, 0.4, 0.5], 8), 0b10);
        assert_eq!(plan(&[0.1, 0.1, 0.1], 8), 0);
        // Two threads for three half cores: one cut.
        assert_eq!(plan(&[0.5, 0.5, 0.5], 2).count_ones(), 1);
        assert_eq!(plan(&[0.5, 0.5, 0.5], 1), 0);
        assert_eq!(plan(&[2.0], 4), 0);
    }

    #[test]
    fn the_need_of_a_stack_follows_from_its_units_and_the_speed_of_this_device() {
        let d = device();
        assert!(d.fast_units > 0.0 && d.slow_units > 0.0 && d.slow_units <= d.fast_units);
        assert_eq!(device(), d, "the calibration is made once");
        let c = Cores::new();
        c.set_max(4);
        let stage = |name: &str, cores: f64| StageDecl {
            name: name.to_owned(),
            cost: Some(cores * d.fast_units),
            ahead_bytes: 1 << 20,
        };
        let n = c.need(&[stage("a", 0.5), stage("b", 0.5), stage("c", 0.5)], d);
        assert!((n.cores - 1.5).abs() < 1e-9 && (n.units - 1.5 * d.fast_units).abs() < 1e-6);
        assert_eq!((n.threads, n.real_time, n.cores_free), (3, true, 4.0));
        // A stage that needs more than one core cannot keep up, however many cores are free.
        let n = c.need(&[stage("a", 0.2), stage("b", 1.4)], d);
        assert_eq!((n.threads, n.real_time), (2, false));
        // One core for a stack of one and a half.
        c.set_max(1);
        let n = c.need(&[stage("a", 0.5), stage("b", 0.5), stage("c", 0.5)], d);
        assert_eq!((n.threads, n.real_time), (1, false));
        // A stack with no recorded cost.
        let none = StageDecl {
            name: "front".to_owned(),
            cost: None,
            ahead_bytes: 0,
        };
        assert_eq!(c.need(&[none], d).cores, 0.0);
    }

    #[test]
    fn a_benchmark_of_three_heavy_stages_gets_three_threads_and_one_stage_cannot_be_divided() {
        let stage = |busy_s: f64| StageLoad {
            busy_ns: (busy_s * 1e9) as u64,
            signal_s: 1.0,
            ..StageLoad::default()
        };
        let p = bench_plan(&[stage(0.5), stage(0.5), stage(0.5)], 4);
        assert_eq!((p.threads, p.stage_thread.clone()), (3, vec![0, 1, 2]));
        assert!((p.fullest - 0.5).abs() < 1e-9);
        // A single stage of 1.77 cores: no cut helps, and the thread needs more than a core.
        let p = bench_plan(&[stage(1.77)], 8);
        assert_eq!(p.threads, 1);
        assert!(p.fullest > 1.0);
    }

    #[test]
    fn the_machine_has_a_core() {
        assert!(machine_cores() >= 1);
        let c = Cores::new();
        assert_eq!(c.max(), machine_cores());
        c.set_max(2);
        assert_eq!((c.max(), c.pool.workers()), (2, 1.min(c.pool.threads())));
        assert_eq!(c.free(0.0), 2.0);
    }
}
