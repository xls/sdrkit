//! The AIS test signal: clean GMSK bursts on the four channels at any rate, and the port demo (vessels moving along tracks, a base
//! station, two aids to navigation and a SAR aircraft, sending at the M.1371 reporting intervals on a slot grid). Nothing is
//! transmitted.

use super::encode::{self, PositionA, Static5};
use super::gmsk::{self, Burst};
use super::line;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};
use sdrkit_util_geo::time::Civil;
use std::f64::consts::TAU;

/// The bandwidth the test signal states for its SNR: the 48 kHz channel the receiver measures its SNR in.
pub const SNR_BANDWIDTH_HZ: f64 = 48e3;

/// One burst to send.
#[derive(Clone, Debug)]
pub struct BurstSpec {
    /// A, B, C or D.
    pub channel: char,
    /// The start of the training sequence, seconds.
    pub start_s: f64,
    /// The amplitude: 1.0 is unit mean power.
    pub level: f64,
    /// The carrier, Hz from the channel's centre.
    pub offset_hz: f64,
    /// The carrier phase at the start, radians.
    pub phase: f64,
    /// Payload bits, message-table order.
    pub bits: Vec<u8>,
}

/// Where and how long bursts are rendered.
#[derive(Clone, Debug)]
pub struct Render {
    /// The sample rate.
    pub rate: f64,
    /// The length, seconds (0 for a stream with no end).
    pub duration_s: f64,
    /// The channels sit at -+25 kHz minus this.
    pub tune_offset_hz: f64,
    /// Where 75 + 76 sit (their middle), Hz.
    pub long_range_middle_hz: f64,
}

impl Default for Render {
    fn default() -> Self {
        Render {
            rate: 250e3,
            duration_s: 1.0,
            tune_offset_hz: 0.0,
            long_range_middle_hz: -5.2e6,
        }
    }
}

/// The offset of a channel in the rendered stream.
pub fn channel_offset(ch: char, r: &Render) -> f64 {
    match ch {
        'A' => -25e3 - r.tune_offset_hz,
        'B' => 25e3 - r.tune_offset_hz,
        'C' => r.long_range_middle_hz - 25e3 - r.tune_offset_hz,
        _ => r.long_range_middle_hz + 25e3 - r.tune_offset_hz,
    }
}

/// A burst ready to modulate.
pub fn gmsk_burst(b: &BurstSpec, r: &Render) -> Burst {
    Burst {
        levels: line::burst_levels(&b.bits, 8),
        start_s: b.start_s,
        amplitude: b.level,
        offset_hz: channel_offset(b.channel, r) + b.offset_hz,
        phase: b.phase,
        clock_ppm: 0.0,
        ramp_bits: 1.0,
    }
}

/// Renders the clean bursts at the rate, for `r.duration_s`.
pub fn render(bursts: &[BurstSpec], r: &Render) -> Vec<Complex32> {
    let mut g = Streamer::new(bursts.iter().map(|b| gmsk_burst(b, r)).collect(), r.clone());
    let n = (r.duration_s * r.rate).round() as usize;
    let mut x = vec![Complex32::default(); n];
    g.generate(&mut x);
    x
}

/// Streams modulated bursts: the clean signal, at unit mean power while a level-1 burst is sent.
pub struct Streamer {
    bursts: Vec<Burst>,
    r: Render,
    pos: u64,
    end: Option<u64>,
}

impl Streamer {
    /// The bursts given, in any order.
    pub fn new(mut bursts: Vec<Burst>, r: Render) -> Self {
        bursts.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
        Streamer {
            end: (r.duration_s > 0.0).then(|| (r.duration_s * r.rate).round() as u64),
            bursts,
            r,
            pos: 0,
        }
    }

    /// Adds more bursts (the port demo schedules as it goes).
    pub fn push(&mut self, b: Burst) {
        self.bursts.push(b);
    }
}

impl Generator for Streamer {
    fn rate_hz(&self) -> f64 {
        self.r.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n = match self.end {
            Some(e) => (e.saturating_sub(self.pos) as usize).min(out.len()),
            None => out.len(),
        };
        let out = &mut out[..n];
        out.fill(Complex32::default());
        let t0 = self.pos as f64 / self.r.rate;
        let t1 = (self.pos + n as u64) as f64 / self.r.rate;
        for b in &self.bursts {
            if b.start_s < t1 && b.end_s() > t0 {
                b.add_to(out, t0, self.r.rate);
            }
        }
        self.bursts.retain(|b| b.end_s() > t1);
        self.pos += n as u64;
        n
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(signal_info())
    }
}

