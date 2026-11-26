//! GW FSK: the bursts of a commercial shore-to-ship data service on HF, 2-FSK at 100 Bd with a 200 Hz shift. Free-channel
//! markers with a shore station code, packets with a type and subtype, position reports in fragments, and ACKs.
//!
//! Standard: no issuing body publishes the format. The free-channel marker, the packet layout, the meaning of the packets
//! 2/101 (an MMSI), 5/41 (the opening of a connection), 5/86 (a fragment of an NMEA position report) and 2/106 (the final
//! packet of a ship), and the ACK of 8 bits 10010101 follow the Decoder Musings description of 5 April 2013 and the Wavecom
//! W74PC manual (Gw-fsk).
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [gwfsk: low pass to 8 kHz, the burst detector on 32 ms spectra, per burst: the tone pair,
//!        two matched filters, the bit timing over the whole burst, four readings, the parser] -> rows
//! ```

use crate::receiver::GEN_SECTION;
use crate::senders::{Cx, Layer, Lcg, Playback, RowLog, Sender, StatusLine, Tone};
use crate::typed::{GwPacketEvent, GwfskFields, SenderRow, GW_PACKET, SENDER_ROWS};
use sdrkit_algo_modem::fsk::ToneCorrelator;
use sdrkit_algo_modem::stretchburst::{StretchConfig, StretchDetector};
use sdrkit_algo_modem::toneset::{fit_comb, AveragedSpectrum, CombSpec};
use sdrkit_core::interface::{Fields as _, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, Value, Values,
};
use sdrkit_graph::{values_of, Kept};
use std::sync::Arc;

/// The channel rate.
pub const RATE: f64 = 8000.0;
/// The speed, Bd.
pub const BAUD: f64 = 100.0;
/// The marker pattern after its first byte.
pub const PATTERN: u16 = 0x38A3;
/// The constant bytes of a free-channel marker, before the station code.
pub const MARKER_HEAD: [u8; 11] = [
    0x20, 0x38, 0xA3, 0x1F, 0x6C, 0x35, 0x3B, 0x4B, 0xF2, 0xF2, 0xF2,
];
/// The ACK.
pub const ACK: [u8; 8] = [1, 0, 0, 1, 0, 1, 0, 1];
/// The frame of the burst detector.
const FRAME: usize = 256;

sdrkit_core::choice_enum! {
    /// The sense of the bits.
    pub enum Polarity {
        /// Both readings are tried.
        Auto = ("auto", "Find it"),
        /// The low tone is 1.
        Normal = ("normal", "Low tone is 1"),
        /// The high tone is 1.
        Reversed = ("reversed", "High tone is 1"),
    }
}

/// The sense of the bits.
pub const POLARITY: Setting<field::Choice<Polarity>> =
    Setting::choice("polarity", "Bit sense", Polarity::Auto)
        .help("Find it tries both readings of each burst.");
/// Whether bursts not understood are shown.
pub const SHOW_UNKNOWN: Setting<field::Bool> =
    Setting::new("show_unknown", "Show bursts not understood", "", true);
/// The names of the shore stations.
pub const STATIONS: Setting<field::Text> = Setting::new("stations", "Shore station names", "", "")
    .help("code=name pairs separated by semicolons (hex codes), from the host's data file; a code not listed shows Unknown.");
/// The test signal with the position report.
pub const GEN_POSITION: Setting<field::Bool> = Setting::new(
    "gen_position",
    "Position report",
    "",
    false,
)
.help(
    "The position report, a marker, a connection opening and an ACK instead of the default bursts.",
)
.section(GEN_SECTION)
.advanced();
/// The bursts of the test signal.
pub const GEN_CONTENT: Setting<field::Text> = Setting::new("gen_content", "Bursts", "", DEFAULT_CONTENT)
    .help(
        "Items separated by semicolons: marker HH (station code), text T/S C CHARS, mmsi T/S C DIGITS, packet T/S C HEX (six \
bytes), ack, frame200 HEX (14 bytes at 200 Bd). T/S is type/subtype, C the count bit.",
    )
    .section(GEN_SECTION)
    .advanced();
/// The default bursts of the test signal.
pub const DEFAULT_CONTENT: &str =
    "marker FA; text 2/106 0 AB12E3; mmsi 2/101 1 235012389; marker CC; text 2/106 0 WN,7.<";
/// The silence between the bursts of the test signal.
pub const GEN_GAP_S: Setting<field::Duration> = Setting::new("gen_gap_s", "Gap", "s", 0.6)
    .range(0.1, 10.0, 0.1)
    .section(GEN_SECTION)
    .advanced();
/// The centre of the tones of the test signal.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new("gen_centre_hz", "Centre", "Hz", 1700.0)
    .range(400.0, 3000.0, 1.0)
    .section(GEN_SECTION)
    .advanced();
/// The test signal in the reversed sense.
pub const GEN_REVERSED: Setting<field::Bool> =
    Setting::new("gen_reversed", "Reversed sense", "", false)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the GW FSK receiver.
    pub enum GwfskRxSetting {
        /// Bit sense.
        Polarity = POLARITY,
        /// Unknown bursts.
        ShowUnknown = SHOW_UNKNOWN,
        /// Station names.
        Stations = STATIONS,
    }
}

