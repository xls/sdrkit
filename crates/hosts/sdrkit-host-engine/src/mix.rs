//! The mix of the channel bank: the audio of every channel session, each with its pan and solo, summed into one stereo stream of
//! the bank's session. The bank's session thread mixes after each of its blocks. A session's volume and mute act on its own
//! audio stream.

use crate::streams::{Run, Streams, RUNS};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::Stereo;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// The level settings of a session, which its control writes and the session and mix threads read.
#[derive(Debug)]
pub(crate) struct Gains {
    volume: AtomicU32,
    pan: AtomicU32,
    mute: AtomicBool,
    solo: AtomicBool,
}

impl Default for Gains {
    fn default() -> Self {
        Gains {
            volume: AtomicU32::new(1.0f32.to_bits()),
            pan: AtomicU32::new(0.0f32.to_bits()),
            mute: AtomicBool::new(false),
            solo: AtomicBool::new(false),
        }
    }
}

impl Gains {
    pub(crate) fn set(&self, volume: f64, pan: f64, mute: bool, solo: bool) {
        self.volume
            .store((volume as f32).to_bits(), Ordering::Relaxed);
        self.pan.store((pan as f32).to_bits(), Ordering::Relaxed);
        self.mute.store(mute, Ordering::Relaxed);
        self.solo.store(solo, Ordering::Relaxed);
    }

    /// The factor for the session's own audio: its volume, or 0 when muted.
    pub(crate) fn level(&self) -> f32 {
        if self.mute.load(Ordering::Relaxed) {
            0.0
        } else {
            f32::from_bits(self.volume.load(Ordering::Relaxed))
        }
    }

    fn pan(&self) -> f32 {
        f32::from_bits(self.pan.load(Ordering::Relaxed))
    }

    fn solo(&self) -> bool {
        self.solo.load(Ordering::Relaxed)
    }
}

/// One channel session in the mix: its streams, its gains, the mix's position in its audio, and the resampler of a
/// channel whose rate differs from the rate of the mix.
pub(crate) struct MixIn {
    pub(crate) streams: Arc<Streams>,
    pub(crate) gains: Arc<Gains>,
    at: u64,
    runs: [Run; RUNS],
    /// The input rate of `resampler` and the resampler.
    resampler: Option<(f64, Resampler<Stereo>)>,
    stereo: Vec<f32>,
    resampled: Vec<f32>,
}

impl MixIn {
    pub(crate) fn new(streams: Arc<Streams>, gains: Arc<Gains>) -> Self {
        MixIn {
            streams,
            gains,
            at: 0,
            runs: Default::default(),
            resampler: None,
            stereo: Vec::new(),
            resampled: Vec::new(),
        }
    }

    /// Returns the stereo frames in `stereo` at `rate`, resampled to `mix_rate` if the rates differ. Returns `None` if no
    /// resampler is made for the two rates.
    fn at_rate(&mut self, rate: f64, mix_rate: f64) -> Option<&[f32]> {
        if rate == mix_rate {
            return Some(&self.stereo);
        }
        if self.resampler.as_ref().is_none_or(|(r, _)| *r != rate) {
            self.resampler = Some((rate, Resampler::new(rate, mix_rate).ok()?));
        }
        let (_, r) = self.resampler.as_mut()?;
        self.resampled.clear();
        r.process_interleaved(&self.stereo, &mut self.resampled);
        Some(&self.resampled)
    }
}

/// The audio a mix takes from one channel in one step, in samples.
const STEP: usize = 8192;

/// The mixer of the bank's session.
#[derive(Default)]
pub(crate) struct Mixer {
    inputs: Vec<MixIn>,
    scratch: Vec<f32>,
    sum: Vec<f32>,
    /// The rate of the mix: the rate of the first channel that had audio.
    rate: Option<f64>,
}

impl Mixer {
    /// Mixes the channels in `inputs` from now on.
    pub(crate) fn set(&mut self, inputs: Vec<MixIn>) {
        for i in &self.inputs {
            i.streams.mixed.store(false, Ordering::Relaxed);
        }
        for i in &inputs {
            i.streams.mixed.store(true, Ordering::Relaxed);
        }
        if inputs.is_empty() {
            self.rate = None;
        }
        self.inputs = inputs;
        self.scratch.resize(STEP, 0.0);
    }

