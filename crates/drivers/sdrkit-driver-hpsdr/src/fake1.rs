//! A fake protocol 1 radio on a fake network. It answers the way a Hermes-Lite 2 or the Red Pitaya program answers: the
//! layout follows register 0x00 four packets late.

use core::net::SocketAddr;
use sdrkit_core::net::{Datagram, DatagramOptions, FakeTransport, Transport};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How the fake paces its EP6 packets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pace {
    /// One packet every millisecond.
    Millisecond,
    /// At the rate that register 0x00 selects (rate / samples per packet).
    Rate,
}

/// One packet the fake received, in order.
#[derive(Clone, Debug)]
pub struct Got {
    pub from: SocketAddr,
    pub bytes: Vec<u8>,
}

/// The state and behaviour of the fake radio.
pub struct State {
    pub board: u8,
    pub gateware: u8,
    pub byte13: u8,
    pub busy: bool,
    /// A reply to send instead of the built one.
    pub reply: Option<Vec<u8>>,
    pub answer_discovery: bool,
    pub client: Option<SocketAddr>,
    pub streaming: bool,
    pub starts: u32,
    pub start_times: Vec<Instant>,
    pub stops: u32,
    pub discoveries: u32,
    pub regs: BTreeMap<u8, u32>,
    pub regs_seen: BTreeSet<u8>,
    /// (sequence, register of frame 0, register of frame 1) of each EP2 packet.
    pub ep2_log: Vec<(u32, u8, u8)>,
    pub ep2: u64,
    pub ep2_times: Vec<Instant>,
    pub mox_seen: bool,
    pub mox_last: bool,
    pub first_slot: Option<[u8; 8]>,
    pub ep6: u64,
    pub seq: u32,
    pub layout: usize,
    pub target: usize,
    pub countdown: u32,
    pub pace: Pace,
    /// Sequence numbers not sent (they still count).
    pub skip: Vec<std::ops::RangeInclusive<u32>>,
    /// When the sequence reaches the first, it jumps to the second.
    pub jump: Option<(u32, u32)>,
    pub ignore_ep2: bool,
    pub never_stream: bool,
    /// Streaming paused without a stop command: the starts it needs to resume (u32::MAX: never).
    pub paused: Option<u32>,
    /// C0 to C4 of every EP6 frame.
    pub status: [u8; 5],
    /// Every packet received (capped).
    pub got: Vec<Got>,
}

