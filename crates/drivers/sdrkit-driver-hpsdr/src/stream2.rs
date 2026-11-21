//! The protocol 2 stream of one radio: one UDP socket, the streams of the radio sorted by their source port, one DDC per
//! receiver, and the DUC packets of the transmitter.
//! Standard: openHPSDR Ethernet protocol (protocol 2).

use crate::discover;
use crate::hub::{
    self, lock, Board, Facts, JoinError, Meter, Pool, RxParams, RxSlot, Stream, Thru, TxQueue,
};
use crate::p2::{self, port, Ddc, HighPriority};
use crate::plan::{self, Gain};
use crate::profile::Profile;
use crate::HpsdrError;
use core::net::SocketAddr;
use sdrkit_core::net::{Datagram, NetError, Transport};
use sdrkit_core::{Complex32, Results};
use sdrkit_driver_core::net::Abort;
use sdrkit_driver_core::runner::Notices;
use sdrkit_driver_core::{Doing, DriverError, TxState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub use crate::stream1::{
    FIRST_SAMPLES, GAP_RESTART, LOST_AFTER, OPEN_DISCOVERY, START_RESEND_OPEN, TX_STARVED,
};

/// The high-priority keep-alive period.
pub const KEEPALIVE: Duration = Duration::from_millis(200);
/// How often the General packet is sent again while the stream runs. It carries the amplifier bit: one lost packet
/// must not leave the amplifier enabled after the transmitter is off.
pub const GENERAL_EVERY: Duration = Duration::from_secs(1);
/// One DUC packet's worth of samples at 192 ksps.
pub const DUC_PERIOD: Duration = Duration::from_micros(1250);

struct Rx2 {
    slot: Arc<RxSlot>,
    freq: u32,
    rate: u32,
}

#[derive(Default)]
struct Tx2 {
    owner: Option<Arc<TxQueue>>,
    ptt: bool,
    freq: u32,
    drive: u8,
    since: Option<Instant>,
    sent: u64,
    starved: Duration,
}

struct Ctl2 {
    rx: Vec<Option<Rx2>>,
    last_seq: Vec<Option<u32>>,
    last_ddc0: u32,
    atten: u8,
    ppm: f64,
    tx_timeout: f64,
    tx: Tx2,
    seq: [u32; 5],
    short: Box<[u8; p2::SHORT]>,
    long: Box<[u8; p2::LONG]>,
    duc: Box<[u8; p2::LONG]>,
    stopped: bool,
}

const SEQ_GENERAL: usize = 0;
const SEQ_RX: usize = 1;
const SEQ_TX: usize = 2;
const SEQ_HP: usize = 3;
const SEQ_DUC: usize = 4;

impl Ctl2 {
    fn next(&mut self, which: usize) -> u32 {
        let s = self.seq[which];
        self.seq[which] = s.wrapping_add(1);
        s
    }
    fn ddc0_freq(&self) -> u32 {
        self.rx
            .iter()
            .flatten()
            .next()
            .map(|r| r.freq)
            .unwrap_or(self.last_ddc0)
    }
    fn enabled(&self) -> usize {
        self.rx.iter().filter(|r| r.is_some()).count()
    }
}

/// One radio's protocol 2 stream.
pub struct Stream2 {
    p: Profile,
    base: SocketAddr,
    label: String,
    sock: Mutex<Box<dyn Datagram>>,
    ctl: Mutex<Ctl2>,
    lost: AtomicBool,
    stopping: AtomicBool,
    thread: Mutex<Option<JoinHandle<()>>>,
    board: Mutex<Board>,
    board_id: Option<u8>,
}

impl Stream2 {
    /// Opens the stream for receiver `n` (DDC n - 1) and starts its thread.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        p: Profile,
        hub: &hub::Hub,
        net: &dyn Transport,
        host: &str,
        port: u16,
        n: usize,
        params: &RxParams,
        notices: &Notices,
        abort: &Arc<Abort>,
    ) -> Result<(Arc<Stream2>, Arc<RxSlot>, String), DriverError> {
        let name = p.name;
        let base = net.resolve(host, port)?;
        let label = format!("{host}:{port}");
        let mut rx_sock = hub::socket(net)?;
        let mut sock = rx_sock.try_clone()?;

        let mut facts = Facts::default();
        let _ = sock.send_to(&p2::discovery_request(), base);
        let end = Instant::now() + OPEN_DISCOVERY;
        let mut buf = [0u8; 1500];
        while Instant::now() < end && !abort.fired() {
            if let Ok((len, from)) = rx_sock.recv_from(&mut buf) {
                if from.ip() != base.ip() {
                    continue;
                }
                if let Some(f) = discover::parse(&p, &buf[..len], from) {
                    hub.remember(&label, f.board, f.receivers);
                    if f.busy {
                        notices.log(format!(
                            "{name}: the radio is streaming to another program; it is taken over"
                        ));
                    }
                    facts = f.facts();
                    break;
                }
            }
        }
        if let Some(k) = facts.receivers {
            if n > usize::from(k) {
                return Err(DriverError::Streams {
                    given: n,
                    has: usize::from(k),
                });
            }
        }
        let board_id = facts.board.or(hub.known(&label).board);
        let slot = RxSlot::new(n, notices.clone(), params);
        let mut rx: Vec<Option<Rx2>> = (0..p.max_receivers as usize).map(|_| None).collect();
        let rate = plan::choose_rate(params.rate_asked, p.rates(board_id), params.rate_exact)?;
        rx[n - 1] = Some(Rx2 {
            slot: slot.clone(),
            freq: params.freq,
            rate,
        });
        let atten = match params.gain {
            Gain::Atten(a) => a,
            _ => 0,
        };
        let ctl = Ctl2 {
            rx,
            last_seq: vec![None; p.max_receivers as usize],
            last_ddc0: params.freq,
            atten,
            ppm: params.ppm,
            tx_timeout: params.tx_timeout,
            tx: Tx2::default(),
            seq: [0; 5],
            short: Box::new([0; p2::SHORT]),
            long: Box::new([0; p2::LONG]),
            duc: Box::new([0; p2::LONG]),
            stopped: false,
        };
        let s = Arc::new(Stream2 {
            p,
            base,
            label: label.clone(),
            sock: Mutex::new(sock),
            ctl: Mutex::new(ctl),
            lost: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            thread: Mutex::new(None),
            board: Mutex::new(Board {
                facts,
                meter: Meter::default(),
                thru: Thru::new(p.max_receivers as usize),
                underruns: 0,
            }),
            board_id,
        });
        {
            let mut c = lock(&s.ctl);
            s.send_general(&mut c);
            s.send_rx(&mut c);
            s.send_tx(&mut c);
            s.send_hp(&mut c);
        }
        let ddc_port = port::DDC0 + (n as u16 - 1) + base.port();
        let t0 = Instant::now();
        let mut last = t0;
        let first = loop {
            if abort.fired() {
                return Err(NetError::Aborted.into());
            }
            if t0.elapsed() >= FIRST_SAMPLES {
                return Err(NetError::TimedOut.into());
            }
            if last.elapsed() >= START_RESEND_OPEN {
                let mut c = lock(&s.ctl);
                s.send_general(&mut c);
                s.send_hp(&mut c);
                last = Instant::now();
            }
            if let Ok((len, from)) = rx_sock.recv_from(&mut buf) {
                if from.ip() == base.ip()
                    && from.port() == ddc_port
                    && p2::ddc_count(&buf[..len]).is_some()
                {
                    break p2::sequence(&buf[..len]);
                }
            }
        };
        lock(&s.ctl).last_seq[n - 1] = Some(first);
        notices.log(format!(
            "{name}: streaming from {label} at {} ksps on DDC{}",
            rate / 1000,
            n - 1
        ));
        let s2 = s.clone();
        let t = std::thread::Builder::new()
            .name("sdrkit-hpsdr2".into())
            .spawn(move || s2.run(rx_sock))
            .map_err(|_| DriverError::Thread)?;
        *lock(&s.thread) = Some(t);
        let note = s.update(n, params)?;
        Ok((s, slot, note))
    }

    fn to(&self, off: u16) -> SocketAddr {
        SocketAddr::new(self.base.ip(), self.base.port().wrapping_add(off))
    }

    /// Sends the General packet. Its PA enable bit is set only while PTT is on.
    fn send_general(&self, c: &mut Ctl2) {
        let seq = c.next(SEQ_GENERAL);
        let alex = matches!(self.board_id, Some(3..=5));
        p2::general(&mut c.short, seq, c.tx.ptt, alex);
        let _ = lock(&self.sock).send_to(&c.short[..], self.to(port::GENERAL));
    }

    fn send_rx(&self, c: &mut Ctl2) {
        let seq = c.next(SEQ_RX);
        let mut ddcs = [Ddc {
            enabled: false,
            rate_ksps: 0,
        }; p2::MAX_DDC];
        for (i, r) in c.rx.iter().enumerate() {
            if let Some(r) = r {
                ddcs[i] = Ddc {
                    enabled: true,
                    rate_ksps: (r.rate / 1000) as u16,
                };
            }
        }
        p2::rx_specific(&mut c.long, seq, Profile::adcs(self.board_id), &ddcs);
        let _ = lock(&self.sock).send_to(&c.long[..], self.to(port::RX_SPECIFIC));
    }

    fn send_tx(&self, c: &mut Ctl2) {
        let seq = c.next(SEQ_TX);
        p2::tx_specific(&mut c.short, seq);
        let _ = lock(&self.sock).send_to(&c.short[..], self.to(port::TX_SPECIFIC));
    }

    fn send_hp(&self, c: &mut Ctl2) {
        let seq = c.next(SEQ_HP);
        let d0 = plan::phase_word(c.ddc0_freq());
        let mut hp = HighPriority {
            run: !c.stopped,
            ptt: c.tx.ptt,
            atten: c.atten,
            ..Default::default()
        };
        for (i, w) in hp.ddc.iter_mut().enumerate() {
            *w =
                c.rx.get(i)
                    .and_then(|r| r.as_ref())
                    .map(|r| plan::phase_word(r.freq))
                    .unwrap_or(d0);
        }
        hp.duc = if c.tx.ptt {
            plan::phase_word(c.tx.freq)
        } else {
            d0
        };
        hp.drive = if c.tx.ptt { c.tx.drive } else { 0 };
        hp.alex = plan::alex_hpf(c.ddc0_freq())
            | if c.tx.ptt {
                plan::alex_lpf(c.tx.freq)
            } else {
                0
            };
        p2::high_priority(&mut c.long, seq, &hp);
        let _ = lock(&self.sock).send_to(&c.long[..], self.to(port::HIGH_PRIORITY));
    }

    fn send_duc(&self, c: &mut Ctl2) {
        let mut samples = [Complex32::new(0.0, 0.0); p2::DUC_SAMPLES];
        if let Some(q) = &c.tx.owner {
            c.tx.starved = q.pull(&mut samples);
        }
        let seq = c.next(SEQ_DUC);
        c.duc.fill(0);
        c.duc[..4].copy_from_slice(&seq.to_be_bytes());
        for (i, s) in samples.iter().enumerate() {
            p2::duc_sample(&mut c.duc[..], i, plan::tx24(s.re), plan::tx24(s.im));
        }
        let _ = lock(&self.sock).send_to(&c.duc[..], self.to(port::DUC0));
    }

    fn log_all(&self, c: &Ctl2, line: &str) {
        for r in c.rx.iter().flatten() {
            r.slot.notices.log(line.to_string());
        }
    }

    fn end_tx(&self, c: &mut Ctl2, why: &str, send: bool) {
        if !c.tx.ptt {
            return;
        }
        c.tx.ptt = false;
        c.tx.since = None;
        if let Some(q) = &c.tx.owner {
            q.set_state(TxState::Disarmed);
            if !why.is_empty() {
                q.note(format!("{}: {why}", self.p.name));
            }
        }
        if !why.is_empty() {
            self.log_all(c, &format!("{}: {why}", self.p.name));
        }
        if send {
            self.send_hp(c);
            self.send_hp(c);
            self.send_general(c);
        }
    }

    fn tx_checks(&self, c: &mut Ctl2) {
        if !c.tx.ptt {
            return;
        }
        if let Some(since) = c.tx.since {
            let limit = c.tx_timeout;
            if since.elapsed().as_secs_f64() > limit {
                self.end_tx(c, &format!("transmit timed out after {limit:.0} s"), true);
                return;
            }
        }
        if c.tx.starved >= TX_STARVED {
            self.end_tx(c, "no transmit samples for 2 s: transmit ended", true);
        }
    }

    fn run(self: Arc<Self>, mut sock: Box<dyn Datagram>) {
        let mut pool = Pool::default();
        let mut last_valid = Instant::now();
        let mut last_hp = Instant::now();
        let mut last_general = Instant::now();
        let radio_ip = self.base.ip();
        let base = self.base.port();
        let mut fast = false;
        loop {
            if self.stopping.load(Ordering::Acquire) {
                break;
            }
            let ptt = lock(&self.ctl).tx.ptt;
            if ptt != fast {
                // While transmitting, the thread wakes every millisecond to pace the DUC packets.
                fast = ptt;
                let _ = sock.set_read_timeout(Some(if ptt {
                    Duration::from_millis(1)
                } else {
                    hub::RECV_TIMEOUT
                }));
            }
            let got = {
                let b = pool.buffer();
                match sock.recv_from(&mut b.buf) {
                    Ok((len, a)) if a.ip() == radio_ip => {
                        b.len = len;
                        Some(a.port().wrapping_sub(base))
                    }
                    _ => None,
                }
            };
            if self.stopping.load(Ordering::Acquire) {
                break;
            }
            let now = Instant::now();
            let mut c = lock(&self.ctl);
            match got {
                Some(port::RX_SPECIFIC) => {
                    let pkt = pool.take();
                    if let Some(st) = p2::Status::parse(pkt.bytes()) {
                        let mut b = lock(&self.board);
                        let m = &mut b.meter;
                        if st.overload {
                            m.overload_at = Some(now);
                        }
                        m.ptt = st.ptt;
                        m.dot = st.dot;
                        m.dash = st.dash;
                        m.pll = Some(st.pll);
                        m.exciter = Some(st.exciter);
                        m.fwd = Some(st.fwd);
                        m.rev = Some(st.rev);
                        m.supply = Some(st.supply);
                    }
                }
                Some(off) if off >= port::DDC0 && ((off - port::DDC0) as usize) < c.rx.len() => {
                    let d = (off - port::DDC0) as usize;
                    let open = c.rx[d].is_some();
                    let count = p2::ddc_count(pool.buffer().bytes());
                    if let (true, Some(count)) = (open, count) {
                        last_valid = now;
                        let pkt = pool.take();
                        let seq = p2::sequence(pkt.bytes());
                        let mut b = lock(&self.board);
                        b.thru.packet(pkt.len);
                        let r = c.rx[d].as_ref().expect("open");
                        if let Some(l) = c.last_seq[d] {
                            let gap = seq.wrapping_sub(l).wrapping_sub(1);
                            if gap > 0 && gap < GAP_RESTART {
                                let lost = gap as u64 * p2::DDC_SAMPLES as u64;
                                b.thru.gap();
                                if r.slot.add_loss(lost, false) {
                                    b.thru.samples(d + 1, lost);
                                }
                            }
                        }
                        if r.slot.push(&pkt, 0, count) {
                            b.thru.samples(d + 1, count as u64);
                        }
                        drop(b);
                        c.last_seq[d] = Some(seq);
                    }
                }
                _ => {}
            }
            if now.duration_since(last_valid) >= LOST_AFTER {
                self.lose(&mut c);
                break;
            }
            // No keep-alive once stopped: the stop's two run = 0 packets, 10 ms apart, are the last ones .
            if !c.stopped && now.duration_since(last_general) >= GENERAL_EVERY {
                self.send_general(&mut c);
                last_general = now;
            }
            if !c.stopped && now.duration_since(last_hp) >= KEEPALIVE {
                self.send_hp(&mut c);
                last_hp = now;
            }
            if c.tx.ptt {
                if let Some(since) = c.tx.since {
                    let due = (since.elapsed().as_secs_f64() / DUC_PERIOD.as_secs_f64()) as u64;
                    let mut k = 0;
                    while c.tx.sent < due && k < 8 {
                        self.send_duc(&mut c);
                        c.tx.sent += 1;
                        k += 1;
                    }
                }
                self.tx_checks(&mut c);
                let u =
                    c.tx.owner
                        .as_ref()
                        .map(|q| q.stats().underrun_samples)
                        .unwrap_or(0);
                lock(&self.board).underruns = u;
            }
        }
    }

    fn lose(&self, c: &mut Ctl2) {
        let why = format!("{}: no samples from {} for 3 s", self.p.name, self.label);
        let err = DriverError::Net(NetError::TimedOut);
        self.lost.store(true, Ordering::Release);
        self.end_tx(c, "", false);
        if let Some(q) = &c.tx.owner {
            q.set_state(TxState::Closed);
        }
        self.log_all(c, &why);
        for r in c.rx.iter().flatten() {
            r.slot.lost(&err);
        }
    }
}

