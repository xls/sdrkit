//! The Aero test signal: P channels carrying system tables, log-ons and ACARS uplinks to six made-up aircraft, and their position
//! reports as downlinks (ADS-C basic reports in labels B6 and H1, an ARINC 702 text POS report). The channels sit at their places
//! from the dial (10500 bit/s at -50 kHz, 1200 bit/s at +150 kHz, 600 bit/s at +100 kHz), each with its own small carrier offset
//! and a slow drift. Noise and other impairments come from the standard `gen_*` settings.

use super::coding::{encode_frame, header, su_seal, Layout};
use super::su::build_user_data;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_fec::conv::codes::INMARSAT_K7;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_algo_sim::modulate::{Cpm, Linear};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, GenConfig, Generator, Setting, SignalInfo};
use sdrkit_proto_aero::acars::{BlockSpec, End};
use sdrkit_proto_aero::adsc::{Fom, Group, Report, ReportKind};
use sdrkit_proto_aero::arinc622::{self, Imi};
use std::collections::VecDeque;

/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The channels of the test signal, as a mask (1 = 600, 2 = 1200, 4 = 10500 bit/s).
pub const GEN_CHANNELS: Setting<field::I64> = Setting::new(
    "gen_channels",
    "Channels (1 = 600, 2 = 1200, 4 = 10500)",
    "",
    6,
)
.range(1.0, 7.0, 1.0)
.section(TEST_SIGNAL)
.advanced();
/// The test signal has no built-in offsets and drift.
pub const GEN_PLAIN: Setting<field::Bool> =
    Setting::new("gen_plain", "No built-in offsets and drift", "", false)
        .section(TEST_SIGNAL)
        .advanced();

/// The made-up aircraft: AES id, registration, flight, start position.
pub const AIRCRAFT: [(u32, &str, &str, f64, f64); 6] = [
    (0x4B1A01, "HB-JHM", "SWR123", 47.4, 8.5),
    (0x3C6444, "D-AIXA", "DLH400", 50.0, 8.6),
    (0x406A3B, "G-XLEA", "BAW15", 51.5, -0.4),
    (0x44CE6F, "OO-SNA", "BEL21", 50.9, 4.5),
    (0x4CA9E1, "EI-DEP", "EIN105", 53.4, -6.3),
    (0x896231, "A6-EEA", "UAE7", 25.2, 55.4),
];

/// The GES id of the test signal.
pub const GES: u8 = 0x90;

/// The channels: rate, offset from the dial, built-in carrier offset, built-in drift.
pub const CHANNELS: [(u32, f64, f64, f64); 3] = [
    (600, 100e3, 230.0, 0.2),
    (1200, 150e3, -310.0, -0.3),
    (10500, -50e3, 410.0, 0.15),
];

#[derive(Clone, Debug)]
pub struct GenParams {
    pub rate: f64,
    pub mask: u32,
    pub plain: bool,
    /// Where the dial lies from the output's 0 Hz.
    pub dial_hz: f64,
}

impl GenParams {
    pub fn new(rate: f64) -> Self {
        GenParams {
            rate,
            mask: 6,
            plain: false,
            dial_hz: -super::DESCRIPTOR.tuning.tune_offset_hz,
        }
    }
}

/// An ACARS block as the user data of an SU: SOH, the body, the BCS and DEL.
fn acars_block(reg: &str, label: &str, block_id: u8, text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let _ = BlockSpec::new('2', reg, block_id, End::Etx)
        .label(label)
        .text(text)
        .frame(0, false, &mut out);
    out
}

/// An ADS-C basic report in its ARINC 622 envelope.
fn adsc_text(
    ground: &str,
    reg: &str,
    lat: f64,
    lon: f64,
    alt_ft: f64,
    sec: f64,
    accuracy: u8,
) -> String {
    let g = Group::Report(Report {
        kind: ReportKind::Basic,
        lat_deg: lat,
        lon_deg: lon,
        alt_ft,
        sec_past_hour: sec,
        fom: Fom {
            accuracy,
            redundancy: true,
            tcas: true,
        },
    });
    let mut data = Vec::new();
    let _ = g.encode(&mut data);
    let mut text = String::new();
    let _ = arinc622::build(ground, Imi::Ads, reg, &data, &mut text);
    text
}

/// The SU scheduler of one channel: system tables, log-ons, P/R channel control, ACARS uplinks and position downlinks, fill-in.
pub struct SuSource {
    queue: VecDeque<[u8; 12]>,
    k: u64,
    rate: u32,
    refno: u8,
}

fn plain_su(t: u8, f: impl Fn(&mut [u8; 12])) -> [u8; 12] {
    let mut s = [0u8; 12];
    s[0] = t;
    f(&mut s);
    su_seal(&mut s);
    s
}

impl SuSource {
    pub fn new(rate: u32) -> Self {
        SuSource {
            queue: VecDeque::new(),
            k: 0,
            rate,
            refno: 0,
        }
    }

