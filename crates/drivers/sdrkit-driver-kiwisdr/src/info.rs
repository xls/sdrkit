//! The information the driver holds about a receiver, per address: the server information, the meters of the stream, the listener
//! list and the last waterfall lines.

use crate::proto::{self, parse_gps, url_decode, Token};
use sdrkit_driver_core::DriverError;
use serde::de::IgnoredAny;
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The most listener rows kept.
pub const MAX_LISTENERS: usize = 32;
/// "ADC overload" is shown this long after the last frame that had it.
pub const OVERLOAD_HOLD: Duration = Duration::from_secs(1);
/// The waterfall lines kept, the newest last.
pub const WATERFALL_ROWS: usize = 64;

/// One row of the listener list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listener {
    /// The channel number.
    pub channel: u32,
    /// The listener's name. Empty for a free channel.
    pub name: String,
    /// The listener's location.
    pub location: String,
    /// The frequency, Hz.
    pub freq_hz: Option<f64>,
    /// The demodulator the listener uses, as the receiver names it.
    pub demod: String,
    /// How long the listener is connected, seconds.
    pub connected_s: Option<f64>,
}

impl Listener {
    /// The channel is free.
    pub fn free(&self) -> bool {
        self.name.is_empty()
    }
}

/// One waterfall line, in dB.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterfallLine {
    /// The frequency of the first pixel, Hz.
    pub start_hz: f64,
    /// The span of the line, Hz.
    pub span_hz: f64,
    /// The zoom, 0 to 14.
    pub zoom: u16,
    /// The sequence number of the line.
    pub seq: u32,
    /// One level per pixel, dB (0 is the strongest).
    pub levels_db: Vec<f32>,
}

/// The figures of the stream, written by the read loop without a lock and read by the host's poll.
#[derive(Debug)]
pub(crate) struct Meters {
    epoch: Instant,
    pub frames: AtomicU64,
    pub missed: AtomicU64,
    pub bad: AtomicU64,
    pub not_iq: AtomicU64,
    pub discontinuities: AtomicU64,
    rssi: AtomicU64,
    rssi_max: AtomicU64,
    overload_ms: AtomicU64,
    pub gps_valid: AtomicBool,
    gps_unix: AtomicU64,
    mbit_s: AtomicU64,
    iq_fraction: AtomicU64,
}

fn none() -> AtomicU64 {
    AtomicU64::new(f64::NAN.to_bits())
}

fn read(a: &AtomicU64) -> Option<f64> {
    Some(f64::from_bits(a.load(Ordering::Relaxed))).filter(|v| !v.is_nan())
}

fn write(a: &AtomicU64, v: f64) {
    a.store(v.to_bits(), Ordering::Relaxed);
}

impl Default for Meters {
    fn default() -> Self {
        Meters {
            epoch: Instant::now(),
            frames: AtomicU64::new(0),
            missed: AtomicU64::new(0),
            bad: AtomicU64::new(0),
            not_iq: AtomicU64::new(0),
            discontinuities: AtomicU64::new(0),
            rssi: none(),
            rssi_max: none(),
            overload_ms: AtomicU64::new(0),
            gps_valid: AtomicBool::new(false),
            gps_unix: none(),
            mbit_s: none(),
            iq_fraction: none(),
        }
    }
}

impl Meters {
    /// Adds `n` to counter `c`.
    pub fn add(c: &AtomicU64, n: u64) {
        c.fetch_add(n, Ordering::Relaxed);
    }
    /// The S-meter reading and the highest of the last second, dBm.
    pub fn rssi(&self) -> (Option<f64>, Option<f64>) {
        (read(&self.rssi), read(&self.rssi_max))
    }
    /// Writes the S-meter reading and the highest of the last second.
    pub fn set_rssi(&self, dbm: f64, max_dbm: f64) {
        write(&self.rssi, dbm);
        write(&self.rssi_max, max_dbm);
    }
    /// A frame with the overflow flag came at `now`.
    pub fn overloaded(&self, now: Instant) {
        let ms = now.saturating_duration_since(self.epoch).as_millis() + 1;
        self.overload_ms
            .store(u64::try_from(ms).unwrap_or(u64::MAX), Ordering::Relaxed);
    }
    /// The ADC overload is to be shown.
    pub fn overload(&self) -> bool {
        match self.overload_ms.load(Ordering::Relaxed) {
            0 => false,
            ms => {
                let at = self.epoch + Duration::from_millis(ms - 1);
                at.elapsed() < OVERLOAD_HOLD
            }
        }
    }
    /// The last time put on a delivered sample (Unix time).
    pub fn gps_unix(&self) -> Option<f64> {
        read(&self.gps_unix)
    }
    /// Writes the last time put on a delivered sample.
    pub fn set_gps_unix(&self, t: f64) {
        write(&self.gps_unix, t);
    }
    /// The throughput of the last window, Mbit/s, and the IQ samples received as a fraction of the rate.
    pub fn link(&self) -> (Option<f64>, Option<f64>) {
        (read(&self.mbit_s), read(&self.iq_fraction))
    }
    /// Writes the figures of one throughput window.
    pub fn set_link(&self, mbit_s: f64, iq_fraction: f64) {
        write(&self.mbit_s, mbit_s);
        write(&self.iq_fraction, iq_fraction);
    }
}