/// What the AIS test signal states: unit power, its SNR measured in [`SNR_BANDWIDTH_HZ`], 9600 symbols and bits a second.
pub fn signal_info() -> SignalInfo {
    SignalInfo::new(SNR_BANDWIDTH_HZ)
        .with_symbol_rate(gmsk::BIT_RATE)
        .with_bit_rate(gmsk::BIT_RATE)
}

/// What the port demo sent.
#[derive(Clone, Debug)]
pub struct Sent {
    /// The start of the burst, seconds.
    pub time: f64,
    /// The channel letter.
    pub channel: char,
    /// The station.
    pub mmsi: u32,
    /// The message type.
    pub msg_type: u8,
    /// The message bits.
    pub bits: Vec<u8>,
}

#[derive(Clone, Debug)]
enum Kind {
    ClassA {
        imo: u32,
        call: String,
        ship_type: u8,
    },
    ClassB {
        ship_type: u8,
        call: String,
    },
    ClassBExt {
        ship_type: u8,
    },
    Base,
    Aid {
        aid_type: u8,
        virtual_aid: bool,
    },
    Sar,
}

#[derive(Clone, Debug)]
struct Unit {
    mmsi: u32,
    name: String,
    kind: Kind,
    lat0: f64,
    lon0: f64,
    speed: f64,
    course: f64,
    level: f64,
    offset: f64,
    next_pos: f64,
    next_static: f64,
    next_extra: f64,
    channel: bool,
    part_b: bool,
}

/// Options of the port demo.
#[derive(Clone, Debug)]
pub struct DemoOptions {
    /// Vessels (1 to 40).
    pub vessels: usize,
    /// The seed of the vessels' places, speeds, courses and carriers.
    pub seed: u64,
    /// Farther stations are weaker.
    pub levels_by_distance: bool,
    /// No base station, aids or SAR aircraft.
    pub vessels_only: bool,
    /// Time runs this many times faster for the movements.
    pub speed_factor: f64,
    /// Where and how long.
    pub render: Render,
    /// A fixed carrier error per vessel up to this, Hz.
    pub vessel_offset_hz: f64,
}

impl Default for DemoOptions {
    fn default() -> Self {
        DemoOptions {
            vessels: 15,
            seed: 7,
            levels_by_distance: true,
            vessels_only: false,
            speed_factor: 1.0,
            render: Render {
                duration_s: 0.0,
                ..Default::default()
            },
            vessel_offset_hz: 200.0,
        }
    }
}

const NAMES: [&str; 40] = [
    "AL MARSA",
    "DHOW STAR",
    "GULF TRADER",
    "PEARL OF DUBAI",
    "SEA FALCON",
    "DESERT ROSE",
    "OCEAN SPIRIT",
    "CORAL BAY",
    "BLUE HORIZON",
    "SAND PIPER",
    "JUMEIRAH",
    "HARBOUR PILOT",
    "TUG MAJID",
    "FISHER KING",
    "SAIL AWAY",
    "NORTH WIND",
    "EAST STAR",
    "WEST WAVE",
    "SOUTH CROSS",
    "RED SEA",
    "MOON RIVER",
    "SUN DANCER",
    "STORM PETREL",
    "IRON DUKE",
    "SILVER SPRAY",
    "GOLDEN GATE",
    "AQUA MARINE",
    "LADY GRACE",
    "SEA BREEZE",
    "WAVE RUNNER",
    "BRAVE HEART",
    "LUCKY STAR",
    "NIGHT HAWK",
    "OCEAN QUEEN",
    "SEA LION",
    "SEA HORSE",
    "WIND SONG",
    "TIDE RACER",
    "SALT MARSH",
    "MERMAID",
];

/// The port demo: a streaming generator with a log of what was sent.
pub struct PortDemo {
    units: Vec<Unit>,
    opts: DemoOptions,
    stream: Streamer,
    rng: Rng,
    /// Everything sent, in order.
    pub sent: Vec<Sent>,
    scheduled_to: f64,
    used_slots: [std::collections::BTreeSet<i64>; 2],
}

const SLOT: f64 = 60.0 / 2250.0;

