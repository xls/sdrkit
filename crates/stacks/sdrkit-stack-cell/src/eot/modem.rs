//! The end-of-train modem (`eot.modem`): the channel to 48 kHz, an FM discriminator, DC removal, the AFSK hard bits of eight
//! sampling phases, a frame search on each phase (the search word within one wrong bit and the 16 bits of bit sync before it
//! within two) and the check of each frame body. Of the bodies of one frame read through several phases, the one with the
//! fewest bits repaired is the frame.

use super::frame::{
    self, BODY_BITS, PREAMBLE_BITS, PREAMBLE_MAX_ERRORS, PREAMBLE_WORD, SEARCH_BITS, SEARCH_WORD,
};
use sdrkit_algo_dsp::fir::{self, Fir};
use sdrkit_algo_dsp::fm::fast_atan2;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_modem::afsk::PhaseBits;
use sdrkit_core::{Complex32, ConfigError, Values};
use sdrkit_graph::{no_values, Decoder, Format, Item, Outbox, Port, Timed};
use std::f64::consts::TAU;

/// The working rate, Hz.
pub const RATE: f64 = 48_000.0;
/// The sampling phases of a bit.
pub const PHASES: usize = 8;
/// The audio scale: 1.0 of audio is this deviation, Hz.
pub const AUDIO_HZ: f64 = 3000.0;
/// The factor of the one-pole mean that removes DC, per sample.
const DC_FACTOR: f32 = 0.002;
/// A frame equal to the last good one within this time is the same frame through another phase, s.
pub const SAME_FRAME_S: f64 = 0.05;
/// Samples at [`RATE`] between two level reports.
const LEVEL_EVERY: u64 = 4800;

/// What the modem passes on.
#[derive(Clone, Debug, PartialEq)]
pub enum EotHeard {
    /// A frame whose body the code took: its data bits and the bits repaired.
    Good {
        /// Data bits d0 to d44.
        data: [u8; frame::DATA_BITS],
        /// Bits repaired.
        repaired: u8,
    },
    /// A frame whose body the code refused in every phase.
    Bad,
    /// A frame sync, counted once for a frame.
    Sync,
    /// The channel level, dBFS.
    Level(f32),
}

impl Item for EotHeard {
    const KIND: &'static str = "eot.heard";
}

/// Every phase of one frame ends its body within this many samples of the first: a bit and a half.
const CLUSTER_SAMPLES: u64 = 60;

/// The bodies of one frame read through several phases.
#[derive(Clone, Copy)]
struct Cluster {
    /// The sample at which the first body ended.
    start: u64,
    /// The data of the body with the fewest bits repaired, and that number.
    best: Option<([u8; frame::DATA_BITS], u8)>,
    /// A body the code refused.
    bad: bool,
}

/// What a frame must pass besides the code.
#[derive(Clone, Copy, Debug)]
struct Acceptance {
    /// The bit sync before the search word is checked.
    preamble: bool,
    /// The wrong bits of the sync count against the bits the code repaired.
    sync_errors: bool,
}

impl Acceptance {
    /// Both checks.
    const FULL: Acceptance = Acceptance {
        preamble: true,
        sync_errors: true,
    };
}

/// One sampling phase: its search register (the bit sync checked and the search word) and the body being collected.
#[derive(Clone, Copy)]
struct Phase {
    reg: u64,
    /// Body bits still to come, 0 while searching.
    left: usize,
    body: [u8; BODY_BITS],
    /// Wrong bits of the sync in front of the body.
    sync_errors: u8,
}

impl Default for Phase {
    fn default() -> Self {
        Phase {
            reg: 0,
            left: 0,
            body: [0; BODY_BITS],
            sync_errors: 0,
        }
    }
}

/// The `eot.modem` decoder: IQ in, [`EotHeard`] items out.
pub struct EotModem {
    in_rate: f64,
    osc: Nco,
    resampler: Option<Resampler<Complex32>>,
    channel: Fir<Complex32>,
    mixed: Vec<Complex32>,
    worked: Vec<Complex32>,
    prev: Complex32,
    mean: f32,
    bits: PhaseBits,
    phases: [Phase; PHASES],
    n: u64,
    power: f64,
    /// The data and the sample of the last good frame.
    last_good: Option<([u8; frame::DATA_BITS], u64)>,
    /// The bodies of the frame being read through several phases.
    cluster: Option<Cluster>,
    /// The sample of the last sync counted.
    last_sync: Option<u64>,
    accept: Acceptance,
}

