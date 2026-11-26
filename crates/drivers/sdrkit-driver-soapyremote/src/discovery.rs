//! Finding SoapyRemote servers on the local network by SSDP (UPnP M-SEARCH and NOTIFY) and DNS-SD (`_soapy._tcp`). A search
//! runs only when the host requests one.
//!
//! Standard: UPnP Device Architecture 1.1 clause 1 (SSDP), RFC 6762 (multicast DNS) and RFC 6763 (DNS-SD).

use crate::plan::DEFAULT_PORT;
use crate::SoapyError;
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use sdrkit_core::net::{Datagram, DatagramOptions, Transport};
use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

/// The SSDP search target of a SoapyRemote server.
pub const SEARCH_TARGET: &str = "urn:schemas-pothosware-com:service:soapyRemote:1";
/// The header of an SSDP answer that names the radio a server serves ("HackRF One"). Servers of this library send it.
pub const RADIO_HEADER: &str = "X-SDRKIT-RADIO";
/// The SSDP IPv4 group and port.
pub const SSDP_V4: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(239, 255, 255, 250)), 1900);
/// The SSDP IPv6 link-local group and port.
pub const SSDP_V6: SocketAddr = SocketAddr::new(
    IpAddr::V6(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0xc)),
    1900,
);
/// The mDNS IPv4 group and port.
pub const MDNS_V4: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 251)), 5353);
/// The DNS-SD service type.
pub const SERVICE: &str = "_soapy._tcp.local";
/// How long answers are collected.
pub const COLLECT: Duration = Duration::from_millis(2500);
/// The age-out when an answer provides no max-age.
pub const DEFAULT_MAX_AGE: Duration = Duration::from_secs(120);
/// How long a search waits on each socket for an answer.
const POLL: Duration = Duration::from_millis(10);

/// The M-SEARCH request.
pub fn m_search() -> String {
    format!(
        "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {SEARCH_TARGET}\r\nUSER-AGENT: {} \
         UPnP/1.1 sdrkit/{}\r\n\r\n",
        std::env::consts::OS,
        env!("CARGO_PKG_VERSION")
    )
}

/// What an SSDP message says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SsdpKind {
    /// `HTTP/1.1 200 OK` to a search.
    Response,
    /// `NOTIFY` with `ssdp:alive`.
    Alive,
    /// `NOTIFY` with `ssdp:byebye`.
    Byebye,
}

/// One SSDP message about a SoapyRemote server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SsdpMessage {
    /// Its kind.
    pub kind: SsdpKind,
    /// The server id (the uuid of `USN`).
    pub uuid: String,
    /// The port of `LOCATION`, else 55132.
    pub port: u16,
    /// The `SERVER` text.
    pub server: String,
    /// The radio that the server names in [`RADIO_HEADER`], or empty.
    pub radio: String,
    /// The `CACHE-CONTROL` max-age.
    pub max_age: Duration,
}

/// The SSDP header lines the driver reads.
#[derive(Default)]
struct Headers<'a> {
    nt: Option<&'a str>,
    st: Option<&'a str>,
    nts: Option<&'a str>,
    usn: Option<&'a str>,
    location: Option<&'a str>,
    cache_control: Option<&'a str>,
    server: Option<&'a str>,
    radio: Option<&'a str>,
}

