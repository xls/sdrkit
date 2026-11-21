//! The BGAN test signal: one forward bearer or a group of carriers, each with frames of the real structure (unique word, pilots,
//! FEC blocks coded with the published tables) and synthetic broadcast signalling of its own (not real network values). The
//! group also holds a filtered OQPSK carrier at 135.4 ksym/s, the shape of a GMR-2 class carrier. The generator adds its own
//! white noise so that each carrier has the C/N0 asked for, and places the carriers itself: the samples are centred on the dial.

use super::codec::{encode_block, IqLabels};
use super::pdu::{self, Bulletin, CodeRate};
use super::tables::{self, Bearer, Modulation, BEARERS, SUBTYPES, UW_LEN};
use sdrkit_algo_dsp::pulse::{PulseTable, Shape};
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting};
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The total noise power of the test signal over the whole sample rate.
pub const NOISE_POWER: f64 = 0.01;

sdrkit_core::choice_enum! {
    /// What the test signal holds.
    pub enum GenBearer {
        /// The group of carriers.
        Group = ("0", "A group of carriers"),
        /// F80T0.25Q-1B.
        B1 = ("1", "F80T0.25Q-1B"),
        /// F80T1Q-1B.
        B2 = ("2", "F80T1Q-1B"),
        /// F80T1Q-4B.
        B3 = ("3", "F80T1Q-4B"),
        /// F80T1X-4B.
        B4 = ("4", "F80T1X-4B"),
        /// FR80T2.5X4/16-5B.
        B5 = ("5", "FR80T2.5X4/16-5B"),
        /// FR80T2.5X16-5B.
        B6 = ("6", "FR80T2.5X16-5B"),
        /// FR80T2.5X32-6B.
        B7 = ("7", "FR80T2.5X32-6B"),
        /// FR80T2.5X64-7B.
        B8 = ("8", "FR80T2.5X64-7B"),
        /// F80T4.5X-8B.
        B9 = ("9", "F80T4.5X-8B"),
        /// FR80T5X4/16-9B.
        B10 = ("10", "FR80T5X4/16-9B"),
        /// FR80T5X16-9B.
        B11 = ("11", "FR80T5X16-9B"),
        /// FR80T5X32-11B.
        B12 = ("12", "FR80T5X32-11B"),
        /// FR80T5X64-13B.
        B13 = ("13", "FR80T5X64-13B"),
    }
}

sdrkit_core::choice_enum! {
    /// A subtype.
    pub enum GenSubtype {
        /// L8.
        L8 = ("l8", "L8"),
        /// L7.
        L7 = ("l7", "L7"),
        /// L6.
        L6 = ("l6", "L6"),
        /// L5.
        L5 = ("l5", "L5"),
        /// L4.
        L4 = ("l4", "L4"),
        /// L3.
        L3 = ("l3", "L3"),
        /// L2.
        L2 = ("l2", "L2"),
        /// L1.
        L1 = ("l1", "L1"),
        /// R.
        R = ("r", "R"),
        /// H1.
        H1 = ("h1", "H1"),
        /// H2.
        H2 = ("h2", "H2"),
        /// H3.
        H3 = ("h3", "H3"),
        /// H4.
        H4 = ("h4", "H4"),
        /// H5.
        H5 = ("h5", "H5"),
        /// H6.
        H6 = ("h6", "H6"),
    }
}

/// What the test signal holds.
pub const GEN_BEARER: Setting<field::Choice<GenBearer>> =
    Setting::choice("gen_bearer", "Bearers", GenBearer::Group)
        .section("Test signal")
        .advanced();
/// The subtype of the unique word of a single bearer.
pub const GEN_SUBTYPE: Setting<field::Choice<GenSubtype>> =
    Setting::choice("gen_subtype", "Subtype", GenSubtype::R)
        .section("Test signal")
        .advanced();
/// The C/N0 of each carrier.
pub const GEN_CARRIER_CN0: Setting<field::F64> =
    Setting::new("gen_carrier_cn0", "Carrier C/N0", "dB-Hz", 70.0)
        .range(20.0, 100.0, 0.5)
        .section("Test signal")
        .advanced();
