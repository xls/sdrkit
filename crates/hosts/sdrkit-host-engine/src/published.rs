//! The published copies of a session. The session thread fills a buffer of its own with the receiver, the components of
//! its graph and its run state, and swaps it with the front buffer. A thread of the host copies from the front buffer into
//! a [`Mirror`] that it owns. Neither side waits for the other for longer than the swap.
//!
//! The engine, a radio and a service are owners behind a lock of their own ([`Kept`]). Each release of that lock publishes
//! the owner, and a read of the host takes the published copy: it does not wait for a thread that holds the lock.

use crate::feed::lock;
use crate::transmit::TxReport;
use sdrkit_core::results::copy_if_changed;
use sdrkit_core::{
    Commands, EventDecl, Owner, Receiver, Results, ResultsDecl, Schema, TableId, Values,
};
use sdrkit_graph::{ComponentHandle, Graph};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

/// The values of the settings are compared with the copy this often, for an owner that changes a value itself.
const DESCRIBE: Duration = Duration::from_millis(100);

/// True if table `t` changed after copy `since`, by the copy numbers `changed_at`.
fn changed(changed_at: &[u64], t: TableId, since: u64) -> bool {
    changed_at.get(usize::from(t.0)).is_none_or(|&c| c > since)
}

/// One owner (the receiver, or a component of its graph) as the session thread copied it.
#[derive(Default)]
pub(crate) struct OwnerCopy {
    pub(crate) results: Results,
    /// For each table, the number of the copy at which it last changed.
    pub(crate) changed_at: Vec<u64>,
    schema: Option<Arc<Schema>>,
    pub(crate) values: Option<Values>,
    commands: Commands,
    events: Vec<EventDecl>,
    /// The number of the copy at which the settings, their values, the commands or the event kinds last changed.
    described_at: u64,
}

impl OwnerCopy {
    /// Brings the copy, which was filled last at copy `since`, up to date with `owner`.
    fn fill(&mut self, track: &Track, owner: &dyn Owner, since: u64) {
        let src = owner.results();
        let all = !Arc::ptr_eq(self.results.decl(), src.decl());
        if all {
            self.results = Results::new(Arc::clone(src.decl()));
        }
        let at = &track.changed_at;
        self.results
            .copy_from_where(src, |t| all || changed(at, t, since));
        self.changed_at.clone_from(at);
        if all || track.described_at > since {
            self.schema = Some(Arc::clone(owner.schema()));
            copy_values(&mut self.values, owner.values());
            copy_if_changed(&mut self.commands, owner.commands());
            self.events.clear();
            self.events.extend_from_slice(owner.events());
        }
        self.described_at = track.described_at;
    }
}

fn copy_values(dst: &mut Option<Values>, src: Option<&Values>) {
    match (dst.as_mut(), src) {
        (Some(d), Some(s)) => d.clone_from(s),
        _ => *dst = src.cloned(),
    }
}

/// What the session thread knows of one owner between two copies.
#[derive(Default)]
struct Track {
    decl: Option<Arc<ResultsDecl>>,
    /// The change counter of the results and of each table at the last copy.
    whole: u64,
    seen: Vec<u64>,
    changed_at: Vec<u64>,
    values: Option<Values>,
    described_at: u64,
}

impl Track {
    /// Notes at copy `seq` which tables of `owner` changed since the copy before, and whether its settings did. `all`
    /// takes every part as changed. `describe` compares the values of the settings.
    fn observe(&mut self, owner: &dyn Owner, seq: u64, all: bool, describe: bool) {
        let src = owner.results();
        let same = self
            .decl
            .as_ref()
            .is_some_and(|d| Arc::ptr_eq(d, src.decl()));
        // A counter that went back belongs to new results.
        let all = all || !same || src.change_counter() < self.whole;
        if !same {
            self.decl = Some(Arc::clone(src.decl()));
        }
        self.whole = src.change_counter();
        let n = src.decl().tables().len();
        self.seen.resize(n, u64::MAX);
        self.changed_at.resize(n, seq);
        for (i, (seen, at)) in self.seen.iter_mut().zip(&mut self.changed_at).enumerate() {
            let c = u16::try_from(i)
                .ok()
                .and_then(|i| src.table(TableId(i)))
                .map_or(0, |t| t.change_counter());
            if all || c != *seen {
                (*seen, *at) = (c, seq);
            }
        }
        if all || (describe && self.values.as_ref() != owner.values()) {
            copy_values(&mut self.values, owner.values());
            self.described_at = seq;
        }
    }
}

