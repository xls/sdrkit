//! RTCM SC-104 version 2 as the DGPS beacons send it: 30-bit words (24 data bits and the GPS word parity), frames of a two-word
//! header and N data words. The framer finds the preamble and cuts frames, and the decoder reads message types 1, 2, 3, 5, 6, 7,
//! 9 and 16. The word parity is the shared one of `sdrkit-util-bits`.
//! Standard: RTCM 10402.3 (RTCM SC-104 version 2.3) section 4, IS-GPS-200 20.3.5.

use super::StationHealth;
use crate::typed::{RtcmFrameEvent, RTCM_FRAME};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::keys;
use sdrkit_core::{ConfigError, EventDef, Record, Values};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use sdrkit_util_bits::crc::cyclic::{gps_lnav_check, gps_lnav_word};
use sdrkit_util_bits::{BitReader, BitWriter};
use sdrkit_util_geo::{Ecef, Position};

/// The preamble.
pub const PREAMBLE: u32 = 0x66;

/// A 30-bit word as sent, from 24 data bits and the previous word's D29* and D30* (IS-GPS-200 20.3.5.2).
pub fn encode_word(data: u32, d29s: bool, d30s: bool) -> u32 {
    gps_lnav_word(data, d29s as u8, d30s as u8)
}

/// The 24 data bits of a 30-bit word as received, if its parity is right.
pub fn decode_word(word: u32, d29s: bool, d30s: bool) -> Option<u32> {
    gps_lnav_check(word, d29s as u8, d30s as u8).ok()
}

/// One frame. It leaves the graph as the event `rtcm.frame`.
#[derive(Clone, Debug, PartialEq)]
pub struct RtcmFrame {
    /// Message type, 1 to 63.
    pub msg_type: u8,
    /// Base station id, 0 to 1023.
    pub station_id: u16,
    /// Modified Z-count: seconds into the hour (0.6 s steps).
    pub zcount_s: f64,
    /// Sequence number, 0 to 7.
    pub sequence: u8,
    /// Station health, 0 to 7 (7: not working, 6: not monitored).
    pub health: u8,
    /// The data words, 24 bits each.
    pub data: Vec<u32>,
}

impl Item for RtcmFrame {
    const KIND: &'static str = "rtcm.frame";
    fn event_def() -> Option<&'static EventDef> {
        Some(RTCM_FRAME.def())
    }
    fn write_fields(&self, r: &mut Record) {
        RtcmFrameEvent {
            source: &self.station_id.to_string(),
            text: &decode(self).summary(),
            family: keys::Family::Other,
            msg_type: i64::from(self.msg_type),
            station_id: i64::from(self.station_id),
            zcount_s: self.zcount_s,
            sequence: i64::from(self.sequence),
            health: StationHealth::of(self.health),
        }
        .write(r);
    }
}

/// The header's two data words of a frame.
pub fn header_words(
    msg_type: u8,
    station: u16,
    zcount_s: f64,
    seq: u8,
    n: usize,
    health: u8,
) -> [u32; 2] {
    let z = ((zcount_s / 0.6).round() as u32) % 6000;
    [
        (PREAMBLE << 16) | ((msg_type as u32 & 0x3F) << 10) | (station as u32 & 0x3FF),
        ((z & 0x1FFF) << 11)
            | ((seq as u32 & 7) << 8)
            | ((n as u32 & 0x1F) << 3)
            | (health as u32 & 7),
    ]
}

/// The framer's counters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramerStats {
    /// Words with good parity.
    pub good_words: u64,
    /// Words with bad parity while in a frame.
    pub bad_words: u64,
    /// Frames delivered.
    pub frames: u64,
    /// Locked to the word boundaries.
    pub locked: bool,
}

/// The framer state.
#[derive(Clone, Debug, Default)]
struct Framer {
    /// The last 32 bits received (newest in bit 0), as received.
    reg: u64,
    nbits: u32,
    /// Bits received are inverted (the demodulator's sense is opposite).
    invert: bool,
    locked: bool,
    /// Bits collected for the current word.
    word: u32,
    have: u32,
    /// D29* and D30*: the last two bits of the previous word, corrected.
    d29: bool,
    d30: bool,
    header: Option<(u32, u32)>,
    first: Option<u32>,
    data: Vec<u32>,
    need: usize,
    stats: FramerStats,
}

