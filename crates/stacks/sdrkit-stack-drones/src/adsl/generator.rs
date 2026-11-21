//! The ADS-L test signal: the chips of a transmission on each band, the GFSK burst, and the scene of four aircraft that circle
//! a centre with a Status packet and a Traffic Uplink. The generator makes the clean signal only: noise and the impairments
//! of the receiver come from the standard `gen_*` settings.

use super::channels::{Band, NARROW_CENTRE_HZ, WIDE_CENTRE_HZ, WIDE_RATE_HZ};
use super::proto::{
    self, network_packet, Address, DataHeader, NetHeader, Status, Traffic, LDR_CRC8, LDR_HEADER,
};
use sdrkit_algo_sim::modulate::{Cpm, Ramp};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::scene::{self, Scene};
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_util_geo::Position;

/// The repeats of "01" in the M-band preamble when nothing else is asked.
pub const M_PREAMBLE: usize = 8;

fn push_bits(chips: &mut Vec<u8>, bytes: &[u8], manchester: bool) {
    for &b in bytes {
        for k in (0..8).rev() {
            let bit = (b >> k) & 1;
            if manchester {
                // Bit 1 is the chips 0 then 1.
                chips.extend_from_slice(&[1 - bit, bit]);
            } else {
                chips.push(bit);
            }
        }
    }
}

/// The chips of one transmission of the network packet `packet` on `band`: preamble, sync word, the length byte or the fixed
/// header, the packet, and on O-band LDR the CRC-8. `preamble` is the number of repeats of the preamble pair (M-band "01",
/// O-band "10").
pub fn air_chips(band: Band, packet: &[u8], preamble: usize) -> Vec<u8> {
    air_chips_with(band, packet, preamble, packet.len() as u8, &[])
}

/// The chips of [`air_chips`] with the length byte `length` (a band with a length byte sends it as given) and with `tail`
/// after the packet: the bytes of a signature, which the M-band sends without Manchester coding.
pub fn air_chips_with(
    band: Band,
    packet: &[u8],
    preamble: usize,
    length: u8,
    tail: &[u8],
) -> Vec<u8> {
    let mut c = Vec::new();
    match band {
        Band::M1 | Band::M2 => {
            for _ in 0..preamble {
                c.extend_from_slice(&[0, 1]);
            }
            c.extend_from_slice(&[1, 0, 0, 1, 1, 0, 0, 1]);
            push_bits(&mut c, &[0x72, 0x4B, length], true);
            push_bits(&mut c, packet, true);
            push_bits(&mut c, tail, false);
        }
        Band::Ldr => {
            for _ in 0..preamble {
                c.extend_from_slice(&[1, 0]);
            }
            push_bits(&mut c, &[0xB4, 0x2B], false);
            push_bits(&mut c, &LDR_HEADER, false);
            push_bits(&mut c, packet, false);
            push_bits(&mut c, &[LDR_CRC8.compute(packet) as u8], false);
        }
        Band::Hdr => {
            for _ in 0..preamble {
                c.extend_from_slice(&[1, 0]);
            }
            push_bits(&mut c, &[0x2D, 0xD4, length], false);
            push_bits(&mut c, packet, false);
            push_bits(&mut c, tail, false);
        }
    }
    c
}

/// The preamble repeats of a band when nothing else is asked: M-band 8, O-band LDR 40, O-band HDR 12.
pub fn default_preamble(band: Band) -> usize {
    match band {
        Band::M1 | Band::M2 => M_PREAMBLE,
        Band::Ldr => 40,
        Band::Hdr => 12,
    }
}

/// The Gaussian filter of a band: the product of bandwidth and chip time.
fn bt(band: Band) -> f64 {
    match band {
        Band::Ldr => 1.0,
        _ => 0.5,
    }
}

/// The chip times that the transmitter is on before the first chip and after the last one, so that the ramps of the amplitude
/// lie outside the chips.
pub const LEAD_CHIPS: usize = 3;

