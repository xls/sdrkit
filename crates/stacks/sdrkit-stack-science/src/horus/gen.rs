//! The Horus test signal (section 3.13 of the specification): Horus Binary v2 or v1 as phase-continuous 4FSK at 100 baud, or
//! RTTY sentences at 50, 100 or 300 baud, of a simulated flight; or three balloons in one capture. The signal is made piece by
//! piece, so a long capture at a high rate needs no buffer of its own. The generator adds no noise.

use super::layer2::{Layer2, PREAMBLE};
use super::packet::Binary;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

sdrkit_core::choice_enum! {
    /// What the test signal sends.
    pub enum Content {
        /// Horus Binary v2.
        V2 = ("v2", "Horus Binary v2"),
        /// Horus Binary v1.
        V1 = ("v1", "Horus Binary v1"),
        /// RTTY at 100 baud, 7 data bits, 2 stop bits.
        Rtty100 = ("rtty100", "RTTY 100 baud 7N2"),
        /// RTTY at 300 baud, 8 data bits, 2 stop bits.
        Rtty300 = ("rtty300", "RTTY 300 baud 8N2"),
        /// RTTY at 50 baud, 7 data bits, 2 stop bits.
        Rtty50 = ("rtty50", "RTTY 50 baud 7N2"),
        /// v2 on the dial, v1 12 kHz above with the opposite clock error, RTTY at 100 baud 18 kHz below.
        Three = ("three", "Three balloons"),
    }
}

/// The options of the test signal.
#[derive(Clone, Debug, PartialEq)]
pub struct GenOptions {
    /// The output rate, Hz.
    pub rate_hz: f64,
    /// What it sends.
    pub content: Content,
    /// The tone spacing, Hz; `None`: 270 for 4FSK, 500 for RTTY.
    pub spacing_hz: Option<f64>,
    /// The tone order is reversed.
    pub mirrored: bool,
    /// The symbol clock error, ppm (positive: fast).
    pub clock_ppm: f64,
    /// The dial, Hz from the centre of the samples.
    pub dial_hz: f64,
    /// The carrier error, Hz, added to the dial.
    pub carrier_hz: f64,
    /// The length that [`render`], [`complete`] and [`packet_starts`] take, s.
    pub seconds: f64,
    /// The amplitude of each balloon.
    pub amplitude: f64,
}

impl Default for GenOptions {
    fn default() -> Self {
        GenOptions {
            rate_hz: 48_000.0,
            content: Content::V2,
            spacing_hz: None,
            mirrored: false,
            clock_ppm: 0.0,
            dial_hz: 0.0,
            carrier_hz: 0.0,
            seconds: 30.0,
            amplitude: 0.5,
        }
    }
}

/// The dial of the specification's test captures, Hz from the centre: 30 kHz below at 120 ksps and more, else the centre.
pub fn dial_hz(rate: f64) -> f64 {
    if rate >= 120_000.0 {
        -30_000.0
    } else {
        0.0
    }
}

/// The v2 custom bytes of the test signal: ascent 5.00 m/s, -12.3 degrees C, 45 %, 850.0 hPa in the RS41ng format.
pub const CUSTOM: [u8; 9] = [0xF4, 0x01, 0x85, 0xFF, 0x2D, 0x34, 0x21, 0x00, 0x00];
/// The callsign of the RTTY balloon.
pub const RTTY_CALLSIGN: &str = "RTTYTEST";

/// The packet `k` of the simulated flight of `version` (1, 2, or 0 for RTTY), `t` seconds after 12:00:00.
pub fn flight(version: u8, k: usize, t: f64) -> Binary {
    let dlat = match version {
        1 => 0.05,
        0 => -0.05,
        _ => 0.0,
    };
    let alt = (250.0 + 5.0 * t).min(33_000.0);
    let secs = t as u32;
    Binary {
        version: version.max(1),
        id: if version == 2 { 256 } else { 0 },
        seq: 100 + k as u16,
        time: (
            12 + (secs / 3600) as u8,
            ((secs / 60) % 60) as u8,
            (secs % 60) as u8,
        ),
        lat: 54.8985 + 0.00003 * t + dlat,
        lon: 23.9036 + 0.00016 * t,
        alt_m: alt as u16,
        speed_kmh: 38,
        sats: 9,
        temp_c: (15.0 - 0.0065 * alt).max(-60.0).round() as i8,
        battery: 153,
        custom: (version == 2).then_some(CUSTOM),
    }
}