impl Framer {
    fn reset(&mut self) {
        self.locked = false;
        self.header = None;
        self.first = None;
        self.data.clear();
        self.have = 0;
        self.word = 0;
    }

    /// Takes one bit. Returns a frame that this bit finishes.
    fn push(&mut self, bit: bool) -> Option<RtcmFrame> {
        self.reg = (self.reg << 1) | bit as u64;
        self.nbits = self.nbits.saturating_add(1);
        if !self.locked {
            if self.nbits < 32 {
                return None;
            }
            // Try both senses: the word must carry the preamble and good parity.
            for inv in [false, true] {
                let r = if inv { !self.reg } else { self.reg };
                let w = (r & 0x3FFF_FFFF) as u32;
                let d29 = (r >> 31) & 1 == 1;
                let d30 = (r >> 30) & 1 == 1;
                if let Some(d) = decode_word(w, d29, d30) {
                    if d >> 16 == PREAMBLE {
                        self.invert = inv;
                        self.locked = true;
                        self.first = Some(d);
                        self.header = None;
                        self.data.clear();
                        self.d29 = (w >> 1) & 1 == 1;
                        self.d30 = w & 1 == 1;
                        self.have = 0;
                        self.word = 0;
                        self.stats.good_words += 1;
                        return None;
                    }
                }
            }
            return None;
        }
        let b = bit ^ self.invert;
        self.word = (self.word << 1) | b as u32;
        self.have += 1;
        if self.have < 30 {
            return None;
        }
        let w = self.word & 0x3FFF_FFFF;
        self.have = 0;
        self.word = 0;
        let Some(d) = decode_word(w, self.d29, self.d30) else {
            self.stats.bad_words += 1;
            self.reset();
            return None;
        };
        self.stats.good_words += 1;
        self.d29 = (w >> 1) & 1 == 1;
        self.d30 = w & 1 == 1;
        match (self.first, self.header) {
            (None, _) => {
                // Expecting the next frame's first word.
                if d >> 16 == PREAMBLE {
                    self.first = Some(d);
                } else {
                    self.reset();
                }
                None
            }
            (Some(w1), None) => {
                self.header = Some((w1, d));
                self.need = ((d >> 3) & 0x1F) as usize;
                self.data.clear();
                self.finish_if_complete()
            }
            (Some(_), Some(_)) => {
                self.data.push(d);
                self.finish_if_complete()
            }
        }
    }

    fn finish_if_complete(&mut self) -> Option<RtcmFrame> {
        let (w1, w2) = self.header?;
        if self.data.len() < self.need {
            return None;
        }
        self.first = None;
        self.header = None;
        self.stats.frames += 1;
        Some(RtcmFrame {
            msg_type: ((w1 >> 10) & 0x3F) as u8,
            station_id: (w1 & 0x3FF) as u16,
            zcount_s: ((w2 >> 11) & 0x1FFF) as f64 * 0.6,
            sequence: ((w2 >> 8) & 7) as u8,
            health: (w2 & 7) as u8,
            data: std::mem::take(&mut self.data),
        })
    }
}

/// The RTCM framer (`avnav.rtcm_framer`, LLR bits -> [`RtcmFrame`]). It tries both senses of the bits until a word with the
/// preamble passes its parity, then checks the parity of every word and cuts frames. A word with bad parity drops the lock.
#[derive(Default)]
pub struct RtcmFramer {
    f: Framer,
}

impl RtcmFramer {
    /// The counters.
    pub fn stats(&self) -> FramerStats {
        FramerStats {
            locked: self.f.locked,
            ..self.f.stats
        }
    }
    /// Takes bits directly (without a graph) and appends the finished frames to `out`.
    pub fn push_bits(&mut self, bits: &[bool], out: &mut Vec<RtcmFrame>) {
        for &b in bits {
            if let Some(fr) = self.f.push(b) {
                out.push(fr);
            }
        }
    }
}