    /// The made-up aircraft `i`'s position at cycle `k` (moving 20 times faster than real).
    pub fn position(i: usize, k: u64) -> (f64, f64) {
        let (_, _, _, la, lo) = AIRCRAFT[i];
        (la + 0.01 * k as f64, lo + 0.015 * k as f64)
    }

    fn refill(&mut self) {
        let k = self.k;
        self.k += 1;
        let i = (k as usize) % AIRCRAFT.len();
        let (aes, reg, flight, _, _) = AIRCRAFT[i];
        // System table: the satellite (Alphasat at 25.5E, P channel), P/R channel control, GES channels.
        self.queue.push_back(plain_su(0x0C, |s| {
            s[2] = 0;
            s[3] = 0x30;
            s[5] = 17;
            let p: u16 = 0x8000 | (((1545.115e6 - 1510e6) / 2.5e3) as u16);
            s[6] = (p >> 8) as u8;
            s[7] = p as u8;
        }));
        self.queue.push_back(plain_su(0x40, |s| {
            s[4] = GES;
            s[7] = match self.rate {
                600 => 0,
                1200 => 1,
                _ => 6,
            } << 4;
            let p: u16 = ((1545.115e6 - 1510e6) / 2.5e3) as u16;
            s[8] = (p >> 8) as u8;
            s[9] = p as u8;
        }));
        // A log-on confirm.
        self.queue.push_back(plain_su(0x11, |s| {
            s[1] = (aes >> 16) as u8;
            s[2] = (aes >> 8) as u8;
            s[3] = aes as u8;
            s[4] = GES;
        }));
        // An ACARS uplink, then a position downlink of the same aircraft.
        let up = acars_block(
            reg,
            "H1",
            b'A' + (k % 26) as u8,
            &format!("MSG {k:04} TO {reg}"),
        );
        self.user(aes, &up);
        let (lat, lon) = Self::position(i, k);
        let prefix = format!("M{:02}A{:<6}", k % 100, flight);
        let sec = (k * 10 % 3600) as f64;
        let down = match k % 3 {
            0 => {
                let text = adsc_text("LPAFAYA", reg, lat, lon, 37_000.0, sec, 6);
                acars_block(reg, "B6", b'1', &format!("{prefix}{text}"))
            }
            1 => {
                let text = adsc_text("LHWE1YA", reg, lat, lon, 35_000.0, sec, 7);
                acars_block(reg, "H1", b'2', &format!("{prefix}#M1B/B6 {text}"))
            }
            _ => {
                let (ns, la) = if lat >= 0.0 { ('N', lat) } else { ('S', -lat) };
                let (ew, lo) = if lon >= 0.0 { ('E', lon) } else { ('W', -lon) };
                let ddmm = |v: f64, w: usize| {
                    let d = v.floor();
                    let m = (v - d) * 60.0;
                    format!("{:0w$}{:03}", d as u32, (m * 10.0).round() as u32, w = w)
                };
                let text = format!(
                    "{prefix}POS{ns}{}{ew}{},EASON,215754,370",
                    ddmm(la, 2),
                    ddmm(lo, 3)
                );
                acars_block(reg, "H1", b'3', &text)
            }
        };
        self.user(aes, &down);
    }

    fn user(&mut self, aes: u32, block: &[u8]) {
        self.refno = (self.refno + 1) % 16;
        for s in build_user_data(aes, GES, 1, self.refno, block) {
            self.queue.push_back(s);
        }
    }

    /// The next `n` SUs (fill-in when the queue is empty is never needed: it is refilled).
    pub fn take(&mut self, n: usize) -> Vec<[u8; 12]> {
        while self.queue.len() < n {
            self.refill();
        }
        self.queue.drain(..n).collect()
    }
}

/// How one channel is modulated.
enum Modulation {
    /// MSK at 600 or 1200 bit/s.
    Msk(Cpm),
    /// Offset QPSK at 10500 bit/s, the Q arm leading.
    Oqpsk(Linear),
}

struct Chan {
    layout: Layout,
    sus: SuSource,
    enc: Encoder,
    bits: VecDeque<u8>,
    frame: u32,
    modulation: Modulation,
    gain: f32,
    nco: Nco,
    f0: f64,
    drift: f64,
    held: Vec<Complex32>,
    sent: usize,
    levels: Vec<f32>,
    syms: Vec<Complex32>,
}

impl Chan {
    fn next_frame(&mut self) {
        let s = self.sus.take(self.layout.sus());
        let info: Vec<u8> = s.iter().flat_map(|x| x.iter().copied()).collect();
        let f = self.frame;
        let h = header((f / 16) as u8, (f % 16) as u8, (f % 16) as u8);
        self.frame += 1;
        if let Some(b) = encode_frame(&self.layout, &info, h, &mut self.enc) {
            self.bits.extend(b);
        }
    }

