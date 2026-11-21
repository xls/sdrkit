//! The tests of the aviation stacks: one program with a module per stack, the fixed faults and the helpers.

mod acars;
mod airband;
mod airband_tx;
mod alloc;
mod bugs;
mod common;
mod gbas_vdb;
mod hfdl;
mod ofdm_shape;
mod selcal;
mod vdl2;
mod vdl3;
mod vdl4;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    /// Allocations made by this thread.
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

/// The system allocator, counting the allocations of each thread (for the tests that a hot path allocates nothing).
struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: the layout is passed on unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `alloc` above with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|c| c.set(c.get() + 1));
        // SAFETY: `ptr` came from `alloc` with this layout.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// The allocations this thread makes while `f` runs.
pub fn allocations(f: impl FnOnce()) -> u64 {
    let before = ALLOCATIONS.with(Cell::get);
    f();
    ALLOCATIONS.with(Cell::get) - before
}

/// A plan file of the crate's `data/` folder, read as the host's plan import reads it.
pub fn plan_file(name: &str) -> sdrkit_data_plans::data::PlanFile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("the data file");
    let mut plans = sdrkit_data_plans::Plans::new();
    sdrkit_data_plans::api::import_as(
        &mut plans,
        &text,
        sdrkit_data_plans::api::PlanFormat::Json,
        "",
        "",
    )
    .expect("the registry takes the file");
    sdrkit_data_plans::data::parse(name, &text).expect("a plan file")
}
