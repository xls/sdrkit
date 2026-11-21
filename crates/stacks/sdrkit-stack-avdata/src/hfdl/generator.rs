//! The HFDL test signal: bursts with root raised cosine pulses, the carrier 1440 Hz above the dial. A scripted list of bursts,
//! or the traffic scene (squitters every 32 s frame, logons, performance data, ACARS both ways, every rate). The signal is
//! clean: the engine adds the noise and the HF channel.
//! Standard: ARINC 635-4 clause 4.

use super::pdu;
use super::phy::{self, rate, CARRIER_HZ, PREKEY, SYMBOL_RATE, T};
use sdrkit_algo_dsp::interp::{CatmullRom, Reader};
use sdrkit_algo_dsp::pulse::{PulseTable, Shape};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_proto_aero::acars::{BlockSpec, End};
use std::f64::consts::TAU;

/// The LPDU of an ACARS block from `registration` with this label, block id and text.
fn acars_lpdu(registration: &str, label: &str, id: u8, text: &str) -> Vec<u8> {
    let mut b = Vec::new();
    let c = BlockSpec::new('2', registration, id, End::Etx)
        .label(label)
        .text(text)
        .body(&mut b)
        .unwrap_or([0; 2]);
    pdu::acars_lpdu(&b, c)
}

/// A burst to send.
#[derive(Clone, Debug)]
pub struct Tx {
    /// Start of A1, s.
    pub time_s: f64,
    /// Dial, Hz.
    pub dial_hz: f64,
    /// M1 index.
    pub rate: usize,
    /// The PDU.
    pub pdu: Vec<u8>,
    /// Level, dB.
    pub level_db: f64,
}

/// The scene.
#[derive(Clone, Debug)]
pub struct GenParams {
    /// Output rate.
    pub rate_hz: f64,
    /// Centre.
    pub centre_hz: f64,
    /// The bursts (empty: the traffic scene is made).
    pub bursts: Vec<Tx>,
    /// Traffic: aircraft.
    pub aircraft: usize,
    /// Traffic: stations (id, dial Hz).
    pub stations: Vec<(u8, f64)>,
    /// Traffic factor.
    pub rate_factor: f64,
    /// Traffic: length, s.
    pub duration_s: f64,
    /// Seed.
    pub seed: u64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            rate_hz: 48e3,
            centre_hz: 8.9e6,
            bursts: vec![],
            aircraft: 6,
            stations: vec![(1, 8.927e6), (7, 8.942e6), (3, 8.977e6)],
            rate_factor: 1.0,
            duration_s: 100.0,
            seed: 1,
        }
    }
}

/// The burst pair of the acceptance tests: a squitter at 600 bit/s and a data burst at `idx`.
pub fn test_pair(t: f64, dial: f64, idx: usize) -> Vec<Tx> {
    let sq = pdu::build_spdu(1, true, 777, 50, 0x21, &[(7, true, 3)]);
    let data = if idx.is_multiple_of(4) {
        pdu::downlink(1, 23, &[acars_lpdu("N1UA", "H1", b'2', "OK")])
    } else {
        let text = "M01AUA0901#DFB TEST MESSAGE FROM HFDL LOOPBACK FL350";
        pdu::downlink(
            1,
            23,
            &[
                pdu::perf_lpdu("UA0901", 51.5, -30.25, 40000, 1, 2),
                acars_lpdu("N123UA", "H1", b'2', text),
            ],
        )
    };
    let r1 = rate(1).total_symbols() + PREKEY;
    vec![
        Tx {
            time_s: t,
            dial_hz: dial,
            rate: 1,
            pdu: sq,
            level_db: 0.0,
        },
        Tx {
            time_s: t + r1 as f64 / SYMBOL_RATE + 0.3,
            dial_hz: dial,
            rate: idx,
            pdu: data,
            level_db: 0.0,
        },
    ]
}

/// A burst being sent: its complex baseband at 8 samples per symbol, from `start`.
struct Live {
    start: f64,
    bb: Vec<Complex32>,
    freq: f64,
    amp: f64,
}

/// Baseband samples per symbol.
const BB_SPS: f64 = 8.0;

/// The amplitude of a burst at 0 dB.
pub const AMPLITUDE: f64 = 0.1;

/// The mean power of a burst at 0 dB while it is sent, in the symbol rate's bandwidth.
pub fn burst_power() -> f64 {
    AMPLITUDE * AMPLITUDE
}

