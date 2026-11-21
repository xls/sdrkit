//! Data feeds: Beast and SBS-1 lines out and in, ASTERIX through [`crate::asterix`], the TCP servers, the outbound targets, and
//! the feeds from other receivers. A slow client or target loses data (counted), and the decoder never waits.

use crate::codes::Callsign;
use crate::message::{Frame, NON_ICAO};
use crate::settings::{
    ALLOW_LAN, ALLOW_REMOTE, ASTERIX_CAT, ASTERIX_PORT, ASTERIX_SERVER, BEAST_PORT, BEAST_SERVER,
    FEEDS_IN, SAC, SBS_PORT, SBS_SERVER, SIC, TARGETS,
};
use crate::typed::{FeedFields, FeedRow, FEEDS};
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, Clock, ConfigError, Results, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom,
    RunContext, Timed,
};
use sdrkit_util_geo::time::Civil;
use std::fmt;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver as Rx, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- Beast

/// The Beast signal byte for a level in dBFS: round(255 x 10^(level/20)), held to 0..255.
pub fn beast_signal(level_dbfs: f32) -> u8 {
    (255.0 * 10f32.powf(level_dbfs / 20.0))
        .round()
        .clamp(0.0, 255.0) as u8
}

/// The 12 MHz Beast time stamp of a signal time.
pub fn beast_stamp(time_s: f64) -> u64 {
    ((time_s.max(0.0) * 12e6).round() as u64) & ((1u64 << 48) - 1)
}

/// Encodes one frame as a Beast record (0x1A, '2' or '3', 6 stamp bytes, signal, message; 0x1A doubled after the first).
pub fn beast_encode(f: &Frame, stamp: u64, signal: u8, out: &mut Vec<u8>) {
    out.push(0x1A);
    out.push(if f.bits == 56 { b'2' } else { b'3' });
    let put = |b: u8, out: &mut Vec<u8>| {
        out.push(b);
        if b == 0x1A {
            out.push(0x1A);
        }
    };
    for i in (0..6).rev() {
        put((stamp >> (8 * i)) as u8, out);
    }
    put(signal, out);
    for &b in f.data() {
        put(b, out);
    }
}

/// A Beast record read back.
#[derive(Clone, Debug, PartialEq)]
pub struct BeastFrame {
    /// The frame.
    pub frame: Frame,
    /// The 12 MHz time stamp.
    pub stamp: u64,
    /// The signal byte.
    pub signal: u8,
}

/// A streaming Beast parser: it finds its way back after garbage or a record cut short.
#[derive(Clone, Debug, Default)]
pub struct BeastParser {
    buf: Vec<u8>,
    /// Resynchronisations (garbage skipped, records cut short).
    pub resyncs: u64,
}

impl BeastParser {
    /// Feeds bytes; complete '2' and '3' records are appended to `out` ('1' Mode A/C and '4' records are dropped).
    pub fn feed(&mut self, data: &[u8], out: &mut Vec<BeastFrame>) {
        self.buf.extend_from_slice(data);
        let mut i = 0;
        let b = &self.buf;
        let mut consumed = 0;
        'outer: while i < b.len() {
            if b[i] != 0x1A {
                i += 1;
                consumed = i;
                continue;
            }
            if i + 1 >= b.len() {
                break;
            }
            let t = b[i + 1];
            let n = match t {
                b'1' => 2,
                b'2' => 7,
                b'3' => 14,
                b'4' => 14,
                _ => {
                    // Not the start of a record (an escaped 0x1A, or garbage).
                    self.resyncs += 1;
                    i += 1;
                    consumed = i;
                    continue;
                }
            };
            let need = 6 + 1 + n;
            let mut v = [0u8; 21];
            let mut got = 0;
            let mut j = i + 2;
            while got < need {
                if j >= b.len() {
                    break 'outer; // wait for more
                }
                let c = b[j];
                if c == 0x1A {
                    if j + 1 >= b.len() {
                        break 'outer;
                    }
                    if b[j + 1] == 0x1A {
                        v[got] = 0x1A;
                        got += 1;
                        j += 2;
                        continue;
                    }
                    // A new record begins here: this one was cut short.
                    self.resyncs += 1;
                    i = j;
                    consumed = i;
                    continue 'outer;
                }
                v[got] = c;
                got += 1;
                j += 1;
            }
            if t == b'2' || t == b'3' {
                let stamp = v[..6].iter().fold(0u64, |a, &x| (a << 8) | x as u64);
                if let Some(frame) = Frame::new(&v[7..7 + n]) {
                    out.push(BeastFrame {
                        frame,
                        stamp,
                        signal: v[6],
                    });
                }
            }
            i = j;
            consumed = i;
        }
        self.buf.drain(..consumed);
        if self.buf.len() > 4096 {
            self.buf.clear();
            self.resyncs += 1;
        }
    }
}

// ---------------------------------------------------------------- SBS

/// What an SBS line says, beside the message type and the address.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SbsFields {
    /// 1 to 8.
    pub msg_type: u8,
    /// The aircraft key.
    pub key: u32,
    /// Type 1.
    pub callsign: Option<Callsign>,
    /// Feet.
    pub alt_ft: Option<i32>,
    /// Whole knots.
    pub gs_kt: Option<f64>,
    /// Whole degrees.
    pub track_deg: Option<f64>,
    /// Latitude and longitude when this message completed a position.
    pub pos: Option<(f64, f64)>,
    /// ft/min.
    pub vrate_fpm: Option<i32>,
    /// Type 6.
    pub squawk: Option<u16>,
    /// Types 5 and 6: alert, emergency (squawk 7500/7600/7700), SPI.
    pub flags: Option<(bool, bool, bool)>,
    /// Every line.
    pub on_ground: bool,
}

fn b(x: bool) -> &'static str {
    if x {
        "-1"
    } else {
        "0"
    }
}

