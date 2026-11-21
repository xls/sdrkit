//! The protocol 1 stream of one radio: one UDP socket, one thread, every receiver of the radio fed from it, and the transmit
//! samples in the EP2 packets.
//! Standard: HPSDR USB data protocol, carried over UDP as the Metis documents describe it.

use crate::discover;
use crate::hub::{
    self, lock, Board, Facts, JoinError, Meter, Pool, RxParams, RxSlot, Stream, Thru, TxQueue,
};
use crate::p1;
use crate::plan::{self, Gain};
use crate::profile::{FilterBoard, Model, Profile};
use crate::HpsdrError;
use core::net::SocketAddr;
use sdrkit_core::net::{Datagram, NetError, Transport};
use sdrkit_core::{Complex32, Results};
use sdrkit_driver_core::net::Abort;
use sdrkit_driver_core::runner::Notices;
use sdrkit_driver_core::{Doing, DriverError, TxState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long discovery at open waits for a reply.
pub const OPEN_DISCOVERY: Duration = Duration::from_millis(300);
/// How long an open waits for the first samples .
pub const FIRST_SAMPLES: Duration = Duration::from_millis(1500);
/// How often an open resends the start command while waiting.
pub const START_RESEND_OPEN: Duration = Duration::from_millis(400);
/// No valid packet for this long: the start command is sent again, at most this often.
pub const START_RESEND: Duration = Duration::from_millis(500);
/// No valid packet for this long: the stream is lost.
pub const LOST_AFTER: Duration = Duration::from_secs(3);
/// Packets that do not fit the new layout are dropped for at most this long.
pub const SETTLE: Duration = Duration::from_millis(300);
/// A join waits this long for the stream to carry its layout.
pub const JOIN_WAIT: Duration = Duration::from_secs(3);
/// A sequence gap this large is a restart of the radio's counter.
pub const GAP_RESTART: u32 = 100_000;
/// While transmitting, at least one EP2 packet goes this often.
pub const TX_MIN_GAP: Duration = Duration::from_millis(100);
/// Transmit ends after the host supplied no samples for this long.
pub const TX_STARVED: Duration = Duration::from_secs(2);

struct RxReg {
    slot: Arc<RxSlot>,
    freq: u32,
    n2adr: bool,
}

#[derive(Default)]
struct TxReg {
    owner: Option<Arc<TxQueue>>,
    mox: bool,
    freq: u32,
    drive: u8,
    since: Option<Instant>,
    sent: u64,
    rx_samples: u64,
    starved: Duration,
}

struct Ctl {
    rate: u32,
    n_stream: usize,
    rx: Vec<Option<RxReg>>,
    last_low: u32,
    lna: i32,
    atten: u8,
    ppm: f64,
    tx_timeout: f64,
    hl2pa: bool,
    tx: TxReg,
    pos: usize,
    seq: u32,
    buf: Box<[u8; p1::PACKET]>,
    settling: bool,
    settle_since: Instant,
    stopped: bool,
}

impl Ctl {
    /// The receiver count needed: the highest open receiver number.
    fn needed(&self) -> Option<usize> {
        self.rx.iter().rposition(|r| r.is_some()).map(|i| i + 1)
    }
    fn open_count(&self) -> usize {
        self.rx.iter().filter(|r| r.is_some()).count()
    }
    fn lowest_freq(&self) -> u32 {
        self.rx
            .iter()
            .flatten()
            .next()
            .map(|r| r.freq)
            .unwrap_or(self.last_low)
    }
}

/// One radio's protocol 1 stream.
pub struct Stream1 {
    p: Profile,
    addr: SocketAddr,
    label: String,
    sock: Mutex<Box<dyn Datagram>>,
    ctl: Mutex<Ctl>,
    layout_cv: Condvar,
    lost: AtomicBool,
    stopping: AtomicBool,
    thread: Mutex<Option<JoinHandle<()>>>,
    board: Mutex<Board>,
}

impl Stream1 {
    /// Opens the stream for receiver `n`  and starts its thread.
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
    ) -> Result<(Arc<Stream1>, Arc<RxSlot>, String), DriverError> {
        let name = p.name;
        let addr = net.resolve(host, port)?;
        let label = format!("{host}:{port}");
        let mut rx_sock = hub::socket(net)?;
        let mut sock = rx_sock.try_clone()?;

        // Discovery at open.
        let mut facts = Facts::default();
        let _ = sock.send_to(&p1::discovery_request(), addr);
        let end = Instant::now() + OPEN_DISCOVERY;
        let mut buf = [0u8; 1500];
        while Instant::now() < end && !abort.fired() {
            match rx_sock.recv_from(&mut buf) {
                Ok((len, from)) if from.ip() == addr.ip() => {
                    if let Some(f) = discover::parse(&p, &buf[..len], from) {
                        hub.remember(&label, f.board, f.receivers);
                        facts = f.facts();
                        break;
                    }
                }
                _ => {}
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

        let slot = RxSlot::new(n, notices.clone(), params);
        let mut rx: Vec<Option<RxReg>> = (0..p.max_receivers as usize).map(|_| None).collect();
        rx[n - 1] = Some(RxReg {
            slot: slot.clone(),
            freq: params.freq,
            n2adr: params.n2adr,
        });
        let mut ctl = Ctl {
            rate: params.rate,
            n_stream: n,
            rx,
            last_low: params.freq,
            lna: plan::LNA_DEFAULT_DB,
            atten: 0,
            ppm: params.ppm,
            tx_timeout: params.tx_timeout,
            hl2pa: params.hl2pa,
            tx: TxReg::default(),
            pos: 0,
            seq: 0,
            buf: Box::new([0u8; p1::PACKET]),
            settling: true,
            settle_since: Instant::now(),
            stopped: false,
        };
        apply_gain(&mut ctl, params.gain);
        let s = Arc::new(Stream1 {
            p,
            addr,
            label: label.clone(),
            sock: Mutex::new(sock),
            ctl: Mutex::new(ctl),
            layout_cv: Condvar::new(),
            lost: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            thread: Mutex::new(None),
            board: Mutex::new(Board {
                facts,
                meter: Meter::default(),
                thru: Thru::new(p.max_receivers as usize),
                underruns: 0,
            }),
        });

        // Registers, start, registers.
        {
            let mut c = lock(&s.ctl);
            s.send_ep2(&mut c);
            s.send_ep2(&mut c);
        }
        s.send_command(1);
        {
            let mut c = lock(&s.ctl);
            s.send_ep2(&mut c);
            s.send_ep2(&mut c);
        }

        // The first samples.
        let t0 = Instant::now();
        let mut last_start = t0;
        let first_seq = loop {
            if abort.fired() {
                return Err(NetError::Aborted.into());
            }
            if t0.elapsed() >= FIRST_SAMPLES {
                return Err(NetError::TimedOut.into());
            }
            if last_start.elapsed() >= START_RESEND_OPEN {
                s.send_command(1);
                last_start = Instant::now();
            }
            match rx_sock.recv_from(&mut buf) {
                Ok((len, from)) if from.ip() == addr.ip() && p1::is_ep6(&buf[..len]) => {
                    break p1::sequence(&buf[..len]);
                }
                _ => {}
            }
        };
        {
            let mut c = lock(&s.ctl);
            // The settling window runs from the first packet, not from the first start command, which the radio can miss.
            c.settle_since = Instant::now();
            s.send_ep2(&mut c);
        }
        notices.log(format!(
            "{name}: streaming from {label} at {} ksps, {n} receiver{}",
            params.rate / 1000,
            if n == 1 { "" } else { "s" }
        ));
        let s2 = s.clone();
        let t = std::thread::Builder::new()
            .name("sdrkit-hpsdr1".into())
            .spawn(move || s2.run(rx_sock, first_seq))
            .map_err(|_| DriverError::Thread)?;
        *lock(&s.thread) = Some(t);
        let note = s.update(n, params)?;
        Ok((s, slot, note))
    }

    fn send_command(&self, cmd: u8) {
        let _ = lock(&self.sock).send_to(&p1::command(cmd), self.addr);
    }

    /// The register cycle of the profile with `n` receivers. The drive register is last: the registers of the receivers
    /// reach the radio first.
    fn cycle(&self, c: &Ctl, out: &mut [u8; 20]) -> usize {
        let mut k = 0;
        let mut put = |r: u8| {
            out[k] = r;
            k += 1;
        };
        put(0x00);
        put(0x01);
        for i in 1..=c.n_stream {
            put(p1::nco_register(i));
        }
        if self.p.model != Model::RedPitaya {
            put(0x0a);
        }
        put(0x09);
        k
    }

    fn value(&self, c: &Ctl, reg: u8) -> u32 {
        let n = c.n_stream;
        match reg {
            0x00 => {
                let filter = if self.p.filter == FilterBoard::N2adr {
                    c.rx.iter()
                        .flatten()
                        .filter(|r| r.n2adr)
                        .map(|r| r.freq)
                        .max()
                        .map(plan::n2adr_bits)
                        .unwrap_or(0)
                } else {
                    0
                };
                let c4 = ((n as u8 - 1) << 3) | 0x04;
                u32::from_be_bytes([plan::rate_code(c.rate), filter, 0, c4])
            }
            0x01 => {
                if c.tx.mox {
                    c.tx.freq
                } else {
                    c.lowest_freq()
                }
            }
            0x09 => {
                let drive = if c.tx.mox { c.tx.drive } else { 0 };
                let pa = self.p.model == Model::HermesLite2 && c.tx.mox && c.hl2pa;
                u32::from_be_bytes([drive, if pa { 0x08 } else { 0 }, 0, 0])
            }
            0x0a => {
                if self.p.model == Model::HermesLite2 {
                    plan::lna_byte(c.lna) as u32
                } else {
                    0x20 | c.atten as u32
                }
            }
            r => {
                let i = if (0x02..=0x08).contains(&r) {
                    (r - 0x01) as usize
                } else {
                    (r - 0x12 + 8) as usize
                };
                c.rx.get(i - 1)
                    .and_then(|x| x.as_ref())
                    .map(|x| x.freq)
                    .unwrap_or_else(|| c.lowest_freq())
            }
        }
    }

    /// Builds and sends the next EP2 packet: two registers of the cycle, and transmit samples while MOX is on.
    fn send_ep2(&self, c: &mut Ctl) {
        let mut cyc = [0u8; 20];
        let len = self.cycle(c, &mut cyc);
        c.pos %= len;
        let (r0, r1) = (cyc[c.pos], cyc[(c.pos + 1) % len]);
        c.pos = (c.pos + 2) % len;
        let mox = c.tx.mox;
        let (v0, v1) = (self.value(c, r0), self.value(c, r1));
        let seq = c.seq;
        c.seq = c.seq.wrapping_add(1);
        let mut samples = [Complex32::new(0.0, 0.0); p1::TX_PER_PACKET];
        if mox {
            if let Some(q) = &c.tx.owner {
                c.tx.starved = q.pull(&mut samples);
            }
        }
        let b = &mut c.buf[..];
        b.fill(0);
        p1::ep2_header(b, seq);
        p1::ep2_frame(b, 0, r0, mox, v0);
        p1::ep2_frame(b, 1, r1, mox, v1);
        if mox {
            for (i, s) in samples.iter().enumerate() {
                p1::tx_slot(
                    b,
                    i / p1::TX_SLOTS,
                    i % p1::TX_SLOTS,
                    plan::tx16(s.re),
                    plan::tx16(s.im),
                );
            }
        }
        let _ = lock(&self.sock).send_to(&c.buf[..], self.addr);
    }

    fn log_all(&self, c: &Ctl, line: &str) {
        for r in c.rx.iter().flatten() {
            r.slot.notices.log(line.to_string());
        }
    }

    /// Ends a transmission: MOX, drive and PA cleared at once and in the next two packets.
    fn end_tx(&self, c: &mut Ctl, why: &str, send: bool) {
        if !c.tx.mox {
            return;
        }
        c.tx.mox = false;
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
            self.send_ep2(c);
            self.send_ep2(c);
        }
    }

    fn tx_checks(&self, c: &mut Ctl) {
        if !c.tx.mox {
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

    fn run(self: Arc<Self>, mut sock: Box<dyn Datagram>, first_seq: u32) {
        let mut pool = Pool::default();
        let mut last_seq = Some(first_seq);
        let mut last_valid = Instant::now();
        let mut last_start = Instant::now();
        let mut last_ep2 = Instant::now();
        let radio_ip = self.addr.ip();
        loop {
            if self.stopping.load(Ordering::Acquire) {
                break;
            }
            let valid = {
                let b = pool.buffer();
                match sock.recv_from(&mut b.buf) {
                    Ok((len, a)) => {
                        b.len = len;
                        a.ip() == radio_ip && p1::is_ep6(b.bytes())
                    }
                    Err(_) => false,
                }
            };
            if self.stopping.load(Ordering::Acquire) {
                break;
            }
            let now = Instant::now();
            let mut c = lock(&self.ctl);
            if let Some(need) = c.needed() {
                if need != c.n_stream {
                    c.n_stream = need;
                    c.settling = true;
                    c.settle_since = now;
                    let line = format!(
                        "{}: {} now streams {need} receiver{}",
                        self.p.name,
                        self.label,
                        if need == 1 { "" } else { "s" }
                    );
                    self.log_all(&c, &line);
                    lock(&self.board).thru.restart();
                }
            }
            if !valid {
                let quiet = now.duration_since(last_valid);
                if quiet >= LOST_AFTER {
                    self.lose(&mut c);
                    break;
                }
                if quiet >= START_RESEND && now.duration_since(last_start) >= START_RESEND {
                    self.send_command(1);
                    last_start = now;
                }
                if c.tx.mox && now.duration_since(last_ep2) >= TX_MIN_GAP {
                    self.send_ep2(&mut c);
                    c.tx.sent += 1;
                    last_ep2 = now;
                }
                self.tx_checks(&mut c);
                continue;
            }
            last_valid = now;
            let pkt = pool.take();
            let bytes = pkt.bytes();
            let n = c.n_stream;
            let per = p1::per_packet(n);
            let mut board = lock(&self.board);
            board.thru.packet(bytes.len());

            // Sequence.
            let seq = p1::sequence(bytes);
            if let Some(l) = last_seq {
                let gap = seq.wrapping_sub(l).wrapping_sub(1);
                if gap > 0 && gap < GAP_RESTART {
                    let lost = gap as u64 * per as u64;
                    board.thru.gap();
                    for r in c.rx.iter().take(n).flatten() {
                        if r.slot.add_loss(lost, false) {
                            board.thru.samples(r.slot.n, lost);
                        }
                    }
                }
            }
            last_seq = Some(seq);

            // Layout settling.
            let mut write = p1::both_synced(bytes);
            if c.settling {
                if p1::packet_fits(bytes, n) || now.duration_since(c.settle_since) >= SETTLE {
                    c.settling = false;
                    self.layout_cv.notify_all();
                } else {
                    write = false;
                    board.thru.settled();
                    for r in c.rx.iter().take(n).flatten() {
                        if r.slot.add_loss(per as u64, true) {
                            board.thru.samples(r.slot.n, per as u64);
                        }
                    }
                }
            }

            // Status.
            if write {
                let hl2 = self.p.model == Model::HermesLite2;
                for f in 0..2 {
                    status(
                        &mut board.meter,
                        &p1::frame_status(p1::frame(bytes, f), hl2),
                        hl2,
                        now,
                    );
                }
            }

            // Samples to the receivers.
            if write {
                for r in c.rx.iter().take(n).flatten() {
                    if r.slot.push(&pkt, n as u8, per) {
                        board.thru.samples(r.slot.n, per as u64);
                    }
                }
            }
            drop(board);

            // EP2: one per EP6, or the transmit pacing.
            if c.tx.mox {
                c.tx.rx_samples += per as u64;
                let target = c.tx.rx_samples * p1::TX_RATE as u64
                    / c.rate.max(1) as u64
                    / p1::TX_PER_PACKET as u64;
                let mut k = 0;
                while c.tx.sent < target && k < 16 && c.tx.mox {
                    self.send_ep2(&mut c);
                    c.tx.sent += 1;
                    k += 1;
                    last_ep2 = now;
                }
                if c.tx.mox && now.duration_since(last_ep2) >= TX_MIN_GAP {
                    self.send_ep2(&mut c);
                    c.tx.sent += 1;
                    last_ep2 = now;
                }
            } else {
                self.send_ep2(&mut c);
                last_ep2 = now;
            }
            self.tx_checks(&mut c);
            let under = c.tx.owner.as_ref().map(|q| q.stats().underrun_samples);
            drop(c);
            if let Some(u) = under {
                lock(&self.board).underruns = u;
            }
        }
    }

    fn lose(&self, c: &mut MutexGuard<'_, Ctl>) {
        let why = format!("{}: no samples from {} for 3 s", self.p.name, self.label);
        let err = DriverError::Net(NetError::TimedOut);
        self.lost.store(true, Ordering::Release);
        self.end_tx(c, "", true);
        if let Some(q) = &c.tx.owner {
            q.set_state(TxState::Closed);
        }
        self.log_all(c, &why);
        for r in c.rx.iter().flatten() {
            r.slot.lost(&err);
        }
        self.layout_cv.notify_all();
    }
}

fn apply_gain(c: &mut Ctl, g: Gain) {
    match g {
        Gain::Lna(db) => c.lna = db,
        Gain::Atten(a) => c.atten = a,
        Gain::Words(_) | Gain::None => {}
    }
}

fn status(m: &mut Meter, s: &p1::FrameStatus, hl2: bool, now: Instant) {
    if s.ack {
        return;
    }
    m.ptt = s.ptt;
    m.dash = s.dash;
    m.dot = s.dot;
    let w = |a: u8, b: u8| ((a as u16) << 8) | b as u16;
    match s.address {
        0 => {
            if s.c[0] & 1 != 0 {
                m.overload_at = Some(now);
            }
            m.versions = Some([s.c[1], s.c[2], s.c[3]]);
        }
        1 => {
            if hl2 {
                m.temp = Some(w(s.c[0], s.c[1]) & 0x0FFF);
            } else {
                m.exciter = Some(w(s.c[0], s.c[1]));
            }
            m.fwd = Some(w(s.c[2], s.c[3]));
        }
        2 => {
            m.rev = Some(w(s.c[0], s.c[1]));
            m.ain3 = Some(w(s.c[2], s.c[3]));
        }
        3 => {
            m.ain4 = Some(w(s.c[0], s.c[1]));
            m.supply = Some(w(s.c[2], s.c[3]));
        }
        4 if s.c[0] & 1 != 0 || s.c[1] & 1 != 0 || s.c[2] & 1 != 0 => m.overload_at = Some(now),
        _ => {}
    }
}

impl Stream for Stream1 {
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
        let slot = RxSlot::new(n, notices.clone(), p);
        c.rx[n - 1] = Some(RxReg {
            slot: slot.clone(),
            freq: p.freq,
            n2adr: p.n2adr,
        });
        drop(c);
        let note = self.update(n, p).map_err(JoinError::Failed);
        let mut c = lock(&self.ctl);
        let note = match note {
            Ok(note) => note,
            Err(e) => {
                c.rx[n - 1] = None;
                return Err(e);
            }
        };
        let end = Instant::now() + JOIN_WAIT;
        loop {
            if self.lost.load(Ordering::Acquire) {
                c.rx[n - 1] = None;
                return Err(JoinError::Failed(NetError::TimedOut.into()));
            }
            let need = c.needed().unwrap_or(n);
            if c.n_stream == need && !c.settling {
                break;
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                c.rx[n - 1] = None;
                return Err(JoinError::Failed(NetError::TimedOut.into()));
            }
            c = self
                .layout_cv
                .wait_timeout(c, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        Ok((slot, note))
    }

    fn update(&self, n: usize, p: &RxParams) -> Result<String, DriverError> {
        let name = self.p.name;
        let mut c = lock(&self.ctl);
        let Some(reg) = c.rx.get_mut(n - 1).and_then(|r| r.as_mut()) else {
            return Ok(String::new());
        };
        reg.freq = p.freq;
        reg.n2adr = p.n2adr;
        reg.slot.set(p);
        apply_gain(&mut c, p.gain);
        c.last_low = c.lowest_freq();
        c.ppm = p.ppm;
        c.tx_timeout = p.tx_timeout;
        c.hl2pa = p.hl2pa;
        let mut notes = Vec::new();
        if p.rate != c.rate {
            if c.open_count() == 1 {
                c.rate = p.rate;
                lock(&self.board).thru.restart();
            } else {
                let asked = if p.rate_asked > 0.0 {
                    (p.rate_asked / 1000.0).round() as u32
                } else {
                    p.rate / 1000
                };
                notes.push(format!(
                    "the receivers of a {name} share one sample rate: RX{n} runs at the {} ksps another receiver of it set, {asked} ksps were asked for",
                    c.rate / 1000
                ));
            }
        }
        notes.push(plan::freq_note(&self.p, p.freq));
        Ok(plan::join_notes(&notes))
    }

    fn leave(&self, n: usize) {
        let mut c = lock(&self.ctl);
        let Some(reg) = c.rx.get_mut(n - 1).and_then(|r| r.take()) else {
            return;
        };
        reg.slot.close();
        if c.needed().is_some() || c.stopped {
            return;
        }
        c.stopped = true;
        let lost = self.lost.load(Ordering::Acquire);
        self.end_tx(&mut c, "the stream stopped: transmit ended", !lost);
        if let Some(q) = c.tx.owner.take() {
            q.set_state(TxState::Closed);
        }
        drop(c);
        if !lost {
            self.send_command(0);
        }
        self.stopping.store(true, Ordering::Release);
        if let Some(t) = lock(&self.thread).take() {
            if t.thread().id() != std::thread::current().id() {
                let _ = t.join();
            }
        }
    }

    fn rate(&self, _n: usize) -> u32 {
        lock(&self.ctl).rate
    }

    fn lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }

    fn facts(&self) -> Facts {
        lock(&self.board).facts.clone()
    }

    fn write_readings(&self, n: usize, r: &mut Results) {
        let (ns, freq, slot) = {
            let c = lock(&self.ctl);
            let reg = c.rx.get(n - 1).and_then(|r| r.as_ref());
            (
                c.n_stream,
                reg.map(|r| r.freq).unwrap_or(0),
                reg.map(|r| r.slot.clone()),
            )
        };
        let Some(slot) = slot else { return };
        let b = lock(&self.board);
        hub::write_readings(&self.p, &b, &slot, ns, freq, r);
    }

    fn status(&self, n: usize) -> String {
        let (rate, ns, freq) = {
            let c = lock(&self.ctl);
            (
                c.rate,
                c.n_stream,
                c.rx.get(n - 1)
                    .and_then(|r| r.as_ref())
                    .map(|r| r.freq)
                    .unwrap_or(0),
            )
        };
        let b = lock(&self.board);
        let mut s = format!(
            "{} RX{n} at {} ksps, {ns} receiver{} in the stream, {:.3} MHz",
            self.p.name,
            rate / 1000,
            if ns == 1 { "" } else { "s" },
            freq as f64 / 1e6
        );
        if let Some(t) = &b.thru.last {
            s.push_str(&format!("; {:.1} Mbit/s, {:.0} packets/s", t.mbps, t.pps));
        }
        if b.meter.overload() {
            s.push_str("; ADC overload");
        }
        if self.p.model == Model::HermesLite2 {
            if let Some(t) = b.meter.temp {
                s.push_str(&format!("; {:.1} °C", plan::hl2_temperature(t)));
            }
        }
        s
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
        if !c.tx.mox {
            c.tx.mox = true;
            c.tx.since = Some(Instant::now());
            c.tx.sent = 0;
            c.tx.rx_samples = 0;
            c.tx.starved = Duration::ZERO;
        }
        q.set_state(TxState::Armed);
        Ok(())
    }

    fn tx_set(&self, freq: u32, drive: u8) {
        let mut c = lock(&self.ctl);
        c.tx.freq = freq;
        c.tx.drive = drive;
    }

    fn tx_end(&self, why: &str) {
        let mut c = lock(&self.ctl);
        let send = !self.lost.load(Ordering::Acquire) && !c.stopped;
        self.end_tx(&mut c, why, send);
    }

    fn ppm(&self) -> f64 {
        lock(&self.ctl).ppm
    }

    fn claim(&self, q: &Arc<TxQueue>) -> Result<(), DriverError> {
        let mut c = lock(&self.ctl);
        if hub::other_owner(c.tx.owner.as_ref(), q) {
            return Err(DriverError::driver(HpsdrError::TransmitterTaken));
        }
        c.tx.owner = Some(q.clone());
        Ok(())
    }
}