/// The HFDL generator.
pub struct HfdlGenerator {
    p: GenParams,
    n: u64,
    pulse: PulseTable,
    live: Vec<Live>,
    queue: Vec<Tx>,
    /// Every burst sent.
    pub sent: Vec<Tx>,
}

impl HfdlGenerator {
    /// A generator for the scene.
    pub fn new(p: GenParams) -> Self {
        let mut rng = Rng::new(p.seed);
        let mut queue = if p.bursts.is_empty() {
            traffic(&p, &mut rng)
        } else {
            p.bursts.clone()
        };
        queue.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
        HfdlGenerator {
            n: 0,
            pulse: PulseTable::new(Shape::Rrc { beta: 0.35 }, 8.0, 64)
                .expect("the HFDL pulse is valid"),
            live: Vec::new(),
            sent: queue.clone(),
            queue,
            p,
        }
    }

    /// A generator from the stack's `gen_*` settings and the rate.
    pub fn from_config(cfg: &GenConfig) -> Self {
        HfdlGenerator::new(GenParams {
            rate_hz: cfg.rate_hz,
            seed: cfg.seed,
            ..Default::default()
        })
    }
}

/// The traffic scene.
fn traffic(p: &GenParams, rng: &mut Rng) -> Vec<Tx> {
    let mut v = Vec::new();
    // Squitters in slot 0 of every 32 s frame.
    let mut f = 0.5;
    while f < p.duration_s {
        for (k, &(id, dial)) in p.stations.iter().enumerate() {
            let others: Vec<(u8, bool, u32)> = p
                .stations
                .iter()
                .filter(|s| s.0 != id)
                .take(2)
                .map(|s| (s.0, true, 1))
                .collect();
            v.push(Tx {
                time_s: f + 0.01 * k as f64,
                dial_hz: dial,
                rate: 1,
                pdu: pdu::build_spdu(id, true, (f / 32.0) as u16, 50, 0x3F, &others),
                level_db: 0.0,
            });
        }
        f += 32.0;
    }
    let slot = 32.0 / 13.0;
    // (dial, start, end) of every burst, the squitters first.
    let mut busy: Vec<(f64, f64, f64)> = v
        .iter()
        .map(|b: &Tx| {
            (
                b.dial_hz,
                b.time_s - PREKEY as f64 / SYMBOL_RATE,
                b.time_s + rate(b.rate).total_symbols() as f64 / SYMBOL_RATE + 0.3,
            )
        })
        .collect();
    let mut t = 3.0;
    let mut k = 0usize;
    let nac = p.aircraft.max(1);
    let mut step = vec![0usize; nac];
    while t < p.duration_s - 6.0 {
        let a = k % nac;
        let (gs, dial) = p.stations[a % p.stations.len()];
        let ac_id = 20 + a as u8;
        let icao = 0x4CA2B1 + a as u32;
        let reg = format!("N{:03}UA", 100 + a);
        let idx = (k / nac) % 8;
        let (pdu_bytes, r) = match step[a] {
            0 => (pdu::downlink(gs, 0, &[pdu::logon_lpdu(0x8F, icao, 0)]), 1),
            1 => (
                pdu::uplink(gs, &[(0, vec![pdu::logon_lpdu(0x9F, icao, ac_id)])]),
                1,
            ),
            2 => {
                let lat = 50.0 + rng.range(-5.0, 5.0);
                let lon = -30.0 + rng.range(-5.0, 5.0);
                (
                    pdu::downlink(
                        gs,
                        ac_id,
                        &[pdu::perf_lpdu(
                            &format!("UA{:04}", 900 + a),
                            lat,
                            lon,
                            40_000 + k as u32 * 2,
                            gs,
                            1,
                        )],
                    ),
                    idx,
                )
            }
            3 => {
                let text = format!("M01AUA{:04}#DFB POSITION REPORT", 900 + a);
                let l = acars_lpdu(&reg, "H1", b'3', &text);
                (pdu::downlink(gs, ac_id, &[l]), idx)
            }
            _ => {
                let l = acars_lpdu(&reg, "H1", b'A', "- #M1/TEST UPLINK OK");
                (pdu::uplink(gs, &[(ac_id, vec![l])]), idx)
            }
        };
        step[a] = if step[a] >= 4 { 2 } else { step[a] + 1 };
        let rr = rate(r);
        if pdu_bytes.len() <= rr.octets() {
            // One burst at a time on a channel (TDMA): the first free time at or after `t`, with a guard.
            let dur = (rr.total_symbols() + PREKEY) as f64 / SYMBOL_RATE + 0.3;
            let mut start = t;
            while let Some(e) = busy
                .iter()
                .filter(|b| b.0 == dial && b.1 < start + dur && start < b.2)
                .map(|b| b.2)
                .reduce(f64::max)
            {
                start = e;
            }
            if start + dur < p.duration_s - 1.0 {
                busy.push((dial, start, start + dur));
                v.push(Tx {
                    time_s: start + PREKEY as f64 / SYMBOL_RATE,
                    dial_hz: dial,
                    rate: r,
                    pdu: pdu_bytes,
                    level_db: 0.0,
                });
            }
        }
        t += (rr.slots as f64 * slot + 0.6) / p.rate_factor.max(0.25) + rng.range(0.0, 0.5);
        k += 1;
    }
    v
}

