//! Updating an object that exists and writing its row allocates nothing.

use sdrkit_core::publish::{field, Declare, Field, Published, TableDef};
use sdrkit_util_objects::publish::{self, columns, Cells, Object};
use sdrkit_util_objects::{ObjectTable, Seen, Stamp};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

struct Counting;

// SAFETY: every call goes to the system allocator unchanged. The count is a thread-local cell.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `System` with layout `l`.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        // SAFETY: `p` came from `System` with layout `l`.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn count(f: impl FnOnce()) -> usize {
    let before = COUNT.with(Cell::get);
    f();
    COUNT.with(Cell::get) - before
}

struct Ship {
    name: String,
    speed_kt: f64,
}

const NAME: Field<field::Text> = Field::new("name", "Name", "");
const SPEED: Field<field::F64> = Field::new("speed_kt", "Speed", "kt");
const SHIPS: TableDef = TableDef {
    key: "ships",
    label: "Ships",
    columns: &[
        &NAME as &dyn Declare,
        &SPEED,
        &columns::LAST_HEARD,
        &columns::HEARD,
    ],
    cap: 64,
};

impl Object for Ship {
    fn write(&self, seen: Option<&Seen>, _now_s: f64, out: &mut impl Cells) {
        out.set(&NAME, &self.name);
        out.set(&SPEED, self.speed_kt);
        out.opt(&columns::LAST_HEARD, columns::last_heard(seen));
        out.opt(&columns::HEARD, columns::heard(seen));
    }
}

#[test]
fn updating_an_object_that_exists_allocates_nothing() {
    let mut ships: ObjectTable<u32, Ship> = ObjectTable::new(64);
    let mut p = Published::new(&[]).with_tables(&[&SHIPS]);
    let make = || Ship {
        name: "PRIDE OF KENT".into(),
        speed_kt: 0.0,
    };
    for k in 0..64u32 {
        ships
            .upsert(&(235_000_000 + k), Stamp::at(0.0), make)
            .unwrap();
    }
    publish::rows(&ships, &SHIPS, 0.0, &mut p);
    assert!(count(|| drop(String::from("x"))) > 0);
    let n = count(|| {
        for i in 0..1000u32 {
            let t = f64::from(i) * 0.1;
            let up = ships
                .upsert(
                    &(235_000_000 + i % 64),
                    Stamp::at(t).with_wall(1.0e9 + t),
                    make,
                )
                .unwrap();
            up.row.speed_kt = f64::from(i % 20);
            if i % 50 == 0 {
                publish::rows(&ships, &SHIPS, t, &mut p);
            }
        }
    });
    assert_eq!(n, 0, "allocations when objects that exist are updated");
}