/// One SBS line (CRLF included). `local` is the local date and time of reception (`sdrkit_util_geo::time::Civil`).
pub fn sbs_line(f: &SbsFields, local: &Civil) -> String {
    use std::fmt::Write as _;
    let date = format!("{:04}/{:02}/{:02}", local.year, local.month, local.day);
    let time = format!(
        "{:02}:{:02}:{:02}.{:03}",
        local.hour,
        local.minute,
        local.second,
        local.nanos / 1_000_000
    );
    let hex = if f.key & NON_ICAO != 0 {
        format!("~{:06X}", f.key & 0xFF_FFFF)
    } else {
        format!("{:06X}", f.key & 0xFF_FFFF)
    };
    let mut s = String::with_capacity(160);
    let _ = write!(
        s,
        "MSG,{},1,1,{hex},1,{date},{time},{date},{time},",
        f.msg_type
    );
    fn opt(s: &mut String, v: Option<impl fmt::Display>) {
        if let Some(v) = v {
            let _ = write!(s, "{v}");
        }
        s.push(',');
    }
    opt(&mut s, f.callsign);
    opt(&mut s, f.alt_ft);
    opt(&mut s, f.gs_kt.map(|v| Fixed(v, 0)));
    opt(&mut s, f.track_deg.map(|v| Fixed(v, 0)));
    opt(&mut s, f.pos.map(|p| Fixed(p.0, 5)));
    opt(&mut s, f.pos.map(|p| Fixed(p.1, 5)));
    opt(&mut s, f.vrate_fpm);
    opt(&mut s, f.squawk.map(crate::codes::squawk_text));
    match f.flags {
        Some((a, e, p)) => {
            let _ = write!(s, "{},{},{},", b(a), b(e), b(p));
        }
        None => s.push_str(",,,"),
    }
    s.push_str(b(f.on_ground));
    s.push_str("\r\n");
    s
}

/// A number with a fixed count of decimals.
struct Fixed(f64, usize);

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.*}", self.1, self.0)
    }
}

/// The wall-clock date and time now, `utc_offset_s` ahead of UTC (the station's offset).
fn local_now(clock: &Clock, utc_offset_s: i32) -> Civil {
    let (secs, nanos) = sdrkit_util_geo::time::split_f64(clock.now()).unwrap_or((0, 0));
    Civil::from_unix_local(secs, nanos, utc_offset_s / 60)
        .or_else(|_| Civil::from_unix(0, 0))
        .expect("the epoch is a date")
}

/// An aircraft state taken from another receiver (an SBS line or an ASTERIX report).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoteState {
    /// The aircraft key.
    pub key: u32,
    /// Trailing spaces removed.
    pub callsign: Option<Callsign>,
    /// Feet.
    pub alt_ft: Option<i32>,
    /// Knots.
    pub speed_kt: Option<f64>,
    /// Degrees.
    pub track_deg: Option<f64>,
    /// Degrees (both or neither).
    pub pos: Option<(f64, f64)>,
    /// ft/min.
    pub vrate_fpm: Option<i32>,
    /// Octal digits as a number.
    pub squawk: Option<u16>,
    /// On the ground.
    pub on_ground: Option<bool>,
}

fn num<T: std::str::FromStr>(s: &str) -> Option<T> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        s.parse().ok()
    }
}

/// Parses one SBS line ("MSG," lines only).
pub fn sbs_parse(line: &str) -> Option<RemoteState> {
    let line = line.trim_end_matches(['\r', '\n']);
    if !line.starts_with("MSG,") {
        return None;
    }
    let f: Vec<&str> = line.split(',').collect();
    if f.len() < 22 {
        return None;
    }
    let mut hex = f[4].trim();
    let mut key_flag = 0;
    if let Some(h) = hex.strip_prefix('~') {
        hex = h;
        key_flag = NON_ICAO;
    }
    if hex.len() != 6 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let key = u32::from_str_radix(hex, 16).ok()?;
    if key == 0 {
        return None;
    }
    let cs = f[10].trim_end();
    let lat: Option<f64> = num(f[14]);
    let lon: Option<f64> = num(f[15]);
    let pos = match (lat, lon) {
        (Some(a), Some(o)) if a.abs() <= 90.0 && o.abs() <= 180.0 && !(a == 0.0 && o == 0.0) => {
            Some((a, o))
        }
        _ => None,
    };
    let sq = f[17].trim();
    let squawk = if sq.is_empty() {
        None
    } else {
        sq.parse::<u32>()
            .ok()
            .filter(|&v| v <= 7777)
            .and_then(|_| crate::codes::parse_squawk(sq))
    };
    let og = f[21].trim();
    Some(RemoteState {
        key: key | key_flag,
        callsign: (!cs.trim().is_empty())
            .then(|| Callsign::try_from_str(cs.trim()).ok())
            .flatten(),
        alt_ft: num::<f64>(f[11]).map(|v| v.round() as i32),
        speed_kt: num(f[12]),
        track_deg: num(f[13]),
        pos,
        vrate_fpm: num::<f64>(f[16]).map(|v| v.round() as i32),
        squawk,
        on_ground: (!og.is_empty()).then(|| og == "-1" || og == "1"),
    })
}

// ---------------------------------------------------------------- hosts

/// Whether a host is on the local network: loopback, private IPv4 ranges, link-local, IPv6 fe80::/10 and fc00::/7, and
/// names ending in .local, .lan, .home, .internal, .localdomain, .home.arpa.
pub fn host_is_local(host: &str) -> bool {
    let h = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    if h == "localhost" {
        return true;
    }
    if let Ok(ip) = h.parse::<IpAddr>() {
        return ip_is_local(&ip);
    }
    [
        ".local",
        ".lan",
        ".home",
        ".internal",
        ".localdomain",
        ".home.arpa",
    ]
    .iter()
    .any(|s| h.ends_with(s))
}

/// Whether an address is on the local network.
pub fn ip_is_local(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            o[0] == 127
                || o[0] == 10
                || (o[0] == 172 && (16..=31).contains(&o[1]))
                || (o[0] == 192 && o[1] == 168)
                || (o[0] == 169 && o[1] == 254)
        }
        IpAddr::V6(v) => {
            let s = v.segments();
            v.is_loopback()
                || (s[0] & 0xFFC0) == 0xFE80
                || (s[0] & 0xFE00) == 0xFC00
                || v.to_ipv4_mapped()
                    .is_some_and(|m| ip_is_local(&IpAddr::V4(m)))
        }
    }
}

/// Why a feed address or target does not parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressError {
    /// It is not `host:port` or `[ipv6]:port`.
    NotHostPort,
    /// The port is not a number from 1 to 65535.
    Port,
    /// A target has no `format@` in front.
    NoFormat,
    /// A target's format is not beast, sbs, asterix21, asterix48 or asterix62.
    UnknownFormat,
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AddressError::NotHostPort => "write it as host:port, for example 192.168.1.20:30005",
            AddressError::Port => "the port must be a number from 1 to 65535",
            AddressError::NoFormat => {
                "write a target as format@host:port, for example beast@192.168.1.20:30004"
            }
            AddressError::UnknownFormat => {
                "the format must be beast, sbs, asterix21, asterix48 or asterix62"
            }
        })
    }
}

impl std::error::Error for AddressError {}