/// Reads an SSDP message. `None` if it is not about a SoapyRemote server.
pub fn parse_ssdp(text: &str) -> Option<SsdpMessage> {
    let mut lines = text.split("\r\n").flat_map(|l| l.split('\n'));
    let first = lines.next()?.trim();
    let notify = first.to_uppercase().starts_with("NOTIFY");
    if !notify && !first.to_uppercase().starts_with("HTTP/1.1 200") {
        return None;
    }
    let mut h = Headers::default();
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            let v = Some(v.trim());
            match k.trim().to_ascii_lowercase().as_str() {
                "nt" => h.nt = v,
                "st" => h.st = v,
                "nts" => h.nts = v,
                "usn" => h.usn = v,
                "location" => h.location = v,
                "cache-control" => h.cache_control = v,
                "server" => h.server = v,
                k if k.eq_ignore_ascii_case(RADIO_HEADER) => h.radio = v,
                _ => {}
            }
        }
    }
    let target = if notify { h.nt } else { h.st }?;
    if target != SEARCH_TARGET {
        return None;
    }
    let kind = if !notify {
        SsdpKind::Response
    } else if h.nts.is_some_and(|v| v.eq_ignore_ascii_case("ssdp:byebye")) {
        SsdpKind::Byebye
    } else {
        SsdpKind::Alive
    };
    let usn = h.usn?;
    let uuid = usn
        .strip_prefix("uuid:")
        .unwrap_or(usn)
        .split("::")
        .next()?
        .to_string();
    if uuid.is_empty() {
        return None;
    }
    let port = h
        .location
        .and_then(|l| {
            let rest = l.split("://").nth(1).unwrap_or(l);
            let hostport = rest.split('/').next()?;
            hostport.rsplit_once(':').and_then(|(_, p)| p.parse().ok())
        })
        .unwrap_or(DEFAULT_PORT);
    let max_age = h
        .cache_control
        .and_then(|c| {
            c.split(',').find_map(|part| {
                let (k, v) = part.split_once('=')?;
                (k.trim().eq_ignore_ascii_case("max-age"))
                    .then(|| v.trim().parse().ok())
                    .flatten()
            })
        })
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_MAX_AGE);
    Some(SsdpMessage {
        kind,
        uuid,
        port,
        server: h.server.unwrap_or_default().to_owned(),
        radio: h.radio.unwrap_or_default().to_owned(),
        max_age,
    })
}

/// A server found on the network.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The server id.
    pub uuid: String,
    /// Its address (the source of its answer, IPv4 preferred).
    pub ip: IpAddr,
    /// Its port.
    pub port: u16,
    /// The `SERVER` text (SSDP), or empty.
    pub server: String,
    /// The radio that the server names, or empty.
    pub radio: String,
    /// When the entry ages out.
    pub expires: Instant,
}

impl Found {
    /// `host:port`.
    pub fn address(&self) -> String {
        sdrkit_core::net::show(&self.ip.to_string(), self.port)
    }
}

/// Collects answers: one entry per server id, IPv4 preferred. A byebye removes it, and entries age out.
#[derive(Clone, Debug, Default)]
pub struct Collector {
    entries: HashMap<String, Found>,
}

impl Collector {
    /// Takes an SSDP message from `from`.
    pub fn ssdp(&mut self, m: &SsdpMessage, from: IpAddr, now: Instant) {
        if m.kind == SsdpKind::Byebye {
            self.entries.remove(&m.uuid);
            return;
        }
        self.add(
            Found {
                uuid: m.uuid.clone(),
                ip: from,
                port: m.port,
                server: m.server.clone(),
                radio: m.radio.clone(),
                expires: now + m.max_age,
            },
            now,
        );
    }
    /// Adds an entry, merged by uuid. An IPv4 address wins over IPv6, and a known server text is kept.
    pub fn add(&mut self, f: Found, now: Instant) {
        self.expire(now);
        match self.entries.get_mut(&f.uuid) {
            Some(e) => {
                if f.ip.is_ipv4() || !e.ip.is_ipv4() {
                    e.ip = f.ip;
                    e.port = f.port;
                }
                if e.server.is_empty() {
                    e.server = f.server;
                }
                if e.radio.is_empty() {
                    e.radio = f.radio;
                }
                e.expires = e.expires.max(f.expires);
            }
            None => {
                self.entries.insert(f.uuid.clone(), f);
            }
        }
    }
    /// Drops entries past their age.
    pub fn expire(&mut self, now: Instant) {
        self.entries.retain(|_, e| e.expires > now);
    }
    /// The servers, by uuid.
    pub fn list(&self) -> Vec<Found> {
        let mut v: Vec<Found> = self.entries.values().cloned().collect();
        v.sort_by(|a, b| a.uuid.cmp(&b.uuid));
        v
    }
}

