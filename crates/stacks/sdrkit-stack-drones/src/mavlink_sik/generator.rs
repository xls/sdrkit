//! The `mavlink_sik` test signal: SiK packets in GFSK (BT 0.5, deviation half the bit rate, a 1 the higher frequency) at
//! unit power less 3 dB. Three vehicles send a HEARTBEAT and a GLOBAL_POSITION_INT in MAVLink v2 at 64, 128 and 250 kbit/s,
//! each hopping over three places of the capture with a carrier offset of -30 kHz, 0 and +25 kHz; a fourth sends MAVLink v1
//! with error correction at 96 kbit/s. The places scale with the sample rate. Every 100 ms each vehicle sends one packet, over
//! a background floor of -30 dBFS.

use super::{mavlink, sik};
use crate::fsklink::bits_of;
use sdrkit_algo_sim::modulate::{Cpm, Ramp};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{Burst, Scene};
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, SignalInfo};

/// The radio centre that the test signal assumes: the default frequency of the stack, Hz.
pub const TEST_CENTRE_HZ: f64 = 433.92e6;
/// The position of the first vehicle (degrees, metres).
pub const POSITION: (f64, f64, f64) = (54.6872, 25.2797, 180.0);
/// The bandwidth-time product of the Gaussian filter.
pub const BT: f64 = 0.5;
/// Preamble bytes of the test signal.
pub const PREAMBLE: usize = 8;
/// The cycle of the test signal, s.
pub const PERIOD_S: f64 = 0.1;
/// The power of the background noise of the scene, per sample (-30 dBFS): a capture always has a floor, and on an 8-bit
/// source it lies above the quantisation noise of the bursts.
pub const BACKGROUND: f32 = 1e-3;
/// The level of the bursts, dB (headroom for the noise on an 8-bit source).
pub const LEVEL_DB: f64 = -3.0;
/// The places of the hops, fractions of the sample rate from the centre.
pub const HOPS: [f64; 3] = [-0.28, 0.03, 0.22];
/// The carrier offsets of the hops, Hz.
pub const CFO_HZ: [f64; 3] = [-30e3, 0.0, 25e3];

/// A vehicle of the test signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vehicle {
    /// The net id of its radio.
    pub net_id: u16,
    /// Its system id.
    pub sys: u8,
    /// The air rate, bit/s.
    pub rate: f64,
    /// Error correction on.
    pub ecc: bool,
    /// MAVLink version.
    pub version: u8,
    /// Its position.
    pub position: (f64, f64, f64),
    /// When it sends in the cycle, s.
    pub at_s: f64,
}

/// The vehicles of the test signal.
pub fn vehicles() -> [Vehicle; 4] {
    let (lat, lon, alt) = POSITION;
    [
        Vehicle {
            net_id: 0x1234,
            sys: 1,
            rate: 64e3,
            ecc: false,
            version: 2,
            position: POSITION,
            at_s: 0.002,
        },
        Vehicle {
            net_id: 0x0101,
            sys: 2,
            rate: 128e3,
            ecc: false,
            version: 2,
            position: (lat + 0.01, lon - 0.02, alt + 20.0),
            at_s: 0.014,
        },
        Vehicle {
            net_id: 0x0202,
            sys: 3,
            rate: 250e3,
            ecc: false,
            version: 2,
            position: (lat - 0.015, lon + 0.01, alt + 40.0),
            at_s: 0.022,
        },
        Vehicle {
            net_id: 0x0A5A,
            sys: 4,
            rate: 96e3,
            ecc: true,
            version: 1,
            position: (lat + 0.02, lon + 0.03, alt + 60.0),
            at_s: 0.030,
        },
    ]
}

/// The data of one packet of vehicle `v` (MAVLink frames and the trailer) in cycle `n`.
pub fn data(v: &Vehicle, n: u32) -> Vec<u8> {
    let seq = (2 * n) as u8;
    let hb = [0u8, 0, 0, 0, 2, 3, 0x51, 4, 3];
    let (lat, lon, alt) = v.position;
    let gp = mavlink::global_position(n * 100, lat, lon, alt);
    let mut d = mavlink::build(v.version, seq, v.sys, 1, 0, &hb).unwrap_or_default();
    d.extend(mavlink::build(v.version, seq.wrapping_add(1), v.sys, 1, 33, &gp).unwrap_or_default());
    let t = sik::Trailer {
        window: (777 + n as u16) & 0x1FFF,
        ..sik::Trailer::default()
    };
    d.extend(t.bytes());
    d
}

