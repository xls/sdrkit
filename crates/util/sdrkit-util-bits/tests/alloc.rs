//! Checks that the steady-state paths allocate nothing: each is run 1,000 times after its buffers are made, under an allocator
//! that counts. (The counting allocator needs `unsafe impl GlobalAlloc`; the library itself forbids `unsafe`.)

use sdrkit_util_bits::crc::catalog::{CRC16_IBM_SDLC, CRC32_ISO_HDLC};
use sdrkit_util_bits::hdlc::{hdlc_encode, hdlc_encode_without_fcs, HdlcDeframer, Unstuffer};
use sdrkit_util_bits::interleave::{DelayLanes, Permutation};
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::per::{PerIo, PerReader};
use sdrkit_util_bits::protobuf::Fields;
use sdrkit_util_bits::sync::{SoftSync, SyncWord};
use sdrkit_util_bits::{pack, seq, BitOrder, BitReader, BitWriter, Pad};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn count(f: impl FnOnce()) -> usize {
    let before = ALLOCS.load(Ordering::Relaxed);
    f();
    ALLOCS.load(Ordering::Relaxed) - before
}

// One test function, so no other test thread allocates while counting.
#[test]
fn steady_state_paths_do_not_allocate() {
    let data: Vec<u8> = (0..1500u32).map(|i| (i * 31 + 7) as u8).collect();
    let mut bits = Vec::with_capacity(data.len() * 8);
    let mut packed = Vec::with_capacity(data.len() + 1);
    let mut frame_bits = Vec::new();
    hdlc_encode(&[&data[..200]], &CRC16_IBM_SDLC, 1, &mut frame_bits);
    let mut deframer = HdlcDeframer::new(&CRC16_IBM_SDLC, 1, 400);
    let perm = Permutation::wifi(288, 6, 16).unwrap();
    let mut perm_out = vec![0i8; 288];
    let soft_in = vec![5i8; 288];
    let mut lanes = DelayLanes::forney(12, 17, true, 0u8);
    let mut lane_buf = data.clone();
    let mut sync = SyncWord::new(0x1ACF_FC1D, 32, 3).unwrap();
    let mut soft_sync = SoftSync::new(0x1ACF_FC1D, 32, 0.9).unwrap();
    let mut hits = Vec::with_capacity(64);
    let mut soft_hits = Vec::with_capacity(64);
    let mut scr = Additive::new(seq::DVB_DISPERSAL);
    let mut scr_buf = data.clone();
    let mut wbuf = Vec::with_capacity(4096);
    let mut pb = Vec::new();
    sdrkit_util_bits::protobuf::Writer::new(&mut pb)
        .uint(1, 150)
        .string(2, "testing")
        .fixed32(3, 9);
    let mut unstuffer = Unstuffer::new(sdrkit_util_bits::hdlc::ByteStuffing::KISS, 300);
    let mut kiss = Vec::new();
    sdrkit_util_bits::hdlc::ByteStuffing::KISS.frame(&data[..100], &mut kiss);
    let mut gold = [0u8; 1000];
    let mut checksum = 0u64;
    let mut frame32 = Vec::new();
    hdlc_encode(&[&data[..200]], &CRC32_ISO_HDLC, 1, &mut frame32);
    let mut deframer32 = HdlcDeframer::try_new(&CRC32_ISO_HDLC, 1, 400).unwrap();
    let mut frame_nofcs = Vec::new();
    hdlc_encode_without_fcs(&[&data[..255]], 1, &mut frame_nofcs);
    let mut deframer_nofcs = HdlcDeframer::without_fcs(1, 400).unwrap();
    let mut byte_seqs: Vec<seq::ByteSequence> = seq::BYTE_SEQUENCES
        .iter()
        .map(|k| seq::ByteSequence::named(k).unwrap())
        .collect();

    let n = count(|| {
        for _ in 0..1000 {
            checksum ^= CRC32_ISO_HDLC.checksum(&data) as u64;
            checksum ^= CRC16_IBM_SDLC.bits(&frame_bits[..100]) as u64;
            bits.clear();
            pack::unpack(&data, BitOrder::Msb, &mut bits);
            packed.clear();
            pack::pack(&bits, BitOrder::Lsb, Pad::Zero, &mut packed);
            let mut r = BitReader::new(&data);
            while r.left() >= 13 {
                checksum ^= r.u(13);
            }
            wbuf.clear();
            let mut w = BitWriter::new(&mut wbuf);
            for k in 0..500u64 {
                w.put(k, 13);
            }
            w.finish();
            for &b in &frame_bits {
                if let Some(f) = deframer.push(b) {
                    checksum ^= f.len as u64;
                }
            }
            perm.apply(&soft_in, &mut perm_out).unwrap();
            lanes.process(&mut lane_buf);
            hits.clear();
            sync.scan(&bits[..2000], &mut hits);
            soft_hits.clear();
            soft_sync.scan(&perm_out, &mut soft_hits);
            scr.reset();
            scr.apply_bytes(&mut scr_buf, BitOrder::Msb);
            for f in Fields::new(&pb) {
                checksum ^= f.unwrap().0 as u64;
            }
            let mut pr = PerReader::new(&data);
            let mut v = 0i64;
            pr.int(&mut v, 0, 1000);
            checksum ^= v as u64;
            for &b in &kiss {
                if let Some(l) = unstuffer.push(b) {
                    checksum ^= l as u64;
                }
            }
            seq::gold_3gpp(0x1234, &mut gold);
            seq::gold_dvbs2(77, 0, &mut gold).unwrap();
            seq::CCSDS.apply(&mut scr_buf, 0);
            // Lookups by name and the steady state of what they give.
            let e = sdrkit_util_bits::crc::catalog::by_key("crc16_ibm_sdlc").unwrap();
            checksum ^= e.engine.checksum(&data[..64]);
            checksum ^= seq::scrambler("dvb_dispersal").unwrap().state();
            deframer32.set_fcs(&CRC32_ISO_HDLC).unwrap();
            for &b in &frame32 {
                if let Some(f) = deframer32.push(b) {
                    checksum ^= f.len as u64 + f.fcs_ok as u64;
                }
            }
            deframer_nofcs.clear_fcs();
            for &b in &frame_nofcs {
                if let Some(f) = deframer_nofcs.push(b) {
                    checksum ^= f.len as u64;
                }
            }
            for s in byte_seqs.iter_mut() {
                s.apply(&mut scr_buf[..300]);
                s.advance(17);
            }
        }
    });
    assert_eq!(n, 0, "allocations in steady state");
    assert!(checksum != 1);
}
