//! The OGN tracker test signal: the chips of a packet, the FSK burst, and the scene of four trackers that circle a centre with
//! a status packet and a packet with an encrypted position. The generator makes the clean signal only: noise and the
//! impairments of the receiver come from the standard `gen_*` settings.

use super::channels::{fits, plan_frequencies};
use super::proto::{self, Header, PositionReport, PACKET};
use crate::adsl::channels::Band;
use crate::adsl::generator::{burst_samples, scene_traffic};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{self, Scene};
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};

/// The preamble bits of the test signal: 0 1 0 1 0 1 0 1.
pub const PREAMBLE_BITS: usize = 8;

/// The chips of one transmission: the preamble, the sync word and the packet, all Manchester coded.
pub fn air_chips(packet: &[u8; PACKET]) -> Vec<u8> {
    let mut c = Vec::with_capacity(2 * (PREAMBLE_BITS + 32 + 8 * PACKET));
    // Bit 1 is the chips 0 then 1.
    let mut push = |bit: u8| c.extend_from_slice(&[1 - bit, bit]);
    for i in 0..PREAMBLE_BITS {
        push((i % 2) as u8);
    }
    for k in (0..32).rev() {
        push((proto::SYNC >> k & 1) as u8);
    }
    for b in packet {
        for k in (0..8).rev() {
            push((b >> k) & 1);
        }
    }
    c
}

/// One transmission of a scene.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    /// The start, seconds.
    pub start_s: f64,
    /// The channel as an offset from the centre of the samples, Hz.
    pub offset_hz: f64,
    /// The 26 bytes.
    pub packet: [u8; PACKET],
    /// The level, dB.
    pub level_db: f64,
    /// The carrier offset, Hz.
    pub cfo_hz: f64,
    /// The error of the chip clock, ppm.
    pub ppm: f64,
    /// The spectrum is mirrored.
    pub inverted: bool,
}

impl Burst {
    /// A burst of `packet` at `start_s` on the channel `offset_hz` from the centre of the samples.
    pub fn new(start_s: f64, offset_hz: f64, packet: [u8; PACKET]) -> Burst {
        Burst {
            start_s,
            offset_hz,
            packet,
            level_db: 0.0,
            cfo_hz: 0.0,
            ppm: 0.0,
            inverted: false,
        }
    }

    /// The burst as samples at `rate`. The air signal is the one of the Manchester channels of ADS-L: 100 000 chips a second,
    /// ±50 kHz, a Gaussian filter with BT 0.5.
    pub fn in_scene(&self, rate: f64) -> scene::Burst {
        let mut samples = burst_samples(Band::M1, &air_chips(&self.packet), rate, self.ppm);
        if self.inverted {
            for v in &mut samples {
                *v = v.conj();
            }
        }
        scene::Burst {
            start_s: self.start_s,
            offset_hz: self.offset_hz + self.cfo_hz,
            level_db: self.level_db,
            samples,
        }
    }
}

/// The options of the scene of the stack.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneOptions {
    /// The position that the trackers circle.
    pub centre: (f64, f64),
    /// The trackers, 1 to 4.
    pub trackers: usize,
    /// The wrong bits in every third packet, 0 to 4.
    pub bad_bits: u32,
    /// Tracker 0 sets the emergency flag.
    pub emergency: bool,
    /// The channels as offsets from the centre of the samples, Hz.
    pub offsets: Vec<f64>,
}

/// The trackers of the scene: address, address type, aircraft type.
pub const SCENE_TRACKERS: [(u32, u8, u8); 4] = [
    (0xA12345, 1, 1),
    (0xDD4711, 2, 1),
    (0x7B0042, 3, 7),
    (0x5C3901, 0, 3),
];

/// The header of tracker `i` of the scene.
pub fn scene_header(i: usize, o: &SceneOptions) -> Header {
    Header {
        address: SCENE_TRACKERS[i].0,
        address_type: SCENE_TRACKERS[i].1,
        emergency: o.emergency && i == 0,
        ..Header::default()
    }
}

/// The report of tracker `i` of the scene at `t` seconds: the motion of the ADS-L scene.
pub fn scene_report(i: usize, t: f64, centre: (f64, f64)) -> PositionReport {
    let tr = scene_traffic(i, t, centre);
    let (lat_deg, lon_deg) = tr.position.unwrap_or(centre);
    PositionReport {
        lat_deg,
        lon_deg,
        second: (t as u64 % 60) as u8,
        fix_quality: 1,
        alt_m: tr.alt_m.unwrap_or(0.0),
        speed_m_s: tr.speed_m_s.unwrap_or(0.0),
        turn_deg_s: 0.0,
        heading_deg: tr.track_deg.unwrap_or(0.0),
        climb_m_s: tr.climb_m_s.unwrap_or(0.0),
        stealth: false,
        aircraft_type: SCENE_TRACKERS[i].2,
    }
}

