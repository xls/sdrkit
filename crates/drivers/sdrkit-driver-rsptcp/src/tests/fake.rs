//! A fake rsp_tcp server on a fake network. It sends a given greeting, then bytes in pieces or I/Q at a pace, and records every
//! 5-byte command it receives, in order, per connection.

use crate::proto::cmd;
use sdrkit_core::net::{Conn, FakeTransport};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The address the fake serves on.
pub const HOST: &str = "127.0.0.1";
/// The port the fake serves on.
pub const PORT: u16 = 1234;

/// What the server sends after the greeting and the pieces.
#[derive(Clone, Copy, Debug)]
pub enum Flow {
    /// Nothing more (the connection stays open).
    Idle,
    /// I/Q at the rate that command 0x02 sets.
    Obey,
    /// I/Q at this rate whatever rate is requested.
    Own(f64),
}

#[derive(Clone, Debug)]
pub struct Fake {
    pub greeting: Vec<u8>,
    /// Sent after the greeting, one write each, `gap` apart.
    pub pieces: Vec<Vec<u8>>,
    pub gap: Duration,
    pub flow: Flow,
    /// Bytes per complex sample of the flow.
    pub bps: usize,
    /// (after, for): the flow goes silent `after` its start, for `for`.
    pub silence: Option<(Duration, Duration)>,
    /// Close the first connection after this long.
    pub drop_first_after: Option<Duration>,
    /// Connections after the first get no greeting and no samples.
    pub mute_after_first: bool,
}

impl Fake {
    pub fn new(greeting: &[u8]) -> Fake {
        Fake {
            greeting: greeting.to_vec(),
            pieces: Vec::new(),
            gap: Duration::from_millis(30),
            flow: Flow::Idle,
            bps: 2,
            silence: None,
            drop_first_after: None,
            mute_after_first: false,
        }
    }
}

#[derive(Default)]
pub struct Seen {
    pub cmds: Mutex<Vec<[u8; 5]>>,
    /// The client closed its side (seen after every command it sent).
    pub closed: AtomicBool,
}

pub struct FakeServer {
    net: FakeTransport,
    pub conns: Arc<Mutex<Vec<Arc<Seen>>>>,
    stop: Arc<AtomicBool>,
}

impl FakeServer {
    pub fn start(f: Fake) -> FakeServer {
        Self::start_on(FakeTransport::new(), f)
    }

    pub fn start_on(net: FakeTransport, f: Fake) -> FakeServer {
        let conns: Arc<Mutex<Vec<Arc<Seen>>>> = Default::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (c2, s2) = (conns.clone(), stop.clone());
        net.serve(HOST, PORT, move |s| {
            let seen = Arc::new(Seen::default());
            let first = {
                let mut c = c2.lock().unwrap();
                c.push(seen.clone());
                c.len() == 1
            };
            serve(s, seen, &f, first, &s2);
        });
        FakeServer { net, conns, stop }
    }

    pub fn net(&self) -> FakeTransport {
        self.net.clone()
    }

    pub fn addr(&self) -> String {
        format!("{HOST}:{PORT}")
    }

    pub fn n_conns(&self) -> usize {
        self.conns.lock().unwrap().len()
    }

    pub fn cmds(&self, i: usize) -> Vec<[u8; 5]> {
        self.conns
            .lock()
            .unwrap()
            .get(i)
            .map(|c| c.cmds.lock().unwrap().clone())
            .unwrap_or_default()
    }

    pub fn closed(&self, i: usize) -> bool {
        self.conns
            .lock()
            .unwrap()
            .get(i)
            .is_some_and(|c| c.closed.load(Ordering::Acquire))
    }

    pub fn shutdown(&mut self) {
        self.net.stop_serving(HOST, PORT);
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn serve(mut s: Box<dyn Conn>, seen: Arc<Seen>, f: &Fake, first: bool, stop: &AtomicBool) {
    let rate = Arc::new(Mutex::new(match f.flow {
        Flow::Own(r) => r,
        _ => 0.0,
    }));
    // The command reader: every 5 bytes, in order, then the close.
    let mut r = s.try_clone().unwrap();
    let (rate2, seen2, obey) = (rate.clone(), seen.clone(), matches!(f.flow, Flow::Obey));
    let reader = std::thread::spawn(move || {
        let mut c = [0u8; 5];
        while r.read_exact(&mut c).is_ok() {
            seen2.cmds.lock().unwrap().push(c);
            if obey && c[0] == cmd::RATE {
                *rate2.lock().unwrap() = f64::from(u32::from_be_bytes([c[1], c[2], c[3], c[4]]));
            }
        }
        seen2.closed.store(true, Ordering::Release);
    });
    let start = Instant::now();
    let done = |stop: &AtomicBool| {
        stop.load(Ordering::Relaxed)
            || (first && f.drop_first_after.is_some_and(|d| start.elapsed() >= d))
            || seen.closed.load(Ordering::Acquire)
    };
    let quit = |s: &dyn Conn, reader: std::thread::JoinHandle<()>| {
        s.shutdown();
        let _ = reader.join();
    };
    if !first && f.mute_after_first {
        while !done(stop) {
            std::thread::sleep(Duration::from_millis(5));
        }
        quit(s.as_ref(), reader);
        return;
    }
    if !f.greeting.is_empty() && s.write_all(&f.greeting).is_err() {
        return;
    }
    for p in &f.pieces {
        std::thread::sleep(f.gap);
        if s.write_all(p).is_err() {
            return;
        }
    }
    let pattern: Vec<u8> = (0..(1usize << 20) + 256).map(|i| i as u8).collect();
    let flow_start = Instant::now();
    let mut epoch = (Instant::now(), -1.0f64);
    let mut sent: u64 = 0;
    loop {
        if done(stop) {
            quit(s.as_ref(), reader);
            return;
        }
        if matches!(f.flow, Flow::Idle) {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        if let Some((after, len)) = f.silence {
            let t = flow_start.elapsed();
            if t >= after && t < after + len {
                // Start the pace again after the silence.
                epoch.1 = -1.0;
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
        }
        let r = *rate.lock().unwrap();
        if r != epoch.1 {
            epoch = (Instant::now(), r);
            sent = 0;
        }
        let due = (epoch.0.elapsed().as_secs_f64() * r * f.bps as f64) as u64;
        if due > sent {
            let n = (due - sent).min(1 << 20) as usize;
            let off = (sent % 256) as usize;
            if s.write_all(&pattern[off..off + n]).is_err() {
                return;
            }
            sent += n as u64;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
