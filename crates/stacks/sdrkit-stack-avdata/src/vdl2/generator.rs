//! The VDL2 test signal: ground stations sending GSIFs, aircraft logging on and sending ACARS, ADS-C, X.25 and CPDLC frames,
//! the ground answering. D8PSK bursts with raised-cosine pulses, with an answer key. The signal is clean: the engine adds the
//! noise.
//! Standard: ICAO Doc 9776 clause 6.

use super::avlc::{
    self, acars_info, header, i_control, location, s_control, x25_call, x25_data_clnp, xid_control,
    xid_info,
};
use super::phy::{self, Codec, SYMBOL_RATE, TRAINING};
use crate::messages::{adsc_envelope, AdscReport};
use sdrkit_algo_dsp::pulse::rc;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use sdrkit_proto_aero::acars::{BlockSpec, End};
use sdrkit_proto_link::hdlc::{Line, Transmitter};
use std::f64::consts::{FRAC_PI_4, TAU};

/// The amplitude of a burst at 0 dB: the bursts of every channel together stay within full scale.
pub const AMPLITUDE: f64 = 0.08;

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
    /// Channels, Hz, on the same scale as `centre_hz`.
    pub channels: Vec<f64>,
    /// Traffic factor.
    pub rate_factor: f64,
    /// Extra attenuation of the last channel, dB.
    pub last_atten_db: f64,
    /// Spread of the aircraft levels below the ground stations, dB.
    pub level_spread_db: f64,
    /// Transmitter offsets up to this, Hz.
    pub tx_offset_hz: f64,
    /// A fixed ACARS text.
    pub fixed_text: Option<String>,
    /// A fixed registration of the first aircraft.
    pub fixed_reg: Option<String>,
    /// Stop planning after this time (s).
    pub until_s: f64,
}

impl Default for GenParams {
    fn default() -> Self {
        GenParams {
            rate_hz: 1e6,
            centre_hz: 0.0,
            aircraft: 6,
            seed: 1,
            // One channel at the station.
            channels: vec![0.0],
            rate_factor: 1.0,
            last_atten_db: 0.0,
            level_spread_db: 10.0,
            tx_offset_hz: 150.0,
            fixed_text: None,
            fixed_reg: None,
            until_s: f64::INFINITY,
        }
    }
}

/// A frame sent.
#[derive(Clone, Debug)]
pub struct SentFrame {
    /// Burst start (the first training symbol), s.
    pub time_s: f64,
    /// Channel.
    pub freq_hz: f64,
    /// Source.
    pub src: u32,
    /// Destination.
    pub dst: u32,
    /// The frame octets (FCS removed).
    pub raw: Vec<u8>,
}

struct Tx {
    start: f64,
    end: f64,
    symbols: Vec<Complex32>,
    freq: f64,
    amp: f64,
}

struct Plane {
    addr: u32,
    reg: String,
    flight: String,
    chan: usize,
    level: f64,
    offset: f64,
    step: usize,
    ns: u8,
    lat: f64,
    lon: f64,
    alt: f64,
    vel: (f64, f64),
}

/// The mean power of a burst at 0 dB while it is sent: the D8PSK symbols at [`AMPLITUDE`] through the raised-cosine pulse.
pub fn burst_power() -> f64 {
    AMPLITUDE * AMPLITUDE * 0.85
}

/// The VDL2 generator.
pub struct Vdl2Generator {
    p: GenParams,
    rng: Rng,
    codec: Codec,
    tx: Transmitter,
    er: f64,
    n: u64,
    planes: Vec<Plane>,
    gs_offset: Vec<f64>,
    free: Vec<f64>,
    next_gs: Vec<f64>,
    next_ac: Vec<f64>,
    planned: f64,
    txs: Vec<Tx>,
    pulse: Vec<f64>,
    /// Every frame sent.
    pub sent: Vec<SentFrame>,
}

/// Ground station address of channel `i`.
pub fn gs_addr(i: usize) -> u32 {
    0x10A000 + 0x123 * i as u32
}