sdrkit_core::settings! {
    /// The settings of the GW FSK stack.
    pub enum GwfskSetting {
        /// Bit sense.
        Polarity = POLARITY,
        /// Unknown bursts.
        ShowUnknown = SHOW_UNKNOWN,
        /// Station names.
        Stations = STATIONS,
        /// Position report test signal.
        GenPosition = GEN_POSITION,
        /// Gap of the test signal.
        GenGapS = GEN_GAP_S,
        /// Centre of the test signal.
        GenCentreHz = GEN_CENTRE_HZ,
        /// Reversed sense.
        GenReversed = GEN_REVERSED,
        /// Bursts of the test signal.
        GenContent = GEN_CONTENT,
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "No issuing body publishes GW FSK; the format follows a public description from recordings. Not \
confirmed: the bit sense (low tone 1), the packets 5/102, 5/118, 5/54 and 5/63, the character table of the text fields (only \
the filler < is confirmed), the coding of the MMSI digits of 2/101, the shore form of 2/101, the rule of ten fragments of a \
position report, the bits after the six bytes of a packet, the station codes other than DE (the names come from the host). \
The 200 Bd form is read as frames of 14 bytes in the sense of the setting (low tone 1 when it is Find it), with the \
200 Hz shift of the 100 Bd form and the frame starting at the first bit of the burst (both assumed); its CRC is not \
checked, as no document gives the polynomial.";

/// The character of a text byte, or `None`.
pub fn char_of(b: u8) -> Option<char> {
    Some(match b {
        0x60 => '0',
        0x20 => '1',
        0x40 => '2',
        0x00 => '3',
        0x70 => '4',
        0x30 => '5',
        0x50 => '6',
        0x10 => '7',
        0x68 => '8',
        0x28 => '9',
        0x78 | 0x3E | 0x4E | 0xF8 => '<',
        0x2E | 0x7C => ',',
        0x5C => '.',
        0x74 => '$',
        0x4C => '*',
        0x27 => 'A',
        0x47 | 0x3F => 'B',
        0x37 | 0x07 => 'E',
        0x17 => 'G',
        0x7F | 0x63 => 'L',
        0x5F => 'N',
        0x1F | 0x43 => 'O',
        0x13 => 'W',
        _ => return None,
    })
}

/// The first byte of character `c` in the text table.
pub fn byte_of(c: char) -> Option<u8> {
    (0u8..=255).find(|&b| char_of(b) == Some(c))
}

/// The six characters of a text field.
pub fn text_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| char_of(b).map_or_else(|| format!("[0x{b:02x}]"), String::from))
        .collect()
}

/// The digit of a nibble's low three bits in the ship coding, and its alternative.
const SHIP: [(u8, u8); 8] = [
    (3, 3),
    (7, 7),
    (1, 9),
    (5, 5),
    (2, 2),
    (6, 6),
    (0, 8),
    (4, 4),
];
/// The same in the shore coding.
const SHORE: [(u8, u8); 8] = [
    (7, 7),
    (3, 3),
    (5, 5),
    (1, 9),
    (6, 6),
    (2, 2),
    (4, 4),
    (0, 8),
];

/// The digits of `bytes` in the ship coding (`shore` false) or the shore coding: two digits a byte, the low nibble first, up
/// to `count`.
pub fn mmsi_digits(bytes: &[u8], count: usize, shore: bool) -> String {
    let table = if shore { &SHORE } else { &SHIP };
    let mut s = String::new();
    for (i, &b) in bytes.iter().enumerate() {
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        let (lo, hi) = (b & 15, b >> 4);
        let alt_lo = if shore {
            (next >> 4) < 8
        } else {
            i + 1 < bytes.len() && (next >> 4) >= 8
        };
        let alt_hi = if shore { lo < 8 } else { lo >= 8 };
        for (n, alt) in [(lo, alt_lo), (hi, alt_hi)] {
            if s.len() == count {
                return s;
            }
            let (d, a) = table[usize::from(n & 7)];
            s.push((b'0' + if alt { a } else { d }) as char);
        }
    }
    s
}

