//! OFDM: the pieces every OFDM receiver shares, driven by a layout table, so DVB-T, ISDB-T, DRM, DAB, 802.11a/g, LTE and NR keep
//! only their tables.
//!
//! | Step | Type | What it does |
//! |---|---|---|
//! | describe | [`OfdmLayout`], [`PilotPattern`] | FFT size, cyclic prefix per symbol, used carriers, nulls, reserved carriers, known cells per symbol; named layouts [`OfdmLayout::ieee80211a`], [`OfdmLayout::dvbt_2k`] and [`OfdmLayout::dvbt_8k`] |
//! | find symbols | [`LagCorrelator`] | the cyclic prefix (or a repeated preamble) by a normalised lag correlation, with the fractional carrier offset from its phase |
//! | frame start (DAB) | [`quietest_window`], [`ImpulseResponse`], [`place_window`], [`DelayCleaner`] | the null symbol; the impulse response against the known first symbol (timing and integer offset); the FFT window for a single-frequency network; a channel estimate cleaned in the delay domain |
//! | integer offset | [`integer_cfo`] | the carrier offset in whole carriers from known cells: coherent, differential or power metric |
//! | demodulate | [`FftFramer`] | carrier offset removed with a continuous phase (also across a change of the offset, [`FftFramer::set_freq_continuous`]), FFT with an early window, its phase ramp removed per carrier |
//! | track | [`phase_fit`], [`ClockDrift`] | common phase and timing slope by weighted least squares over the pilots; the sampling clock error |
//! | estimate | [`ChannelEstimator`]: [`PilotLs`], [`TimeFreq`]; [`training_ls`]; [`GridInterp`]; [`PilotTimeInterp`], [`delay_window`] | the channel and the noise per cell from pilots or training symbols; a pilot grid filled by a windowed sinc with the mean delay taken out; scattered pilots interpolated in time with an averaging gain from the measured noise and change, and the delay window of the impulse response |
//! | equalise | [`equalise`] | zero forcing with weights `|H|^2 / sigma^2` for `Constellation::demap_block_weighted` |
//! | framed modem | [`FrameLayout`], [`FrameModulator`], [`FrameDemodulator`] | a frame of symbols with known cells given as data (DRM, HD Radio, CDR): the modulator, and a streaming demodulator that finds the frame start, the carrier and clock offsets and a mirrored spectrum, and emits one [`CellFrame`] of equalised cells per frame |
//! | receive (streaming) | [`CpSync`], [`OfdmReceiver`], [`Estimator`] | samples to whole symbols with the offset removed (lock, tracking, frame starts); whole symbols to equalised cells with their weights, any estimator, chosen by name or by type |
//! | find the structure | [`StructureDetector`], [`Numerology`] | whether a stream holds CP-OFDM of a numerology (prefix correlation folded over the symbol period), its symbol timing, carrier offset and frame shape |
//!
//! [`OfdmLayout::by_name`] and [`Estimator::by_name`] pick a layout and an estimator by name, for settings.
//!
//! Carriers are signed indices from DC. Everything is sized when it is made; nothing allocates per symbol, and no library `sin`,
//! `cos` or `atan2` runs per sample (rotators re-anchored every 64 steps, `sdrkit_algo_dsp::approx`).
//!
//! ```
//! use sdrkit_algo_modem::ofdm::{equalise, ChannelEstimator, FftFramer, OfdmLayout, PilotLs};
//! use sdrkit_algo_modem::Complex32;
//! let layout = OfdmLayout::ieee80211a();
//! let mut framer = FftFramer::new(&layout, 4).unwrap();
//! let mut est = PilotLs::new(&layout, false);
//! let x = vec![Complex32::new(0.1, 0.0); 80]; // one symbol of samples
//! let mut cells = vec![Complex32::default(); layout.used()];
//! framer.demodulate(&x, 16, 0, &mut cells).unwrap();
//! est.push(0, &cells).unwrap();
//! let (mut z, mut w) = (vec![Complex32::default(); layout.used()], vec![0f32; layout.used()]);
//! equalise(est.cells(), est.h(), est.noise().max(1e-6), &mut z, &mut w).unwrap();
//! ```

mod estimate;
mod frame;
mod framed;
mod framer;
mod gridwindow;
mod layout;
mod receiver;
pub mod slots;
mod structure;
mod sync;
mod timeinterp;

pub use estimate::{
    equalise, phase_fit, training_ls, ChannelEstimator, ClockDrift, GridInterp, PhaseFit, PilotLs,
    TimeFreq, MAX_GRID_STEP,
};
pub use frame::{place_window, quietest_window, DelayCleaner, ImpulseResponse, QuietWindow};
pub use framed::{CellFrame, FrameDemodulator, FrameLayout, FrameModulator, FrameState};
pub(crate) use framer::rotator;
pub use framer::FftFramer;
pub use gridwindow::{WindowedGridInterp, MAX_WINDOW_HALF};
pub use layout::{
    ieee80211_pilot_polarity, OfdmLayout, Pilot, PilotPattern, DVBT_2K_CONTINUAL, DVBT_2K_TPS,
    DVBT_8K_CONTINUAL, DVBT_8K_TPS, IEEE80211A_LTF, IEEE80211A_STF,
};
pub use receiver::{CpSync, CpSyncConfig, CpSyncEvent, Estimator, OfdmReceiver, OfdmSymbol};
pub use structure::{
    FrameShape, Numerology, StructureConfig, StructureDetector, StructureReport, FRAME_BINS,
    FRAME_BLOCK,
};
pub use sync::{integer_cfo, CfoMetric, IntegerCfo, LagCorrelator, LagPeak};
pub use timeinterp::{
    continual_mean, delay_window, DelayWindowRule, PilotTimeConfig, PilotTimeInterp, POINT_HISTORY,
};

