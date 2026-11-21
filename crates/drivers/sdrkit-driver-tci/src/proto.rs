//! The text commands and binary stream packets of TCI, as pure functions.
//!
//! Standard: Expert Electronics TCI Protocol, versions 1.4 to 2.0.

use sdrkit_core::Hz;
use std::time::Instant;

/// The TCI port when the address names none.
pub const DEFAULT_PORT: u16 = 50001;
/// The IQ rates the stream offers, slowest first.
pub const IQ_RATES: [u32; 4] = [48_000, 96_000, 192_000, 384_000];
/// The receive audio rates. The driver does not use the receive audio stream.
pub const AUDIO_RATES: [u32; 4] = [8_000, 12_000, 24_000, 48_000];
/// The packet header size.
pub const HEADER: usize = 64;
/// The most receivers a server is taken to have.
pub const MAX_RECEIVERS: usize = 8;
/// The longest command name kept.
pub const MAX_NAME: usize = 64;
/// The most arguments a command may have.
pub const MAX_ARGS: usize = 64;

/// Packet types (header word 6).
pub mod kind {
    /// IQ.
    pub const IQ: u32 = 0;
    /// Receive audio.
    pub const RX_AUDIO: u32 = 1;
    /// Transmit audio.
    pub const TX_AUDIO: u32 = 2;
    /// Transmit timing ("chrono").
    pub const TX_CHRONO: u32 = 3;
    /// Line out.
    pub const LINE_OUT: u32 = 4;
}

/// Sample formats (header word 2).
pub mod format {
    /// Signed 16-bit.
    pub const INT16: u32 = 0;
    /// Signed 24-bit.
    pub const INT24: u32 = 1;
    /// Signed 32-bit.
    pub const INT32: u32 = 2;
    /// IEEE 754 single.
    pub const FLOAT32: u32 = 3;
    /// A name for display.
    pub fn name(f: u32) -> &'static str {
        match f {
            INT16 => "int16",
            INT24 => "int24",
            INT32 => "int32",
            FLOAT32 => "float32",
            _ => "unknown",
        }
    }
}

// ---- Text commands ----

/// One parsed command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command<'a> {
    /// The name, lower case.
    pub name: String,
    /// The arguments, trimmed.
    pub args: Vec<&'a str>,
}

impl Command<'_> {
    /// Argument `i`, or "".
    pub fn arg(&self, i: usize) -> &str {
        self.args.get(i).copied().unwrap_or("")
    }
    /// Argument `i` as a number.
    pub fn num(&self, i: usize) -> Option<f64> {
        self.args
            .get(i)?
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    }
    /// Argument `i` as a receiver number below [`MAX_RECEIVERS`].
    pub fn rx(&self, i: usize) -> Option<usize> {
        self.args
            .get(i)?
            .parse::<usize>()
            .ok()
            .filter(|r| *r < MAX_RECEIVERS)
    }
    /// Argument `i` as a boolean (`true`, case-insensitive; anything else is false).
    pub fn flag(&self, i: usize) -> Option<bool> {
        self.args.get(i).map(|a| a.eq_ignore_ascii_case("true"))
    }
}

const TRIM: [char; 4] = [' ', '\t', '\r', '\n'];

/// Splits a text message into commands: text after the last `;` is dropped, empty commands are skipped, a command with an
/// empty or over-long name or too many arguments is dropped.
pub fn parse(text: &str) -> Vec<Command<'_>> {
    let mut out = Vec::new();
    let Some(end) = text.rfind(';') else {
        return out;
    };
    for raw in text[..end].split(';') {
        let c = raw.trim_matches(TRIM);
        if c.is_empty() {
            continue;
        }
        let (name, rest) = match c.split_once(':') {
            Some((n, r)) => (n.trim_matches(TRIM), Some(r)),
            None => (c, None),
        };
        if name.is_empty() || name.len() > MAX_NAME {
            continue;
        }
        let args: Vec<&str> = match rest {
            Some(r) => r.split(',').map(|a| a.trim_matches(TRIM)).collect(),
            None => Vec::new(),
        };
        if args.len() > MAX_ARGS {
            continue;
        }
        out.push(Command {
            name: name.to_ascii_lowercase(),
            args,
        });
    }
    out
}

// ---- What a server said ----

