//! Datagrams between the stations of a lightning network: detections, timing-transmitter readings and hellos, each signed
//! with a truncated HMAC-SHA256; the replay guard per station and the rate limit per address of the server.
//!
//! Layout (little endian, at most 1200 bytes): magic `SKLT`, version 1, type, station id (FNV-1a 32 of the name), sequence,
//! send time (UTC ns), the station header (position, altitude, time source, flags, clock sigma), the body of the type, and
//! the first 16 bytes of HMAC-SHA256 over everything before it, keyed with SHA-256 of the network key text.

use sdrkit_algo_crypto::{sha256, HmacSha256};
pub use sdrkit_algo_locate::clock::TimeSource;
use std::collections::HashMap;

/// The first four bytes of every datagram.
pub const MAGIC: [u8; 4] = *b"SKLT";
/// The format version.
pub const VERSION: u8 = 1;
/// The largest datagram, bytes.
pub const MAX_LEN: usize = 1200;
/// The default UDP port.
pub const DEFAULT_PORT: u16 = 47390;
/// Bytes before the station header.
pub const HEAD_LEN: usize = 22;
/// Bytes of the station header.
pub const STATION_LEN: usize = 16;
/// Bytes of the MAC.
pub const MAC_LEN: usize = 16;
/// Bytes of one detection record.
pub const RECORD_LEN: usize = 32;
/// The most detections in one datagram.
pub const MAX_DETECTIONS: usize = 32;
/// The longest station name, characters.
pub const MAX_NAME: usize = 24;
/// The shortest network key, characters.
pub const MIN_KEY: usize = 12;

/// The FNV-1a 32-bit hash of a station name; 0 becomes 1.
pub fn station_id(name: &str) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for b in name.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(16_777_619);
    }
    h.max(1)
}

/// The first sequence number of a station that starts at `utc_ns`: tenths of a second since 2020-01-01 00:00 UTC, so that
/// sequence numbers rise over restarts.
pub fn first_sequence(utc_ns: i64) -> u32 {
    const EPOCH_2020_NS: i64 = 1_577_836_800_000_000_000;
    ((utc_ns - EPOCH_2020_NS).max(0) / 100_000_000) as u32
}

/// The wire code of a time source: 0 for none, else its rank (1 GPS to 6 system clock).
pub fn source_code(source: Option<TimeSource>) -> u8 {
    source.map_or(0, TimeSource::rank)
}

/// The time source of a wire code: `Ok(None)` for 0, `Err` for a code above 6.
fn source_of(code: u8) -> Result<Option<TimeSource>, PacketError> {
    if code == 0 {
        return Ok(None);
    }
    TimeSource::from_rank(code)
        .map(Some)
        .ok_or(PacketError::Malformed)
}

/// What every datagram says about its station.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct StationHeader {
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Altitude, m.
    pub alt_m: i16,
    /// The time source of the detections; `None` when the station has no usable time.
    pub source: Option<TimeSource>,
    /// The user set the position.
    pub position_by_user: bool,
    /// The common-view clock is in use.
    pub common_view: bool,
    /// The clock's 1-sigma, ns (saturating).
    pub clock_sigma_ns: u32,
}

/// One detection of a sferic.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Detection {
    /// The leading edge, UTC ns.
    pub utc_ns: i64,
    /// Its 1-sigma, ns.
    pub sigma_ns: u32,
    /// Peak over the floor, dB.
    pub peak_db: f64,
    /// Peak envelope.
    pub amplitude: f32,
    /// Width, us.
    pub width_us: f64,
    /// 10 % to 90 % rise, us.
    pub rise_us: f64,
    /// Mean frequency from the tuned centre, Hz.
    pub mean_hz: f64,
}

/// One boundary reading of a timing transmitter.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct TimingReading {
    /// The transmitter id, 1 to 255, the same at every station.
    pub tx_id: u8,
    /// A symbol boundary on the station's clock, UTC ns.
    pub boundary_ns: i64,
    /// Its 1-sigma, ns.
    pub sigma_ns: u32,
    /// SNR, dB.
    pub snr_db: f64,
    /// Symbol rate, Bd.
    pub baud: f32,
}

/// The body of a datagram.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    /// Detections (type 1).
    Detections(Vec<Detection>),
    /// A timing-transmitter reading (type 2).
    Timing(TimingReading),
    /// A hello with the station name (type 3).
    Hello(String),
}