impl Stream for Stream2 {
    fn join(
        &self,
        n: usize,
        p: &RxParams,
        notices: &Notices,
    ) -> Result<(Arc<RxSlot>, String), JoinError> {
        let mut c = lock(&self.ctl);
        if c.stopped || self.lost.load(Ordering::Acquire) {
            return Err(JoinError::Unusable);
        }
        if c.rx[n - 1].is_some() {
            return Err(JoinError::Failed(DriverError::driver(
                HpsdrError::ReceiverInUse { receiver: n },
            )));
        }
        let rate = plan::choose_rate(p.rate_asked, self.p.rates(self.board_id), p.rate_exact)
            .map_err(JoinError::Failed)?;
        let slot = RxSlot::new(n, notices.clone(), p);
        c.rx[n - 1] = Some(Rx2 {
            slot: slot.clone(),
            freq: p.freq,
            rate,
        });
        c.last_seq[n - 1] = None;
        self.send_rx(&mut c);
        self.send_hp(&mut c);
        drop(c);
        Ok((slot, self.update(n, p).map_err(JoinError::Failed)?))
    }

    fn update(&self, n: usize, p: &RxParams) -> Result<String, DriverError> {
        let rate = plan::choose_rate(p.rate_asked, self.p.rates(self.board_id), p.rate_exact)?;
        let mut c = lock(&self.ctl);
        let Some(r) = c.rx.get_mut(n - 1).and_then(|r| r.as_mut()) else {
            return Ok(String::new());
        };
        r.freq = p.freq;
        r.slot.set(p);
        let rate_changed = r.rate != rate;
        r.rate = rate;
        if let Gain::Atten(a) = p.gain {
            c.atten = a;
        }
        c.last_ddc0 = c.ddc0_freq();
        c.ppm = p.ppm;
        c.tx_timeout = p.tx_timeout;
        if rate_changed {
            lock(&self.board).thru.restart();
            self.send_rx(&mut c);
        }
        self.send_hp(&mut c);
        Ok(plan::freq_note(&self.p, p.freq))
    }