impl PortDemo {
    /// The demo with these options.
    pub fn new(opts: DemoOptions) -> Self {
        let mut rng = Rng::new(opts.seed);
        let mut units = Vec::new();
        let (clat, clon) = (25.0, 55.05);
        for i in 0..opts.vessels.clamp(1, 40) {
            let k = i % 15;
            let kind = match k {
                0..=9 => Kind::ClassA {
                    imo: 9_100_000 + i as u32 * 17,
                    call: format!("A6{:03}", i),
                    ship_type: [70, 80, 60, 52, 50, 30, 71, 81, 61, 40][k],
                },
                10..=13 => Kind::ClassB {
                    ship_type: [37, 36, 30, 37][k - 10],
                    call: format!("B{:04}", i),
                },
                _ => Kind::ClassBExt { ship_type: 37 },
            };
            let r = rng.range(0.005, 0.05);
            let a = rng.range(0.0, TAU);
            units.push(Unit {
                mmsi: 470_100_000 + i as u32 * 1013,
                name: NAMES[i % NAMES.len()].to_string(),
                kind,
                lat0: clat + r * a.sin(),
                lon0: clon + r * a.cos(),
                speed: if k == 3 { 0.0 } else { rng.range(2.0, 25.0) },
                course: rng.range(0.0, 360.0),
                level: 1.0,
                offset: rng.range(-1.0, 1.0) * opts.vessel_offset_hz,
                next_pos: rng.range(0.1, 2.0),
                next_static: rng.range(0.5, 3.0),
                next_extra: 0.0,
                channel: i % 2 == 0,
                part_b: false,
            });
        }
        if !opts.vessels_only {
            units.push(Unit {
                mmsi: 4_701_001,
                name: String::new(),
                kind: Kind::Base,
                lat0: clat,
                lon0: clon,
                speed: 0.0,
                course: 0.0,
                level: 1.0,
                offset: 0.0,
                next_pos: 0.3,
                next_static: 1e9,
                next_extra: 5.0,
                channel: false,
                part_b: false,
            });
            for (j, (name, t, v)) in [
                ("PORT ENTRANCE BUOY 1", 24u8, false),
                ("WRECK MARK VIRTUAL", 28u8, true),
            ]
            .iter()
            .enumerate()
            {
                units.push(Unit {
                    mmsi: 994_701_001 + j as u32,
                    name: name.to_string(),
                    kind: Kind::Aid {
                        aid_type: *t,
                        virtual_aid: *v,
                    },
                    lat0: clat + 0.02 * (j as f64 + 1.0),
                    lon0: clon - 0.01,
                    speed: 0.0,
                    course: 0.0,
                    level: 1.0,
                    offset: 0.0,
                    next_pos: 1.0 + j as f64,
                    next_static: 1e9,
                    next_extra: 0.0,
                    channel: j == 0,
                    part_b: false,
                });
            }
            units.push(Unit {
                mmsi: 111_470_001,
                name: String::new(),
                kind: Kind::Sar,
                lat0: clat + 0.03,
                lon0: clon + 0.03,
                speed: 120.0,
                course: 0.0,
                level: 1.0,
                offset: 0.0,
                next_pos: 0.7,
                next_static: 1e9,
                next_extra: 0.0,
                channel: true,
                part_b: false,
            });
        }
        if opts.levels_by_distance {
            for u in &mut units {
                let d = ((u.lat0 - clat).powi(2) + (u.lon0 - clon).powi(2)).sqrt();
                u.level = 10f64.powf(-(d / 0.06).min(1.0));
            }
        }
        let stream = Streamer::new(Vec::new(), opts.render.clone());
        PortDemo {
            units,
            rng,
            stream,
            sent: Vec::new(),
            scheduled_to: 0.0,
            used_slots: Default::default(),
            opts,
        }
    }

    /// Stations the demo holds.
    pub fn stations(&self) -> usize {
        self.units.len()
    }