/// The run state of a session at a copy.
#[derive(Default)]
pub(crate) struct RunState {
    /// The receiver refused its input.
    pub(crate) failed: bool,
    /// The receiver is configured for an input.
    pub(crate) has_input: bool,
    /// The refusal of the input or of a setting, as text.
    pub(crate) problem: Option<String>,
    /// Moves on when `problem` changes.
    pub(crate) problem_seq: u64,
    /// The number of the last change of frequency that the receiver has taken.
    pub(crate) retunes: u64,
    /// Events of a kind that the catalog does not list.
    pub(crate) unlisted: u64,
    /// The number of the last copy for which the session's copy of the receiver's values was out of date.
    pub(crate) stale_at: u64,
    pub(crate) tx: Option<TxReport>,
    /// The load of the session, as its thread last measured it.
    pub(crate) load: crate::stages::Load,
}

/// One complete copy of a session.
#[derive(Default)]
pub(crate) struct Snapshot {
    /// The number of the copy. It grows with each copy.
    pub(crate) seq: u64,
    /// The number of the receiver: it moves on when another receiver takes the place of the one that runs.
    pub(crate) generation: u64,
    pub(crate) stack: OwnerCopy,
    /// The components of the receiver's graph, by handle. Empty for a receiver that is no graph.
    pub(crate) components: Vec<OwnerCopy>,
    /// The names of the components that a plan names, by handle.
    pub(crate) names: Vec<String>,
    /// The receiver is a graph.
    pub(crate) graph: bool,
    pub(crate) state: RunState,
}

/// The side of the session thread: the buffer it fills, and what it knows of each owner.
#[derive(Default)]
pub(crate) struct Publisher {
    back: Snapshot,
    seq: u64,
    stack: Track,
    components: Vec<Track>,
    names: Vec<String>,
    /// `names` holds the names of the graph that runs.
    named: bool,
    described: Option<Instant>,
    /// The buffer holds a copy that the front buffer does not hold yet.
    pub(crate) waits: bool,
}

impl Publisher {
    /// Copies `rx` and the components of its graph into the buffer and returns the state of the buffer, for the caller to
    /// set. `all` takes every part as changed: after a setting, a command or another receiver.
    pub(crate) fn fill(&mut self, rx: &mut dyn Receiver, all: bool) -> &mut Snapshot {
        self.seq += 1;
        let (seq, since) = (self.seq, self.back.seq);
        let describe = all || self.described.is_none_or(|t| t.elapsed() >= DESCRIBE);
        if describe {
            self.described = Some(Instant::now());
        }
        self.stack.observe(&*rx, seq, all, describe);
        self.back.stack.fill(&self.stack, &*rx, since);
        let graph = rx.as_any_mut().and_then(|a| a.downcast_mut::<Graph>());
        self.back.graph = graph.is_some();
        let mut n = 0;
        if let Some(g) = graph {
            if all || !self.named {
                self.names.clear();
                self.names
                    .extend(g.describe().components.into_iter().map(|c| c.id));
                self.named = true;
            }
            while let Some(o) = u32::try_from(n)
                .ok()
                .and_then(|i| g.owner(ComponentHandle(i)))
            {
                if self.components.len() == n {
                    self.components.push(Track::default());
                }
                if self.back.components.len() == n {
                    self.back.components.push(OwnerCopy::default());
                }
                self.components[n].observe(&o, seq, all, describe);
                self.back.components[n].fill(&self.components[n], &o, since);
                n += 1;
            }
        } else {
            self.names.clear();
            self.named = false;
        }
        self.components.truncate(n);
        self.back.components.truncate(n);
        self.back.names.clone_from(&self.names);
        self.back.seq = seq;
        self.waits = true;
        &mut self.back
    }