/// The carrier of a single bearer from the dial.
pub const GEN_OFFSET_HZ: Setting<field::F64> =
    Setting::new("gen_carrier_offset_hz", "Carrier from the dial", "Hz", 0.0)
        .range(-900e3, 900e3, 1.0)
        .section("Test signal")
        .advanced();
/// Clock error of the symbols.
pub const GEN_CLOCK_PPM: Setting<field::F64> = Setting::new("gen_carrier_clock_ppm", "Symbol clock error", "ppm", 0.0)
    .range(-100.0, 100.0, 0.5)
    .section("Test signal")
    .advanced();
/// Frequency drift.
pub const GEN_DRIFT: Setting<field::F64> = Setting::new("gen_carrier_drift_hz_s", "Carrier drift", "Hz/s", 0.0)
    .range(-1000.0, 1000.0, 1.0)
    .section("Test signal")
    .advanced();
/// Fading rate.
pub const GEN_FADE_HZ: Setting<field::F64> = Setting::new("gen_fade_hz", "Fading rate", "Hz", 0.0)
    .range(0.0, 100.0, 0.5)
    .section("Test signal")
    .advanced()
    .help("0 for none.");
/// Fading depth.
pub const GEN_FADE_DEPTH: Setting<field::F64> = Setting::new("gen_fade_depth", "Fading depth", "", 0.0)
    .range(0.0, 1.0, 0.05)
    .section("Test signal")
    .advanced()
    .help("The share of the amplitude that the fading takes away at its deepest.");
/// The spot beam of the bulletin board of a single bearer.
pub const GEN_SPOT: Setting<field::I64> = Setting::new("gen_spot", "Spot beam", "", 42)
    .range(0.0, 255.0, 1.0)
    .section("Test signal")
    .advanced();

/// One carrier of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct CarrierSpec {
    /// The bearer; `None` for the GMR-2 class OQPSK carrier.
    pub bearer: Option<&'static Bearer>,
    /// Subtype index of the unique word.
    pub subtype: usize,
    /// Hz from the centre of the samples.
    pub offset_hz: f64,
    /// dB-Hz.
    pub cn0_dbhz: f64,
    /// Clock error, ppm.
    pub clock_ppm: f64,
    /// Drift, Hz/s.
    pub drift_hz_s: f64,
    /// Fading rate, Hz (0 for none).
    pub fade_hz: f64,
    /// Fading depth, 0 to 1.
    pub fade_depth: f64,
    /// Spot beam id of the bulletin board.
    pub spot: u8,
    /// Beam type of BeamInfo (0 global, 2 narrow).
    pub beam_type: u8,
}

impl CarrierSpec {
    /// A carrier of `bearer` at subtype R, `offset_hz`, 70 dB-Hz, narrow beam, spot 42.
    pub fn new(bearer: Option<&'static Bearer>, offset_hz: f64) -> Self {
        CarrierSpec {
            bearer,
            subtype: tables::R,
            offset_hz,
            cn0_dbhz: 70.0,
            clock_ppm: 0.0,
            drift_hz_s: 0.0,
            fade_hz: 0.0,
            fade_depth: 0.0,
            spot: 42,
            beam_type: 2,
        }
    }
}

/// The carriers of the group: a global-beam bulletin carrier, BGAN bearers of every symbol rate and a GMR-2 class carrier.
pub fn group() -> Vec<CarrierSpec> {
    let b = |name: &str| tables::bearer_by_name(name);
    let mut v = vec![
        CarrierSpec {
            subtype: 0,
            spot: 0,
            beam_type: 0,
            ..CarrierSpec::new(b("F80T0.25Q-1B"), -720e3)
        },
        CarrierSpec::new(b("F80T1Q-4B"), -640e3),
        CarrierSpec::new(b("F80T1X-4B"), -560e3),
        CarrierSpec::new(b("FR80T2.5X4/16-5B"), -420e3),
        CarrierSpec::new(b("F80T4.5X-8B"), -200e3),
        CarrierSpec {
            spot: 77,
            ..CarrierSpec::new(b("FR80T5X16-9B"), 160e3)
        },
        CarrierSpec {
            spot: 77,
            ..CarrierSpec::new(b("FR80T2.5X32-6B"), 380e3)
        },
        CarrierSpec::new(None, 640e3),
    ];
    for c in &mut v {
        if c.bearer.is_some_and(|b| b.dynamic()) {
            c.subtype = 5;
        }
    }
    v
}