/// The S-meter of a stream: it keeps the newest reading and the highest of the last second, and publishes them at most 4 times
/// a second.
#[derive(Debug, Default)]
pub(crate) struct SMeter {
    window: Option<(Instant, f64)>,
    pending: Option<(f64, f64)>,
    published: Option<Instant>,
}

impl SMeter {
    /// A new reading at `now`. Returns the reading and the highest of the last second when they are due to be published.
    pub fn reading(&mut self, dbm: f64, now: Instant) -> Option<(f64, f64)> {
        let max = match self.window {
            Some((t, m)) if now.duration_since(t) < Duration::from_secs(1) => m.max(dbm),
            _ => {
                self.window = Some((now, dbm));
                dbm
            }
        };
        if let Some((t, _)) = self.window {
            self.window = Some((t, max));
        }
        self.pending = Some((dbm, max));
        self.due(now)
    }

    /// The pending reading if 250 ms passed since the last one published.
    pub fn due(&mut self, now: Instant) -> Option<(f64, f64)> {
        if self
            .published
            .is_some_and(|t| now.duration_since(t) < Duration::from_millis(250))
        {
            return None;
        }
        let p = self.pending.take()?;
        self.published = Some(now);
        Some(p)
    }
}

/// Everything known about one receiver address that the messages of the receiver say.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerInfo {
    /// A stream is connected.
    pub connected: bool,
    /// Why it does not stream. `None` when all is well.
    pub reason: Option<DriverError>,
    /// `version_maj`.
    pub version_maj: Option<u32>,
    /// `version_min`.
    pub version_min: Option<u32>,
    /// The channel count.
    pub rx_chans: Option<u32>,
    /// Channels without a password.
    pub chan_no_pwd: Option<u32>,
    /// The `bandwidth` token, Hz.
    pub bandwidth_hz: Option<f64>,
    /// The `freq_offset` token, Hz.
    pub freq_offset_hz: f64,
    /// The inactivity limit, minutes.
    pub inactivity_min: Option<u32>,
    /// The daily limit per address, minutes.
    pub ip_limit_min: Option<u32>,
    /// The IQ rate the server reported.
    pub sample_rate: Option<f64>,
    /// `rx_name` of the configuration.
    pub rx_name: String,
    /// `rx_location`.
    pub rx_location: String,
    /// `rx_antenna`.
    pub rx_antenna: String,
    /// `rx_grid`.
    pub rx_grid: String,
    /// `rx_gps`, latitude and longitude.
    pub rx_gps: Option<(f64, f64)>,
    /// The listener list.
    pub listeners: Vec<Listener>,
    /// The last waterfall lines, the newest last.
    pub waterfall: VecDeque<WaterfallLine>,
    /// Why the waterfall is not available. `None` when it is, or when it is off.
    pub waterfall_problem: Option<DriverError>,
}

impl ServerInfo {
    /// The version as "1.800".
    pub fn version(&self) -> Option<String> {
        Some(format!("{}.{}", self.version_maj?, self.version_min?))
    }

    /// The covered range, Hz.
    pub fn covered(&self) -> (f64, f64) {
        proto::covered(self.bandwidth_hz.unwrap_or(0.0), self.freq_offset_hz)
    }

    /// The session limit, minutes.
    pub fn limit_min(&self) -> Option<u32> {
        self.inactivity_min.or(self.ip_limit_min)
    }