const REGS: [&str; 8] = [
    "G-EUUA", "D-AIBC", "F-GKXA", "EI-DVM", "PH-BXA", "OO-SNA", "HB-JZA", "SE-RJE",
];

impl Vdl2Generator {
    /// A generator for the scene.
    pub fn new(p: GenParams) -> Self {
        let mut rng = Rng::new(p.seed);
        let nch = p.channels.len();
        let mut planes = Vec::new();
        let mut addr = 0x4CA2B1u32;
        for i in 0..p.aircraft.clamp(1, 24) {
            let reg = if i == 0 {
                p.fixed_reg.clone().unwrap_or(REGS[0].into())
            } else {
                format!(
                    "{}{}",
                    REGS[i % 8],
                    if i >= 8 {
                        (b'A' + i as u8) as char
                    } else {
                        ' '
                    }
                )
                .trim()
                .to_string()
            };
            let hdg = rng.range(0.0, TAU);
            planes.push(Plane {
                addr,
                reg,
                flight: format!("BA{:04}", 100 + i * 17),
                chan: if i == 0 {
                    nch - 1
                } else {
                    rng.below(nch as u64) as usize
                },
                level: -rng.range(0.0, p.level_spread_db),
                offset: rng.range(-p.tx_offset_hz, p.tx_offset_hz),
                step: 0,
                ns: 0,
                lat: 51.5 + rng.range(-1.0, 1.0),
                lon: rng.range(-1.0, 1.0),
                alt: 30000.0 + 1000.0 * (i % 8) as f64,
                vel: (
                    450.0 * 20.0 / 3600.0 / 60.0 * hdg.cos(),
                    450.0 * 20.0 / 3600.0 / 60.0 * hdg.sin() / 51.5f64.to_radians().cos(),
                ),
            });
            addr = addr.wrapping_mul(1_103_515_245).wrapping_add(12_345) & 0xFF_FFFF | 0x400000;
        }
        let gs_offset = (0..nch)
            .map(|_| rng.range(-p.tx_offset_hz, p.tx_offset_hz))
            .collect();
        let next_gs = (0..nch).map(|_| rng.range(0.05, 1.0)).collect();
        let next_ac = (0..planes.len())
            .map(|_| rng.range(0.2, 6.0 / p.rate_factor.max(0.1)))
            .collect();
        // The pulse: raised cosine 0.6, truncated at +-5 symbols with a Hann taper, at 64 points per symbol.
        let pulse: Vec<f64> = (0..=640)
            .map(|i| {
                let t = (i as f64 - 320.0) / 64.0;
                rc(t, 0.6) * (0.5 + 0.5 * (std::f64::consts::PI * t / 5.0).cos())
            })
            .collect();
        Vdl2Generator {
            er: p.rate_hz,
            codec: Codec::default(),
            tx: Transmitter::new(Line::Nrz),
            rng,
            n: 0,
            planes,
            gs_offset,
            free: vec![-1.0; nch],
            next_gs,
            next_ac,
            planned: 0.0,
            txs: Vec::new(),
            pulse,
            sent: Vec::new(),
            p,
        }
    }