/// The bursts of second `second` of the scene in a capture at `rate`: one position packet of each tracker, in turn on the
/// channels inside the capture. Every tenth second holds a status packet and a packet with an encrypted position.
pub fn scene_second(second: u64, rate: f64, o: &SceneOptions, rng: &mut Rng) -> Vec<Burst> {
    let inside: Vec<f64> = o
        .offsets
        .iter()
        .copied()
        .filter(|f| fits(*f, rate))
        .collect();
    if inside.is_empty() {
        return Vec::new();
    }
    let on = |k: u64| inside[(k % inside.len() as u64) as usize];
    let t0 = second as f64;
    let mut v = Vec::new();
    for i in 0..o.trackers.clamp(1, 4) {
        let t = t0 + 0.05 + 0.20 * i as f64 + 0.028 * rng.uniform();
        let p = proto::position_packet(&scene_header(i, o), &scene_report(i, t, o.centre));
        v.push(Burst::new(t, on(second + i as u64), p));
    }
    if second.is_multiple_of(10) {
        let report = scene_report(1, t0, o.centre);
        let status = Header {
            other: true,
            ..scene_header(1, o)
        };
        v.push(Burst::new(
            t0 + 0.85,
            on(second),
            proto::position_packet(&status, &report),
        ));
        let encrypted = Header {
            encrypted: true,
            ..scene_header(2, o)
        };
        v.push(Burst::new(
            t0 + 0.90,
            on(second + 1),
            proto::position_packet(&encrypted, &report),
        ));
    }
    v
}

/// The test signal of the stack.
pub struct OgnScene {
    rate: f64,
    scene: Scene,
}

impl OgnScene {
    /// The scene of `bursts` in a capture at `rate`, `seconds` long (`None`: endless).
    pub fn new(rate: f64, mut bursts: Vec<Burst>, seconds: Option<f64>) -> Self {
        bursts.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
        let mut scene = Scene::new(rate).expect("a positive rate");
        if let Some(s) = seconds {
            scene = scene.with_duration(s).expect("a positive duration");
        }
        let mut list = std::collections::VecDeque::from(bursts);
        scene.add_source(Box::new(move || list.pop_front().map(|b| b.in_scene(rate))));
        OgnScene { rate, scene }
    }

    /// The scene of the stack in a capture at `rate` whose centre is the middle of the channel plan.
    pub fn demo(rate: f64, o: SceneOptions, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let mut second = 0u64;
        let mut sent = 0u32;
        let mut queue = std::collections::VecDeque::new();
        let mut scene = Scene::new(rate).expect("a positive rate");
        scene.add_source(Box::new(move || {
            if queue.is_empty() {
                queue.extend(scene_second(second, rate, &o, &mut rng));
                second += 1;
            }
            let mut b: Burst = queue.pop_front()?;
            sent += 1;
            if o.bad_bits > 0 && sent.is_multiple_of(3) {
                // The wrong bits lie in the data words, 37 bits apart.
                for k in 0..o.bad_bits.min(4) {
                    let at = 39 + 37 * k as usize;
                    b.packet[at / 8] ^= 0x80 >> (at % 8);
                }
            }
            Some(b.in_scene(rate))
        }));
        OgnScene { rate, scene }
    }
}

impl Generator for OgnScene {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.scene.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(
            SignalInfo::new(200e3)
                .with_symbol_rate(100e3)
                .with_bit_rate(50e3),
        )
    }
}

/// The generator of the stack from its settings. The schema of `cfg.settings` must hold the settings of
/// [`super::OgnSetting`].
pub fn from_config(cfg: &GenConfig) -> OgnScene {
    let s = &cfg.settings;
    let n = usize::try_from(s.get(&super::channels::CHANNELS))
        .unwrap_or(1)
        .max(1);
    let o = SceneOptions {
        centre: s.get(&super::GEN_CENTRE),
        trackers: usize::try_from(s.get(&super::GEN_TRACKERS)).unwrap_or(4),
        bad_bits: u32::try_from(s.get(&super::GEN_BAD_BITS)).unwrap_or(0),
        emergency: s.get(&super::GEN_EMERGENCY),
        offsets: plan_frequencies(0.0, n, s.get(&super::channels::CHANNEL_STEP_HZ)),
    };
    OgnScene::demo(cfg.rate_hz, o, cfg.seed)
}