/// The bytes of `digits` in the ship coding (digits past the ninth padded with 3).
pub fn ship_bytes(digits: &str) -> [u8; 6] {
    let mut d: Vec<u8> = digits.bytes().map(|b| b - b'0').collect();
    while d.len() < 12 {
        d.push(3);
    }
    let nib = |x: u8| -> u8 {
        let base = match x {
            9 => 2,
            8 => 6,
            _ => SHIP.iter().position(|&(a, _)| a == x).unwrap_or(0) as u8,
        };
        base & 7
    };
    let mut out = [0u8; 6];
    for i in 0..6 {
        let (a, b) = (d[2 * i], d[2 * i + 1]);
        let mut lo = nib(a);
        let hi = nib(b);
        // An odd digit 9 or 8 sets bit 3 of the same byte's low nibble.
        if matches!(b, 8 | 9) {
            lo |= 8;
        }
        out[i] = (hi << 4) | lo;
    }
    for i in 0..6 {
        // An even digit 9 or 8 sets bit 3 of the next byte's high nibble.
        if matches!(d[2 * i], 8 | 9) && i + 1 < 6 {
            out[i + 1] |= 0x80;
        }
    }
    out
}

/// A row of the parser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GwRow {
    /// Channel marker, Packet, Position, ACK or Unknown.
    pub kind: &'static str,
    /// The id.
    pub id: String,
    /// The text.
    pub text: String,
    /// The type and subtype of a packet.
    pub types: Option<(u8, u8)>,
}

/// The parser of GW bursts with the position report it assembles.
#[derive(Clone, Debug, Default)]
pub struct GwParser {
    stations: Vec<(u8, String)>,
    /// The last shore station.
    pub last_station: Option<u8>,
    report: Option<(Vec<String>, String)>,
}

fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    bits.as_chunks::<8>()
        .0
        .iter()
        .map(|c| c.iter().fold(0u8, |v, &b| (v << 1) | b))
        .collect()
}

fn field_of(bits: &[u8], a: usize, n: usize) -> u8 {
    bits[a..a + n].iter().fold(0u8, |v, &b| (v << 1) | b)
}

impl GwParser {
    /// A parser that names the stations of `list` (`code=name` pairs separated by semicolons).
    pub fn new(list: &str) -> Self {
        let stations = list
            .split(';')
            .filter_map(|p| {
                let (c, n) = p.split_once('=')?;
                Some((u8::from_str_radix(c.trim(), 16).ok()?, n.trim().to_string()))
            })
            .collect();
        GwParser {
            stations,
            ..Self::default()
        }
    }

    fn name(&self, code: u8) -> &str {
        self.stations
            .iter()
            .find(|(c, _)| *c == code)
            .map_or("Unknown", |(_, n)| n.as_str())
    }

    /// The rows of the bits of a burst: the readings in order (the bits, the bits inverted, both without the first bit, as
    /// `polarity` allows); the first that parses wins.
    pub fn read(&mut self, bits: &[u8], polarity: Polarity, out: &mut Vec<GwRow>) -> bool {
        let inv: Vec<u8> = bits.iter().map(|b| b ^ 1).collect();
        let tries: Vec<&[u8]> = match polarity {
            Polarity::Auto => vec![bits, &inv, &bits[1..], &inv[1..]],
            Polarity::Normal => vec![bits, &bits[1..]],
            Polarity::Reversed => vec![&inv, &inv[1..]],
        };
        tries.iter().any(|t| !t.is_empty() && self.parse(t, out))
    }

    /// The rows of `bits` if they parse.
    pub fn parse(&mut self, bits: &[u8], out: &mut Vec<GwRow>) -> bool {
        let n = bits.len();
        if n >= 144 {
            return self.marker(bits, out);
        }
        if (64..=94).contains(&n) {
            return self.packet(bits, out);
        }
        if (8..=11).contains(&n) && bits[..8] == ACK {
            out.push(GwRow {
                kind: "ACK",
                id: String::new(),
                text: "GW ACK".into(),
                types: None,
            });
            return true;
        }
        false
    }

    fn marker(&mut self, bits: &[u8], out: &mut Vec<GwRow>) -> bool {
        let pat: Vec<u8> = (0..16).rev().map(|i| ((PATTERN >> i) & 1) as u8).collect();
        let Some(at) = bits.windows(16).position(|w| w == pat.as_slice()) else {
            return false;
        };
        let mut b: Vec<u8> = Vec::new();
        if at < 8 {
            b.extend(std::iter::repeat_n(0, 8 - at));
            b.extend(bits);
        } else {
            b.extend(&bits[at - 8..]);
        }
        let bytes = bits_to_bytes(&b);
        if bytes.len() < 18
            || bytes[8..11] != [0xF2; 3]
            || bytes[11..17].iter().any(|&v| v != bytes[11])
            || bytes[17] != 0xFF
        {
            return false;
        }
        let code = bytes[11];
        self.last_station = Some(code);
        let hex: Vec<String> = bytes[..18].iter().map(|v| format!("{v:02x}")).collect();
        out.push(GwRow {
            kind: "Channel marker",
            id: format!("0x{code:02X}"),
            text: format!(
                "free channel marker from station 0x{code:02x} ({}) | {}",
                self.name(code),
                hex.join(" ")
            ),
            types: None,
        });
        true
    }