    fn position(u: &Unit, t: f64, factor: f64) -> (f64, f64, f64) {
        if let Kind::Sar = u.kind {
            let a = TAU * t * factor / 120.0;
            return (
                u.lat0 + 0.01 * a.sin(),
                u.lon0 + 0.01 * a.cos(),
                ((a.to_degrees() + 90.0) % 360.0 + 360.0) % 360.0,
            );
        }
        // Along a closed loop so vessels stay in the port.
        let nm = u.speed * t * factor / 3600.0;
        let ang = nm / 3.0;
        let c = u.course.to_radians() + ang;
        let r = 3.0 / 60.0;
        let lat = u.lat0 + r * (c.sin() - u.course.to_radians().sin());
        let lon = u.lon0 + r * (c.cos() - u.course.to_radians().cos()) / u.lat0.to_radians().cos();
        (lat, lon, (90.0 - c.to_degrees()).rem_euclid(360.0))
    }

    fn slot(&mut self, t: f64, ch: usize) -> f64 {
        let mut s = (t / SLOT).ceil() as i64;
        while self.used_slots[ch].contains(&s) {
            s += 1;
        }
        self.used_slots[ch].insert(s);
        if self.used_slots[ch].len() > 4000 {
            let cut = s - 3000;
            self.used_slots[ch].retain(|&v| v > cut);
        }
        s as f64 * SLOT
    }

    fn send(&mut self, t: f64, i: usize, bits: Vec<u8>, msg_type: u8) {
        let u = &mut self.units[i];
        u.channel = !u.channel;
        let ch = usize::from(u.channel);
        // Type 5 and 19 use two slots: keep the next one free too.
        let long = bits.len() > 256;
        let (level, offset, mmsi) = (u.level, u.offset, u.mmsi);
        let start = self.slot(t, ch);
        if long {
            self.used_slots[ch].insert((start / SLOT).round() as i64 + 1);
        }
        let letter = if ch == 0 { 'A' } else { 'B' };
        let phase = self.rng.range(0.0, TAU);
        let spec = BurstSpec {
            channel: letter,
            start_s: start + 0.0005,
            level,
            offset_hz: offset,
            phase,
            bits: bits.clone(),
        };
        self.stream.push(gmsk_burst(&spec, &self.opts.render));
        self.sent.push(Sent {
            time: spec.start_s,
            channel: letter,
            mmsi,
            msg_type,
            bits,
        });
    }

