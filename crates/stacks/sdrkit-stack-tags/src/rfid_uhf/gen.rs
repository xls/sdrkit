//! The EPC Gen2 test signal: a reader carrier of amplitude 1 at 0 Hz that sends commands by pulse-interval encoding (Gen2
//! version 2.0.1, sections 6.3.1.2.3 to 6.3.1.2.8). Each command starts with a delimiter of 12.5 us at the low level, then a
//! data-0, RTcal and, before a Query, TRcal, then its bits. Every symbol is carrier for its length less PW, then the low level
//! (1 - depth) for PW; PW is at least 2 us. A data-0 is one Tari, a data-1 the set share of Tari. The edges pass a one-pole
//! low-pass at 500 kHz. A lead, the commands with a gap between them and a tail of 0.5 s make one pass, which then starts
//! again. The noise is stated against the carrier in 600 kHz.

use super::command::encode;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The band that the noise is stated in, Hz.
pub const NOISE_BAND_HZ: f64 = 600e3;
/// The cut-off of the edge filter, Hz.
pub const EDGE_HZ: f64 = 500e3;
/// The delimiter, s.
pub const DELIMITER_S: f64 = 12.5e-6;
/// The shortest PW, s (Table 6.5).
pub const PW_MIN_S: f64 = 2e-6;
/// The steady carrier after the last command, s.
pub const TAIL_S: f64 = 0.5;

/// The commands of the default list.
pub const DEFAULT_COMMANDS: &str = "query dr=0 m=2 trext=1 sel=3 session=2 target=1 q=4; ack rn=BEEF; queryrep session=2; \
queryadjust session=1 updn=110; nak; select target=4 action=0 bank=1 pointer=32 mask=1010011100110101 truncate=0; \
reqrn rn=1234; read bank=3 pointer=5 count=4 rn=A5A5";

/// Tari.
pub const GEN_TARI_US: Setting<field::F64> = Setting::new("gen_tari_us", "Tari", "us", 12.5)
    .range(6.25, 25.0, 0.0)
    .section(SECTION);
/// The length of a data-1.
pub const GEN_DATA1_TARI: Setting<field::F64> =
    Setting::new("gen_data1_tari", "Data-1", "Tari", 1.75)
        .range(1.5, 2.0, 0.0)
        .section(SECTION);
/// The low pulse.
pub const GEN_PW_TARI: Setting<field::F64> =
    Setting::new("gen_pw_tari", "Low pulse (PW)", "Tari", 0.45)
        .range(0.265, 0.525, 0.0)
        .section(SECTION);
/// The depth.
pub const GEN_DEPTH: Setting<field::F64> = Setting::new("gen_depth", "Modulation depth", "", 0.9)
    .range(0.8, 1.0, 0.0)
    .section(SECTION);
/// TRcal.
pub const GEN_TRCAL_RTCAL: Setting<field::F64> =
    Setting::new("gen_trcal_rtcal", "TRcal", "RTcal", 2.0)
        .range(1.1, 3.0, 0.0)
        .section(SECTION);
/// The error of the reader clock.
pub const GEN_READER_CLOCK_PPM: Setting<field::F64> =
    Setting::new("gen_reader_clock_ppm", "Reader clock error", "ppm", 0.0)
        .range(-50_000.0, 50_000.0, 0.0)
        .section(SECTION)
        .help("Every time of the reader runs this much fast (positive) or slow.");
/// The gap between commands.
pub const GEN_GAP_S: Setting<field::F64> =
    Setting::new("gen_gap_s", "Gap between commands", "s", 0.002)
        .range(0.0005, 1.0, 0.0)
        .section(SECTION);
/// The lead.
pub const GEN_LEAD_S: Setting<field::F64> = Setting::new("gen_lead_s", "Lead", "s", 0.002)
    .range(0.0, 10.0, 0.0)
    .section(SECTION);
/// The commands.
pub const GEN_COMMANDS: Setting<field::Text> = Setting::new("gen_commands", "Commands", "", DEFAULT_COMMANDS)
    .section(SECTION)
    .help("Commands separated by ';': a name (query, queryrep, queryadjust, ack, nak, select, reqrn, read, write, kill, lock, \
access) and key=value fields; rn, data, half and payload are hex, updn and mask are bits, flip=N sends bit N inverted.");

/// A command of the list: its bits and whether a preamble (with TRcal) leads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The bits, CRC included.
    pub bits: Vec<u8>,
    /// A Query: the preamble has TRcal.
    pub trcal: bool,
}

fn bin(s: &str) -> Option<Vec<u8>> {
    s.bytes()
        .map(|c| match c {
            b'0' => Some(0),
            b'1' => Some(1),
            _ => None,
        })
        .collect()
}

