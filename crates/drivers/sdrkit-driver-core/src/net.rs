//! The network parts that the network drivers share: an [`Abort`] that cuts a connect or a blocked read short, a connect
//! through the host's [`Transport`] that an abort can stop, and the receive buffer of a stream.

use sdrkit_core::net::{Conn, NetError, Transport};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// Cuts blocking network work short from another thread. Connections registered with it are shut down when it fires, and a
/// connect in progress stops waiting. Firing is permanent: make a new one for the next attempt.
#[derive(Default)]
pub struct Abort {
    fired: AtomicBool,
    conns: Mutex<Vec<Box<dyn Conn>>>,
}

impl std::fmt::Debug for Abort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Abort")
            .field("fired", &self.fired())
            .finish()
    }
}

impl Abort {
    /// A new handle, not fired.
    pub fn new() -> Arc<Abort> {
        Arc::new(Abort::default())
    }

    /// Shuts every registered connection down, which wakes a blocked read, and makes later connects fail at once.
    pub fn fire(&self) {
        self.fired.store(true, Ordering::Release);
        for c in self.lock().drain(..) {
            c.shutdown();
        }
    }

    /// True if [`Abort::fire`] was called.
    pub fn fired(&self) -> bool {
        self.fired.load(Ordering::Acquire)
    }

    /// Registers a connection to shut down on [`Abort::fire`]. If the abort fired already, the connection is shut down at once.
    pub fn register(&self, conn: &dyn Conn) {
        let Ok(c) = conn.try_clone() else { return };
        let mut v = self.lock();
        if self.fired() {
            c.shutdown();
        } else {
            v.push(c);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Box<dyn Conn>>> {
        self.conns
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Connects to `host:port` through `transport` within `timeout`, over TLS when `tls` is set. The connect runs on a helper
/// thread, and the caller looks at `abort` every 20 ms. The connection is registered with `abort`.
///
/// # Errors
///
/// [`NetError::Aborted`] if the abort fired, else what the transport returns.
pub fn connect(
    transport: &Arc<dyn Transport>,
    host: &str,
    port: u16,
    tls: bool,
    timeout: Duration,
    abort: &Abort,
) -> Result<Box<dyn Conn>, NetError> {
    if abort.fired() {
        return Err(NetError::Aborted);
    }
    let (tx, rx) = mpsc::channel();
    let (t, h) = (transport.clone(), host.to_owned());
    std::thread::Builder::new()
        .name("sdrkit-connect".into())
        .spawn(move || {
            let _ = tx.send(t.connect(&h, port, tls, timeout));
        })
        .map_err(NetError::from)?;
    let deadline = Instant::now() + timeout + Duration::from_millis(500);
    loop {
        if abort.fired() {
            return Err(NetError::Aborted);
        }
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(Ok(c)) => {
                abort.register(c.as_ref());
                return Ok(c);
            }
            Ok(Err(e)) => return Err(e),
            Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
            Err(_) => return Err(NetError::TimedOut),
        }
    }
}

/// Asks for the first receive buffer size of `sizes` (largest first) that the system takes.
pub fn recv_buffer(conn: &mut dyn Conn, sizes: &[usize]) {
    for &size in sizes {
        if conn.set_recv_buffer(size).is_ok() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::net::FakeTransport;
    use std::io::Read;

    #[test]
    fn a_refused_connect_says_so() {
        let t: Arc<dyn Transport> = Arc::new(FakeTransport::new());
        let r = connect(
            &t,
            "127.0.0.1",
            4000,
            false,
            Duration::from_secs(2),
            &Abort::default(),
        );
        assert_eq!(r.err(), Some(NetError::Refused));
        let r = connect(
            &t,
            "127.0.0.1",
            4000,
            true,
            Duration::from_secs(2),
            &Abort::default(),
        );
        assert_eq!(r.err(), Some(NetError::TlsUnavailable));
    }

    #[test]
    fn an_abort_wakes_a_blocked_read() {
        let net = FakeTransport::new();
        net.serve("127.0.0.1", 4001, |c| {
            std::thread::sleep(Duration::from_secs(5));
            drop(c);
        });
        let t: Arc<dyn Transport> = Arc::new(net);
        let abort = Abort::new();
        let mut s = connect(&t, "127.0.0.1", 4001, false, Duration::from_secs(2), &abort).unwrap();
        let a = abort.clone();
        let th = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            a.fire();
        });
        let mut buf = [0u8; 4];
        let started = Instant::now();
        assert_eq!(s.read(&mut buf).unwrap_or(0), 0);
        assert!(started.elapsed() < Duration::from_secs(2));
        th.join().unwrap();
        let r = connect(&t, "127.0.0.1", 4001, false, Duration::from_secs(1), &abort);
        assert_eq!(r.err(), Some(NetError::Aborted));
    }
}