/// The sockets a search to `target` sends from: for a multicast group, one per interface of `net` that has an address of the
/// family of the group. If there is none, one socket on the default interface.
pub(crate) fn sockets_for(net: &dyn Transport, target: SocketAddr) -> Vec<Box<dyn Datagram>> {
    let any = match target {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    let ttl = target.is_ipv4().then_some(2);
    let mut v = Vec::new();
    if target.ip().is_multicast() {
        for i in net.interfaces() {
            let fits = if target.is_ipv4() {
                i.v4.is_some()
            } else {
                i.v6 && i.index > 0
            };
            if fits {
                let opts = DatagramOptions {
                    multicast_ttl: ttl,
                    multicast_interface: Some(i),
                    ..DatagramOptions::at(any)
                };
                if let Ok(s) = net.bind(&opts) {
                    v.push(s);
                }
            }
        }
    }
    if v.is_empty() {
        let opts = DatagramOptions {
            multicast_ttl: ttl.filter(|_| target.ip().is_multicast()),
            ..DatagramOptions::at(any)
        };
        if let Ok(s) = net.bind(&opts) {
            v.push(s);
        }
    }
    for s in &mut v {
        let _ = s.set_read_timeout(Some(POLL));
    }
    v
}

/// The system does not allow this program to use the local network.
fn denied(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::PermissionDenied)
        || matches!(e.raw_os_error(), Some(1 | 13 | 65))
}

/// Why no search went out.
fn not_sent(not_allowed: bool) -> SoapyError {
    if not_allowed {
        SoapyError::SearchNotAllowed
    } else {
        SoapyError::SearchNotSent
    }
}

/// Searches by SSDP: sends M-SEARCH to each target twice, 200 ms apart, and collects answers (and NOTIFYs) for `collect`.
/// `targets` is normally [`SSDP_V4`] and [`SSDP_V6`].
///
/// # Errors
///
/// [`SoapyError::SearchNotAllowed`] or [`SoapyError::SearchNotSent`] if no search went out.
pub fn ssdp_search(
    net: &dyn Transport,
    targets: &[SocketAddr],
    collect: Duration,
) -> Result<Vec<Found>, SoapyError> {
    let msg = m_search();
    let mut socks = Vec::new();
    let mut sent_any = false;
    let mut not_allowed = false;
    for t in targets {
        for mut s in sockets_for(net, *t) {
            match s.send_to(msg.as_bytes(), *t) {
                Ok(()) => sent_any = true,
                Err(e) => not_allowed |= denied(&e),
            }
            socks.push((s, *t));
        }
    }
    if !sent_any {
        return Err(not_sent(not_allowed));
    }
    let mut c = Collector::default();
    let start = Instant::now();
    let mut resent = false;
    let mut buf = [0u8; 2048];
    while start.elapsed() < collect {
        if !resent && start.elapsed() >= Duration::from_millis(200) {
            resent = true;
            for (s, t) in &mut socks {
                let _ = s.send_to(msg.as_bytes(), *t);
            }
        }
        for (s, _) in &mut socks {
            while let Ok((n, from)) = s.recv_from(&mut buf) {
                if let Some(m) = parse_ssdp(&String::from_utf8_lossy(&buf[..n])) {
                    c.ssdp(&m, from.ip(), Instant::now());
                }
            }
        }
    }
    Ok(c.list())
}

// ---- DNS-SD ----

/// A DNS-SD service as found: its instance, host, port, address and TXT keys.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Service {
    /// The instance name.
    pub instance: String,
    /// The SRV target host.
    pub host: String,
    /// The SRV port.
    pub port: u16,
    /// Addresses of the host (A and AAAA records).
    pub ips: Vec<IpAddr>,
    /// The `uuid` key of its TXT record.
    pub uuid: Option<String>,
}