impl Component for RtcmFramer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.rtcm_framer",
            vec![Port::llr("in")],
            vec![
                Port::typed::<RtcmFrame>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FramerStats>("stats").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.f = Framer::default();
        let f = &inputs[0];
        Ok(vec![
            Format {
                rate: 0.0,
                ..f.with_kind(Kind::typed::<RtcmFrame>())
            },
            Format {
                rate: 0.0,
                ..f.with_kind(Kind::typed::<FramerStats>())
            },
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let rate = io.input_format(0).rate.max(1.0);
        let bits = io.llrs(0);
        for (i, &b) in bits.iter().enumerate() {
            // Positive means 0.
            if let Some(fr) = self.f.push(b < 0) {
                let time = (first + i as u64 + 1) as f64 / rate;
                io.items_out::<RtcmFrame>(0).push(Timed { time, value: fr });
            }
        }
        if !bits.is_empty() {
            let time = (first + bits.len() as u64) as f64 / rate;
            let s = self.stats();
            io.items_out::<FramerStats>(1)
                .push(Timed { time, value: s });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

impl Item for FramerStats {
    const KIND: &'static str = "rtcm.stats";
}

/// A pseudorange correction (types 1, 2 and 9).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Correction {
    /// Satellite PRN, 1 to 32.
    pub prn: u8,
    /// User differential range error class, 0 (≤ 1 m) to 3 (> 8 m).
    pub udre: u8,
    /// Pseudorange correction, metres.
    pub prc_m: f64,
    /// Range-rate correction, metres a second.
    pub rrc_m_s: f64,
    /// Issue of data.
    pub iod: u8,
}

/// One beacon of a type 7 almanac.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlmanacEntry {
    /// Latitude, degrees.
    pub lat_deg: f64,
    /// Longitude, degrees.
    pub lon_deg: f64,
    /// Range, km.
    pub range_km: u16,
    /// Frequency, kHz.
    pub freq_khz: f64,
    /// Health, 0 (healthy) to 3.
    pub health: u8,
    /// Its station id.
    pub station_id: u16,
    /// Its bit rate, bits a second.
    pub bitrate: u16,
    /// MSK (false) or FSK (true).
    pub fsk: bool,
}

/// A satellite's health (type 5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SatHealth {
    /// PRN.
    pub prn: u8,
    /// The satellite's data health bits.
    pub data_health: u8,
    /// C/N0 at the base station, dB-Hz (`None`: not tracked).
    pub cn0_dbhz: Option<u8>,
    /// The satellite is unhealthy or about to be (loss warning).
    pub warning: bool,
}

/// A decoded message.
#[derive(Clone, Debug, PartialEq)]
pub enum RtcmMessage {
    /// Types 1, 2 and 9.
    Corrections {
        msg_type: u8,
        corrections: Vec<Correction>,
    },
    /// Type 3: the base station's position, ECEF metres.
    BaseStation { x_m: f64, y_m: f64, z_m: f64 },
    /// Type 5.
    ConstellationHealth { satellites: Vec<SatHealth> },
    /// Type 6.
    Null,
    /// Type 7.
    Almanac { beacons: Vec<AlmanacEntry> },
    /// Type 16.
    Text { text: String },
    /// Anything else, as raw data words.
    Other { msg_type: u8, words: Vec<u32> },
}

/// Writes the data bits of `words` (24 each, the first sent first) as bytes into `buf`. Returns the number of bytes used.
fn word_bytes(words: &[u32], buf: &mut [u8; 96]) -> usize {
    let n = words.len().min(32);
    for (i, w) in words[..n].iter().enumerate() {
        buf[3 * i..3 * i + 3].copy_from_slice(&w.to_be_bytes()[1..]);
    }
    3 * n
}

/// The data words (24 bits each) that `write` makes. The last word is filled with alternating ones and zeros (`alternate`) or with
/// zeros.
fn to_words(alternate: bool, write: impl FnOnce(&mut BitWriter)) -> Vec<u32> {
    let mut bytes = Vec::new();
    let mut w = BitWriter::new(&mut bytes);
    write(&mut w);
    let mut k = 0;
    while !w.bits_written().is_multiple_of(24) {
        w.put((alternate && k % 2 == 0) as u64, 1);
        k += 1;
    }
    bytes
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| u32::from_be_bytes([0, c[0], c[1], c[2]]))
        .collect()
}