/// Parses `host:port` or `[ipv6]:port` (spaces trimmed, port 1 to 65535).
///
/// # Errors
///
/// [`AddressError::NotHostPort`] or [`AddressError::Port`].
pub fn parse_host_port(s: &str) -> Result<(String, u16), AddressError> {
    let s = s.trim();
    let bad = AddressError::NotHostPort;
    let (host, port) = if let Some(rest) = s.strip_prefix('[') {
        let (h, p) = rest.split_once(']').ok_or(bad)?;
        (h.to_string(), p.strip_prefix(':').ok_or(bad)?)
    } else {
        let (h, p) = s.rsplit_once(':').ok_or(bad)?;
        if h.contains(':') {
            return Err(bad);
        }
        (h.to_string(), p)
    };
    if host.trim().is_empty() {
        return Err(bad);
    }
    let port: u32 = port.trim().parse().map_err(|_| AddressError::Port)?;
    if !(1..=65535).contains(&port) {
        return Err(AddressError::Port);
    }
    Ok((host.trim().to_string(), port as u16))
}

/// The entries of a feed list: separated by commas, semicolons, spaces or line ends, each with its byte position.
fn feed_entries(list: &str) -> impl Iterator<Item = (usize, &str)> {
    list.split([',', ' ', ';', '\n']).filter_map(move |e| {
        let t = e.trim();
        (!t.is_empty()).then(|| (t.as_ptr() as usize - list.as_ptr() as usize, t))
    })
}

/// The byte position of the first entry of a feed list that does not parse, or `None` when every entry parses.
pub fn feed_list_error(list: &str) -> Option<usize> {
    feed_entries(list).find_map(|(at, e)| parse_host_port(e).is_err().then_some(at))
}

// ---------------------------------------------------------------- feeds in

sdrkit_core::choice_enum! {
    /// The format of a feed in.
    pub enum FeedFormat {
        /// Beast binary.
        Beast = ("beast", "Beast"),
        /// BaseStation (SBS) text lines.
        Sbs = ("sbs", "SBS"),
        /// EUROCONTROL ASTERIX.
        Asterix = ("asterix", "ASTERIX"),
    }
}

/// What went wrong with a feed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedProblem {
    /// The host name is not known.
    UnknownHost,
    /// The other side closed the connection.
    Closed,
    /// The host is outside the local network, and the setting does not allow it.
    NotLocal,
    /// A system error of this kind.
    Io(std::io::ErrorKind),
}

impl fmt::Display for FeedProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeedProblem::UnknownHost => f.write_str("the host name is not known"),
            FeedProblem::Closed => f.write_str("the connection was closed"),
            FeedProblem::NotLocal => f.write_str(
                "only hosts on the local network are used (allow hosts outside the local network to use it)",
            ),
            FeedProblem::Io(kind) => write!(f, "{kind}"),
        }
    }
}

/// The state of one feed in, as reported.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceStatus {
    /// host:port.
    pub name: String,
    /// The format, known after the first bytes.
    pub format: Option<FeedFormat>,
    /// Connected now.
    pub connected: bool,
    /// Messages taken.
    pub messages: u64,
    /// Aircraft this feed updated last (filled in by the table).
    pub aircraft: u64,
    /// The last problem.
    pub error: Option<FeedProblem>,
}

/// What the feeds in hand to the graph.
#[derive(Clone, Debug)]
pub enum RemoteOut {
    /// A frame (Beast).
    Frame {
        /// The frame.
        frame: Frame,
        /// Feed number (1 and up).
        source: u16,
        /// Its time stamp and signal byte.
        stamp: Option<(u64, u8)>,
    },
    /// A state (SBS, ASTERIX).
    State {
        /// The state.
        state: RemoteState,
        /// Feed number.
        source: u16,
    },
    /// The feeds' status (at a report).
    Sources(Vec<SourceStatus>),
}

impl Item for RemoteOut {
    const KIND: &'static str = "modes.remote";
}

enum NetEvent {
    Out(RemoteOut),
    Status(usize, SourceStatus),
}