/// Chips as a burst at baseband (the channel at 0 Hz), at `rate`, unit amplitude: GFSK with the deviation and the Gaussian
/// filter of the band, the chip clock error `ppm` of the transmitter, and ramps of 4 µs at the ends. The transmitter is on
/// for [`LEAD_CHIPS`] chip times before the first chip (the preamble pattern goes on backwards) and as long after the last
/// one (it holds the last chip).
pub fn burst_samples(band: Band, chips: &[u8], rate: f64, ppm: f64) -> Vec<Complex32> {
    let cfg = band.chip_config(true);
    let level = |c: u8| 2.0 * f32::from(c) - 1.0;
    let (first, last) = (
        chips.first().copied().unwrap_or(0),
        chips.last().copied().unwrap_or(0),
    );
    let mut levels: Vec<f32> = (0..LEAD_CHIPS)
        .map(|i| level(first ^ ((LEAD_CHIPS - i) % 2) as u8))
        .collect();
    levels.extend(chips.iter().map(|&c| level(c)));
    levels.extend(std::iter::repeat_n(level(last), LEAD_CHIPS));
    let chip_rate = cfg.chip_rate * (1.0 + ppm * 1e-6);
    let h = 2.0 * cfg.deviation_hz / cfg.chip_rate;
    let mut m = Cpm::gfsk(bt(band), h, rate / chip_rate)
        .expect("a GFSK modulator")
        .with_held_edges()
        .with_ramp(Ramp::RaisedCosine(4e-6 * rate))
        .expect("a ramp");
    let mut out = Vec::with_capacity((levels.len() as f64 * rate / chip_rate) as usize + 8);
    m.push(&levels, &mut out);
    m.flush(&mut out);
    out
}

/// One transmission of a scene.
#[derive(Clone, Debug)]
pub struct Burst {
    /// When it starts, seconds from the start of the scene.
    pub start_s: f64,
    /// The band.
    pub band: Band,
    /// The network packet.
    pub packet: Vec<u8>,
    /// The repeats of the preamble pair.
    pub preamble: usize,
    /// The level, dB (0 dB: amplitude 1).
    pub level_db: f64,
    /// The carrier offset of the transmitter, Hz.
    pub cfo_hz: f64,
    /// The chip clock error of the transmitter, ppm.
    pub ppm: f64,
    /// The length byte sent. `None` sends the length of the packet.
    pub length: Option<u8>,
    /// The bytes sent after the packet: a signature.
    pub tail: Vec<u8>,
    /// The spectrum is inverted: the burst is sent as its complex conjugate.
    pub inverted: bool,
}

impl Burst {
    /// `packet` on `band` at `start_s`, with the default preamble, at full level, without errors of the transmitter.
    pub fn new(start_s: f64, band: Band, packet: Vec<u8>) -> Burst {
        Burst {
            start_s,
            band,
            packet,
            preamble: default_preamble(band),
            level_db: 0.0,
            cfo_hz: 0.0,
            ppm: 0.0,
            length: None,
            tail: Vec::new(),
            inverted: false,
        }
    }

    /// The burst in a capture at `rate` centred on `centre_hz`.
    pub fn in_scene(&self, rate: f64, centre_hz: f64) -> scene::Burst {
        let length = self.length.unwrap_or(self.packet.len() as u8);
        let chips = air_chips_with(self.band, &self.packet, self.preamble, length, &self.tail);
        let mut samples = burst_samples(self.band, &chips, rate, self.ppm);
        if self.inverted {
            for v in &mut samples {
                *v = v.conj();
            }
        }
        scene::Burst {
            start_s: self.start_s,
            offset_hz: self.band.frequency_hz() - centre_hz + self.cfo_hz,
            level_db: self.level_db,
            samples,
        }
    }
}

