//! The streaming paths allocate nothing in steady state. Each runs after a warm-up under an allocator that counts the
//! allocations of the test's own thread, so other tests running at the same time do not disturb the count. (The counting
//! allocator needs `unsafe impl GlobalAlloc`. The library forbids `unsafe`.)

use super::impair;
use sdrkit_proto_wlan_phy::{ble, dsss, mac, ofdm, Complex32};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = COUNT.try_with(|c| c.set(c.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn count(f: impl FnOnce()) -> usize {
    let before = COUNT.with(|c| c.get());
    f();
    COUNT.with(|c| c.get()) - before
}

fn beacon(i: u8, len: usize) -> Vec<u8> {
    let mut f = Vec::new();
    mac::push_mgmt_header(
        &mut f,
        mac::MgmtSubtype::Beacon,
        mac::BROADCAST,
        [2, 0, 0, 0, 0, i],
        [2, 0, 0, 0, 0, i],
        1,
    );
    mac::push_beacon_fields(&mut f, 0, 100, 1);
    while f.len() < len {
        f.push(f.len() as u8);
    }
    mac::push_fcs(&mut f);
    f
}

#[test]
fn ble_receiver_allocates_nothing() {
    let mut x = Vec::new();
    for i in 0..4u8 {
        let mut data = Vec::new();
        ble::ad::push(&mut data, ble::ad::COMPLETE_NAME, b"alloc").unwrap();
        let mut pdu = Vec::new();
        ble::build_adv(
            &mut pdu,
            ble::PduType::AdvNonconnInd,
            &ble::Addr::public([i, 1, 2, 3, 4, 5]),
            &data,
        )
        .unwrap();
        let mut bits = Vec::new();
        ble::air_bits(&pdu, 37, &mut bits).unwrap();
        let levels: Vec<f32> = bits.iter().map(|&b| 2.0 * b as f32 - 1.0).collect();
        let mut m = sdrkit_algo_sim::modulate::Cpm::gfsk(0.5, 0.5, 4.0).unwrap();
        let mut s = Vec::new();
        m.push(&levels, &mut s);
        m.flush(&mut s);
        x.extend(impair(&s, 4e6, 20e3, 0.05, 3000, i as u64));
    }
    let mut rx = ble::AdvReceiver::new(4).unwrap();
    // Warm up on twice the signal, so the history has grown to its working size.
    for _ in 0..2 {
        rx.process(&x, |_| {});
    }
    let mut n = 0;
    let a = count(|| {
        for chunk in x.chunks(1000) {
            rx.process(chunk, |p| n += p.pdu.len());
        }
    });
    assert!(n > 0);
    assert_eq!(a, 0);
}

#[test]
fn dsss_receiver_allocates_nothing() {
    let psdu = beacon(1, 80);
    let h = dsss::PlcpHeader::for_psdu(dsss::Rate::Dqpsk2, psdu.len()).unwrap();
    let mut bits = Vec::new();
    dsss::ppdu_bits(dsss::Preamble::Short, &h, &psdu, &mut bits).unwrap();
    dsss::scramble(&mut bits, dsss::Preamble::Short);
    let mut sym = Vec::new();
    dsss::symbols(&bits, dsss::Preamble::Short, dsss::Rate::Dqpsk2, &mut sym).unwrap();
    let mut chips = Vec::new();
    dsss::spread(&sym, &mut chips);
    let s: Vec<Complex32> = chips.iter().flat_map(|&c| [c, c]).collect();
    let x = impair(&s, 22e6, 0.0, 0.05, 3000, 2);
    let mut rx = dsss::DsssReceiver::new(22).unwrap();
    rx.process(&x, |_| {});
    let mut n = 0;
    let a = count(|| {
        for chunk in x.chunks(777) {
            rx.process(chunk, |f| n += f.mpdu.len());
        }
    });
    assert!(n > 0);
    assert_eq!(a, 0);
}

#[test]
fn ofdm_receiver_allocates_nothing() {
    let mut x = Vec::new();
    for (i, r) in ofdm::Rate::ALL.iter().enumerate() {
        let mut p = Vec::new();
        ofdm::build_ppdu(&beacon(i as u8, 600), *r, 0x5D, &mut p).unwrap();
        x.extend(impair(&p, 20e6, 10e3, 1e-3, 1000, i as u64));
    }
    let mut rx = ofdm::OfdmReceiver::new();
    rx.process(&x, |_| {});
    let mut n = 0;
    let a = count(|| {
        for chunk in x.chunks(1500) {
            rx.process(chunk, |f| n += f.mpdu.len());
        }
    });
    assert_eq!(n, 8 * 600);
    assert_eq!(a, 0);
}

#[test]
fn parsers_allocate_nothing() {
    let f = beacon(3, 200);
    let mut ad = Vec::new();
    ble::ad::push_manufacturer(&mut ad, 0x004C, &[1, 2, 3]).unwrap();
    let mut pdu = Vec::new();
    ble::build_adv(
        &mut pdu,
        ble::PduType::AdvInd,
        &ble::Addr::public([1; 6]),
        &ad,
    )
    .unwrap();
    let mut sink = 0usize;
    let a = count(|| {
        for _ in 0..1000 {
            let fr = mac::Frame::parse_with_fcs(&f).unwrap();
            sink += mac::elements(fr.body).count();
            let p = ble::AdvPdu::parse(&pdu).unwrap();
            sink += ble::ad::structures(p.data().unwrap()).count();
            sink += ble::crc24(&pdu, ble::ADV_CRC_INIT) as usize & 1;
        }
    });
    assert!(sink > 0);
    assert_eq!(a, 0);
}