struct Client {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for Client {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn client_thread(
    idx: usize,
    name: String,
    host: String,
    port: u16,
    tx: SyncSender<NetEvent>,
    stop: Arc<AtomicBool>,
) {
    let mut st = SourceStatus {
        name: name.clone(),
        ..Default::default()
    };
    let send_status = |st: &SourceStatus| {
        let _ = tx.try_send(NetEvent::Status(idx, st.clone()));
    };
    let source = (idx + 1) as u16;
    while !stop.load(Ordering::Relaxed) {
        let addr = (host.as_str(), port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next());
        let conn = addr.ok_or(FeedProblem::UnknownHost).and_then(|a| {
            TcpStream::connect_timeout(&a, Duration::from_secs(3))
                .map_err(|e| FeedProblem::Io(e.kind()))
        });
        match conn {
            Err(e) => {
                st.connected = false;
                st.error = Some(e);
                send_status(&st);
            }
            Ok(mut s) => {
                let _ = s.set_read_timeout(Some(Duration::from_millis(200)));
                st.connected = true;
                st.error = None;
                send_status(&st);
                let mut beast = BeastParser::default();
                let mut line = Vec::<u8>::new();
                let mut ast = Vec::<u8>::new();
                let mut fmt: Option<u8> = None;
                let mut buf = [0u8; 8192];
                let mut frames = Vec::new();
                let mut last_status = Instant::now();
                loop {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    match s.read(&mut buf) {
                        Ok(0) => {
                            st.error = Some(FeedProblem::Closed);
                            break;
                        }
                        Ok(n) => {
                            let data = &buf[..n];
                            let f = *fmt.get_or_insert(match data[0] {
                                0x1A => 0,
                                0x15 | 0x30 | 0x3E => 2,
                                _ => 1,
                            });
                            if st.format.is_none() {
                                st.format = FeedFormat::from_index(u16::from(f));
                                send_status(&st);
                            }
                            match f {
                                0 => {
                                    beast.feed(data, &mut frames);
                                    for bf in frames.drain(..) {
                                        st.messages += 1;
                                        let _ = tx.try_send(NetEvent::Out(RemoteOut::Frame {
                                            frame: bf.frame,
                                            source,
                                            stamp: Some((bf.stamp, bf.signal)),
                                        }));
                                    }
                                }
                                1 => {
                                    for &c in data {
                                        if c == b'\n' {
                                            if let Ok(t) = std::str::from_utf8(&line) {
                                                if let Some(state) = sbs_parse(t) {
                                                    st.messages += 1;
                                                    let _ = tx.try_send(NetEvent::Out(
                                                        RemoteOut::State { state, source },
                                                    ));
                                                }
                                            }
                                            line.clear();
                                        } else if line.len() < 512 {
                                            line.push(c);
                                        }
                                    }
                                }
                                _ => {
                                    ast.extend_from_slice(data);
                                    let mut states = Vec::new();
                                    let used = crate::asterix::decode_stream(&ast, &mut states);
                                    ast.drain(..used);
                                    if ast.len() > 65536 {
                                        ast.clear();
                                    }
                                    for state in states {
                                        st.messages += 1;
                                        let _ = tx.try_send(NetEvent::Out(RemoteOut::State {
                                            state,
                                            source,
                                        }));
                                    }
                                }
                            }
                            if last_status.elapsed() > Duration::from_millis(500) {
                                send_status(&st);
                                last_status = Instant::now();
                            }
                        }
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) => {}
                        Err(e) => {
                            st.error = Some(FeedProblem::Io(e.kind()));
                            break;
                        }
                    }
                }
                st.connected = false;
                send_status(&st);
            }
        }
        // Retry after 3 s.
        for _ in 0..30 {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

sdrkit_core::settings! {
    /// The settings of [`RemoteFeeds`].
    pub enum RemoteFeedsSetting {
        /// The feeds in.
        FeedsIn = FEEDS_IN,
        /// Hosts outside the local network.
        AllowRemote = ALLOW_REMOTE,
    }
}

/// The feeds from other receivers (a generator): `feeds_in` lists them as `host:port`.
#[derive(Default)]
pub struct RemoteFeeds {
    list: String,
    allow_remote: bool,
    rx: Option<Rx<NetEvent>>,
    clients: Vec<Client>,
    status: Vec<SourceStatus>,
}

impl RemoteFeeds {
    fn restart(&mut self) {
        self.clients.clear();
        self.status.clear();
        self.rx = None;
        let names: Vec<String> = feed_entries(&self.list)
            .map(|(_, e)| e.to_owned())
            .collect();
        if names.is_empty() {
            return;
        }
        let (tx, rx) = sync_channel(8192);
        self.rx = Some(rx);
        for (i, n) in names.iter().enumerate() {
            let mut st = SourceStatus {
                name: n.clone(),
                ..Default::default()
            };
            match parse_host_port(n) {
                Err(_) => {}
                Ok((host, port)) => {
                    if !self.allow_remote && !host_is_local(&host) {
                        st.error = Some(FeedProblem::NotLocal);
                    } else {
                        let stop = Arc::new(AtomicBool::new(false));
                        let (tx2, stop2, name) = (tx.clone(), stop.clone(), n.clone());
                        let handle = std::thread::Builder::new()
                            .name(format!("adsb-feed-in-{i}"))
                            .spawn(move || client_thread(i, name, host, port, tx2, stop2))
                            .ok();
                        self.clients.push(Client { stop, handle });
                    }
                }
            }
            self.status.push(st);
        }
    }
    /// The feeds' status.
    pub fn status(&self) -> &[SourceStatus] {
        &self.status
    }
}

impl Component for RemoteFeeds {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator(
            "adsb.remote",
            vec![Port::typed::<RemoteOut>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(RemoteFeedsSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<RemoteOut>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let tick = io.tick();
        let o = io.items_out::<RemoteOut>(0);
        if let Some(rx) = &self.rx {
            for _ in 0..4096 {
                match rx.try_recv() {
                    Ok(NetEvent::Out(v)) => o.push(Timed {
                        time: 0.0,
                        value: v,
                    }),
                    Ok(NetEvent::Status(i, s)) => {
                        if let Some(d) = self.status.get_mut(i) {
                            *d = s;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
        if tick && !self.status.is_empty() {
            o.push(Timed {
                time: 0.0,
                value: RemoteOut::Sources(self.status.clone()),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RemoteFeedsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RemoteFeedsSetting::FeedsIn => {
                let list = FEEDS_IN.read(id, v)?;
                if let Some(at) = feed_list_error(list) {
                    return Err(SettingError::Malformed { id, at: Some(at) });
                }
                list.clone_into(&mut self.list);
            }
            RemoteFeedsSetting::AllowRemote => self.allow_remote = ALLOW_REMOTE.read(id, v)?,
        }
        self.restart();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (RemoteFeedsSetting::FeedsIn, Value::Text(self.list.clone())),
            (
                RemoteFeedsSetting::AllowRemote,
                Value::Bool(self.allow_remote),
            ),
        ])
    }
}

// ---------------------------------------------------------------- feeds out

/// An aircraft's state as the ASTERIX outputs need it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TargetReport {
    /// The aircraft key.
    pub key: u32,
    /// Signal time, s.
    pub time: f64,
    /// Seconds since midnight UTC of the report.
    pub tod_s: f64,
    /// Callsign.
    pub callsign: Option<Callsign>,
    /// Squawk.
    pub squawk: Option<u16>,
    /// Barometric altitude, ft.
    pub alt_ft: Option<i32>,
    /// GNSS height, ft.
    pub gnss_ft: Option<i32>,
    /// Position.
    pub pos: Option<(f64, f64)>,
    /// Range (NM) and bearing (deg) from the receiver position.
    pub polar: Option<(f64, f64)>,
    /// Ground speed and track.
    pub ground: Option<(f64, f64)>,
    /// Magnetic heading.
    pub heading_deg: Option<f64>,
    /// IAS.
    pub ias_kt: Option<f64>,
    /// TAS.
    pub tas_kt: Option<f64>,
    /// Mach.
    pub mach: Option<f64>,
    /// Vertical rate, ft/min, and whether it is barometric.
    pub vrate: Option<(i32, bool)>,
    /// On the ground.
    pub on_ground: bool,
    /// Emitter category (set letter A to D and code).
    pub category: Option<(char, u8)>,
    /// Selected altitude.
    pub sel_alt_ft: Option<i32>,
    /// Emergency state 0 to 7.
    pub emergency: u8,
    /// SPI.
    pub spi: bool,
    /// ADS-B version.
    pub version: Option<u8>,
    /// NACp, SIL, NIC.
    pub nacp: Option<u8>,
    /// SIL.
    pub sil: Option<u8>,
    /// NIC.
    pub nic: Option<u8>,
    /// Track number (the table's).
    pub track_no: u16,
    /// The DF of the message (CAT048 report descriptor), 0 for a track.
    pub df: u8,
    /// The MB field of a Comm-B reply and its register, for CAT048.
    pub mb: Option<(u64, u8, u8)>,
    /// From TIS-B.
    pub tisb: bool,
    /// Message level, dBFS.
    pub level_dbfs: Option<f32>,
}

/// The item the table passes to the feeds, one per message.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum FeedItem {
    /// One accepted message of this receiver.
    Message {
        /// The frame.
        frame: Frame,
        /// Signal time.
        time: f64,
        /// Level.
        level_dbfs: f32,
        /// The content of the SBS line, when the message produces one.
        sbs: Option<SbsFields>,
        /// The aircraft after this message (CAT021 for extended squitters, CAT048 for replies).
        target: Option<TargetReport>,
    },
    /// The tracks (CAT062), at a report.
    Tracks(Arc<Vec<TargetReport>>),
}

impl Item for FeedItem {
    const KIND: &'static str = "adsb.feed";
}

struct Server {
    tx: SyncSender<Arc<Vec<u8>>>,
    stop: Arc<AtomicBool>,
    port: u16,
    dropped: Arc<AtomicU64>,
    clients: Arc<AtomicU64>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Server {
    fn start(port: u16, allow_lan: bool) -> std::io::Result<Server> {
        let bind = if allow_lan { "0.0.0.0" } else { "127.0.0.1" };
        let l = TcpListener::bind((bind, port))?;
        l.set_nonblocking(true)?;
        let port = l.local_addr()?.port();
        let (tx, rx) = sync_channel::<Arc<Vec<u8>>>(4096);
        let stop = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicU64::new(0));
        let clients = Arc::new(AtomicU64::new(0));
        let (s2, d2, c2) = (stop.clone(), dropped.clone(), clients.clone());
        let handle = std::thread::Builder::new()
            .name(format!("adsb-server-{port}"))
            .spawn(move || {
                let mut conns: Vec<(TcpStream, Vec<u8>)> = Vec::new();
                while !s2.load(Ordering::Relaxed) {
                    while let Ok((s, peer)) = l.accept() {
                        if allow_lan && !ip_is_local(&peer.ip()) {
                            continue;
                        }
                        let _ = s.set_nonblocking(true);
                        let _ = s.set_nodelay(true);
                        conns.push((s, Vec::new()));
                    }
                    let mut any = false;
                    while let Ok(m) = rx.try_recv() {
                        any = true;
                        for (_, pend) in conns.iter_mut() {
                            if pend.len() + m.len() > 1 << 20 {
                                d2.fetch_add(1, Ordering::Relaxed);
                            } else {
                                pend.extend_from_slice(&m);
                            }
                        }
                    }
                    conns.retain_mut(|(s, pend)| {
                        if pend.is_empty() {
                            return true;
                        }
                        match s.write(pend) {
                            Ok(0) => false,
                            Ok(n) => {
                                pend.drain(..n);
                                true
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => true,
                            Err(_) => false,
                        }
                    });
                    c2.store(conns.len() as u64, Ordering::Relaxed);
                    if !any {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            })
            .ok();
        Ok(Server {
            tx,
            stop,
            port,
            dropped,
            clients,
            handle,
        })
    }
    fn send(&self, data: Arc<Vec<u8>>) {
        if let Err(TrySendError::Full(_)) = self.tx.try_send(data) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

sdrkit_core::choice_enum! {
    /// The format of an outbound target.
    pub enum TargetFormat {
        /// Beast binary.
        Beast = ("beast", "Beast"),
        /// BaseStation (SBS) text lines.
        Sbs = ("sbs", "SBS"),
        /// ASTERIX category 021.
        Asterix21 = ("asterix21", "ASTERIX 021"),
        /// ASTERIX category 048.
        Asterix48 = ("asterix48", "ASTERIX 048"),
        /// ASTERIX category 062.
        Asterix62 = ("asterix62", "ASTERIX 062"),
    }
}

/// An outbound target: a format, a host and port, TCP or UDP, and an optional first line.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetSpec {
    /// The format.
    pub format: TargetFormat,
    /// host:port.
    pub host: String,
    /// Port.
    pub port: u16,
    /// UDP instead of TCP.
    pub udp: bool,
    /// Sent after a TCP connect.
    pub first_line: Option<String>,
}

/// Parses targets: entries separated by ';', each `format@host:port[/udp][#first line]`. Each entry comes with its byte
/// position in `s`.
pub fn parse_targets(s: &str) -> Vec<(usize, Result<TargetSpec, AddressError>)> {
    s.split(';')
        .filter_map(|e| {
            let t = e.trim();
            (!t.is_empty()).then(|| (t.as_ptr() as usize - s.as_ptr() as usize, t))
        })
        .map(|(at, e)| (at, parse_target(e)))
        .collect()
}

/// Parses one target, `format@host:port[/udp][#first line]`.
fn parse_target(e: &str) -> Result<TargetSpec, AddressError> {
    let (e, first) = match e.split_once('#') {
        Some((a, b)) => (a.trim(), Some(b.to_string())),
        None => (e, None),
    };
    let (fmt, rest) = e.split_once('@').ok_or(AddressError::NoFormat)?;
    let fmt = fmt.trim().to_ascii_lowercase();
    let format = TargetFormat::CHOICES
        .iter()
        .position(|c| c.key == fmt.as_str())
        .and_then(|i| TargetFormat::from_index(i as u16))
        .ok_or(AddressError::UnknownFormat)?;
    let (hp, udp) = match rest.strip_suffix("/udp") {
        Some(h) => (h, true),
        None => (rest.strip_suffix("/tcp").unwrap_or(rest), false),
    };
    let (host, port) = parse_host_port(hp)?;
    Ok(TargetSpec {
        format,
        host,
        port,
        udp,
        first_line: first,
    })
}

struct Target {
    spec: TargetSpec,
    tx: SyncSender<Arc<Vec<u8>>>,
    stop: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
    connected: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for Target {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Target {
    fn start(spec: TargetSpec) -> Target {
        let (tx, rx) = sync_channel::<Arc<Vec<u8>>>(4096);
        let stop = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicU64::new(0));
        let connected = Arc::new(AtomicBool::new(false));
        let (s2, c2, sp) = (stop.clone(), connected.clone(), spec.clone());
        let handle = std::thread::Builder::new()
            .name("adsb-target".into())
            .spawn(move || {
                let mut wait = Duration::from_secs(1);
                while !s2.load(Ordering::Relaxed) {
                    let addr: Option<SocketAddr> = (sp.host.as_str(), sp.port)
                        .to_socket_addrs()
                        .ok()
                        .and_then(|mut a| a.next());
                    let ok = if let Some(a) = addr {
                        if sp.udp {
                            UdpSocket::bind(if a.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })
                                .ok()
                                .and_then(|u| u.connect(a).ok().map(|_| u))
                                .map(|u| {
                                    c2.store(true, Ordering::Relaxed);
                                    while !s2.load(Ordering::Relaxed) {
                                        match rx.recv_timeout(Duration::from_millis(100)) {
                                            Ok(m) => {
                                                let _ = u.send(&m);
                                            }
                                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                                            Err(_) => return,
                                        }
                                    }
                                })
                        } else {
                            TcpStream::connect_timeout(&a, Duration::from_secs(3))
                                .ok()
                                .map(|mut s| {
                                    c2.store(true, Ordering::Relaxed);
                                    wait = Duration::from_secs(1);
                                    if let Some(l) = &sp.first_line {
                                        let _ = s.write_all(l.as_bytes());
                                        let _ = s.write_all(b"\r\n");
                                    }
                                    let _ = s.set_write_timeout(Some(Duration::from_secs(2)));
                                    while !s2.load(Ordering::Relaxed) {
                                        match rx.recv_timeout(Duration::from_millis(100)) {
                                            Ok(m) => {
                                                if s.write_all(&m).is_err() {
                                                    break;
                                                }
                                            }
                                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                                            Err(_) => return,
                                        }
                                    }
                                })
                        }
                    } else {
                        None
                    };
                    c2.store(false, Ordering::Relaxed);
                    let _ = ok;
                    let until = Instant::now() + wait;
                    while Instant::now() < until && !s2.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(50));
                        while rx.try_recv().is_ok() {}
                    }
                    wait = (wait * 2).min(Duration::from_secs(60));
                }
            })
            .ok();
        Target {
            spec,
            tx,
            stop,
            dropped,
            connected,
            handle,
        }
    }
    fn send(&self, data: Arc<Vec<u8>>) {
        if self.tx.try_send(data).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

sdrkit_core::choice_enum! {
    /// The ASTERIX categories that the ASTERIX server sends.
    pub enum AsterixCategories {
        /// 021, 048 and 062.
        All = ("all", "021, 048 and 062"),
        /// 021 only.
        Cat21 = ("21", "021 only"),
        /// 048 only.
        Cat48 = ("48", "048 only"),
        /// 062 only.
        Cat62 = ("62", "062 only"),
    }
}

impl AsterixCategories {
    /// Whether it sends 021, 048 and 062.
    fn sends(self) -> [bool; 3] {
        match self {
            AsterixCategories::All => [true, true, true],
            AsterixCategories::Cat21 => [true, false, false],
            AsterixCategories::Cat48 => [false, true, false],
            AsterixCategories::Cat62 => [false, false, true],
        }
    }
}

/// A server of [`FeedOut`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerKind {
    /// The Beast server.
    Beast,
    /// The SBS server.
    Sbs,
    /// The ASTERIX server.
    Asterix,
}

impl ServerKind {
    /// The name of its row in the feeds table.
    fn name(self) -> &'static str {
        match self {
            ServerKind::Beast => "beast",
            ServerKind::Sbs => "sbs",
            ServerKind::Asterix => "asterix",
        }
    }
}

sdrkit_core::settings! {
    /// The settings of [`FeedOut`].
    pub enum FeedOutSetting {
        /// Beast server.
        BeastServer = BEAST_SERVER,
        /// Beast port.
        BeastPort = BEAST_PORT,
        /// SBS server.
        SbsServer = SBS_SERVER,
        /// SBS port.
        SbsPort = SBS_PORT,
        /// ASTERIX server.
        AsterixServer = ASTERIX_SERVER,
        /// ASTERIX port.
        AsterixPort = ASTERIX_PORT,
        /// ASTERIX categories.
        AsterixCat = ASTERIX_CAT,
        /// ASTERIX SAC.
        Sac = SAC,
        /// ASTERIX SIC.
        Sic = SIC,
        /// Servers accept the local network.
        AllowLan = ALLOW_LAN,
        /// Outbound targets.
        Targets = TARGETS,
    }
}

/// The data feeds out (a sink): the Beast, SBS and ASTERIX servers, and the outbound targets.
pub struct FeedOut {
    beast_on: bool,
    beast_port: u16,
    sbs_on: bool,
    sbs_port: u16,
    ast_on: bool,
    ast_port: u16,
    ast_cat: AsterixCategories,
    sac_sic: (u8, u8),
    allow_lan: bool,
    targets_text: String,
    beast: Option<Server>,
    sbs: Option<Server>,
    ast: Option<Server>,
    targets: Vec<Target>,
    errors: Vec<(ServerKind, FeedProblem)>,
    published: Published,
    utc_offset_s: i32,
    clock: Clock,
    sent: u64,
    rows: Vec<String>,
}

impl Default for FeedOut {
    fn default() -> Self {
        FeedOut {
            beast_on: false,
            beast_port: 30005,
            sbs_on: false,
            sbs_port: 30003,
            ast_on: false,
            ast_port: 30010,
            ast_cat: AsterixCategories::All,
            sac_sic: (0, 1),
            allow_lan: false,
            targets_text: String::new(),
            beast: None,
            sbs: None,
            ast: None,
            targets: Vec::new(),
            errors: Vec::new(),
            published: Published::new(FeedFields::SHAPE.fields).with_tables(&[&FEEDS]),
            utc_offset_s: 0,
            clock: Clock::fixed(0.0),
            sent: 0,
            rows: Vec::new(),
        }
    }
}

impl FeedOut {
    fn restart(&mut self) {
        self.errors.clear();
        let mk = |on: bool,
                  port: u16,
                  lan: bool,
                  errors: &mut Vec<(ServerKind, FeedProblem)>,
                  what: ServerKind|
         -> Option<Server> {
            if !on {
                return None;
            }
            match Server::start(port, lan) {
                Ok(s) => Some(s),
                Err(e) => {
                    errors.push((what, FeedProblem::Io(e.kind())));
                    None
                }
            }
        };
        self.beast = None;
        self.sbs = None;
        self.ast = None;
        self.beast = mk(
            self.beast_on,
            self.beast_port,
            self.allow_lan,
            &mut self.errors,
            ServerKind::Beast,
        );
        self.sbs = mk(
            self.sbs_on,
            self.sbs_port,
            self.allow_lan,
            &mut self.errors,
            ServerKind::Sbs,
        );
        self.ast = mk(
            self.ast_on,
            self.ast_port,
            self.allow_lan,
            &mut self.errors,
            ServerKind::Asterix,
        );
        self.targets.clear();
        for (_, t) in parse_targets(&self.targets_text) {
            if let Ok(spec) = t {
                self.targets.push(Target::start(spec));
            }
        }
        self.publish();
    }
    /// The port a server listens on (0 when off).
    pub fn port(&self, which: ServerKind) -> u16 {
        match which {
            ServerKind::Beast => self.beast.as_ref().map_or(0, |s| s.port),
            ServerKind::Sbs => self.sbs.as_ref().map_or(0, |s| s.port),
            ServerKind::Asterix => self.ast.as_ref().map_or(0, |s| s.port),
        }
    }
    fn publish(&mut self) {
        let p = &mut self.published;
        let mut written = std::mem::take(&mut self.rows);
        let mut now: Vec<String> = Vec::with_capacity(written.len());
        for (kind, s) in [
            (ServerKind::Beast, &self.beast),
            (ServerKind::Sbs, &self.sbs),
            (ServerKind::Asterix, &self.ast),
        ] {
            let error = self.errors.iter().find(|e| e.0 == kind).map(|e| e.1);
            if s.is_none() && error.is_none() {
                continue;
            }
            let name = kind.name();
            FEEDS.put(
                p,
                name,
                &FeedRow {
                    name,
                    port: s.as_ref().map(|s| i64::from(s.port)),
                    clients: s.as_ref().map(|s| s.clients.load(Ordering::Relaxed) as i64),
                    dropped: s.as_ref().map(|s| s.dropped.load(Ordering::Relaxed) as i64),
                    connected: None,
                    error: error.map(|e| e.to_string()).as_deref(),
                },
            );
            now.push(name.to_owned());
        }
        for t in &self.targets {
            let name = format!(
                "{}@{}:{}",
                TargetFormat::CHOICES[usize::from(t.spec.format.index())].key,
                t.spec.host,
                t.spec.port
            );
            let row = FeedRow {
                name: &name,
                port: Some(i64::from(t.spec.port)),
                clients: None,
                dropped: Some(t.dropped.load(Ordering::Relaxed) as i64),
                connected: Some(t.connected.load(Ordering::Relaxed)),
                error: None,
            };
            FEEDS.put(p, &name, &row);
            now.push(name);
        }
        written.retain(|k| !now.contains(k));
        for k in &written {
            p.remove_row(&FEEDS, k);
        }
        p.put(&FeedFields {
            feed_messages: self.sent as i64,
        });
        self.rows = now;
    }
    fn any(&self) -> bool {
        self.beast.is_some() || self.sbs.is_some() || self.ast.is_some() || !self.targets.is_empty()
    }
}

impl Component for FeedOut {
    fn attach(&mut self, ctx: &RunContext) {
        self.utc_offset_s = ctx.station.utc_offset_s;
        self.clock = ctx.clock.clone();
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("adsb.feeds", vec![Port::typed::<FeedItem>("in")])
            .with_settings(FeedOutSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let items = io.items::<FeedItem>(0);
        if !self.any() {
            return;
        }
        let local = local_now(&self.clock, self.utc_offset_s);
        let [c21, c48, c62] = self.ast_cat.sends();
        for it in items {
            match &it.value {
                FeedItem::Message {
                    frame,
                    time,
                    level_dbfs,
                    sbs,
                    target,
                } => {
                    self.sent += 1;
                    let mut beast = Vec::new();
                    beast_encode(
                        frame,
                        beast_stamp(*time),
                        beast_signal(*level_dbfs),
                        &mut beast,
                    );
                    let beast = Arc::new(beast);
                    let sbs_line = sbs
                        .as_ref()
                        .map(|f| Arc::new(sbs_line(f, &local).into_bytes()));
                    let (a21, a48) = match target {
                        Some(t) => {
                            let es = matches!(frame.df(), 17 | 18);
                            (
                                es.then(|| Arc::new(crate::asterix::encode_021(t, self.sac_sic))),
                                (!es)
                                    .then(|| Arc::new(crate::asterix::encode_048(t, self.sac_sic))),
                            )
                        }
                        None => (None, None),
                    };
                    if let Some(s) = &self.beast {
                        s.send(beast.clone());
                    }
                    if let (Some(s), Some(l)) = (&self.sbs, &sbs_line) {
                        s.send(l.clone());
                    }
                    if let Some(s) = &self.ast {
                        if let (true, Some(a)) = (c21, &a21) {
                            s.send(a.clone());
                        }
                        if let (true, Some(a)) = (c48, &a48) {
                            s.send(a.clone());
                        }
                    }
                    for t in &self.targets {
                        match t.spec.format {
                            TargetFormat::Beast => t.send(beast.clone()),
                            TargetFormat::Sbs => {
                                if let Some(l) = &sbs_line {
                                    t.send(l.clone())
                                }
                            }
                            TargetFormat::Asterix21 => {
                                if let Some(a) = &a21 {
                                    t.send(a.clone())
                                }
                            }
                            TargetFormat::Asterix48 => {
                                if let Some(a) = &a48 {
                                    t.send(a.clone())
                                }
                            }
                            TargetFormat::Asterix62 => {}
                        }
                    }
                }
                FeedItem::Tracks(tracks) => {
                    let need = (self.ast.is_some() && c62)
                        || self
                            .targets
                            .iter()
                            .any(|t| t.spec.format == TargetFormat::Asterix62);
                    if !need || tracks.is_empty() {
                        continue;
                    }
                    let data = Arc::new(crate::asterix::encode_062(tracks, self.sac_sic));
                    if let (Some(s), true) = (&self.ast, c62) {
                        s.send(data.clone());
                    }
                    for t in &self.targets {
                        if t.spec.format == TargetFormat::Asterix62 {
                            t.send(data.clone());
                        }
                    }
                }
            }
        }
        if io.tick() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        // If the port is 0, the system chooses a free port (the tests use it). The schema limits a port to 0 to 65535 and
        // a code to 0 to 255.
        let port = |n: i64| u16::try_from(n).map_err(|_| SettingError::NotPermitted { id });
        let byte = |n: i64| u8::try_from(n).map_err(|_| SettingError::NotPermitted { id });
        match FeedOutSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FeedOutSetting::BeastServer => self.beast_on = BEAST_SERVER.read(id, v)?,
            FeedOutSetting::BeastPort => self.beast_port = port(BEAST_PORT.read(id, v)?)?,
            FeedOutSetting::SbsServer => self.sbs_on = SBS_SERVER.read(id, v)?,
            FeedOutSetting::SbsPort => self.sbs_port = port(SBS_PORT.read(id, v)?)?,
            FeedOutSetting::AsterixServer => self.ast_on = ASTERIX_SERVER.read(id, v)?,
            FeedOutSetting::AsterixPort => self.ast_port = port(ASTERIX_PORT.read(id, v)?)?,
            FeedOutSetting::AsterixCat => self.ast_cat = ASTERIX_CAT.read(id, v)?,
            FeedOutSetting::Sac => self.sac_sic.0 = byte(SAC.read(id, v)?)?,
            FeedOutSetting::Sic => self.sac_sic.1 = byte(SIC.read(id, v)?)?,
            FeedOutSetting::AllowLan => self.allow_lan = ALLOW_LAN.read(id, v)?,
            FeedOutSetting::Targets => {
                let text = TARGETS.read(id, v)?;
                if let Some((at, _)) = parse_targets(text).into_iter().find(|t| t.1.is_err()) {
                    return Err(SettingError::Malformed { id, at: Some(at) });
                }
                text.clone_into(&mut self.targets_text);
            }
        }
        self.restart();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (FeedOutSetting::BeastServer, Value::Bool(self.beast_on)),
            (
                FeedOutSetting::BeastPort,
                Value::I64(i64::from(self.beast_port)),
            ),
            (FeedOutSetting::SbsServer, Value::Bool(self.sbs_on)),
            (
                FeedOutSetting::SbsPort,
                Value::I64(i64::from(self.sbs_port)),
            ),
            (FeedOutSetting::AsterixServer, Value::Bool(self.ast_on)),
            (
                FeedOutSetting::AsterixPort,
                Value::I64(i64::from(self.ast_port)),
            ),
            (
                FeedOutSetting::AsterixCat,
                Value::Choice(self.ast_cat.index()),
            ),
            (FeedOutSetting::Sac, Value::I64(i64::from(self.sac_sic.0))),
            (FeedOutSetting::Sic, Value::I64(i64::from(self.sac_sic.1))),
            (FeedOutSetting::AllowLan, Value::Bool(self.allow_lan)),
            (
                FeedOutSetting::Targets,
                Value::Text(self.targets_text.clone()),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beast_round_trip_and_resync() {
        let f1 = Frame::from_hex("8D4840D6202CC371C32CE0576098").unwrap();
        let f2 = Frame::from_hex("5D484FDEA248F5").unwrap();
        let mut s = Vec::new();
        beast_encode(&f1, 0x112233441A55, 0x5A, &mut s);
        beast_encode(&f2, 1, 0x20, &mut s);
        s.extend_from_slice(b"junk\x1a\x1a garbage");
        beast_encode(&f1, 0x112233441A55, 0x5A, &mut s);
        let mut p = BeastParser::default();
        let mut out = Vec::new();
        for c in s.chunks(5) {
            p.feed(c, &mut out);
        }
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].stamp, 0x112233441A55);
        assert_eq!((out[0].signal, out[1].signal), (0x5A, 0x20));
        assert_eq!(out[1].frame.bits, 56);
        assert_eq!(out[2].frame, f1);
        // A record cut short is skipped and the next read.
        let mut s = Vec::new();
        beast_encode(&f1, 5, 1, &mut s);
        s.truncate(10);
        beast_encode(&f2, 6, 2, &mut s);
        let mut out = Vec::new();
        BeastParser::default().feed(&s, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].frame, f2);
    }

    #[test]
    fn hosts() {
        for h in [
            "127.0.0.1",
            "localhost",
            "10.1.2.3",
            "172.20.0.1",
            "192.168.1.20",
            "169.254.3.4",
            "::1",
            "fe80::1",
            "fd00::5",
            "pi.local",
            "x.home.arpa",
        ] {
            assert!(host_is_local(h), "{h}");
        }
        for h in ["8.8.8.8", "172.32.0.1", "example.com", "2001:db8::1"] {
            assert!(!host_is_local(h), "{h}");
        }
        assert_eq!(
            parse_host_port(" 192.168.1.20:30005 "),
            Ok(("192.168.1.20".into(), 30005))
        );
        assert_eq!(
            parse_host_port("[fe80::1]:30005"),
            Ok(("fe80::1".into(), 30005))
        );
        assert_eq!(parse_host_port("host"), Err(AddressError::NotHostPort));
        assert_eq!(parse_host_port("host:0"), Err(AddressError::Port));
        assert_eq!(parse_host_port("host:x"), Err(AddressError::Port));
    }

    #[test]
    fn sbs_lines() {
        let l = "MSG,3,1,1,40621D,1,2024/01/01,12:00:00.000,2024/01/01,12:00:00.000,,38000,,,52.26578,3.93891,,,0,0,0,0";
        let s = sbs_parse(l).unwrap();
        assert_eq!((s.alt_ft, s.pos), (Some(38000), Some((52.26578, 3.93891))));
        let l = "MSG,1,1,1,4840D6,1,2024/01/01,12:00:00.000,2024/01/01,12:00:00.000,KLM1023 ,,,,,,,,,,,0";
        let s = sbs_parse(l).unwrap();
        assert_eq!(
            (s.callsign.as_deref(), s.alt_ft, s.pos),
            (Some("KLM1023"), None, None)
        );
        let l = "MSG,4,1,1,485020,1,2024/01/01,12:00:00.000,2024/01/01,12:00:00.000,,,450.5,182.9,,,-832,,,,,0";
        let s = sbs_parse(l).unwrap();
        assert_eq!((s.speed_kt, s.vrate_fpm), (Some(450.5), Some(-832)));
        assert!(s.track_deg.is_some());
        let l = "MSG,6,1,1,485020,1,2024/01/01,12:00:00.000,2024/01/01,12:00:00.000,,,,,,,,7700,0,1,0,0";
        assert_eq!(sbs_parse(l).unwrap().squawk, Some(0o7700));
        for bad in [
            "STA,,5,179,400AE7,10103,2008/11/28,14:58:51.153,2008/11/28,14:58:51.153,RM",
            "MSG,3,1,1,40621G,1,a,b,c,d,,38000,,,52.2,3.9,,,0,0,0,0",
            "MSG,3,1,1,000000,1,a,b,c,d,,38000,,,52.2,3.9,,,0,0,0,0",
            "MSG,3,1,1,40621D",
            "",
        ] {
            assert!(sbs_parse(bad).is_none(), "{bad}");
        }
        let l = "MSG,3,1,1,40621D,1,a,b,c,d,,abc,,,95,3.9,,,0,0,0,0";
        let s = sbs_parse(l).unwrap();
        assert_eq!((s.alt_ft, s.pos), (None, None));
        let f = SbsFields {
            msg_type: 3,
            key: 0x40621D,
            alt_ft: Some(38000),
            pos: Some((52.26578, 3.93891)),
            ..Default::default()
        };
        let line = sbs_line(&f, &local_now(&Clock::fixed(0.0), 0));
        assert!(line.ends_with(",0\r\n"));
        assert_eq!(sbs_parse(&line).unwrap().pos, Some((52.26578, 3.93891)));
    }

    #[test]
    fn targets_parse() {
        let t = parse_targets("beast@192.168.1.2:30004; sbs@host.lan:30003/udp#hello");
        assert_eq!(t.len(), 2);
        assert_eq!(t[1].0, 25);
        let t1 = t[1].1.as_ref().unwrap();
        assert!(t1.udp && t1.first_line.as_deref() == Some("hello"));
        assert_eq!(t1.format, TargetFormat::Sbs);
        assert_eq!(
            parse_targets("x@1.2.3.4:5")[0].1,
            Err(AddressError::UnknownFormat)
        );
        assert_eq!(feed_list_error("1.2.3.4:5, host"), Some(11));
    }
}