    fn packet(&mut self, bits: &[u8], out: &mut Vec<GwRow>) -> bool {
        if bits[0] != 1 || bits[1] != 0 {
            return false;
        }
        let ty = field_of(bits, 2, 4);
        let count = bits[6];
        let sub = field_of(bits, 7, 7);
        let bytes = bits_to_bytes(&bits[14..62]);
        let head: String = bits[..14].iter().map(|b| (b'0' + b) as char).collect();
        let list: Vec<String> = bytes.iter().map(|v| format!("0x{v:x}")).collect();
        let mut text = format!(
            "Type={ty} Count={count} Subtype={sub} ({head}) ({})",
            list.join(",")
        );
        let chars = text_of(&bytes);
        let mut position = None;
        let tail = match (ty, sub) {
            (2, 106) => format!("final packet {chars}"),
            (2, 101) if bytes == [0xE2, 0x72, 0xFF, 0xFF, 0xE7, 0xE6] => {
                let d = mmsi_digits(&bytes, 12, true);
                format!("{} | Command={}", &d[..9], &d[9..])
            }
            (2, 101) => format!("MMSI : {}", mmsi_digits(&bytes, 9, false)),
            (5, 41) => "connection opening".to_string(),
            (5, 102) => {
                self.report = Some((Vec::new(), String::new()));
                format!("position report start {chars}")
            }
            (5, 86) => {
                self.fragment(&chars);
                chars.clone()
            }
            (5, 118) | (5, 54) => {
                position = self.end_report(&chars);
                chars.clone()
            }
            (5, 63) => String::new(),
            _ => return false,
        };
        if !tail.is_empty() {
            text.push_str(" | ");
            text.push_str(&tail);
        }
        out.push(GwRow {
            kind: "Packet",
            id: format!("{ty}/{sub}"),
            text,
            types: Some((ty, sub)),
        });
        out.extend(position);
        true
    }

    fn fragment(&mut self, chars: &str) {
        let r = self
            .report
            .get_or_insert_with(|| (Vec::new(), String::new()));
        if chars.starts_with('$') {
            r.0.clear();
        }
        if r.0.last().is_some_and(|l| l == chars) {
            return;
        }
        r.0.push(chars.to_string());
    }

    fn end_report(&mut self, chars: &str) -> Option<GwRow> {
        let (frags, _) = self.report.take()?;
        let text = if frags.len() >= 10 {
            let mut t = frags.concat();
            if frags.last().is_some_and(|l| l != chars) {
                t.push_str(chars);
            }
            t
        } else {
            format!("position report incomplete: {} fragments", frags.len())
        };
        Some(GwRow {
            kind: "Position",
            id: String::new(),
            text,
            types: None,
        })
    }
}

/// The bits of one burst read from its samples at 8000 samples/s: the tone pair, the matched
/// filters, the bit timing over the whole burst. `None` when the tones are not found or the burst is not understood; with the
/// tones, the burst's centre is returned too.
pub fn burst_bits(x: &[Complex32]) -> (Option<f64>, Option<Vec<u8>>) {
    let Ok(mut spec) = AveragedSpectrum::new(RATE, 0.064, 1.0 / 64.0) else {
        return (None, None);
    };
    for &v in x {
        spec.push(v);
    }
    let comb = CombSpec::new(&[-100.0, 100.0], 70.0, -3200.0, 3200.0)
        .with_guards(200.0, 1)
        .with_thresholds(6.0, 1.0)
        .with_min_segments(2)
        .centred();
    let Some(fit) = fit_comb(&mut spec, &comb).filter(|f| f.found) else {
        return (None, None);
    };
    let df = fit.df_hz;
    (Some(df), bits_at(x, df, BAUD))
}