/// The mDNS query for the service type's PTR records (a one-shot query from an ordinary port: answers come back by unicast).
pub fn mdns_query() -> Vec<u8> {
    let mut q = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in SERVICE.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&[0, 12, 0, 1]); // PTR, IN
    q
}

fn read_name(msg: &[u8], mut pos: usize) -> Option<(String, usize)> {
    let mut parts = Vec::new();
    let mut end = None;
    let mut jumps = 0;
    loop {
        let len = *msg.get(pos)? as usize;
        if len == 0 {
            pos += 1;
            break;
        }
        if len & 0xc0 == 0xc0 {
            let off = ((len & 0x3f) << 8) | *msg.get(pos + 1)? as usize;
            end.get_or_insert(pos + 2);
            jumps += 1;
            if jumps > 32 {
                return None;
            }
            pos = off;
            continue;
        }
        let label = msg.get(pos + 1..pos + 1 + len)?;
        parts.push(String::from_utf8_lossy(label).into_owned());
        pos += 1 + len;
    }
    Some((parts.join("."), end.unwrap_or(pos)))
}

fn u16_at(m: &[u8], p: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*m.get(p)?, *m.get(p + 1)?]))
}

/// Reads the `_soapy._tcp` services of an mDNS answer.
pub fn parse_mdns(msg: &[u8]) -> Vec<Service> {
    let mut out: HashMap<String, Service> = HashMap::new();
    let mut hosts: HashMap<String, Vec<IpAddr>> = HashMap::new();
    let Some(qd) = u16_at(msg, 4) else {
        return Vec::new();
    };
    let (Some(an), Some(ns), Some(ar)) = (u16_at(msg, 6), u16_at(msg, 8), u16_at(msg, 10)) else {
        return Vec::new();
    };
    let mut pos = 12;
    for _ in 0..qd {
        let Some((_, p)) = read_name(msg, pos) else {
            return Vec::new();
        };
        pos = p + 4;
    }
    let svc = SERVICE.to_lowercase();
    for _ in 0..(an as usize + ns as usize + ar as usize) {
        let Some((name, p)) = read_name(msg, pos) else {
            break;
        };
        let (Some(ty), Some(rdlen)) = (u16_at(msg, p), u16_at(msg, p + 8)) else {
            break;
        };
        let rd = p + 10;
        let rdlen = rdlen as usize;
        if msg.len() < rd + rdlen {
            break;
        }
        let lname = name.to_lowercase();
        match ty {
            12 if lname == svc => {
                if let Some((inst, _)) = read_name(msg, rd) {
                    out.entry(inst.to_lowercase()).or_insert_with(|| Service {
                        instance: inst,
                        ..Default::default()
                    });
                }
            }
            33 if lname.ends_with(&svc) => {
                if let (Some(port), Some((host, _))) = (u16_at(msg, rd + 4), read_name(msg, rd + 6))
                {
                    let e = out.entry(lname.clone()).or_insert_with(|| Service {
                        instance: name.clone(),
                        ..Default::default()
                    });
                    e.port = port;
                    e.host = host;
                }
            }
            16 if lname.ends_with(&svc) => {
                let e = out.entry(lname.clone()).or_insert_with(|| Service {
                    instance: name.clone(),
                    ..Default::default()
                });
                let mut i = rd;
                while i < rd + rdlen {
                    let l = msg[i] as usize;
                    let Some(s) = msg.get(i + 1..(i + 1 + l).min(rd + rdlen)) else {
                        break;
                    };
                    let s = String::from_utf8_lossy(s);
                    let (k, v) = s.split_once('=').unwrap_or((&s, ""));
                    if k.eq_ignore_ascii_case("uuid") {
                        e.uuid = Some(v.to_string());
                    }
                    i += 1 + l;
                }
            }
            1 if rdlen == 4 => {
                let b = &msg[rd..rd + 4];
                hosts
                    .entry(lname)
                    .or_default()
                    .push(IpAddr::V4(Ipv4Addr::new(b[0], b[1], b[2], b[3])));
            }
            28 if rdlen == 16 => {
                let mut b = [0u8; 16];
                b.copy_from_slice(&msg[rd..rd + 16]);
                hosts
                    .entry(lname)
                    .or_default()
                    .push(IpAddr::V6(Ipv6Addr::from(b)));
            }
            _ => {}
        }
        pos = rd + rdlen;
    }
    let mut v: Vec<Service> = out
        .into_values()
        .map(|mut s| {
            if let Some(ips) = hosts.get(&s.host.to_lowercase()) {
                s.ips = ips.clone();
            }
            s
        })
        .collect();
    v.sort_by(|a, b| a.instance.cmp(&b.instance));
    v
}

