//! A [`Transport`] in memory, with servers that a test scripts.

use super::{Conn, Datagram, DatagramOptions, Listener, NetError, Transport};
use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The most bytes that wait in one direction of a connection. A write waits for room beyond it.
const PIPE_CAP: usize = 4 << 20;

type Handler = Arc<dyn Fn(Box<dyn Conn>) + Send + Sync>;

/// The datagrams that wait at one socket, with their senders.
type Inbox = Queue<(Vec<u8>, SocketAddr)>;

/// A [`Transport`] in memory, for tests. A test scripts a server with [`FakeTransport::serve`], or listens through
/// [`Transport::listen`]. A connection to an address with no server is refused. A host name resolves only if it is an IP
/// address, `localhost`, or a name given to [`FakeTransport::add_host`]. Datagrams go between sockets of the same transport.
/// Clones share the same network.
#[derive(Clone, Default)]
pub struct FakeTransport {
    inner: Arc<Mutex<State>>,
}

impl std::fmt::Debug for FakeTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FakeTransport")
    }
}

#[derive(Default)]
struct State {
    servers: HashMap<SocketAddr, Handler>,
    listeners: HashMap<SocketAddr, Arc<Queue<Box<dyn Conn>>>>,
    hosts: HashMap<String, IpAddr>,
    silent: HashSet<IpAddr>,
    sockets: HashMap<SocketAddr, Arc<Inbox>>,
    /// The multicast groups that each datagram socket joined.
    groups: HashMap<SocketAddr, Vec<IpAddr>>,
    next_port: u16,
    connects: u64,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl FakeTransport {
    /// A network with no servers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Serves `host:port`: each connection runs `server` on its own thread with the server end of the connection.
    /// A second call for the same address replaces the server.
    ///
    /// # Panics
    ///
    /// If `host` does not resolve.
    pub fn serve(
        &self,
        host: &str,
        port: u16,
        server: impl Fn(Box<dyn Conn>) + Send + Sync + 'static,
    ) {
        let addr = self
            .resolve(host, port)
            .expect("the host of a fake server resolves");
        lock(&self.inner).servers.insert(addr, Arc::new(server));
    }

    /// Stops serving `host:port`. Later connections are refused. Open connections stay.
    pub fn stop_serving(&self, host: &str, port: u16) {
        if let Ok(addr) = self.resolve(host, port) {
            lock(&self.inner).servers.remove(&addr);
        }
    }

    /// Lets the name `name` resolve to `ip`.
    pub fn add_host(&self, name: &str, ip: IpAddr) {
        lock(&self.inner)
            .hosts
            .insert(name.to_ascii_lowercase(), ip);
    }

    /// Makes `host` silent: a connection to it waits for its whole timeout and fails with [`NetError::TimedOut`].
    ///
    /// # Panics
    ///
    /// If `host` does not resolve.
    pub fn silence(&self, host: &str) {
        let addr = self.resolve(host, 0).expect("the silent host resolves");
        lock(&self.inner).silent.insert(addr.ip());
    }

    /// The number of connections made so far, to any address.
    pub fn connects(&self) -> u64 {
        lock(&self.inner).connects
    }

    fn free_port(state: &mut State) -> u16 {
        if state.next_port < 40_000 {
            state.next_port = 40_000;
        }
        state.next_port = state.next_port.wrapping_add(1).max(40_000);
        state.next_port
    }
}

impl Transport for FakeTransport {
    fn connect(
        &self,
        host: &str,
        port: u16,
        tls: bool,
        timeout: Duration,
    ) -> Result<Box<dyn Conn>, NetError> {
        let addr = self.resolve(host, port)?;
        if tls {
            return Err(NetError::TlsUnavailable);
        }
        let mut state = lock(&self.inner);
        if state.silent.contains(&addr.ip()) {
            drop(state);
            std::thread::sleep(timeout);
            return Err(NetError::TimedOut);
        }
        let target = state
            .servers
            .get(&addr)
            .cloned()
            .map(Ok)
            .or_else(|| state.listeners.get(&addr).cloned().map(Err));
        let (mut client, mut server) = FakeConn::pair();
        client.peer = Some(addr);
        server.peer = Some(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            Self::free_port(&mut state),
        ));
        match target {
            Some(Ok(handler)) => {
                state.connects += 1;
                drop(state);
                std::thread::Builder::new()
                    .name("fake-server".into())
                    .spawn(move || handler(Box::new(server)))
                    .map_err(NetError::from)?;
            }
            Some(Err(queue)) => {
                state.connects += 1;
                queue.push(Box::new(server));
            }
            None => return Err(NetError::Refused),
        }
        client
            .end
            .write_timeout
            .store(nanos(Some(timeout)), Ordering::Relaxed);
        Ok(Box::new(client))
    }