    /// Swaps the buffer with `front` and returns the number of the copy that `front` then holds. `None` if a thread of the
    /// host reads `front` now and `wait` is false: the buffer keeps the copy for the next call.
    pub(crate) fn swap(&mut self, front: &Mutex<Snapshot>, wait: bool) -> Option<u64> {
        let mut f = if wait {
            lock(front)
        } else {
            match front.try_lock() {
                Ok(f) => f,
                Err(TryLockError::Poisoned(p)) => p.into_inner(),
                Err(TryLockError::WouldBlock) => return None,
            }
        };
        std::mem::swap(&mut *f, &mut self.back);
        self.waits = false;
        Some(f.seq)
    }
}

static NO_SETTINGS: std::sync::LazyLock<Arc<Schema>> = std::sync::LazyLock::new(Arc::default);

/// The copy of one owner that the threads of the host read. Its change counters grow from copy to copy.
#[derive(Default)]
pub(crate) struct Mirror {
    /// The number of the copy it holds.
    synced: u64,
    results: Results,
    schema: Option<Arc<Schema>>,
    values: Option<Values>,
    commands: Commands,
    events: Vec<EventDecl>,
}

impl Mirror {
    /// Makes the mirror hold `copy`, which is part of copy `seq` of the session.
    pub(crate) fn sync(&mut self, copy: &OwnerCopy, seq: u64) {
        if seq == self.synced {
            return;
        }
        let src = &copy.results;
        let all = !Arc::ptr_eq(self.results.decl(), src.decl());
        if all {
            self.results = Results::new(Arc::clone(src.decl()));
        }
        let since = self.synced;
        self.results
            .copy_from_where(src, |t| all || changed(&copy.changed_at, t, since));
        if all || copy.described_at > since {
            self.schema.clone_from(&copy.schema);
            copy_values(&mut self.values, copy.values.as_ref());
            copy_if_changed(&mut self.commands, &copy.commands);
            self.events.clone_from(&copy.events);
        }
        self.synced = seq;
    }

    /// The number of the copy that the mirror holds. 0 before the first.
    pub(crate) fn synced(&self) -> u64 {
        self.synced
    }

    pub(crate) fn schema(&self) -> &Arc<Schema> {
        self.schema.as_ref().unwrap_or(&NO_SETTINGS)
    }

    pub(crate) fn values(&self) -> Option<&Values> {
        self.values.as_ref()
    }

    pub(crate) fn results(&self) -> &Results {
        &self.results
    }

    pub(crate) fn commands(&self) -> &Commands {
        &self.commands
    }

    pub(crate) fn events(&self) -> &[EventDecl] {
        &self.events
    }
}

/// The side that publishes one owner: the buffer it fills, and what it knows of the owner.
#[derive(Default)]
struct Solo {
    track: Track,
    back: OwnerCopy,
    /// The number of the copy that `back` holds.
    back_seq: u64,
    seq: u64,
}

/// The newest copy of one owner, and its number. 0 before the first copy.
#[derive(Default)]
struct Front {
    copy: OwnerCopy,
    seq: u64,
}

/// The published copy of one owner and the mirror that the threads of the host read.
pub(crate) struct Shown {
    writer: Mutex<Solo>,
    front: Mutex<Front>,
    /// The mirror, or `None` while a thread of the host reads through it.
    mirror: Mutex<Option<Mirror>>,
    /// Told when a thread puts the mirror back.
    back: std::sync::Condvar,
}

impl Default for Shown {
    fn default() -> Self {
        Shown {
            writer: Mutex::default(),
            front: Mutex::default(),
            mirror: Mutex::new(Some(Mirror::default())),
            back: std::sync::Condvar::new(),
        }
    }
}

/// Runs a function on the owner itself, under its lock, and publishes the owner after it.
type Real<'a> = &'a (dyn Fn(&mut dyn FnMut(&mut dyn Owner)) + Sync);