    fn leave(&self, n: usize) {
        let mut c = lock(&self.ctl);
        let Some(r) = c.rx.get_mut(n - 1).and_then(|r| r.take()) else {
            return;
        };
        r.slot.close();
        let lost = self.lost.load(Ordering::Acquire);
        if c.enabled() > 0 {
            if !lost {
                self.send_rx(&mut c);
                self.send_hp(&mut c);
            }
            return;
        }
        if c.stopped {
            return;
        }
        self.end_tx(&mut c, "the stream stopped: transmit ended", !lost);
        if let Some(q) = c.tx.owner.take() {
            q.set_state(TxState::Closed);
        }
        c.stopped = true;
        if !lost {
            self.send_hp(&mut c);
            drop(c);
            std::thread::sleep(Duration::from_millis(10));
            let mut c = lock(&self.ctl);
            self.send_hp(&mut c);
        } else {
            drop(c);
        }
        self.stopping.store(true, Ordering::Release);
        if let Some(t) = lock(&self.thread).take() {
            if t.thread().id() != std::thread::current().id() {
                let _ = t.join();
            }
        }
    }

    fn rate(&self, n: usize) -> u32 {
        let c = lock(&self.ctl);
        c.rx.get(n - 1)
            .and_then(|r| r.as_ref())
            .map(|r| r.rate)
            .unwrap_or(0)
    }