    fn datagram(&self, host: &str, port: u16) -> Result<Box<dyn Datagram>, NetError> {
        let peer = self.resolve(host, port)?;
        let mut d = self.open_socket(&DatagramOptions::any_v4())?;
        d.peer = Some(peer);
        Ok(Box::new(d))
    }

    fn bind(&self, options: &DatagramOptions) -> Result<Box<dyn Datagram>, NetError> {
        Ok(Box::new(self.open_socket(options)?))
    }

    fn listen(&self, host: &str, port: u16) -> Result<Box<dyn Listener>, NetError> {
        let mut addr = self.resolve(host, port)?;
        let mut state = lock(&self.inner);
        if port == 0 {
            addr.set_port(Self::free_port(&mut state));
        }
        if state.listeners.contains_key(&addr) || state.servers.contains_key(&addr) {
            return Err(NetError::Io {
                kind: io::ErrorKind::AddrInUse,
            });
        }
        let queue = Arc::new(Queue::default());
        state.listeners.insert(addr, queue.clone());
        Ok(Box::new(FakeListener {
            addr,
            queue,
            net: self.inner.clone(),
        }))
    }

    fn resolve(&self, host: &str, port: u16) -> Result<SocketAddr, NetError> {
        let h = host.trim_start_matches('[').trim_end_matches(']');
        if let Ok(ip) = h.parse::<IpAddr>() {
            return Ok(SocketAddr::new(ip, port));
        }
        if h.eq_ignore_ascii_case("localhost") {
            return Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
        }
        lock(&self.inner)
            .hosts
            .get(&h.to_ascii_lowercase())
            .map(|ip| SocketAddr::new(*ip, port))
            .ok_or(NetError::NoSuchHost)
    }
}

impl FakeTransport {
    fn open_socket(&self, options: &DatagramOptions) -> Result<FakeDatagram, NetError> {
        let mut addr = options.local;
        let mut state = lock(&self.inner);
        if addr.port() == 0 {
            addr.set_port(Self::free_port(&mut state));
        }
        if state.sockets.contains_key(&addr) {
            return Err(NetError::Io {
                kind: io::ErrorKind::AddrInUse,
            });
        }
        let queue = Arc::new(Queue::default());
        state.sockets.insert(addr, queue.clone());
        if !options.join.is_empty() {
            state.groups.insert(addr, options.join.clone());
        }
        Ok(FakeDatagram {
            shared: Arc::new(SocketShared {
                addr,
                recv_buffer: options.recv_buffer,
                queue,
                net: self.inner.clone(),
                read_timeout: AtomicU64::new(0),
            }),
            peer: None,
        })
    }
}

/// A queue that a reader waits on.
struct Queue<T> {
    items: Mutex<VecDeque<T>>,
    ready: Condvar,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Queue {
            items: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
        }
    }
}