/// The bits of a burst whose tones are `df` -+ 100 Hz, at `baud`: the matched filters of one bit, the bit timing over the
/// whole burst, the bits of its extent. `None` for fewer than 8 bits or a mean decision under 0.55.
pub fn bits_at(x: &[Complex32], df: f64, baud: f64) -> Option<Vec<u8>> {
    let t = RATE / baud;
    let mut corr = ToneCorrelator::new(&[(df - 100.0) / RATE, (df + 100.0) / RATE], t as usize)
        .expect("valid filters");
    let mut d = Vec::with_capacity(x.len());
    let mut e = Vec::with_capacity(x.len());
    for &v in x {
        corr.push(v);
        let mut q = [0f32; 2];
        let _ = corr.energies(&mut q);
        let s = q[0] + q[1];
        d.push(if s > 0.0 { (q[0] - q[1]) / s } else { 0.0 });
        e.push(s);
    }
    let mut sorted = e.clone();
    sorted.sort_by(f32::total_cmp);
    let thr = 0.5 * sorted.get(sorted.len() * 9 / 10).copied().unwrap_or(0.0);
    let mut best = (0.0f64, 0.0f64, t);
    for s in -8..=8 {
        let tq = t * (1.0 + 0.0025 * f64::from(s));
        for q in 0..tq.ceil() as usize {
            let (mut sum, mut n) = (0.0f64, 0usize);
            let mut k = 0.0;
            loop {
                let i = (q as f64 + k * tq).round() as usize;
                if i >= x.len() {
                    break;
                }
                if e[i] > thr {
                    sum += f64::from(d[i].abs());
                    n += 1;
                }
                k += 1.0;
            }
            if n > 0 && sum / n as f64 > best.0 {
                best = (sum / n as f64, q as f64, tq);
            }
        }
    }
    // The extent of the burst: from the first bit over half the threshold (a bit read off its centre keeps more than half
    // its energy) to the last one before two bits in a row under it (one bit dips under by noise, the end of the signal
    // drops by 20 dB or more).
    let (_, q, tq) = best;
    let edge = 0.5 * thr;
    let mut bits = Vec::new();
    let mut quality = 0.0f64;
    let (mut kept, mut kept_q, mut under) = (0usize, 0.0f64, 0);
    let mut k = 0.0;
    loop {
        let i = (q + k * tq).round() as usize;
        k += 1.0;
        if i >= x.len() || under >= 2 {
            break;
        }
        if e[i] <= edge {
            if !bits.is_empty() {
                under += 1;
                bits.push(u8::from(d[i] > 0.0));
                quality += f64::from(d[i].abs());
            }
            continue;
        }
        under = 0;
        bits.push(u8::from(d[i] > 0.0));
        quality += f64::from(d[i].abs());
        kept = bits.len();
        kept_q = quality;
    }
    bits.truncate(kept);
    quality = kept_q;
    if bits.len() < 8 || quality / (bits.len() as f64) < 0.55 {
        return None;
    }
    Some(bits)
}

/// Bits in a frame of the 200 Bd form.
pub const FRAME_200_BITS: usize = 112;

/// A frame of the 200 Bd form read from a burst with tones at `df` -+ 100 Hz: 14 bytes, most significant bit first, the
/// low tone 1 unless `polarity` is reversed. `None` when the burst does not read at 200 Bd as 112 to 127 bits, or when three
/// quarters of the pairs of its bits are equal at one offset (a 100 Bd burst). Its 16-bit CRC is not checked: no document gives the polynomial.
pub fn frame_200(x: &[Complex32], df: f64, polarity: Polarity) -> Option<GwRow> {
    let mut bits = bits_at(x, df, 2.0 * BAUD)?;
    if !(FRAME_200_BITS..FRAME_200_BITS + 16).contains(&bits.len()) {
        return None;
    }
    // A 100 Bd burst read at 200 Bd gives pairs of equal bits at one of the two offsets (a random frame: half of them).
    let paired = |o: usize| {
        let p = bits[o..].as_chunks::<2>().0;
        4 * p.iter().filter(|p| p[0] == p[1]).count() >= 3 * p.len()
    };
    if paired(0) || paired(1) {
        return None;
    }
    if polarity == Polarity::Reversed {
        bits.iter_mut().for_each(|b| *b ^= 1);
    }
    let bytes = bits_to_bytes(&bits[..FRAME_200_BITS]);
    let hex: Vec<String> = bytes.iter().map(|v| format!("{v:02x}")).collect();
    let mut text = format!("14 bytes: {}", hex.join(" "));
    if bits.len() > FRAME_200_BITS {
        text.push_str(&format!(" | {} more bits", bits.len() - FRAME_200_BITS));
    }
    text.push_str(" | CRC not checked (its polynomial is not published)");
    Some(GwRow {
        kind: "Frame",
        id: "200 Bd".into(),
        text,
        types: None,
    })
}

/// The burst detector: a stretch of 310 Hz (both tones) within 0.42 of the rate, two frames over 5.5 times the noise open a
/// burst (noise alone about once in 2.5e8 frames) and three under 1.6 close it; bursts from 0.07 s to 12 s.
fn detector() -> StretchDetector {
    let mut c = StretchConfig::new(RATE, FRAME, vec![(-0.42 * RATE, 0.42 * RATE)], 310.0);
    c.min_len = (0.07 * RATE) as usize;
    c.max_len = (12.0 * RATE) as usize;
    StretchDetector::new(c).expect("valid detector")
}

