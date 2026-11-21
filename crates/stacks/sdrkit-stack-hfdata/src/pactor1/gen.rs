//! The PACTOR-I test signal generator: a script of packets and control signals on the 1.25 s cycle grid as
//! continuous-phase FSK in the upper sideband of a dial at 0 Hz, with the polarity swap.

use super::proto::{self, Huffman, Status};
use sdrkit_core::{Complex32, GenConfig, Generator, SignalInfo};
use std::f64::consts::TAU;

/// One event of a script.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptEvent {
    /// Start, seconds (before the clock error).
    pub time: f64,
    /// Bits in sending order (1 = the higher tone in the normal polarity).
    pub bits: Vec<u8>,
    pub baud: f64,
}

/// Cycle `k` starts at 0.5 + 1.25 k s.
pub fn cycle(k: usize) -> f64 {
    0.5 + 1.25 * k as f64
}

/// A packet at the start of cycle `k`.
pub fn packet(k: usize, baud: u32, header: u8, data: &[u8], status: Status) -> ScriptEvent {
    ScriptEvent {
        time: cycle(k),
        bits: proto::packet_bits(header, data, status),
        baud: baud as f64,
    }
}

/// Control signal `i` (0 = CS1) of cycle `k`, 1.13 s after its start.
pub fn control(k: usize, i: usize) -> ScriptEvent {
    ScriptEvent {
        time: cycle(k) + 1.13,
        bits: proto::cs_bits(i).to_vec(),
        baud: 100.0,
    }
}

/// ASCII text padded with spaces to the packet's length.
pub fn ascii(text: &str, len: usize) -> Vec<u8> {
    let mut d: Vec<u8> = text.bytes().take(len).collect();
    d.resize(len, b' ');
    d
}

/// Huffman text cut to the data length and padded with space codes "10" (and a final 1 when one bit is left).
pub fn huffman(text: &str, len: usize) -> Vec<u8> {
    let h = Huffman::default();
    let mut bits = h.encode(text).unwrap_or_default();
    bits.truncate(len * 8);
    while bits.len() + 2 <= len * 8 {
        bits.extend_from_slice(&[1, 0]);
    }
    if bits.len() < len * 8 {
        bits.push(1);
    }
    proto::bits_to_bytes(&bits)
}

/// The ARQ test script (neutral callsigns).
pub fn arq_script() -> Vec<ScriptEvent> {
    let st = |n: u8, h: bool, b: bool, q: bool| Status {
        number: n,
        huffman: h,
        break_in: b,
        qrt: q,
    };
    vec![
        packet(
            0,
            100,
            0x55,
            &proto::connect_data("XX1AA"),
            st(0, false, false, false),
        ),
        control(0, 3),
        packet(
            1,
            200,
            0xAA,
            &ascii("XX2BB de XX1AA", 20),
            st(1, false, false, false),
        ),
        control(1, 0),
        packet(
            2,
            100,
            0x55,
            &huffman("tea set at sea", 8),
            st(2, true, false, false),
        ),
        control(2, 1),
        packet(
            3,
            100,
            0x55,
            &huffman("tea set at sea", 8),
            st(2, true, false, false),
        ),
        control(3, 0),
        packet(
            4,
            200,
            0xAA,
            &ascii("ASCII at 200 baud 73", 20),
            st(3, false, false, false),
        ),
        control(4, 2),
        packet(
            5,
            100,
            0x55,
            &huffman("a tea set", 8),
            st(0, true, true, false),
        ),
        control(5, 0),
        packet(6, 100, 0xAA, &[b' '; 8], st(1, false, false, true)),
        control(6, 0),
    ]
}

