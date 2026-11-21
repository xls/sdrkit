//! The KiwiSDR protocol as pure functions: addresses, URL coding, `MSG` tokens, `SND` and `W/F` frames, the commands, sequence numbers, the S-meter, GPS time and the refusals.
//!
//! Standard: KiwiSDR server protocol (the `kiwi` WebSocket messages of the KiwiSDR server software).

use crate::{KiwiError, Passband};
use sdrkit_core::net::{host_port, show};
use sdrkit_core::Hz;
use sdrkit_driver_core::DriverError;
use std::mem::MaybeUninit;

/// The default port of a KiwiSDR.
pub const DEFAULT_PORT: u16 = 8073;
/// The lowest `sample_rate` that a KiwiSDR reports.
pub const MIN_FS: f64 = 8000.0;
/// The highest `sample_rate` that a KiwiSDR reports.
pub const MAX_FS: f64 = 25_000.0;
/// The most samples in one IQ frame.
pub const MAX_FRAME_SAMPLES: usize = 8192;
/// At most this many `MSG` tokens are kept per message.
pub const MAX_TOKENS: usize = 256;
/// Token names longer than this are dropped.
pub const MAX_NAME: usize = 64;
/// The lowest frequency a KiwiSDR is tuned to.
pub const MIN_DIAL_HZ: f64 = 10_000.0;
/// The highest top of the covered range that the driver uses.
pub const MAX_TOP_HZ: f64 = 30e6;
/// The audio output rate sent in `SET AR OK`.
pub const AR_OUT: u32 = 44_100;
/// GPS time minus UTC, in seconds, since 2017-01-01.
///
/// Standard: IERS Bulletin C 52 (no leap second until further notice).
pub const LEAP_SECONDS: i64 = 18;
/// Unix time of the GPS epoch, 1980-01-06 00:00:00 UTC.
pub const GPS_EPOCH_UNIX: i64 = 315_964_800;
/// Seconds in a GPS week.
pub const WEEK_S: i64 = 604_800;

// ---- Addresses ----

/// A KiwiSDR address: host (IPv6 without brackets) and port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    /// The host name or IP address, IPv6 without brackets.
    pub host: String,
    /// The port.
    pub port: u16,
}

impl Address {
    /// `host:port`, IPv6 in brackets.
    pub fn show(&self) -> String {
        show(&self.host, self.port)
    }
}

fn host_ok(h: &str) -> bool {
    !h.is_empty()
        && h.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':' | b'%'))
}

/// Reads an address: `host:port`, `[v6]:port`, a bare host (port 8073), or a pasted `http://` or `ws://` address.
///
/// # Errors
///
/// [`KiwiError::SecureAddress`] (in [`DriverError::Driver`]) for an `https://` or `wss://` address, else [`DriverError::Address`].
pub fn parse_address(text: &str) -> Result<Address, DriverError> {
    let t = text.trim();
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("wss://") {
        return Err(DriverError::driver(KiwiError::SecureAddress));
    }
    let mut rest = t;
    for p in ["http://", "ws://"] {
        if lower.starts_with(p) {
            rest = &t[p.len()..];
            break;
        }
    }
    // Everything from the first '/' after the host goes (a bracketed IPv6 host holds no '/').
    if let Some(i) = rest.find(['/', '?', '#']) {
        rest = &rest[..i];
    }
    let (host, port) = host_port(rest, Some(DEFAULT_PORT)).map_err(|_| DriverError::Address)?;
    if !host_ok(host) || port == 0 {
        return Err(DriverError::Address);
    }
    Ok(Address {
        host: host.to_owned(),
        port,
    })
}

// ---- URL coding ----