impl Shown {
    /// Publishes `owner` if it changed since the last copy. The caller holds the lock of `owner`.
    pub(crate) fn publish(&self, owner: &dyn Owner) {
        let mut w = lock(&self.writer);
        let Solo { track, back, .. } = &mut *w;
        let src = owner.results();
        let same_decl = track
            .decl
            .as_ref()
            .is_some_and(|d| Arc::ptr_eq(d, src.decl()));
        // The buffer holds an older copy of the same owner, so a difference from it is a change.
        let described = !back
            .schema
            .as_ref()
            .is_some_and(|s| Arc::ptr_eq(s, owner.schema()))
            || back.commands != *owner.commands()
            || back.events.as_slice() != owner.events();
        let changed = described
            || !same_decl
            || src.change_counter() != track.whole
            || track.values.as_ref() != owner.values();
        if !changed {
            return;
        }
        w.seq += 1;
        let (seq, since) = (w.seq, w.back_seq);
        let Solo { track, back, .. } = &mut *w;
        track.observe(owner, seq, described, true);
        back.fill(track, owner, since);
        let mut f = lock(&self.front);
        std::mem::swap(&mut f.copy, &mut w.back);
        w.back_seq = f.seq;
        f.seq = seq;
    }

    /// Takes the mirror, holding the newest copy. Waits while another thread of the host reads through it.
    fn take_mirror(&self) -> Mirror {
        let mut slot = lock(&self.mirror);
        let mut m = loop {
            match slot.take() {
                Some(m) => break m,
                None => slot = self.back.wait(slot).unwrap_or_else(|p| p.into_inner()),
            }
        };
        drop(slot);
        let f = lock(&self.front);
        m.sync(&f.copy, f.seq);
        m
    }

    /// Puts the mirror back for the next read.
    fn put_mirror(&self, m: Mirror) {
        *lock(&self.mirror) = Some(m);
        self.back.notify_one();
    }

    /// Runs `f` on the owner as the host sees it. A read takes the published copy and does not wait for the lock of
    /// the owner. A setting or a command runs through `real`, and the reads after it see its effect.
    pub(crate) fn host<R>(&self, real: Real<'_>, f: impl FnOnce(&mut dyn Owner) -> R) -> R {
        if lock(&self.front).seq == 0 {
            real(&mut |_| ());
        }
        f(&mut ShownProxy {
            shown: self,
            real,
            mirror: Some(self.take_mirror()),
        })
    }
}

/// An owner as the host sees it: reads from the mirror, settings and commands on the owner itself.
struct ShownProxy<'a> {
    shown: &'a Shown,
    real: Real<'a>,
    /// `None` only while a setting or a command runs: the mirror is free for the other threads of the host then.
    mirror: Option<Mirror>,
}

impl Drop for ShownProxy<'_> {
    fn drop(&mut self) {
        if let Some(m) = self.mirror.take() {
            self.shown.put_mirror(m);
        }
    }
}

static NO_RESULTS: std::sync::LazyLock<Results> = std::sync::LazyLock::new(Results::default);
static NO_COMMANDS: std::sync::LazyLock<Commands> = std::sync::LazyLock::new(Commands::default);

impl ShownProxy<'_> {
    /// Runs `op` on the owner itself and brings the mirror up to date with its effect.
    fn write<T>(&mut self, mut op: impl FnMut(&mut dyn Owner) -> T) -> Option<T> {
        if let Some(m) = self.mirror.take() {
            self.shown.put_mirror(m);
        }
        let mut out = None;
        (self.real)(&mut |o| out = Some(op(o)));
        self.mirror = Some(self.shown.take_mirror());
        out
    }
}

impl Owner for ShownProxy<'_> {
    fn schema(&self) -> &Arc<Schema> {
        self.mirror.as_ref().map_or(&NO_SETTINGS, |m| m.schema())
    }

    fn values(&self) -> Option<&Values> {
        self.mirror.as_ref().and_then(|m| m.values())
    }

    fn apply(
        &mut self,
        id: sdrkit_core::SettingId,
        value: &sdrkit_core::Value,
    ) -> Result<(), sdrkit_core::SettingError> {
        self.write(|o| o.apply(id, value))
            .unwrap_or(Err(sdrkit_core::SettingError::NotPermitted { id }))
    }

    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results().decl()
    }

    fn results(&self) -> &Results {
        self.mirror.as_ref().map_or(&NO_RESULTS, |m| m.results())
    }

    fn events(&self) -> &[EventDecl] {
        self.mirror.as_ref().map_or(&[], |m| m.events())
    }

    fn commands(&self) -> &Commands {
        self.mirror.as_ref().map_or(&NO_COMMANDS, |m| m.commands())
    }

    fn invoke(
        &mut self,
        id: sdrkit_core::CommandId,
        args: &Values,
    ) -> Result<Option<sdrkit_core::Record>, sdrkit_core::CommandError> {
        self.write(|o| o.invoke(id, args))
            .unwrap_or(Err(sdrkit_core::CommandError::NoSuchId { id }))
    }

    fn problem(&mut self, text: &str) {
        let _ = self.write(|o| o.problem(text));
    }
}