/// The FEC test script: every packet three times, no control signals, the header swapped with every new packet.
pub fn fec_script() -> Vec<ScriptEvent> {
    let mut v = Vec::new();
    let mut k = 0;
    let mut header = 0xAAu8;
    let mut n = 0u8;
    let text = "CQ CQ DE XX2BB XX2BB PSE K";
    let mut pieces: Vec<(Vec<u8>, u32, bool)> = text
        .as_bytes()
        .chunks(20)
        .map(|c| (ascii(std::str::from_utf8(c).unwrap_or(""), 20), 200, false))
        .collect();
    let h = "eat tea set at sea sea";
    for c in h.as_bytes().chunks(6) {
        pieces.push((huffman(std::str::from_utf8(c).unwrap_or(""), 8), 100, true));
    }
    for (data, baud, huf) in pieces {
        for _ in 0..3 {
            v.push(packet(
                k,
                baud,
                header,
                &data,
                Status {
                    number: n,
                    huffman: huf,
                    ..Default::default()
                },
            ));
            k += 1;
        }
        header = if header == 0xAA { 0x55 } else { 0xAA };
        n = (n + 1) & 3;
    }
    v
}

/// The generator's options.
#[derive(Clone, Debug)]
pub struct Pactor1GenOptions {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The FSK centre in the sideband audio, Hz above the dial.
    pub centre_hz: f64,
    /// Swaps the polarity on every second cycle, as the real system does.
    pub swap: bool,
    /// Starts again after 3 s of silence.
    pub repeat: bool,
    /// Seconds after the last event.
    pub tail_s: f64,
    /// The packets and control signals.
    pub script: Vec<ScriptEvent>,
}

impl Default for Pactor1GenOptions {
    fn default() -> Self {
        Pactor1GenOptions {
            rate: 24_000.0,
            centre_hz: 1500.0,
            swap: true,
            repeat: false,
            tail_s: 0.3,
            script: arq_script(),
        }
    }
}

/// The generator: unit mean power while a packet or a control signal is on.
pub struct Pactor1Generator {
    o: Pactor1GenOptions,
    i: u64,
    cur: usize,
    phase: f64,
    end: f64,
}

impl Pactor1Generator {
    /// The generator of `o`.
    pub fn new(o: Pactor1GenOptions) -> Self {
        let end = o
            .script
            .iter()
            .map(|e| e.time + e.bits.len() as f64 / e.baud)
            .fold(0.0, f64::max)
            + o.tail_s;
        Pactor1Generator {
            i: 0,
            cur: 0,
            phase: 0.0,
            end,
            o,
        }
    }

    /// The generator of the stack's settings: `gen_script`.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        Pactor1Generator::new(Pactor1GenOptions {
            rate: cfg.rate_hz,
            repeat: true,
            script: if s.get(&super::GEN_SCRIPT) == super::GenScript::Fec {
                fec_script()
            } else {
                arq_script()
            },
            ..Default::default()
        })
    }

    /// The length of the script, seconds.
    pub fn length_s(&self) -> f64 {
        self.end
    }
}

impl Generator for Pactor1Generator {
    fn rate_hz(&self) -> f64 {
        self.o.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The tone SNR of PACTOR-I is stated in 2.5 kHz. A script mixes 100 and 200 baud.
        Some(SignalInfo::new(2500.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let rate = self.o.rate;
        let cycle_len = if self.o.repeat {
            self.end + 3.0
        } else {
            self.end
        };
        for (k, o) in out.iter_mut().enumerate() {
            let mut t = self.i as f64 / rate;
            if t >= cycle_len {
                if !self.o.repeat {
                    return k;
                }
                self.i = 0;
                self.cur = 0;
                t = 0.0;
            }
            self.i += 1;
            while self.cur < self.o.script.len() {
                let e = &self.o.script[self.cur];
                if t >= e.time + e.bits.len() as f64 / e.baud {
                    self.cur += 1;
                } else {
                    break;
                }
            }
            let mut s = Complex32::new(0.0, 0.0);
            if let Some(e) = self.o.script.get(self.cur) {
                if t >= e.time {
                    let bit = ((t - e.time) * e.baud) as usize;
                    if bit < e.bits.len() {
                        let cyc = ((e.time - 0.5 + 1e-6) / 1.25).floor().max(0.0) as i64;
                        let inv = self.o.swap && cyc % 2 == 1;
                        let up = (e.bits[bit] == 1) != inv;
                        let f = self.o.centre_hz + if up { 100.0 } else { -100.0 };
                        self.phase = (self.phase + TAU * f / rate) % TAU;
                        s = Complex32::new(self.phase.cos() as f32, self.phase.sin() as f32);
                    }
                }
            }
            *o = s;
        }
        out.len()
    }
}
