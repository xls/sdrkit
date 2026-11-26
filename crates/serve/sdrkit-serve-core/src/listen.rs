//! Listeners: one thread per listener takes connections through [`Transport::listen`], refuses an address outside the allow
//! list or a client over `max_clients`, and starts the threads of each client it admits.

use crate::client::{self, ClientEntry};
use crate::control::Control;
use crate::hub::{lock, Inner};
use crate::protocol::Protocol;
use crate::stats::{Event, Refusal};
use crate::ServeError;
use sdrkit_core::net::{Conn, Listener, Transport};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

/// How long a listener sleeps when no connection waits.
const ACCEPT_POLL: Duration = Duration::from_millis(5);

/// A range of addresses: an address and the number of leading bits that must match (`192.168.1.0/24`, `fd00::/8`). An
/// address without a length is one host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cidr {
    net: IpAddr,
    bits: u8,
}

/// The text is not an address range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("not an address range")]
pub struct BadCidr;

impl Cidr {
    /// The range of `net` with `bits` leading bits, the bits after them cleared. An IPv4 address written as IPv6
    /// (`::ffff:a.b.c.d/104`) becomes the IPv4 range with 96 bits fewer.
    ///
    /// # Errors
    ///
    /// [`BadCidr`] if `bits` is longer than the address.
    pub fn new(net: IpAddr, bits: u8) -> Result<Cidr, BadCidr> {
        let (net, bits) = match (net, net.to_canonical()) {
            (IpAddr::V6(_), v4 @ IpAddr::V4(_)) => (v4, bits.checked_sub(96).ok_or(BadCidr)?),
            (_, n) => (n, bits),
        };
        let max = if net.is_ipv4() { 32 } else { 128 };
        if bits > max {
            return Err(BadCidr);
        }
        let net = match net {
            IpAddr::V4(a) => IpAddr::V4((u32::from(a) & mask32(bits)).into()),
            IpAddr::V6(a) => IpAddr::V6((u128::from(a) & mask128(bits)).into()),
        };
        Ok(Cidr { net, bits })
    }

    /// True if `ip` lies in the range. An IPv4 address written as IPv6 (`::ffff:a.b.c.d`) is taken as IPv4.
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.net, ip.to_canonical()) {
            (IpAddr::V4(n), IpAddr::V4(a)) => u32::from(a) & mask32(self.bits) == u32::from(n),
            (IpAddr::V6(n), IpAddr::V6(a)) => u128::from(a) & mask128(self.bits) == u128::from(n),
            _ => false,
        }
    }
}

fn mask32(bits: u8) -> u32 {
    u32::MAX.checked_shl(32 - u32::from(bits)).unwrap_or(0)
}

fn mask128(bits: u8) -> u128 {
    u128::MAX.checked_shl(128 - u32::from(bits)).unwrap_or(0)
}

impl FromStr for Cidr {
    type Err = BadCidr;
    fn from_str(s: &str) -> Result<Cidr, BadCidr> {
        let s = s.trim();
        let (addr, bits) = match s.split_once('/') {
            Some((a, b)) => (a, Some(b.parse::<u8>().map_err(|_| BadCidr)?)),
            None => (s, None),
        };
        let ip: IpAddr = addr
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse()
            .map_err(|_| BadCidr)?;
        let full = if ip.is_ipv4() { 32 } else { 128 };
        Cidr::new(ip, bits.unwrap_or(full))
    }
}

/// True if `peer` may connect: the list is empty, or the peer's address lies in a range of it. A peer without an address is
/// refused by a list that is not empty.
pub fn allowed(allow: &[Cidr], peer: Option<SocketAddr>) -> bool {
    allow.is_empty() || peer.is_some_and(|p| allow.iter().any(|c| c.contains(p.ip())))
}

/// Where and for whom a listener listens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listen {
    /// The local address (`127.0.0.1`, `0.0.0.0`, `::`).
    pub host: String,
    /// The port; 0 lets the system choose.
    pub port: u16,
    /// The ranges that may connect; empty allows every address.
    pub allow: Vec<Cidr>,
}

/// A listener that runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListenerInfo {
    /// Its id, unique in the hub.
    pub id: u64,
    /// The address it listens on, with the port the system chose.
    pub addr: SocketAddr,
}