/// A datagram.
#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    /// FNV-1a of the station name.
    pub station: u32,
    /// Sequence number.
    pub seq: u32,
    /// Send time, UTC ns.
    pub sent_ns: i64,
    /// The station header.
    pub header: StationHeader,
    /// The body.
    pub body: Body,
}

/// Why a datagram was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketError {
    /// Shorter than the least datagram.
    Short,
    /// Longer than [`MAX_LEN`].
    Long,
    /// Not this format or not this version.
    Foreign,
    /// The MAC does not match.
    Mac,
    /// A field or length is wrong.
    Malformed,
}

/// The MAC key of a network key text: SHA-256 of its bytes.
pub fn mac_key(network_key: &str) -> [u8; 32] {
    sha256(network_key.as_bytes())
}

fn clamp_i16(v: f64) -> i16 {
    v.round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

fn clamp_u16(v: f64) -> u16 {
    v.round().clamp(0.0, f64::from(u16::MAX)) as u16
}

impl Packet {
    /// The datagram of the packet, signed with `key` (from [`mac_key`]). A body with more than [`MAX_DETECTIONS`]
    /// detections or a name longer than [`MAX_NAME`] is cut.
    pub fn encode(&self, key: &[u8; 32]) -> Vec<u8> {
        let mut v = Vec::with_capacity(256);
        v.extend_from_slice(&MAGIC);
        v.push(VERSION);
        v.push(match self.body {
            Body::Detections(_) => 1,
            Body::Timing(_) => 2,
            Body::Hello(_) => 3,
        });
        v.extend_from_slice(&self.station.to_le_bytes());
        v.extend_from_slice(&self.seq.to_le_bytes());
        v.extend_from_slice(&self.sent_ns.to_le_bytes());
        let h = &self.header;
        v.extend_from_slice(&((h.lat * 1e7).round() as i32).to_le_bytes());
        v.extend_from_slice(&((h.lon * 1e7).round() as i32).to_le_bytes());
        v.extend_from_slice(&h.alt_m.to_le_bytes());
        v.push(source_code(h.source));
        v.push(u8::from(h.position_by_user) | (u8::from(h.common_view) << 1));
        v.extend_from_slice(&h.clock_sigma_ns.to_le_bytes());
        match &self.body {
            Body::Detections(d) => {
                let d = &d[..d.len().min(MAX_DETECTIONS)];
                v.push(d.len() as u8);
                for r in d {
                    v.extend_from_slice(&r.utc_ns.to_le_bytes());
                    v.extend_from_slice(&r.sigma_ns.to_le_bytes());
                    v.extend_from_slice(&clamp_i16(r.peak_db * 100.0).to_le_bytes());
                    v.extend_from_slice(&r.amplitude.to_le_bytes());
                    v.extend_from_slice(&clamp_u16(r.width_us * 10.0).to_le_bytes());
                    v.extend_from_slice(&clamp_u16(r.rise_us * 10.0).to_le_bytes());
                    v.extend_from_slice(&clamp_i16(r.mean_hz / 10.0).to_le_bytes());
                    v.extend_from_slice(&[0u8; 8]);
                }
            }
            Body::Timing(t) => {
                v.push(t.tx_id);
                v.extend_from_slice(&t.boundary_ns.to_le_bytes());
                v.extend_from_slice(&t.sigma_ns.to_le_bytes());
                v.extend_from_slice(&clamp_i16(t.snr_db * 100.0).to_le_bytes());
                v.extend_from_slice(&t.baud.to_le_bytes());
            }
            Body::Hello(name) => {
                let n: Vec<u8> = name.bytes().take(MAX_NAME).collect();
                v.push(n.len() as u8);
                v.extend_from_slice(&n);
            }
        }
        let mut mac = HmacSha256::new(key);
        mac.update(&v);
        v.extend_from_slice(&mac.finalize_reset()[..MAC_LEN]);
        v
    }

    /// Reads a datagram signed with `key`. The order of the checks: length, magic, version, MAC (in constant time), then the
    /// fields; every length must match exactly.
    ///
    /// # Errors
    ///
    /// [`PacketError`] with the first check that fails.
    pub fn decode(b: &[u8], key: &[u8; 32]) -> Result<Packet, PacketError> {
        if b.len() < HEAD_LEN + STATION_LEN + 1 + MAC_LEN {
            return Err(PacketError::Short);
        }
        if b.len() > MAX_LEN {
            return Err(PacketError::Long);
        }
        if b[..4] != MAGIC || b[4] != VERSION {
            return Err(PacketError::Foreign);
        }
        let (msg, tag) = b.split_at(b.len() - MAC_LEN);
        let mut mac = HmacSha256::new(key);
        mac.update(msg);
        mac.verify_truncated_reset(tag)
            .map_err(|_| PacketError::Mac)?;
        let u32_at = |i: usize| u32::from_le_bytes([msg[i], msg[i + 1], msg[i + 2], msg[i + 3]]);
        let i32_at = |i: usize| u32_at(i) as i32;
        let i64_at = |i: usize| {
            let mut a = [0u8; 8];
            a.copy_from_slice(&msg[i..i + 8]);
            i64::from_le_bytes(a)
        };
        let u16_at = |i: usize| u16::from_le_bytes([msg[i], msg[i + 1]]);
        let i16_at = |i: usize| u16_at(i) as i16;
        let f32_at = |i: usize| f32::from_bits(u32_at(i));
        let lat = f64::from(i32_at(22)) * 1e-7;
        let lon = f64::from(i32_at(26)) * 1e-7;
        if lat.abs() > 90.0 || lon.abs() > 180.0 {
            return Err(PacketError::Malformed);
        }
        let source = source_of(msg[32])?;
        let header = StationHeader {
            lat,
            lon,
            alt_m: i16_at(30),
            source,
            position_by_user: msg[33] & 1 != 0,
            common_view: msg[33] & 2 != 0,
            clock_sigma_ns: u32_at(34),
        };
        let at = HEAD_LEN + STATION_LEN;
        let body = match msg[5] {
            1 => {
                let n = usize::from(msg[at]);
                if n > MAX_DETECTIONS || msg.len() != at + 1 + n * RECORD_LEN {
                    return Err(PacketError::Malformed);
                }
                let mut d = Vec::with_capacity(n);
                for k in 0..n {
                    let r = at + 1 + k * RECORD_LEN;
                    let amplitude = f32_at(r + 14);
                    if !amplitude.is_finite() {
                        return Err(PacketError::Malformed);
                    }
                    d.push(Detection {
                        utc_ns: i64_at(r),
                        sigma_ns: u32_at(r + 8),
                        peak_db: f64::from(i16_at(r + 12)) / 100.0,
                        amplitude,
                        width_us: f64::from(u16_at(r + 18)) / 10.0,
                        rise_us: f64::from(u16_at(r + 20)) / 10.0,
                        mean_hz: f64::from(i16_at(r + 22)) * 10.0,
                    });
                }
                Body::Detections(d)
            }
            2 => {
                if msg.len() != at + 19 || msg[at] == 0 {
                    return Err(PacketError::Malformed);
                }
                let baud = f32_at(at + 15);
                if !(baud.is_finite() && baud > 0.0 && baud <= 1e5) {
                    return Err(PacketError::Malformed);
                }
                Body::Timing(TimingReading {
                    tx_id: msg[at],
                    boundary_ns: i64_at(at + 1),
                    sigma_ns: u32_at(at + 9),
                    snr_db: f64::from(i16_at(at + 13)) / 100.0,
                    baud,
                })
            }
            3 => {
                let n = usize::from(msg[at]);
                let name = &msg[at + 1..];
                if n > MAX_NAME || name.len() != n || !name.iter().all(|c| (0x20..=0x7e).contains(c)) {
                    return Err(PacketError::Malformed);
                }
                Body::Hello(String::from_utf8_lossy(name).into_owned())
            }
            _ => return Err(PacketError::Malformed),
        };
        Ok(Packet {
            station: u32_at(6),
            seq: u32_at(10),
            sent_ns: i64_at(14),
            header,
            body,
        })
    }
}

/// What the replay guard says of a datagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    /// New: take it.
    Fresh,
    /// Seen before.
    Replay,
    /// Too old, or its send time is too far from the server clock.
    Stale,
    /// The table of stations is full.
    Full,
}