impl<T> Queue<T> {
    fn push(&self, item: T) {
        lock(&self.items).push_back(item);
        self.ready.notify_all();
    }
    fn pop(&self, timeout: Option<Duration>) -> Option<T> {
        let deadline = timeout.map(|t| Instant::now() + t);
        let mut items = lock(&self.items);
        loop {
            if let Some(x) = items.pop_front() {
                return Some(x);
            }
            items = match deadline {
                None => self
                    .ready
                    .wait(items)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                Some(d) => {
                    let left = d.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return None;
                    }
                    self.ready
                        .wait_timeout(items, left)
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .0
                }
            };
        }
    }
}

/// One direction of a connection.
#[derive(Default)]
struct Pipe {
    state: Mutex<PipeState>,
    changed: Condvar,
}

#[derive(Default)]
struct PipeState {
    bytes: VecDeque<u8>,
    closed: bool,
}

impl Pipe {
    fn close(&self) {
        lock(&self.state).closed = true;
        self.changed.notify_all();
    }

    /// Waits on the pipe until `ready` holds or the deadline passes. It returns the guard and false on a timeout.
    fn wait<'a>(
        &'a self,
        mut s: MutexGuard<'a, PipeState>,
        deadline: Option<Instant>,
        ready: impl Fn(&PipeState) -> bool,
    ) -> (MutexGuard<'a, PipeState>, bool) {
        while !ready(&s) {
            s = match deadline {
                None => self
                    .changed
                    .wait(s)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
                Some(d) => {
                    let left = d.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return (s, false);
                    }
                    self.changed
                        .wait_timeout(s, left)
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .0
                }
            };
        }
        (s, true)
    }
}

/// The shared end of a connection: every handle of one side holds it.
struct End {
    rx: Arc<Pipe>,
    tx: Arc<Pipe>,
    read_timeout: AtomicU64,
    write_timeout: AtomicU64,
}

impl Drop for End {
    fn drop(&mut self) {
        self.rx.close();
        self.tx.close();
    }
}

fn nanos(t: Option<Duration>) -> u64 {
    t.map_or(0, |d| {
        u64::try_from(d.as_nanos()).unwrap_or(u64::MAX).max(1)
    })
}

fn deadline(n: &AtomicU64) -> Option<Instant> {
    match n.load(Ordering::Relaxed) {
        0 => None,
        n => Some(Instant::now() + Duration::from_nanos(n)),
    }
}

struct FakeConn {
    end: Arc<End>,
    peer: Option<SocketAddr>,
}

impl FakeConn {
    fn pair() -> (FakeConn, FakeConn) {
        let a = Arc::new(Pipe::default());
        let b = Arc::new(Pipe::default());
        let end = |rx: &Arc<Pipe>, tx: &Arc<Pipe>| FakeConn {
            end: Arc::new(End {
                rx: rx.clone(),
                tx: tx.clone(),
                read_timeout: AtomicU64::new(0),
                write_timeout: AtomicU64::new(0),
            }),
            peer: None,
        };
        (end(&a, &b), end(&b, &a))
    }
}

impl Read for FakeConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let pipe = &self.end.rx;
        let s = lock(&pipe.state);
        let (mut s, ready) = pipe.wait(s, deadline(&self.end.read_timeout), |s| {
            !s.bytes.is_empty() || s.closed
        });
        if !ready {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let n = buf.len().min(s.bytes.len());
        for (dst, src) in buf.iter_mut().zip(s.bytes.drain(..n)) {
            *dst = src;
        }
        drop(s);
        pipe.changed.notify_all();
        Ok(n)
    }
}