use crate::Complex32;
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark};

/// DVB-T 2K at 64/7 MHz with guard 1/4: framer, time-frequency estimate and equaliser per symbol.
fn dvbt_symbol(seconds: f64) -> BenchResult {
    let layout = OfdmLayout::dvbt_2k(4).unwrap_or_else(|_| unreachable!());
    let mut fr = FftFramer::new(&layout, 32).unwrap_or_else(|_| unreachable!());
    fr.set_freq(1e-4);
    let mut est = TimeFreq::new(&layout).unwrap_or_else(|_| unreachable!());
    let n = layout.symbol_len(0);
    let x: Vec<Complex32> = (0..n)
        .map(|i| Complex32::new(((i * 7) % 13) as f32 - 6.0, ((i * 5) % 11) as f32 - 5.0))
        .collect();
    let used = layout.used();
    let (mut cells, mut z, mut w) = (
        vec![Complex32::default(); used],
        vec![Complex32::default(); used],
        vec![0f32; used],
    );
    let mut s = 0u64;
    let rate = 64e6 / 7.0;
    measure(seconds, n, n as f64 / rate, || {
        let _ = fr.demodulate(&x, 512, s * n as u64, &mut cells);
        let _ = est.push(s, &cells);
        let _ = equalise(est.cells(), est.h(), 0.01, &mut z, &mut w);
        s += 1;
    })
}

/// 802.11a at 20 MHz: framer, pilot LS estimate and equaliser per symbol.
fn wifi_symbol(seconds: f64) -> BenchResult {
    let layout = OfdmLayout::ieee80211a();
    let mut fr = FftFramer::new(&layout, 4).unwrap_or_else(|_| unreachable!());
    let mut est = PilotLs::new(&layout, true);
    let x: Vec<Complex32> = (0..80)
        .map(|i| Complex32::new(((i * 7) % 13) as f32 - 6.0, 1.0))
        .collect();
    let (mut cells, mut z, mut w) = (
        vec![Complex32::default(); 53],
        vec![Complex32::default(); 53],
        vec![0f32; 53],
    );
    let reps = 256;
    measure(seconds, 80 * reps, 80.0 * reps as f64 / 20e6, || {
        for s in 0..reps as u64 {
            let _ = fr.demodulate(&x, 16, s * 80, &mut cells);
            let _ = est.push(s, &cells);
            let _ = equalise(est.cells(), est.h(), 0.01, &mut z, &mut w);
        }
    })
}

/// The cyclic-prefix lag correlator over DVB-T 2K samples at 64/7 MHz.
fn lag_correlator(seconds: f64) -> BenchResult {
    let mut c = LagCorrelator::new(2048, 512, 0.6, 0.3).unwrap_or_else(|_| unreachable!());
    let x: Vec<Complex32> = (0..65_536)
        .map(|i| Complex32::new(((i * 7) % 13) as f32 - 6.0, ((i * 3) % 5) as f32))
        .collect();
    let mut peaks = 0usize;
    measure(seconds, x.len(), x.len() as f64 / (64e6 / 7.0), || {
        c.process(&x, |_| peaks += 1)
    })
}

/// 802.11a-like symbols (80 samples, the pattern of [`CpSync`]'s example) at 20 MHz: [`CpSync`] locked and cutting symbols,
/// then [`OfdmReceiver`] with the pilot LS estimator, per sample.
fn wifi_stream(seconds: f64) -> BenchResult {
    let layout = OfdmLayout::ieee80211a();
    let mut x = Vec::with_capacity(80 * 820);
    for s in 0..820u32 {
        let body: Vec<Complex32> = (0..64u32)
            .map(|i| {
                let v = (i * 97 + s * 31) % 13;
                Complex32::new(v as f32 - 6.0, ((i * 7 + s) % 5) as f32 - 2.0)
            })
            .collect();
        x.extend_from_slice(&body[48..]);
        x.extend_from_slice(&body);
    }
    let mut sync = CpSync::new(&layout, CpSyncConfig::default()).unwrap_or_else(|_| unreachable!());
    let est = Estimator::by_name("pilot_ls", &layout).unwrap_or_else(|_| unreachable!());
    let mut rx = OfdmReceiver::new(&layout, est, 2, 0).unwrap_or_else(|_| unreachable!());
    let (mut out, mut events) = (Vec::with_capacity(x.len()), Vec::with_capacity(1024));
    let mut cells = 0usize;
    measure(seconds, x.len(), x.len() as f64 / 20e6, || {
        out.clear();
        events.clear();
        sync.process(&x, &mut out, &mut events);
        rx.process(&out, |s| cells += s.cells.len());
    })
}