    fn schedule(&mut self, until: f64) {
        let f = self.opts.speed_factor.max(0.01);
        while self.scheduled_to < until {
            let t = self.scheduled_to;
            let step = 0.05;
            for i in 0..self.units.len() {
                let u = self.units[i].clone();
                let (lat, lon, course) = Self::position(&u, t, f);
                let sec = (t as u64 % 60) as u8;
                if t >= u.next_pos {
                    let (bits, ty, interval) = match &u.kind {
                        Kind::ClassA { .. } => {
                            let ty = [1u8, 2, 3][(t as usize / 10) % 3];
                            let iv = if u.speed > 23.0 {
                                2.0
                            } else if u.speed > 14.0 {
                                6.0
                            } else {
                                10.0
                            };
                            (
                                encode::position_a(&PositionA {
                                    msg_type: ty,
                                    mmsi: u.mmsi,
                                    status: if u.speed == 0.0 { 5 } else { 0 },
                                    rot_raw: 0,
                                    sog_kn: Some(u.speed),
                                    lat: Some(lat),
                                    lon: Some(lon),
                                    cog_deg: Some((course * 10.0).round() / 10.0),
                                    heading: Some(course.round() as u16 % 360),
                                    second: sec,
                                }),
                                ty,
                                iv,
                            )
                        }
                        Kind::ClassB { .. } => (
                            encode::position_b(
                                u.mmsi,
                                Some(u.speed),
                                Some(lat),
                                Some(lon),
                                Some(course),
                                None,
                                sec,
                            ),
                            18,
                            30.0,
                        ),
                        Kind::ClassBExt { .. } => (
                            encode::position_b(
                                u.mmsi,
                                Some(u.speed),
                                Some(lat),
                                Some(lon),
                                Some(course),
                                None,
                                sec,
                            ),
                            18,
                            30.0,
                        ),
                        Kind::Base => {
                            // The demo's clock starts at 2023-11-14 22:13:20 UTC.
                            let utc = Civil::from_unix_f64(1.7e9 + t).map_or(
                                [2023, 11, 14, 22, 13, 20],
                                |c| {
                                    [
                                        c.year as u32,
                                        c.month.into(),
                                        c.day.into(),
                                        c.hour.into(),
                                        c.minute.into(),
                                        c.second.into(),
                                    ]
                                },
                            );
                            (encode::base_station(u.mmsi, utc, lat, lon), 4, 10.0)
                        }
                        Kind::Aid {
                            aid_type,
                            virtual_aid,
                        } => (
                            encode::aid_to_navigation(
                                u.mmsi,
                                *aid_type,
                                &u.name,
                                lat,
                                lon,
                                *virtual_aid,
                            ),
                            21,
                            30.0,
                        ),
                        Kind::Sar => (
                            encode::sar_aircraft(u.mmsi, 300, 120, lat, lon, course, sec),
                            9,
                            10.0,
                        ),
                    };
                    self.units[i].next_pos = t + interval;
                    self.send(t, i, bits, ty);
                }
                if t >= u.next_static {
                    match &u.kind {
                        Kind::ClassA {
                            imo,
                            call,
                            ship_type,
                        } => {
                            let bits = encode::static_voyage(&Static5 {
                                mmsi: u.mmsi,
                                imo: *imo,
                                callsign: call.clone(),
                                name: u.name.clone(),
                                ship_type: *ship_type,
                                dim: [100, 20, 10, 10],
                                eta: [6, 1, 12, 0],
                                draught_m: 7.5,
                                destination: "JEBEL ALI".into(),
                            });
                            self.send(t, i, bits, 5);
                        }
                        Kind::ClassB { ship_type, call } => {
                            let bits = if u.part_b {
                                encode::static_b_b(u.mmsi, *ship_type, "SRT", call, [8, 4, 2, 2])
                            } else {
                                encode::static_b_a(u.mmsi, &u.name)
                            };
                            self.units[i].part_b = !u.part_b;
                            self.send(t, i, bits, 24);
                        }
                        Kind::ClassBExt { ship_type } => {
                            let bits = encode::position_b_ext(
                                u.mmsi,
                                u.speed,
                                lat,
                                lon,
                                course,
                                None,
                                sec,
                                &u.name,
                                *ship_type,
                                [10, 5, 2, 2],
                            );
                            self.send(t, i, bits, 19);
                        }
                        _ => {}
                    }
                    self.units[i].next_static = t + if matches!(u.kind, Kind::ClassB { .. }) {
                        15.0
                    } else {
                        30.0
                    };
                }
                if let Kind::Base = u.kind {
                    if t >= u.next_extra {
                        self.units[i].next_extra = t + 60.0;
                        self.send(t, i, encode::safety_broadcast(u.mmsi, "PORT OPEN"), 14);
                        let mut data = Vec::new();
                        for k in 0..48 {
                            data.push(((k * 7) % 3 == 0) as u8);
                        }
                        self.send(
                            t + 0.1,
                            i,
                            encode::broadcast_binary(u.mmsi, 1, 29, &data),
                            8,
                        );
                    }
                }
            }
            self.scheduled_to += step;
        }
    }
}

impl Generator for PortDemo {
    fn rate_hz(&self) -> f64 {
        self.opts.render.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let t_end = (self.stream.pos + out.len() as u64) as f64 / self.opts.render.rate;
        self.schedule(t_end + 0.2);
        self.stream.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(signal_info())
    }
}

const GEN_VESSELS: Setting<field::I64> = Setting::new("gen_vessels", "Vessels", "", 15)
    .range(1.0, 40.0, 1.0)
    .help("Vessels in the port demo, beside the base station, two aids and a SAR aircraft.")
    .section("Test signal")
    .advanced();
const GEN_SPEED: Setting<field::F64> = Setting::new("gen_speed", "Speed of time", "", 1.0)
    .range(0.1, 100.0, 0.1)
    .help("Vessels move this many times faster than real time.")
    .section("Test signal")
    .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the port demo.
    pub enum AisGenSetting {
        /// Vessels in the port demo.
        Vessels = GEN_VESSELS,
        /// How much faster than real time the vessels move.
        Speed = GEN_SPEED,
    }
}

/// The generator the descriptor makes: the port demo with the test-signal settings.
pub fn from_config(cfg: &GenConfig) -> Box<dyn Generator> {
    let s = &cfg.settings;
    let opts = DemoOptions {
        vessels: s.get(&GEN_VESSELS) as usize,
        seed: cfg.seed,
        speed_factor: s.get(&GEN_SPEED),
        render: Render {
            rate: cfg.rate_hz,
            duration_s: 0.0,
            ..Default::default()
        },
        ..Default::default()
    };
    Box::new(PortDemo::new(opts))
}
