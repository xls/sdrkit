//! The Orbcomm test signal: one or two downlinks (satellites 23 and 41) with the packet framing, SDPSK, RRC 0.4, a fixed Doppler or
//! a triangular ramp, a share of packets with a broken check, and a circular orbit (775 km, 45 degrees) whose ephemeris the
//! satellites send. Each downlink is at unit power. Noise, a sample clock error and other impairments come from the `gen_*` keys.

use super::demod::BAUD;
use super::packet::{build, Elements, Ephemeris, Packet, FRAME_PACKETS, PACKET};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The downlinks of the test signal.
pub const GEN_DOWNLINKS: Setting<field::I64> = Setting::new("gen_downlinks", "Downlinks", "", 1)
    .range(1.0, 2.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();
/// The first downlink's Doppler.
pub const GEN_SATELLITE_DOPPLER_HZ: Setting<field::Hz> = Setting::new(
    "gen_satellite_doppler_hz",
    "Satellite Doppler",
    "Hz",
    1200.0,
)
.range(-10e3, 10e3, 10.0)
.help("The first downlink's Doppler. The second has the opposite sign.")
.section(TEST_SIGNAL)
.advanced();
/// The span of a Doppler ramp (0: the fixed Doppler).
pub const GEN_RAMP_HZ: Setting<field::Hz> =
    Setting::new("gen_ramp_hz", "Doppler ramp (+-)", "Hz", 0.0)
        .range(0.0, 10e3, 10.0)
        .help("0: the fixed Doppler. Else a triangular ramp between + and - this.")
        .section(TEST_SIGNAL)
        .advanced();
/// One leg of the ramp.
pub const GEN_RAMP_S: Setting<field::Duration> = Setting::new("gen_ramp_s", "Ramp leg", "s", 20.0)
    .range(1.0, 600.0, 1.0)
    .section(TEST_SIGNAL)
    .advanced();
/// The share of packets sent with a broken check.
pub const GEN_BROKEN: Setting<field::F64> = Setting::new("gen_broken", "Broken checks", "", 0.0)
    .range(0.0, 1.0, 0.01)
    .help("The share of packets sent with a broken Fletcher check.")
    .section(TEST_SIGNAL)
    .advanced();
/// Where the radio is tuned.
pub const GEN_CENTRE_HZ: Setting<field::Hz> =
    Setting::new("gen_centre_hz", "Radio centre", "Hz", 137.275e6)
        .range(136e6, 139e6, 1.0)
        .help(
            "Where the radio is tuned, so the generator puts the downlinks in the right place.",
        )
        .section(TEST_SIGNAL)
        .advanced();

/// What the test signal holds.
#[derive(Clone, Debug)]
pub struct GenParams {
    pub rate: f64,
    /// The radio's centre (absolute).
    pub centre_hz: f64,
    pub downlinks: usize,
    pub doppler_hz: f64,
    pub ramp_hz: f64,
    pub ramp_s: f64,
    pub broken: f64,
    pub seed: u64,
}

impl GenParams {
    /// One downlink at +1200 Hz, the radio 25 kHz above the first channel.
    pub fn new(rate: f64) -> Self {
        GenParams {
            rate,
            centre_hz: 137.275e6,
            downlinks: 1,
            doppler_hz: 1200.0,
            ramp_hz: 0.0,
            ramp_s: 20.0,
            broken: 0.0,
            seed: 1,
        }
    }
}

/// The satellites and their raster channels.
pub const SATS: [(u8, f64); 2] = [(23, 137.25e6), (41, 137.2875e6)];
pub const ALT_KM: f64 = 775.0;
pub const INCL_DEG: f64 = 45.0;
pub const CLOCK_START_S: f64 = 600.0;

/// The made-up circular orbit: Earth-fixed position (km) of satellite `k` at clock `t` seconds.
pub fn position(k: usize, t: f64) -> (f64, f64, f64) {
    let r = 6378.137 + ALT_KM;
    let n = (398_600.441_8 / (r * r * r)).sqrt();
    let raan = (40.0 + 70.0 * k as f64).to_radians();
    let u = (10.0 + 100.0 * k as f64).to_radians() + n * t;
    let i = INCL_DEG.to_radians();
    let x = r * (raan.cos() * u.cos() - raan.sin() * u.sin() * i.cos());
    let y = r * (raan.sin() * u.cos() + raan.cos() * u.sin() * i.cos());
    let z = r * u.sin() * i.sin();
    let th = 7.292_115e-5 * t;
    (x * th.cos() + y * th.sin(), -x * th.sin() + y * th.cos(), z)
}

/// The orbit's period, minutes.
pub fn period_min() -> f64 {
    let r = 6378.137 + ALT_KM;
    TAU / (398_600.441_8 / (r * r * r)).sqrt() / 60.0
}

/// The messages the satellites send.
pub const MESSAGES: [&str; 4] = [
    "HELLO FROM ORBCOMM TEST",
    "POSITION REPORT 42N 12E",
    "WIND 270 AT 15 KT",
    "BUOY 7 OK",
];

struct Downlink {
    sat: u8,
    k: usize,
    m: Linear,
    nco: Nco,
    bits: VecDeque<bool>,
    phase: f64,
    frame: u32,
    msg: usize,
    part: u8,
    rng: Rng,
    raster: f64,
    /// Modulated samples not sent yet, and the next one to send.
    queue: Vec<Complex32>,
    pos: usize,
}

/// The Orbcomm test signal.
pub struct OrbGenerator {
    p: GenParams,
    links: Vec<Downlink>,
    n: u64,
}

impl OrbGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let mut p = GenParams::new(cfg.rate_hz);
        p.downlinks = usize::try_from(s.get(&GEN_DOWNLINKS)).unwrap_or(1);
        p.doppler_hz = s.get(&GEN_SATELLITE_DOPPLER_HZ);
        p.ramp_hz = s.get(&GEN_RAMP_HZ);
        p.ramp_s = s.get(&GEN_RAMP_S).max(1.0);
        p.broken = s.get(&GEN_BROKEN);
        p.centre_hz = s.get(&GEN_CENTRE_HZ);
        p.seed = cfg.seed;
        Self::new(p)
    }

    /// The generator of these parameters.
    pub fn new(p: GenParams) -> Self {
        let n = p.downlinks.clamp(1, 2);
        let links = (0..n)
            .map(|k| {
                let (sat, raster) = SATS[k];
                Downlink {
                    sat,
                    k,
                    m: Linear::rrc(0.4, 12.0, p.rate / BAUD).expect("a valid RRC pulse"),
                    nco: Nco::new(p.rate, raster - p.centre_hz),
                    bits: VecDeque::new(),
                    phase: 0.0,
                    frame: 0,
                    msg: k,
                    part: 0,
                    rng: Rng::new(p.seed ^ (0x0B0B + k as u64)),
                    raster,
                    queue: Vec::new(),
                    pos: 0,
                }
            })
            .collect();
        OrbGenerator { links, n: 0, p }
    }

    /// The Doppler of downlink `k` at signal time `t`.
    pub fn doppler(&self, k: usize, t: f64) -> f64 {
        let sign = if k == 0 { 1.0 } else { -1.0 };
        if self.p.ramp_hz <= 0.0 {
            return self.p.doppler_hz * sign;
        }
        let x = (t / self.p.ramp_s) % 2.0;
        let tri = if x < 1.0 {
            1.0 - 2.0 * x
        } else {
            -1.0 + 2.0 * (x - 1.0)
        };
        self.p.ramp_hz * tri * sign
    }
}