/// The RTTY sentence of packet `p`.
pub fn rtty_sentence(p: &Binary) -> String {
    p.sentence(RTTY_CALLSIGN, &[])
}

/// What one balloon sends.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// Horus Binary of this version.
    Binary(u8),
    /// RTTY at this rate with this many data bits.
    Rtty(f64, u8),
}

impl Kind {
    fn baud(self) -> f64 {
        match self {
            Kind::Binary(_) => 100.0,
            Kind::Rtty(b, _) => b,
        }
    }

    fn tones(self) -> usize {
        match self {
            Kind::Binary(_) => 4,
            Kind::Rtty(..) => 2,
        }
    }

    /// The symbols of packet `k`, which starts `t` s after 12:00:00: a tone index or `None` for off. Binary: 8 preamble bytes,
    /// the frame, 1 s off. RTTY: 0.5 s of mark, the sentence and a newline with two stop bits a character, 0.3 s off; 1 is mark
    /// (the higher tone).
    fn packet(self, k: usize, t: f64) -> Vec<Option<u8>> {
        let mut out = Vec::new();
        match self {
            Kind::Binary(version) => {
                let bytes = flight(version, k, t).to_bytes();
                let frame = Layer2::new(bytes.len())
                    .expect("a v1 or v2 payload")
                    .encode(&bytes);
                for b in std::iter::repeat_n(PREAMBLE, 8).chain(frame) {
                    out.extend([b >> 6, (b >> 4) & 3, (b >> 2) & 3, b & 3].map(Some));
                }
                out.extend(std::iter::repeat_n(None, 100));
            }
            Kind::Rtty(baud, data) => {
                let text = format!("{}\n", rtty_sentence(&flight(0, k, t)));
                out.extend(std::iter::repeat_n(Some(1), (0.5 * baud) as usize));
                for c in text.bytes() {
                    out.push(Some(0));
                    out.extend((0..data).map(|i| Some((c >> i) & 1)));
                    out.extend([Some(1), Some(1)]);
                }
                out.extend(std::iter::repeat_n(None, (0.3 * baud) as usize));
            }
        }
        out
    }
}

/// One balloon: its symbols made packet by packet, and its tones as phasors that turn by one sample at a time.
#[derive(Clone, Debug)]
struct Balloon {
    kind: Kind,
    /// Symbols a sample, with the clock error.
    per_sample: f64,
    /// The turn of each tone in one sample.
    turn: Vec<Complex32>,
    phasor: Complex32,
    mirrored: bool,
    amp: f32,
    /// The symbols of the present packet, and the symbol index of its first.
    packet: Vec<Option<u8>>,
    first: usize,
    k: usize,
}

impl Balloon {
    fn new(
        kind: Kind,
        rate: f64,
        centre_hz: f64,
        spacing: f64,
        clock_ppm: f64,
        mirrored: bool,
        amp: f64,
    ) -> Balloon {
        let m = kind.tones();
        let mid = (m as f64 - 1.0) / 2.0;
        let turn = (0..m)
            .map(|s| {
                let f = centre_hz + (s as f64 - mid) * spacing;
                Complex32::from_polar(1.0, (TAU * f / rate) as f32)
            })
            .collect();
        Balloon {
            kind,
            per_sample: kind.baud() * (1.0 + clock_ppm * 1e-6) / rate,
            turn,
            phasor: Complex32::new(1.0, 0.0),
            mirrored,
            amp: amp as f32,
            packet: kind.packet(0, 0.0),
            first: 0,
            k: 0,
        }
    }

    /// Adds the samples from sample `n` on to `out`.
    fn add(&mut self, n: u64, out: &mut [Complex32]) {
        let m = self.kind.tones() as u8;
        for (i, z) in out.iter_mut().enumerate() {
            let j = ((n + i as u64) as f64 * self.per_sample) as usize;
            while j >= self.first + self.packet.len() {
                self.first += self.packet.len();
                self.k += 1;
                self.packet = self
                    .kind
                    .packet(self.k, self.first as f64 / self.kind.baud());
            }
            let Some(s) = self.packet[j - self.first] else {
                continue;
            };
            let s = if self.mirrored { m - 1 - s } else { s };
            self.phasor *= self.turn[usize::from(s)];
            // The rounding of the turns would change the amplitude slowly.
            if (n + i as u64).is_multiple_of(256) {
                self.phasor /= self.phasor.norm();
            }
            *z += self.phasor * self.amp;
        }
    }
}