impl RtcmMessage {
    /// A short text of the message, for the event of a frame.
    pub fn summary(&self) -> String {
        match self {
            RtcmMessage::Corrections { corrections, .. } => {
                format!("{} corrections", corrections.len())
            }
            RtcmMessage::BaseStation { x_m, y_m, z_m } => match base_position(*x_m, *y_m, *z_m) {
                Some(p) => format!("base station {:.5}, {:.5}", p.lat_deg, p.lon_deg),
                None => "base station".to_string(),
            },
            RtcmMessage::ConstellationHealth { satellites } => {
                format!("health of {} satellites", satellites.len())
            }
            RtcmMessage::Null => "null".to_string(),
            RtcmMessage::Almanac { beacons } => format!("almanac of {} beacons", beacons.len()),
            RtcmMessage::Text { text } => format!("text \"{text}\""),
            RtcmMessage::Other { msg_type, .. } => format!("type {msg_type}"),
        }
    }
}

/// The bit rates of the type 7 almanac, by their code.
const ALMANAC_RATES: [u16; 8] = [25, 50, 100, 110, 150, 200, 250, 300];

/// Decodes a frame.
pub fn decode(f: &RtcmFrame) -> RtcmMessage {
    let mut buf = [0u8; 96];
    let n = word_bytes(&f.data, &mut buf);
    let mut r = BitReader::new(&buf[..n]);
    match f.msg_type {
        1 | 2 | 9 => {
            let mut corrections = Vec::new();
            while r.left() >= 40 {
                let scale = r.bit();
                let udre = r.u(2) as u8;
                let sat = r.u(5) as u8;
                let prc = r.i(16);
                let rrc = r.i(8);
                let iod = r.u(8) as u8;
                let (kp, kr) = if scale { (0.32, 0.032) } else { (0.02, 0.002) };
                corrections.push(Correction {
                    prn: if sat == 0 { 32 } else { sat },
                    udre,
                    prc_m: prc as f64 * kp,
                    rrc_m_s: rrc as f64 * kr,
                    iod,
                });
            }
            RtcmMessage::Corrections {
                msg_type: f.msg_type,
                corrections,
            }
        }
        3 if r.left() >= 96 => RtcmMessage::BaseStation {
            x_m: r.i(32) as f64 * 0.01,
            y_m: r.i(32) as f64 * 0.01,
            z_m: r.i(32) as f64 * 0.01,
        },
        5 => {
            let mut satellites = Vec::new();
            while r.left() >= 24 {
                r.skip(1);
                let sat = r.u(5) as u8;
                r.skip(1);
                let data_health = r.u(3) as u8;
                let cn0 = r.u(5) as u8;
                r.skip(2);
                let loss = r.bit();
                r.skip(6);
                satellites.push(SatHealth {
                    prn: if sat == 0 { 32 } else { sat },
                    data_health,
                    cn0_dbhz: (cn0 > 0).then_some(cn0 + 24),
                    warning: loss || data_health != 0,
                });
            }
            RtcmMessage::ConstellationHealth { satellites }
        }
        6 => RtcmMessage::Null,
        7 => {
            let mut beacons = Vec::new();
            while r.left() >= 72 {
                let lat = r.i(16) as f64 * 0.002747;
                let lon = r.i(16) as f64 * 0.005493;
                let range = r.u(10) as u16;
                let freq = r.u(12) as f64 * 0.1 + 190.0;
                let health = r.u(2) as u8;
                let id = r.u(10) as u16;
                let br = ALMANAC_RATES[r.u(3) as usize];
                let fsk = r.bit();
                r.skip(2);
                beacons.push(AlmanacEntry {
                    lat_deg: lat,
                    lon_deg: lon,
                    range_km: range,
                    freq_khz: freq,
                    health,
                    station_id: id,
                    bitrate: br,
                    fsk,
                });
            }
            RtcmMessage::Almanac { beacons }
        }
        16 => {
            let mut text = String::new();
            while r.left() >= 8 {
                let c = r.u(8) as u8;
                if c == 0 {
                    break;
                }
                text.push(if c.is_ascii_graphic() || c == b' ' {
                    c as char
                } else {
                    '?'
                });
            }
            RtcmMessage::Text {
                text: text.trim_end().to_string(),
            }
        }
        t => RtcmMessage::Other {
            msg_type: t,
            words: f.data.clone(),
        },
    }
}

