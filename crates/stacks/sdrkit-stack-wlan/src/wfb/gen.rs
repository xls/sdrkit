//! The `wfb` test signal: HT-mixed PPDUs at unit power carrying the frames of four links, MCS 0 to 7 with the long and the
//! short guard interval in turn, the pilots fixed (the values the specification of the stack gives). One cycle of 39 frames
//! every 50 ms: a wfb-ng video link that loses one fragment and one whole block of 12 in each cycle, a wfb-ng link with
//! session key packets, and an OpenHD air and ground link. The encrypted payloads are random bytes.

use super::frames;
use crate::ht::{build_ppdu, Mcs, PilotPattern};
use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, SignalInfo};
use sdrkit_proto_wlan_phy::ofdm::SAMPLE_RATE;

/// The video link of the test signal.
pub const VIDEO_LINK: u32 = 0x0A_1B2C;
/// Its radio port.
pub const VIDEO_PORT: u8 = 3;
/// The second wfb-ng link.
pub const SECOND_LINK: u32 = 0x00_7E57;
/// Its radio port.
pub const SECOND_PORT: u8 = 4;
/// Fragments of a FEC block of the video link.
pub const BLOCK: u8 = 12;
/// The first block of the video link.
pub const FIRST_BLOCK: u64 = 1234;
/// The cycle, s.
pub const PERIOD_S: f64 = 0.05;
/// Frames of one cycle.
pub const FRAMES: usize = 39;
/// Frames the video link loses in one cycle: one fragment and one block.
pub const LOST: u64 = 1 + BLOCK as u64;

/// A frame of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct TestFrame {
    /// The MPDU with its FCS.
    pub psdu: Vec<u8>,
    /// The MCS.
    pub mcs: Mcs,
    /// The short guard interval.
    pub short_gi: bool,
}

/// The MPDU `m` with its FCS.
fn with_fcs(mut m: Vec<u8>) -> Vec<u8> {
    let c = sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC.checksum(&m);
    m.extend(c.to_le_bytes());
    m
}

/// The frames of cycle `k`, in order.
pub fn cycle(k: u32) -> Vec<TestFrame> {
    let mut rng = Rng::stream(7, u64::from(k));
    let mut body = |n: usize| -> Vec<u8> {
        let mut b = vec![0u8; n];
        rng.fill_bytes(&mut b);
        b
    };
    let mut mpdus = Vec::with_capacity(FRAMES);
    let b0 = FIRST_BLOCK + 3 * u64::from(k);
    // The video link: block b0 without fragment 5, block b0 + 1 lost, block b0 + 2 whole.
    for f in (0..BLOCK).filter(|&f| f != 5) {
        mpdus.push(frames::wfb_data(VIDEO_LINK, VIDEO_PORT, b0, f, &body(180)));
    }
    for f in 0..BLOCK {
        mpdus.push(frames::wfb_data(VIDEO_LINK, VIDEO_PORT, b0 + 2, f, &body(180)));
    }
    // The second link: a session key packet, then data.
    mpdus.push(frames::wfb_session(SECOND_LINK, SECOND_PORT, &body(24 + 16)));
    for f in 0..5 {
        mpdus.push(frames::wfb_data(SECOND_LINK, SECOND_PORT, u64::from(k), f, &body(100)));
    }
    // OpenHD from the air on port 5 and from the ground on port 2.
    for n in 0..6u64 {
        mpdus.push(frames::openhd(1, 5, (u64::from(k) << 32) + 100 + n, &body(120)));
    }
    for n in 0..4u64 {
        mpdus.push(frames::openhd(2, 2, (u64::from(k) << 32) + 7 + n, &body(60)));
    }
    mpdus
        .into_iter()
        .enumerate()
        .map(|(i, m)| TestFrame {
            psdu: with_fcs(m),
            mcs: Mcs((i % 8) as u8),
            short_gi: (i / 8) % 2 == 1,
        })
        .collect()
}

/// The samples of `f` at `rate_hz` at unit mean power, with scrambler seed `seed` and pilots `pattern`.
///
/// # Errors
///
/// [`ConfigError`] if the resampler refuses the rates.
pub fn ppdu(
    f: &TestFrame,
    seed: u8,
    rate_hz: f64,
    pattern: PilotPattern,
) -> Result<Vec<Complex32>, ConfigError> {
    let mut y = Vec::new();
    build_ppdu(&f.psdu, f.mcs, f.short_gi, seed, pattern, &mut y);
    let mut x = if (rate_hz - SAMPLE_RATE).abs() < 1.0 {
        y
    } else {
        let mut r = FractionalResampler::with_rates(SAMPLE_RATE, rate_hz).map_err(ConfigError::from)?;
        y.extend(std::iter::repeat_n(Complex32::default(), 64));
        let mut x = Vec::new();
        r.process(&y, &mut x);
        x
    };
    let p = x.iter().map(|s| f64::from(s.norm_sqr())).sum::<f64>() / x.len().max(1) as f64;
    if p > 0.0 {
        let g = (1.0 / p).sqrt() as f32;
        x.iter_mut().for_each(|s| *s *= g);
    }
    Ok(x)
}

/// The test signal at `rate_hz`.
///
/// # Errors
///
/// [`ConfigError`] if the scene refuses the rate.
pub fn scene(rate_hz: f64) -> Result<Scene, ConfigError> {
    let mut scene = Scene::new(rate_hz)?;
    let (mut k, mut i, mut t) = (0u32, 0usize, 0.002);
    let mut frames = cycle(0);
    scene.add_source(Box::new(move || {
        if i == frames.len() {
            k += 1;
            i = 0;
            frames = cycle(k);
            t = 0.002 + f64::from(k) * PERIOD_S;
        }
        let f = &frames[i];
        let seed = 1 + ((i as u32 * 37 + k) % 127) as u8;
        let samples = ppdu(f, seed, rate_hz, PilotPattern::Fixed).ok()?;
        let b = Burst {
            start_s: t,
            offset_hz: 0.0,
            level_db: 0.0,
            samples,
        };
        t += b.samples.len() as f64 / rate_hz + 300e-6;
        i += 1;
        Some(b)
    }));
    Ok(scene)
}

/// The `wfb` generator.
pub struct WfbGenerator {
    scene: Scene,
    rate: f64,
}

/// The generator for `cfg`.
///
/// # Errors
///
/// As [`scene`].
pub fn from_config(cfg: &GenConfig) -> Result<WfbGenerator, ConfigError> {
    Ok(WfbGenerator {
        scene: scene(cfg.rate_hz)?,
        rate: cfg.rate_hz,
    })
}

impl Generator for WfbGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(20e6).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}