/// URL-encodes every byte except `A`-`Z`, `a`-`z`, `0`-`9`, `-`, `_`, `.`, `~`.
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(char::from(b"0123456789ABCDEF"[(b >> 4) as usize]));
            out.push(char::from(b"0123456789ABCDEF"[(b & 15) as usize]));
        }
    }
    out
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// URL-decodes: each `%XX` (either case) becomes that byte. A `%` without two hex digits after it stays.
pub fn url_decode(s: &[u8]) -> String {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'%' && i + 2 < s.len() {
            if let (Some(a), Some(b)) = (hex(s[i + 1]), hex(s[i + 2])) {
                out.push(a << 4 | b);
                i += 3;
                continue;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---- Tags ----

/// What a server message is, by its 3-byte tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// `MSG`.
    Msg,
    /// `SND`.
    Snd,
    /// `W/F`.
    Wf,
    /// `EXT`.
    Ext,
    /// Shorter than 3 bytes, or another tag.
    Bad,
}

/// The tag of a message.
pub fn tag(m: &[u8]) -> Tag {
    match m.get(..3) {
        Some(b"MSG") => Tag::Msg,
        Some(b"SND") => Tag::Snd,
        Some(b"W/F") => Tag::Wf,
        Some(b"EXT") => Tag::Ext,
        _ => Tag::Bad,
    }
}

/// One `MSG` token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// The name.
    pub name: String,
    /// The URL-decoded value. Empty for a bare flag.
    pub value: String,
}

/// Splits a `MSG` message into its tokens. `None` when it is not a `MSG`.
pub fn parse_msg(m: &[u8]) -> Option<Vec<Token>> {
    if tag(m) != Tag::Msg {
        return None;
    }
    let body = m.get(4..).unwrap_or(&[]);
    let mut out = Vec::new();
    for t in body.split(|&b| b == b' ' || b == b'\n' || b == 0) {
        if t.is_empty() {
            continue;
        }
        if out.len() >= MAX_TOKENS {
            break;
        }
        let (name, value) = match t.iter().position(|&b| b == b'=') {
            Some(i) => (&t[..i], url_decode(&t[i + 1..])),
            None => (t, String::new()),
        };
        if name.is_empty() || name.len() > MAX_NAME {
            continue;
        }
        out.push(Token {
            name: String::from_utf8_lossy(name).into_owned(),
            value,
        });
    }
    Some(out)
}

// ---- SND frames ----

/// `SND` flag: the ADC overflowed.
pub const FLAG_OVERFLOW: u8 = 0x02;
/// `SND` flag: stereo, meaning IQ.
pub const FLAG_IQ: u8 = 0x08;
/// `SND` flag: IMA ADPCM compressed.
pub const FLAG_COMPRESSED: u8 = 0x10;
/// `SND` flag: little-endian samples.
pub const FLAG_LITTLE: u8 = 0x80;

/// Why a frame was not used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadFrame {
    /// Shorter than its header.
    TooShort,
    /// An IQ frame whose GPS header is cut.
    IqHeaderCut,
    /// No whole sample.
    NoSamples,
    /// More than [`MAX_FRAME_SAMPLES`].
    TooMany,
    /// IQ marked compressed.
    CompressedIq,
    /// Not the expected tag.
    NotSnd,
}

/// The GPS fields of an IQ frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gps {
    /// Seconds since the last GPS solution, 254 and 255: none.
    pub state: u8,
    /// Seconds of the GPS week.
    pub seconds: u32,
    /// Nanoseconds.
    pub nanos: u32,
}

impl Gps {
    /// The time is usable.
    pub fn valid(&self) -> bool {
        self.state <= 253 && (self.seconds as i64) < WEEK_S
    }
}

/// An IQ frame, its samples still the wire bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IqFrame<'a> {
    /// The flags byte.
    pub flags: u8,
    /// The sequence number.
    pub seq: u32,
    /// The raw S-meter value.
    pub smeter: u16,
    /// The GPS fields.
    pub gps: Gps,
    /// The bytes of the whole samples (4 per sample).
    pub data: &'a [u8],
    /// 1 to 3 bytes were left over (counted as bad, the samples are still used).
    pub odd: bool,
}

/// What an `SND` frame holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Snd<'a> {
    /// IQ samples.
    Iq(IqFrame<'a>),
    /// Mono audio (flags, payload).
    Audio {
        /// The flags byte.
        flags: u8,
        /// The sequence number.
        seq: u32,
        /// The raw S-meter value.
        smeter: u16,
        /// The audio bytes.
        payload: &'a [u8],
    },
}

/// RSSI in dBm from the raw S-meter value.
pub fn rssi_dbm(smeter: u16) -> f64 {
    0.1 * smeter as f64 - 127.0
}

/// Reads an `SND` frame.
pub fn parse_snd(m: &[u8]) -> Result<Snd<'_>, BadFrame> {
    if tag(m) != Tag::Snd {
        return Err(BadFrame::NotSnd);
    }
    if m.len() < 10 {
        return Err(BadFrame::TooShort);
    }
    let flags = m[3];
    let seq = u32::from_le_bytes([m[4], m[5], m[6], m[7]]);
    let smeter = u16::from_be_bytes([m[8], m[9]]);
    if flags & FLAG_IQ == 0 {
        return Ok(Snd::Audio {
            flags,
            seq,
            smeter,
            payload: &m[10..],
        });
    }
    if m.len() < 20 {
        return Err(BadFrame::IqHeaderCut);
    }
    if flags & FLAG_COMPRESSED != 0 {
        return Err(BadFrame::CompressedIq);
    }
    let gps = Gps {
        state: m[10],
        seconds: u32::from_le_bytes([m[12], m[13], m[14], m[15]]),
        nanos: u32::from_le_bytes([m[16], m[17], m[18], m[19]]),
    };
    let body = &m[20..];
    let n = body.len() / 4;
    if n == 0 {
        return Err(BadFrame::NoSamples);
    }
    if n > MAX_FRAME_SAMPLES {
        return Err(BadFrame::TooMany);
    }
    Ok(Snd::Iq(IqFrame {
        flags,
        seq,
        smeter,
        gps,
        data: &body[..n * 4],
        odd: !body.len().is_multiple_of(4),
    }))
}