/// The test signal, made piece by piece.
#[derive(Clone, Debug)]
pub struct Signal {
    balloons: Vec<Balloon>,
    n: u64,
}

impl Signal {
    /// The signal of `o`.
    pub fn new(o: &GenOptions) -> Signal {
        let dial = o.dial_hz + o.carrier_hz;
        let one = |kind: Kind, at: f64, ppm: f64| {
            let spacing = o
                .spacing_hz
                .unwrap_or(if kind.tones() == 4 { 270.0 } else { 500.0 });
            Balloon::new(kind, o.rate_hz, at, spacing, ppm, o.mirrored, o.amplitude)
        };
        let balloons = match o.content {
            Content::V2 => vec![one(Kind::Binary(2), dial, o.clock_ppm)],
            Content::V1 => vec![one(Kind::Binary(1), dial, o.clock_ppm)],
            Content::Rtty100 => vec![one(Kind::Rtty(100.0, 7), dial, o.clock_ppm)],
            Content::Rtty300 => vec![one(Kind::Rtty(300.0, 8), dial, o.clock_ppm)],
            Content::Rtty50 => vec![one(Kind::Rtty(50.0, 7), dial, o.clock_ppm)],
            Content::Three => vec![
                one(Kind::Binary(2), dial, o.clock_ppm),
                one(Kind::Binary(1), dial + 12_000.0, -o.clock_ppm),
                one(Kind::Rtty(100.0, 7), dial - 18_000.0, o.clock_ppm),
            ],
        };
        Signal { balloons, n: 0 }
    }

    /// Writes the next `out.len()` samples.
    pub fn fill(&mut self, out: &mut [Complex32]) {
        out.fill(Complex32::default());
        for b in &mut self.balloons {
            b.add(self.n, out);
        }
        self.n += out.len() as u64;
    }
}

/// `o.seconds` of the test signal of `o`.
pub fn render(o: &GenOptions) -> Vec<Complex32> {
    let mut out = vec![Complex32::default(); (o.seconds * o.rate_hz) as usize];
    Signal::new(o).fill(&mut out);
    out
}

/// The kind of the single balloon of `o`; `None` for three balloons.
fn single(o: &GenOptions) -> Option<Kind> {
    Some(match o.content {
        Content::V2 => Kind::Binary(2),
        Content::V1 => Kind::Binary(1),
        Content::Rtty100 => Kind::Rtty(100.0, 7),
        Content::Rtty300 => Kind::Rtty(300.0, 8),
        Content::Rtty50 => Kind::Rtty(50.0, 7),
        Content::Three => return None,
    })
}

/// The start of each packet of the single balloon of `o` that starts within `o.seconds`, s after 12:00:00 (the time its
/// content carries); empty for three balloons.
pub fn packet_starts(o: &GenOptions) -> Vec<f64> {
    let Some(kind) = single(o) else {
        return Vec::new();
    };
    let have = o.seconds * kind.baud() * (1.0 + o.clock_ppm * 1e-6);
    let mut starts = Vec::new();
    let mut first = 0usize;
    while (first as f64) < have {
        let t = first as f64 / kind.baud();
        starts.push(t);
        first += kind.packet(starts.len() - 1, t).len();
    }
    starts
}

/// The packets that the single balloon of `o` sends in full within `o.seconds` (none for three balloons).
pub fn complete(o: &GenOptions) -> usize {
    let Some(kind) = single(o) else {
        return 0;
    };
    let have = o.seconds * kind.baud() * (1.0 + o.clock_ppm * 1e-6);
    let mut first = 0usize;
    let mut n = 0;
    for (k, &t) in packet_starts(o).iter().enumerate() {
        let p = kind.packet(k, t);
        // A packet ends where its symbols meet the first symbol that is off.
        let on = p.iter().take_while(|s| s.is_some()).count();
        if ((first + on) as f64) <= have {
            n += 1;
        }
        first += p.len();
    }
    n
}