/// The counters of one listener.
#[derive(Debug)]
pub(crate) struct ListenerShared {
    pub id: u64,
    pub addr: SocketAddr,
    pub protocol: &'static str,
    pub since: SystemTime,
    pub accepted: AtomicU64,
    pub refused_allow: AtomicU64,
    pub refused_full: AtomicU64,
    /// Bytes sent to clients of this listener that have left.
    pub closed_bytes: AtomicU64,
    pub stop: AtomicBool,
}

pub(crate) struct ListenerEntry {
    pub shared: Arc<ListenerShared>,
    pub thread: Option<JoinHandle<()>>,
}

/// Opens a listener and starts its thread.
pub(crate) fn start(
    inner: &Arc<Inner>,
    transport: &Arc<dyn Transport>,
    listen: &Listen,
    protocol: Arc<dyn Protocol>,
    control: Arc<dyn Control>,
) -> Result<ListenerInfo, ServeError> {
    let l = transport.listen(&listen.host, listen.port)?;
    let shared = Arc::new(ListenerShared {
        id: inner.next_id(),
        addr: l.local_addr(),
        protocol: protocol.name(),
        since: SystemTime::now(),
        accepted: AtomicU64::new(0),
        refused_allow: AtomicU64::new(0),
        refused_full: AtomicU64::new(0),
        closed_bytes: AtomicU64::new(0),
        stop: AtomicBool::new(false),
    });
    let info = ListenerInfo {
        id: shared.id,
        addr: shared.addr,
    };
    let run = Run {
        inner: inner.clone(),
        shared: shared.clone(),
        allow: listen.allow.clone(),
        protocol,
        control,
    };
    let thread = std::thread::Builder::new()
        .name(format!("sdrkit-serve-l{}", info.id))
        .spawn(move || run.run(l))
        .map_err(|_| ServeError::Thread)?;
    lock(&inner.listeners).push(ListenerEntry {
        shared,
        thread: Some(thread),
    });
    Ok(info)
}

struct Run {
    inner: Arc<Inner>,
    shared: Arc<ListenerShared>,
    allow: Vec<Cidr>,
    protocol: Arc<dyn Protocol>,
    control: Arc<dyn Control>,
}

impl Run {
    fn run(self, mut l: Box<dyn Listener>) {
        while !self.shared.stop.load(Ordering::Acquire) && !self.inner.stop.load(Ordering::Acquire)
        {
            housekeep(&self.inner);
            match l.accept() {
                Ok(Some(c)) => self.admit(c),
                Ok(None) => std::thread::sleep(ACCEPT_POLL),
                Err(_) => std::thread::sleep(ACCEPT_POLL * 10),
            }
        }
    }

    fn refuse(&self, conn: &dyn Conn, why: Refusal) {
        let n = match why {
            Refusal::NotAllowed => &self.shared.refused_allow,
            Refusal::Full => &self.shared.refused_full,
        };
        n.fetch_add(1, Ordering::Relaxed);
        conn.shutdown();
        self.inner.event(Event::Refused {
            listener: self.shared.id,
            peer: conn.peer_addr(),
            reason: why,
        });
    }

    fn admit(&self, conn: Box<dyn Conn>) {
        let peer = conn.peer_addr();
        if !allowed(&self.allow, peer) {
            return self.refuse(conn.as_ref(), Refusal::NotAllowed);
        }
        let mut clients = lock(&self.inner.clients);
        let active = clients
            .iter()
            .filter(|c| !c.shared.closed.load(Ordering::Acquire))
            .count();
        if active >= self.inner.config.max_clients {
            drop(clients);
            return self.refuse(conn.as_ref(), Refusal::Full);
        }
        let probe = conn.try_clone();
        match client::start(
            &self.inner,
            conn,
            self.shared.id,
            &self.protocol,
            &self.control,
        ) {
            // The client joins once it asks for a setting or a stream (`ClientHandle::engage`).
            Ok(entry) => {
                clients.push(entry);
                self.shared.accepted.fetch_add(1, Ordering::Relaxed);
            }
            Err(_) => {
                drop(clients);
                if let Ok(c) = probe {
                    self.refuse(c.as_ref(), Refusal::Full);
                }
            }
        }
    }
}