#[derive(Clone, Copy, Debug, Default)]
struct Window {
    top: u32,
    seen: u64,
    last_sent_ns: i64,
}

/// The replay guard of the server: per station a window of the last 64 sequence numbers.
#[derive(Clone, Debug)]
pub struct ReplayGuard {
    stations: HashMap<u32, Window>,
    max_age_ns: i64,
}

impl Default for ReplayGuard {
    fn default() -> Self {
        ReplayGuard::new(600.0)
    }
}

impl ReplayGuard {
    /// The most stations held.
    pub const MAX_STATIONS: usize = 256;
    /// The width of the sequence window.
    pub const WINDOW: u32 = 64;

    /// A guard that refuses send times more than `max_age_s` from the server clock.
    pub fn new(max_age_s: f64) -> ReplayGuard {
        ReplayGuard {
            stations: HashMap::new(),
            max_age_ns: (max_age_s * 1e9) as i64,
        }
    }

    /// Judges the datagram `seq` of `station` sent at `sent_ns`, the server clock being `now_ns`, and records it when it is
    /// fresh. A sequence 64 or more below the top is stale unless its send time is newer than the last by more than 1 s (a
    /// restarted station), which resets the window.
    pub fn check(&mut self, station: u32, seq: u32, sent_ns: i64, now_ns: i64) -> Freshness {
        if (sent_ns - now_ns).abs() > self.max_age_ns {
            return Freshness::Stale;
        }
        if !self.stations.contains_key(&station) && self.stations.len() >= Self::MAX_STATIONS {
            return Freshness::Full;
        }
        let w = self.stations.entry(station).or_default();
        if w.seen == 0 {
            *w = Window {
                top: seq,
                seen: 1,
                last_sent_ns: sent_ns,
            };
            return Freshness::Fresh;
        }
        if seq > w.top {
            let shift = seq - w.top;
            w.seen = if shift >= Self::WINDOW {
                1
            } else {
                (w.seen << shift) | 1
            };
            w.top = seq;
        } else {
            let back = w.top - seq;
            if back >= Self::WINDOW {
                if sent_ns > w.last_sent_ns + 1_000_000_000 {
                    *w = Window {
                        top: seq,
                        seen: 1,
                        last_sent_ns: sent_ns,
                    };
                    return Freshness::Fresh;
                }
                return Freshness::Stale;
            }
            let bit = 1u64 << back;
            if w.seen & bit != 0 {
                return Freshness::Replay;
            }
            w.seen |= bit;
        }
        w.last_sent_ns = w.last_sent_ns.max(sent_ns);
        Freshness::Fresh
    }

