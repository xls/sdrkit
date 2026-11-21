//! Checks that the steady-state paths of the OFDM and DSSS types allocate nothing: each runs 1,000 times after its buffers are
//! made, under an allocator that counts. (The counting allocator needs `unsafe impl GlobalAlloc`; it is defined in this test only.)

use sdrkit_algo_modem::dsss::{
    chips_from_bits, ChipDespreader, CodeAcquisition, CodeBank, CodeTracker, TrackerConfig,
    BARKER_11,
};
use sdrkit_algo_modem::ofdm::{
    equalise, integer_cfo, phase_fit, training_ls, CfoMetric, ChannelEstimator, ClockDrift,
    FftFramer, LagCorrelator, OfdmLayout, PilotLs, TimeFreq,
};
use sdrkit_algo_modem::Complex32;
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

fn noise(n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            Complex32::new(
                ((i * 7919) % 101) as f32 / 50.0 - 1.0,
                ((i * 104_729) % 97) as f32 / 48.0 - 1.0,
            )
        })
        .collect()
}

// One test function, so no other test thread allocates while counting.
#[test]
fn steady_state_paths_do_not_allocate() {
    // OFDM: 802.11a and DVB-T 2K.
    let wifi = OfdmLayout::ieee80211a();
    let dvbt = OfdmLayout::dvbt_2k(32).unwrap();
    let x = noise(4096);
    let mut lag = LagCorrelator::new(64, 16, 0.9, 0.5).unwrap();
    let mut metric = vec![0f32; 1024];
    let mut fr = FftFramer::new(&wifi, 4).unwrap();
    let mut fr2 = FftFramer::new(&dvbt, 16).unwrap();
    let mut pls = PilotLs::new(&wifi, true);
    let mut tf = TimeFreq::new(&dvbt).unwrap();
    let mut drift = ClockDrift::new(0.99).unwrap();
    let mut cells = vec![Complex32::default(); wifi.used()];
    let mut cells2 = vec![Complex32::default(); dvbt.used()];
    let (mut z, mut w) = (
        vec![Complex32::default(); dvbt.used()],
        vec![0f32; dvbt.used()],
    );
    let mut h = vec![Complex32::default(); 53];
    let known: Vec<Complex32> = sdrkit_algo_modem::ofdm::IEEE80211A_LTF
        .iter()
        .map(|&v| Complex32::new(v as f32, 0.0))
        .collect();
    let pil: Vec<(i32, Complex32)> = [-21, -7, 7, 21]
        .iter()
        .map(|&c| (c, Complex32::new(1.0, 0.0)))
        .collect();
    let mut data = vec![Complex32::default(); 48];
    let mut peaks = 0usize;
    let n = count(|| {
        for s in 0..1000u64 {
            lag.process(&x[..256], |_| peaks += 1);
            lag.metric_into(&x[256..512], &mut metric).unwrap();
            fr.set_freq(1e-3);
            fr.set_timing((s % 3) as f64 * 0.1);
            fr.demodulate(&x, 20, s * 80, &mut cells).unwrap();
            integer_cfo(fr.bins(), &pil, 4, CfoMetric::Differential).unwrap();
            pls.push(s, &cells).unwrap();
            let fit = phase_fit(&wifi, s, &cells, pls.h()).unwrap();
            fit.remove(wifi.first(), &mut cells);
            drift.push(s as f64 * 80.0, fit.timing(64));
            wifi.take_data(s, &cells, &mut data).unwrap();
            training_ls(&[&cells, &cells], &known, &mut h).unwrap();
            equalise(pls.cells(), pls.h(), 0.1, &mut z[..53], &mut w[..53]).unwrap();
            if s % 10 == 0 {
                fr2.demodulate(&x, 100, s * 2112, &mut cells2).unwrap();
                tf.push(s / 10, &cells2).unwrap();
                equalise(tf.cells(), tf.h(), 0.1, &mut z, &mut w).unwrap();
            }
        }
    });
    assert_eq!(n, 0, "OFDM allocated {n} times");

    // DSSS.
    let code: Vec<f32> = BARKER_11.iter().map(|&c| c as f32).collect();
    let mut desp = ChipDespreader::new(&code, 2.0).unwrap();
    let mut out = Vec::with_capacity(1024);
    let walsh: Vec<Vec<f32>> = (0..4u32)
        .map(|r| {
            (0..8u32)
                .map(|c| {
                    if (r & c).count_ones() % 2 == 0 {
                        1.0
                    } else {
                        -1.0
                    }
                })
                .collect()
        })
        .collect();
    let refs: Vec<&[f32]> = walsh.iter().map(|c| c.as_slice()).collect();
    let bank = CodeBank::new(&refs, 2.0).unwrap();
    let mut e = [0f32; 4];
    let mut bits = [0u8; 1023];
    sdrkit_util_bits::seq::gps_ca(3, &mut bits).unwrap();
    let mut ca = Vec::new();
    chips_from_bits(&bits, &mut ca);
    let mut acq = CodeAcquisition::new(&ca[..127], 2.0, 3, 2).unwrap();
    let mut trk = CodeTracker::new(&ca, 2.0, TrackerConfig::new(0.02)).unwrap();
    let mut corr = Vec::with_capacity(64);
    let n = count(|| {
        for i in 0..1000usize {
            out.clear();
            desp.process(&x[..440], &mut out);
            desp.despread_at(&x, (i % 7) as f64 * 0.3).unwrap();
            bank.correlate_at(&x, 1.5, &mut e).unwrap();
            if i % 20 == 0 {
                acq.accumulate(&x[..254]).unwrap();
                acq.result().unwrap();
            }
            corr.clear();
            trk.set_carrier(1e-3);
            trk.process(&x[..2046], &mut corr);
        }
    });
    assert_eq!(n, 0, "DSSS allocated {n} times");
}