impl HfdlGenerator {
    /// The baseband of a burst's symbols at 8 samples per symbol through the root raised cosine pulse.
    fn render(&self, sym: &[Complex32]) -> Vec<Complex32> {
        let n = ((sym.len() as f64 + 9.0) * BB_SPS).ceil() as usize + 4;
        let mut bb = vec![Complex32::default(); n];
        for (m, v) in bb.iter_mut().enumerate() {
            let u = m as f64 / BB_SPS;
            let c = u.round() as i64;
            for i in (c - 8).max(0)..=(c + 8).min(sym.len() as i64 - 1) {
                *v += sym[i as usize] * self.pulse.value(u - i as f64);
            }
        }
        bb
    }
}

impl Generator for HfdlGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate_hz
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        let mut s = SignalInfo::new(SYMBOL_RATE).with_symbol_rate(SYMBOL_RATE);
        s.power = burst_power();
        Some(s)
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let fs = self.p.rate_hz;
        let er = fs;
        let t1 = (self.n + out.len() as u64) as f64 / er;
        // Start the bursts due.
        while let Some(b) = self.queue.first() {
            if b.time_s - PREKEY as f64 / SYMBOL_RATE > t1 + 0.1 {
                break;
            }
            let b = self.queue.remove(0);
            let r = rate(b.rate);
            let mut sym: Vec<Complex32> = (0..PREKEY)
                .map(|i| Complex32::new(T[i % 15], 0.0))
                .collect();
            let mut pd = b.pdu.clone();
            pd.resize(r.octets(), 0);
            sym.extend(phy::burst_symbols(&pd, r));
            let start = b.time_s - PREKEY as f64 / SYMBOL_RATE;
            let bb = self.render(&sym);
            self.live.push(Live {
                start,
                bb,
                freq: b.dial_hz + CARRIER_HZ - self.p.centre_hz,
                amp: AMPLITUDE * 10f64.powf(b.level_db / 20.0),
            });
        }
        out.fill(Complex32::default());
        let t0 = self.n as f64 / er;
        let tim = 0.0;
        let bbr = SYMBOL_RATE * BB_SPS;
        for l in &self.live {
            let end = l.start + (l.bb.len() as f64 - 2.0) / bbr;
            if end < t0 || l.start > t1 {
                continue;
            }
            let first = (((l.start + tim) * er).ceil().max(0.0) as u64).max(self.n);
            let last =
                (((end + tim) * er).floor().max(0.0) as u64).min(self.n + out.len() as u64 - 1);
            if first > last {
                continue;
            }
            // The carrier by a phasor recurrence from an exact start.
            let a0 = TAU * l.freq * (first as f64 / er);
            let mut ph = Complex32::new(a0.cos() as f32, a0.sin() as f32);
            let w = TAU * l.freq / er;
            let step = Complex32::new(w.cos() as f32, w.sin() as f32);
            let amp = l.amp as f32;
            for k in first..=last {
                let u = ((k as f64 / er - tim) - l.start) * bbr;
                out[(k - self.n) as usize] += Reader::new(CatmullRom).at(&l.bb, u) * ph * amp;
                ph *= step;
            }
        }
        self.n += out.len() as u64;
        let tn = self.n as f64 / er;
        self.live
            .retain(|l| l.start + l.bb.len() as f64 / (SYMBOL_RATE * BB_SPS) + 0.1 >= tn);
        out.len()
    }
}