impl IqFrame<'_> {
    /// The number of samples.
    pub fn len(&self) -> usize {
        self.data.len() / 4
    }
    /// No samples (never true for a parsed frame).
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    /// The ADC overflowed.
    pub fn overflow(&self) -> bool {
        self.flags & FLAG_OVERFLOW != 0
    }
    /// The raw values (I, Q) of sample `i`.
    #[inline]
    pub fn raw(&self, i: usize) -> (i16, i16) {
        let b = &self.data[i * 4..i * 4 + 4];
        if self.flags & FLAG_LITTLE != 0 {
            (
                i16::from_le_bytes([b[0], b[1]]),
                i16::from_le_bytes([b[2], b[3]]),
            )
        } else {
            (
                i16::from_be_bytes([b[0], b[1]]),
                i16::from_be_bytes([b[2], b[3]]),
            )
        }
    }
    /// True if the wire bytes are already little-endian `Cs16` in the required order (`swap` off).
    pub fn is_native(&self, swap: bool) -> bool {
        self.flags & FLAG_LITTLE != 0 && !swap
    }
    /// Writes samples `from..from + dst.len() / 4` into `dst` as little-endian `Cs16`, I and Q swapped when `swap`.
    pub fn write_cs16(&self, from: usize, swap: bool, dst: &mut [MaybeUninit<u8>]) {
        for (k, d) in dst.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (i, q) = self.raw(from + k);
            let (a, b) = if swap { (q, i) } else { (i, q) };
            let (a, b) = (a.to_le_bytes(), b.to_le_bytes());
            for (slot, byte) in d.iter_mut().zip([a[0], a[1], b[0], b[1]]) {
                slot.write(byte);
            }
        }
    }
}

// ---- W/F frames ----

/// One waterfall line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WfFrame<'a> {
    /// The first bin, in bins at zoom 14.
    pub first_bin: u32,
    /// The zoom, 0 to 14.
    pub zoom: u16,
    /// The sequence number.
    pub seq: u32,
    /// One byte per pixel.
    pub pixels: &'a [u8],
}

/// Reads a `W/F` frame.
pub fn parse_wf(m: &[u8]) -> Result<WfFrame<'_>, BadFrame> {
    if tag(m) != Tag::Wf {
        return Err(BadFrame::NotSnd);
    }
    if m.len() < 16 {
        return Err(BadFrame::TooShort);
    }
    let u = |i: usize| u32::from_le_bytes([m[i], m[i + 1], m[i + 2], m[i + 3]]);
    Ok(WfFrame {
        first_bin: u(4),
        zoom: (u(8) & 0xFFFF) as u16,
        seq: u(12),
        pixels: &m[16..],
    })
}

impl WfFrame<'_> {
    /// The span of the line in Hz.
    pub fn span_hz(&self, bandwidth: Hz) -> Hz {
        bandwidth / f64::from(1u32 << self.zoom.min(14))
    }
    /// The start frequency in Hz (before any `freq_offset`).
    pub fn start_hz(&self, bandwidth: Hz) -> Hz {
        self.first_bin as f64 * bandwidth / (1024.0 * 16384.0)
    }
}

/// The level of an uncompressed pixel in dB.
pub fn pixel_db(p: u8) -> f32 {
    p as f32 - 255.0
}

// ---- Commands ----

/// Hz as kHz with exactly 3 decimals ("7074.500"). `hz` is rounded to whole Hz.
pub fn khz3(hz: f64) -> String {
    let h = hz.round() as i64;
    let sign = if h < 0 { "-" } else { "" };
    let a = h.unsigned_abs();
    format!("{sign}{}.{:03}", a / 1000, a % 1000)
}

/// The auth command. `pw` and `tpw` are the raw passwords.
pub fn auth_line(pw: &str, tpw: &str) -> String {
    let p = if pw.is_empty() {
        "#".to_string()
    } else {
        url_encode(pw)
    };
    if tpw.is_empty() {
        format!("SET auth t=kiwi p={p}")
    } else {
        format!("SET auth t=kiwi p={p} ipl={}", url_encode(tpw))
    }
}