/// The fake radio.
pub struct Fake1 {
    pub ip: String,
    pub port: u16,
    pub net: FakeTransport,
    pub st: Arc<Mutex<State>>,
    quit: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Fake1 {
    /// A fake Hermes-Lite 2 (board 6, gateware 73, 4 receivers).
    pub fn hl2() -> Fake1 {
        Fake1::new(6, 73, 4)
    }

    /// A fake Red Pitaya (board 1, gateware 32, receivers not said).
    pub fn red_pitaya() -> Fake1 {
        Fake1::new(1, 32, 0)
    }

    /// A fake radio at 127.0.0.1:1024 on a network of its own.
    pub fn new(board: u8, gateware: u8, byte13: u8) -> Fake1 {
        Fake1::on(
            &FakeTransport::new(),
            "127.0.0.1",
            1024,
            board,
            gateware,
            byte13,
        )
    }

    /// A fake radio at `ip:port` on `net`.
    pub fn on(
        net: &FakeTransport,
        ip: &str,
        port: u16,
        board: u8,
        gateware: u8,
        byte13: u8,
    ) -> Fake1 {
        let local = SocketAddr::new(ip.parse().unwrap(), port);
        let mut sock = net.bind(&DatagramOptions::at(local)).unwrap();
        sock.set_read_timeout(Some(Duration::from_micros(300)))
            .unwrap();
        let st = Arc::new(Mutex::new(State {
            board,
            gateware,
            byte13,
            busy: false,
            reply: None,
            answer_discovery: true,
            client: None,
            streaming: false,
            starts: 0,
            start_times: Vec::new(),
            stops: 0,
            discoveries: 0,
            regs: BTreeMap::new(),
            regs_seen: BTreeSet::new(),
            ep2_log: Vec::new(),
            ep2: 0,
            ep2_times: Vec::new(),
            mox_seen: false,
            mox_last: false,
            first_slot: None,
            ep6: 0,
            seq: 0,
            layout: 1,
            target: 1,
            countdown: 0,
            pace: Pace::Millisecond,
            skip: Vec::new(),
            jump: None,
            ignore_ep2: false,
            never_stream: false,
            paused: None,
            status: [0; 5],
            got: Vec::new(),
        }));
        let quit = Arc::new(AtomicBool::new(false));
        let (s2, q2) = (st.clone(), quit.clone());
        let thread = std::thread::spawn(move || run(sock, s2, q2));
        Fake1 {
            ip: ip.to_string(),
            port,
            net: net.clone(),
            st,
            quit,
            thread: Some(thread),
        }
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }

    /// The network of the fake, for a driver.
    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.net.clone())
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut self.st.lock().unwrap())
    }

    pub fn reg(&self, r: u8) -> Option<u32> {
        self.with(|s| s.regs.get(&r).copied())
    }

    /// Waits until `f` holds, up to `t`.
    pub fn wait(&self, t: Duration, f: impl Fn(&State) -> bool) -> bool {
        let end = Instant::now() + t;
        while Instant::now() < end {
            if f(&self.st.lock().unwrap()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        f(&self.st.lock().unwrap())
    }

    /// Waits until register `r` reads `v`.
    pub fn wait_reg(&self, r: u8, v: u32) -> bool {
        self.wait(Duration::from_secs(3), |s| s.regs.get(&r) == Some(&v))
    }
}

impl Drop for Fake1 {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn reply(s: &State) -> Vec<u8> {
    if let Some(r) = &s.reply {
        return r.clone();
    }
    let mut b = vec![0u8; 60];
    b[0] = 0xEF;
    b[1] = 0xFE;
    b[2] = if s.busy { 3 } else { 2 };
    b[3..9].copy_from_slice(&[0x00, 0x1C, 0xC0, 0xA2, 0x12, 0x34]);
    b[9] = s.gateware;
    b[10] = s.board;
    b[0x13] = s.byte13;
    b[0x15] = if s.board == 6 { 2 } else { 0 };
    b
}

fn ep6(s: &State, out: &mut [u8; 1032]) {
    out.fill(0);
    out[..4].copy_from_slice(&[0xEF, 0xFE, 0x01, 0x06]);
    out[4..8].copy_from_slice(&s.seq.to_be_bytes());
    let n = s.layout;
    let size = 6 * n + 2;
    let slots = 504 / size;
    for f in 0..2 {
        let o = 8 + f * 512;
        out[o..o + 3].copy_from_slice(&[0x7F, 0x7F, 0x7F]);
        out[o + 3..o + 8].copy_from_slice(&s.status);
        for sl in 0..slots {
            let so = o + 8 + sl * size;
            for k in 0..n {
                let ko = so + 6 * k;
                out[ko..ko + 3].copy_from_slice(&[0xE0, 0x00, 0x00]);
                let b = 0x40u8.saturating_sub(0x10 * k as u8);
                out[ko + 3..ko + 6].copy_from_slice(&[b, 0x00, 0x00]);
            }
        }
    }
}

fn rate_of(regs: &BTreeMap<u8, u32>) -> u32 {
    match regs.get(&0).map(|v| (v >> 24) & 3) {
        Some(1) => 96_000,
        Some(2) => 192_000,
        Some(3) => 384_000,
        _ => 48_000,
    }
}

fn run(mut sock: Box<dyn Datagram>, st: Arc<Mutex<State>>, quit: Arc<AtomicBool>) {
    let mut buf = [0u8; 2048];
    let mut out = [0u8; 1032];
    let mut since = Instant::now();
    let mut sent_since: u64 = 0;
    let mut paced_at = (0u32, 0usize);
    while !quit.load(Ordering::Acquire) {
        if let Ok((len, from)) = sock.recv_from(&mut buf) {
            let b = &buf[..len];
            let mut s = st.lock().unwrap();
            if s.got.len() < 20_000 {
                s.got.push(Got {
                    from,
                    bytes: b.to_vec(),
                });
            }
            if len == 63 && b[..3] == [0xEF, 0xFE, 0x02] {
                s.discoveries += 1;
                if s.answer_discovery {
                    let r = reply(&s);
                    let _ = sock.send_to(&r, from);
                }
            } else if len == 64 && b[..3] == [0xEF, 0xFE, 0x04] {
                if b[3] & 1 != 0 {
                    s.starts += 1;
                    s.start_times.push(Instant::now());
                    s.client = Some(from);
                    match s.paused {
                        Some(k) if k != u32::MAX => {
                            if k <= 1 {
                                s.paused = None;
                            } else {
                                s.paused = Some(k - 1);
                            }
                        }
                        _ => {}
                    }
                    if !s.streaming {
                        s.streaming = true;
                        since = Instant::now();
                        sent_since = 0;
                    }
                } else {
                    s.stops += 1;
                    s.streaming = false;
                }
            } else if len == 1032 && b[..4] == [0xEF, 0xFE, 0x01, 0x02] {
                s.ep2 += 1;
                if s.ep2_times.len() < 100_000 {
                    s.ep2_times.push(Instant::now());
                }
                let seq = u32::from_be_bytes([b[4], b[5], b[6], b[7]]);
                let r0 = b[8 + 3] >> 1;
                let r1 = b[520 + 3] >> 1;
                if s.ep2_log.len() < 100_000 {
                    s.ep2_log.push((seq, r0, r1));
                }
                let mox = b[8 + 3] & 1 != 0;
                s.mox_last = mox;
                if mox {
                    s.mox_seen = true;
                    let mut fs = [0u8; 8];
                    fs.copy_from_slice(&b[16..24]);
                    s.first_slot = Some(fs);
                }
                if !s.ignore_ep2 {
                    for f in 0..2 {
                        let o = 8 + f * 512;
                        if b[o..o + 3] == [0x7F, 0x7F, 0x7F] {
                            let reg = b[o + 3] >> 1;
                            let v = u32::from_be_bytes([b[o + 4], b[o + 5], b[o + 6], b[o + 7]]);
                            s.regs.insert(reg, v);
                            s.regs_seen.insert(reg);
                            if reg == 0 {
                                let n = (((v >> 3) & 0xF) + 1) as usize;
                                if n != s.target {
                                    s.target = n;
                                    s.countdown = 4;
                                }
                            }
                        }
                    }
                }
            }
        }
        // Stream.
        let mut s = st.lock().unwrap();
        let Some(client) = s.client else { continue };
        if !s.streaming || s.never_stream {
            continue;
        }
        if s.pace == Pace::Rate && paced_at != (rate_of(&s.regs), s.layout) {
            paced_at = (rate_of(&s.regs), s.layout);
            since = Instant::now();
            sent_since = 0;
        }
        let elapsed = since.elapsed();
        let due = match s.pace {
            Pace::Millisecond => elapsed.as_millis() as u64,
            Pace::Rate => {
                let per = 2 * (504 / (6 * s.layout + 2)) as u64;
                (elapsed.as_secs_f64() * rate_of(&s.regs) as f64 / per as f64) as u64
            }
        };
        let mut k = 0;
        while sent_since < due && k < 64 {
            sent_since += 1;
            k += 1;
            if let Some((from, to)) = s.jump {
                if s.seq == from {
                    s.seq = to;
                    s.jump = None;
                }
            }
            let skipped = s.skip.iter().any(|r| r.contains(&s.seq));
            if s.paused.is_none() && !skipped {
                ep6(&s, &mut out);
                let _ = sock.send_to(&out, client);
                s.ep6 += 1;
                if s.countdown > 0 {
                    s.countdown -= 1;
                    if s.countdown == 0 {
                        s.layout = s.target;
                    }
                }
            }
            s.seq = s.seq.wrapping_add(1);
        }
    }
}