/// What a server said about itself, remembered per address.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerInfo {
    /// The server program (`protocol`, first argument).
    pub program: Option<String>,
    /// Its TCI version.
    pub version: Option<String>,
    /// The radio's name.
    pub device: Option<String>,
    /// The radio cannot transmit; None when the server said nothing.
    pub receive_only: Option<bool>,
    /// Receivers, 1 to 8; None when not said.
    pub trx_count: Option<u32>,
    /// VFO channels per receiver.
    pub channels_count: Option<u32>,
    /// Tuning range (low, high), stored when high > 0.
    pub vfo_limits: Option<(Hz, Hz)>,
    /// IF offset range.
    pub if_limits: Option<(Hz, Hz)>,
    /// The server's demodulators.
    pub modulations: Vec<String>,
    /// The IQ rate the server reports.
    pub iq_rate: Option<Hz>,
    /// The receive audio rate.
    pub audio_rate: Option<Hz>,
    /// `ready` arrived.
    pub ready: bool,
    /// The server's radio runs (`start`) or is stopped (`stop`).
    pub running: Option<bool>,
    /// Per receiver: the panorama centre last reported.
    pub dds: [Option<Hz>; MAX_RECEIVERS],
    /// Per receiver, channel 0: the last level in dBm and when it came.
    pub smeter: [Option<(f64, Instant)>; MAX_RECEIVERS],
    /// Per receiver: may transmit now.
    pub tx_enable: [Option<bool>; MAX_RECEIVERS],
    /// Per receiver: transmitting.
    pub trx: [Option<bool>; MAX_RECEIVERS],
    /// A handshake succeeded at least once.
    pub seen: bool,
}

impl ServerInfo {
    /// The receiver count (1 when unknown).
    pub fn receivers(&self) -> usize {
        self.trx_count.unwrap_or(1).clamp(1, MAX_RECEIVERS as u32) as usize
    }
    /// The server said it is receive only.
    pub fn is_receive_only(&self) -> bool {
        self.receive_only == Some(true)
    }
    /// Stores what one command says. Returns true when it was one of the commands of 2.4.
    pub fn apply(&mut self, c: &Command<'_>) -> bool {
        match c.name.as_str() {
            "protocol" => {
                self.program = Some(c.arg(0).to_string());
                self.version = Some(c.arg(1).to_string());
            }
            "device" => self.device = Some(c.arg(0).to_string()),
            "receive_only" => self.receive_only = c.flag(0),
            "trx_count" => {
                if let Some(n) = c.args.first().and_then(|a| a.parse::<i64>().ok()) {
                    self.trx_count = Some(n.clamp(1, MAX_RECEIVERS as i64) as u32);
                }
            }
            "channels_count" => {
                if let Some(n) = c.args.first().and_then(|a| a.parse::<u32>().ok()) {
                    self.channels_count = Some(n);
                }
            }
            "vfo_limits" => {
                if let (Some(lo), Some(hi)) = (c.num(0), c.num(1)) {
                    if hi > 0.0 {
                        self.vfo_limits = Some((lo, hi));
                    }
                }
            }
            "if_limits" => {
                if let (Some(lo), Some(hi)) = (c.num(0), c.num(1)) {
                    self.if_limits = Some((lo, hi));
                }
            }
            "modulations_list" => {
                self.modulations = c
                    .args
                    .iter()
                    .filter(|a| !a.is_empty())
                    .map(|a| a.to_string())
                    .collect();
            }
            "iq_samplerate" => {
                if let Some(r) = c.num(0).filter(|r| *r > 0.0 && *r < 1e8) {
                    self.iq_rate = Some(r);
                }
            }
            "audio_samplerate" => {
                if let Some(r) = c.num(0).filter(|r| *r > 0.0 && *r < 1e8) {
                    self.audio_rate = Some(r);
                }
            }
            "ready" => self.ready = true,
            "start" => self.running = Some(true),
            "stop" => self.running = Some(false),
            "dds" => {
                if let (Some(rx), Some(f)) = (c.rx(0), c.num(1)) {
                    self.dds[rx] = Some(f);
                }
            }
            "rx_smeter" | "rx_channel_sensors" => {
                if let (Some(rx), Some(ch), Some(l)) = (c.rx(0), c.num(1), c.num(2)) {
                    if ch == 0.0 {
                        self.smeter[rx] = Some((l, Instant::now()));
                    }
                }
            }
            "tx_enable" => {
                if let Some(rx) = c.rx(0) {
                    self.tx_enable[rx] = c.flag(1);
                }
            }
            "trx" => {
                if let Some(rx) = c.rx(0) {
                    self.trx[rx] = c.flag(1);
                }
            }
            _ => return false,
        }
        true
    }
}

// ---- Rate and frequency ----

/// The IQ rate chosen for a tune rate: the smallest offered rate at or above (rate - 1), else 384 000.
pub fn choose_rate(tune_rate: Hz) -> u32 {
    let r = if tune_rate.is_finite() {
        tune_rate.max(0.0)
    } else {
        0.0
    };
    IQ_RATES
        .iter()
        .copied()
        .find(|&o| o as Hz >= r - 1.0)
        .unwrap_or(IQ_RATES[3])
}

