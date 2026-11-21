//! The services of the engine. Each one is an owner reached by its key. The engine makes each one with the facilities and
//! the lookups it declares, and passes it the events of the sessions on the control thread.

use crate::feed::lock;
use crate::published::Kept;
use crate::request::Action;
use crate::{EngineError, SessionId};
use sdrkit_core::{
    Clock, Event, EventCatalog, EventDecl, Facilities, HostRequest, Owner, OwnerKey, SignalType,
    StackDescriptor, StackLookup, StorePath, Transport,
};
use sdrkit_svc_automation::Automation;
use sdrkit_svc_bank::BankService;
use sdrkit_svc_events::{Context, Kinds};
use sdrkit_svc_freqdb::FreqDbService;
use sdrkit_svc_identify::{IdentifyService, OpenLookup, OpenWith};
use sdrkit_svc_memory::{MemoryFilter, MemoryService};
use sdrkit_svc_outputs::Outputs;
use sdrkit_svc_record::RecordService;
use sdrkit_svc_scan::{MemoryLookup, ScanService};
use sdrkit_svc_store::session::SessionBook;
use sdrkit_svc_store::Store;
use sdrkit_svc_sweep::SweepService;
use sdrkit_svc_watch::Watch;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// The events a service keeps for the host. The oldest go first.
const EVENT_CAP: usize = 1024;

/// The file of the message store in the host's storage.
const STORE_FILE: &str = "store/messages.jsonl";

/// One service of the engine, in the order of the engine's `services` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ServiceKind {
    Store,
    Sessions,
    Automation,
    Outputs,
    Watch,
    Memory,
    Scan,
    Freqdb,
    Sweep,
    Identify,
    Bank,
    Record,
}

/// The owner keys of the services, in the order of [`ServiceKind::ALL`].
static KEYS: [OwnerKey; 12] = [
    sdrkit_svc_store::KEY,
    sdrkit_svc_store::session::KEY,
    sdrkit_svc_automation::KEY,
    sdrkit_svc_outputs::KEY,
    sdrkit_svc_watch::KEY,
    sdrkit_svc_memory::KEY,
    sdrkit_svc_scan::KEY,
    sdrkit_svc_freqdb::KEY,
    sdrkit_svc_sweep::KEY,
    sdrkit_svc_identify::KEY,
    sdrkit_svc_bank::KEY,
    sdrkit_svc_record::KEY,
];

impl ServiceKind {
    /// Every service, in the order of the `services` table.
    pub(crate) const ALL: [ServiceKind; 12] = [
        ServiceKind::Store,
        ServiceKind::Sessions,
        ServiceKind::Automation,
        ServiceKind::Outputs,
        ServiceKind::Watch,
        ServiceKind::Memory,
        ServiceKind::Scan,
        ServiceKind::Freqdb,
        ServiceKind::Sweep,
        ServiceKind::Identify,
        ServiceKind::Bank,
        ServiceKind::Record,
    ];

    /// The service at row `i` of the `services` table.
    pub(crate) fn at(i: u32) -> Option<ServiceKind> {
        Self::ALL.get(usize::try_from(i).ok()?).copied()
    }

    /// The row of the service in the `services` table.
    pub(crate) fn row(self) -> u32 {
        self as u32
    }

    /// The owner key.
    pub(crate) fn key(self) -> &'static str {
        KEYS[self as usize].as_str()
    }

    /// The name for people.
    pub(crate) fn name(self) -> &'static str {
        match self {
            ServiceKind::Store => "Message store",
            ServiceKind::Sessions => "Conversations",
            ServiceKind::Automation => "Automation",
            ServiceKind::Outputs => "Outputs",
            ServiceKind::Watch => "Watch list",
            ServiceKind::Memory => "Memories",
            ServiceKind::Scan => "Scanner",
            ServiceKind::Freqdb => "Frequency database",
            ServiceKind::Sweep => "Sweep",
            ServiceKind::Identify => "Identify",
            ServiceKind::Bank => "Channel bank",
            ServiceKind::Record => "Recorder",
        }
    }
}