/// A row as kept.
#[derive(Clone, Debug)]
struct Row {
    time: f64,
    kind: &'static str,
    id: String,
    text: String,
}

/// The GW FSK layer.
pub struct Gwfsk {
    polarity: Polarity,
    show_unknown: bool,
    stations: String,
    detector: StretchDetector,
    parser: GwParser,
    rows: RowLog<Row>,
    bursts: i64,
    not_understood: i64,
    packets: i64,
    markers: i64,
    last_tones: Option<f64>,
}

impl Default for Gwfsk {
    fn default() -> Self {
        Self::new()
    }
}

impl Gwfsk {
    /// A layer at the default settings.
    pub fn new() -> Self {
        Gwfsk {
            polarity: Polarity::Auto,
            show_unknown: true,
            stations: String::new(),
            detector: detector(),
            parser: GwParser::new(""),
            rows: RowLog::default(),
            bursts: 0,
            not_understood: 0,
            packets: 0,
            markers: 0,
            last_tones: None,
        }
    }

    fn burst(&mut self, x: &[Complex32], cx: &mut Cx) {
        self.bursts += 1;
        let (df, bits) = burst_bits(x);
        if df.is_some() {
            self.last_tones = df;
        }
        let mut out = Vec::new();
        let read = bits
            .as_ref()
            .is_some_and(|b| self.parser.read(b, self.polarity, &mut out));
        if !read {
            if let Some(f) = df.and_then(|d| frame_200(x, d, self.polarity)) {
                out.push(f);
            } else if df.is_some() || bits.is_some() {
                self.not_understood += 1;
                if let Some(bits) = bits.as_ref().filter(|_| self.show_unknown) {
                    let mut s: String = bits.iter().take(200).map(|b| (b'0' + b) as char).collect();
                    if bits.len() > 200 {
                        s.push_str("...");
                    }
                    out.push(GwRow {
                        kind: "Unknown",
                        id: format!("{} bits", bits.len()),
                        text: s,
                        types: None,
                    });
                }
            }
        }
        for r in out {
            match r.kind {
                "Packet" | "Frame" => self.packets += 1,
                "Channel marker" => self.markers += 1,
                _ => {}
            }
            if r.kind != "Unknown" {
                cx.events.push(GW_PACKET.event(
                    cx.time,
                    cx.dial,
                    &GwPacketEvent {
                        text: &r.text,
                        kind: r.kind,
                        packet_type: r.types.map(|t| i64::from(t.0)),
                        subtype: r.types.map(|t| i64::from(t.1)),
                    },
                ));
            }
            self.rows.push(Row {
                time: cx.time,
                kind: r.kind,
                id: r.id,
                text: r.text,
            });
        }
    }
}