/// What the scene sends beside its aircraft.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneOptions {
    /// The centre that the aircraft circle: latitude and longitude, degrees.
    pub centre: (f64, f64),
    /// The aircraft, 1 to 4.
    pub aircraft: usize,
    /// The key index of every packet: 0 scrambled, 3 not scrambled.
    pub key_index: u8,
    /// The Traffic packets on the M-band and on HDR carry Reed-Solomon parity.
    pub fec: bool,
    /// The repeats of "01" in the M-band preamble.
    pub m_preamble: usize,
    /// Every n-th packet has one wrong bit (0: none).
    pub bad_bit: u32,
}

impl Default for SceneOptions {
    fn default() -> Self {
        SceneOptions {
            centre: (54.70, 25.30),
            aircraft: 4,
            key_index: 0,
            fec: false,
            m_preamble: M_PREAMBLE,
            bad_bit: 0,
        }
    }
}

/// The four aircraft of the scene: address, address mapping table, category.
pub const SCENE_AIRCRAFT: [(u32, u8, u8); 4] = [
    (0xA12345, 5, 1),
    (0xDD4711, 6, 4),
    (0x7B0042, 7, 7),
    (0x5C3901, 0, 3),
];

/// The Traffic payload of aircraft `i` of the scene at `t` seconds.
pub fn scene_traffic(i: usize, t: f64, centre: (f64, f64)) -> Traffic {
    let fi = i as f64;
    let (radius, speed) = (3000.0 + 1800.0 * fi, 26.0 + 14.0 * fi);
    let ang = 90.0 * fi + (speed / radius * t).to_degrees();
    let at = Position::new(centre.0, centre.1).destination(ang, radius);
    Traffic {
        timestamp: ((t * 4.0) as u64 % 60) as u8,
        flight_state: 2,
        category: SCENE_AIRCRAFT[i].2,
        emergency: 1,
        position: Some((at.lat_deg, at.lon_deg)),
        speed_m_s: Some(speed),
        alt_m: Some(800.0 + 150.0 * fi),
        climb_m_s: Some(-1.5 + fi),
        track_deg: Some((ang + 90.0).rem_euclid(360.0)),
        hfom: 6,
        vfom: 2,
        velocity_accuracy: 2,
        ..Traffic::default()
    }
}

fn address(i: usize) -> Address {
    Address {
        amt: SCENE_AIRCRAFT[i].1,
        address: SCENE_AIRCRAFT[i].0,
    }
}

/// The transmissions of second `second` of the scene. `wide` says that the capture holds the O-band.
pub fn scene_second(second: u64, wide: bool, o: &SceneOptions, rng: &mut Rng) -> Vec<Burst> {
    let t0 = second as f64;
    let head = |fec: bool| NetHeader {
        version: 1,
        signed: false,
        key_index: o.key_index,
        reed_solomon: fec,
    };
    let mut v = Vec::new();
    for i in 0..o.aircraft.clamp(1, 4) {
        let t = t0 + 0.05 + 0.20 * i as f64 + 0.028 * rng.uniform();
        let data = proto::data(
            DataHeader {
                payload_type: 2,
                address: address(i),
                relay: false,
            },
            &scene_traffic(i, t, o.centre).bytes(),
        );
        // Aircraft 2 uses O-band LDR in a wide capture. The others change between the M channels from second to second.
        let band = if i == 2 && wide {
            Band::Ldr
        } else if (second + i as u64).is_multiple_of(2) {
            Band::M1
        } else {
            Band::M2
        };
        let fec = o.fec && band != Band::Ldr;
        let mut b = Burst::new(t, band, network_packet(head(fec), &data));
        if band != Band::Ldr {
            b.preamble = o.m_preamble;
        }
        v.push(b);
    }
    if second.is_multiple_of(3) && wide && o.aircraft >= 2 {
        // A Traffic Uplink of the ground station OGN F00001 with aircraft 0 and 1, in the uplink slot.
        let t = t0 + 0.35;
        let list = [0, 1].map(|i| (address(i), scene_traffic(i, t, o.centre)));
        let data = proto::uplink_data(
            Address {
                amt: 7,
                address: 0xF00001,
            },
            &list,
        );
        v.push(Burst::new(t, Band::Hdr, network_packet(head(true), &data)));
    }
    if second.is_multiple_of(5) && o.aircraft >= 2 {
        let data = proto::data(
            DataHeader {
                payload_type: 3,
                address: address(1),
                relay: false,
            },
            &Status {
                version: 1,
                rx_m: 3,
                ..Status::default()
            }
            .bytes(),
        );
        let mut b = Burst::new(t0 + 0.9, Band::M1, network_packet(head(false), &data));
        b.preamble = o.m_preamble;
        v.push(b);
    }
    v.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
    v
}