/// The DVB-T2 P1 test: a lag correlator of 1024 samples over 542 with the second copy one carrier higher, at 64/7 MHz.
fn lag_shifted(seconds: f64) -> BenchResult {
    let mut c = LagCorrelator::with_shift(1024, 542, 1.0 / 1024.0, 0.6, 0.3)
        .unwrap_or_else(|_| unreachable!());
    let x: Vec<Complex32> = (0..65_536)
        .map(|i| Complex32::new(((i * 7) % 13) as f32 - 6.0, ((i * 3) % 5) as f32))
        .collect();
    let mut peaks = 0usize;
    measure(seconds, x.len(), x.len() as f64 / (64e6 / 7.0), || {
        c.process(&x, |_| peaks += 1)
    })
}

/// 802.11a-like symbols at 20 MHz through [`CpSync`] with the clock loop on.
fn cp_sync_clock(seconds: f64) -> BenchResult {
    let layout = OfdmLayout::ieee80211a();
    let mut x = Vec::with_capacity(80 * 820);
    for s in 0..820u32 {
        let body: Vec<Complex32> = (0..64u32)
            .map(|i| {
                let v = (i * 97 + s * 31) % 13;
                Complex32::new(v as f32 - 6.0, ((i * 7 + s) % 5) as f32 - 2.0)
            })
            .collect();
        x.extend_from_slice(&body[48..]);
        x.extend_from_slice(&body);
    }
    let mut sync = CpSync::new(&layout, CpSyncConfig::default()).unwrap_or_else(|_| unreachable!());
    sync.enable_clock_loop(0.01, 1000.0)
        .unwrap_or_else(|_| unreachable!());
    let (mut out, mut events) = (Vec::with_capacity(x.len()), Vec::with_capacity(1024));
    measure(seconds, x.len(), x.len() as f64 / 20e6, || {
        out.clear();
        events.clear();
        sync.process(&x, &mut out, &mut events);
    })
}

/// The ISDB-T mode 3 pilot grid (every third of 5617 carriers) filled by the windowed sinc, one symbol per 1.008 ms.
fn grid_interp(seconds: f64) -> BenchResult {
    let gi = GridInterp::new(3, 4).unwrap_or_else(|_| unreachable!());
    let grid: Vec<Complex32> = (0..1873)
        .map(|j| Complex32::from_polar(1.0, -0.537 * (3 * j) as f32))
        .collect();
    let mut out = vec![Complex32::default(); 5617];
    measure(seconds, 1, 1.008e-3, || {
        let _ = gi.fill(&grid, &mut out);
    })
}

/// DAB mode I: the PRS impulse response and the delay-domain cleaning of one estimate, one frame per 96 ms.
fn dab_frame(seconds: f64) -> BenchResult {
    let known: Vec<(i32, Complex32)> = (-768..=768)
        .filter(|&k| k != 0)
        .map(|k| (k, Complex32::new(if k % 3 == 0 { 1.0 } else { -1.0 }, 0.0)))
        .collect();
    let bins: Vec<usize> = known
        .iter()
        .map(|&(k, _)| k.rem_euclid(2048) as usize)
        .collect();
    let mut ir = ImpulseResponse::new(2048, &known).unwrap_or_else(|_| unreachable!());
    let mut cl = DelayCleaner::new(2048, &bins).unwrap_or_else(|_| unreachable!());
    let x: Vec<Complex32> = (0..2048)
        .map(|i| Complex32::new(((i * 7) % 13) as f32 - 6.0, ((i * 3) % 5) as f32))
        .collect();
    let mut h: Vec<Complex32> = x[..1536].to_vec();
    measure(seconds, 1, 0.096, || {
        let _ = ir.compute(&x, 0);
        let _ = ir.peak();
        let _ = cl.make_mask(&h, 12.0, 2, 0.25);
        let _ = cl.clean(&mut h);
    })
}

/// The OFDM benchmarks.
pub(crate) fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "modem.ofdm_80211a_stream_20m",
            run: wifi_stream,
        },
        Benchmark {
            name: "modem.ofdm_dvbt_2k_symbol_9m14",
            run: dvbt_symbol,
        },
        Benchmark {
            name: "modem.ofdm_80211a_symbol_20m",
            run: wifi_symbol,
        },
        Benchmark {
            name: "modem.ofdm_lag_correlator_9m14",
            run: lag_correlator,
        },
        Benchmark {
            name: "modem.ofdm_lag_shifted_p1_9m14",
            run: lag_shifted,
        },
        Benchmark {
            name: "modem.ofdm_cp_sync_clock_20m",
            run: cp_sync_clock,
        },
        Benchmark {
            name: "modem.ofdm_grid_interp_isdbt_mode3",
            run: grid_interp,
        },
        Benchmark {
            name: "modem.ofdm_dab_impulse_and_clean",
            run: dab_frame,
        },
    ]
}