impl Write for FakeConn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let pipe = &self.end.tx;
        let s = lock(&pipe.state);
        let (mut s, ready) = pipe.wait(s, deadline(&self.end.write_timeout), |s| {
            s.bytes.len() < PIPE_CAP || s.closed
        });
        if s.closed {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if !ready {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let n = buf.len().min(PIPE_CAP - s.bytes.len());
        s.bytes.extend(&buf[..n]);
        drop(s);
        pipe.changed.notify_all();
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Conn for FakeConn {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        self.end
            .read_timeout
            .store(nanos(timeout), Ordering::Relaxed);
        Ok(())
    }
    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        self.end
            .write_timeout
            .store(nanos(timeout), Ordering::Relaxed);
        Ok(())
    }
    fn set_recv_buffer(&mut self, _bytes: usize) -> Result<(), NetError> {
        Ok(())
    }
    fn try_clone(&self) -> Result<Box<dyn Conn>, NetError> {
        Ok(Box::new(FakeConn {
            end: self.end.clone(),
            peer: self.peer,
        }))
    }
    fn shutdown(&self) {
        self.end.rx.close();
        self.end.tx.close();
    }
    fn peer_addr(&self) -> Option<SocketAddr> {
        self.peer
    }
}

struct FakeListener {
    addr: SocketAddr,
    queue: Arc<Queue<Box<dyn Conn>>>,
    net: Arc<Mutex<State>>,
}

impl Listener for FakeListener {
    fn local_addr(&self) -> SocketAddr {
        self.addr
    }
    fn accept(&mut self) -> Result<Option<Box<dyn Conn>>, NetError> {
        Ok(lock(&self.queue.items).pop_front())
    }
}

impl Drop for FakeListener {
    fn drop(&mut self) {
        lock(&self.net).listeners.remove(&self.addr);
    }
}

struct SocketShared {
    addr: SocketAddr,
    recv_buffer: Option<usize>,
    queue: Arc<Inbox>,
    net: Arc<Mutex<State>>,
    read_timeout: AtomicU64,
}

impl Drop for SocketShared {
    fn drop(&mut self) {
        let mut net = lock(&self.net);
        net.sockets.remove(&self.addr);
        net.groups.remove(&self.addr);
    }
}

struct FakeDatagram {
    shared: Arc<SocketShared>,
    peer: Option<SocketAddr>,
}

impl FakeDatagram {
    fn sender(&self) -> SocketAddr {
        let a = self.shared.addr;
        if a.ip().is_unspecified() {
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), a.port())
        } else {
            a
        }
    }
}

/// True if a datagram to `to` reaches the socket at `socket`, which joined the multicast groups `joined`: the same port,
/// and the address of the socket, a broadcast address, or a multicast group that the socket joined.
fn reaches(socket: SocketAddr, joined: &[IpAddr], to: SocketAddr) -> bool {
    if socket.port() != to.port() {
        return false;
    }
    if to.ip().is_multicast() {
        return joined.contains(&to.ip());
    }
    let broadcast = match to.ip() {
        IpAddr::V4(v4) => v4.is_broadcast() || v4.octets()[3] == 255,
        IpAddr::V6(_) => false,
    };
    broadcast || socket.ip() == to.ip() || socket.ip().is_unspecified()
}

