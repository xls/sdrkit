//! The FLARM test signal: single bursts (GFSK, BT 0.5, +-50 kHz, Manchester at 100 kchip/s), scenes of
//! bursts on the two channels, and the demo source of six made-up devices circling a centre position. The generator makes the
//! clean signal only: noise and the receiver's impairments come from the standard `gen_*` settings.

use super::demod::Channel;
use super::proto::{self, V6Tx, PAYLOAD};
use sdrkit_algo_sim::modulate::{Cpm, Ramp};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{self, Scene};
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_util_geo::Position;

/// What one burst carries and how it is sent.
#[derive(Clone, Debug)]
pub struct Burst {
    /// When it starts, seconds from the start of the scene.
    pub start_s: f64,
    /// 0 (868.2 MHz) or 1 (868.4 MHz).
    pub channel: u8,
    /// The 24 payload bytes (encrypted V6, or raw for the newer protocol).
    pub payload: [u8; PAYLOAD],
    /// Level in dB (0 dB: amplitude 1).
    pub level_db: f64,
    /// The transmitter's carrier offset, Hz.
    pub cfo_hz: f64,
    /// The transmitter's chip clock error, ppm.
    pub ppm: f64,
    /// Two chips of the CRC flipped.
    pub corrupt_crc: bool,
}

impl Burst {
    /// A V6 packet from `tx` encrypted for key time `key_t`, on `channel` at `start_s`.
    pub fn v6(start_s: f64, channel: u8, tx: &V6Tx, key_t: f64) -> Burst {
        Burst {
            start_s,
            channel,
            payload: proto::encode(&tx.fields(), key_t.floor() as u32),
            level_db: 0.0,
            cfo_hz: 0.0,
            ppm: 0.0,
            corrupt_crc: false,
        }
    }
    /// A newer-protocol packet: header type 2 with `address`, the rest random.
    pub fn newer(start_s: f64, channel: u8, address: u32, rng: &mut Rng) -> Burst {
        let mut p = [0u8; PAYLOAD];
        for b in p.iter_mut() {
            *b = rng.next_u64() as u8;
        }
        let w0 = (address & 0xFF_FFFF) | (2 << 24) | (2 << 28);
        p[..4].copy_from_slice(&w0.to_le_bytes());
        Burst {
            start_s,
            channel,
            payload: p,
            level_db: 0.0,
            cfo_hz: 0.0,
            ppm: 0.0,
            corrupt_crc: false,
        }
    }
}

/// The chips of a burst: 16 preamble chips, the sync and the frame.
pub fn burst_chips(b: &Burst) -> Vec<u8> {
    let mut c: Vec<u8> = (0..16).map(|i| (i % 2) as u8).collect();
    let mut f = proto::frame_chips(&b.payload);
    if b.corrupt_crc {
        let n = f.len();
        f[n - 5] ^= 1;
        f[n - 6] ^= 1;
    }
    c.extend_from_slice(&f);
    c
}

/// A burst at baseband (its channel at 0 Hz, without its own carrier offset), at `rate`, unit amplitude: GFSK of BT 0.5 with a
/// peak deviation of 50 kHz, 4 µs ramps at the ends, and the transmitter's chip clock error.
pub fn burst_samples(b: &Burst, rate: f64) -> Vec<Complex32> {
    let chips = burst_chips(b);
    let levels: Vec<f32> = chips.iter().map(|&c| 2.0 * c as f32 - 1.0).collect();
    let chip_rate = proto::CHIP_RATE * (1.0 + b.ppm * 1e-6);
    let h = 2.0 * proto::DEVIATION / proto::CHIP_RATE;
    let mut m = Cpm::gfsk(0.5, h, rate / chip_rate)
        .expect("a GFSK modulator")
        .with_held_edges()
        .with_ramp(Ramp::RaisedCosine(4e-6 * rate))
        .expect("a ramp");
    let mut out = Vec::with_capacity((levels.len() as f64 * rate / chip_rate) as usize + 8);
    m.push(&levels, &mut out);
    m.flush(&mut out);
    out
}

/// One made-up device of the demo source.
#[derive(Clone, Debug)]
pub struct DemoDevice {
    /// Address.
    pub address: u32,
    /// Address type.
    pub addr_type: u8,
    /// Aircraft type.
    pub aircraft_type: u8,
    /// No-track flag.
    pub no_track: bool,
    /// Stealth flag.
    pub stealth: bool,
    /// Sends the newer protocol.
    pub newer: bool,
    /// Circle radius, m.
    pub radius: f64,
    /// Speed, m/s.
    pub speed: f64,
    /// Climb, m/s.
    pub climb: f64,
    /// Base altitude, m.
    pub alt: f64,
    /// Starting angle, degrees.
    pub phase: f64,
}