/// The carriers of a generator configuration.
pub fn specs_of(cfg: &GenConfig) -> Vec<CarrierSpec> {
    let s = &cfg.settings;
    let choice = s.get(&GEN_BEARER);
    let mut v = match choice {
        GenBearer::Group => group(),
        other => {
            let i = sdrkit_core::value::Choice::index(other) as usize - 1;
            let mut c = CarrierSpec::new(BEARERS.get(i), s.get(&GEN_OFFSET_HZ));
            c.subtype = sdrkit_core::value::Choice::index(s.get(&GEN_SUBTYPE)) as usize;
            c.spot = s.get(&GEN_SPOT).clamp(0, 255) as u8;
            vec![c]
        }
    };
    for c in &mut v {
        c.cn0_dbhz = s.get(&GEN_CARRIER_CN0);
        c.clock_ppm = s.get(&GEN_CLOCK_PPM);
        c.drift_hz_s = s.get(&GEN_DRIFT);
        c.fade_hz = s.get(&GEN_FADE_HZ);
        c.fade_depth = s.get(&GEN_FADE_DEPTH);
    }
    v
}

/// The octets of block `blk` of frame `frame` of a carrier.
fn block_data(spec: &CarrierSpec, blk: usize, frame: u64, k: usize, rates: &[CodeRate]) -> Vec<u8> {
    let mut v = Vec::new();
    if blk == 0 {
        let bb = Bulletin {
            rnc: 7,
            net_ver: 2,
            frame: (frame % 4096) as u16,
            f_bearer: 1,
            bct_id: 3,
            spot: Some(spec.spot),
            avps: Vec::new(),
        };
        let mut avps = vec![
            pdu::avp_satellite(3, 249),
            pdu::avp_beam(spec.beam_type, spec.spot),
            pdu::avp_plmn("901", "11"),
            pdu::avp_primary(spec.spot, tables::forward_channel(1545.0e6), 1, 5),
            pdu::avp_time(2026, 11, 9, 12, 20, 30, 0),
        ];
        if !rates.is_empty() {
            avps.push(pdu::avp_code_rates(rates));
        }
        let mut content = pdu::sdu(pdu::sdu_type::BULLETIN, &pdu::bulletin(&bb, &avps), true);
        content.extend(pdu::sdu(
            pdu::sdu_type::RETURN_SCHEDULE,
            &pdu::return_schedule(0x0500, &[7, 9]),
            false,
        ));
        v = pdu::broadcast_pdu(&content);
    } else if blk % 2 == 1 {
        let body: Vec<u8> = (0..10).map(|i| (frame as u8).wrapping_mul(31).wrapping_add(i)).collect();
        v = pdu::connection_pdu(0x123, &body);
    }
    v.resize(k / 8, 0);
    v
}

/// The code rates a carrier announces for the next frame: on a bearer with dynamic modulation the even blocks after block 0
/// switch to 16-QAM at subtype R.
fn announced(b: &Bearer) -> Vec<CodeRate> {
    if !b.dynamic() {
        return Vec::new();
    }
    (2..b.blocks)
        .step_by(2)
        .map(|blk| CodeRate {
            higher: true,
            block: blk as u8,
            rate: 0,
        })
        .collect()
}

