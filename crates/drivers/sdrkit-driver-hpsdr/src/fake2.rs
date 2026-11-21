//! A fake protocol 2 radio on a fake network: the ports 1024 to 1039, an Orion MkII discovery reply, DDC packets from port
//! 1024 + 11 + n while running.

use core::net::SocketAddr;
use sdrkit_core::net::{Datagram, DatagramOptions, FakeTransport, Transport};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const PORTS: u16 = 16;

pub struct State {
    pub reply: Vec<u8>,
    pub general: Vec<Vec<u8>>,
    pub rx_specific: Vec<Vec<u8>>,
    pub tx_specific: Vec<Vec<u8>>,
    /// (time, bytes) of each high-priority packet.
    pub hp: Vec<(Instant, Vec<u8>)>,
    pub duc: u64,
    pub duc_times: Vec<Instant>,
    pub duc_last: Option<Vec<u8>>,
    pub client: Option<SocketAddr>,
    pub running: bool,
    pub enabled: u128,
    pub bits: u16,
    /// Also send DDC1 packets when DDC1 is not enabled.
    pub stray_ddc1: bool,
    pub overload: bool,
    pub seq: [u32; 4],
    pub silent: bool,
    /// Sequence numbers DDC0 skips once, before its next packet.
    pub skip_ddc0: u32,
}

pub struct Fake2 {
    pub port: u16,
    pub net: FakeTransport,
    pub st: Arc<Mutex<State>>,
    quit: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn port_run(net: &FakeTransport) -> (u16, Vec<Box<dyn Datagram>>) {
    let base = 1024;
    let socks = (0..PORTS)
        .map(|i| {
            let local = SocketAddr::new([127, 0, 0, 1].into(), base + i);
            let mut s = net.bind(&DatagramOptions::at(local)).unwrap();
            s.set_read_timeout(Some(Duration::from_nanos(1))).unwrap();
            s
        })
        .collect();
    (base, socks)
}

impl Fake2 {
    pub fn orion_mk2() -> Fake2 {
        let net = FakeTransport::new();
        let (port, socks) = port_run(&net);
        let mut reply = vec![
            0, 0, 0, 0, 0x02, 0x00, 0x1c, 0xc0, 0xa2, 0x56, 0x78, 0x05, 0x26, 0x15, 0, 0, 0, 0, 0,
            0, 0x04, 0x01, 0x00,
        ];
        reply.resize(60, 0);
        let st = Arc::new(Mutex::new(State {
            reply,
            general: Vec::new(),
            rx_specific: Vec::new(),
            tx_specific: Vec::new(),
            hp: Vec::new(),
            duc: 0,
            duc_times: Vec::new(),
            duc_last: None,
            client: None,
            running: false,
            enabled: 0,
            bits: 24,
            stray_ddc1: false,
            overload: false,
            seq: [0; 4],
            silent: false,
            skip_ddc0: 0,
        }));
        let quit = Arc::new(AtomicBool::new(false));
        let (s2, q2) = (st.clone(), quit.clone());
        let thread = std::thread::spawn(move || run(socks, s2, q2));
        Fake2 {
            port,
            net,
            st,
            quit,
            thread: Some(thread),
        }
    }

    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// The network of the fake, for a driver.
    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.net.clone())
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut self.st.lock().unwrap())
    }

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
}

impl Drop for Fake2 {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn ddc_packet(seq: u32, bits: u16, n: usize) -> Vec<u8> {
    let mut b = vec![0u8; 1444];
    b[..4].copy_from_slice(&seq.to_be_bytes());
    b[12..14].copy_from_slice(&bits.to_be_bytes());
    b[14..16].copy_from_slice(&238u16.to_be_bytes());
    let i = 0x40u8.saturating_sub(0x10 * n as u8);
    for k in 0..238 {
        let o = 16 + 6 * k;
        b[o..o + 6].copy_from_slice(&[i, 0, 0, 0xe0, 0, 0]);
    }
    b
}

fn run(mut socks: Vec<Box<dyn Datagram>>, st: Arc<Mutex<State>>, quit: Arc<AtomicBool>) {
    let mut buf = [0u8; 2048];
    let mut tick = Instant::now();
    let mut status_tick = Instant::now();
    while !quit.load(Ordering::Acquire) {
        let mut idle = true;
        for (i, s) in socks.iter_mut().enumerate().take(6) {
            while let Ok((len, from)) = s.recv_from(&mut buf) {
                idle = false;
                let b = &buf[..len];
                let mut g = st.lock().unwrap();
                match i {
                    0 if len == 60 && b[..4] == [0, 0, 0, 0] && b[4] == 2 => {
                        let r = g.reply.clone();
                        let _ = s.send_to(&r, from);
                    }
                    0 => g.general.push(b.to_vec()),
                    1 => {
                        let mut e = 0u128;
                        for byte in 0..10 {
                            e |= (b[7 + byte] as u128) << (8 * byte);
                        }
                        g.enabled = e;
                        g.rx_specific.push(b.to_vec());
                    }
                    2 => g.tx_specific.push(b.to_vec()),
                    3 => {
                        g.hp.push((Instant::now(), b.to_vec()));
                        g.running = b[4] & 1 != 0;
                        g.client = Some(from);
                    }
                    5 => {
                        g.duc += 1;
                        g.duc_times.push(Instant::now());
                        g.duc_last = Some(b.to_vec());
                    }
                    _ => {}
                }
            }
        }
        let mut g = st.lock().unwrap();
        if let (true, false, Some(client)) = (g.running, g.silent, g.client) {
            while tick.elapsed() >= Duration::from_millis(1) {
                tick += Duration::from_millis(1);
                for n in 0..4 {
                    let on = g.enabled & (1 << n) != 0 || (n == 1 && g.stray_ddc1);
                    if on {
                        if n == 0 && g.skip_ddc0 > 0 {
                            g.seq[0] = g.seq[0].wrapping_add(g.skip_ddc0);
                            g.skip_ddc0 = 0;
                        }
                        let p = ddc_packet(g.seq[n], g.bits, n);
                        g.seq[n] = g.seq[n].wrapping_add(1);
                        let _ = socks[11 + n].send_to(&p, client);
                    }
                }
            }
            if status_tick.elapsed() >= Duration::from_millis(50) {
                status_tick = Instant::now();
                let mut p = vec![0u8; 60];
                p[4] = 0x10;
                p[5] = g.overload as u8;
                let _ = socks[1].send_to(&p, client);
            }
        } else {
            tick = Instant::now();
        }
        drop(g);
        if idle {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
}