/// The first stack of `stacks` that receives `signal`, with the id of its submode whose id is the key of `signal`. The
/// submode id is empty when the stack has no such submode: the stack then runs its default.
pub(crate) fn open_for(
    stacks: &[&'static StackDescriptor],
    signal: SignalType,
) -> Option<(&'static StackDescriptor, &'static str)> {
    let d = stacks
        .iter()
        .find(|d| d.signal_types.contains(&signal))
        .copied()?;
    let key = sdrkit_core::Choice::key(signal);
    let submode = d.submodes.iter().find(|m| m.id == key).map_or("", |m| m.id);
    Some((d, submode))
}

/// The lookup of the stack descriptors of `stacks` by stack id.
fn find_stack(stacks: &Arc<[&'static StackDescriptor]>) -> StackLookup {
    let stacks = Arc::clone(stacks);
    Arc::new(move |id: &str| stacks.iter().find(|d| d.id == id).copied())
}

/// How the identify service opens a signal type, by its key: as [`open_for`] finds it, with the stack's defaults.
fn open_with(stacks: &Arc<[&'static StackDescriptor]>) -> OpenLookup {
    let stacks = Arc::clone(stacks);
    Arc::new(move |signal: &str| {
        let signal = <SignalType as sdrkit_core::Choice>::from_key(signal)?;
        let (d, submode) = open_for(&stacks, signal)?;
        Some(OpenWith {
            stack: d.id.to_owned(),
            submode: submode.to_owned(),
            settings: Vec::new(),
        })
    })
}

/// The memories of `memory` that a filter takes, for a memory scan.
fn memories(memory: &Arc<Kept<MemoryService>>) -> MemoryLookup {
    let memory = Arc::clone(memory);
    Arc::new(move |f: &MemoryFilter| {
        memory
            .lock()
            .store()
            .filter(f)
            .into_iter()
            .cloned()
            .collect()
    })
}

/// The services that need no event catalog. The engine reads their events to build the catalog.
pub(crate) struct First {
    sessions: SessionBook,
    memory: Arc<Kept<MemoryService>>,
    scan: ScanService,
    freqdb: FreqDbService,
    sweep: SweepService,
    identify: IdentifyService,
    bank: BankService,
    record: RecordService,
}

impl First {
    /// Makes the services that need no event catalog, on the host's facilities, with lookups of the engine's `stacks`.
    ///
    /// # Errors
    ///
    /// The error of the service that does not open.
    pub(crate) fn make(
        f: &Facilities,
        stacks: &[&'static StackDescriptor],
    ) -> Result<First, EngineError> {
        let stacks: Arc<[&'static StackDescriptor]> = stacks.into();
        let memory = Arc::new(Kept::new(MemoryService::open(
            Arc::clone(&f.storage),
            f.clock.clone(),
        )?));
        Ok(First {
            sessions: SessionBook::new(),
            scan: ScanService::open(
                Arc::clone(&f.storage),
                f.clock.clone(),
                find_stack(&stacks),
                memories(&memory),
            )?,
            memory,
            freqdb: FreqDbService::open(Arc::clone(&f.storage))?,
            sweep: SweepService::new(Arc::clone(&f.storage), f.clock.clone()),
            identify: IdentifyService::new(open_with(&stacks)),
            bank: BankService::new(find_stack(&stacks)),
            record: RecordService::open(Arc::clone(&f.storage), f.clock.clone())?,
        })
    }

    /// The event kinds of these services, and those of the services made later.
    pub(crate) fn events(&self, out: &mut Vec<EventDecl>) {
        let owners: [&dyn Owner; 8] = [
            &self.sessions,
            &*self.memory.lock(),
            &self.scan,
            &self.freqdb,
            &self.sweep,
            &self.identify,
            &self.bank,
            &self.record,
        ];
        for o in owners {
            out.extend_from_slice(o.events());
        }
        let later = sdrkit_svc_automation::AUTOMATION_EVENTS
            .iter()
            .chain(&sdrkit_svc_watch::WATCH_EVENTS)
            .chain(&sdrkit_svc_record::RECORD_EVENTS);
        out.extend(later.map(|d| d.decl()));
    }
}

/// Every service of the engine, each behind its own lock.
pub(crate) struct Services {
    kinds: Arc<Kinds>,
    store: Kept<Store>,
    sessions: Kept<SessionBook>,
    automation: Kept<Automation>,
    outputs: Kept<Outputs>,
    watch: Kept<Watch>,
    memory: Arc<Kept<MemoryService>>,
    pub(crate) scan: Kept<ScanService>,
    pub(crate) freqdb: Kept<FreqDbService>,
    pub(crate) sweep: Kept<SweepService>,
    pub(crate) identify: Kept<IdentifyService>,
    pub(crate) bank: Kept<BankService>,
    pub(crate) record: Kept<RecordService>,
    events: [Mutex<VecDeque<Event>>; 12],
}

/// What the services returned in one tick of the control thread.
#[derive(Default)]
pub(crate) struct Tick {
    /// The requests, each with the service that made it.
    pub(crate) requests: Vec<(ServiceKind, HostRequest)>,
    /// Lines for the engine's log: what a service could not do.
    pub(crate) problems: Vec<String>,
    /// The actions that the rules of the automation requested from the engine.
    pub(crate) actions: Vec<Action>,
    /// Events of the services, before they are stamped.
    scratch: Vec<Event>,
    again: Vec<Event>,
}

impl Tick {
    /// Keeps the actions among the events in `scratch` that the automation just raised, for session `session` and the event
    /// `trigger` that set them off.
    fn take_actions(&mut self, session: Option<SessionId>, trigger: Option<&Event>) {
        for e in &self.scratch {
            if let Some(kind) = Action::kind_of(e) {
                self.actions.push(Action {
                    kind,
                    session,
                    event: e.clone(),
                    trigger: trigger.cloned(),
                });
            }
        }
    }
}

impl Services {
    /// Makes the services that need the event catalog, and holds them with `first`.
    ///
    /// # Errors
    ///
    /// The error of the service that does not open.
    pub(crate) fn new(
        first: First,
        catalog: Arc<EventCatalog>,
        f: &Facilities,
        transport: &Arc<dyn Transport>,
    ) -> Result<Services, EngineError> {
        let kinds = Arc::new(Kinds::new(catalog));
        let path = StorePath::new(STORE_FILE)?;
        let store = Store::open(Arc::clone(&kinds), Arc::clone(&f.storage), path)?;
        let net = || Arc::clone(transport);
        let clock = || -> Clock { f.clock.clone() };
        Ok(Services {
            store: Kept::new(store),
            automation: Kept::new(Automation::open(
                Arc::clone(&kinds),
                Arc::clone(&f.storage),
            )?),
            outputs: Kept::new(Outputs::open(
                Arc::clone(&kinds),
                Arc::clone(&f.storage),
                net(),
                clock(),
            )?),
            watch: Kept::new(Watch::open(
                Arc::clone(&kinds),
                Arc::clone(&f.storage),
                net(),
                clock(),
            )?),
            kinds,
            sessions: Kept::new(first.sessions),
            memory: first.memory,
            scan: Kept::new(first.scan),
            freqdb: Kept::new(first.freqdb),
            sweep: Kept::new(first.sweep),
            identify: Kept::new(first.identify),
            bank: Kept::new(first.bank),
            record: Kept::new(first.record),
            events: Default::default(),
        })
    }

    /// Runs `f` on service `kind` as the host sees it: a read takes the published copy of the service and does not wait
    /// for its lock.
    pub(crate) fn with<R>(&self, kind: ServiceKind, f: impl FnOnce(&mut dyn Owner) -> R) -> R {
        match kind {
            ServiceKind::Store => self.store.host(f),
            ServiceKind::Sessions => self.sessions.host(f),
            ServiceKind::Automation => self.automation.host(f),
            ServiceKind::Outputs => self.outputs.host(f),
            ServiceKind::Watch => self.watch.host(f),
            ServiceKind::Memory => self.memory.host(f),
            ServiceKind::Scan => self.scan.host(f),
            ServiceKind::Freqdb => self.freqdb.host(f),
            ServiceKind::Sweep => self.sweep.host(f),
            ServiceKind::Identify => self.identify.host(f),
            ServiceKind::Bank => self.bank.host(f),
            ServiceKind::Record => self.record.host(f),
        }
    }

    /// Moves the events of service `kind` that wait into `out`.
    pub(crate) fn take_events(&self, kind: ServiceKind, out: &mut Vec<Event>) {
        out.extend(lock(&self.events[kind as usize]).drain(..));
    }

    /// Passes the events of session `session` to the services that read events: the store, the conversations, the
    /// automation, the watch list and the outputs. The events that these services raise come back through them once.
    pub(crate) fn observe(
        &self,
        events: &[Event],
        session: Option<SessionId>,
        ctx: &Context,
        offset_min: i32,
        catalog: &EventCatalog,
        tick: &mut Tick,
    ) {
        self.observe_once(events, session, ctx, offset_min, catalog, tick);
        let again = std::mem::take(&mut tick.again);
        self.observe_once(&again, session, ctx, offset_min, catalog, tick);
        tick.again.clear();
    }

    fn observe_once(
        &self,
        events: &[Event],
        session: Option<SessionId>,
        ctx: &Context,
        offset_min: i32,
        catalog: &EventCatalog,
        tick: &mut Tick,
    ) {
        for ev in events {
            let Some(o) = self.kinds.observe(ev, ctx) else {
                continue;
            };
            if let Err(e) = self.store.lock().insert(&o) {
                tick.problems
                    .push(format!("{}: {e}", ServiceKind::Store.key()));
            }
            let changes = self.sessions.lock().observe(&o);
            tick.scratch.extend(changes.into_iter().map(|(_, e)| e));
            self.keep(ServiceKind::Sessions, catalog, tick);
            self.automation
                .lock()
                .handle(&o, ctx, offset_min, &mut tick.scratch);
            tick.take_actions(session, Some(ev));
            self.keep(ServiceKind::Automation, catalog, tick);
            self.watch.lock().check(&o, offset_min, &mut tick.scratch);
            self.keep(ServiceKind::Watch, catalog, tick);
            self.outputs.lock().submit(&o);
        }
    }

    /// Sends the event `trigger` to the output with the row key `row`, past its filter.
    ///
    /// # Errors
    ///
    /// [`EngineError::Outputs`] if the output refuses it, [`EngineError::Unlisted`] if the catalog does not list the event.
    pub(crate) fn send_to(
        &self,
        row: &str,
        trigger: &Event,
        ctx: &Context,
    ) -> Result<(), EngineError> {
        let o = self
            .kinds
            .observe(trigger, ctx)
            .ok_or(EngineError::Unlisted)?;
        Ok(self.outputs.lock().send_to(row, &o)?)
    }

    /// Stamps the events in `tick.scratch`, keeps them for the host on service `kind`, and keeps a copy to bring back
    /// through the services.
    fn keep(&self, kind: ServiceKind, catalog: &EventCatalog, tick: &mut Tick) {
        if tick.scratch.is_empty() {
            return;
        }
        let mut q = lock(&self.events[kind as usize]);
        for mut e in tick.scratch.drain(..) {
            if catalog.stamp(&mut e).is_none() {
                tick.problems.push(format!(
                    "{}: the event kind {} is not listed",
                    kind.key(),
                    e.kind()
                ));
                continue;
            }
            if q.len() == EVENT_CAP {
                q.pop_front();
            }
            tick.again.push(e.clone());
            q.push_back(e);
        }
    }

    /// The work of the services at a tick of the control thread at wall time `now`: their clocks, the jobs they finished,
    /// and the requests they made.
    pub(crate) fn tick(
        &self,
        now: f64,
        ctx: &Context,
        offset_min: i32,
        catalog: &EventCatalog,
        tick: &mut Tick,
    ) {
        self.store.lock().prune(now);
        tick.scratch
            .extend(self.sessions.lock().tick(now).into_iter().map(|(_, e)| e));
        self.keep(ServiceKind::Sessions, catalog, tick);
        self.automation
            .lock()
            .tick(now, offset_min, &mut tick.scratch);
        tick.take_actions(None, None);
        self.keep(ServiceKind::Automation, catalog, tick);
        self.outputs.lock().tick();
        self.watch.lock().tick();
        self.record.lock().tick(now, &mut tick.scratch);
        self.keep(ServiceKind::Record, catalog, tick);
        {
            let mut id = self.identify.lock();
            id.tick(now);
            id.take_events(&mut tick.scratch);
        }
        self.keep(ServiceKind::Identify, catalog, tick);
        self.sweep.lock().take_events(&mut tick.scratch);
        self.keep(ServiceKind::Sweep, catalog, tick);
        let again = std::mem::take(&mut tick.again);
        self.observe_once(&again, None, ctx, offset_min, catalog, tick);
        tick.again.clear();
        let mut take = |kind: ServiceKind, reqs: Vec<HostRequest>| {
            tick.requests.extend(reqs.into_iter().map(|r| (kind, r)));
        };
        take(ServiceKind::Scan, self.scan.lock().take_requests());
        take(ServiceKind::Identify, self.identify.lock().take_requests());
        take(ServiceKind::Sweep, self.sweep.lock().take_requests());
        take(ServiceKind::Record, self.record.lock().take_requests());
        take(ServiceKind::Memory, self.memory.lock().take_requests());
        take(ServiceKind::Freqdb, self.freqdb.lock().take_requests());
    }

    /// The memory service, for a request that stores a memory.
    pub(crate) fn memory(&self) -> &Kept<MemoryService> {
        &self.memory
    }
}
