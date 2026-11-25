//! Allocations on the receive path of the stacks, counted by an allocator that counts only on the thread that enables it, so
//! other tests that run at the same time do not change the count.

use sdrkit_core::{Complex32, InputInfo, Output, StackConfig, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_graph::Graph;
use sdrkit_stack_drones::family::REPORT_S;
use std::alloc::{GlobalAlloc, Layout, System as Heap};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn note() {
    // `try_with` fails only while the thread ends. Nothing is counted then.
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: every call goes to the system allocator with the same arguments. `note` only touches thread-local cells.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        note();
        // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
        unsafe { Heap.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `Heap` with layout `l`.
        unsafe { Heap.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        note();
        // SAFETY: `p` came from `Heap` with layout `l`.
        unsafe { Heap.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The allocations that `f` makes on this thread.
fn allocations(f: impl FnOnce()) -> usize {
    ALLOCS.with(|n| n.set(0));
    COUNTING.with(|on| on.set(true));
    f();
    COUNTING.with(|on| on.set(false));
    ALLOCS.with(|n| n.get())
}

const BLOCK: usize = 16_384;

/// The results of a run after its warm-up.
#[derive(Debug, Default)]
struct Tally {
    /// Blocks with no report tick and no copy of results, and their allocations.
    quiet: usize,
    quiet_allocs: usize,
    /// Blocks with a report tick or a copy of results, and their allocations.
    busy: usize,
    busy_allocs: usize,
}

/// Feeds `seconds` of `x` (repeated) at the default rate of stack `d` to its receiver in blocks of [`BLOCK`] samples, and sorts
/// the blocks after the first `warm_s` seconds.
fn run(d: &'static StackDescriptor, x: &[Complex32], warm_s: f64, seconds: f64) -> Tally {
    let rate = d.tuning.rate_hz;
    let mut rx = (d.make_receiver)(&StackConfig::new(Harness::new(d).values().unwrap())).unwrap();
    let dial = d.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, dial + d.tuning.tune_offset_hz, dial).recording())
        .unwrap();
    let mut out = Output::default();
    let block_s = BLOCK as f64 / rate;
    // A report comes at each tick of the graph clock, and the graph copies the changed results every merge interval.
    let busy_s = Graph::MERGE_INTERVAL_S + block_s;
    let blocks = ((warm_s + seconds) / block_s) as usize;
    let mut t = Tally::default();
    let mut piece = vec![Complex32::default(); BLOCK];
    for k in 0..blocks {
        for (i, s) in piece.iter_mut().enumerate() {
            *s = x[(k * BLOCK + i) % x.len()];
        }
        let n = allocations(|| rx.feed(&piece, &mut out));
        out.clear();
        let end = (k + 1) as f64 * block_s;
        if end < warm_s {
            continue;
        }
        let since_tick = end % REPORT_S;
        if since_tick <= busy_s {
            t.busy += 1;
            t.busy_allocs += n;
        } else {
            t.quiet += 1;
            t.quiet_allocs += n;
        }
    }
    t
}

/// White noise at the default rate of stack `d`.
fn noise(d: &StackDescriptor, seconds: f64) -> Vec<Complex32> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    (0..(d.tuning.rate_hz * seconds) as usize)
        .map(|_| Complex32::new(rng.normal() as f32, rng.normal() as f32) * 0.01)
        .collect()
}

/// Once warm, a block of noise without a report allocates nothing in the receiver of `d`. The test prints what a report costs.
fn quiet_on_noise(d: &'static StackDescriptor) {
    let t = run(d, &noise(d, 1.0), 2.0, 3.0);
    eprintln!(
        "{} on noise: {} quiet blocks: {} allocations; {} blocks with a report: {} allocations ({:.1} a block)",
        d.id,
        t.quiet,
        t.quiet_allocs,
        t.busy,
        t.busy_allocs,
        t.busy_allocs as f64 / t.busy.max(1) as f64
    );
    assert!(t.quiet > 20, "{}: only {} quiet blocks", d.id, t.quiet);
    assert_eq!(t.quiet_allocs, 0, "{}: {t:?}", d.id);
}

/// Once warm, a block of noise without a report allocates nothing in the receiver of each stack, run through its stack graph.
#[test]
fn every_stack_allocates_nothing_between_reports_once_warm() {
    for d in sdrkit_stack_drones::descriptors() {
        quiet_on_noise(d);
    }
}

/// The Remote ID packets of a known drone: drone 1 on Bluetooth LE (its Location message) and drone 2 in a Wi-Fi beacon (its
/// Message Pack), as the receive paths make them.
fn rid_packets() -> [sdrkit_stack_drones::remote_id::RidPacket; 2] {
    use sdrkit_stack_drones::remote_id::generator::{drone1_messages, drone2_messages, DRONE2};
    use sdrkit_stack_drones::remote_id::messages::*;
    use sdrkit_stack_drones::remote_id::RidPacket;
    let sd = ble_service_data(7, &encode_message(&drone1_messages()[2], 2));
    let b = from_ble_adv_data(&sd).unwrap();
    let el = wifi_element(3, &encode_pack(&drone2_messages(), 2));
    let (address, w) = from_wifi_frame(&beacon(DRONE2, "RID-DJI-2", 1, &el)).unwrap();
    let packet = |transport, link, address: String, c: Carried| RidPacket {
        transport,
        channel: 38,
        address,
        counter: c.counter,
        messages: c.messages,
        link,
        payload: c.payload,
        beacon: c.beacon,
        refused: c.refused,
        out_of_range: c.out_of_range,
        level_dbfs: None,
        cfo_hz: 0.0,
        snr_db: 20.0,
    };
    [
        packet(Transport::Ble, Link::Ble4, "C6:12:34:56:78:9A".into(), b),
        packet(Transport::WiFi, Link::WifiBeacon, address, w),
    ]
}

/// Once a drone is known, its packet allocates in the drone table only what its message log lines need (the address and the
/// summary of each message): the trust check, the verdict and the reasons allocate nothing. The carrier's payload allocates
/// nothing beyond the decoded messages.
#[test]
fn a_known_drones_packet_allocates_only_its_log_lines() {
    use sdrkit_stack_drones::remote_id::messages::{decode_payload, from_ble_adv_data, summary};
    use sdrkit_stack_drones::remote_id::table::DroneTable;
    use sdrkit_stack_drones::remote_id::trust::{Heard, Trust};
    let packets = rid_packets();
    let mut table = DroneTable::default();
    let mut t = 0.0;
    for _ in 0..300 {
        for p in &packets {
            t += 0.01;
            table.take(t, p);
        }
    }
    // 2 s with no event: a drone with a position sends one every 5 s.
    let (mut take, mut log) = (0, 0);
    for _ in 0..200 {
        for p in &packets {
            t += 0.005;
            take += allocations(|| table.take(t, p));
            // Each log line holds the address and the summary of its message.
            log += allocations(|| {
                for v in &p.messages {
                    std::hint::black_box(p.address.clone());
                    std::hint::black_box(summary(&v.msg));
                }
            });
        }
    }
    let cfg = remoteid::TrustConfig::default();
    let mut trust = Trust::new();
    let mut judged = 0;
    for k in 0..400u64 {
        let p = &packets[(k % 2) as usize];
        let h = Heard {
            link: p.link,
            counter: p.counter.wrapping_add(k as u8),
            payload: &p.payload,
            now_ms: 10 * k,
            unix_ms: Some(1_795_000_000_000 + 10 * k),
            beacon: p.beacon,
        };
        // The first packets may grow the text of the reasons once.
        let n = allocations(|| {
            trust.observe(&cfg, &h);
        });
        if k >= 100 {
            judged += n;
        }
    }
    let sd = sdrkit_stack_drones::remote_id::messages::ble_service_data(
        7,
        packets[0].payload.as_slice(),
    );
    let carried = allocations(|| {
        std::hint::black_box(from_ble_adv_data(&sd));
    });
    let decoded = allocations(|| {
        std::hint::black_box(decode_payload(packets[0].payload.as_slice()).unwrap());
    });
    eprintln!(
        "remote_id: {:.2} allocations per packet in the table, {:.2} for the log lines; trust check {judged} in 300 \
         packets; BLE carrier {carried}, its messages {decoded}",
        take as f64 / 400.0,
        log as f64 / 400.0
    );
    assert_eq!(take, log);
    assert_eq!(judged, 0);
    assert_eq!(carried, decoded);
}