    /// A generator from the stack's `gen_*` settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        Vdl2Generator::new(GenParams {
            rate_hz: cfg.rate_hz,
            seed: cfg.seed,
            aircraft: usize::try_from(cfg.settings.get(&super::GEN_AIRCRAFT)).unwrap_or(1),
            ..Default::default()
        })
    }

    /// Queues a burst of `frames` on channel `chan` no earlier than `t`. Returns its end.
    pub fn send(
        &mut self,
        t: f64,
        chan: usize,
        level_db: f64,
        offset: f64,
        frames: &[Vec<u8>],
    ) -> f64 {
        let refs: Vec<&[u8]> = frames.iter().map(|f| f.as_slice()).collect();
        let mut bits = Vec::new();
        self.tx.encode(&refs, 1, &mut bits);
        let mut bb = Vec::new();
        phy::burst_bits(&mut self.codec, &bits, &mut bb);
        let mut syms: Vec<Complex32> = Vec::new();
        let ph = |k: i32, a: f64| {
            let x = k as f64 * FRAC_PI_4;
            Complex32::new((a * x.cos()) as f32, (a * x.sin()) as f32)
        };
        for a in [0.2, 0.6, 0.9] {
            syms.push(ph(0, a));
        }
        for &tp in TRAINING.iter() {
            syms.push(ph(tp, 1.0));
        }
        let mut cur = TRAINING[TRAINING.len() - 1];
        for c in bb.chunks(3) {
            let b = c[0] << 2 | c[1] << 1 | c[2];
            cur += phy::step_of(b) as i32;
            syms.push(ph(cur, 1.0));
        }
        for a in [0.6, 0.25] {
            syms.push(ph(cur, a));
        }
        let start = t.max(self.free[chan] + 0.004);
        let dur = syms.len() as f64 / SYMBOL_RATE;
        let end = start + dur;
        self.free[chan] = end;
        let mut atten = level_db;
        if chan + 1 == self.p.channels.len() && self.p.channels.len() > 1 {
            atten -= self.p.last_atten_db;
        }
        // Rotate by a random carrier phase.
        let rot = self.rng.range(0.0, TAU);
        let r = Complex32::new(rot.cos() as f32, rot.sin() as f32);
        for s in syms.iter_mut() {
            *s *= r;
        }
        let freq = self.p.channels[chan] - self.p.centre_hz + offset;
        let t_train = start + 3.0 / SYMBOL_RATE;
        for f in frames {
            let (dst, _, _) = avlc::read_addr(&f[0..4]);
            let (src, _, _) = avlc::read_addr(&f[4..8]);
            self.sent.push(SentFrame {
                time_s: t_train,
                freq_hz: self.p.channels[chan],
                src,
                dst,
                raw: f.clone(),
            });
        }
        self.txs.push(Tx {
            start,
            end: end + 5.0 / SYMBOL_RATE,
            symbols: syms,
            freq,
            amp: AMPLITUDE * 10f64.powf(atten / 20.0),
        });
        end
    }

    fn acars(
        &mut self,
        reg: &str,
        label: &str,
        id: u8,
        text: &str,
        downlink_prefix: Option<(&str, &str)>,
    ) -> (Vec<u8>, [u8; 2]) {
        let t = match downlink_prefix {
            Some((m, f)) => format!("{m}A{f:<6}{text}"),
            None => text.to_string(),
        };
        let mut spec = BlockSpec::new('2', reg, id, End::Etx).label(label);
        if !t.is_empty() {
            spec = spec.text(&t);
        }
        let mut body = Vec::new();
        let bcs = spec.body(&mut body).unwrap_or([0; 2]);
        (body, bcs)
    }

    fn plan(&mut self, until: f64) {
        let until = until.min(self.p.until_s);
        loop {
            // The earliest event: a ground station's GSIF or an aircraft's next burst.
            let g = self
                .next_gs
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.total_cmp(b.1))
                .map(|(i, &t)| (t, i));
            let a = self
                .next_ac
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.total_cmp(b.1))
                .map(|(i, &t)| (t, i));
            let (t, is_gs, i) = match (g, a) {
                (Some(g), Some(a)) if g.0 <= a.0 => (g.0, true, g.1),
                (_, Some(a)) => (a.0, false, a.1),
                (Some(g), None) => (g.0, true, g.1),
                _ => return,
            };
            if t > until {
                return;
            }
            if is_gs {
                let gs = gs_addr(i);
                let mut f = header(0xFFFFFF, 7, false, gs, 4, true, xid_control(false));
                f.extend(xid_info(&[
                    (0x01, vec![3]),
                    (0xC1, b"EGLLEGKK".to_vec()),
                    (0xC8, location(51.4 + 0.1 * i as f64, -0.5, None)),
                ]));
                let off = self.gs_offset[i];
                self.send(t, i, 0.0, off, &[f]);
                self.next_gs[i] = t + self.rng.range(3.0, 6.0);
                continue;
            }
            let pl = &self.planes[i];
            let (addr, chan, level, off, reg, flight) = (
                pl.addr,
                pl.chan,
                pl.level,
                pl.offset,
                pl.reg.clone(),
                pl.flight.clone(),
            );
            let gs = gs_addr(chan);
            let lat = pl.lat + pl.vel.0 * t;
            let lon = pl.lon + pl.vel.1 * t;
            let alt = pl.alt;
            let step = pl.step;
            let ns = pl.ns;
            self.planes[i].step += 1;
            let ctl = i_control(ns, 0, false);
            let mut frames: Vec<Vec<u8>> = Vec::new();
            let mut reply = false;
            match step % 8 {
                0 => {
                    let mut f = header(gs, 4, false, addr, 1, false, xid_control(true));
                    f.extend(xid_info(&[
                        (0x01, vec![0]),
                        (0x84, location(lat, lon, Some(alt))),
                        (0x83, b"EGLL".to_vec()),
                    ]));
                    frames.push(f);
                }
                1 => {
                    let text = self
                        .p
                        .fixed_text
                        .clone()
                        .unwrap_or("#DFB REQUEST WX EGLL".into());
                    let (b, c) = self.acars(&reg, "H1", b'1', &text, Some(("M01", &flight)));
                    let mut f = header(gs, 4, false, addr, 1, false, ctl);
                    f.extend(acars_info(&b, c));
                    frames.push(f);
                    reply = true;
                }
                2 => {
                    let trk = self.planes[i]
                        .vel
                        .1
                        .atan2(self.planes[i].vel.0)
                        .to_degrees()
                        .rem_euclid(360.0);
                    let r = AdscReport {
                        lat_deg: lat,
                        lon_deg: lon,
                        alt_ft: alt,
                        sec_past_hour: (t * 20.0) % 3600.0,
                        track: Some((trk, 450.0)),
                        route: None,
                        meteo: None,
                    };
                    let env = adsc_envelope("TESTGND", &reg, &flight, &r).unwrap_or_default();
                    let text = format!("/{env}");
                    let (b, c) = self.acars(&reg, "B6", b'2', &text, Some(("M02", &flight)));
                    let mut f = header(gs, 4, false, addr, 1, false, ctl);
                    f.extend(acars_info(&b, c));
                    frames.push(f);
                }
                3 => {
                    let text = format!(
                        "{} {:.3},{} {:.3},{:.0},6, 290",
                        if lat >= 0.0 { 'N' } else { 'S' },
                        lat.abs(),
                        if lon >= 0.0 { 'E' } else { 'W' },
                        lon.abs(),
                        alt
                    );
                    let (b, c) = self.acars(&reg, "16", b'3', &text, Some(("M03", &flight)));
                    let mut f = header(gs, 4, false, addr, 1, false, ctl);
                    f.extend(acars_info(&b, c));
                    frames.push(f);
                }
                4 => {
                    let mut f = header(gs, 4, false, addr, 1, false, ctl);
                    f.extend(x25_call(1, "1234", "56789", &[0xC1, 0x00]));
                    frames.push(f);
                }
                5 => {
                    let app: Vec<u8> = (0..20).map(|_| self.rng.below(256) as u8).collect();
                    let mut f = header(gs, 4, false, addr, 1, false, ctl);
                    f.extend(x25_data_clnp(1, ns, 0, &app));
                    frames.push(f);
                }
                6 => {
                    let h: String = (0..24)
                        .map(|_| {
                            char::from_digit(self.rng.below(16) as u32, 16)
                                .expect("a value below 16")
                                .to_ascii_uppercase()
                        })
                        .collect();
                    let text = format!("/TESTGND.AT1.{reg:.>7}{h}");
                    let (b, c) = self.acars(&reg, "AA", b'4', &text, Some(("M04", &flight)));
                    let mut f = header(gs, 4, false, addr, 1, false, ctl);
                    f.extend(acars_info(&b, c));
                    frames.push(f);
                }
                _ => {
                    let mut f = header(gs, 4, false, addr, 1, false, s_control(0, ns, false));
                    f.truncate(9);
                    frames.push(f);
                    let (b, c) = self.acars(&reg, "_d", b'5', "", None);
                    let mut g = header(gs, 4, false, addr, 1, false, i_control(ns, 0, false));
                    g.extend(acars_info(&b, c));
                    frames.push(g);
                }
            }
            if !step.is_multiple_of(8) {
                self.planes[i].ns = (ns + 1) % 8;
            }
            let end = self.send(t, chan, level, off, &frames);
            if reply {
                // The ground answers: RR and an ACARS uplink in one burst.
                let rr = header(
                    addr,
                    1,
                    false,
                    gs,
                    4,
                    true,
                    s_control(0, (ns + 1) % 8, false),
                );
                let (b, c) = self.acars(&reg, "H1", b'A', "- #M1/TEST UPLINK OK", None);
                let mut up = header(
                    addr,
                    1,
                    false,
                    gs,
                    4,
                    false,
                    i_control(0, (ns + 1) % 8, false),
                );
                up.extend(acars_info(&b, c));
                let gso = self.gs_offset[chan];
                let at = end + self.rng.range(0.05, 0.3);
                self.send(at, chan, 0.0, gso, &[rr, up]);
            }
            self.next_ac[i] = t + 6.0 / self.p.rate_factor.max(0.01) * self.rng.exponential();
        }
    }

    /// The frames sent so far.
    pub fn answer_key(&self) -> &[SentFrame] {
        &self.sent
    }
}