/// The symbols of one frame of a carrier.
fn frame_symbols(
    spec: &CarrierSpec,
    b: &Bearer,
    frame: u64,
    rates: &[CodeRate],
    msb_first: bool,
    labels: IqLabels,
    rng: &mut Rng,
) -> Vec<Complex32> {
    let mut out = vec![Complex32::default(); b.mos];
    for (i, o) in out.iter_mut().enumerate().take(UW_LEN) {
        *o = b.uw_symbol(tables::uw_bit(spec.subtype, i, msb_first));
    }
    for k in 0..b.ps {
        out[b.pilot_position(k)] = b.pilot();
    }
    let qpsk = Modulation::Qpsk.points();
    let Some(family) = b.family else {
        // No published tables: random QPSK data in the real frame structure.
        for d in 0..b.data_symbols() {
            out[b.data_position(d)] = qpsk[(rng.next_u32() & 3) as usize];
        }
        return out;
    };
    let mut blocks: Vec<Vec<Complex32>> = Vec::with_capacity(b.blocks);
    for blk in 0..b.blocks {
        let (fam, sub) = match rates.iter().find(|r| usize::from(r.block) == blk && blk > 0) {
            Some(r) => (
                b.block_family(r.higher && b.dynamic()).unwrap_or(family),
                (tables::R as i32 + i32::from(r.rate)).clamp(0, 14) as usize,
            ),
            None => (family, spec.subtype),
        };
        let Some(t) = tables::table_index(&fam, sub).map(|i| &tables::tables()[i]) else {
            blocks.push(vec![Complex32::default(); b.teo]);
            continue;
        };
        let next = announced(b);
        let data = block_data(spec, blk, frame, t.k(), &next);
        let points = fam.modulation.points();
        blocks.push(
            encode_block(t, &data, labels)
                .into_iter()
                .map(|v| points[usize::from(v)])
                .collect(),
        );
    }
    for d in 0..b.data_symbols() {
        let (blk, s) = b.block_symbol(d);
        out[b.data_position(d)] = blocks[blk][s];
    }
    out
}

/// One carrier being made.
struct Live {
    spec: CarrierSpec,
    /// Symbols per second with the clock error.
    rate: f64,
    amp: f32,
    pulse: PulseTable,
    half: f64,
    /// Queued symbols; `syms[0]` is symbol `first`.
    syms: VecDeque<Complex32>,
    first: i64,
    frame: u64,
    rates: Vec<CodeRate>,
    /// OQPSK rails of the GMR-2 class carrier.
    oqpsk: bool,
    phase: f64,
}

/// The test signal generator.
pub struct BganGenerator {
    rate: f64,
    carriers: Vec<Live>,
    msb_first: bool,
    labels: IqLabels,
    rng: Rng,
    noise: Option<Awgn>,
    n: u64,
}

impl BganGenerator {
    /// The generator of a stack configuration.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let msb_first = cfg.settings.get(&super::UW_ORDER) == super::UwOrder::MsbFirst;
        let labels = match cfg.settings.get(&super::IQ_LABELS) {
            super::IqLabelChoice::InOrder => IqLabels::InOrder,
            super::IqLabelChoice::Swapped => IqLabels::Swapped,
        };
        Self::new(cfg.rate_hz, specs_of(cfg), msb_first, labels, cfg.seed)
    }

    /// A generator at `rate` of `specs`.
    pub fn new(rate: f64, specs: Vec<CarrierSpec>, msb_first: bool, labels: IqLabels, seed: u64) -> Self {
        let n0 = NOISE_POWER / rate;
        let carriers = specs
            .into_iter()
            .map(|spec| {
                let (sym_rate, beta) = match spec.bearer {
                    Some(b) => (b.rate, b.rolloff),
                    None => (super::rx::GMR2_RATE, 0.35),
                };
                let amp = (n0 * 10f64.powf(spec.cn0_dbhz / 10.0)).sqrt() as f32;
                Live {
                    rate: sym_rate * (1.0 + spec.clock_ppm * 1e-6),
                    amp,
                    pulse: PulseTable::new(Shape::Rrc { beta }, 8.0, 256).unwrap_or_else(|_| {
                        PulseTable::new(Shape::Rrc { beta: 0.25 }, 8.0, 256).expect("an RRC table")
                    }),
                    half: 8.0,
                    syms: VecDeque::new(),
                    first: -8,
                    frame: 0,
                    rates: Vec::new(),
                    oqpsk: spec.bearer.is_none(),
                    phase: 0.0,
                    spec,
                }
            })
            .collect();
        BganGenerator {
            rate,
            carriers,
            msb_first,
            labels,
            rng: Rng::new(seed ^ 0xB6A7),
            noise: Awgn::with_power(NOISE_POWER, seed.wrapping_add(17)).ok(),
            n: 0,
        }
    }

    /// The carriers.
    pub fn specs(&self) -> Vec<CarrierSpec> {
        self.carriers.iter().map(|c| c.spec.clone()).collect()
    }
}

