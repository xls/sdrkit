//! The ACARS test signal: one or more VHF channels around the station, aircraft sending downlinks and the ground answering, a carrier
//! amplitude modulated by the MSK audio, with an answer key of every block sent. The signal is clean: the engine adds the noise.
//! Standard: ARINC 618 clause 2.3 (MSK 1200 and 2400 Hz at 2400 bit/s, AM of the carrier).

use crate::messages::{adsc_envelope, AdscReport};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_proto_aero::acars::assemble::Quality;
use sdrkit_proto_aero::acars::{self, strip_parity, BlockSpec, End, NAK};
use std::collections::BinaryHeap;

/// The scene.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// Output rate.
    pub rate_hz: f64,
    /// The frequency of the centre of the samples, Hz: the station of the session, where the engine puts it. 0 makes the
    /// channels offsets from that centre.
    pub centre_hz: f64,
    /// Aircraft (1 - 24).
    pub aircraft: usize,
    /// Seed.
    pub seed: u64,
    /// AM depth, 0.1 - 0.95.
    pub depth: f64,
    /// The channel frequencies, Hz, on the same scale as `centre_hz`.
    pub channels: Vec<f64>,
    /// Traffic rate factor, 0.5 - 4 (more is accepted for tests).
    pub rate_factor: f64,
    /// Extra attenuation of the last channel, dB.
    pub last_atten_db: f64,
    /// A fixed text for every downlink (label H1).
    pub fixed_text: Option<String>,
    /// A fixed registration for the first aircraft.
    pub fixed_reg: Option<String>,
    /// Ground replies.
    pub uplinks: bool,
    /// Position reports from the first six aircraft.
    pub positions: bool,
    /// The time of the first downlink of the first aircraft, s; `None` draws it like those of the others.
    pub first_s: Option<f64>,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            rate_hz: 2e6,
            centre_hz: 0.0,
            aircraft: 6,
            seed: 1,
            depth: 0.6,
            // One channel one step of the 25 kHz grid above the station: the receiver skips a carrier at the centre.
            channels: vec![25e3],
            rate_factor: 1.0,
            last_atten_db: 0.0,
            fixed_text: None,
            fixed_reg: None,
            uplinks: true,
            positions: false,
            first_s: None,
        }
    }
}

/// A block the generator sent.
#[derive(Clone, Debug)]
pub struct Sent {
    /// Start of the burst, s.
    pub start_s: f64,
    /// End of the check bytes, s.
    pub end_s: f64,
    /// Channel, Hz.
    pub freq_hz: f64,
    /// The block as a message.
    pub msg: acars::Message,
}

struct Burst {
    start: f64,
    mod_start: f64,
    mod_end: f64,
    end: f64,
    bits: Vec<bool>,
    amp: f64,
    freq: f64,
    phase: f64,
    aphase: f64,
    started: bool,
}

