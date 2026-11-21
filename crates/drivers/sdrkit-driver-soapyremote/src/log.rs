//! The log connection: a second connection on which the server forwards its log lines. A helper thread reads them and passes
//! them on at a limited rate. It counts the stream event characters and does not log them.
//!
//! Standard: the SoapyRemote protocol, RPC version 0x0400 (`rpc::VERSION`).

use crate::rpc::{self, call, Packer, Unpacker};
use crate::stream::StreamStats;
use crate::Timing;
use sdrkit_core::net::{Conn, Transport};
use sdrkit_driver_core::net::{self, Abort};
use sdrkit_driver_core::runner::Notices;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Lines passed on per second.
pub const LINES_PER_SECOND: u32 = 20;
/// The summary of dropped lines comes at most this often.
pub const SUMMARY_EVERY: Duration = Duration::from_secs(10);

/// SoapySDR log levels.
pub mod level {
    /// Warning (and every level below it is worse).
    pub const WARNING: i8 = 4;
    /// Stream event characters.
    pub const SSI: i8 = 9;
}

/// At most [`LINES_PER_SECOND`] lines per second. The rest are counted, and one line per [`SUMMARY_EVERY`] returns the count.
#[derive(Clone, Debug, Default)]
pub struct LogLimiter {
    sec_start: Option<Instant>,
    in_sec: u32,
    dropped: u64,
    since: Option<Instant>,
}

impl LogLimiter {
    /// Whether a line arriving at `now` is passed on.
    pub fn admit(&mut self, now: Instant) -> bool {
        match self.sec_start {
            Some(s) if now.duration_since(s) < Duration::from_secs(1) => {}
            _ => (self.sec_start, self.in_sec) = (Some(now), 0),
        }
        if self.in_sec < LINES_PER_SECOND {
            self.in_sec += 1;
            true
        } else {
            if self.dropped == 0 {
                self.since.get_or_insert(now);
            }
            self.dropped += 1;
            false
        }
    }
    /// The summary line, if lines were dropped and the line is due at `now` (or `force` is set, at the end).
    pub fn summary(&mut self, now: Instant, force: bool) -> Option<String> {
        if self.dropped == 0 {
            return None;
        }
        let since = *self.since.get_or_insert(now);
        if !force && now.duration_since(since) < SUMMARY_EVERY {
            return None;
        }
        let n = std::mem::take(&mut self.dropped);
        self.since = Some(now);
        Some(format!("{n} more server log lines"))
    }
}

/// A running log connection.
pub(crate) struct LogConn {
    stop: Arc<AtomicBool>,
    abort: Arc<Abort>,
    sock: Slot,
    thread: Option<JoinHandle<()>>,
}

/// The helper's socket, and whether forwarding was started on it.
type Slot = Arc<Mutex<Option<(Box<dyn Conn>, bool)>>>;

/// What a log connection needs.
pub(crate) struct LogSetup {
    /// The network.
    pub transport: Arc<dyn Transport>,
    /// Server host.
    pub host: String,
    /// Server port.
    pub port: u16,
    /// `host:port` for messages.
    pub label: String,
    /// Timings.
    pub timing: Timing,
    /// Where lines go.
    pub notices: Notices,
    /// Where `O` and `S` are counted.
    pub stats: Arc<StreamStats>,
}

impl LogConn {
    /// Starts the log connection on a helper thread. If it fails, one log line says so. It never fails the open.
    pub fn start(s: LogSetup) -> LogConn {
        let stop = Arc::new(AtomicBool::new(false));
        let sock = Arc::new(Mutex::new(None));
        let abort = Abort::new();
        let (st, so, ab) = (stop.clone(), sock.clone(), abort.clone());
        let thread = std::thread::Builder::new()
            .name("sdrkit-soapy-log".into())
            .spawn(move || run(s, st, so, ab))
            .ok();
        LogConn {
            stop,
            abort,
            sock,
            thread,
        }
    }

    /// Stops forwarding (call 22), hangs up (call 3), closes, and joins the helper.
    pub fn close(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some((mut s, started)) = crate::lock(&self.sock).take() {
            if started {
                let _ = s.write_all(&rpc::packet(
                    &Packer::call(call::STOP_LOG).buf,
                    rpc::VERSION,
                ));
                let _ = s.write_all(&rpc::packet(&Packer::call(call::HANGUP).buf, rpc::VERSION));
            }
            s.shutdown();
        }
        self.abort.fire();
        if let Some(t) = self.thread.take() {
            // The helper ends when its socket is shut. A pending connect ends within its own timeout slices.
            let end = Instant::now() + Duration::from_secs(2);
            while !t.is_finished() && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(2));
                if let Some((s, _)) = crate::lock(&self.sock).take() {
                    s.shutdown();
                }
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }
}

impl Drop for LogConn {
    fn drop(&mut self) {
        self.close();
    }
}

fn run(s: LogSetup, stop: Arc<AtomicBool>, slot: Slot, abort: Arc<Abort>) {
    let fail = |why: &str| {
        s.notices.log(format!(
            "SoapyRemote {}: the server's log is not forwarded ({why})",
            s.label
        ));
    };
    let timeout = s.timing.connect_timeout(&s.host);
    let Ok(mut sock) = net::connect(&s.transport, &s.host, s.port, false, timeout, &abort) else {
        return fail("cannot connect");
    };
    {
        let mut g = crate::lock(&slot);
        if stop.load(Ordering::Acquire) {
            sock.shutdown();
            return;
        }
        match sock.try_clone() {
            Ok(c) => *g = Some((c, false)),
            Err(_) => return,
        }
    }
    if sock
        .write_all(&rpc::packet(
            &Packer::call(call::START_LOG).buf,
            rpc::VERSION,
        ))
        .is_err()
    {
        return fail("the connection closed");
    }
    match rpc::read_packet(sock.as_mut(), s.timing.log_start) {
        Ok(p) if Unpacker::new(&p.payload, p.version).void().is_ok() => {}
        Ok(_) => return fail("unexpected answer"),
        Err(rpc::RpcError::Timeout(_)) => {
            sock.shutdown();
            return fail(&format!(
                "no answer within {:.1} s",
                s.timing.log_start.as_secs_f64()
            ));
        }
        Err(_) => return fail("the connection closed"),
    }
    match crate::lock(&slot).as_mut() {
        Some(e) => e.1 = true,
        None => return,
    }
    let mut lim = LogLimiter::default();
    while let Ok(p) = rpc::read_packet(sock.as_mut(), Duration::from_secs(24 * 3600)) {
        let now = Instant::now();
        let mut u = Unpacker::new(&p.payload, p.version);
        let (Ok(lvl), Ok(msg)) = (u.char(), u.string()) else {
            continue;
        };
        if lvl == level::SSI {
            for c in msg.chars() {
                match c {
                    'O' => s.stats.ssi_overflows.fetch_add(1, Ordering::Relaxed),
                    'S' => s.stats.ssi_sequence.fetch_add(1, Ordering::Relaxed),
                    _ => 0,
                };
            }
            continue;
        }
        if lim.admit(now) {
            let line = format!("server: {}", msg.trim_end());
            if lvl <= level::WARNING {
                s.notices.warn(line);
            } else {
                s.notices.log(line);
            }
        } else {
            s.stats.log_dropped.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(sum) = lim.summary(now, false) {
            s.notices.log(sum);
        }
    }
    if let Some(sum) = lim.summary(Instant::now(), true) {
        s.notices.log(sum);
    }
}