/// A DNS-SD service as a found server: it needs a `uuid` TXT key and a port. The address is its A record, else its AAAA record,
/// else the source of the answer.
pub fn service_found(s: &Service, from: IpAddr, now: Instant) -> Option<Found> {
    let uuid = s.uuid.clone().filter(|u| !u.is_empty())?;
    if s.port == 0 {
        return None;
    }
    let ip = s
        .ips
        .iter()
        .find(|i| i.is_ipv4())
        .or_else(|| s.ips.first())
        .copied()
        .unwrap_or(from);
    Some(Found {
        uuid,
        ip,
        port: s.port,
        server: String::new(),
        radio: String::new(),
        expires: now + DEFAULT_MAX_AGE,
    })
}

/// Searches by DNS-SD: one mDNS query to each target, and the answers collected for `collect`. `targets` is normally
/// [`MDNS_V4`].
///
/// # Errors
///
/// [`SoapyError::SearchNotAllowed`] or [`SoapyError::SearchNotSent`] if no query went out.
pub fn dnssd_search(
    net: &dyn Transport,
    targets: &[SocketAddr],
    collect: Duration,
) -> Result<Vec<Found>, SoapyError> {
    let q = mdns_query();
    let mut socks = Vec::new();
    let mut not_allowed = false;
    for t in targets {
        for mut s in sockets_for(net, *t) {
            match s.send_to(&q, *t) {
                Ok(()) => socks.push(s),
                Err(e) => not_allowed |= denied(&e),
            }
        }
    }
    if socks.is_empty() {
        return Err(not_sent(not_allowed));
    }
    let mut c = Collector::default();
    let start = Instant::now();
    let mut buf = [0u8; 9000];
    while start.elapsed() < collect {
        for s in &mut socks {
            while let Ok((n, from)) = s.recv_from(&mut buf) {
                for svc in parse_mdns(&buf[..n]) {
                    if let Some(f) = service_found(&svc, from.ip(), Instant::now()) {
                        c.add(f, Instant::now());
                    }
                }
            }
        }
    }
    Ok(c.list())
}

/// Merges the results of both searches by uuid (IPv4 preferred, the SSDP server text kept).
pub fn merge(a: &[Found], b: &[Found]) -> Vec<Found> {
    let mut c = Collector::default();
    let now = Instant::now();
    for f in a.iter().chain(b) {
        let mut f = f.clone();
        f.expires = f.expires.max(now + Duration::from_millis(1));
        c.add(f, now);
    }
    c.list()
}

/// Both searches on the standard groups through `net`, merged.
///
/// # Errors
///
/// The error of the SSDP search if neither search went out.
pub fn search(net: &dyn Transport) -> Result<Vec<Found>, SoapyError> {
    let (s, d) = std::thread::scope(|scope| {
        let ssdp = scope.spawn(|| ssdp_search(net, &[SSDP_V4, SSDP_V6], COLLECT));
        let d = dnssd_search(net, &[MDNS_V4], COLLECT);
        (ssdp.join().unwrap_or(Err(SoapyError::SearchNotSent)), d)
    });
    match (s, d) {
        (Err(e), Err(_)) => Err(e),
        (s, d) => Ok(merge(&s.unwrap_or_default(), &d.unwrap_or_default())),
    }
}