/// Reads one command of the list; `None` when it cannot be read.
pub fn parse_command(text: &str) -> Option<Frame> {
    let mut words = text.split_whitespace();
    let name = words.next()?;
    let mut kv = std::collections::HashMap::new();
    for w in words {
        let (k, v) = w.split_once('=')?;
        kv.insert(k, v);
    }
    let num = |k: &str| -> Option<u64> { kv.get(k).map_or(Some(0), |v| v.parse().ok()) };
    let hex = |k: &str| -> Option<u64> {
        kv.get(k)
            .map_or(Some(0), |v| u64::from_str_radix(v, 16).ok())
    };
    let bits = |k: &str| -> Option<u64> {
        kv.get(k).map_or(Some(0), |v| {
            bin(v).map(|b| b.iter().fold(0, |a, &x| (a << 1) | u64::from(x)))
        })
    };
    let mut f = match name {
        "query" => Frame {
            bits: encode::query(
                num("dr")?,
                num("m")?,
                num("trext")?,
                num("sel")?,
                num("session")?,
                num("target")?,
                num("q")?,
            ),
            trcal: true,
        },
        "queryrep" => Frame {
            bits: encode::query_rep(num("session")?),
            trcal: false,
        },
        "queryadjust" => Frame {
            bits: encode::query_adjust(num("session")?, bits("updn")?),
            trcal: false,
        },
        "ack" => Frame {
            bits: encode::ack(hex("rn")?),
            trcal: false,
        },
        "nak" => Frame {
            bits: encode::nak(),
            trcal: false,
        },
        "select" => {
            let mask = kv.get("mask").map_or(Some(Vec::new()), |m| bin(m))?;
            Frame {
                bits: encode::select(
                    num("target")?,
                    num("action")?,
                    num("bank")?,
                    num("pointer")?,
                    &mask,
                    num("truncate")?,
                ),
                trcal: false,
            }
        }
        "reqrn" => Frame {
            bits: encode::req_rn(hex("rn")?),
            trcal: false,
        },
        "read" => Frame {
            bits: encode::read(num("bank")?, num("pointer")?, num("count")?, hex("rn")?),
            trcal: false,
        },
        "write" => Frame {
            bits: encode::write(num("bank")?, num("pointer")?, hex("data")?, hex("rn")?),
            trcal: false,
        },
        "kill" => Frame {
            bits: encode::kill(hex("half")?, num("rfu")?, hex("rn")?),
            trcal: false,
        },
        "lock" => Frame {
            bits: encode::lock(hex("payload")?, hex("rn")?),
            trcal: false,
        },
        "access" => Frame {
            bits: encode::access(hex("half")?, hex("rn")?),
            trcal: false,
        },
        _ => return None,
    };
    if let Some(k) = kv.get("flip") {
        let k: usize = k.parse().ok()?;
        *f.bits.get_mut(k)? ^= 1;
    }
    Some(f)
}

/// Reads the list of commands; `None` when one cannot be read.
pub fn parse_commands(text: &str) -> Option<Vec<Frame>> {
    text.split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(parse_command)
        .collect()
}

/// The timing of the reader.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Timing {
    tari: f64,
    data1: f64,
    pw: f64,
    trcal: f64,
    scale: f64,
}

/// The changes of level of one pass: `(t, high after)`, and its length.
fn schedule(frames: &[Frame], tm: Timing, lead: f64, gap: f64) -> (Vec<(f64, bool)>, f64) {
    let s = tm.scale;
    let mut ch = Vec::new();
    let mut t = lead;
    let rtcal = tm.tari + tm.data1;
    for f in frames {
        ch.push((t, false));
        t += DELIMITER_S * s;
        ch.push((t, true));
        let mut syms = vec![tm.tari, rtcal];
        if f.trcal {
            syms.push(tm.trcal * rtcal);
        }
        syms.extend(
            f.bits
                .iter()
                .map(|&b| if b == 1 { tm.data1 } else { tm.tari }),
        );
        for l in syms {
            t += (l - tm.pw) * s;
            ch.push((t, false));
            t += tm.pw * s;
            ch.push((t, true));
        }
        t += gap;
    }
    (ch, t + TAIL_S)
}

fn timing(v: &Values) -> Timing {
    let tari = v.get(&GEN_TARI_US) * 1e-6;
    Timing {
        tari,
        data1: v.get(&GEN_DATA1_TARI) * tari,
        pw: (v.get(&GEN_PW_TARI) * tari).max(PW_MIN_S),
        trcal: v.get(&GEN_TRCAL_RTCAL),
        scale: 1.0 / (1.0 + v.get(&GEN_READER_CLOCK_PPM) * 1e-6),
    }
}

/// The length of one pass of the settings `v`, s.
pub fn pass_s(v: &Values) -> f64 {
    let frames = parse_commands(v.get(&GEN_COMMANDS)).unwrap_or_default();
    schedule(&frames, timing(v), v.get(&GEN_LEAD_S), v.get(&GEN_GAP_S)).1
}

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct UhfGenerator {
    rate: f64,
    low: f64,
    changes: Vec<(f64, bool)>,
    period: f64,
    next: usize,
    high: bool,
    alpha: f64,
    y: f64,
    n: u64,
    pass: u64,
}

impl UhfGenerator {
    /// The signal of the settings `v` at `rate_hz`; a list that cannot be read gives the default list.
    pub fn new(rate_hz: f64, v: &Values) -> UhfGenerator {
        let frames = parse_commands(v.get(&GEN_COMMANDS))
            .or_else(|| parse_commands(DEFAULT_COMMANDS))
            .unwrap_or_default();
        let (changes, period) = schedule(&frames, timing(v), v.get(&GEN_LEAD_S), v.get(&GEN_GAP_S));
        UhfGenerator {
            rate: rate_hz,
            low: 1.0 - v.get(&GEN_DEPTH),
            changes,
            period,
            next: 0,
            high: true,
            alpha: 1.0 - (-TAU * EDGE_HZ / rate_hz).exp(),
            y: 1.0,
            n: 0,
            pass: 0,
        }
    }
}

impl Generator for UhfGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            if t - self.pass as f64 * self.period >= self.period {
                self.pass += 1;
                self.next = 0;
            }
            let tl = t - self.pass as f64 * self.period;
            while self.next < self.changes.len() && self.changes[self.next].0 <= tl {
                self.high = self.changes[self.next].1;
                self.next += 1;
            }
            let a = if self.high { 1.0 } else { self.low };
            self.y += self.alpha * (a - self.y);
            *z = Complex32::new(self.y as f32, 0.0);
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
