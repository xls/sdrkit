//! Removal of the DC offset of a converter: a slow estimate of the mean of I and of Q, subtracted from the samples. A
//! radio without a correction of its own shows the offset as a carrier at its centre frequency.
//!
//! The estimate moves in steps of [`BLOCK`] samples with the time constant [`TAU`] samples: a one-pole high-pass with
//! its corner at rate / (2π · [`TAU`]), 5.8 Hz at 2.4 Msps and 49 Hz at 20 Msps. A tone 1 kHz from the centre loses less
//! than 0.001 dB. A carrier exactly at the centre is DC and is removed; the rest of a station at the centre loses the
//! part of its spectrum within a few corner widths of the centre.

use crate::ring::RingStats;
use sdrkit_core::{Complex32, Values};

/// The time constant of the estimate, in samples.
pub const TAU: f32 = 65_536.0;
/// The samples of one step of the estimate.
pub const BLOCK: usize = 1024;

/// The running estimate of the DC offset of one stream.
#[derive(Clone, Debug)]
pub struct DcBlock {
    mean: Complex32,
    /// After a restart, the sum and the count of the samples seen, while fewer than [`BLOCK`].
    fresh: Option<(Complex32, usize)>,
}

impl Default for DcBlock {
    fn default() -> Self {
        DcBlock {
            mean: Complex32::default(),
            fresh: Some((Complex32::default(), 0)),
        }
    }
}

impl DcBlock {
    /// Starts the estimate again: after a retune or a change of gain, the mean of the first [`BLOCK`] samples sets it
    /// at once, however the reads cut them. Until then the estimate from before stays.
    pub fn restart(&mut self) {
        self.fresh = Some((Complex32::default(), 0));
    }

    /// The estimate now.
    pub fn mean(&self) -> Complex32 {
        self.mean
    }

    /// Subtracts the estimate from `iq`, block by block, and moves the estimate toward the mean of each block.
    pub fn process(&mut self, iq: &mut [Complex32]) {
        for block in iq.chunks_mut(BLOCK) {
            let (mut si, mut sq) = (0.0f32, 0.0f32);
            for x in block.iter() {
                si += x.re;
                sq += x.im;
            }
            let n = block.len() as f32;
            match &mut self.fresh {
                Some((sum, count)) => {
                    *sum += Complex32::new(si, sq);
                    *count += block.len();
                    if *count >= BLOCK {
                        self.mean = *sum / *count as f32;
                        self.fresh = None;
                    }
                }
                None => self.mean += (Complex32::new(si / n, sq / n) - self.mean) * (n / TAU),
            }
            let d = self.mean;
            for x in block.iter_mut() {
                *x -= d;
            }
        }
    }
}