/// The 50 packets of one frame.
fn frame_packets(d: &mut Downlink, broken: f64) -> Vec<[u8; PACKET]> {
    let clock = CLOCK_START_S + d.frame as f64;
    let mut v = Vec::with_capacity(FRAME_PACKETS);
    v.push(build(&Packet::Sync {
        satellite: d.sat,
        frame: d.frame as u8,
    }));
    let (x, y, z) = position(d.k, clock);
    v.push(build(&Packet::Ephemeris(Ephemeris {
        x_km: x,
        y_km: y,
        z_km: z,
        seconds: (clock as u32) % 3600,
        ..Default::default()
    })));
    let r = 6378.137 + ALT_KM;
    let n = (398_600.441_8 / (r * r * r)).sqrt();
    let u = ((10.0 + 100.0 * d.k as f64).to_radians() + n * clock).rem_euclid(TAU);
    v.push(build(&Packet::Elements(Elements {
        satellite: d.sat,
        inclination: INCL_DEG,
        raan: 40.0 + 70.0 * d.k as f64,
        arg_latitude: u * 180.0 / PI,
        period_min: period_min(),
    })));
    v.push(build(&Packet::Downlink {
        channels: vec![d.raster, d.raster + 37.5e3],
    }));
    v.push(build(&Packet::Uplink {
        channels: vec![148.0e6 + 400.0 * 2.5e3, 148.0e6 + 420.0 * 2.5e3],
    }));
    v.push(build(&Packet::Network {
        gateway: 3,
        status: 0x0F,
    }));
    // Message parts.
    for _ in 0..6 {
        let text = MESSAGES[d.msg % MESSAGES.len()];
        let chunks: Vec<String> = text
            .as_bytes()
            .chunks(7)
            .map(|c| String::from_utf8_lossy(c).to_string())
            .collect();
        let last = d.part as usize + 1 >= chunks.len();
        v.push(build(&Packet::Message {
            id: (d.msg % 100) as u8,
            part: d.part,
            last,
            text: chunks[d.part as usize].clone(),
        }));
        if last {
            d.msg += 1;
            d.part = 0;
        } else {
            d.part += 1;
        }
    }
    while v.len() < FRAME_PACKETS {
        v.push(build(&Packet::Fill));
    }
    // Broken checksums (never the sync packet).
    for p in v.iter_mut().skip(1) {
        if d.rng.uniform() < broken {
            p[10] ^= 0x5A;
        }
    }
    d.frame += 1;
    v
}