#[derive(PartialEq)]
struct Req(f64, u64, usize, u8);
impl Eq for Req {}
impl PartialOrd for Req {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Req {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

struct Plane {
    reg: String,
    flight: String,
    chan: usize,
    level: f64,
    offset: f64,
    cycle: usize,
    msg_no: u32,
    block_id: u8,
    last_dl_id: u8,
    pos: (f64, f64, f64),
    vel: (f64, f64),
    pos_kind: usize,
    pos_count: u32,
}

const REGS: [&str; 24] = [
    "G-EUUA", "D-AIBC", "F-GKXA", "EI-DVM", "PH-BXA", "OO-SNA", "HB-JZA", "SE-RJE", "LN-NOA",
    "OY-KBA", "EC-MXV", "I-ADJA", "OE-LBA", "SP-LWA", "CS-TNA", "TC-JHA", "9H-AEA", "G-TTNA",
    "D-AXYZ", "F-HBLA", "EI-FMA", "PH-EXA", "OO-JAA", "HB-IJA",
];
const AIRLINES: [&str; 12] = [
    "BA", "LH", "AF", "EI", "KL", "SN", "LX", "SK", "DY", "OS", "IB", "AZ",
];
const AIRPORTS: [&str; 8] = [
    "EGLL", "EDDF", "LFPG", "EIDW", "EHAM", "EBBR", "LSZH", "ESSA",
];
const LABELS: [&str; 12] = [
    "QA", "5Z", "QB", "H1", "H1", "80", "H1", "10", "QC", "QD", "SA", "Q0",
];

/// The ACARS generator.
pub struct AcarsGenerator {
    p: GenParams,
    rng: Rng,
    eff_rate: f64,
    n: u64,
    planes: Vec<Plane>,
    chans: Vec<f64>,
    free: Vec<f64>,
    reqs: BinaryHeap<Req>,
    seq: u64,
    planned_to: f64,
    bursts: Vec<Burst>,
    uplink_id: u8,
    nominal: f64,
    /// Every block sent, in the order planned.
    pub sent: Vec<Sent>,
    pending_ack: Vec<(usize, u8, bool)>,
}

impl AcarsGenerator {
    /// A generator for the scene.
    pub fn new(p: GenParams) -> Self {
        let mut rng = Rng::new(p.seed);
        let chans = p.channels.clone();
        let eff_rate = p.rate_hz;
        // The carriers of every channel together stay within full scale at the crests of their modulation.
        let nominal = 0.85 / (chans.len().max(1) as f64 * (1.0 + p.depth));
        let mut planes = Vec::new();
        let mut reqs = BinaryHeap::new();
        let mut seq = 0;
        for i in 0..p.aircraft.clamp(1, 24) {
            let reg = if i == 0 {
                p.fixed_reg.clone().unwrap_or(REGS[i].into())
            } else {
                REGS[i].into()
            };
            let flight = format!(
                "{}{:04}",
                AIRLINES[i % AIRLINES.len()],
                100 + (rng.below(9000))
            );
            let level = -rng.range(0.0, 4.0);
            let offset = rng.range(-300.0, 300.0);
            let lat = 50.0 + rng.range(-2.0, 2.0);
            let lon = rng.range(-4.0, 4.0);
            let hdg = rng.range(0.0, std::f64::consts::TAU);
            planes.push(Plane {
                reg,
                flight,
                chan: i % chans.len().max(1),
                level,
                offset,
                cycle: rng.below(LABELS.len() as u64) as usize,
                msg_no: 1,
                block_id: b'0' + (i as u8 % 10),
                last_dl_id: b'0',
                pos: (lat, lon, 35000.0 - 1000.0 * (i % 5) as f64),
                // 450 kt, ten times faster than real: degrees per second.
                vel: (
                    450.0 / 3600.0 / 60.0 * 10.0 * hdg.cos(),
                    450.0 / 3600.0 / 60.0 * 10.0 * hdg.sin() / lat.to_radians().cos(),
                ),
                pos_kind: i % 5,
                pos_count: 0,
            });
            let drawn = rng.range(0.2, 2.0 + 13.0 / p.rate_factor.max(0.1));
            let t0 = p.first_s.filter(|_| i == 0).unwrap_or(drawn);
            reqs.push(Req(t0, seq, i, 0));
            seq += 1;
            if p.positions && i < 6 {
                reqs.push(Req(rng.range(0.5, 3.0), seq, i, 2));
                seq += 1;
            }
        }
        let nch = chans.len();
        AcarsGenerator {
            eff_rate,
            rng,
            n: 0,
            planes,
            free: vec![-1.0; nch],
            chans,
            reqs,
            seq,
            planned_to: 0.0,
            bursts: Vec::new(),
            uplink_id: b'A',
            nominal,
            sent: Vec::new(),
            pending_ack: Vec::new(),
            p,
        }
    }