/// The data words of a type 1 or 9 message.
pub fn encode_corrections(c: &[Correction]) -> Vec<u32> {
    to_words(true, |w| {
        for x in c {
            let small = x.prc_m.abs() < 655.0 && x.rrc_m_s.abs() < 0.25;
            let (kp, kr) = if small { (0.02, 0.002) } else { (0.32, 0.032) };
            w.put(!small as u64, 1);
            w.put(x.udre as u64, 2);
            w.put((x.prn % 32) as u64, 5);
            w.put_signed((x.prc_m / kp).round() as i64, 16);
            w.put_signed((x.rrc_m_s / kr).round() as i64, 8);
            w.put(x.iod as u64, 8);
        }
    })
}

/// The data words of a type 3 message: the base station's ECEF position, metres.
pub fn encode_base_station(x: f64, y: f64, z: f64) -> Vec<u32> {
    to_words(true, |w| {
        for v in [x, y, z] {
            w.put_signed((v / 0.01).round() as i64, 32);
        }
    })
}

/// The data words of a type 7 message.
pub fn encode_almanac(b: &[AlmanacEntry]) -> Vec<u32> {
    to_words(true, |w| {
        for a in b {
            w.put_signed((a.lat_deg / 0.002747).round() as i64, 16);
            w.put_signed((a.lon_deg / 0.005493).round() as i64, 16);
            w.put(a.range_km as u64 & 0x3FF, 10);
            w.put((((a.freq_khz - 190.0) / 0.1).round() as u64) & 0xFFF, 12);
            w.put(a.health as u64 & 3, 2);
            w.put(a.station_id as u64 & 0x3FF, 10);
            let code = ALMANAC_RATES
                .iter()
                .position(|&r| r == a.bitrate)
                .unwrap_or(2);
            w.put(code as u64, 3);
            w.put(a.fsk as u64, 1);
            w.put(0, 2);
        }
    })
}

/// The data words of a type 16 message: at most 90 characters, padded with zero bytes (which end the text).
pub fn encode_text(text: &str) -> Vec<u32> {
    to_words(false, |w| {
        for b in text.bytes().take(90) {
            w.put(b as u64, 8);
        }
    })
}

/// The WGS-84 position of a base station given in ECEF metres, or `None` for a point the ellipsoid cannot place.
pub fn base_position(x_m: f64, y_m: f64, z_m: f64) -> Option<Position> {
    Ecef::new(x_m, y_m, z_m).to_position().ok()
}