impl Layer for Gwfsk {
    const NAME: &'static str = "hfdata.gwfsk";
    const DECIM: usize = 3;
    const PASS_HZ: f64 = 3400.0;
    fn schema() -> Arc<Schema> {
        GwfskRxSetting::schema()
    }
    fn declaration() -> ResultsDecl {
        ResultsDecl::new()
            .with_fields(GwfskFields::SHAPE.fields)
            .with_table(&SENDER_ROWS)
            .event(&GW_PACKET)
    }
    fn values(&self) -> Values {
        values_of([
            (
                GwfskRxSetting::Polarity,
                Value::Choice(self.polarity.index()),
            ),
            (GwfskRxSetting::ShowUnknown, Value::Bool(self.show_unknown)),
            (GwfskRxSetting::Stations, Value::Text(self.stations.clone())),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match GwfskRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            GwfskRxSetting::Polarity => self.polarity = POLARITY.read(id, value)?,
            GwfskRxSetting::ShowUnknown => self.show_unknown = SHOW_UNKNOWN.read(id, value)?,
            GwfskRxSetting::Stations => {
                self.stations = STATIONS.read(id, value)?.to_string();
                let last = self.parser.last_station;
                self.parser = GwParser::new(&self.stations);
                self.parser.last_station = last;
            }
        }
        Ok(())
    }
    fn push(&mut self, x: &[Complex32], cx: &mut Cx) {
        let mut at = 0;
        while at < x.len() {
            at += self.detector.process(&x[at..]);
            if let Some(b) = self.detector.take() {
                let b = b.samples.to_vec();
                self.burst(&b, cx);
            }
        }
    }
    fn finish(&mut self, cx: &mut Cx) {
        self.detector.end();
        if let Some(b) = self.detector.take() {
            let b = b.samples.to_vec();
            self.burst(&b, cx);
        }
    }
    fn reset(&mut self) {
        self.detector.reset();
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.bursts = 0;
        self.not_understood = 0;
        self.packets = 0;
        self.markers = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        self.last_tones = None;
        if !kept.table(SENDER_ROWS.key) {
            self.rows.clear();
        }
        self.bursts = 0;
        self.not_understood = 0;
        self.packets = 0;
        self.markers = 0;
    }
    fn report(&mut self, res: &mut Published, level_dbfs: f64) -> StatusLine {
        self.rows.report(
            res,
            |r| r.clear_rows(&SENDER_ROWS),
            |r, key, row| {
                SENDER_ROWS.put(
                    r,
                    key,
                    &SenderRow {
                        time: Time(row.time),
                        kind: row.kind,
                        id: &row.id,
                        text: &row.text,
                        snr: None,
                    },
                );
            },
        );
        let tones = self
            .last_tones
            .map(|d| format!("{:.0} / {:.0} Hz from the dial", d - 100.0, d + 100.0));
        let station = self.parser.last_station.map(|c| format!("0x{c:02X}"));
        let in_burst = self.detector.is_open();
        let state: u8 = if self.packets + self.markers > 0 {
            2
        } else if in_burst || self.bursts > 0 {
            1
        } else {
            0
        };
        res.put(&GwfskFields {
            note: NOTE_TEXT,
            state: i64::from(state),
            tones: tones.as_deref(),
            last_station: station.as_deref(),
            bursts: self.bursts,
            not_understood: self.not_understood,
            packets: self.packets,
            markers: self.markers,
            level_dbfs,
        });
        StatusLine {
            state,
            summary: format!(
                "GW FSK: {} packets, {} channel markers",
                self.packets, self.markers
            ),
            snr_db: None,
            offset_hz: None,
        }
    }
}

/// The schema of the stack.
pub fn schema() -> Arc<Schema> {
    GwfskSetting::schema()
}

// ---- The test signal ----

/// The bits of a free-channel marker of `station`: 16 bits of reversals, the 18 bytes, 16 bits of reversals.
pub fn marker_bits(station: u8) -> Vec<u8> {
    let mut bytes = MARKER_HEAD.to_vec();
    bytes.extend([station; 6]);
    bytes.push(0xFF);
    let mut b: Vec<u8> = (0..16).map(|i| ((i + 1) % 2) as u8).collect();
    b.extend(
        bytes
            .iter()
            .flat_map(|&v| (0..8).rev().map(move |i| (v >> i) & 1)),
    );
    b.extend((0..16).map(|i| ((i + 1) % 2) as u8));
    b
}

/// The bits of a packet: 1 0, the type, the count bit, the subtype, the six bytes, and 8 more bits 11001010.
pub fn packet_bits(ty: u8, count: u8, sub: u8, bytes: &[u8; 6]) -> Vec<u8> {
    let mut b = vec![1, 0];
    b.extend((0..4).rev().map(|i| (ty >> i) & 1));
    b.push(count & 1);
    b.extend((0..7).rev().map(|i| (sub >> i) & 1));
    b.extend(
        bytes
            .iter()
            .flat_map(|&v| (0..8).rev().map(move |i| (v >> i) & 1)),
    );
    b.extend([1, 1, 0, 0, 1, 0, 1, 0]);
    b
}

/// The six bytes of six text characters.
pub fn text_bytes(s: &str) -> [u8; 6] {
    let mut out = [0u8; 6];
    for (o, c) in out.iter_mut().zip(s.chars()) {
        *o = byte_of(c).unwrap_or(0x78);
    }
    out
}

/// The fragments of the position report of the test signal.
pub const POSITION: [&str; 10] = [
    "234.56", ",N,012", "34.56,", "E,1234", "56,A*4", "B<<<<<", "123456", "7890,.", "**<<..",
    "AABBEE",
];

/// The bursts of the test signal.
pub fn test_bursts(position: bool) -> Vec<Vec<u8>> {
    if position {
        let mut v = vec![packet_bits(5, 0, 102, &text_bytes("$GLL,1"))];
        for (i, f) in POSITION.iter().enumerate() {
            v.push(packet_bits(5, (i % 2) as u8, 86, &text_bytes(f)));
        }
        v.push(packet_bits(5, 0, 118, &text_bytes("<<<<<<")));
        v.push(marker_bits(0x9C));
        v.push(packet_bits(5, 1, 41, &ship_bytes("538001234567")));
        v.push(ACK.to_vec());
        v
    } else {
        content_bursts(DEFAULT_CONTENT)
            .expect("the default content parses")
            .into_iter()
            .map(|b| b.1)
            .collect()
    }
}

/// The bursts of a `gen_content` text (the help of [`GEN_CONTENT`]): the speed and the bits of each. `Err` gives the number of
/// the first item that does not parse.
pub fn content_bursts(text: &str) -> Result<Vec<(f64, Vec<u8>)>, usize> {
    let hex = |h: &str, n: usize| -> Option<Vec<u8>> {
        (h.len() == 2 * n && h.is_ascii()).then(|| {
            (0..n)
                .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok())
                .collect()
        })?
    };
    let head = |ts: &str, c: &str| -> Option<(u8, u8, u8)> {
        let (t, s) = ts.split_once('/')?;
        let (t, s, c): (u8, u8, u8) = (t.parse().ok()?, s.parse().ok()?, c.parse().ok()?);
        (t < 16 && s < 128 && c < 2).then_some((t, s, c))
    };
    text.split(';')
        .map(str::trim)
        .filter(|i| !i.is_empty())
        .enumerate()
        .map(|(k, item)| {
            let w: Vec<&str> = item.split_whitespace().collect();
            let burst = match w.as_slice() {
                ["marker", h] => hex(h, 1).map(|b| (BAUD, marker_bits(b[0]))),
                ["ack"] => Some((BAUD, ACK.to_vec())),
                ["frame200", h] => hex(h, 14).map(|b| {
                    let bits = b
                        .iter()
                        .flat_map(|v| (0..8).rev().map(move |i| (v >> i) & 1))
                        .collect();
                    (2.0 * BAUD, bits)
                }),
                [kind, ts, c, body] => head(ts, c).and_then(|(t, s, c)| {
                    let bytes: [u8; 6] = match *kind {
                        "text"
                            if body.chars().count() == 6
                                && body.chars().all(|ch| byte_of(ch).is_some()) =>
                        {
                            text_bytes(body)
                        }
                        "mmsi" if body.len() <= 12 && body.bytes().all(|b| b.is_ascii_digit()) => {
                            ship_bytes(body)
                        }
                        "packet" => hex(body, 6)?.try_into().ok()?,
                        _ => return None,
                    };
                    Some((BAUD, packet_bits(t, c, s, &bytes)))
                }),
                _ => None,
            };
            burst.ok_or(k)
        })
        .collect()
}