/// The six devices of the demo source.
pub fn demo_devices() -> Vec<DemoDevice> {
    let spec: [(u32, u8, u8, bool, bool, bool); 6] = [
        (0xDD1234, 2, 1, false, false, false),
        (0xDDA5B6, 2, 2, false, false, false),
        (0x3F7C11, 1, 8, false, false, false),
        (0xDD9999, 2, 1, true, false, false),
        (0xDD8888, 2, 7, false, true, false),
        (0xDD7777, 2, 1, false, false, true),
    ];
    spec.iter()
        .enumerate()
        .map(|(i, &(address, at, ty, nt, st, newer))| {
            let fi = i as f64;
            DemoDevice {
                address,
                addr_type: at,
                aircraft_type: ty,
                no_track: nt,
                stealth: st,
                newer,
                radius: 3000.0 + 1500.0 * fi,
                speed: if i == 2 { 38.0 } else { 24.0 + 2.0 * fi },
                climb: if i == 1 { 3.2 } else { -0.6 + 0.4 * fi },
                alt: 900.0 + 120.0 * fi,
                phase: 60.0 * fi,
            }
        })
        .collect()
}

impl DemoDevice {
    /// Where the device is at time `t` around (lat, lon).
    pub fn at(&self, t: f64, lat: f64, lon: f64) -> V6Tx {
        let ang = self.phase + (self.speed / self.radius * t).to_degrees();
        let p = Position::new(lat, lon).destination(ang, self.radius);
        let (la, lo) = (p.lat_deg, p.lon_deg);
        V6Tx {
            address: self.address,
            addr_type: self.addr_type,
            aircraft_type: self.aircraft_type,
            lat: la,
            lon: lo,
            alt_m: self.alt + 100.0 * (0.1 * t).sin(),
            speed: self.speed,
            track: (ang + 90.0).rem_euclid(360.0),
            climb: self.climb,
            airborne: true,
            stealth: self.stealth,
            no_track: self.no_track,
        }
    }
}

/// A scene: bursts on the two channels as a stream, each at its level and carrier offset.
pub struct FlarmScene {
    rate: f64,
    scene: Scene,
}

fn scene_burst(b: &Burst, rate: f64) -> scene::Burst {
    scene::Burst {
        start_s: b.start_s,
        offset_hz: Channel::of(b.channel).offset_hz() + b.cfo_hz,
        level_db: b.level_db,
        samples: burst_samples(b, rate),
    }
}

impl FlarmScene {
    /// A scene of `bursts` at `rate`, `seconds` long (`None`: endless).
    pub fn new(rate: f64, mut bursts: Vec<Burst>, seconds: Option<f64>) -> Self {
        bursts.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
        let mut scene = Scene::new(rate).expect("a positive rate");
        if let Some(s) = seconds {
            scene = scene.with_duration(s).expect("a positive duration");
        }
        let mut list = std::collections::VecDeque::from(bursts);
        scene.add_source(Box::new(move || {
            list.pop_front().map(|b| scene_burst(&b, rate))
        }));
        FlarmScene { rate, scene }
    }

    /// The demo source: six devices circling (lat, lon), each about once a second on alternating channels, encrypted for
    /// `start_time` plus signal time.
    pub fn demo(rate: f64, lat: f64, lon: f64, start_time: f64, seed: u64) -> Self {
        let devices = demo_devices();
        let mut next: Vec<f64> = (0..devices.len()).map(|i| 0.05 + 0.16 * i as f64).collect();
        let mut count = vec![0u64; devices.len()];
        let mut rng = Rng::new(seed);
        let mut scene = Scene::new(rate).expect("a positive rate");
        scene.add_source(Box::new(move || {
            // The device whose next burst comes first.
            let (i, &t) = next.iter().enumerate().min_by(|a, b| a.1.total_cmp(b.1))?;
            let ch = (count[i] % 2) as u8;
            let dev = &devices[i];
            let b = if dev.newer {
                Burst::newer(t, ch, dev.address, &mut rng)
            } else {
                Burst::v6(t, ch, &dev.at(t, lat, lon), start_time + t)
            };
            count[i] += 1;
            next[i] = t + 0.95 + 0.1 * rng.uniform();
            Some(scene_burst(&b, rate))
        }));
        FlarmScene { rate, scene }
    }
}

impl Generator for FlarmScene {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(200e3)
                .with_symbol_rate(proto::CHIP_RATE)
                .with_bit_rate(proto::CHIP_RATE / 2.0),
        )
    }
}

/// The stack's generator from its settings: the demo source around `gen_centre`, encrypted for `start_time` plus signal time. The schema of `cfg.settings` must hold the settings of [`super::FlarmSetting`].
pub fn from_config(cfg: &GenConfig) -> FlarmScene {
    let (lat, lon) = cfg.settings.get(&super::GEN_CENTRE);
    let start = cfg.settings.get(&super::START_TIME);
    FlarmScene::demo(cfg.rate_hz, lat, lon, start, cfg.seed)
}