    /// Takes the audio that waits in each channel and writes the stereo sum into `out`. The rate of the first channel that
    /// had audio is the rate of the mix; the audio of a channel at another rate is resampled to it. While a channel is solo,
    /// only the solo channels sound.
    pub(crate) fn step(&mut self, out: &Streams) {
        if self.inputs.is_empty() {
            return;
        }
        let solo = self.inputs.iter().any(|i| i.gains.solo());
        let mut wrote = false;
        self.sum.clear();
        for i in &mut self.inputs {
            let Some(a) = i
                .streams
                .read_audio(&mut i.at, &mut i.runs, &mut self.scratch)
            else {
                continue;
            };
            if solo && !i.gains.solo() {
                continue;
            }
            let mix_rate = *self.rate.get_or_insert(a.rate_hz);
            let pan = i.gains.pan().clamp(-1.0, 1.0);
            let (gl, gr) = ((1.0 - pan).min(1.0), (1.0 + pan).min(1.0));
            let ch = usize::from(a.channels.max(1));
            i.stereo.clear();
            for frame in self.scratch[..a.frames * ch].chunks_exact(ch) {
                let (l, r) = if ch == 1 {
                    (frame[0], frame[0])
                } else {
                    (frame[0], frame[1])
                };
                i.stereo.extend([l * gl, r * gr]);
            }
            let Some(frames) = i.at_rate(a.rate_hz, mix_rate) else {
                continue;
            };
            if self.sum.len() < frames.len() {
                self.sum.resize(frames.len(), 0.0);
            }
            for (s, v) in self.sum.iter_mut().zip(frames) {
                *s += v;
            }
            wrote = true;
        }
        if let (true, Some(rate)) = (wrote, self.rate) {
            out.write_audio(&self.sum, rate, 2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(level: f32, channels: u16) -> (Arc<Streams>, Arc<Gains>) {
        let s = Arc::new(Streams::new(1024, 1));
        s.write_audio(&vec![level; 100 * usize::from(channels)], 8e3, channels);
        (s, Arc::default())
    }

    fn mixed(inputs: Vec<MixIn>) -> Vec<f32> {
        let out = Streams::new(4096, 1);
        let mut m = Mixer::default();
        m.set(inputs);
        m.step(&out);
        let mut buf = vec![0.0; 4096];
        let (mut at, mut runs) = (0, Default::default());
        let r = out
            .read_audio(&mut at, &mut runs, &mut buf)
            .expect("the mix gives audio");
        assert_eq!((r.rate_hz, r.channels), (8e3, 2));
        buf.truncate(2 * r.frames);
        buf
    }

    #[test]
    fn the_mix_sums_the_channels_with_their_pan_and_solo() {
        let (a, ga) = channel(0.25, 1);
        let (b, gb) = channel(0.5, 2);
        ga.set(1.0, -1.0, false, false);
        let sum = mixed(vec![
            MixIn::new(Arc::clone(&a), Arc::clone(&ga)),
            MixIn::new(Arc::clone(&b), Arc::clone(&gb)),
        ]);
        assert_eq!(sum.len(), 200);
        // The first channel is on the left only.
        assert_eq!((sum[0], sum[1]), (0.75, 0.5));
        let (a, ga) = channel(0.25, 1);
        let (b, gb) = channel(0.5, 2);
        gb.set(1.0, 0.0, false, true);
        let sum = mixed(vec![MixIn::new(a, ga), MixIn::new(b, gb)]);
        assert_eq!((sum[0], sum[1]), (0.5, 0.5), "only the solo channel sounds");
    }

    #[test]
    fn a_channel_at_another_rate_is_resampled_into_the_mix() {
        let out = Streams::new(1 << 16, 1);
        let (a, ga) = channel(0.25, 1);
        let b = Arc::new(Streams::new(1 << 16, 1));
        let mut m = Mixer::default();
        m.set(vec![
            MixIn::new(Arc::clone(&a), ga),
            MixIn::new(Arc::clone(&b), Arc::default()),
        ]);
        m.step(&out);
        // The second channel runs at twice the rate of the mix: it produces half as many frames of the mix per second.
        for _ in 0..20 {
            a.write_audio(&[0.0; 800], 8e3, 1);
            b.write_audio(&[0.5; 1600], 16e3, 1);
            m.step(&out);
        }
        let mut buf = vec![0.0; 1 << 16];
        let (mut at, mut runs) = (0, Default::default());
        let mut frames = 0;
        while let Some(r) = out.read_audio(&mut at, &mut runs, &mut buf) {
            assert_eq!((r.rate_hz, r.channels), (8e3, 2));
            frames += r.frames;
            if frames > 10_000 {
                // Once the resampler has settled, the level of the second channel is in the mix.
                let middle = buf[2 * (r.frames / 2)];
                assert!((middle - 0.5).abs() < 0.05, "{middle}");
            }
        }
        assert!(frames > 15_000, "{frames} frames");
    }
}