/// The tone sequence of `bursts` (the speed and the bits of each; the low tone for 1), `gap_s` of silence before each and
/// after the last.
pub fn test_sequence(
    bursts: &[(f64, Vec<u8>)],
    centre: f64,
    reversed: bool,
    gap_s: f64,
) -> Vec<Tone> {
    let mut seq = Vec::new();
    for (baud, b) in bursts {
        let unit = 1.0 / baud;
        seq.push(Tone::silence(gap_s));
        for &bit in b {
            let low = (bit == 1) != reversed;
            seq.push(Tone::new(
                unit,
                if low { centre - 100.0 } else { centre + 100.0 },
            ));
        }
    }
    seq.push(Tone::silence(gap_s));
    seq
}

/// Random six-character text packets 2/106 of the characters 0 to 9, A and B, from `seed`, and their texts.
pub fn random_packets(n: usize, seed: u64) -> Vec<(Vec<u8>, String)> {
    let set: Vec<char> = "0123456789AB".chars().collect();
    let mut r = Lcg(seed ^ 0x6F);
    (0..n)
        .map(|_| {
            let s: String = (0..6).map(|_| set[r.below(12) as usize]).collect();
            (packet_bits(2, 0, 106, &text_bytes(&s)), s)
        })
        .collect()
}

/// The test signal.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let bursts = if s.get(&GEN_POSITION) {
        test_bursts(true).into_iter().map(|b| (BAUD, b)).collect()
    } else {
        content_bursts(s.get(&GEN_CONTENT)).map_err(|at| {
            ConfigError::Setting(SettingError::Malformed {
                id: schema().id(GEN_CONTENT.field.key).unwrap_or(SettingId(0)),
                at: Some(at),
            })
        })?
    };
    let seq = test_sequence(
        &bursts,
        s.get(&GEN_CENTRE_HZ),
        s.get(&GEN_REVERSED),
        s.get(&GEN_GAP_S),
    );
    Ok(Box::new(Playback::new(
        &seq,
        cfg.rate_hz,
        SignalInfo {
            symbol_rate_hz: Some(BAUD),
            ..SignalInfo::new(3000.0)
        },
    )))
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("gwfsk"),
        "GW FSK",
        Sender::new(Gwfsk::new()),
        cfg,
    )
}

/// The dial of the test signal. It is not a station frequency: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 6.5e6;

/// GW FSK.
pub static GWFSK: StackDescriptor = StackDescriptor {
    id: "gwfsk",
    name: "GW FSK",
    group: "HF utility",
    blurb: "100 Bd 2-FSK bursts of a shore-to-ship data service: free-channel markers with the shore station code, packets with type and subtype, MMSI numbers, position reports, ACKs.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..crate::ms110::hf_tuning(TEST_SIGNAL_HZ)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::GWFSK),
    benchmarks: Some(crate::benchmarks),
};
