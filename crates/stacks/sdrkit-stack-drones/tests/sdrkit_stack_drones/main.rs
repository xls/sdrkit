//! The tests of the drones stacks: the rules that the three stacks share, each stack's acceptance cases and recordings, and the
//! shared signal processing that they rely on.

mod dsp;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Counts the allocations of each thread, so a test can make sure that a hot path allocates nothing.
struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: every call goes to the system allocator with the same arguments. Only a thread-local counter is touched besides.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `System` with layout `l`.
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// The allocations of this thread so far.
pub fn allocations() -> u64 {
    ALLOCATIONS.with(|c| c.get())
}
mod adsl;
mod dji;
mod flarm;
mod mavlink_sik;
mod ogn;
mod ppm72;
mod rc_links;
mod remote_id;
mod rules;

use sdrkit_core::interface::Column;
use sdrkit_core::publish::{field, Declare, Field, TableDef};
use sdrkit_core::{Complex32, Record, Results, StackDescriptor};
use sdrkit_dev_test::{Harness, Run};

/// Row `id` of table `def`.
pub fn row<'r>(r: &'r Results, def: &TableDef, id: &str) -> Option<&'r Record> {
    r.table(r.decl().table_id(def.key)?)?.row(id)
}

/// The row keys of table `def`, newest first.
pub fn rows(r: &Results, def: &TableDef) -> Vec<String> {
    r.decl()
        .table_id(def.key)
        .and_then(|id| r.table(id))
        .map(|t| t.rows_newest_first().map(|(k, _)| k.to_string()).collect())
        .unwrap_or_default()
}

/// A count field: a standard key or a column of a result set of the description.
pub trait Count {
    /// The key of the field.
    fn key(&self) -> &str;
    /// The value of the field in `fields`.
    fn read(&self, fields: &Record) -> Option<i64>;
}

impl Count for Field<field::I64> {
    fn key(&self) -> &str {
        self.key
    }
    fn read(&self, fields: &Record) -> Option<i64> {
        fields.get(self)
    }
}

impl Count for Column<field::I64> {
    fn key(&self) -> &str {
        Declare::key(self)
    }
    fn read(&self, fields: &Record) -> Option<i64> {
        fields.at(self)
    }
}

/// The count field `f`. It panics if the field has no value.
pub fn count(r: &Results, f: &impl Count) -> i64 {
    f.read(r.fields())
        .unwrap_or_else(|| panic!("no field {}", f.key()))
}

/// Feeds `x` to a fresh receiver of the harness (the harness ends the stream, so the results of its end are published).
pub fn feed(h: &Harness, x: Vec<Complex32>) -> Run {
    h.feed(&x).expect("the receiver runs")
}

/// Runs the harness's test signal.
pub fn run(h: Harness) -> Run {
    let x = h.generate().expect("the test signal");
    feed(&h, x)
}

/// Feeds the recording `drones/<name>` to the stack. `None` when the recording is missing. When SDRKIT_RECORDINGS is set, a
/// missing recording fails the test.
pub fn recording(desc: &'static StackDescriptor, name: &str) -> Option<Run> {
    let rec = sdrkit_dev_test::read_recording(&format!("drones/{name}"));
    let Some(rec) = rec else {
        assert!(
            std::env::var("SDRKIT_RECORDINGS").is_err(),
            "the recording {name} is missing"
        );
        return None;
    };
    Some(
        Harness::new(desc)
            .rate(rec.rate_hz)
            .freq(rec.centre_hz)
            .feed(&rec.samples)
            .expect("the receiver runs"),
    )
}