    fn lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }

    fn facts(&self) -> Facts {
        lock(&self.board).facts.clone()
    }

    fn write_readings(&self, n: usize, r: &mut Results) {
        let (en, freq, slot) = {
            let c = lock(&self.ctl);
            let x = c.rx.get(n - 1).and_then(|r| r.as_ref());
            (
                c.enabled(),
                x.map(|r| r.freq).unwrap_or(0),
                x.map(|r| r.slot.clone()),
            )
        };
        let Some(slot) = slot else { return };
        let b = lock(&self.board);
        hub::write_readings(&self.p, &b, &slot, en, freq, r);
    }

    fn status(&self, n: usize) -> String {
        let (rate, en, freq) = {
            let c = lock(&self.ctl);
            let r = c.rx.get(n - 1).and_then(|r| r.as_ref());
            (
                r.map(|r| r.rate).unwrap_or(0),
                c.enabled(),
                r.map(|r| r.freq).unwrap_or(0),
            )
        };
        let b = lock(&self.board);
        let mut s = format!(
            "{} RX{n} (DDC{}) at {} ksps, {en} DDC{} enabled, {:.3} MHz",
            self.p.name,
            n - 1,
            rate / 1000,
            if en == 1 { "" } else { "s" },
            freq as f64 / 1e6
        );
        if let Some(t) = &b.thru.last {
            s.push_str(&format!("; {:.1} Mbit/s, {:.0} packets/s", t.mbps, t.pps));
        }
        if b.meter.overload() {
            s.push_str("; ADC overload");
        }
        if b.meter.pll == Some(false) {
            s.push_str("; PLL unlocked");
        }
        s
    }

    fn claim(&self, q: &Arc<TxQueue>) -> Result<(), DriverError> {
        let mut c = lock(&self.ctl);
        if hub::other_owner(c.tx.owner.as_ref(), q) {
            return Err(DriverError::driver(HpsdrError::TransmitterTaken));
        }
        c.tx.owner = Some(q.clone());
        Ok(())
    }

    fn tx_arm(&self, q: &Arc<TxQueue>, freq: u32, drive: u8) -> Result<(), DriverError> {
        let mut c = lock(&self.ctl);
        if c.stopped || self.lost.load(Ordering::Acquire) {
            return Err(DriverError::NotNow {
                state: Doing::Stopped,
            });
        }
        if hub::other_owner(c.tx.owner.as_ref(), q) {
            return Err(DriverError::driver(HpsdrError::TransmitterTaken));
        }
        c.tx.owner = Some(q.clone());
        c.tx.freq = freq;
        c.tx.drive = drive;
        if !c.tx.ptt {
            c.tx.ptt = true;
            c.tx.since = Some(Instant::now());
            c.tx.sent = 0;
            c.tx.starved = Duration::ZERO;
        }
        q.set_state(TxState::Armed);
        self.send_general(&mut c);
        self.send_hp(&mut c);
        Ok(())
    }

    fn tx_set(&self, freq: u32, drive: u8) {
        let mut c = lock(&self.ctl);
        c.tx.freq = freq;
        c.tx.drive = drive;
        if c.tx.ptt {
            self.send_hp(&mut c);
        }
    }

    fn tx_end(&self, why: &str) {
        let mut c = lock(&self.ctl);
        let send = !self.lost.load(Ordering::Acquire) && !c.stopped;
        self.end_tx(&mut c, why, send);
    }

    fn ppm(&self) -> f64 {
        lock(&self.ctl).ppm
    }
}