    /// A generator from the stack's `gen_*` settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        AcarsGenerator::new(GenParams {
            rate_hz: cfg.rate_hz,
            seed: cfg.seed,
            aircraft: usize::try_from(s.get(&super::GEN_AIRCRAFT)).unwrap_or(1),
            rate_factor: s.get(&super::GEN_RATE_FACTOR),
            positions: s.get(&super::GEN_POSITIONS),
            // A short run of the test signal hears a message.
            first_s: Some(1.0),
            ..Default::default()
        })
    }

    fn text_for(&mut self, label: &str, pi: usize) -> String {
        let pl = &self.planes[pi];
        let ap = AIRPORTS[pi % AIRPORTS.len()];
        let dest = AIRPORTS[(pi + 3) % AIRPORTS.len()];
        let hh = 6 + self.rng.below(16);
        let mm = self.rng.below(60);
        match label {
            "QA" | "QB" | "QC" | "QD" => format!("{ap}{hh:02}{mm:02}"),
            "10" => format!("ARR {dest} {hh:02}{mm:02} FUEL {:04}", self.rng.below(9000)),
            "5Z" => format!("/RQ GATE ASSIGNMENT {dest} FLT {}", pl.flight),
            "H1" => format!("#DFB REQUEST WX {dest}\r\nTHANK YOU"),
            "80" => format!("PLEASE ADVISE ARRIVAL GATE\r\nFOR {} AT {dest}", pl.flight),
            "SA" => format!("0EV{hh:02}{mm:02}{:02}SH", self.rng.below(60)),
            "Q1" => format!(
                "{ap}{hh:02}{mm:02}{hh:02}{:02}{:02}{:02}{:02}{:02}0000{dest}",
                (mm + 5) % 60,
                (hh + 2) % 24,
                mm,
                (hh + 2) % 24,
                (mm + 7) % 60
            ),
            _ => String::new(),
        }
    }

    fn position_text(&mut self, pi: usize, t: f64) -> (String, String) {
        let pl = &mut self.planes[pi];
        let lat = pl.pos.0 + pl.vel.0 * t;
        let lon = pl.pos.1 + pl.vel.1 * t;
        let alt = pl.pos.2;
        let trk = pl.vel.1.atan2(pl.vel.0).to_degrees().rem_euclid(360.0);
        let sec_day = 12.0 * 3600.0 + 50.0 * 60.0 + 10.0 * t;
        let sec_hour = sec_day % 3600.0;
        pl.pos_count += 1;
        let kind = pl.pos_kind;
        pl.pos_kind = (pl.pos_kind + 1) % 5;
        let two = pl.pos_count.is_multiple_of(2);
        let report = AdscReport {
            lat_deg: lat,
            lon_deg: lon,
            alt_ft: alt,
            sec_past_hour: sec_hour,
            track: Some((trk, 450.0)),
            route: two.then_some((
                (lat + 0.5, lon + 0.5, alt),
                600,
                (lat + 1.0, lon + 1.0, alt),
            )),
            meteo: two.then_some((40.0, 270.0, -55.0)),
        };
        let env = adsc_envelope("TESTGND", &pl.reg, &pl.flight, &report).unwrap_or_default();
        let ns = |v: f64, p: char, n: char| if v >= 0.0 { p } else { n };
        let dm = |v: f64, w: usize| {
            let a = v.abs();
            let d = a.floor();
            let m = ((a - d) * 600.0).round().min(599.0);
            format!("{:0w$}{:03}", d as u32, m as u32, w = w)
        };
        let hms = {
            let s = sec_day as u64;
            format!("{:02}{:02}{:02}", s / 3600 % 24, s / 60 % 60, s % 60)
        };
        match kind {
            0 => ("B6".into(), format!("/{env}")),
            1 => ("B6".into(), env),
            2 => ("H1".into(), format!("#M1B/B6 /{env}")),
            3 => (
                "H1".into(),
                format!(
                    "#M1BPOS{}{} {}{},WPT01,{hms},{:.0},",
                    ns(lat, 'N', 'S'),
                    dm(lat, 2),
                    ns(lon, 'E', 'W'),
                    dm(lon, 3),
                    alt / 100.0
                ),
            ),
            _ => (
                "16".into(),
                format!(
                    "{} {:.3},{} {:.3},{:.0},6, 290",
                    ns(lat, 'N', 'S'),
                    lat.abs(),
                    ns(lon, 'E', 'W'),
                    lon.abs(),
                    alt
                ),
            ),
        }
    }

    fn burst(
        &mut self,
        start: f64,
        chan: usize,
        level_db: f64,
        offset: f64,
        spec: &BlockSpec,
    ) -> f64 {
        let mut bytes: Vec<u8> = Vec::with_capacity(280);
        if spec.frame(16, true, &mut bytes).is_err() {
            return start;
        }
        let mut bits = Vec::with_capacity(bytes.len() * 8);
        for b in &bytes {
            for i in 0..8 {
                bits.push((b >> i) & 1 != 0);
            }
        }
        let dur = bits.len() as f64 / 2400.0;
        let mod_start = start + 0.006;
        let mod_end = mod_start + dur;
        let end = mod_end + 0.006;
        // The end of the check bytes: before the DEL.
        let bcs_end = mod_start + (bits.len() - 8) as f64 / 2400.0;
        let mut atten = level_db;
        if chan + 1 == self.chans.len() && self.chans.len() > 1 {
            atten -= self.p.last_atten_db;
        }
        let amp = self.nominal * 10f64.powf(atten / 20.0);
        let freq = self.chans[chan] - self.p.centre_hz + offset;
        let phase = self.rng.range(0.0, std::f64::consts::TAU);
        let aphase = self.rng.range(0.0, std::f64::consts::TAU);
        self.bursts.push(Burst {
            start,
            mod_start,
            mod_end,
            end,
            bits,
            amp,
            freq,
            phase,
            aphase,
            started: false,
        });
        let mut body = Vec::new();
        if spec.body(&mut body).is_ok() {
            strip_parity(&mut body);
            if let Ok(b) = acars::Block::parse(&body) {
                self.sent.push(Sent {
                    start_s: start,
                    end_s: bcs_end,
                    freq_hz: self.chans[chan],
                    msg: acars::Message::from_block(&b, Quality::GOOD),
                });
            }
        }
        end
    }

    fn schedule(&mut self, chan: usize, at: f64) -> f64 {
        at.max(self.free[chan] + 0.15)
    }

    fn plan(&mut self, until: f64) {
        loop {
            let Some(r) = self.reqs.peek_mut() else {
                break;
            };
            if r.0 > until {
                break;
            }
            let Req(t, _, pi, kind) = std::collections::binary_heap::PeekMut::pop(r);
            match kind {
                0 | 2 => {
                    let chan = self.planes[pi].chan;
                    let (label, text, two) = if kind == 2 {
                        let (l, x) = self.position_text(pi, t);
                        (l, x, false)
                    } else if let Some(f) = self.p.fixed_text.clone() {
                        ("H1".to_string(), f, false)
                    } else {
                        let pl = &mut self.planes[pi];
                        let mut label = LABELS[pl.cycle % LABELS.len()].to_string();
                        pl.cycle += 1;
                        if self.rng.uniform() < 0.25 {
                            label = ["Q0", "Q1", "5Z", "80", "H1", "SA"]
                                [self.rng.below(6) as usize]
                                .to_string();
                        }
                        let two = label == "5Z" && self.rng.below(6) == 0;
                        let text = self.text_for(&label, pi);
                        (label, text, two)
                    };
                    let start = self.schedule(chan, t);
                    let pl = &mut self.planes[pi];
                    let msg = format!("M{:02}", pl.msg_no % 100);
                    pl.msg_no += 1;
                    let (reg, flight, level, offset) =
                        (pl.reg.clone(), pl.flight.clone(), pl.level, pl.offset);
                    let mut texts: Vec<(char, String, bool)> = Vec::new();
                    if two {
                        let long: String =
                            (0..200).map(|k| (b'A' + (k % 26) as u8) as char).collect();
                        let tail: String =
                            (0..60).map(|k| (b'0' + (k % 10) as u8) as char).collect();
                        texts.push(('A', long, false));
                        texts.push(('B', tail, true));
                    } else {
                        texts.push(('A', text, true));
                    }
                    let mut s = start;
                    let mut last_id = b'0';
                    for (seq, body, fin) in texts {
                        let pl = &mut self.planes[pi];
                        let id = pl.block_id;
                        pl.block_id = if pl.block_id >= b'9' {
                            b'0'
                        } else {
                            pl.block_id + 1
                        };
                        last_id = id;
                        let txt = format!("{msg}{seq}{flight:<6}{body}");
                        let end = if fin { End::Etx } else { End::Etb };
                        let mut spec = BlockSpec::new('2', &reg, id, end).label(&label);
                        if !(label == "Q0" && body.is_empty()) {
                            spec = spec.text(&txt);
                        }
                        let e = self.burst(s, chan, level, offset, &spec);
                        self.free[chan] = e;
                        s = self.schedule(chan, e);
                    }
                    self.planes[pi].last_dl_id = last_id;
                    if self.p.uplinks {
                        let rt = self.free[chan] + self.rng.range(0.3, 1.2);
                        self.pending_ack.push((pi, last_id, label.starts_with('Q')));
                        self.reqs.push(Req(rt, self.seq, pi, 1));
                        self.seq += 1;
                    }
                    let next = if kind == 2 {
                        t + 7.0 / self.p.rate_factor
                    } else {
                        t + 15.0 / self.p.rate_factor * self.rng.exponential()
                    };
                    self.reqs.push(Req(next, self.seq, pi, kind));
                    self.seq += 1;
                }
                _ => {
                    let Some(k) = self.pending_ack.iter().position(|a| a.0 == pi) else {
                        continue;
                    };
                    let (_, dl_id, qlabel) = self.pending_ack.remove(k);
                    let chan = self.planes[pi].chan;
                    let reg = self.planes[pi].reg.clone();
                    let id = self.uplink_id;
                    self.uplink_id = if self.uplink_id >= b'Z' {
                        b'A'
                    } else {
                        self.uplink_id + 1
                    };
                    let (label, text, ack) = if qlabel || self.rng.uniform() < 0.55 {
                        ("_d".to_string(), String::new(), dl_id)
                    } else {
                        match self.rng.below(3) {
                            0 => (
                                "H1".into(),
                                "- #M1/WX EGLL 271250Z 24012KT 9999 FEW030 15/09 Q1012".to_string(),
                                NAK,
                            ),
                            1 => ("5Z".into(), "/GATE B12 STAND 5 CONFIRMED".to_string(), NAK),
                            _ => (
                                "80".into(),
                                "CLEARED TO EGLL VIA UMLAT\r\nSQUAWK 4521".to_string(),
                                NAK,
                            ),
                        }
                    };
                    let start = self.schedule(chan, t);
                    let mut spec = BlockSpec::new('2', &reg, id, End::Etx)
                        .label(&label)
                        .tak(ack);
                    if !text.is_empty() {
                        spec = spec.text(&text);
                    }
                    let e = self.burst(start, chan, -1.5, 0.0, &spec);
                    self.free[chan] = e;
                }
            }
        }
    }

    /// The mean power of one carrier at the nominal level while it sends. Every channel's carrier is at this level, so the
    /// SNR of a channel is stated against it.
    pub fn carrier_power(&self) -> f64 {
        self.nominal * self.nominal * (1.0 + self.p.depth * self.p.depth / 2.0)
    }

    /// Every block sent so far.
    pub fn answer_key(&self) -> &[Sent] {
        &self.sent
    }
}