/// The answer to `audio_rate`. `None` if the rate is not a number.
pub fn ar_ok_line(audio_rate: &str) -> Option<String> {
    let n = audio_rate
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())?;
    Some(format!("SET AR OK in={} out={AR_OUT}", n.round() as i64))
}

/// The demodulator command for IQ, the dial in whole Hz and the passband half-width P.
pub fn mod_line(dial_hz: f64, p: u32) -> String {
    format!(
        "SET mod=iq low_cut=-{p} high_cut={p} freq={}",
        khz3(dial_hz)
    )
}

/// The AGC command: `None` for the server's AGC, `Some(G)` for a fixed gain.
pub fn agc_line(manual: Option<i32>) -> String {
    match manual {
        None => "SET agc=1 hang=0 thresh=-100 slope=6 decay=1000 manGain=50".into(),
        Some(g) => format!("SET agc=0 hang=0 thresh=-100 slope=6 decay=1000 manGain={g}"),
    }
}

/// The manual gain of the receiver for a gain in dB, 0 to 120.
pub fn manual_gain(gain_db: f64) -> i32 {
    gain_db.round() as i32
}

/// The covered range: [10 kHz, min(bandwidth, 30 MHz)], moved by the offset.
pub fn covered(bandwidth: Hz, offset_hz: Hz) -> (Hz, Hz) {
    let top = if bandwidth > 1e6 && bandwidth < 1e9 {
        bandwidth.min(MAX_TOP_HZ)
    } else {
        MAX_TOP_HZ
    };
    (MIN_DIAL_HZ + offset_hz, top + offset_hz)
}

/// True if `dial` lies in the covered range `range`.
pub fn covers(range: (Hz, Hz), dial: Hz) -> bool {
    (range.0.round()..=range.1.round()).contains(&dial.round())
}

/// The largest passband: the largest multiple of 100 Hz that is at most fs / 2 - 100.
pub fn max_passband(fs: f64) -> u32 {
    (((fs / 2.0 - 100.0) / 100.0).floor().max(0.0) as u32) * 100
}

/// The passband half-width for `p` at rate `fs`. `None` if it is wider than the receiver allows at that rate.
pub fn passband(p: Passband, fs: f64) -> Option<u32> {
    let max = max_passband(fs);
    let want = match p {
        Passband::Max => return Some(max),
        Passband::P3000 => 3000,
        Passband::P5000 => 5000,
        Passband::P5900 => 5900,
    };
    (want <= max).then_some(want)
}

/// Reads a reported `sample_rate`.
///
/// # Errors
///
/// [`KiwiError::NotKiwiRate`] if it is not a number from [`MIN_FS`] to [`MAX_FS`].
pub fn check_fs(value: &str) -> Result<f64, KiwiError> {
    let v = value.trim().parse::<f64>().ok().filter(|v| v.is_finite());
    match v {
        Some(v) if (MIN_FS..=MAX_FS).contains(&v) => Ok(v),
        _ => Err(KiwiError::NotKiwiRate { reported: v }),
    }
}

// ---- Sequence numbers ----

/// What a frame's sequence number says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeqStep {
    /// The first frame of a connection.
    First,
    /// In order.
    InOrder,
    /// The last frame again: dropped, bad.
    Repeat,
    /// This many frames were lost before it.
    Gap(u32),
    /// A step backwards: the server counts from a new start.
    Restart,
}

/// Follows the sequence numbers of one connection.
#[derive(Clone, Copy, Debug, Default)]
pub struct SeqTracker {
    expected: Option<u32>,
}

impl SeqTracker {
    /// Looks at a frame's number and moves on (except for a repeat).
    pub fn step(&mut self, seq: u32) -> SeqStep {
        let Some(e) = self.expected else {
            self.expected = Some(seq.wrapping_add(1));
            return SeqStep::First;
        };
        let d = seq.wrapping_sub(e);
        let r = match d {
            0 => SeqStep::InOrder,
            0xFFFF_FFFF => return SeqStep::Repeat,
            d if d < 0x8000_0000 => SeqStep::Gap(d),
            _ => SeqStep::Restart,
        };
        self.expected = Some(seq.wrapping_add(1));
        r
    }
}

/// The zero samples written for `d` lost frames of `n` samples: at most 2 s of samples.
pub fn fill_samples(d: u32, n: usize, fs: f64) -> usize {
    let cap = (fs * 2.0).floor() as u64;
    (d as u64 * n as u64).min(cap) as usize
}

// ---- S-meter ----

