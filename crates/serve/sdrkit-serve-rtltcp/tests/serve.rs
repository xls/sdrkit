//! The rtl_tcp server on a hub, on a fake network and on localhost: loopback into the rtl_tcp client driver, a throttled
//! client beside a fast one, a slice, and refusals.

use sdrkit_core::net::{Conn, FakeTransport, StdNet, Transport};
use sdrkit_core::{Choice, Complex32, Owner, Value, Values};
use sdrkit_driver_core::rtl_tcp::{cmd, command, DirectSampling, RtlTcpDriver};
use sdrkit_driver_core::sample::to_iq;
use sdrkit_driver_core::{ring, DriverKind, SourceDriver};
use sdrkit_serve_core::{
    Answer, Block, ClientHandle, Config, Control, Event, Feeder, Hub, Leave, Listen, Refusal,
    Request, SampleType, Slice,
};
use sdrkit_serve_rtltcp::{RtlTcp, DEFAULT_PORT};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const CENTRE: f64 = 100e6;

/// Byte `n` of the synthetic 8-bit source. The period 251 divides no chunk size, so a lost chunk shows.
fn pattern(n: usize) -> u8 {
    (n % 251) as u8
}

fn wait_for(mut what: impl FnMut() -> bool, secs: f64) -> bool {
    let t = Instant::now();
    while t.elapsed().as_secs_f64() < secs {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    what()
}

/// A control that records every request and accepts it.
#[derive(Default)]
struct Recorder(Mutex<Vec<Request>>);

impl Control for Recorder {
    fn request(&self, _client: &ClientHandle, request: &Request) -> Answer {
        self.0.lock().unwrap().push(*request);
        Answer::Accepted
    }
}

impl Recorder {
    fn seen(&self) -> Vec<Request> {
        self.0.lock().unwrap().clone()
    }
}

fn block(format: SampleType, rate_hz: f64) -> Block {
    Block {
        format,
        centre_hz: CENTRE,
        rate_hz,
        lost: 0,
    }
}

fn listen_on(port: u16) -> Listen {
    Listen {
        host: "127.0.0.1".into(),
        port,
        allow: Vec::new(),
    }
}

/// Feeds `secs` of `bytes_of(offset, len)` at `rate` samples per second of `ss`-byte samples, in blocks of `per_block`
/// samples, paced to the clock. It returns the bytes pushed.
fn paced(
    mut feeder: Feeder,
    b: Block,
    ss: usize,
    per_block: usize,
    secs: f64,
    bytes_of: impl Fn(usize, usize) -> Vec<u8> + Send + 'static,
) -> JoinHandle<usize> {
    std::thread::spawn(move || {
        let start = Instant::now();
        let total = (secs * b.rate_hz) as usize;
        let mut sent = 0;
        while sent < total {
            let n = per_block.min(total - sent);
            let due = Duration::from_secs_f64(sent as f64 / b.rate_hz);
            if let Some(w) = due.checked_sub(start.elapsed()) {
                std::thread::sleep(w);
            }
            feeder.push(&bytes_of(sent * ss, n * ss), &b);
            sent += n;
        }
        sent * ss
    })
}

fn counter_bytes(off: usize, len: usize) -> Vec<u8> {
    (off..off + len).map(pattern).collect()
}

/// A raw client: it reads the greeting, then everything into a buffer until the server closes or `stop` is set. It reads
/// at most `per_10ms` bytes every 10 ms when set.
struct RawClient {
    conn: Box<dyn Conn>,
    got: Arc<Mutex<Vec<u8>>>,
    done: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl RawClient {
    fn connect(t: &Arc<dyn Transport>, port: u16, per_10ms: Option<usize>) -> RawClient {
        let conn = t
            .connect("127.0.0.1", port, false, Duration::from_secs(2))
            .unwrap();
        let mut r = conn.try_clone().unwrap();
        let (got, done, stop) = (
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
        );
        let (g, d, s) = (got.clone(), done.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let _ = r.set_read_timeout(Some(Duration::from_millis(20)));
            let mut buf = vec![0u8; per_10ms.unwrap_or(1 << 16)];
            while !s.load(Ordering::Relaxed) {
                match r.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => g.lock().unwrap().extend_from_slice(&buf[..n]),
                    Err(e) if sdrkit_core::net::timed_out(&e) => {}
                    Err(_) => break,
                }
                if per_10ms.is_some() {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            d.store(true, Ordering::Relaxed);
        });
        RawClient {
            conn,
            got,
            done,
            stop,
            thread: Some(thread),
        }
    }

    fn len(&self) -> usize {
        self.got.lock().unwrap().len()
    }

    fn send(&mut self, c: u8, v: u32) {
        self.conn.write_all(&command(c, v)).unwrap();
    }

    fn finish(mut self) -> Vec<u8> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
        std::mem::take(&mut *self.got.lock().unwrap())
    }
}

#[test]
fn loopback_into_our_client_is_bit_exact_and_carries_the_commands() {
    let rate = 2.048e6;
    let net: Arc<dyn Transport> = Arc::new(FakeTransport::new());
    let (hub, feeder) = Hub::new(
        Config {
            block_bytes: 2 * 16384,
            ..Config::default()
        },
        &block(SampleType::Cu8, rate),
    )
    .unwrap();
    let control = Arc::new(Recorder::default());
    hub.listen(
        &net,
        &listen_on(DEFAULT_PORT),
        Arc::new(RtlTcp::rtl_sdr(5)),
        control.clone(),
    )
    .unwrap();

    let driver = RtlTcpDriver::with_timing(
        net.clone(),
        Duration::from_secs(1000),
        Duration::from_secs(30),
    );
    let kind = DriverKind::new(Box::new(driver.clone()));
    let schema = kind.schema();
    let mut v = Values::defaults(schema);
    for (key, value) in [
        ("centre_hz", Value::Hz(CENTRE)),
        ("rate", Value::Hz(rate)),
        ("gain", Value::F64(30.0)),
        ("agc", Value::Bool(false)),
        ("ds", Value::Choice(DirectSampling::Off.index())),
    ] {
        v.set(schema.id(key).unwrap(), &value).unwrap();
    }
    let mut src = driver
        .open(&format!("127.0.0.1:{DEFAULT_PORT}"), &v)
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cu8);
    src.start(w).unwrap();
    let seen = wait_for(|| control.seen().len() >= 7, 5.0);
    assert!(
        seen,
        "{:?} {:?} {:?}",
        control.seen(),
        hub.take_events(),
        hub.snapshot().clients
    );
    assert_eq!(hub.snapshot().clients.len(), 1);

    let producer = paced(
        feeder,
        block(SampleType::Cu8, rate),
        2,
        16384,
        1.5,
        counter_bytes,
    );
    let mut got = Vec::new();
    while !producer.is_finished() {
        r.wait(1, Duration::from_millis(20));
        r.read_with(1 << 20, |b| got.extend_from_slice(b));
    }
    let pushed = producer.join().unwrap();
    let t = Instant::now();
    while got.len() < pushed && t.elapsed() < Duration::from_secs(5) {
        r.wait(1, Duration::from_millis(20));
        r.read_with(1 << 20, |b| got.extend_from_slice(b));
    }
    assert_eq!(got.len(), pushed, "every byte arrives");
    let bad = got.iter().enumerate().find(|(i, b)| **b != pattern(*i));
    assert_eq!(bad, None, "bytes arrive as sent");
    let c = &hub.snapshot().clients[0];
    assert_eq!(
        (c.dropped_samples, c.dropped_chunks, c.drop_events, c.gaps),
        (0, 0, 0, 0)
    );
    let o = |name, a| Request::Other { name, args: [a, 0] };
    assert_eq!(
        control.seen()[..7],
        [
            Request::Agc { on: false },
            o("digital_agc", 0),
            Request::Rate { hz: rate },
            o("direct_sampling", 0),
            Request::Correction { ppm: 0.0 },
            Request::Frequency { hz: CENTRE },
            Request::Gain { db: 29.7 },
        ]
    );
    src.stop();
}