impl Live {
    /// Queues symbols until symbol `upto` exists.
    fn fill(&mut self, upto: i64, msb_first: bool, labels: IqLabels, rng: &mut Rng) {
        while self.first + (self.syms.len() as i64) <= upto {
            if self.first + (self.syms.len() as i64) < 0 {
                self.syms.push_back(Complex32::default());
                continue;
            }
            match self.spec.bearer {
                Some(b) => {
                    let syms = frame_symbols(&self.spec, b, self.frame, &self.rates, msb_first, labels, rng);
                    self.rates = announced(b);
                    self.frame += 1;
                    self.syms.extend(syms);
                }
                None => {
                    // OQPSK: symbol pairs (I, Q) of the two rails, each a sign scaled to unit power.
                    let a = std::f32::consts::FRAC_1_SQRT_2;
                    for _ in 0..1024 {
                        let i = if rng.next_u32() & 1 == 1 { a } else { -a };
                        let q = if rng.next_u32() & 1 == 1 { a } else { -a };
                        self.syms.push_back(Complex32::new(i, q));
                    }
                }
            }
        }
    }

    /// The baseband value at symbol time `u`.
    fn value(&self, u: f64) -> Complex32 {
        let lo = (u - self.half).ceil() as i64;
        let hi = (u + self.half).floor() as i64;
        let mut acc = Complex32::default();
        for m in lo..=hi {
            let Some(s) = self.syms.get((m - self.first) as usize) else {
                continue;
            };
            if self.oqpsk {
                let pi = self.pulse.value(u - m as f64);
                let pq = self.pulse.value(u - m as f64 - 0.5);
                acc += Complex32::new(s.re * pi, s.im * pq);
            } else {
                acc += s * self.pulse.value(u - m as f64);
            }
        }
        acc
    }
}

impl Generator for BganGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        let fs = self.rate;
        for c in &mut self.carriers {
            let t_end = (self.n + out.len() as u64) as f64 / fs;
            c.fill((t_end * c.rate + c.half + 2.0) as i64, self.msb_first, self.labels, &mut self.rng);
            for (k, o) in out.iter_mut().enumerate() {
                let t = (self.n + k as u64) as f64 / fs;
                let f = c.spec.offset_hz + c.spec.drift_hz_s * t;
                c.phase = (c.phase + TAU * f / fs) % TAU;
                let fade = if c.spec.fade_hz > 0.0 {
                    1.0 - c.spec.fade_depth * (0.5 + 0.5 * (TAU * c.spec.fade_hz * t).sin())
                } else {
                    1.0
                };
                let v = c.value(t * c.rate) * (c.amp * fade as f32);
                *o += v * Complex32::new(c.phase.cos() as f32, c.phase.sin() as f32);
            }
            // Drop symbols no longer needed.
            let keep_from = ((self.n + out.len() as u64) as f64 / fs * c.rate - c.half - 2.0) as i64;
            while c.first < keep_from && !c.syms.is_empty() {
                c.syms.pop_front();
                c.first += 1;
            }
        }
        if let Some(n) = self.noise.as_mut() {
            n.process(out);
        }
        self.n += out.len() as u64;
        out.len()
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
}

/// The name of subtype index `i`.
pub fn subtype_name(i: usize) -> &'static str {
    SUBTYPES.get(i).copied().unwrap_or("?")
}