fn channel_taps() -> Vec<f32> {
    fir::lowpass(RATE, 7500.0, 12_000.0, 50.0).expect("the filter settings are valid")
}

impl Default for EotModem {
    fn default() -> Self {
        EotModem {
            in_rate: 0.0,
            osc: Nco::new(1.0, 0.0),
            resampler: None,
            channel: Fir::new(&channel_taps()),
            mixed: Vec::new(),
            worked: Vec::new(),
            prev: Complex32::new(0.0, 0.0),
            mean: 0.0,
            bits: PhaseBits::new(RATE, frame::BIT_RATE, frame::ONE_HZ, frame::ZERO_HZ, PHASES)
                .expect("40 samples a bit in 8 phases"),
            phases: [Phase::default(); PHASES],
            n: 0,
            power: 0.0,
            last_good: None,
            cluster: None,
            last_sync: None,
            accept: Acceptance::FULL,
        }
    }
}

impl EotModem {
    fn same_frame_samples() -> u64 {
        (SAME_FRAME_S * RATE) as u64
    }

    /// Forgets the frames in progress (a retune).
    fn reset_sync(&mut self) {
        self.phases = [Phase::default(); PHASES];
        self.bits.reset();
        self.cluster = None;
    }

    /// A body is complete, behind a sync with `sync_errors` wrong bits: it joins the bodies of the same frame read through the
    /// other phases.
    fn body(&mut self, body: [u8; BODY_BITS], sync_errors: u8) {
        let n = self.n;
        let c = self.cluster.get_or_insert(Cluster {
            start: n,
            best: None,
            bad: false,
        });
        let sync_errors = if self.accept.sync_errors {
            sync_errors
        } else {
            0
        };
        match frame::check(&body, sync_errors) {
            Some((data, repaired)) => {
                if c.best.is_none_or(|(_, r)| repaired < r) {
                    c.best = Some((data, repaired));
                }
            }
            None => c.bad = true,
        }
    }

    /// The phases of the frame are all read: the body with the fewest bits repaired is the frame.
    fn close(&mut self, out: &mut Vec<Timed<EotHeard>>) {
        let Some(c) = self.cluster.take() else {
            return;
        };
        let t = c.start as f64 / RATE;
        let near = Self::same_frame_samples();
        match c.best {
            Some((data, repaired)) => {
                let same = self
                    .last_good
                    .is_some_and(|(d, at)| d == data && c.start - at <= near);
                self.last_good = Some((data, c.start));
                if !same {
                    out.push(Timed {
                        time: t,
                        value: EotHeard::Good { data, repaired },
                    });
                }
            }
            None if c.bad => out.push(Timed {
                time: t,
                value: EotHeard::Bad,
            }),
            None => {}
        }
    }

    /// One decision of the hard-bit detector.
    fn bit(&mut self, phase: usize, bit: u8, out: &mut Vec<Timed<EotHeard>>) {
        let mask = (1u64 << (SEARCH_BITS + PREAMBLE_BITS)) - 1;
        let p = &mut self.phases[phase];
        p.reg = ((p.reg << 1) | u64::from(bit)) & mask;
        if p.left > 0 {
            p.body[BODY_BITS - p.left] = bit;
            p.left -= 1;
            if p.left == 0 {
                let (body, sync_errors) = (p.body, p.sync_errors);
                p.reg = 0;
                self.body(body, sync_errors);
            }
            return;
        }
        let sync_errors = ((p.reg as u32 & ((1 << SEARCH_BITS) - 1)) ^ SEARCH_WORD).count_ones();
        let preamble_ok = !self.accept.preamble
            || ((p.reg >> SEARCH_BITS) as u32 ^ PREAMBLE_WORD).count_ones() <= PREAMBLE_MAX_ERRORS;
        if sync_errors <= 1 && preamble_ok && self.n >= SEARCH_BITS as u64 * 40 {
            p.left = BODY_BITS;
            p.sync_errors = sync_errors as u8;
            let n = self.n;
            if self
                .last_sync
                .is_none_or(|at| n - at > Self::same_frame_samples())
            {
                self.last_sync = Some(n);
                out.push(Timed {
                    time: n as f64 / RATE,
                    value: EotHeard::Sync,
                });
            }
        }
    }