    /// Takes one information token. Returns true if it was one.
    pub fn note(&mut self, t: &Token) -> bool {
        let v = t.value.trim();
        let int = || {
            v.parse::<f64>()
                .ok()
                .filter(|x| x.is_finite() && *x >= 0.0)
                .map(|x| x as u32)
        };
        match t.name.as_str() {
            "version_maj" => self.version_maj = int(),
            "version_min" => self.version_min = int(),
            "bandwidth" => self.bandwidth_hz = v.parse().ok().filter(|b: &f64| b.is_finite()),
            "freq_offset" => {
                self.freq_offset_hz = v
                    .parse::<f64>()
                    .ok()
                    .filter(|o| o.is_finite())
                    .map_or(0.0, |khz| khz * 1e3);
            }
            "rx_chans" => self.rx_chans = int(),
            "chan_no_pwd" => self.chan_no_pwd = int(),
            "inactivity_timeout" => self.inactivity_min = proto::limit_minutes(v),
            "ip_limit" => self.ip_limit_min = proto::limit_minutes(v),
            "load_cfg" => self.load_cfg(&t.value),
            "user_cb" => {
                if let Some(l) = parse_listeners(&t.value) {
                    self.listeners = l;
                }
            }
            _ => return false,
        }
        true
    }

    /// Reads the public configuration of the receiver (JSON). A value that does not parse is ignored.
    pub fn load_cfg(&mut self, json: &str) {
        let Ok(c) = serde_json::from_str::<Config>(json) else {
            return;
        };
        for (to, from) in [
            (&mut self.rx_name, c.rx_name),
            (&mut self.rx_location, c.rx_location),
            (&mut self.rx_antenna, c.rx_antenna),
            (&mut self.rx_grid, c.rx_grid),
        ] {
            if let Some(Loose::Text(s)) = from {
                *to = s;
            }
        }
        if let Some(Loose::Text(s)) = c.rx_gps {
            self.rx_gps = parse_gps(&s);
        }
    }

    /// Adds a waterfall line. The oldest goes past [`WATERFALL_ROWS`].
    pub fn add_line(&mut self, line: WaterfallLine) {
        if self.waterfall.len() >= WATERFALL_ROWS {
            self.waterfall.pop_front();
        }
        self.waterfall.push_back(line);
    }
}

/// A JSON value that is text, a number, a truth value, or something else.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Loose {
    /// A string.
    Text(String),
    /// A number.
    Number(f64),
    /// `true` or `false`.
    Bool(bool),
    /// Anything else.
    Other(IgnoredAny),
}

impl Loose {
    /// A string as it is, a number in its shortest form, else empty.
    pub fn text(self) -> String {
        match self {
            Loose::Text(s) => s,
            Loose::Number(n) => n.to_string(),
            Loose::Bool(_) | Loose::Other(_) => String::new(),
        }
    }
    /// A number, or the first number of a text "12,34".
    pub fn number(&self) -> Option<f64> {
        match self {
            Loose::Number(n) => Some(*n),
            Loose::Text(s) => s.split(',').next()?.trim().parse().ok(),
            Loose::Bool(_) | Loose::Other(_) => None,
        }
    }
}

/// The fields of the configuration that the driver reads.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Config {
    rx_name: Option<Loose>,
    rx_location: Option<Loose>,
    rx_antenna: Option<Loose>,
    rx_grid: Option<Loose>,
    rx_gps: Option<Loose>,
}

/// One element of a `user_cb` list.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UserItem {
    User(User),
    Other(IgnoredAny),
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct User {
    i: Option<Loose>,
    n: Option<Loose>,
    g: Option<Loose>,
    f: Option<Loose>,
    m: Option<Loose>,
    t: Option<Loose>,
}

/// A text of the list, URL-decoded once more if it holds a `%`.
fn user_text(v: Option<Loose>) -> String {
    let s = v.map(Loose::text).unwrap_or_default();
    if s.contains('%') {
        url_decode(s.as_bytes())
    } else {
        s
    }
}

/// Seconds of a "h:mm:ss" text.
fn hms(s: &str) -> Option<f64> {
    s.trim().split(':').try_fold(0.0, |acc, p| {
        let n = p.trim().parse::<u32>().ok()?;
        Some(acc * 60.0 + f64::from(n))
    })
}

