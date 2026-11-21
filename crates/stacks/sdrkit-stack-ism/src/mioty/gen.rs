//! The `mioty` test signal: uplink frames of the 17 patterns in turn (UPG3 first), each MPDU a short-address frame (byte 0 = 00, address
//! 12 34, a counter, random bytes), one frame starting every second, the 24 carriers centred on the samples. Each burst is
//! sent at unit amplitude.

use super::code::{
    encode, extension_bursts, extension_count, placement, BURST_SYMBOLS, CORE_PILOT,
    EXTENSION_PILOT, PATTERNS, PILOT_START, SYMBOL_RATE,
};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, SignalInfo};

/// The frequency of carrier 0 of the test frames, Hz from the centre: the 24 carriers centred on 0 Hz.
pub const TEST_F0_HZ: f64 = -12.0 * SYMBOL_RATE;
/// The time between two frame starts, s.
pub const FRAME_EVERY_S: f64 = 1.0;
/// The MPDU lengths of the test frames, in turn.
pub const TEST_PSI: [usize; 4] = [14, 20, 21, 30];

/// The symbols (+1 or −1 before the rotation by (−j)^n) of each burst of a frame: core bursts first, then extension bursts.
pub fn burst_levels(coded: &[u8], psi: usize) -> Vec<[f32; BURST_SYMBOLS]> {
    let se = extension_count(psi);
    let mut levels = vec![[0f32; BURST_SYMBOLS]; 24 + se];
    for (b, l) in levels.iter_mut().enumerate() {
        let pilot = if b < 24 { CORE_PILOT } else { EXTENSION_PILOT };
        for (i, &p) in pilot.iter().enumerate() {
            l[PILOT_START + i] = 1.0 - 2.0 * f32::from(p);
        }
    }
    for (&bit, &(b, m)) in coded.iter().zip(&placement(coded.len())) {
        levels[usize::from(b)][usize::from(m)] = 1.0 - 2.0 * f32::from(bit);
    }
    levels
}

/// (−j)^n.
pub fn rotation(n: usize) -> Complex32 {
    match n % 4 {
        0 => Complex32::new(1.0, 0.0),
        1 => Complex32::new(0.0, -1.0),
        2 => Complex32::new(-1.0, 0.0),
        _ => Complex32::new(0.0, 1.0),
    }
}

/// The samples of one burst at `rate`, from 18.5 symbols before its pilot centre to 18.5 symbols after: symbol n at
/// (n + 0.5 − 18) T from the centre, each a half-sine pulse cos(π t / 2T) over |t| < T.
pub fn burst_samples(levels: &[f32; BURST_SYMBOLS], rate: f64) -> Vec<Complex32> {
    let t = 1.0 / SYMBOL_RATE;
    let len = (37.0 * t * rate).ceil() as usize;
    (0..len)
        .map(|i| {
            let u = i as f64 / rate - 18.5 * t;
            let k = ((u / t) + 17.5).floor() as isize;
            let mut z = Complex32::default();
            for n in [k, k + 1] {
                if !(0..BURST_SYMBOLS as isize).contains(&n) {
                    continue;
                }
                let d = u - (n as f64 + 0.5 - 18.0) * t;
                if d.abs() < t {
                    let g = (std::f64::consts::PI * d / (2.0 * t)).cos() as f32;
                    z += rotation(n as usize) * (levels[n as usize] * g);
                }
            }
            z
        })
        .collect()
}

/// One test frame: for each burst, its pilot centre after the frame start (s), its carrier and its samples at `rate`.
pub fn frame(mpdu: &[u8], pattern: usize, rate: f64) -> Vec<(f64, u16, Vec<Complex32>)> {
    let (coded, hcrc, pcrc) = encode(mpdu, 0);
    let p = &PATTERNS[pattern];
    let levels = burst_levels(&coded, mpdu.len());
    let times = p.times();
    let mut at: Vec<(f64, u16)> = (0..24)
        .map(|k| (f64::from(times[k]), u16::from(p.carriers[k])))
        .collect();
    let mut t = f64::from(times[23]);
    for (dt, c) in extension_bursts(hcrc, pcrc, extension_count(mpdu.len()), p.spacing) {
        t += f64::from(dt);
        at.push((t, c));
    }
    at.iter()
        .zip(&levels)
        .map(|(&(sym, c), l)| (sym / SYMBOL_RATE, c, burst_samples(l, rate)))
        .collect()
}

/// The MPDU of test frame `k`: short address 12 34, counter `k`, then random bytes up to `psi`.
pub fn test_mpdu(k: u32, psi: usize, rng: &mut Rng) -> Vec<u8> {
    let mut m = vec![0x00, 0x12, 0x34, (k >> 16) as u8, (k >> 8) as u8, k as u8];
    m.extend((m.len()..psi).map(|_| rng.next_u64() as u8));
    m.truncate(psi);
    m
}

/// The test signal at `rate`.
///
/// # Errors
///
/// [`ConfigError`] if the scene refuses the rate.
pub fn scene(rate: f64, seed: u64) -> Result<Scene, ConfigError> {
    let mut scene = Scene::new(rate)?;
    let mut rng = Rng::stream(seed, 0x310);
    let mut k = 0u32;
    // Bursts of several frames interleave, so the source keeps them in order of start.
    let mut queue: Vec<Burst> = Vec::new();
    let mut next_frame = 0.2f64;
    let lead = 18.5 / SYMBOL_RATE;
    scene.add_source(Box::new(move || {
        // Make frames until the earliest queued burst starts before the next frame could add an earlier one.
        while queue
            .iter()
            .map(|b| b.start_s)
            .fold(f64::INFINITY, f64::min)
            > next_frame - lead
        {
            let psi = TEST_PSI[k as usize % TEST_PSI.len()];
            let mpdu = test_mpdu(k, psi, &mut rng);
            // UPG3 first: its frames are the shortest, so the first frame ends within a second.
            let pattern = (k as usize + PATTERNS.len() - 1) % PATTERNS.len();
            for (dt, c, samples) in frame(&mpdu, pattern, rate) {
                queue.push(Burst {
                    start_s: next_frame + dt - lead,
                    offset_hz: TEST_F0_HZ + f64::from(c) * SYMBOL_RATE,
                    level_db: 0.0,
                    samples,
                });
            }
            k += 1;
            next_frame += FRAME_EVERY_S;
        }
        let i = queue
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.start_s.total_cmp(&b.1.start_s))
            .map(|(i, _)| i)?;
        Some(queue.swap_remove(i))
    }));
    Ok(scene)
}

/// The `mioty` generator.
pub struct MiotyGenerator {
    scene: Scene,
    rate: f64,
}

impl MiotyGenerator {
    /// The test signal of `cfg`.
    ///
    /// # Errors
    ///
    /// As [`scene`].
    pub fn from_config(cfg: &GenConfig) -> Result<MiotyGenerator, ConfigError> {
        Ok(MiotyGenerator {
            scene: scene(cfg.rate_hz, cfg.seed)?,
            rate: cfg.rate_hz,
        })
    }
}

impl Generator for MiotyGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(SYMBOL_RATE)
                .with_symbol_rate(SYMBOL_RATE)
                .with_bit_rate(SYMBOL_RATE / 3.0)
                .with_power(0.5),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}