/// The frequency sent: whole Hz, never below 0.
pub fn freq_sent(centre: Hz) -> u64 {
    if centre.is_finite() {
        centre.max(0.0).round() as u64
    } else {
        0
    }
}

/// A frequency in MHz with up to 6 significant digits ("0.01", "30", "7.07412").
pub fn mhz(hz: Hz) -> String {
    let m = hz / 1e6;
    let int_digits = if m.abs() >= 1.0 {
        (m.abs().log10().floor() as i32) + 1
    } else {
        1
    };
    let dec = (6 - int_digits).max(0) as usize;
    let s = format!("{m:.dec$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        &s
    };
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

// ---- Binary packets ----

/// The 64-byte packet header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// The receiver.
    pub receiver: u32,
    /// The stream's rate.
    pub sample_rate: u32,
    /// The sample type.
    pub format: u32,
    /// The codec.
    pub codec: u32,
    /// The crc.
    pub crc: u32,
    /// Sample values (real numbers) in the data.
    pub length: u32,
    /// The stream type.
    pub kind: u32,
    /// Channels (2 for IQ; 0 from older servers).
    pub channels: u32,
}

impl Header {
    /// Reads the first 64 bytes of `b` (which must hold them).
    pub fn parse(b: &[u8]) -> Header {
        let w = |i: usize| u32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        Header {
            receiver: w(0),
            sample_rate: w(1),
            format: w(2),
            codec: w(3),
            crc: w(4),
            length: w(5),
            kind: w(6),
            channels: w(7),
        }
    }
    /// Writes the header (reserved words 0) into the first 64 bytes of `out`.
    pub fn write(&self, out: &mut [u8]) {
        let words = [
            self.receiver,
            self.sample_rate,
            self.format,
            self.codec,
            self.crc,
            self.length,
            self.kind,
            self.channels,
        ];
        for (i, w) in words.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        out[32..HEADER].fill(0);
    }
}

/// Bytes per sample value for a format, or None for an unknown one.
pub fn bytes_per_value(format: u32) -> Option<usize> {
    match format {
        format::INT16 => Some(2),
        format::INT24 => Some(3),
        format::INT32 | format::FLOAT32 => Some(4),
        _ => None,
    }
}

/// What 2.8 decided about one binary message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Dropped and counted.
    Unusable,
    /// Not IQ of this receiver: dropped silently.
    Skipped,
    /// IQ to use: `n` complex samples of `format`, from byte 64.
    Use {
        /// The header.
        header: Header,
        /// Complex samples.
        n: usize,
    },
}

/// Steps 1 to 6 of 2.8 for a message of this driver's receiver `rx`.
pub fn check(msg: &[u8], rx: u32) -> Verdict {
    if msg.len() < HEADER {
        return Verdict::Unusable;
    }
    let h = Header::parse(msg);
    if h.kind != kind::IQ || h.receiver != rx {
        return Verdict::Skipped;
    }
    if !(h.channels == 2 || h.channels == 0) {
        return Verdict::Unusable;
    }
    let Some(b) = bytes_per_value(h.format) else {
        return Verdict::Unusable;
    };
    let p = msg.len() - HEADER;
    let n = p / (2 * b);
    if n == 0 || !p.is_multiple_of(2 * b) {
        return Verdict::Unusable;
    }
    let l = h.length as usize;
    if l != 0 && l * b != p && l * b * 2 != p {
        return Verdict::Unusable;
    }
    Verdict::Use { header: h, n }
}

/// A packet's rate is followed: at least 8000, below 1e8, and more than 0.5 sps from `rate`.
pub fn follows_rate(sample_rate: u32, rate: Hz) -> bool {
    let r = sample_rate as Hz;
    (8000.0..1e8).contains(&r) && (r - rate).abs() > 0.5
}

