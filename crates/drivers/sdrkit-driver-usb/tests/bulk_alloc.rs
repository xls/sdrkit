//! A bulk IN stream of 100 MiB through the scripted bus arrives whole and in order, and allocates nothing per
//! transfer once it runs; neither do control reads into the caller's buffer nor a queued bulk OUT writer.

use sdrkit_driver_usb::{
    BulkItem, BulkKind, FakeUsb, Payload, Script, Setup, Step, UsbBus, UsbDeviceInfo, UsbError,
    UsbId,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: the layout is passed on as the caller supplied it.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The bytes of one block of the stream: a sequence that does not repeat within the block.
fn block() -> Vec<u8> {
    let mut x = 0x1234_5678u32;
    (0..1 << 16)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 24) as u8
        })
        .collect()
}

#[test]
fn the_transfer_paths_arrive_whole_without_allocation() {
    const BLOCKS: u32 = 1600;
    const TRANSFER: usize = 16 * 1024;
    const READS: usize = 4000;
    const WRITES: usize = 4096;
    let data = block();
    let total = data.len() as u64 * u64::from(BLOCKS);
    let dev = UsbId(1);
    let mut script = Script::new();
    script.devices.push(UsbDeviceInfo::new(dev, 0x0bda, 0x2838));
    script
        .step(Step::Open {
            device: dev,
            reply: Ok(()),
        })
        .step(Step::Claim {
            device: dev,
            interface: 0,
            reply: Ok(()),
        })
        .bulk(BulkItem {
            device: dev,
            endpoint: 0x81,
            after: Duration::ZERO,
            repeat: BLOCKS,
            kind: BulkKind::Data(Payload::Full(data.clone())),
        });
    let read = Setup::vendor(0x10, 0, 0).input();
    for i in 0..READS {
        script.step(Step::ControlIn {
            device: dev,
            setup: read,
            length: 4,
            reply: Ok((i as u32).to_le_bytes().to_vec()),
        });
    }
    // The bytes of each write are those of the next transfer-sized piece of the block, kept by length and CRC.
    let pieces: Vec<Payload> = data.chunks(TRANSFER).map(|c| Payload::of(c, 0)).collect();
    for i in 0..WRITES {
        script.step(Step::BulkOut {
            device: dev,
            endpoint: 0x02,
            data: pieces[i % pieces.len()].clone(),
            reply: Ok(TRANSFER),
        });
    }
    let bus = FakeUsb::new(script);
    let mut d = bus.open(dev).unwrap();
    d.claim(0).unwrap();
    let mut s = d.bulk_in(0x81, TRANSFER, 8).unwrap();

    let first = s.next(Duration::from_secs(1)).unwrap();
    assert_eq!(first, &data[..TRANSFER]);
    let mut got = TRANSFER as u64;
    let before = ALLOCS.load(Ordering::Relaxed);
    let mut wrong = 0u64;
    while got < total {
        let chunk = s.next(Duration::from_secs(1)).unwrap();
        let at = (got % data.len() as u64) as usize;
        if chunk != &data[at..at + chunk.len()] {
            wrong += 1;
        }
        got += chunk.len() as u64;
    }
    let allocs = ALLOCS.load(Ordering::Relaxed) - before;
    assert_eq!(got, total);
    assert_eq!(wrong, 0);
    assert_eq!(allocs, 0, "allocations while the stream ran");
    assert_eq!(s.next(Duration::from_millis(5)), Err(UsbError::Timeout));

    // Control reads into the caller's buffer.
    let mut reply = [0u8; 4];
    assert_eq!(
        d.control_in(read, &mut reply, Duration::from_secs(1)),
        Ok(4)
    );
    let before = ALLOCS.load(Ordering::Relaxed);
    let mut wrong = 0;
    for i in 1..READS {
        let n = d
            .control_in(read, &mut reply, Duration::from_secs(1))
            .unwrap();
        if n != 4 || u32::from_le_bytes(reply) != i as u32 {
            wrong += 1;
        }
    }
    let allocs = ALLOCS.load(Ordering::Relaxed) - before;
    assert_eq!(wrong, 0);
    assert_eq!(allocs, 0, "allocations while control reads ran");

    // A queued bulk OUT writer: fill in place, submit, take results in order.
    let mut w = d.bulk_out_queue(0x02, TRANSFER, 8).unwrap();
    let pieces = data.len() / TRANSFER;
    let mut submitted = 0usize;
    let mut sent = 0u64;
    let mut failed = 0;
    let write = |w: &mut dyn sdrkit_driver_usb::BulkOut, k: usize| {
        let b = w.buffer().expect("a free buffer");
        let at = (k % pieces) * TRANSFER;
        b.copy_from_slice(&data[at..at + TRANSFER]);
        w.submit(TRANSFER).unwrap();
    };
    write(&mut *w, 0);
    submitted += 1;
    let before = ALLOCS.load(Ordering::Relaxed);
    while submitted < WRITES {
        if w.in_flight() == 8 {
            match w.completed(Duration::from_secs(1)) {
                Some(Ok(n)) => sent += n as u64,
                _ => failed += 1,
            }
        }
        write(&mut *w, submitted);
        submitted += 1;
    }
    sent += w.flush(Duration::from_secs(1)).unwrap();
    let allocs = ALLOCS.load(Ordering::Relaxed) - before;
    assert_eq!(failed, 0);
    assert_eq!(sent, (WRITES * TRANSFER) as u64);
    assert_eq!(allocs, 0, "allocations while the writer ran");
    bus.check().unwrap();
}