/// Makes the readers of the ring of `stats` remove the DC offset when `values` have [`DC_REMOVAL`](crate::form::DC_REMOVAL)
/// on, and start their estimate again from the samples written next.
pub fn follow(stats: &RingStats, values: &Values) {
    let on = values
        .schema()
        .id(crate::form::DC_REMOVAL_FORM.key)
        .ok()
        .and_then(|id| values.value(id))
        .is_some_and(|v| *v == sdrkit_core::Value::Bool(true));
    stats.set_dc_removal(on);
    stats.restart_dc();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The power of `iq` at `f` cycles per sample, from a correlation.
    fn power_at(iq: &[Complex32], f: f64) -> f64 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (k, x) in iq.iter().enumerate() {
            let p = -2.0 * std::f64::consts::PI * f * k as f64;
            let (c, s) = (p.cos(), p.sin());
            let (a, b) = (f64::from(x.re), f64::from(x.im));
            re += a * c - b * s;
            im += a * s + b * c;
        }
        (re * re + im * im) / (iq.len() as f64).powi(2)
    }

    fn capture(n: usize, dc: Complex32, tone: Option<(f64, f32)>) -> Vec<Complex32> {
        let mut seed = 1u32;
        (0..n)
            .map(|k| {
                let mut noise = || {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
                };
                let mut x = dc + Complex32::new(noise(), noise()) * 0.02;
                if let Some((f, a)) = tone {
                    let p = 2.0 * std::f64::consts::PI * f * k as f64;
                    x += Complex32::new(p.cos() as f32, p.sin() as f32) * a;
                }
                x
            })
            .collect()
    }

    #[test]
    fn the_offset_goes_and_a_tone_beside_the_centre_stays() {
        // 2.4 Msps: a tone 1 kHz from the centre, an offset 20 dB above it.
        let fs = 2.4e6;
        let mut iq = capture(1 << 20, Complex32::new(0.03, -0.02), Some((1e3 / fs, 0.0036)));
        let dc_before = power_at(&iq, 0.0);
        let tone_before = power_at(&iq, 1e3 / fs);
        let mut b = DcBlock::default();
        b.process(&mut iq);
        let tail = &iq[iq.len() / 2..];
        let dc_after = power_at(tail, 0.0);
        let tone_after = power_at(tail, 1e3 / fs);
        let down = 10.0 * (dc_before / dc_after).log10();
        assert!(down > 40.0, "{down} dB");
        let tone_db = 10.0 * (tone_after / tone_before).log10();
        assert!(tone_db.abs() < 0.05, "{tone_db} dB");
    }

    #[test]
    fn a_step_settles_at_once_after_a_restart_and_within_the_time_constant_without() {
        let mut b = DcBlock::default();
        let mut a = capture(BLOCK * 64, Complex32::new(0.05, 0.05), None);
        b.process(&mut a);
        // The offset moves, as after a retune.
        let step = Complex32::new(-0.04, 0.01);
        let mut s = capture(BLOCK * 4, step, None);
        let mut slow = b.clone();
        b.restart();
        b.process(&mut s);
        assert!((b.mean() - step).norm() < 2e-3, "{:?}", b.mean());
        // Without a restart, the estimate reaches the new offset within about five time constants.
        let mut t = capture(5 * TAU as usize, step, None);
        slow.process(&mut t);
        assert!((slow.mean() - step).norm() < 2e-3, "{:?}", slow.mean());
    }

    /// `n` samples whose I part goes up and down by `level` around 0.004 and whose Q part is 0.004, from sample `from`.
    fn swing(from: usize, n: usize, level: f32) -> Vec<Complex32> {
        (from..from + n)
            .map(|k| Complex32::new(if k % 2 == 0 { level } else { -level } + 0.004, 0.004))
            .collect()
    }

    /// True if every sample of `iq` has the magnitude `level` within 0.02.
    fn all_at(iq: &[Complex32], level: f32) -> bool {
        iq.iter().all(|x| (x.norm() - level).abs() < 0.02)
    }

    #[test]
    fn short_pieces_after_a_restart_do_not_set_the_estimate() {
        let mut b = DcBlock::default();
        let mut a = swing(0, BLOCK * 64, 0.25);
        b.process(&mut a);
        assert!(all_at(&a[BLOCK..], 0.25));
        // A retune: the next pieces have 1, 3 and 15 samples, then the reads are long again.
        b.restart();
        let mut k = 0;
        for n in [1, 3, 15, 1300, 16_384, 16_384] {
            let mut p = swing(k, n, 0.4);
            b.process(&mut p);
            assert!(all_at(&p, 0.4), "piece of {n}: {:?}", &p[..n.min(4)]);
            k += n;
        }
        assert!((b.mean() - Complex32::new(0.004, 0.004)).norm() < 1e-3, "{:?}", b.mean());
    }

    #[test]
    fn a_restart_on_the_last_sample_of_a_read_does_not_set_the_estimate_from_that_sample() {
        let (mut w, mut r) = crate::ring(1 << 16, sdrkit_graph::stream::SampleType::Cf32);
        w.stats().set_dc_removal(true);
        let mut out = vec![Complex32::default(); 1 << 15];
        w.write_iq(&swing(0, BLOCK * 8 - 1, 0.25));
        w.stats().restart_dc();
        w.write_iq(&swing(BLOCK * 8 - 1, BLOCK * 16 + 1, 0.25));
        // The first read ends on the first sample after the restart.
        let n = r.read_iq(&mut out[..BLOCK * 8]);
        assert!(all_at(&out[BLOCK..n], 0.25), "{:?}", &out[n - 4..n]);
        let n = r.read_iq(&mut out);
        assert!(all_at(&out[..n], 0.25), "{:?}", &out[..4]);
    }

    #[test]
    fn a_ring_reader_removes_the_offset_only_when_asked_and_restarts_where_the_writer_says() {
        let (mut w, mut r) = crate::ring(1 << 16, sdrkit_graph::stream::SampleType::Cf32);
        let block = capture(8 * BLOCK, Complex32::new(0.1, -0.1), None);
        let mut out = vec![Complex32::default(); block.len()];
        // Off: the offset stays.
        w.write_iq(&block);
        r.read_iq(&mut out);
        let mean = |v: &[Complex32]| v.iter().sum::<Complex32>() / v.len() as f32;
        assert!((mean(&out) - Complex32::new(0.1, -0.1)).norm() < 2e-3);
        // On: it goes.
        w.stats().set_dc_removal(true);
        w.write_iq(&block);
        r.read_iq(&mut out);
        assert!(mean(&out[BLOCK..]).norm() < 2e-3, "{:?}", mean(&out));
        // A step written after a restart is taken out from its first block.
        w.stats().restart_dc();
        let step = capture(8 * BLOCK, Complex32::new(-0.2, 0.05), None);
        w.write_iq(&step);
        r.read_iq(&mut out);
        assert!(mean(&out).norm() < 2e-3, "{:?}", mean(&out));
    }
}
