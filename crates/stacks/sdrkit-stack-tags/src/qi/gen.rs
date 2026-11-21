//! The Qi test signal: the power signal of a transmitter, of amplitude 1 at 0 Hz, whose amplitude the power receiver lowers by
//! the depth while its load is switched on (Qi version 1.2.3, section 5.2.2.1). The packets are sent at 2 kbit/s in the
//! differential bi-phase code (section 5.2.2.2: a change at the start of each bit, one more in the middle of a 1), each with
//! its preamble, bytes and checksum (section 5.2.2.4), 30 ms apart. A lead of 50 ms and a tail of 0.6 s of steady power
//! signal frame the list, which then starts again. The edges pass a one-pole low-pass at 12 kHz. The noise is stated against the
//! power signal in 10 kHz.

use super::packet::{checksum, packet_bits, SEND_PREAMBLE_MAX, SEND_PREAMBLE_MIN};
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use sdrkit_util_bits::line::{Biphase, BiphaseKind};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The bit rate of the test signal, bit/s (section 5.2.2.2).
pub const BIT_RATE: f64 = 2000.0;
/// The band that the noise is stated in, Hz.
pub const NOISE_BAND_HZ: f64 = 10e3;
/// The cut-off of the edge filter, Hz.
pub const EDGE_HZ: f64 = 12e3;
/// The steady power signal before the first packet, s.
pub const LEAD_S: f64 = 0.05;
/// The steady power signal after the last packet, s.
pub const TAIL_S: f64 = 0.6;

/// The packets of the default list: header and message in hex; the checksum is added.
pub const DEFAULT_PACKETS: &str =
    "01 A0; 02 01; 03 F6; 04 50; 05 64; 06 0A; 51 0C 00 00 00 00; 71 11 00 1D 12 34 56 78; p11 04 12";

/// The modulation depth.
pub const GEN_DEPTH: Setting<field::F64> =
    Setting::new("gen_depth", "Load modulation depth", "", 0.05)
        .range(0.001, 0.9, 0.0)
        .section(SECTION);
/// The error of the bit clock of the power receiver.
pub const GEN_BIT_CLOCK_PPM: Setting<field::F64> =
    Setting::new("gen_bit_clock_ppm", "Bit clock error", "ppm", 0.0)
        .range(-200_000.0, 200_000.0, 0.0)
        .section(SECTION)
        .help("The bits run this much fast (positive) or slow.");
/// The frequency of a slow change of the power signal.
pub const GEN_WANDER_HZ: Setting<field::F64> =
    Setting::new("gen_wander_hz", "Wander frequency", "Hz", 0.0)
        .range(0.0, 100.0, 0.0)
        .section(SECTION);
/// The depth of a slow change of the power signal.
pub const GEN_WANDER_DEPTH: Setting<field::F64> =
    Setting::new("gen_wander_depth", "Wander depth", "", 0.0)
        .range(0.0, 0.5, 0.0)
        .section(SECTION);
/// The time between packets.
pub const GEN_GAP_S: Setting<field::F64> =
    Setting::new("gen_gap_s", "Time between packets", "s", 0.03)
        .range(0.005, 10.0, 0.0)
        .section(SECTION);
/// The preamble.
pub const GEN_PREAMBLE: Setting<field::I64> = Setting::new("gen_preamble", "Preamble", "bits", 12)
    .range(SEND_PREAMBLE_MIN as f64, SEND_PREAMBLE_MAX as f64, 1.0)
    .section(SECTION);
/// The packets.
pub const GEN_PACKETS: Setting<field::Text> = Setting::new("gen_packets", "Packets", "", DEFAULT_PACKETS)
    .section(SECTION)
    .help("Packets separated by ';': the header and the message in hex, the checksum added. In an entry, 'pN' sets a preamble of N bits, 'x' sends the bytes as given (the last is the checksum), 'fN' sends bit N of the packet inverted.");
/// A change after the last bit of each packet, as the modulator turns off.
pub const GEN_END_EDGE_BITS: Setting<field::F64> =
    Setting::new("gen_end_edge_bits", "Change after each packet", "bit", 0.0)
        .range(0.0, 10.0, 0.0)
        .section(SECTION)
        .help("One more change this long after the middle of the last stop bit; 0 for none.");

/// A packet of the list: its preamble and its bytes with the checksum.
#[derive(Clone, Debug, PartialEq)]
pub struct Sent {
    /// The preamble bits.
    pub preamble: usize,
    /// Header, message and checksum.
    pub bytes: Vec<u8>,
    /// A bit of the packet (from the first preamble bit) sent inverted, for tests of refused packets.
    pub flip: Option<usize>,
}