/// The 30-bit words of a frame, continuing the parity chain from `d29s`, `d30s` (updated).
pub fn frame_words(header: [u32; 2], data: &[u32], d29s: &mut bool, d30s: &mut bool) -> Vec<u32> {
    header
        .iter()
        .chain(data)
        .map(|&d| {
            let w = encode_word(d, *d29s, *d30s);
            *d29s = (w >> 1) & 1 == 1;
            *d30s = w & 1 == 1;
            w
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_round_trips_and_catches_errors() {
        for (d, a, b) in [
            (0x66_1234u32, false, false),
            (0xABCDEF, true, false),
            (0x123456, true, true),
        ] {
            let w = encode_word(d, a, b);
            assert_eq!(decode_word(w, a, b), Some(d));
            for bit in 0..30 {
                assert_eq!(decode_word(w ^ (1 << bit), a, b), None, "bit {bit}");
            }
        }
    }

    #[test]
    fn frames_round_trip_through_the_framer() {
        let corr = vec![
            Correction {
                prn: 5,
                udre: 0,
                prc_m: -12.34,
                rrc_m_s: 0.012,
                iod: 77,
            },
            Correction {
                prn: 32,
                udre: 1,
                prc_m: 3.5,
                rrc_m_s: -0.004,
                iod: 3,
            },
            Correction {
                prn: 17,
                udre: 2,
                prc_m: 1000.0,
                rrc_m_s: 0.5,
                iod: 200,
            },
        ];
        let e = Position {
            lat_deg: -33.86,
            lon_deg: 151.2,
            alt_m: 40.0,
        }
        .to_ecef();
        let (x, y, z) = (e.x_m, e.y_m, e.z_m);
        let msgs: Vec<(u8, Vec<u32>)> = vec![
            (9, encode_corrections(&corr)),
            (3, encode_base_station(x, y, z)),
            (16, encode_text("TEST BEACON")),
            (6, vec![]),
        ];
        let (mut a, mut b) = (false, false);
        // Some bits before, the last two being the D29* and D30* the first word was made with.
        let mut bits = vec![true, false, true, true, false, false];
        for (i, (t, d)) in msgs.iter().enumerate() {
            let h = header_words(*t, 655, 1234.2, i as u8, d.len(), 0);
            for w in frame_words(h, d, &mut a, &mut b) {
                for k in (0..30).rev() {
                    bits.push((w >> k) & 1 == 1);
                }
            }
        }
        // Inverted, to check both senses are found.
        for inv in [false, true] {
            let bb: Vec<bool> = bits.iter().map(|b| b ^ inv).collect();
            let mut f = RtcmFramer::default();
            let mut out = Vec::new();
            f.push_bits(&bb, &mut out);
            assert_eq!(out.len(), 4, "inverted {inv}");
            assert_eq!(out[0].station_id, 655);
            assert!((out[0].zcount_s - 1234.2).abs() < 1e-9);
            match decode(&out[0]) {
                RtcmMessage::Corrections { corrections, .. } => {
                    assert_eq!(corrections.len(), 3);
                    assert_eq!(corrections[0].prn, 5);
                    assert!((corrections[0].prc_m + 12.34).abs() < 0.011);
                    assert_eq!(corrections[1].prn, 32);
                    assert!((corrections[2].prc_m - 1000.0).abs() < 0.2);
                }
                m => panic!("{m:?}"),
            }
            match decode(&out[1]) {
                RtcmMessage::BaseStation { x_m, y_m, z_m } => {
                    let p = base_position(x_m, y_m, z_m).unwrap();
                    assert!(
                        (p.lat_deg + 33.86).abs() < 1e-6
                            && (p.lon_deg - 151.2).abs() < 1e-6
                            && (p.alt_m - 40.0).abs() < 0.05
                    );
                }
                m => panic!("{m:?}"),
            }
            assert_eq!(
                decode(&out[2]),
                RtcmMessage::Text {
                    text: "TEST BEACON".into()
                }
            );
            assert_eq!(decode(&out[3]), RtcmMessage::Null);
        }
    }

    #[test]
    fn almanac_round_trips() {
        let b = AlmanacEntry {
            lat_deg: -33.8,
            lon_deg: 151.2,
            range_km: 300,
            freq_khz: 304.5,
            health: 0,
            station_id: 321,
            bitrate: 200,
            fsk: false,
        };
        let w = encode_almanac(&[b]);
        let f = RtcmFrame {
            msg_type: 7,
            station_id: 1,
            zcount_s: 0.0,
            sequence: 0,
            health: 0,
            data: w,
        };
        match decode(&f) {
            RtcmMessage::Almanac { beacons } => {
                assert_eq!(beacons.len(), 1);
                let a = beacons[0];
                assert!((a.lat_deg + 33.8).abs() < 0.003 && (a.lon_deg - 151.2).abs() < 0.006);
                assert_eq!((a.range_km, a.station_id, a.bitrate), (300, 321, 200));
                assert!((a.freq_khz - 304.5).abs() < 1e-9);
            }
            m => panic!("{m:?}"),
        }
    }
}
