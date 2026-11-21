//! Stream outputs: a file in the host's storage with size rotation, UDP datagrams, and a TCP server that every connected
//! program reads from.

use std::io::Write;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use sdrkit_core::storage::{Storage, StoreWriter, WriteMode};
use sdrkit_core::{StorageError, StorePath};

use crate::net::host_port;
use crate::runner::{Item, Sink};
use crate::OutputError;
use sdrkit_core::net::{Conn, Datagram, Transport};

/// Appends lines to a file in the host's storage, and rotates it by size: `<path>.1` is the newest old file, `<path>.<keep>`
/// the oldest.
pub struct FileSink {
    storage: Arc<dyn Storage>,
    path: StorePath,
    max_bytes: u64,
    keep: u32,
    file: Option<(Box<dyn StoreWriter>, u64)>,
}

impl FileSink {
    /// A file sink at `path` that rotates before the file passes `max_bytes` (0: never) and keeps `keep` old files.
    pub fn new(storage: Arc<dyn Storage>, path: StorePath, max_bytes: u64, keep: u32) -> Self {
        FileSink {
            storage,
            path,
            max_bytes,
            keep,
            file: None,
        }
    }

    fn err(&self) -> impl Fn(StorageError) -> OutputError + '_ {
        |source| OutputError::Storage {
            path: self.path.clone(),
            source,
        }
    }

    fn rotated(&self, n: u32) -> Result<StorePath, OutputError> {
        let name = format!("{}.{n}", self.path.as_str());
        StorePath::new(&name).map_err(|e| (self.err())(StorageError::InvalidPath(e)))
    }

    fn rotate(&mut self) -> Result<(), OutputError> {
        self.file = None;
        if self.keep == 0 {
            return self.storage.remove(&self.path).map_err(self.err());
        }
        let _ = self.storage.remove(&self.rotated(self.keep)?);
        for n in (1..self.keep).rev() {
            let from = self.rotated(n)?;
            if self.storage.stat(&from).is_ok() {
                self.storage
                    .rename(&from, &self.rotated(n + 1)?)
                    .map_err(self.err())?;
            }
        }
        self.storage
            .rename(&self.path, &self.rotated(1)?)
            .map_err(self.err())
    }

    /// Writes bytes, and rotates first when they would pass the size limit.
    ///
    /// # Errors
    ///
    /// [`OutputError::Storage`] if the storage refuses.
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), OutputError> {
        if self.file.is_none() {
            let w = self
                .storage
                .open_writer(&self.path, WriteMode::Append)
                .map_err(self.err())?;
            let len = self.storage.stat(&self.path).map_or(0, |e| e.size);
            self.file = Some((w, len));
        }
        let len = self.file.as_ref().map_or(0, |f| f.1);
        if self.max_bytes > 0 && len > 0 && len + bytes.len() as u64 > self.max_bytes {
            self.rotate()?;
            return self.write(bytes);
        }
        let path = self.path.clone();
        let io = |e: std::io::Error| OutputError::Storage {
            path: path.clone(),
            source: e.into(),
        };
        if let Some((f, len)) = self.file.as_mut() {
            f.write_all(bytes).map_err(io)?;
            f.flush().map_err(io)?;
            *len += bytes.len() as u64;
        }
        Ok(())
    }
}

impl Sink<Item> for FileSink {
    fn deliver(&mut self, item: &Item) -> Result<(), OutputError> {
        self.write(item.body())
    }
    fn reset(&mut self) {
        self.file = None;
    }
}

/// The largest UDP payload over IPv4.
const UDP_MAX: usize = 65_507;

/// Sends each item as UDP datagrams.
pub struct UdpSink {
    host: String,
    port: u16,
    per_line: bool,
    transport: Arc<dyn Transport>,
    socket: Option<Box<dyn Datagram>>,
}

impl UdpSink {
    /// A UDP sink to `target` (`host:port`). If `per_line` is true, each line goes in its own datagram, as receivers of NMEA
    /// and SBS feeds expect.
    ///
    /// # Errors
    ///
    /// [`crate::BadSetting::Address`] if `target` is not `host:port`.
    pub fn new(
        target: &str,
        per_line: bool,
        transport: Arc<dyn Transport>,
    ) -> Result<Self, OutputError> {
        let (host, port) = host_port(target)?;
        Ok(UdpSink {
            host,
            port,
            per_line,
            transport,
            socket: None,
        })
    }
}

impl Sink<Item> for UdpSink {
    fn deliver(&mut self, item: &Item) -> Result<(), OutputError> {
        let bytes = item.body();
        if bytes.is_empty() {
            return Ok(());
        }
        if self.socket.is_none() {
            self.socket = Some(self.transport.datagram(&self.host, self.port)?);
        }
        let Some(sock) = self.socket.as_mut() else {
            return Ok(());
        };
        if self.per_line {
            for l in bytes.split_inclusive(|b| *b == b'\n') {
                if l.len() <= UDP_MAX {
                    sock.send(l)?;
                }
            }
        } else if bytes.len() <= UDP_MAX {
            sock.send(bytes)?;
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.socket = None;
    }
}

/// A TCP server that writes every item to every connected client. A client that cannot take data within a short time is
/// dropped, so a slow client never holds the others up.
pub struct TcpServerSink {
    clients: Arc<Mutex<Vec<Box<dyn Conn>>>>,
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl TcpServerSink {
    /// Starts to listen on `listen` (`host:port`) for at most `max_clients` clients. A host other than the loopback needs
    /// `allow_remote`.
    ///
    /// # Errors
    ///
    /// [`crate::BadSetting::Remote`] for a host other than the loopback without `allow_remote`, or what the transport returns.
    pub fn bind(
        listen: &str,
        allow_remote: bool,
        max_clients: usize,
        transport: &dyn Transport,
    ) -> Result<Self, OutputError> {
        let (host, port) = host_port(listen)?;
        let loopback = host == "localhost"
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
        if !loopback && !allow_remote {
            return Err(OutputError::Setting(crate::BadSetting::Remote));
        }
        let mut listener = transport.listen(&host, port)?;
        let addr = listener.local_addr();
        let clients: Arc<Mutex<Vec<Box<dyn Conn>>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (c2, s2, max) = (clients.clone(), stop.clone(), max_clients.max(1));
        let thread = std::thread::Builder::new()
            .name("sdrkit-out-listen".into())
            .spawn(move || {
                while !s2.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok(Some(sock)) => {
                            let mut list = c2.lock().unwrap_or_else(|p| p.into_inner());
                            if list.len() < max {
                                list.push(sock);
                            }
                        }
                        _ => std::thread::sleep(Duration::from_millis(20)),
                    }
                }
            })?;
        Ok(TcpServerSink {
            clients,
            addr,
            stop,
            thread: Some(thread),
        })
    }

    /// The address it listens on, with the port the system chose for port 0.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Sends bytes to every client, and drops those that fail.
    pub fn broadcast(&self, bytes: &[u8]) {
        let mut list = self.clients.lock().unwrap_or_else(|p| p.into_inner());
        list.retain_mut(|c| c.write_all(bytes).is_ok());
    }
}

impl Sink<Item> for TcpServerSink {
    fn deliver(&mut self, item: &Item) -> Result<(), OutputError> {
        if !item.body().is_empty() {
            self.broadcast(item.body());
        }
        Ok(())
    }
}

impl Drop for TcpServerSink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