/// Reads a `user_cb` value (already URL-decoded once). `None` if it does not parse.
pub fn parse_listeners(json: &str) -> Option<Vec<Listener>> {
    let items: Vec<UserItem> = serde_json::from_str(json).ok()?;
    let mut out: Vec<Listener> = Vec::new();
    for (k, it) in items.into_iter().enumerate() {
        let UserItem::User(u) = it else { continue };
        let channel =
            u.i.as_ref()
                .and_then(Loose::number)
                .filter(|c| *c >= 0.0 && c.fract() == 0.0)
                .map_or(k as u32, |c| c as u32);
        if out.iter().any(|l| l.channel == channel) {
            continue;
        }
        let name = user_text(u.n);
        let row = if name.is_empty() {
            Listener {
                channel,
                ..Listener::default()
            }
        } else {
            Listener {
                channel,
                name,
                location: user_text(u.g),
                freq_hz: match u.f {
                    Some(Loose::Number(f)) => Some(f),
                    _ => None,
                },
                demod: user_text(u.m),
                connected_s: hms(&user_text(u.t)),
            }
        };
        out.push(row);
        if out.len() >= MAX_LISTENERS {
            break;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::parse_msg;

    #[test]
    fn the_listener_list_reads_and_a_cut_one_is_ignored() {
        let m = b"MSG user_cb=%5B%7B%22i%22%3A0%2C%22n%22%3A%22sdrkit%22%2C%22f%22%3A7074000%7D%2C%7B%22i%22%3A1%7D%5D";
        let mut info = ServerInfo::default();
        for t in parse_msg(m).unwrap() {
            info.note(&t);
        }
        assert_eq!(info.listeners.len(), 2);
        assert_eq!(info.listeners[0].name, "sdrkit");
        assert_eq!(info.listeners[0].freq_hz, Some(7_074_000.0));
        assert!(info.listeners[1].free());
        for t in parse_msg(b"MSG user_cb=%5B").unwrap() {
            info.note(&t);
        }
        assert_eq!(info.listeners.len(), 2);
        // A name can be encoded once more.
        let l = parse_listeners(r#"[{"i":2,"n":"Ann%20B","g":"Oslo","m":"usb","t":"0:01:02"}, 7]"#)
            .unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(
            (l[0].name.as_str(), l[0].connected_s),
            ("Ann B", Some(62.0))
        );
        let many: String = format!(
            "[{}]",
            (0..40)
                .map(|i| format!("{{\"i\":{i}}}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(parse_listeners(&many).unwrap().len(), 32);
    }

    #[test]
    fn configuration_fields_are_kept_and_junk_ignored() {
        let mut info = ServerInfo::default();
        info.load_cfg(r#"{"rx_name":"Test","rx_location":"Here","rx_antenna":"Loop","rx_grid":"JO59","rx_gps":"(51.5, -0.12)","x":1}"#);
        assert_eq!(info.rx_name, "Test");
        assert_eq!(info.rx_gps, Some((51.5, -0.12)));
        info.load_cfg("{not json");
        assert_eq!(info.rx_name, "Test");
        info.load_cfg(r#"{"rx_name":5,"rx_grid":"KP20"}"#);
        assert_eq!(
            (info.rx_name.as_str(), info.rx_grid.as_str()),
            ("Test", "KP20")
        );
        for t in parse_msg(b"MSG version_maj=1 version_min=800 bandwidth=30000000 freq_offset=0.000 rx_chans=8 inactivity_timeout=30")
            .unwrap()
        {
            assert!(info.note(&t));
        }
        assert_eq!(info.version().as_deref(), Some("1.800"));
        assert_eq!(info.limit_min(), Some(30));
        assert_eq!(info.covered(), (10e3, 30e6));
    }

    #[test]
    fn the_s_meter_is_published_at_most_four_times_a_second_with_the_highest() {
        let mut m = SMeter::default();
        let t0 = Instant::now();
        assert_eq!(m.reading(-80.0, t0), Some((-80.0, -80.0)));
        assert_eq!(m.reading(-50.0, t0 + Duration::from_millis(100)), None);
        assert_eq!(
            m.reading(-90.0, t0 + Duration::from_millis(300)),
            Some((-90.0, -50.0))
        );
        assert_eq!(m.reading(-60.0, t0 + Duration::from_millis(350)), None);
        assert_eq!(m.due(t0 + Duration::from_millis(600)), Some((-60.0, -50.0)));
    }

    #[test]
    fn the_meters_hold_no_value_until_written() {
        let m = Meters::default();
        assert_eq!(m.rssi(), (None, None));
        assert!(!m.overload());
        m.overloaded(Instant::now());
        assert!(m.overload());
        m.set_rssi(-57.0, -50.0);
        assert_eq!(m.rssi(), (Some(-57.0), Some(-50.0)));
    }

    #[test]
    fn the_waterfall_keeps_its_newest_lines() {
        let mut info = ServerInfo::default();
        for seq in 0..(WATERFALL_ROWS as u32 + 3) {
            info.add_line(WaterfallLine {
                seq,
                ..WaterfallLine::default()
            });
        }
        assert_eq!(info.waterfall.len(), WATERFALL_ROWS);
        assert_eq!(
            info.waterfall.back().map(|l| l.seq),
            Some(WATERFALL_ROWS as u32 + 2)
        );
    }
}