/// The byte sent after a packet, while the transmitter ramps down.
pub const TAIL: u8 = 0x55;

/// The air bits of a packet: the preamble, the sync word, the bytes after it and [`TAIL`], most significant bit first.
pub fn air_bits(after_sync: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xAA; PREAMBLE];
    bytes.extend(sik::SYNC);
    bytes.extend_from_slice(after_sync);
    bytes.push(TAIL);
    let mut bits = Vec::with_capacity(8 * bytes.len());
    bits_of(&bytes, false, &mut bits);
    bits
}

/// GFSK samples of `bits` at `rate` bit/s and deviation `deviation_hz`, at `fs`, with a raised-cosine rise and fall over two
/// symbols (the power ramp of a transmitter).
///
/// # Errors
///
/// [`ConfigError`] if the modulator refuses the values.
pub fn gfsk(bits: &[u8], rate: f64, deviation_hz: f64, fs: f64) -> Result<Vec<Complex32>, ConfigError> {
    let mut m = Cpm::gfsk(BT, 2.0 * deviation_hz / rate, fs / rate)
        .map_err(ConfigError::from)?
        .with_held_edges()
        .with_ramp(Ramp::RaisedCosine(2.0 * fs / rate))
        .map_err(ConfigError::from)?;
    let levels: Vec<f32> = bits
        .iter()
        .map(|&b| if b == 1 { 1.0 } else { -1.0 })
        .collect();
    let mut out = Vec::with_capacity((bits.len() as f64 * fs / rate) as usize + 64);
    m.push(&levels, &mut out);
    m.flush(&mut out);
    Ok(out)
}

/// The samples of one packet of vehicle `v` in cycle `n` at `fs`.
///
/// # Errors
///
/// As [`gfsk`].
pub fn packet(v: &Vehicle, n: u32, fs: f64) -> Result<Vec<Complex32>, ConfigError> {
    let d = data(v, n);
    let after = if v.ecc {
        sik::build_ecc(&sik::golay(), v.net_id, &d)
    } else {
        sik::build_plain(v.net_id, &d)
    };
    gfsk(&air_bits(&after), v.rate, 0.5 * v.rate, fs)
}

/// The test signal at `fs`: the vehicles that fit the rate.
///
/// # Errors
///
/// [`ConfigError`] if the scene refuses the rate.
pub fn scene(fs: f64) -> Result<Scene, ConfigError> {
    let mut scene = Scene::new(fs)?;
    background(&mut scene, fs, 0x51C);
    for (i, v) in vehicles().into_iter().enumerate() {
        if fs < crate::fsklink::RATE_MARGIN * v.rate {
            continue;
        }
        let mut n = 0u32;
        scene.add_source(Box::new(move || {
            let hop = (n as usize + i) % HOPS.len();
            let samples = packet(&v, n, fs).ok()?;
            let b = Burst {
                start_s: v.at_s + f64::from(n) * PERIOD_S,
                offset_hz: HOPS[hop] * fs + CFO_HZ[hop],
                level_db: LEVEL_DB,
                samples,
            };
            n += 1;
            Some(b)
        }));
    }
    Ok(scene)
}

/// Adds a stream of complex white noise of power [`BACKGROUND`] to `scene`, in pieces of 10 ms.
pub fn background(scene: &mut Scene, fs: f64, seed: u64) {
    let mut rng = Rng::new(seed);
    let n = (fs * 0.01).ceil() as usize;
    let k = (BACKGROUND / 2.0).sqrt();
    let mut t = 0.0;
    scene.add_source(Box::new(move || {
        let samples = (0..n)
            .map(|_| Complex32::new(k * rng.normal_f32(), k * rng.normal_f32()))
            .collect();
        let b = Burst {
            start_s: t,
            offset_hz: 0.0,
            level_db: 0.0,
            samples,
        };
        t += n as f64 / fs;
        Some(b)
    }));
}

/// The `mavlink_sik` generator.
pub struct SikGenerator {
    scene: Scene,
    rate: f64,
}

/// The generator of the settings of `cfg`.
///
/// # Errors
///
/// As [`scene`].
pub fn from_config(cfg: &GenConfig) -> Result<SikGenerator, ConfigError> {
    Ok(SikGenerator {
        scene: scene(cfg.rate_hz)?,
        rate: cfg.rate_hz,
    })
}

impl Generator for SikGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(300e3)
                .with_symbol_rate(64e3)
                .with_bit_rate(64e3)
                .with_power(1.0),
        )
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}