/// Builds a transmit audio packet into `out` (cleared first): a header for receiver `rx`, 48 000 sps, float32, `length`
/// values, type 2, 2 channels, then the stereo frames, each value taken from `next` (or 0 when it has none).
pub fn tx_audio_packet(
    out: &mut Vec<u8>,
    rx: u32,
    length: u32,
    mut next: impl FnMut() -> Option<f32>,
) -> usize {
    out.clear();
    out.resize(HEADER, 0);
    Header {
        receiver: rx,
        sample_rate: 48_000,
        format: format::FLOAT32,
        length,
        kind: kind::TX_AUDIO,
        channels: 2,
        ..Default::default()
    }
    .write(out);
    let mut short = 0;
    for _ in 0..length / 2 {
        let v = match next() {
            Some(v) => v.clamp(-1.0, 1.0),
            None => {
                short += 1;
                0.0
            }
        };
        out.extend_from_slice(&v.to_le_bytes());
        out.extend_from_slice(&v.to_le_bytes());
    }
    if length % 2 == 1 {
        out.extend_from_slice(&0f32.to_le_bytes());
    }
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parser_follows_the_rules() {
        let c = parse("DDS:0,7100000;vfo:0,1,7100000;ready;");
        assert_eq!(c.len(), 3);
        assert_eq!(
            (c[0].name.as_str(), c[0].args.clone()),
            ("dds", vec!["0", "7100000"])
        );
        assert_eq!(
            (c[1].name.as_str(), c[1].args.clone()),
            ("vfo", vec!["0", "1", "7100000"])
        );
        assert_eq!((c[2].name.as_str(), c[2].args.len()), ("ready", 0));
        let c = parse(" dds : 0 , 7100000 ;");
        assert_eq!(
            (c[0].name.as_str(), c[0].args.clone()),
            ("dds", vec!["0", "7100000"])
        );
        assert!(parse("no semicolon").is_empty());
        assert!(parse("").is_empty());
        assert!(parse(";;;").is_empty());
        let long = format!("{};a:1;", "x".repeat(5000));
        let c = parse(&long);
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].name.as_str(), c[0].args.clone()), ("a", vec!["1"]));
        let c = parse("a:1;b");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].name, "a");
        let many = format!("m:{};", vec!["1"; 65].join(","));
        assert!(parse(&many).is_empty());
        let ok = format!("m:{};", vec!["1"; 64].join(","));
        assert_eq!(parse(&ok).len(), 1);
    }

    #[test]
    fn server_information_is_stored_with_limits() {
        let mut i = ServerInfo::default();
        for c in parse("receive_only:TRUE;trx_count:0;iq_samplerate:-5;") {
            i.apply(&c);
        }
        assert!(i.is_receive_only());
        assert_eq!(i.receivers(), 1);
        assert_eq!(i.iq_rate, None);
        for c in parse(
            "trx_count:20;vfo_limits:0,0;iq_samplerate:96000;rx_smeter:1,0,-73;rx_smeter:1,1,-20;",
        ) {
            i.apply(&c);
        }
        assert_eq!(i.receivers(), 8);
        assert_eq!(i.vfo_limits, None);
        assert_eq!(i.iq_rate, Some(96_000.0));
        assert_eq!(i.smeter[1].map(|s| s.0), Some(-73.0));
        assert!(!i.apply(&parse("volume:-10;")[0]));
    }

    #[test]
    fn rates_follow_the_examples() {
        assert_eq!(choose_rate(100_000.0), 192_000);
        assert_eq!(choose_rate(48_000.0), 48_000);
        assert_eq!(choose_rate(48_001.0), 48_000);
        assert_eq!(choose_rate(48_002.0), 96_000);
        assert_eq!(choose_rate(0.0), 48_000);
        assert_eq!(choose_rate(-3.0), 48_000);
        assert_eq!(choose_rate(384_000.0), 384_000);
        assert_eq!(choose_rate(1e6), 384_000);
        assert_eq!(freq_sent(-5.0), 0);
        assert_eq!(freq_sent(7_100_000.4), 7_100_000);
        assert_eq!(mhz(10_000.0), "0.01");
        assert_eq!(mhz(30e6), "30");
        assert_eq!(mhz(50e6), "50");
        assert_eq!(mhz(7_074_120.0), "7.07412");
        assert_eq!(mhz(1_234_567_890.0), "1234.57");
    }

    #[test]
    fn packets_are_checked_in_order() {
        let mut p = vec![0u8; HEADER + 16];
        Header {
            sample_rate: 48_000,
            format: 3,
            length: 4,
            channels: 2,
            ..Default::default()
        }
        .write(&mut p);
        assert!(matches!(check(&p, 0), Verdict::Use { n: 2, .. }));
        assert_eq!(check(&p, 1), Verdict::Skipped);
        assert_eq!(check(&p[..10], 0), Verdict::Unusable);
        p[20] = 2; // length as complex samples
        assert!(matches!(check(&p, 0), Verdict::Use { .. }));
        p[20] = 3;
        assert_eq!(check(&p, 0), Verdict::Unusable);
        p[20] = 0;
        assert!(matches!(check(&p, 0), Verdict::Use { .. }));
        p[28] = 0; // channels 0
        assert!(matches!(check(&p, 0), Verdict::Use { .. }));
        p[28] = 1;
        assert_eq!(check(&p, 0), Verdict::Unusable);
        p[28] = 2;
        p[8] = 9;
        assert_eq!(check(&p, 0), Verdict::Unusable);
        p[8] = 3;
        p.push(0);
        assert_eq!(check(&p, 0), Verdict::Unusable);
        assert!(!follows_rate(7999, 48_000.0));
        assert!(!follows_rate(0, 48_000.0));
        assert!(!follows_rate(48_000, 48_000.0));
        assert!(follows_rate(96_000, 48_000.0));
    }
}