impl Downlink {
    /// The next modulated sample at 0 Hz.
    fn next(&mut self, broken: f64) -> Complex32 {
        while self.pos >= self.queue.len() {
            self.queue.clear();
            self.pos = 0;
            if self.bits.len() < 64 {
                for p in frame_packets(self, broken) {
                    for byte in p {
                        for i in 0..8 {
                            self.bits.push_back((byte >> i) & 1 == 1);
                        }
                    }
                }
            }
            let b = self.bits.pop_front().unwrap_or(false);
            self.phase = (self.phase + if b { PI / 2.0 } else { -PI / 2.0 }) % TAU;
            let sym = Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32);
            self.m.push(&[sym], &mut self.queue);
        }
        self.pos += 1;
        self.queue[self.pos - 1]
    }
}

impl Generator for OrbGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let broken = self.p.broken;
        for (j, o) in out.iter_mut().enumerate() {
            let n = self.n + j as u64;
            let mut v = Complex32::new(0.0, 0.0);
            for k in 0..self.links.len() {
                if n.is_multiple_of(256) {
                    let f = self.links[k].raster - self.p.centre_hz
                        + self.doppler(k, n as f64 / self.p.rate);
                    self.links[k].nco.set_freq(f);
                }
                let l = &mut self.links[k];
                let s = l.next(broken);
                v += s * l.nco.next_sample();
            }
            *o = v;
        }
        self.n += out.len() as u64;
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Each downlink at unit power: Es/N0 is per downlink.
        let mut i = SignalInfo::new(BAUD * 1.4);
        i.symbol_rate_hz = Some(BAUD);
        i.bit_rate_hz = Some(BAUD);
        Some(i)
    }
}