/// S-units for a level in dBm ("S7", "S9+16"): S9 is -73 dBm and one S-unit is 6 dB.
///
/// Standard: IARU Region 1 Technical Recommendation R.1 (HF S-meter).
pub fn s_units(dbm: f64) -> String {
    if dbm <= -73.0 {
        let s = ((dbm + 127.0) / 6.0).floor().clamp(0.0, 9.0) as i32;
        format!("S{s}")
    } else {
        format!("S9+{}", (dbm + 73.0).round() as i64)
    }
}

/// "S7 (-85 dBm)".
pub fn smeter_text(dbm: f64) -> String {
    format!("{} ({:.0} dBm)", s_units(dbm), dbm)
}

// ---- GPS time ----

/// The Unix time of a GPS seconds-of-week and nanoseconds. The week is the one nearest to the Unix time `now_unix` of the
/// host clock.
pub fn gps_to_unix(seconds: u32, nanos: u32, now_unix: f64) -> f64 {
    let gps_now = now_unix.floor() as i64 - GPS_EPOCH_UNIX + LEAP_SECONDS;
    let week = gps_now.div_euclid(WEEK_S);
    let mut t = week * WEEK_S + seconds as i64;
    if t - gps_now > WEEK_S / 2 {
        t -= WEEK_S;
    } else if gps_now - t > WEEK_S / 2 {
        t += WEEK_S;
    }
    (t + GPS_EPOCH_UNIX - LEAP_SECONDS) as f64 + nanos as f64 * 1e-9
}

// ---- Refusals and limits ----

/// How the driver waits after a failure before it connects again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// It can pass by itself: connect again after the usual pause.
    Retry,
    /// The receiver is busy: a longer pause.
    Busy,
    /// The receiver requests a wait: a longer pause.
    Hold,
    /// A password problem: no automatic reconnect.
    Password,
    /// Nothing will change it: no automatic reconnect.
    Final,
}

/// Reads a token that refuses the connection. `has_password`: a password was set.
pub fn refusal(t: &Token, has_password: bool) -> Option<KiwiError> {
    match t.name.as_str() {
        "badp" => match t.value.trim().parse::<i64>() {
            Ok(0) => None,
            Ok(1) => Some(KiwiError::Password { set: has_password }),
            Ok(2) => Some(KiwiError::Starting),
            Ok(5) => Some(KiwiError::OneConnection),
            Ok(6) => Some(KiwiError::Updating),
            code => Some(KiwiError::Refused { code: code.ok() }),
        },
        "too_busy" => Some(KiwiError::AllBusy {
            channels: t.value.trim().parse().ok(),
        }),
        "down" => Some(KiwiError::Down),
        "redirect" => Some(KiwiError::Redirect {
            to: parse_address(&t.value)
                .map(|a| a.show())
                .unwrap_or_else(|_| t.value.clone()),
        }),
        _ => None,
    }
}

/// The minutes of a limit token (`inactivity_timeout=30`, `ip_limit=60,1.2.3.4`).
pub fn limit_minutes(value: &str) -> Option<u32> {
    value
        .split(',')
        .next()?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| *v >= 0.0)
        .map(|v| v as u32)
}