#[test]
fn a_throttled_client_loses_only_its_own_samples_and_is_cut() {
    let rate = 2.4e6;
    let net: Arc<dyn Transport> = Arc::new(FakeTransport::new());
    let drop_after = 0.5;
    let (hub, feeder) = Hub::new(
        Config {
            queue_ms: 100,
            drop_after_s: drop_after,
            block_bytes: 2 * 12_000,
            ..Config::default()
        },
        &block(SampleType::Cu8, rate),
    )
    .unwrap();
    let l = hub
        .listen(
            &net,
            &listen_on(0),
            Arc::new(RtlTcp::rtl_sdr(5)),
            Arc::new(Recorder::default()),
        )
        .unwrap();
    let fast = RawClient::connect(&net, l.addr.port(), None);
    let slow = RawClient::connect(&net, l.addr.port(), Some(1024));
    assert!(wait_for(|| hub.snapshot().clients.len() == 2, 2.0));
    let ids: Vec<u64> = hub.snapshot().clients.iter().map(|c| c.id).collect();
    let (fast_id, slow_id) = (ids[0], ids[1]);

    let producer = paced(
        feeder,
        block(SampleType::Cu8, rate),
        2,
        12_000,
        3.0,
        counter_bytes,
    );
    let mut first_drop = None;
    let mut cut = None;
    let mut events = Vec::new();
    while !producer.is_finished() {
        let s = hub.snapshot();
        if first_drop.is_none()
            && s.clients
                .iter()
                .any(|c| c.id == slow_id && c.dropped_chunks > 0)
        {
            first_drop = Some(Instant::now());
        }
        for e in hub.take_events() {
            if matches!(e, Event::ClientLeft { client, .. } if client == slow_id) {
                cut = Some(Instant::now());
            }
            events.push(e);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let pushed = producer.join().unwrap();
    assert!(wait_for(|| fast.len() >= pushed + 12, 5.0));
    let fast_stats = hub
        .snapshot()
        .clients
        .into_iter()
        .find(|c| c.id == fast_id)
        .unwrap();
    assert_eq!((fast_stats.dropped_samples, fast_stats.drop_events), (0, 0));
    let got = fast.finish();
    assert_eq!(got.len(), pushed + 12);
    let bad = got[12..]
        .iter()
        .enumerate()
        .find(|(i, b)| **b != pattern(*i));
    assert_eq!(bad, None, "the fast client gets every byte as sent");

    events.extend(hub.take_events());
    let left: Vec<Leave> = events
        .iter()
        .filter_map(|e| match e {
            Event::ClientLeft { client, reason, .. } if *client == slow_id => Some(*reason),
            _ => None,
        })
        .collect();
    assert_eq!(left, [Leave::TooSlow]);
    let held = cut.unwrap() - first_drop.unwrap();
    println!("the slow client was cut {held:?} after its first drop (limit {drop_after} s)");
    assert!(
        held.as_secs_f64() >= drop_after - 0.02 && held.as_secs_f64() < drop_after + 0.3,
        "cut after {held:?}"
    );
    let _ = slow.finish();
}

/// The slice policy of a test: frequency and rate requests move the client's slice; a slice outside the capture is refused.
struct SliceControl;

impl Control for SliceControl {
    fn request(&self, client: &ClientHandle, request: &Request) -> Answer {
        let cap = client.capture();
        let now = client.slice().unwrap_or(cap);
        let want = match *request {
            Request::Frequency { hz } => Slice {
                centre_hz: hz,
                ..now
            },
            Request::Rate { hz } => Slice { rate_hz: hz, ..now },
            _ => return Answer::Refused,
        };
        match client.set_slice(Some(want)) {
            Ok(()) => Answer::Accepted,
            Err(_) => Answer::Refused,
        }
    }
}

#[test]
fn a_slice_brings_a_tone_to_the_clients_centre_and_rate() {
    let rate = 2.4e6;
    let (tone_hz, centre, out_rate) = (CENTRE + 310e3, CENTRE + 300e3, 240e3);
    let net: Arc<dyn Transport> = Arc::new(FakeTransport::new());
    let (hub, feeder) = Hub::new(
        Config {
            block_bytes: 4 * 16384,
            ..Config::default()
        },
        &block(SampleType::Cs16, rate),
    )
    .unwrap();
    let l = hub
        .listen(
            &net,
            &listen_on(0),
            Arc::new(RtlTcp::rtl_sdr(5)),
            Arc::new(SliceControl),
        )
        .unwrap();
    let mut c = RawClient::connect(&net, l.addr.port(), None);
    c.send(cmd::RATE, out_rate as u32);
    c.send(cmd::FREQ, centre as u32);
    let want = Slice {
        centre_hz: centre,
        rate_hz: out_rate,
    };
    assert!(wait_for(
        || hub.snapshot().clients.first().and_then(|c| c.slice) == Some(want),
        2.0
    ));
    // Outside the capture: 1.5 MHz off with a rate of 240 kHz.
    c.send(cmd::FREQ, (CENTRE + 1.5e6) as u32);
    assert!(wait_for(
        || hub.snapshot().clients[0].refused_requests == 1,
        2.0
    ));
    assert_eq!(hub.snapshot().clients[0].slice, Some(want));

    let step = (tone_hz - CENTRE) / rate;
    let tone = move |off: usize, len: usize| -> Vec<u8> {
        let first = off / 4;
        (first..first + len / 4)
            .flat_map(|n| {
                let ph = std::f64::consts::TAU * step * n as f64;
                let (i, q) = (
                    (0.5 * ph.cos() * 32767.0).round() as i16,
                    (0.5 * ph.sin() * 32767.0).round() as i16,
                );
                let mut b = [0u8; 4];
                b[..2].copy_from_slice(&i.to_le_bytes());
                b[2..].copy_from_slice(&q.to_le_bytes());
                b
            })
            .collect()
    };
    let producer = paced(feeder, block(SampleType::Cs16, rate), 4, 16384, 1.0, tone);
    let pushed = producer.join().unwrap() / 4;
    assert!(wait_for(
        || c.len() >= 12 && hub.snapshot().clients[0].samples as usize * 2 == c.len() - 12,
        2.0
    ));
    std::thread::sleep(Duration::from_millis(100));
    let got = c.finish();
    let n_out = (got.len() - 12) / 2;
    assert_eq!(n_out, pushed / 10, "the rate is the slice's: one tenth");
    let mut iq = vec![Complex32::default(); n_out];
    to_iq(SampleType::Cu8, &got[12..], &mut iq);
    // Skip the start of the filter; measure the frequency from the mean phase step and the share of power in the tone.
    let x: Vec<(f64, f64)> = iq[2400..]
        .iter()
        .map(|c| (f64::from(c.re), f64::from(c.im)))
        .collect();
    let mul_conj = |a: (f64, f64), b: (f64, f64)| (a.0 * b.0 + a.1 * b.1, a.1 * b.0 - a.0 * b.1);
    let rot = x
        .windows(2)
        .map(|w| mul_conj(w[1], w[0]))
        .fold((0.0, 0.0), |s, v| (s.0 + v.0, s.1 + v.1));
    let f = rot.1.atan2(rot.0) / std::f64::consts::TAU * out_rate;
    let w = std::f64::consts::TAU * f / out_rate;
    let sum = x.iter().enumerate().fold((0.0, 0.0), |s, (n, v)| {
        let d = mul_conj(*v, ((w * n as f64).cos(), (w * n as f64).sin()));
        (s.0 + d.0, s.1 + d.1)
    });
    let tone_amp = sum.0.hypot(sum.1) / x.len() as f64;
    let rms = (x.iter().map(|v| v.0 * v.0 + v.1 * v.1).sum::<f64>() / x.len() as f64).sqrt();
    println!("slice: tone at {f:.1} Hz, amplitude {tone_amp:.3}, rms {rms:.3}");
    assert!((f - 10e3).abs() < 5.0, "the tone is at +10 kHz: {f}");
    assert!(
        tone_amp > 0.45 && tone_amp / rms > 0.99,
        "amplitude {tone_amp}, rms {rms}"
    );
}

#[test]
fn the_allow_list_and_the_client_limit_refuse() {
    let net: Arc<dyn Transport> = Arc::new(FakeTransport::new());
    let (hub, _feeder) = Hub::new(
        Config {
            max_clients: 2,
            ..Config::default()
        },
        &block(SampleType::Cu8, 2.4e6),
    )
    .unwrap();
    let only_lan = Listen {
        allow: vec!["192.168.0.0/16".parse().unwrap()],
        ..listen_on(0)
    };
    let p = Arc::new(RtlTcp::rtl_sdr(5));
    let c = Arc::new(Recorder::default());
    let lan = hub.listen(&net, &only_lan, p.clone(), c.clone()).unwrap();
    let local = Listen {
        allow: vec!["127.0.0.0/8".parse().unwrap()],
        ..listen_on(0)
    };
    let open = hub.listen(&net, &local, p, c).unwrap();

    let refused = RawClient::connect(&net, lan.addr.port(), None);
    assert!(wait_for(|| refused.done.load(Ordering::Relaxed), 2.0));
    assert_eq!(refused.finish(), b"", "a refused client gets nothing");

    let a = RawClient::connect(&net, open.addr.port(), None);
    let b = RawClient::connect(&net, open.addr.port(), None);
    assert!(wait_for(|| a.len() == 12 && b.len() == 12, 2.0));
    let third = RawClient::connect(&net, open.addr.port(), None);
    assert!(wait_for(|| third.done.load(Ordering::Relaxed), 2.0));
    assert_eq!(third.finish(), b"");

    let s = hub.snapshot();
    let row = |id| s.listeners.iter().find(|l| l.id == id).unwrap().clone();
    assert_eq!((row(lan.id).refused_allow, row(lan.id).accepted), (1, 0));
    assert_eq!(
        (
            row(open.id).refused_full,
            row(open.id).accepted,
            row(open.id).clients
        ),
        (1, 2, 2)
    );
    let ev = hub.take_events();
    let refusals: Vec<Refusal> = ev
        .iter()
        .filter_map(|e| match e {
            Event::Refused { reason, .. } => Some(*reason),
            _ => None,
        })
        .collect();
    assert_eq!(refusals, [Refusal::NotAllowed, Refusal::Full]);

    // A client that leaves makes room.
    let _ = a.finish();
    let gone = hub.snapshot().clients[0].id;
    assert!(hub.kick(gone));
    assert!(wait_for(
        || hub.snapshot().clients.iter().all(|c| c.id != gone),
        2.0
    ));
    assert!(hub.take_events().contains(&Event::ClientLeft {
        client: gone,
        listener: open.id,
        peer: s.clients.iter().find(|c| c.id == gone).unwrap().peer,
        reason: Leave::Kicked,
    }));
    let d = RawClient::connect(&net, open.addr.port(), None);
    assert!(wait_for(|| d.len() == 12, 2.0));
    let _ = (b.finish(), d.finish());
}

#[test]
fn a_real_localhost_socket_carries_the_stream() {
    let rate = 1.024e6;
    let net: Arc<dyn Transport> = Arc::new(StdNet);
    let (hub, feeder) = Hub::new(Config::default(), &block(SampleType::Cu8, rate)).unwrap();
    let l = hub
        .listen(
            &net,
            &listen_on(0),
            Arc::new(RtlTcp::rtl_sdr(5)),
            Arc::new(Recorder::default()),
        )
        .unwrap();
    assert!(l.addr.ip().is_loopback() && l.addr.port() != 0);
    let c = RawClient::connect(&net, l.addr.port(), None);
    assert!(wait_for(
        || hub.snapshot().clients.len() == 1 && c.len() == 12,
        2.0
    ));
    let pushed = paced(
        feeder,
        block(SampleType::Cu8, rate),
        2,
        8192,
        0.5,
        counter_bytes,
    )
    .join()
    .unwrap();
    assert!(wait_for(|| c.len() >= pushed + 12, 5.0));
    let got = c.finish();
    assert_eq!(&got[..12], b"RTL0\0\0\0\x05\0\0\0\x1d");
    let bad = got[12..]
        .iter()
        .enumerate()
        .find(|(i, b)| **b != pattern(*i));
    assert_eq!((got.len() - 12, bad), (pushed, None));
}
