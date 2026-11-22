//! A record write allocates nothing after the first write of a text of that size, a full table takes new rows without
//! allocating, and a new event allocates only its values.

use sdrkit_core::publish::{field, keys, Field, Published, TableDef};
use sdrkit_core::{EventDef, Record, RecordDecl};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

struct Counting;

thread_local! {
    /// The number of allocations of this thread.
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

/// Adds one allocation to the count of this thread.
fn count() {
    let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
}

/// Returns the number of allocations of this thread so far.
fn allocs() -> usize {
    ALLOCS.with(Cell::get)
}

// SAFETY: every call goes to the system allocator unchanged. The counter has no effect on the memory.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        count();
        // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: the caller keeps the contract of `GlobalAlloc::dealloc`.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        count();
        // SAFETY: the caller keeps the contract of `GlobalAlloc::realloc`.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const STATION: Field<field::Text> = Field::new("station", "Station", "");
static BLOCK: EventDef = EventDef::new("test.block", "Block", &[&STATION, &keys::SNR_DB]);

#[test]
fn a_record_write_allocates_nothing_after_the_first() {
    let decl = Arc::new(RecordDecl::of(&[&STATION, &keys::SNR_DB, &keys::LOCKED]));
    let mut r = Record::new(decl);
    let write = |r: &mut Record, i: u32| {
        r.set(
            &STATION,
            if i.is_multiple_of(2) {
                "Triple M"
            } else {
                "Triple J"
            },
        );
        r.set(&keys::SNR_DB, f64::from(i));
        r.set(&keys::LOCKED, i.is_multiple_of(2));
    };
    write(&mut r, 0);
    let before = allocs();
    for i in 1..100 {
        write(&mut r, i);
    }
    assert_eq!(allocs() - before, 0);
}

#[test]
fn a_new_event_allocates_only_its_value_slots() {
    drop(BLOCK.event(0.0, 0.0));
    let before = allocs();
    let e = BLOCK.event(1.0, 7e6).set(&keys::SNR_DB, 3.0);
    assert_eq!(allocs() - before, 1);
    assert_eq!(e.get(&keys::SNR_DB), Some(3.0));
}

const HEARD: TableDef = TableDef {
    key: "heard",
    label: "Heard",
    columns: &[&keys::SNR_DB, &keys::LOCKED],
    cap: 100,
    marks: sdrkit_core::RowMarks::NONE,
};

#[test]
fn a_full_table_takes_new_rows_without_allocating() {
    let mut p = Published::new(&[]).with_tables(&[&HEARD]);
    let keys: Vec<String> = (0..1000).map(|i| format!("{i:06}")).collect();
    let write = |p: &mut Published, k: &str, i: usize| {
        let mut r = p.row_mut(&HEARD, k).unwrap();
        r.set(&keys::SNR_DB, i as f64);
        r.set(&keys::LOCKED, i.is_multiple_of(2));
    };
    for (i, k) in keys.iter().enumerate() {
        write(&mut p, k, i);
    }
    let before = allocs();
    for round in 0..5 {
        for (i, k) in keys.iter().enumerate() {
            write(&mut p, k, i + round);
        }
    }
    assert_eq!(allocs() - before, 0);
}