/// Reads a list of packets; `None` for an entry that cannot be read.
pub fn parse_packets(text: &str, preamble: usize) -> Option<Vec<Sent>> {
    text.split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| {
            let (mut pre, mut raw, mut flip) = (preamble, false, None);
            let mut bytes = Vec::new();
            for w in e.split_whitespace() {
                if let Some(n) = w.strip_prefix('p') {
                    pre = n.parse().ok()?;
                } else if let Some(n) = w.strip_prefix('f') {
                    flip = Some(n.parse().ok()?);
                } else if w == "x" {
                    raw = true;
                } else {
                    bytes.push(u8::from_str_radix(w, 16).ok()?);
                }
            }
            if bytes.is_empty() {
                return None;
            }
            if !raw {
                bytes.push(checksum(&bytes));
            }
            Some(Sent {
                preamble: pre,
                bytes,
                flip,
            })
        })
        .collect()
}

/// The line levels of a list, as times of changes: `(t, level after)`, seconds from the start of the list.
fn schedule(packets: &[Sent], bit_s: f64, gap_s: f64, end_edge_bits: f64) -> (Vec<(f64, u8)>, f64) {
    let mut changes = Vec::new();
    let mut t = LEAD_S;
    let mut level = 0u8;
    for (i, p) in packets.iter().enumerate() {
        if i > 0 {
            t += gap_s;
        }
        let mut bits = packet_bits(p.preamble, &p.bytes);
        if let Some(b) = p.flip.and_then(|k| bits.get_mut(k)) {
            *b ^= 1;
        }
        let mut chips = Vec::with_capacity(2 * bits.len());
        Biphase::new(BiphaseKind::Mark, level).encode(&bits, &mut chips);
        let mut last = level;
        for (k, &c) in chips.iter().enumerate() {
            if c != last {
                changes.push((t + k as f64 * bit_s / 2.0, c));
                last = c;
            }
        }
        // The middle of the last bit.
        let mid = t + (chips.len() - 1) as f64 * bit_s / 2.0;
        t += bits.len() as f64 * bit_s;
        if end_edge_bits > 0.0 {
            last ^= 1;
            changes.push((mid + end_edge_bits * bit_s, last));
        }
        level = last;
    }
    let end = t.max(changes.last().map_or(0.0, |c| c.0)) + TAIL_S;
    (changes, end)
}

/// The length of one pass of the list of the settings `v`, s.
pub fn list_s(v: &Values) -> f64 {
    let pre = usize::try_from(v.get(&GEN_PREAMBLE)).unwrap_or(12);
    let packets = parse_packets(v.get(&GEN_PACKETS), pre).unwrap_or_default();
    let bit_s = 1.0 / (BIT_RATE * (1.0 + v.get(&GEN_BIT_CLOCK_PPM) * 1e-6));
    schedule(
        &packets,
        bit_s,
        v.get(&GEN_GAP_S),
        v.get(&GEN_END_EDGE_BITS),
    )
    .1
}

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct QiGenerator {
    rate: f64,
    depth: f64,
    wander_hz: f64,
    wander: f64,
    changes: Vec<(f64, u8)>,
    period: f64,
    next: usize,
    level: u8,
    alpha: f64,
    y: f64,
    n: u64,
    pass: u64,
}

impl QiGenerator {
    /// The signal of the settings `v` at `rate_hz`; a list that cannot be read gives the default list.
    pub fn new(rate_hz: f64, v: &Values) -> QiGenerator {
        let pre = usize::try_from(v.get(&GEN_PREAMBLE)).unwrap_or(12);
        let packets = parse_packets(v.get(&GEN_PACKETS), pre)
            .or_else(|| parse_packets(DEFAULT_PACKETS, pre))
            .unwrap_or_default();
        let bit_s = 1.0 / (BIT_RATE * (1.0 + v.get(&GEN_BIT_CLOCK_PPM) * 1e-6));
        let (changes, period) = schedule(
            &packets,
            bit_s,
            v.get(&GEN_GAP_S),
            v.get(&GEN_END_EDGE_BITS),
        );
        QiGenerator {
            rate: rate_hz,
            depth: v.get(&GEN_DEPTH),
            wander_hz: v.get(&GEN_WANDER_HZ),
            wander: v.get(&GEN_WANDER_DEPTH),
            changes,
            period,
            next: 0,
            level: 0,
            alpha: 1.0 - (-TAU * EDGE_HZ / rate_hz).exp(),
            y: 1.0,
            n: 0,
            pass: 0,
        }
    }
}

impl Generator for QiGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let tl = t - self.pass as f64 * self.period;
            if tl >= self.period {
                self.pass += 1;
                self.next = 0;
                self.level = 0;
            }
            let tl = t - self.pass as f64 * self.period;
            while self.next < self.changes.len() && self.changes[self.next].0 <= tl {
                self.level = self.changes[self.next].1;
                self.next += 1;
            }
            let a = (1.0 - self.depth * f64::from(self.level))
                * (1.0 + self.wander * (TAU * self.wander_hz * t).sin());
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