/// Reads `rx_gps` text "(lat, lon)" or "lat,lon".
pub fn parse_gps(s: &str) -> Option<(f64, f64)> {
    let t = s.trim().trim_start_matches('(').trim_end_matches(')');
    let (a, b) = t.split_once(',')?;
    let (a, b) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
    (a.is_finite() && b.is_finite() && (-90.0..=90.0).contains(&a) && (-180.0..=180.0).contains(&b))
        .then_some((a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_follow_the_rules() {
        let a = |s: &str| parse_address(s).map(|a| a.show());
        assert_eq!(a("kiwi.example.org").unwrap(), "kiwi.example.org:8073");
        assert_eq!(
            a("http://kiwi.example.org:8074/").unwrap(),
            "kiwi.example.org:8074"
        );
        assert_eq!(
            a("http://[2001:db8::1]:8073/?f=7074").unwrap(),
            "[2001:db8::1]:8073"
        );
        assert_eq!(a("ws://h:9").unwrap(), "h:9");
        for bad in ["host:0", "host:70000", ":8073", "", "a b:80"] {
            let e = parse_address(bad).unwrap_err();
            assert_eq!(e, DriverError::Address, "{bad}");
            assert!(!e.retryable());
        }
        let secure = |a: &str| match parse_address(a) {
            Err(DriverError::Driver(e)) => e.downcast_ref::<KiwiError>().cloned(),
            _ => None,
        };
        assert_eq!(
            secure("https://kiwi.example.org"),
            Some(KiwiError::SecureAddress)
        );
        assert_eq!(secure("WSS://x"), Some(KiwiError::SecureAddress));
    }

    #[test]
    fn url_coding_matches_the_vectors() {
        assert_eq!(url_decode(b"%4"), "%4");
        assert_eq!(url_decode(b"%zz"), "%zz");
        assert_eq!(url_decode(b"a%2Fb"), "a/b");
        assert_eq!(url_decode(b"%41"), "A");
        assert_eq!(url_decode(b"%4a%4A"), "JJ");
        assert_eq!(url_encode("p w;1"), "p%20w%3B1");
        assert_eq!(url_encode("Ann B"), "Ann%20B");
        assert_eq!(url_encode("a-_.~Z9"), "a-_.~Z9");
    }

    #[test]
    fn msg_tokens_are_split_and_decoded() {
        let t = parse_msg(b"MSG version_maj=1 version_min=8 name=a%20b flag too_busy=3").unwrap();
        let pairs: Vec<_> = t
            .iter()
            .map(|t| (t.name.as_str(), t.value.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("version_maj", "1"),
                ("version_min", "8"),
                ("name", "a b"),
                ("flag", ""),
                ("too_busy", "3")
            ]
        );
        assert!(parse_msg(b"MS").is_none());
        assert!(parse_msg(b"SND").is_none());
        let mut many = b"MSG".to_vec();
        for i in 0..300 {
            many.extend(format!(" t{i}=1").bytes());
        }
        let t = parse_msg(&many).unwrap();
        assert_eq!(t.len(), 256);
        assert_eq!(t[255].name, "t255");
        // The separator byte is skipped whatever it is. LF and NUL end tokens. Long names are dropped.
        let long = "x".repeat(65);
        let m = format!("MSG\ta=1\nb=2\0c {long}=3 d");
        let t = parse_msg(m.as_bytes()).unwrap();
        let names: Vec<_> = t.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c", "d"]);
        assert_eq!(tag(b"XX"), Tag::Bad);
        assert_eq!(tag(b"EXT x"), Tag::Ext);
    }

    const FRAME: [u8; 32] = [
        0x53, 0x4e, 0x44, 0x0a, 0xfe, 0xff, 0xff, 0xff, 0x04, 0xf6, 0x01, 0x00, 0x40, 0xe2, 0x01,
        0x00, 0x00, 0x65, 0xcd, 0x1d, 0xff, 0xfe, 0x01, 0x2c, 0x40, 0x00, 0xe0, 0x00, 0x7f, 0xff,
        0x80, 0x00,
    ];

    fn cs16(f: &IqFrame<'_>, swap: bool) -> Vec<(i16, i16)> {
        let (mut w, mut r) = sdrkit_driver_core::ring(16, sdrkit_graph::stream::SampleType::Cs16);
        w.write_with(f.len(), |dst, from| f.write_cs16(from, swap, dst));
        let mut out = Vec::new();
        r.read_with(16, |b| {
            out.extend(b.as_chunks::<4>().0.iter().map(|c| {
                (
                    i16::from_le_bytes([c[0], c[1]]),
                    i16::from_le_bytes([c[2], c[3]]),
                )
            }))
        });
        out
    }

    #[test]
    fn the_iq_frame_vector_reads_as_specified() {
        let Ok(Snd::Iq(f)) = parse_snd(&FRAME) else {
            panic!()
        };
        assert!(f.overflow());
        assert_eq!(f.seq, 4_294_967_294);
        assert_eq!(rssi_dbm(f.smeter), 0.0);
        assert!(f.gps.valid());
        assert_eq!((f.gps.seconds, f.gps.nanos), (123_456, 500_000_000));
        assert_eq!(f.len(), 3);
        assert!(!f.odd);
        assert!(!f.is_native(false));
        assert_eq!(
            cs16(&f, false),
            [(-2, 300), (16384, -8192), (32767, -32768)]
        );
        assert_eq!(cs16(&f, true)[0], (300, -2));
        let mut le = FRAME;
        le[3] = 0x8a;
        let Ok(Snd::Iq(f)) = parse_snd(&le) else {
            panic!()
        };
        assert_eq!(
            (0..3).map(|i| f.raw(i)).collect::<Vec<_>>(),
            [(-257, 11265), (64, 224), (-129, 128)]
        );
        assert!(f.is_native(false) && !f.is_native(true));
        assert_eq!(cs16(&f, false), [(-257, 11265), (64, 224), (-129, 128)]);
    }

    #[test]
    fn cut_and_odd_frames_are_bad() {
        assert_eq!(parse_snd(&FRAME[..9]), Err(BadFrame::TooShort));
        assert_eq!(parse_snd(&FRAME[..19]), Err(BadFrame::IqHeaderCut));
        assert_eq!(parse_snd(&FRAME[..22]), Err(BadFrame::NoSamples));
        let Ok(Snd::Iq(f)) = parse_snd(&FRAME[..26]) else {
            panic!()
        };
        assert!(f.odd && f.len() == 1);
        let mut c = FRAME;
        c[3] = 0x18;
        assert_eq!(parse_snd(&c), Err(BadFrame::CompressedIq));
        c[3] = 0x00;
        assert!(matches!(parse_snd(&c), Ok(Snd::Audio { .. })));
        c[..3].copy_from_slice(b"XXX");
        assert_eq!(parse_snd(&c), Err(BadFrame::NotSnd));
        let mut big = FRAME[..20].to_vec();
        big.resize(20 + 9000 * 4, 0);
        assert_eq!(parse_snd(&big), Err(BadFrame::TooMany));
    }

    #[test]
    fn waterfall_lines_read_as_specified() {
        let b = [
            0x57, 0x2f, 0x46, 0x00, 0x01, 0, 0, 0, 0x02, 0, 0, 0, 0x03, 0, 0, 0, 0xff, 0x00, 0x80,
            0xc8,
        ];
        let f = parse_wf(&b).unwrap();
        assert_eq!((f.first_bin, f.zoom, f.seq), (1, 2, 3));
        let db: Vec<_> = f.pixels.iter().map(|&p| pixel_db(p)).collect();
        assert_eq!(db, [0.0, -255.0, -127.0, -55.0]);
        assert_eq!(parse_wf(&b[..15]), Err(BadFrame::TooShort));
        assert_eq!(f.span_hz(30e6), 7.5e6);
    }

    #[test]
    fn commands_are_formatted_exactly() {
        assert_eq!(
            mod_line(7_074_500.0, 5000),
            "SET mod=iq low_cut=-5000 high_cut=5000 freq=7074.500"
        );
        assert_eq!(
            mod_line(14_074_000.0, 3000),
            "SET mod=iq low_cut=-3000 high_cut=3000 freq=14074.000"
        );
        assert!(!covers(covered(30e6, 0.0), 100e6));
        assert!(covers(covered(30e6, 0.0), 7e6));
        assert!(covers(covered(30e6, 0.0), 30e6));
        assert_eq!(
            agc_line(Some(manual_gain(40.0))),
            "SET agc=0 hang=0 thresh=-100 slope=6 decay=1000 manGain=40"
        );
        assert_eq!(manual_gain(39.6), 40);
        assert_eq!(
            agc_line(None),
            "SET agc=1 hang=0 thresh=-100 slope=6 decay=1000 manGain=50"
        );
        assert_eq!(passband(Passband::Max, 12_001.135), Some(5900));
        assert_eq!(passband(Passband::Max, 20_250.0), Some(10_000));
        assert_eq!(passband(Passband::P3000, 12_000.0), Some(3000));
        assert_eq!(passband(Passband::P5900, 9000.0), None);
        assert_eq!(auth_line("", ""), "SET auth t=kiwi p=#");
        assert_eq!(
            auth_line("s3cret pass", ""),
            "SET auth t=kiwi p=s3cret%20pass"
        );
        assert_eq!(
            auth_line("s3cret pass", "tl"),
            "SET auth t=kiwi p=s3cret%20pass ipl=tl"
        );
        assert_eq!(
            ar_ok_line("12000").as_deref(),
            Some("SET AR OK in=12000 out=44100")
        );
        assert_eq!(ar_ok_line("x"), None);
        assert_eq!(khz3(-1500.0), "-1.500");
        assert_eq!(covered(0.0, 0.0), (10e3, 30e6));
        assert_eq!(covered(32e6, 0.0), (10e3, 30e6));
        assert_eq!(covered(10e6, 100e6), (100.01e6, 110e6));
    }

    #[test]
    fn a_reported_rate_must_be_a_kiwi_rate() {
        assert_eq!(
            check_fs("48000"),
            Err(KiwiError::NotKiwiRate {
                reported: Some(48000.0)
            })
        );
        assert!(check_fs("6000").is_err());
        assert_eq!(
            check_fs("x"),
            Err(KiwiError::NotKiwiRate { reported: None })
        );
        assert_eq!(check_fs("12001.135"), Ok(12_001.135));
    }

    #[test]
    fn sequence_numbers_follow_the_rules() {
        let mut s = SeqTracker::default();
        assert_eq!(s.step(100), SeqStep::First);
        assert_eq!(s.step(101), SeqStep::InOrder);
        assert_eq!(s.step(107), SeqStep::Gap(5));
        let mut s = SeqTracker::default();
        s.step(100);
        s.step(101);
        assert_eq!(s.step(101), SeqStep::Repeat);
        assert_eq!(s.step(102), SeqStep::InOrder);
        let mut s = SeqTracker::default();
        s.step(100);
        s.step(101);
        assert_eq!(s.step(0), SeqStep::Restart);
        assert_eq!(s.step(1), SeqStep::InOrder);
        let mut s = SeqTracker::default();
        s.step(0xFFFF_FFFF);
        assert_eq!(s.step(0), SeqStep::InOrder);
        assert_eq!(fill_samples(5, 512, 12_001.135), 2560);
        assert_eq!(fill_samples(119_898, 512, 12_001.135), 24_002);
    }

    #[test]
    fn s_meter_and_gps_time_follow_the_rules() {
        assert_eq!(rssi_dbm(700), -57.0);
        assert_eq!(rssi_dbm(1270), 0.0);
        assert_eq!(rssi_dbm(0), -127.0);
        assert_eq!(s_units(-57.0), "S9+16");
        assert_eq!(s_units(-85.0), "S7");
        assert_eq!(s_units(-127.0), "S0");
        assert_eq!(s_units(-140.0), "S0");
        assert_eq!(s_units(-73.0), "S9");
        assert_eq!(smeter_text(-85.0), "S7 (-85 dBm)");
        assert_eq!(
            gps_to_unix(123_456, 500_000_000, 1_790_000_000.0),
            1_789_985_838.5
        );
        // Near a week boundary the computer's clock picks the nearer week.
        let t = gps_to_unix(604_790, 0, 1_789_985_838.5 + 3.5 * 86_400.0 + 100.0);
        assert!((t - 1_790_000_000.0).abs() < 7.0 * 86_400.0);
        assert!(!Gps {
            state: 1,
            seconds: 604_800,
            nanos: 0
        }
        .valid());
        assert!(!Gps {
            state: 254,
            seconds: 1,
            nanos: 0
        }
        .valid());
        assert!(!Gps {
            state: 255,
            seconds: 1,
            nanos: 0
        }
        .valid());
        assert!(Gps {
            state: 253,
            seconds: 1,
            nanos: 0
        }
        .valid());
    }

    #[test]
    fn refusals_are_classified() {
        let t = |n: &str, v: &str| Token {
            name: n.into(),
            value: v.into(),
        };
        let class = |n: &str, v: &str| refusal(&t(n, v), false).map(|e| e.class());
        assert_eq!(refusal(&t("badp", "0"), false), None);
        assert_eq!(
            refusal(&t("badp", "1"), false),
            Some(KiwiError::Password { set: false })
        );
        assert_eq!(class("badp", "1"), Some(Class::Password));
        assert!(refusal(&t("badp", "1"), true)
            .unwrap()
            .to_string()
            .contains("wrong password"));
        assert_eq!(class("badp", "2"), Some(Class::Busy));
        assert_eq!(class("badp", "5"), Some(Class::Hold));
        assert_eq!(class("badp", "6"), Some(Class::Busy));
        for c in [3, 4, 7, 9] {
            let e = refusal(&t("badp", &c.to_string()), false).unwrap();
            assert_eq!(e, KiwiError::Refused { code: Some(c) });
            assert!(e.to_string().contains(&format!("(code {c})")));
            assert_eq!(e.class(), Class::Password);
        }
        let e = refusal(&t("too_busy", "4"), false).unwrap();
        assert_eq!(e, KiwiError::AllBusy { channels: Some(4) });
        assert!(e
            .to_string()
            .contains("all 4 channels of the KiwiSDR are in use"));
        assert_eq!(e.class(), Class::Busy);
        assert_eq!(class("down", "1"), Some(Class::Hold));
        let e = refusal(&t("redirect", "http://kiwi2.example.org:8074/"), false).unwrap();
        assert_eq!(
            e,
            KiwiError::Redirect {
                to: "kiwi2.example.org:8074".into()
            }
        );
        assert_eq!(e.class(), Class::Final);
        assert_eq!(refusal(&t("version_maj", "1"), false), None);
        assert_eq!(limit_minutes("60,1.2.3.4"), Some(60));
        assert_eq!(limit_minutes("30"), Some(30));
        assert_eq!(parse_gps("(51.5, -0.12)"), Some((51.5, -0.12)));
        assert_eq!(parse_gps("51.5,-0.12"), Some((51.5, -0.12)));
        assert_eq!(parse_gps("x"), None);
    }
}