/// An owner behind its lock, with its published copy. Each release of the lock publishes the owner.
pub(crate) struct Kept<O: Owner> {
    real: Mutex<O>,
    shown: Shown,
}

impl<O: Owner> Kept<O> {
    pub(crate) fn new(owner: O) -> Self {
        let k = Kept {
            real: Mutex::new(owner),
            shown: Shown::default(),
        };
        drop(k.lock());
        k
    }

    /// Locks the owner itself. The engine works on it through this lock; the host reads through [`Kept::host`].
    pub(crate) fn lock(&self) -> KeptGuard<'_, O> {
        KeptGuard {
            owner: lock(&self.real),
            shown: &self.shown,
        }
    }

    /// Runs `f` on the owner as the host sees it ([`Shown::host`]).
    pub(crate) fn host<R>(&self, f: impl FnOnce(&mut dyn Owner) -> R) -> R {
        let real = |op: &mut dyn FnMut(&mut dyn Owner)| op(&mut *self.lock());
        self.shown.host(&real, f)
    }
}

/// The lock of a [`Kept`] owner. Its release publishes the owner.
pub(crate) struct KeptGuard<'a, O: Owner> {
    owner: MutexGuard<'a, O>,
    shown: &'a Shown,
}

impl<O: Owner> Deref for KeptGuard<'_, O> {
    type Target = O;
    fn deref(&self) -> &O {
        &self.owner
    }
}

impl<O: Owner> DerefMut for KeptGuard<'_, O> {
    fn deref_mut(&mut self) -> &mut O {
        &mut self.owner
    }
}

impl<O: Owner> Drop for KeptGuard<'_, O> {
    fn drop(&mut self) {
        self.shown.publish(&*self.owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::{CommandError, CommandId, Record, SettingError, SettingId, Value};
    use std::time::Duration;

    /// An owner whose setting takes 150 ms, as a retune of a network radio does.
    #[derive(Default)]
    struct SlowOwner {
        schema: Arc<Schema>,
        results: Results,
        commands: Commands,
    }

    impl Owner for SlowOwner {
        fn schema(&self) -> &Arc<Schema> {
            &self.schema
        }
        fn values(&self) -> Option<&Values> {
            None
        }
        fn apply(&mut self, _: SettingId, _: &Value) -> Result<(), SettingError> {
            std::thread::sleep(Duration::from_millis(150));
            Ok(())
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
        fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
            Err(CommandError::NoSuchId { id })
        }
    }

    /// The time in ms that `read` takes when it starts 30 ms after `write` started on another thread.
    fn read_during(write: impl FnOnce() + Send, read: impl FnOnce()) -> f64 {
        std::thread::scope(|s| {
            s.spawn(write);
            std::thread::sleep(Duration::from_millis(30));
            let t = Instant::now();
            read();
            t.elapsed().as_secs_f64() * 1e3
        })
    }

    #[test]
    fn a_read_does_not_wait_for_a_setting_that_another_thread_applies() {
        let id = SettingId(0);
        let plain = Mutex::new(SlowOwner::default());
        let locked = read_during(
            || {
                let _ = lock(&plain).apply(id, &Value::Bool(true));
            },
            || {
                let _ = lock(&plain).results().change_counter();
            },
        );
        let kept = Kept::new(SlowOwner::default());
        let published = read_during(
            || {
                let _ = kept.host(|o| o.apply(id, &Value::Bool(true)));
            },
            || {
                let _ = kept.host(|o| o.results().change_counter());
            },
        );
        eprintln!("a read during a 150 ms setting: {locked:.1} ms under one lock, {published:.3} ms published");
        assert!(locked > 80.0, "{locked}");
        assert!(published < 10.0, "{published}");
    }
}