impl Generator for AcarsGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate_hz
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        let mut s = SignalInfo::new(7e3)
            .with_symbol_rate(2400.0)
            .with_bit_rate(2400.0);
        s.power = self.carrier_power();
        Some(s)
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let er = self.eff_rate;
        let t0 = self.n as f64 / er;
        let t1 = (self.n + out.len() as u64) as f64 / er;
        if t1 + 2.0 > self.planned_to {
            self.planned_to = t1 + 5.0;
            let until = self.planned_to;
            self.plan(until);
        }
        out.fill(Complex32::default());
        let depth = self.p.depth;
        let tau = std::f64::consts::TAU;
        for b in self.bursts.iter_mut() {
            if b.end < t0 || b.start >= t1 {
                continue;
            }
            let first = ((b.start * er).ceil() as u64).max(self.n);
            let last = ((b.end * er).floor() as u64).min(self.n + out.len() as u64 - 1);
            if !b.started {
                b.started = true;
            }
            for k in first..=last {
                let t = k as f64 / er;
                // Carrier ramp (3 ms raised cosine at both ends).
                let ca = if t < b.start + 0.003 {
                    0.5 - 0.5 * (std::f64::consts::PI * (t - b.start) / 0.003).cos()
                } else if t > b.end - 0.003 {
                    0.5 - 0.5 * (std::f64::consts::PI * (b.end - t) / 0.003).cos()
                } else {
                    1.0
                };
                let mut audio = 0.0;
                if t >= b.mod_start && t < b.mod_end {
                    let bi = ((t - b.mod_start) * 2400.0) as usize;
                    let bit = b.bits[bi.min(b.bits.len() - 1)];
                    let prev = if bi == 0 { true } else { b.bits[bi - 1] };
                    let f = if bit == prev { 2400.0 } else { 1200.0 };
                    let ramp = ((t - b.mod_start) / 0.0005)
                        .min((b.mod_end - t) / 0.0005)
                        .min(1.0);
                    audio = b.aphase.sin() * ramp;
                    b.aphase += tau * f / er;
                    if b.aphase > tau {
                        b.aphase -= tau;
                    }
                } else if t < b.mod_start {
                    // The audio phase runs from the start of the modulation.
                }
                let env = b.amp * ca * (1.0 + depth * audio);
                let ph = b.phase + tau * b.freq * t;
                let (sn, cs) = ph.sin_cos();
                let i = (k - self.n) as usize;
                out[i] += Complex32::new((env * cs) as f32, (env * sn) as f32);
            }
        }
        self.n += out.len() as u64;
        let tn = self.n as f64 / er;
        self.bursts.retain(|b| b.end >= tn);
        out.len()
    }
}