impl Datagram for FakeDatagram {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        let to = self.peer.ok_or(io::ErrorKind::NotConnected)?;
        self.send_to(bytes, to)
    }
    fn send_to(&mut self, bytes: &[u8], to: SocketAddr) -> io::Result<()> {
        let from = self.sender();
        let targets: Vec<_> = {
            let net = lock(&self.shared.net);
            net.sockets
                .iter()
                .filter(|(a, _)| {
                    let joined = net.groups.get(*a).map_or(&[][..], Vec::as_slice);
                    **a != self.shared.addr && reaches(**a, joined, to)
                })
                .map(|(_, q)| q.clone())
                .collect()
        };
        for q in targets {
            q.push((bytes.to_vec(), from));
        }
        Ok(())
    }
    fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let timeout = match self.shared.read_timeout.load(Ordering::Relaxed) {
            0 => None,
            n => Some(Duration::from_nanos(n)),
        };
        let (bytes, from) = self
            .shared
            .queue
            .pop(timeout)
            .ok_or(io::ErrorKind::WouldBlock)?;
        let n = bytes.len().min(buf.len());
        buf[..n].copy_from_slice(&bytes[..n]);
        Ok((n, from))
    }
    fn try_recv_from(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        Ok(lock(&self.shared.queue.items)
            .pop_front()
            .map(|(bytes, from)| {
                let n = bytes.len().min(buf.len());
                buf[..n].copy_from_slice(&bytes[..n]);
                (n, from)
            }))
    }
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> Result<(), NetError> {
        self.shared
            .read_timeout
            .store(nanos(timeout), Ordering::Relaxed);
        Ok(())
    }
    fn local_addr(&self) -> SocketAddr {
        self.shared.addr
    }
    fn recv_buffer(&self) -> Option<usize> {
        self.shared.recv_buffer
    }
    fn try_clone(&self) -> Result<Box<dyn Datagram>, NetError> {
        Ok(Box::new(FakeDatagram {
            shared: self.shared.clone(),
            peer: self.peer,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    #[test]
    fn a_scripted_server_answers_each_line() {
        let net = FakeTransport::new();
        net.serve("127.0.0.1", 1234, |mut c| {
            let mut r = BufReader::new(c.try_clone().unwrap());
            let mut line = String::new();
            while r.read_line(&mut line).unwrap_or(0) > 0 {
                c.write_all(line.to_uppercase().as_bytes()).unwrap();
                line.clear();
            }
        });
        let mut c = net
            .connect("localhost", 1234, false, Duration::from_secs(1))
            .unwrap();
        c.write_all(b"hello\n").unwrap();
        let mut r = BufReader::new(c.try_clone().unwrap());
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        assert_eq!(line, "HELLO\n");
        assert_eq!(net.connects(), 1);
    }

    #[test]
    fn no_server_is_refused_and_unknown_names_do_not_resolve() {
        let net = FakeTransport::new();
        let t = Duration::from_millis(50);
        assert_eq!(
            net.connect("127.0.0.1", 9, false, t).err(),
            Some(NetError::Refused)
        );
        assert_eq!(
            net.connect("radio.invalid", 9, false, t).err(),
            Some(NetError::NoSuchHost)
        );
        net.add_host("radio.lan", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)));
        net.serve("radio.lan", 9, |_| {});
        assert!(net.connect("10.0.0.2", 9, false, t).is_ok());
        assert_eq!(
            net.connect("radio.lan", 9, true, t).err(),
            Some(NetError::TlsUnavailable)
        );
        net.silence("10.0.0.3");
        assert_eq!(
            net.connect("10.0.0.3", 9, false, t).err(),
            Some(NetError::TimedOut)
        );
    }

    #[test]
    fn a_shutdown_wakes_a_blocked_read_and_a_drop_ends_the_stream() {
        let net = FakeTransport::new();
        let mut l = net.listen("127.0.0.1", 0).unwrap();
        let port = l.local_addr().port();
        let mut c = net
            .connect("127.0.0.1", port, false, Duration::from_secs(1))
            .unwrap();
        let mut server = l.accept().unwrap().unwrap();
        let shut = c.try_clone().unwrap();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            shut.shutdown();
        });
        let mut buf = [0u8; 4];
        assert_eq!(c.read(&mut buf).unwrap(), 0);
        t.join().unwrap();
        assert!(server.write_all(b"x").is_err());
    }

    #[test]
    fn a_read_times_out() {
        let net = FakeTransport::new();
        net.serve("127.0.0.1", 7, |c| {
            std::thread::sleep(Duration::from_millis(200));
            drop(c);
        });
        let mut c = net
            .connect("127.0.0.1", 7, false, Duration::from_secs(1))
            .unwrap();
        c.set_read_timeout(Some(Duration::from_millis(10))).unwrap();
        let e = c.read(&mut [0u8; 1]).unwrap_err();
        assert!(super::super::timed_out(&e));
    }

    #[test]
    fn datagrams_reach_the_bound_socket_and_broadcasts_reach_every_one() {
        let net = FakeTransport::new();
        let mut a = net
            .bind(&DatagramOptions::at("0.0.0.0:1024".parse().unwrap()))
            .unwrap();
        a.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let mut b = net.datagram("127.0.0.1", 1024).unwrap();
        b.send(b"ping").unwrap();
        let mut buf = [0u8; 8];
        let (n, from) = a.recv_from(&mut buf).unwrap();
        assert_eq!(
            (&buf[..n], from.port()),
            (&b"ping"[..], b.local_addr().port())
        );
        b.send_to(b"all", "255.255.255.255:1024".parse().unwrap())
            .unwrap();
        assert_eq!(a.recv_from(&mut buf).unwrap().0, 3);
        a.send_to(b"back", from).unwrap();
        b.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        assert_eq!(b.recv_from(&mut buf).unwrap().0, 4);
        assert!(super::super::timed_out(&b.recv_from(&mut buf).unwrap_err()));
    }

    #[test]
    fn a_multicast_datagram_reaches_only_the_members_of_its_group() {
        let net = FakeTransport::new();
        let group: SocketAddr = "239.255.255.250:1900".parse().unwrap();
        let mut member = net.bind(&DatagramOptions::member_of(group)).unwrap();
        let other: SocketAddr = "239.1.2.3:1900".parse().unwrap();
        let mut elsewhere = net
            .bind(&DatagramOptions {
                join: vec![other.ip()],
                ..DatagramOptions::at("127.0.0.2:1900".parse().unwrap())
            })
            .unwrap();
        let mut plain = net
            .bind(&DatagramOptions::at("127.0.0.3:1900".parse().unwrap()))
            .unwrap();
        let mut sender = net.bind(&DatagramOptions::any_v4()).unwrap();
        sender.send_to(b"search", group).unwrap();
        let mut buf = [0u8; 16];
        let (n, from) = member.try_recv_from(&mut buf).unwrap().unwrap();
        assert_eq!(
            (&buf[..n], from.port()),
            (&b"search"[..], sender.local_addr().port())
        );
        assert!(elsewhere.try_recv_from(&mut buf).unwrap().is_none());
        assert!(plain.try_recv_from(&mut buf).unwrap().is_none());
        // A socket that is closed leaves its groups.
        drop(member);
        let mut again = net
            .bind(&DatagramOptions::at("0.0.0.0:1900".parse().unwrap()))
            .unwrap();
        sender.send_to(b"search", group).unwrap();
        assert!(again.try_recv_from(&mut buf).unwrap().is_none());
    }

    #[test]
    fn a_socket_receives_without_waiting_and_a_connection_knows_its_peer() {
        let net = FakeTransport::new();
        let mut a = net
            .bind(&DatagramOptions {
                recv_buffer: Some(1 << 20),
                ..DatagramOptions::at("0.0.0.0:2024".parse().unwrap())
            })
            .unwrap();
        assert_eq!(a.recv_buffer(), Some(1 << 20));
        let mut buf = [0u8; 8];
        assert!(a.try_recv_from(&mut buf).unwrap().is_none());
        let mut b = net.datagram("127.0.0.1", 2024).unwrap();
        b.send(b"hi").unwrap();
        assert_eq!(a.try_recv_from(&mut buf).unwrap().map(|r| r.0), Some(2));
        net.serve("10.0.0.2", 80, |c| {
            assert!(c.peer_addr().is_some());
        });
        let c = net
            .connect("10.0.0.2", 80, false, Duration::from_secs(1))
            .unwrap();
        assert_eq!(c.peer_addr(), Some("10.0.0.2:80".parse().unwrap()));
        let n = super::super::Ipv4Net {
            addr: Ipv4Addr::new(192, 168, 10, 9),
            netmask: Ipv4Addr::new(255, 255, 255, 0),
        };
        assert_eq!(n.broadcast(), Ipv4Addr::new(192, 168, 10, 255));
    }
}