/// The most clients that left that snapshots show to the owner.
const DEPARTED_MAX: usize = 64;
/// How long snapshots show a client that left.
const DEPARTED_KEEP: std::time::Duration = std::time::Duration::from_secs(5);

/// Drops the rings that the feeder retired, and joins and removes the clients whose threads have ended. A client that
/// left with requests not carried out is shown by snapshots for [`DEPARTED_KEEP`].
pub(crate) fn housekeep(inner: &Inner) {
    if let Ok(mut r) = inner.retired.try_lock() {
        while r.pop().is_ok() {}
    }
    lock(&inner.departed).retain(|(_, at)| at.elapsed() < DEPARTED_KEEP);
    let done: Vec<ClientEntry> = {
        let mut clients = lock(&inner.clients);
        let mut done = Vec::new();
        let mut i = 0;
        while i < clients.len() {
            if clients[i].finished() {
                done.push(clients.swap_remove(i));
            } else {
                i += 1;
            }
        }
        done
    };
    if done.is_empty() {
        return;
    }
    let listeners = lock(&inner.listeners);
    for mut c in done {
        c.join();
        let refused =
            c.shared.engaged.load(Ordering::Acquire) && !lock(&c.shared.not_done).is_empty();
        if refused {
            let mut d = lock(&inner.departed);
            if d.len() < DEPARTED_MAX {
                d.push((c.shared.clone(), std::time::Instant::now()));
            }
        }
        if let Some(l) = listeners.iter().find(|l| l.shared.id == c.shared.listener) {
            l.shared
                .closed_bytes
                .fetch_add(c.shared.bytes.load(Ordering::Relaxed), Ordering::Relaxed);
        }
    }
}

/// Stops listener `id` and joins its thread.
pub(crate) fn stop(inner: &Inner, id: u64) -> bool {
    let entry = {
        let mut ls = lock(&inner.listeners);
        let Some(i) = ls.iter().position(|l| l.shared.id == id) else {
            return false;
        };
        ls.remove(i)
    };
    finish(entry);
    true
}

/// Stops every listener and joins its thread.
pub(crate) fn stop_all(inner: &Inner) {
    let all: Vec<ListenerEntry> = lock(&inner.listeners).drain(..).collect();
    for l in all {
        finish(l);
    }
}

fn finish(mut l: ListenerEntry) {
    l.shared.stop.store(true, Ordering::Release);
    if let Some(t) = l.thread.take() {
        let _ = t.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> Cidr {
        s.parse().unwrap()
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn ranges_parse_and_match() {
        let lan = c("192.168.10.0/24");
        assert!(lan.contains(ip("192.168.10.9")));
        assert!(!lan.contains(ip("192.168.11.9")));
        assert!(lan.contains(ip("::ffff:192.168.10.200")));
        assert_eq!(c("192.168.10.77/24"), lan);
        assert!(c("10.0.0.1").contains(ip("10.0.0.1")));
        assert!(!c("10.0.0.1").contains(ip("10.0.0.2")));
        assert!(c("0.0.0.0/0").contains(ip("8.8.8.8")));
        assert!(!c("0.0.0.0/0").contains(ip("::1")));
        assert!(c("fd00::/8").contains(ip("fd12::5")));
        assert!(!c("fd00::/8").contains(ip("fe80::1")));
        assert!(c("[::1]").contains(ip("::1")));
        assert!(c("::ffff:10.1.2.3/104").contains(ip("10.9.9.9")));
        for bad in ["", "10.0.0.0/33", "::/129", "host/8", "10.0.0.0/x"] {
            assert_eq!(bad.parse::<Cidr>(), Err(BadCidr), "{bad}");
        }
    }

    #[test]
    fn an_empty_list_allows_all_and_a_list_needs_an_address() {
        let p: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        assert!(allowed(&[], Some(p)) && allowed(&[], None));
        assert!(allowed(&[c("127.0.0.0/8")], Some(p)));
        assert!(!allowed(&[c("10.0.0.0/8")], Some(p)));
        assert!(!allowed(&[c("127.0.0.0/8")], None));
    }
}