impl Generator for Vdl2Generator {
    fn rate_hz(&self) -> f64 {
        self.p.rate_hz
    }

    fn signal_info(&self) -> Option<SignalInfo> {
        let mut s = SignalInfo::new(SYMBOL_RATE)
            .with_symbol_rate(SYMBOL_RATE)
            .with_bit_rate(3.0 * SYMBOL_RATE);
        s.power = burst_power();
        Some(s)
    }

    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let er = self.er;
        let t1 = (self.n + out.len() as u64) as f64 / er;
        if t1 + 1.0 > self.planned {
            self.planned = t1 + 3.0;
            let u = self.planned;
            self.plan(u);
        }
        out.fill(Complex32::default());
        let t0 = self.n as f64 / er;
        for tx in &self.txs {
            if tx.end < t0 || tx.start > t1 {
                continue;
            }
            let first = ((tx.start * er).ceil() as u64).max(self.n);
            let last = ((tx.end * er).floor() as u64).min(self.n + out.len() as u64 - 1);
            for k in first..=last {
                let t = k as f64 / er;
                let u = (t - tx.start) * SYMBOL_RATE;
                let c = u.round() as i64;
                let mut acc = Complex32::default();
                for s in (c - 5).max(0)..=(c + 5).min(tx.symbols.len() as i64 - 1) {
                    let d = u - s as f64;
                    if d.abs() >= 5.0 {
                        continue;
                    }
                    let pi = d * 64.0 + 320.0;
                    let i0 = pi.floor() as usize;
                    let fr = pi - i0 as f64;
                    let w = self.pulse[i0] * (1.0 - fr) + self.pulse[(i0 + 1).min(640)] * fr;
                    acc += tx.symbols[s as usize] * w as f32;
                }
                let ph = TAU * tx.freq * t;
                let rot = Complex32::new(ph.cos() as f32, ph.sin() as f32);
                out[(k - self.n) as usize] += acc * rot * tx.amp as f32;
            }
        }
        self.n += out.len() as u64;
        let tn = self.n as f64 / er;
        self.txs.retain(|t| t.end >= tn);
        out.len()
    }
}