    /// Makes more samples of this channel at 0 Hz: 64 channel bits.
    fn more(&mut self) {
        self.held.clear();
        self.sent = 0;
        while self.bits.len() < 64 {
            let before = self.bits.len();
            self.next_frame();
            if self.bits.len() == before {
                return;
            }
        }
        match &mut self.modulation {
            Modulation::Msk(m) => {
                self.levels.clear();
                for b in self.bits.drain(..64) {
                    self.levels.push(if b == 1 { 1.0 } else { -1.0 });
                }
                m.push(&self.levels, &mut self.held);
            }
            Modulation::Oqpsk(m) => {
                self.syms.clear();
                let a = std::f32::consts::FRAC_1_SQRT_2;
                let lv = |b: u8| if b == 1 { a } else { -a };
                let mut it = self.bits.drain(..64);
                // The real part carries Q, the imaginary part I (delayed by half a symbol); the arms are swapped after.
                while let (Some(q), Some(i)) = (it.next(), it.next()) {
                    self.syms.push(Complex32::new(lv(q), lv(i)));
                }
                drop(it);
                m.push(&self.syms, &mut self.held);
                for v in self.held.iter_mut() {
                    *v = Complex32::new(v.im, v.re);
                }
            }
        }
    }

    fn add(&mut self, out: &mut [Complex32], t0: f64, rate: f64) {
        let mut i = 0;
        while i < out.len() {
            if self.sent == self.held.len() {
                self.more();
                continue;
            }
            let n = (self.held.len() - self.sent).min(out.len() - i);
            // The carrier moves with the drift: the frequency is set at the start of each piece.
            self.nco
                .set_freq(self.f0 + self.drift * (t0 + i as f64 / rate));
            let piece = &mut self.held[self.sent..self.sent + n];
            self.nco.mix_in_place(piece);
            for (o, v) in out[i..i + n].iter_mut().zip(piece.iter()) {
                *o += v * self.gain;
            }
            self.sent += n;
            i += n;
        }
    }
}

pub struct AeroGenerator {
    p: GenParams,
    chans: Vec<Chan>,
    n: u64,
}

impl AeroGenerator {
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let mut p = GenParams::new(cfg.rate_hz);
        p.mask = u32::try_from(s.get(&GEN_CHANNELS)).unwrap_or(p.mask);
        p.plain = s.get(&GEN_PLAIN);
        Self::new(p)
    }

    /// The carrier of channel `rate` at the start, Hz from the output's 0 Hz.
    pub fn carrier_of(p: &GenParams, rate: u32) -> Option<f64> {
        let c = CHANNELS.iter().find(|c| c.0 == rate)?;
        Some(p.dial_hz + c.1 + if p.plain { 0.0 } else { c.2 })
    }

    fn used(p: &GenParams) -> impl Iterator<Item = &'static (u32, f64, f64, f64)> + '_ {
        CHANNELS.iter().filter(|c| {
            p.mask
                & match c.0 {
                    600 => 1,
                    1200 => 2,
                    _ => 4,
                }
                != 0
        })
    }

    pub fn new(p: GenParams) -> Self {
        // Every channel at the same energy per channel bit: each channel's power in proportion to its rate, the sum at unit power.
        let total: f64 = Self::used(&p).map(|c| c.0 as f64).sum::<f64>().max(1.0);
        let chans = Self::used(&p)
            .map(|&(rate, off, coff, dr)| {
                let layout = Layout::of(rate);
                let modulation = if rate == 10500 {
                    Modulation::Oqpsk(
                        Linear::rrc(1.0, 8.0, p.rate / 5250.0)
                            .expect("a valid RRC modulator")
                            .with_offset_q(),
                    )
                } else {
                    Modulation::Msk(Cpm::msk(p.rate / rate as f64).expect("a valid MSK modulator"))
                };
                Chan {
                    layout,
                    sus: SuSource::new(rate),
                    enc: Encoder::new(INMARSAT_K7),
                    bits: VecDeque::new(),
                    frame: 0,
                    modulation,
                    gain: (rate as f64 / total).sqrt() as f32,
                    nco: Nco::new(p.rate, 0.0),
                    f0: p.dial_hz + off + if p.plain { 0.0 } else { coff },
                    drift: if p.plain { 0.0 } else { dr },
                    held: Vec::new(),
                    sent: 0,
                    levels: Vec::new(),
                    syms: Vec::new(),
                }
            })
            .collect();
        AeroGenerator { chans, n: 0, p }
    }
}

impl Generator for AeroGenerator {
    fn rate_hz(&self) -> f64 {
        self.p.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::new(0.0, 0.0));
        let rate = self.p.rate;
        let t0 = self.n as f64 / rate;
        for c in &mut self.chans {
            c.add(out, t0, rate);
        }
        self.n += out.len() as u64;
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The channel bit rates add up: the same Eb/N0 holds for every channel, since each has power in proportion to its rate.
        let bits: f64 = self.chans.iter().map(|c| c.layout.rate as f64).sum();
        Some(SignalInfo::new(1.5 * bits).with_bit_rate(bits.max(1.0)))
    }
}