    fn sample(&mut self, z: Complex32, out: &mut Vec<Timed<EotHeard>>) {
        let y = self.channel.process_sample(z);
        let d = y * self.prev.conj();
        self.prev = y;
        let k = (RATE / TAU / AUDIO_HZ) as f32;
        let a = fast_atan2(d.im, d.re) * k;
        self.mean += DC_FACTOR * (a - self.mean);
        let audio = a - self.mean;
        self.power += (f64::from(y.norm_sqr()) - self.power) / 2400.0;
        if self.n.is_multiple_of(LEVEL_EVERY) {
            out.push(Timed {
                time: self.n as f64 / RATE,
                value: EotHeard::Level(sdrkit_algo_dsp::meter::power_to_db(self.power) as f32),
            });
        }
        if let Some(b) = self.bits.push(audio) {
            self.bit(b.phase, b.bit, out);
        }
        if self
            .cluster
            .as_ref()
            .is_some_and(|c| self.n - c.start > CLUSTER_SAMPLES)
        {
            self.close(out);
        }
        self.n += 1;
    }
}

impl Decoder for EotModem {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "eot.modem"
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<EotHeard>("out")]
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        *self = EotModem::default();
        self.in_rate = f.rate;
        self.osc = Nco::new(f.rate, f.centre_hz - f.tuned_hz);
        if (f.rate - RATE).abs() > 1e-6 {
            self.resampler = Some(Resampler::with_quality(f.rate, RATE, 0.2, 60.0)?);
        }
        Ok(vec![super::item_format::<EotHeard>(f)])
    }
    fn retune(&mut self, f: &Format) {
        self.osc = Nco::new(self.in_rate, f.centre_hz - f.tuned_hz);
        self.reset_sync();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        self.mixed.clear();
        self.mixed.extend_from_slice(x);
        self.osc.mix_in_place(&mut self.mixed);
        let mut worked = std::mem::take(&mut self.worked);
        worked.clear();
        match self.resampler.as_mut() {
            Some(r) => r.process(&self.mixed, &mut worked),
            None => worked.extend_from_slice(&self.mixed),
        }
        let items = out.items::<EotHeard>(0);
        for &z in &worked {
            self.sample(z, items);
        }
        self.worked = worked;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What one phase gives for the bit sync checked with its first `preamble_errors` bits wrong, the search word with its
    /// first `sync_errors` bits wrong and a frame body with `body_errors` bits wrong.
    fn heard(preamble_errors: usize, sync_errors: usize, body_errors: usize) -> Vec<EotHeard> {
        let data = super::super::gen::example(12345, 90, 0, true).to_bits();
        let mut pre: Vec<u8> = (0..PREAMBLE_BITS)
            .rev()
            .map(|i| ((PREAMBLE_WORD >> i) & 1) as u8)
            .collect();
        for b in pre.iter_mut().take(preamble_errors) {
            *b ^= 1;
        }
        let mut sync: Vec<u8> = (0..SEARCH_BITS)
            .rev()
            .map(|i| ((SEARCH_WORD >> i) & 1) as u8)
            .collect();
        for b in sync.iter_mut().take(sync_errors) {
            *b ^= 1;
        }
        let mut body = frame::body(&data);
        for &i in [3, 50].iter().take(body_errors) {
            body[i] ^= 1;
        }
        let mut m = EotModem {
            n: 10_000,
            ..EotModem::default()
        };
        let mut out = Vec::new();
        for b in pre.into_iter().chain(sync).chain(body) {
            m.bit(0, b, &mut out);
        }
        m.close(&mut out);
        out.into_iter()
            .map(|t| t.value)
            .filter(|v| *v != EotHeard::Sync)
            .collect()
    }

    #[test]
    fn a_sync_error_and_two_repaired_bits_are_refused() {
        let data = super::super::gen::example(12345, 90, 0, true).to_bits();
        for (sync, body, repaired) in [(0, 0, 0), (1, 0, 0), (1, 1, 1), (0, 2, 2)] {
            assert_eq!(
                heard(0, sync, body),
                vec![EotHeard::Good { data, repaired }],
                "{sync} {body}"
            );
        }
        assert_eq!(heard(0, 1, 2), vec![EotHeard::Bad]);
    }

    #[test]
    fn the_bit_sync_before_the_search_word_is_checked() {
        let data = super::super::gen::example(12345, 90, 0, true).to_bits();
        assert_eq!(heard(2, 1, 1), vec![EotHeard::Good { data, repaired: 1 }]);
        assert!(heard(3, 0, 0).is_empty());
    }

    /// The frames that each acceptance of `accept` takes from `runs` runs of 600 s of complex noise (seeds 11 on), one fresh
    /// modem per acceptance and run.
    fn false_frames(runs: u64, accept: &[Acceptance]) -> Vec<usize> {
        let mut frames = vec![0; accept.len()];
        let mut x = vec![Complex32::default(); RATE as usize];
        let mut out = Vec::new();
        for seed in 11..11 + runs {
            let mut noise = sdrkit_algo_sim::noise::Awgn::with_power(0.02, seed).unwrap();
            let mut modems: Vec<EotModem> = accept
                .iter()
                .map(|&a| EotModem {
                    accept: a,
                    ..EotModem::default()
                })
                .collect();
            for _ in 0..600 {
                x.fill(Complex32::default());
                noise.process(&mut x);
                for (m, f) in modems.iter_mut().zip(frames.iter_mut()) {
                    for &z in &x {
                        m.sample(z, &mut out);
                    }
                    *f += out
                        .iter()
                        .filter(|t| matches!(t.value, EotHeard::Good { .. }))
                        .count();
                    out.clear();
                }
            }
        }
        frames
    }

    /// The least `b` that a Poisson count of mean `mean` exceeds with a probability under 1 %.
    fn bound(mean: f64) -> usize {
        let (mut p, mut k) = ((-mean).exp(), 0);
        let mut below = p;
        while 1.0 - below >= 0.01 {
            k += 1;
            p *= mean / k as f64;
            below += p;
        }
        k
    }

    /// False frames from noise, per 600 s. The search finds about 750 syncs within one wrong bit, 1 in 18 of them exact. A
    /// random body lies within 2 repairs of a code word with a chance of 2017 / 2^18 and within 1 repair with 64 / 2^18. With
    /// a sync error refused in front of two repairs: 750 x (2017 / 18 + 64 x 17 / 18) / 2^18 = 0.49, about 0.52 with the
    /// several phases of one sync. Without that rule a sync error and two repairs add 750 x 1953 x 17 / 18 / 2^18 = 5.3.
    const ROW_RULE_PER_600_S: f64 = 0.52;
    /// With the bit sync checked too: the rate above times 137 / 2^16.
    const FULL_PER_600_S: f64 = ROW_RULE_PER_600_S * 137.0 / 65_536.0;

    /// `runs` runs of 600 s of noise: the frames that the sync-error rule takes stay within the bound of its theory and the
    /// old rule exceeds that bound on the same noise; with the bit sync checked too, the frames stay within its own bound.
    fn false_frames_within_theory(runs: u64) -> [usize; 3] {
        let old = Acceptance {
            preamble: false,
            sync_errors: false,
        };
        let row = Acceptance {
            preamble: false,
            sync_errors: true,
        };
        let got = false_frames(runs, &[old, row, Acceptance::FULL]);
        let b_row = bound(ROW_RULE_PER_600_S * runs as f64);
        let b_full = bound(FULL_PER_600_S * runs as f64);
        eprintln!("{runs} x 600 s: {got:?}, bounds {b_row} and {b_full}");
        assert!(got[0] > b_row, "{got:?}");
        assert!(got[1] <= b_row, "{got:?}");
        assert!(got[2] <= b_full, "{got:?}");
        [b_row, b_full, got[2]]
    }

    /// 3 runs: a bound of 5 frames for a mean of 1.56 with the sync-error rule, 0 with the bit sync checked too.
    #[test]
    fn false_frames_from_noise() {
        assert_eq!(&false_frames_within_theory(3)[..2], &[5, 0]);
    }

    /// 10 runs: a bound of 11 frames for a mean of 5.2 with the sync-error rule, 1 for a mean of 0.011 with the bit sync
    /// checked too.
    #[test]
    #[ignore = "6000 s of noise through three modems (about 70 s); run with --ignored"]
    fn false_frames_from_noise_long() {
        assert_eq!(&false_frames_within_theory(10)[..2], &[11, 1]);
    }
}