    /// Forgets every station.
    pub fn clear(&mut self) {
        self.stations.clear();
    }
}

/// The rate limit per sender address, applied before the MAC check: a token bucket of `rate` datagrams a second with a
/// burst of `burst`, for at most `addresses` addresses (the oldest is forgotten).
#[derive(Clone, Debug)]
pub struct RateLimit {
    rate: f64,
    burst: f64,
    addresses: usize,
    buckets: HashMap<String, (f64, f64)>,
}

impl Default for RateLimit {
    fn default() -> Self {
        RateLimit::new(200.0, 400.0, 512)
    }
}

impl RateLimit {
    /// A limit of `rate` a second with a burst of `burst` for at most `addresses` addresses.
    pub fn new(rate: f64, burst: f64, addresses: usize) -> RateLimit {
        RateLimit {
            rate,
            burst,
            addresses: addresses.max(1),
            buckets: HashMap::new(),
        }
    }

    /// Whether a datagram from `addr` at time `now_s` passes; it takes one token.
    pub fn allow(&mut self, addr: &str, now_s: f64) -> bool {
        if !self.buckets.contains_key(addr) && self.buckets.len() >= self.addresses {
            let oldest = self
                .buckets
                .iter()
                .min_by(|a, b| a.1 .1.total_cmp(&b.1 .1))
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                self.buckets.remove(&k);
            }
        }
        let b = self
            .buckets
            .entry(addr.to_owned())
            .or_insert((self.burst, now_s));
        b.0 = (b.0 + (now_s - b.1).max(0.0) * self.rate).min(self.burst);
        b.1 = now_s;
        if b.0 >= 1.0 {
            b.0 -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Whether the network settings allow the network: a key of at least [`MIN_KEY`] characters, a station name of 1 to
/// [`MAX_NAME`] printable characters and a port of 1 to 65535.
pub fn network_allowed(key: &str, name: &str, port: i64) -> bool {
    key.chars().count() >= MIN_KEY
        && (1..=MAX_NAME).contains(&name.len())
        && name.bytes().all(|c| (0x20..=0x7e).contains(&c))
        && (1..=65_535).contains(&port)
}
