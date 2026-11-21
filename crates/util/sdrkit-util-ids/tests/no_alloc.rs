//! Lookups run on every message heard, so none of them may allocate. A counting allocator checks 1,000 calls of each.

use sdrkit_util_ids::lists::IdList;
use sdrkit_util_ids::names::{IdKind, IdNames};
use sdrkit_util_ids::tones::{ctcss, dcs, selcal, selcall};
use sdrkit_util_ids::{bluetooth, icao, language, lorawan, mcc, mid, mmsi, oui, pty};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        // SAFETY: forwards the caller's request to the system allocator unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `alloc` above with the same layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn count<F: FnMut(u32)>(mut f: F) -> usize {
    let before = ALLOCATIONS.load(Ordering::SeqCst);
    for i in 0..1000 {
        f(i);
    }
    ALLOCATIONS.load(Ordering::SeqCst) - before
}

#[test]
fn lookups_do_not_allocate() {
    // Built before counting.
    let (names, _) =
        IdNames::from_text(IdKind::Decimal, "capcode,name\n2000-2099,Fire\n2042,Pump\n");
    let (list, _) = IdList::parse(IdKind::Decimal, "1234 2000-2099");
    let plmn = mcc::Plmn::new(262, 1, 2).unwrap();
    let word = dcs::Dcs::from_octal("023").unwrap().word();
    let zvei = selcall::by_name("ZVEI1").unwrap();
    let mut tones = [0.0f32; 8];

    let n = count(|i| {
        let a = 0xA0_0000 + i * 977;
        std::hint::black_box(icao::country(a));
        std::hint::black_box(icao::registration(a));
        std::hint::black_box(icao::address("N628TS"));
        std::hint::black_box(mid::country(200 + (i % 600) as u16));
        std::hint::black_box(mmsi::classify("23212~456"));
        std::hint::black_box(mmsi::Mmsi::new(232_123_456 + i).map(|m| m.is_plausible_ais()));
        std::hint::black_box(mcc::country(262));
        std::hint::black_box(mcc::operator(plmn));
        std::hint::black_box(oui::vendor([0x24, 0x0A, (i & 0xFF) as u8]));
        std::hint::black_box(bluetooth::company(i as u16));
        std::hint::black_box(bluetooth::uuid16(0x1800 + i as u16));
        std::hint::black_box(lorawan::DevAddr(0x2601_0000 + i).operator());
        std::hint::black_box(pty::name((i % 40) as u8, pty::PtyTable::Rbds));
        std::hint::black_box(language::iso639("deu"));
        std::hint::black_box(language::iso639_entry("fr"));
        std::hint::black_box(dcs::decode(word.rotate_left(i % 23) & 0x7F_FFFF));
        std::hint::black_box(ctcss::nearest(67.0 + (i % 190) as f32, 1.2));
        std::hint::black_box(selcal::Code::parse("AM-CP"));
        std::hint::black_box(selcal::nearest(500.0 + i as f32, 3.0));
        std::hint::black_box(zvei.encode("12234", &mut tones));
        std::hint::black_box(names.get_number(2000 + (i % 200) as u64));
        std::hint::black_box(list.contains_number(i as u64 * 3));
    });
    assert_eq!(n, 0, "{n} allocations in 1,000 rounds of lookups");
}