/// A scene of ADS-L transmissions as a stream.
pub struct AdslScene {
    rate: f64,
    scene: Scene,
}

impl AdslScene {
    /// The scene of `bursts` in a capture at `rate` centred on `centre_hz`, `seconds` long (`None`: endless).
    pub fn new(rate: f64, centre_hz: f64, mut bursts: Vec<Burst>, seconds: Option<f64>) -> Self {
        bursts.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
        let mut scene = Scene::new(rate).expect("a positive rate");
        if let Some(s) = seconds {
            scene = scene.with_duration(s).expect("a positive duration");
        }
        let mut list = std::collections::VecDeque::from(bursts);
        scene.add_source(Box::new(move || {
            list.pop_front().map(|b| b.in_scene(rate, centre_hz))
        }));
        AdslScene { rate, scene }
    }

    /// The scene of the stack: four aircraft that circle the centre, in a capture at `rate`. From 1.6 Msps on the capture is
    /// centred on 868.875 MHz and holds all four channels. Below, it is centred on 868.3 MHz and holds the M-band.
    pub fn demo(rate: f64, o: SceneOptions, seed: u64) -> Self {
        let wide = rate >= WIDE_RATE_HZ;
        let centre_hz = if wide {
            WIDE_CENTRE_HZ
        } else {
            NARROW_CENTRE_HZ
        };
        let mut rng = Rng::new(seed);
        let mut second = 0u64;
        let mut sent = 0u32;
        let mut queue = std::collections::VecDeque::new();
        let mut scene = Scene::new(rate).expect("a positive rate");
        scene.add_source(Box::new(move || {
            while queue.is_empty() {
                queue.extend(scene_second(second, wide, &o, &mut rng));
                second += 1;
            }
            let mut b: Burst = queue.pop_front()?;
            sent += 1;
            if o.bad_bit > 0 && sent.is_multiple_of(o.bad_bit) {
                let at = 2 + rng.below(b.packet.len() as u64 - 2) as usize;
                b.packet[at] ^= 0x10;
            }
            Some(b.in_scene(rate, centre_hz))
        }));
        AdslScene { rate, scene }
    }
}

impl Generator for AdslScene {
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
/// [`super::AdslSetting`].
pub fn from_config(cfg: &GenConfig) -> AdslScene {
    let s = &cfg.settings;
    let o = SceneOptions {
        centre: s.get(&super::GEN_CENTRE),
        aircraft: usize::try_from(s.get(&super::GEN_AIRCRAFT)).unwrap_or(4),
        key_index: if s.get(&super::GEN_KEY_INDEX) == 3 {
            3
        } else {
            0
        },
        fec: s.get(&super::GEN_FEC),
        m_preamble: usize::try_from(s.get(&super::GEN_PREAMBLE)).unwrap_or(M_PREAMBLE),
        bad_bit: u32::try_from(s.get(&super::GEN_BAD_BIT)).unwrap_or(0),
    };
    AdslScene::demo(cfg.rate_hz, o, cfg.seed)
}
