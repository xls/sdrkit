//! The calibration workload. The cost of a stage is stored as calibration units per second of signal. A device learns
//! once how many units per second one of its cores delivers ([`units_per_second`]); the cost of a stage divided by that
//! number is the share of a core that the stage needs on this device.
//!
//! One unit is one pass of a fixed mix of what the stacks do: a FIR filter of complex samples, a transform of 1024
//! points, a Viterbi decoder of the K=7 code of DVB, and a table-driven de-interleave. The filter, the transform and the
//! decoder run on the default backends of their kernel families, as the stacks of this build do: on 64-bit ARM the NEON
//! forms, on Apple computers the transform of Accelerate. The sizes are part of the unit and never change.

use sdrkit_algo_dsp::fft::{FftPlan, BACKENDS as FFT};
use sdrkit_algo_dsp::fir::{FirBackend, BACKENDS as FIR};
use sdrkit_algo_fec::conv::{codes::DVB_K7, Ends, Viterbi};
use sdrkit_core::Complex32;
use std::hint::black_box;
use std::time::{Duration, Instant};

const FIR_TAPS: usize = 32;
const FIR_OUT: usize = 1024;
const FFT_POINTS: usize = 1024;
const STEPS: usize = 512;
const BYTES: usize = 8192;
/// The units of one timed batch.
const BATCH: u32 = 4;

/// The buffers and kernels of the workload.
pub struct Workload {
    /// The samples of the filter as pairs of values, the taps laid out by the backend, and its output.
    signal: Vec<f32>,
    fir: &'static dyn FirBackend,
    taps: Vec<f32>,
    filtered: Vec<f32>,
    spectrum: Vec<Complex32>,
    fft: Box<dyn FftPlan>,
    viterbi: Viterbi,
    llr: Vec<i8>,
    bits: Vec<u8>,
    order: Vec<u16>,
    bytes: Vec<u8>,
    spread: Vec<u8>,
}

impl std::fmt::Debug for Workload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workload")
            .field("fir", &self.fir.name())
            .finish_non_exhaustive()
    }
}

impl Default for Workload {
    fn default() -> Self {
        let mut seed = 1u32;
        let mut rand = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 16) as u16
        };
        let mut value = || f32::from(rand()) / 65_536.0 - 0.5;
        let signal = (0..2 * (FIR_OUT + FIR_TAPS)).map(|_| value()).collect();
        let raw: Vec<f32> = (0..FIR_TAPS).map(|_| value()).collect();
        let (fir, taps) = FIR
            .build(|b| b.prepare(&raw, 2).map(|p| (b, p)))
            .expect("the plain filter takes any taps");
        let fft = FFT
            .build(|b| b.plan(FFT_POINTS, false))
            .expect("the plain transform takes 1024 points");
        let viterbi = Viterbi::new(DVB_K7, STEPS).expect("K=7 is supported");
        let llr = (0..2 * STEPS).map(|_| (rand() & 0xff) as u8 as i8).collect();
        // A permutation: a step that is prime to the length.
        let order = (0..BYTES).map(|i| ((i * 4099) % BYTES) as u16).collect();
        let bytes = (0..BYTES).map(|_| rand() as u8).collect();
        Workload {
            signal,
            fir,
            taps,
            filtered: vec![0.0; 2 * FIR_OUT],
            spectrum: vec![Complex32::default(); FFT_POINTS],
            fft,
            viterbi,
            llr,
            bits: vec![0; STEPS],
            order,
            bytes,
            spread: vec![0; BYTES],
        }
    }
}

impl Workload {
    /// Runs one unit and returns a number that depends on every result.
    pub fn unit(&mut self) -> f32 {
        self.fir
            .run(&self.taps, 2, &self.signal, 1, &mut self.filtered);
        let (pairs, _) = self.filtered.as_chunks::<2>();
        for (s, &[re, im]) in self.spectrum.iter_mut().zip(pairs) {
            *s = Complex32::new(re, im);
        }
        self.fft.process(&mut self.spectrum);
        let decoded = self
            .viterbi
            .decode_only(&self.llr, Ends::Open, &mut self.bits)
            .is_ok();
        for (i, &o) in self.order.iter().enumerate() {
            self.spread[usize::from(o)] = self.bytes[i];
        }
        self.spectrum[1].re
            + f32::from(self.bits[STEPS / 2])
            + f32::from(self.spread[BYTES / 2])
            + f32::from(u8::from(decoded))
    }

    /// The backends of the filter and the transform that the workload runs.
    pub fn backends(&self) -> (&'static str, &'static str) {
        (self.fir.name(), FFT.active())
    }
}

/// The calibration units that the calling thread runs per second, measured for about `budget`. The fastest batch
/// counts, so that other work on the device does not lower the number.
pub fn units_per_second(budget: Duration) -> f64 {
    let mut w = Workload::default();
    black_box(w.unit());
    let start = Instant::now();
    let mut best = f64::INFINITY;
    let mut batches = 0;
    while batches < 3 || start.elapsed() < budget {
        let t0 = Instant::now();
        for _ in 0..BATCH {
            black_box(w.unit());
        }
        best = best.min(t0.elapsed().as_secs_f64());
        batches += 1;
    }
    f64::from(BATCH) / best.max(1e-9)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_gives_the_same_result_each_time_on_the_default_kernels() {
        let (mut a, mut b) = (Workload::default(), Workload::default());
        let first = a.unit();
        assert!(first.is_finite());
        assert_eq!(first.to_bits(), b.unit().to_bits());
        assert_eq!(a.unit().to_bits(), first.to_bits());
        let (fir, fft) = a.backends();
        assert_eq!(fir, FIR.active());
        #[cfg(target_arch = "aarch64")]
        assert_ne!(fir, FIR.plain().name(), "the vector filter on 64-bit ARM");
        assert!(!fft.is_empty());
        assert!(units_per_second(Duration::from_millis(5)) > 10.0);
    }
}
