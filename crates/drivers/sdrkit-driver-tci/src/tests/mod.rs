//! Tests against the fake TCI server of `fake.rs`, on a fake network.

mod fake;

use crate::*;
use fake::{binary, iq_packet, text, Cfg, Fake, Mode, INIT1, INIT2};
use sdrkit_core::net::{FakeTransport, NetError};
use sdrkit_core::{Complex32, Owner, Value};
use sdrkit_driver_core::ws::{self, UpgradeError};
use sdrkit_driver_core::{ring, DriverKind, Protocol, RingReader, RingStats, Source, TxState};
use std::time::{Duration, Instant};

const MASK: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];
const KEY: [u8; 16] = *b"the sample nonce";

fn timing() -> Timing {
    Timing {
        fixed_mask: Some(MASK),
        fixed_key: Some(KEY),
        ..Default::default()
    }
}

fn drv(f: &Fake) -> TciDriver {
    TciDriver::with_timing(f.transport(), timing())
}

/// The values of a radio at `centre` and `rate` on receiver `rx`.
fn tune(rate: f64, centre: f64, rx: Option<i64>) -> Values {
    let kind = DriverKind::new(Box::new(TciDriver::new(Arc::new(FakeTransport::new()))));
    let schema = kind.schema();
    let mut v = Values::defaults(schema);
    let mut set = |k: &str, x: Value| v.set(schema.id(k).unwrap(), &x).unwrap();
    set("centre_hz", Value::Hz(centre));
    set("rate", Value::Hz(rate));
    if let Some(r) = rx {
        set("receiver", Value::I64(r));
    }
    v
}

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

fn wait_for(mut f: impl FnMut() -> bool, secs: f64) -> bool {
    let end = Instant::now() + Duration::from_secs_f64(secs);
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    f()
}

fn read_n(r: &mut RingReader, n: usize, secs: f64) -> Vec<Complex32> {
    let mut out = vec![Complex32::default(); n];
    let mut got = 0;
    let end = Instant::now() + Duration::from_secs_f64(secs);
    while got < n && Instant::now() < end {
        r.wait(1, Duration::from_millis(50));
        got += r.read_iq(&mut out[got..]);
    }
    out.truncate(got);
    out
}

fn start(src: &mut Box<dyn Source>, capacity: usize) -> (Started, RingReader, Arc<RingStats>) {
    let (w, r) = ring(capacity, src.sample_type());
    let stats = w.stats();
    (src.start(w).unwrap(), r, stats)
}

fn results() -> Results {
    Results::new(DESCRIPTOR.decl().results().clone())
}

/// The readings of `s` now.
fn readings(s: &mut Box<dyn Source>) -> Results {
    let mut r = results();
    s.poll(&mut r);
    r
}

/// The error of this crate inside `e`.
fn own(e: &DriverError) -> Option<TciError> {
    match e {
        DriverError::Driver(s) => s.downcast_ref::<TciError>().copied(),
        _ => None,
    }
}

/// Sets setting `key` of `t`.
fn set(t: &mut Values, key: &str, value: Value) {
    let schema = DESCRIPTOR.decl().schema().clone();
    t.set(schema.id(key).unwrap(), &value).unwrap();
}

/// Connects to the fake, runs the handshake and closes, as an "add network radio" dialog does.
fn probe(f: &Fake) -> Result<proto::ServerInfo, DriverError> {
    probe_at(&f.net, &f.addr(), Duration::from_secs(2))
}

/// As [`probe`], at `addr` on `net`, each read waiting up to `wait`.
fn probe_at(
    net: &FakeTransport,
    addr: &str,
    wait: Duration,
) -> Result<proto::ServerInfo, DriverError> {
    let (host, port) = parse_address(addr)?;
    let mut c = net.connect(&host, port, false, wait)?;
    let tm = timing();
    let r = radio::handshake(c.as_mut(), addr, &tm, wait, &|| false);
    match r {
        Ok((reader, link, info)) => {
            radio::finish(&link, c.as_mut(), tm.drain_step, tm.drain_total);
            drop(reader);
            Ok(info)
        }
        Err(e) => {
            c.shutdown();
            Err(e)
        }
    }
}

/// Every log line and request gathered so far.
struct Notes {
    lines: Vec<String>,
    restarts: usize,
    caps_changed: bool,
}

impl Notes {
    fn new() -> Notes {
        Notes {
            lines: Vec::new(),
            restarts: 0,
            caps_changed: false,
        }
    }
    fn pull(&mut self, s: &mut Box<dyn Source>) -> &mut Self {
        let mut r = results();
        let p = s.poll(&mut r);
        self.restarts += usize::from(p.restart);
        self.caps_changed |= p.capabilities;
        self.lines.extend(r.log_lines().iter().cloned());
        self
    }
    fn has(&self, part: &str) -> bool {
        self.lines.iter().any(|t| t.contains(part))
    }
}

/// Only the six receive commands are ever sent without transmit.
fn assert_only_receive_commands(f: &Fake) {
    for c in f.cmds() {
        for bad in [
            "trx",
            "tune",
            "drive",
            "audio",
            "cw_",
            "tx_",
            "spot",
            "modulation",
            "volume",
            "mute",
            "rx_",
        ] {
            assert!(!c.starts_with(bad), "sent {c}");
        }
        let name = c.split(':').next().unwrap();
        assert!(
            [
                "iq_samplerate",
                "dds",
                "vfo",
                "iq_start",
                "iq_stop",
                "start"
            ]
            .contains(&name),
            "sent {c}"
        );
    }
}

fn texts_sent(f: &Fake) -> Vec<String> {
    f.frames()
        .iter()
        .filter(|fr| fr.opcode == 1)
        .map(|fr| String::from_utf8_lossy(&fr.payload).into_owned())
        .collect()
}

// ---- Upgrade ----

#[test]
fn the_upgrade_request_is_byte_for_byte() {
    let f = Fake::start(Cfg::default());
    probe(&f).unwrap();
    let req = f.st.requests.lock().unwrap()[0].clone();
    assert_eq!(
        req,
        format!(
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
            fake::PORT
        )
    );
    // The probe closes with a close frame (status 1000) and never sends iq_stop: nothing was started.
    assert!(wait_for(
        || f.frames()
            .iter()
            .any(|fr| fr.raw == hex("88 82 37 fa 21 3d 34 12")),
        2.0
    ));
    assert!(f.cmds().is_empty());
}

#[test]
fn upgrade_answers_are_checked() {
    let ok = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
              Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
    let f = Fake::start(Cfg {
        mode: Mode::Answer(ok.into()),
        ..Default::default()
    });
    assert!(probe(&f).is_ok());
    drop(f);
    let f = Fake::start(Cfg {
        mode: Mode::Answer(ok.replace("xOo=", "xOx=")),
        ..Default::default()
    });
    let e = probe(&f).unwrap_err();
    assert_eq!(e, DriverError::Handshake(UpgradeError::Mismatch));
    assert!(!e.retryable());
    drop(f);
    let f = Fake::start(Cfg {
        mode: Mode::Answer("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".into()),
        ..Default::default()
    });
    let e = probe(&f).unwrap_err();
    assert_eq!(
        e,
        DriverError::Handshake(UpgradeError::Refused { status: Some(404) })
    );
    drop(f);
    let lower = "HTTP/1.1 101 Switching Protocols\r\nsec-websocket-accept:   s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
    let f = Fake::start(Cfg {
        mode: Mode::Answer(lower.into()),
        ..Default::default()
    });
    assert!(probe(&f).is_ok());
    drop(f);
    let f = Fake::start(Cfg {
        mode: Mode::Answer("HTTP/1.1 101 Switching Protocols\r\n\r\n".into()),
        ..Default::default()
    });
    let e = probe(&f).unwrap_err();
    assert_eq!(e, DriverError::Handshake(UpgradeError::NoAccept));
}

#[test]
fn an_answer_with_an_unasked_extension_is_refused() {
    // Nothing is requested, so an extension or a subprotocol in the answer is not accepted.
    for extra in [
        "Sec-WebSocket-Extensions: permessage-deflate",
        "Sec-WebSocket-Protocol: chat",
    ] {
        let answer = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n{extra}\r\n\r\n"
        );
        let f = Fake::start(Cfg {
            mode: Mode::Answer(answer),
            ..Default::default()
        });
        let e = probe(&f).unwrap_err();
        assert_eq!(e, DriverError::Handshake(UpgradeError::Unasked), "{extra}");
        assert!(!e.retryable());
        // Opening fails the same way, at once.
        assert!(drv(&f)
            .open(&f.addr(), &tune(48_000.0, 7.1e6, None))
            .is_err());
    }
}

#[test]
fn an_endless_header_is_not_http() {
    let f = Fake::start(Cfg {
        mode: Mode::Answer(format!("HTTP/1.1 101 OK\r\n{}", "x".repeat(9000))),
        ..Default::default()
    });
    let e = probe(&f).unwrap_err();
    assert_eq!(e, DriverError::Handshake(UpgradeError::NotHttp));
}

// ---- Initialisation ----

#[test]
fn the_initialisation_is_stored() {
    assert_eq!(INIT1.len(), 98);
    assert_eq!(INIT2.len(), 57);
    assert_eq!(text(INIT1)[..8], hex("81 62 70 72 6f 74 6f 63"));
    assert_eq!(text(INIT2)[..8], hex("81 39 69 66 5f 6c 69 6d"));
    assert_eq!(text("stop;"), hex("81 05 73 74 6f 70 3b"));
    let f = Fake::start(Cfg::default());
    let i = probe(&f).unwrap();
    assert_eq!(i.program.as_deref(), Some("ExpertSDR3"));
    assert_eq!(i.version.as_deref(), Some("1.9"));
    assert_eq!(i.device.as_deref(), Some("SunSDR2DX"));
    assert_eq!(i.receive_only, Some(false));
    assert_eq!(i.receivers(), 2);
    assert_eq!(i.vfo_limits, Some((10_000.0, 30e6)));
    assert_eq!(i.if_limits, Some((-24_000.0, 24_000.0)));
    assert_eq!(i.modulations, ["AM", "LSB", "USB"]);
    assert!(i.ready && i.seen);
}

// ---- Open ----

#[test]
fn open_sends_the_configure_then_iq_start_and_closes_cleanly() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(100_000.0, 7.1e6, Some(1))).unwrap();
    assert_eq!(src.sample_type(), SampleType::Cf32);
    let (st, mut r, _) = start(&mut src, 1 << 20);
    assert_eq!(st.rate_hz, 192_000.0);
    assert_eq!(st.centre_hz, 7.1e6);
    let s = read_n(&mut r, 4096, 4.0);
    assert_eq!(s.len(), 4096);
    let frames = f.frames();
    let t: Vec<_> = frames.iter().filter(|fr| fr.opcode == 1).take(4).collect();
    let p: Vec<String> = t
        .iter()
        .map(|fr| String::from_utf8_lossy(&fr.payload).into_owned())
        .collect();
    assert_eq!(
        p,
        [
            "iq_samplerate:192000;",
            "dds:1,7100000;",
            "vfo:1,0,7100000;",
            "iq_start:1;"
        ]
    );
    assert_eq!(
        t[0].raw,
        hex("81 95 37 fa 21 3d 5e 8b 7e 4e 56 97 51 51 52 88 40 49 52 c0 10 04 05 ca 11 0d 0c")
    );
    assert_eq!(
        t[3].raw,
        hex("81 8b 37 fa 21 3d 5e 8b 7e 4e 43 9b 53 49 0d cb 1a")
    );
    let res = readings(&mut src);
    assert_eq!(res.fields().get(&IQ_RATE), Some(192_000.0));
    assert_eq!(res.fields().get(&RECEIVER_IN_USE), Some(1));
    src.stop();
    assert!(wait_for(|| f.count("iq_stop:1") == 1, 15.0));
    let frames = f.frames();
    let stop_at = frames
        .iter()
        .position(|fr| fr.payload == b"iq_stop:1;")
        .unwrap();
    let close_at = frames.iter().position(|fr| fr.opcode == 8).unwrap();
    assert!(stop_at < close_at);
    assert_eq!(frames[close_at].raw, hex("88 82 37 fa 21 3d 34 12"));
    // No socket is left open.
    assert!(wait_for(
        || !f.st.connected.load(std::sync::atomic::Ordering::Acquire),
        2.0
    ));
    assert_only_receive_commands(&f);
}

#[test]
fn a_receiver_the_server_lacks_is_refused() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let e = d
        .open(&f.addr(), &tune(48_000.0, 7.1e6, Some(5)))
        .err()
        .unwrap();
    assert_eq!(e, DriverError::Refused { id: receiver_id() });
    assert!(f.cmds().is_empty());
    // The schema refuses a receiver beyond the most a server has.
    let schema = DESCRIPTOR.decl().schema().clone();
    assert!(Values::defaults(&schema)
        .set(receiver_id(), &Value::I64(8))
        .is_err());
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, Some(1))).unwrap();
    let (_, _r, _) = start(&mut src, 1 << 16);
    assert!(wait_for(|| f.count("iq_start:1") == 1, 3.0));
    src.stop();
    assert_only_receive_commands(&f);
}

// ---- IQ packets and conversion ----

const PKT_F32: &str = "82 50 00 00 00 00 80 bb 00 00 03 00 00 00 00 00
    00 00 00 00 00 00 04 00 00 00 00 00 00 00 02 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 3f 00 00 80 be 00 00 80 3f 00 00
    00 00";
const PKT_I16: &str = "82 48 00 00 00 00 80 bb 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 04 00 00 00 00 00 00 00 02 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 40 00 e0 ff 7f 00 80";
const PKT_I24: &str = "82 4c 00 00 00 00 00 77 01 00 01 00 00 00 00 00
    00 00 00 00 00 00 04 00 00 00 00 00 00 00 02 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 40 00 00 e0 ff ff 7f 00 00 80";
const PKT_I32_RX1: &str = "82 48 01 00 00 00 00 ee 02 00 02 00 00 00 00 00
    00 00 00 00 00 00 02 00 00 00 00 00 00 00 02 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 40 00 00 00 e0";

fn quiet() -> Cfg {
    Cfg {
        stream: false,
        ..Default::default()
    }
}

#[test]
fn float32_packets_go_to_the_ring_and_others_are_dropped() {
    let f = Fake::start(quiet());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 16);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    let mut notes = Notes::new();
    f.send(hex(PKT_F32));
    let s = read_n(&mut r, 2, 3.0);
    assert_eq!(s, [Complex32::new(0.5, -0.25), Complex32::new(1.0, 0.0)]);
    // Integer packets do not fit the float ring: dropped, with a warning.
    f.send(hex(PKT_I16));
    assert!(wait_for(|| notes.pull(&mut src).has("int16 IQ"), 2.0));
    assert_eq!(notes.restarts, 0);
    f.send(hex(PKT_I24));
    // The 96 000 of the header is followed.
    assert!(wait_for(|| notes.pull(&mut src).restarts == 1, 2.0));
    assert_eq!(readings(&mut src).fields().get(&IQ_RATE), Some(96_000.0));
    // The packet of receiver 1 is skipped on receiver 0, and not counted.
    f.send(hex(PKT_I32_RX1));
    // length = 2 (complex samples) is accepted, channels 0 is accepted, a NaN goes on as it is.
    let mut p = hex(PKT_F32);
    p[22] = 2;
    f.send(p.clone());
    p[22] = 4;
    p[30] = 0;
    f.send(p.clone());
    p[30] = 2;
    p[66..70].copy_from_slice(&hex("00 00 c0 7f"));
    f.send(p.clone());
    let s = read_n(&mut r, 6, 3.0);
    assert_eq!(s.len(), 6);
    assert_eq!(s[0], Complex32::new(0.5, -0.25));
    assert_eq!(s[2], Complex32::new(0.5, -0.25));
    assert!(s[4].re.is_nan() && s[4].im == -0.25);
    // length = 3 is unusable.
    let mut p = hex(PKT_F32);
    p[22] = 3;
    f.send(p);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(r.available(), 0);
    src.stop();
    assert!(
        notes
            .pull(&mut src)
            .has("TCI: 3 malformed or unusable packets ignored"),
        "{:?}",
        notes.lines
    );
    assert!(notes.has("TCI: IQ is arriving (48000 Hz, format 3)"));
    assert_only_receive_commands(&f);
}

#[test]
fn int32_packets_on_receiver_1_are_dropped() {
    let f = Fake::start(quiet());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(192_000.0, 7.1e6, Some(1))).unwrap();
    let (_, r, _) = start(&mut src, 1 << 16);
    assert!(wait_for(|| f.count("iq_start:1") == 1, 3.0));
    f.send(hex(PKT_I32_RX1));
    let mut notes = Notes::new();
    assert!(wait_for(|| notes.pull(&mut src).has("int32 IQ"), 2.0));
    assert_eq!(r.available(), 0);
    assert_eq!(readings(&mut src).fields().get(&FORMAT), Some("int32"));
    src.stop();
}

#[test]
fn a_float32_stream_flows_and_an_int16_stream_is_named() {
    for format in [3, 0] {
        let f = Fake::start(Cfg {
            format,
            interval: Duration::from_millis(20),
            ..Default::default()
        });
        let d = drv(&f);
        let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
        let (_, mut r, _) = start(&mut src, 1 << 18);
        if format == 3 {
            assert_eq!(read_n(&mut r, 2048, 4.0).len(), 2048);
        } else {
            let mut notes = Notes::new();
            assert!(wait_for(|| notes.pull(&mut src).has("int16 IQ"), 4.0));
        }
        src.stop();
    }
}

#[test]
fn retune_sends_only_what_changed() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut t = tune(48_000.0, 14.2e6, None);
    let mut src = d.open(&f.addr(), &t).unwrap();
    let (_, _r, _) = start(&mut src, 1 << 18);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    assert_eq!(
        f.cmds()[..4],
        [
            "iq_samplerate:48000",
            "dds:0,14200000",
            "vfo:0,0,14200000",
            "iq_start:0"
        ]
    );
    set(&mut t, "centre_hz", Value::Hz(14.3e6));
    src.apply(&t).unwrap();
    assert!(wait_for(|| f.count("vfo:0,0,14300000") == 1, 2.0));
    assert_eq!(f.count("dds:0,14300000"), 1);
    assert_eq!(f.count("iq_samplerate:48000"), 1);
    set(&mut t, "rate", Value::Hz(384_000.0));
    match src.apply(&t).unwrap() {
        Retuned::Live(s) => assert_eq!(s.rate_hz, 384_000.0),
        r => panic!("{r:?}"),
    }
    assert!(wait_for(|| f.count("iq_samplerate:384000") == 1, 2.0));
    assert_eq!(f.count("dds:0,14300000"), 1);
    let before = f.frames().len();
    src.apply(&t).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        f.frames().len(),
        before,
        "a retune with nothing changed sends nothing"
    );
    set(&mut t, "centre_hz", Value::Hz(50e6));
    let mut notes = Notes::new();
    notes.pull(&mut src);
    src.apply(&t).unwrap();
    assert!(notes.pull(&mut src).has(
        "this TCI radio is specified for 0.01 to 30 MHz; the driver may not reach 50 MHz or the signal may be weak or absent"
    ));
    assert!(wait_for(|| f.count("dds:0,50000000") == 1, 2.0));
    // A new receiver needs a restart; nothing is sent for it.
    let mut t1 = t.clone();
    set(&mut t1, "receiver", Value::I64(1));
    assert_eq!(src.apply(&t1).unwrap(), Retuned::NeedsRestart);
    assert_eq!(f.count("iq_start:1"), 0);
    // The echoes are not taken as a moved panorama.
    let mut notes = Notes::new();
    assert!(!notes.pull(&mut src).has("moved the panorama"));
    src.stop();
    assert_only_receive_commands(&f);
}

#[test]
fn a_rate_beyond_the_offer_is_warned() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(1e6, 7.1e6, None)).unwrap();
    let (s, _r, _) = start(&mut src, 1 << 18);
    assert_eq!(s.rate_hz, 384_000.0);
    let mut notes = Notes::new();
    assert!(notes
        .pull(&mut src)
        .has("this radio only offers 0.38 Msps, 1.00 Msps were requested for this channel width"));
    src.stop();
}

#[test]
fn the_server_moving_the_panorama_is_logged_once() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, _r, _) = start(&mut src, 1 << 18);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    f.send(text("dds:0,7200000;"));
    f.send(text("dds:0,7200000;dds:1,9000000;"));
    let mut notes = Notes::new();
    let line = "TCI: the server moved the panorama to 7.2 MHz; sdrkit keeps tuning 7.1 MHz";
    assert!(wait_for(|| notes.pull(&mut src).has(line), 3.0));
    std::thread::sleep(Duration::from_millis(200));
    notes.pull(&mut src);
    assert_eq!(
        notes
            .lines
            .iter()
            .filter(|t| t.contains("moved the panorama"))
            .count(),
        1
    );
    assert_eq!(readings(&mut src).fields().get(&PANORAMA), Some(7.2e6));
    // The next retune sends the engine's frequency again.
    src.apply(&tune(48_000.0, 7.15e6, None)).unwrap();
    assert!(wait_for(|| f.count("dds:0,7150000") == 1, 2.0));
    assert!(wait_for(
        || readings(&mut src).fields().get(&PANORAMA).is_none(),
        2.0
    ));
    src.stop();
}

// ---- Rate followed from the server ----

#[test]
fn the_servers_rate_wins() {
    let f = Fake::start(Cfg {
        rate_answer: Some(96_000),
        ..Default::default()
    });
    let d = drv(&f);
    let t = tune(100_000.0, 7.1e6, None);
    let mut src = d.open(&f.addr(), &t).unwrap();
    let (_, _r, _) = start(&mut src, 1 << 18);
    assert!(wait_for(|| f.count("iq_samplerate:192000") == 1, 3.0));
    let mut notes = Notes::new();
    assert!(wait_for(|| notes.pull(&mut src).restarts >= 1, 3.0));
    assert_eq!(readings(&mut src).fields().get(&IQ_RATE), Some(96_000.0));
    match src.apply(&t).unwrap() {
        Retuned::Live(s) => assert_eq!(s.rate_hz, 96_000.0),
        r => panic!("{r:?}"),
    }
    std::thread::sleep(Duration::from_millis(300));
    notes.pull(&mut src);
    assert_eq!(notes.restarts, 1, "{:?}", notes.lines);
    src.stop();
    // The engine's restart: the server's answer is remembered, so it is not a change again.
    let mut src = d.open(&f.addr(), &t).unwrap();
    let (s, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(s.rate_hz, 96_000.0);
    assert_eq!(read_n(&mut r, 2048, 3.0).len(), 2048);
    let mut notes = Notes::new();
    assert_eq!(notes.pull(&mut src).restarts, 0);
    src.stop();
}

#[test]
fn slow_or_absent_header_rates_are_not_followed() {
    let f = Fake::start(quiet());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 16);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    f.send(binary(&iq_packet(0, 7999, 3, 4, 0.5, -0.25)));
    f.send(binary(&iq_packet(0, 0, 3, 4, 0.5, -0.25)));
    assert_eq!(read_n(&mut r, 8, 3.0).len(), 8);
    let mut notes = Notes::new();
    assert_eq!(notes.pull(&mut src).restarts, 0);
    match src.apply(&tune(48_000.0, 7.1e6, None)).unwrap() {
        Retuned::Live(s) => assert_eq!(s.rate_hz, 48_000.0),
        r => panic!("{r:?}"),
    }
    src.stop();
}

// ---- Malformed burst ----

#[test]
fn a_malformed_burst_is_survived_and_counted() {
    let f = Fake::start(Cfg {
        interval: Duration::from_millis(30),
        ..Default::default()
    });
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 2048, 3.0).len(), 2048);
    f.send(text("garbage without a semicolon"));
    f.send(binary(&[1, 2]));
    let mut extra = iq_packet(0, 48_000, 3, 10, 0.5, -0.25);
    extra.push(0);
    f.send(binary(&extra));
    let mut p = iq_packet(0, 48_000, 3, 10, 0.5, -0.25);
    p[8] = 9;
    f.send(binary(&p));
    let mut p = iq_packet(0, 48_000, 3, 10, 0.5, -0.25);
    p[24] = 1;
    f.send(binary(&p));
    f.send(binary(&iq_packet(1, 48_000, 3, 10, 0.5, -0.25)));
    let mut p = iq_packet(0, 48_000, 3, 10, 0.5, -0.25);
    p[28] = 1;
    f.send(binary(&p));
    let s = read_n(&mut r, 8192, 4.0);
    assert_eq!(s.len(), 8192);
    assert!(s.iter().all(|c| c.re.is_finite() && c.im.is_finite()));
    assert_eq!(f.conns(), 1);
    src.stop();
    let mut notes = Notes::new();
    assert!(
        notes
            .pull(&mut src)
            .has("TCI: 4 malformed or unusable packets ignored"),
        "{:?}",
        notes.lines
    );
    assert_only_receive_commands(&f);
}

// ---- Reconnect ----

#[test]
fn a_dropped_connection_reconnects_with_the_same_settings() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(384_000.0, 14.3e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 2048, 3.0).len(), 2048);
    f.drop_client();
    assert!(wait_for(|| f.conns() >= 2, 8.0));
    for c in [
        "iq_samplerate:384000",
        "dds:0,14300000",
        "vfo:0,0,14300000",
        "iq_start:0",
    ] {
        assert!(wait_for(|| f.count(c) >= 2, 4.0), "{c}");
    }
    assert_eq!(f.st.requests.lock().unwrap().len(), 2);
    r.skip_all();
    assert_eq!(read_n(&mut r, 2048, 3.0).len(), 2048);
    let mut notes = Notes::new();
    assert!(wait_for(
        || notes.pull(&mut src).has("reconnected after"),
        3.0
    ));
    let a = f.addr();
    assert!(
        notes.has(&format!("TCI {a}: the connection is closed")),
        "{:?}",
        notes.lines
    );
    assert!(notes.lines.iter().any(
        |t| t.starts_with(&format!("TCI {a}: reconnected after")) && t.contains("samples lost")
    ));
    assert!(src.loss().unwrap().link_down > 0);
    src.stop();
    assert_only_receive_commands(&f);
}

#[test]
fn a_retune_while_away_is_applied_on_reconnect() {
    let f = Fake::start(Cfg::default());
    let mut tm = timing();
    tm.connect = Duration::from_millis(300);
    let d = TciDriver::with_timing(f.transport(), tm);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, _r, _) = start(&mut src, 1 << 18);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    // While the server refuses the upgrade, the radio is away.
    f.cfg(|c| c.mode = Mode::Answer("HTTP/1.1 503 Busy\r\n\r\n".into()));
    f.drop_client();
    assert!(wait_for(|| f.conns() >= 2, 4.0));
    let mut notes = Notes::new();
    notes.pull(&mut src);
    assert!(matches!(
        src.apply(&tune(48_000.0, 7.2e6, None)).unwrap(),
        Retuned::Live(_)
    ));
    assert!(notes.pull(&mut src).has("away"), "{:?}", notes.lines);
    f.cfg(|c| c.mode = Mode::Normal);
    assert!(wait_for(|| f.count("dds:0,7200000") == 1, 6.0));
    assert!(wait_for(|| f.count("iq_start:0") == 2, 3.0));
    src.stop();
}

#[test]
fn silence_ends_the_loop_and_says_why() {
    let f = Fake::start(quiet());
    let mut tm = timing();
    tm.stall = Duration::from_millis(500);
    let d = TciDriver::with_timing(f.transport(), tm);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, _r, _) = start(&mut src, 1 << 16);
    let mut notes = Notes::new();
    let line = format!("TCI: connected to {} but no IQ arrived in 0.5 s", f.addr());
    assert!(
        wait_for(|| notes.pull(&mut src).has(&line), 4.0),
        "{:?}",
        notes.lines
    );
    assert!(wait_for(|| f.conns() >= 2, 4.0));
    src.stop();
}

#[test]
fn a_ping_then_silence_still_stalls_on_time() {
    // A control frame between messages does not stretch the wait for the next one .
    let f = Fake::start(quiet());
    let mut tm = timing();
    tm.stall = Duration::from_millis(500);
    let d = TciDriver::with_timing(f.transport(), tm);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, _r, _) = start(&mut src, 1 << 16);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 2.0));
    f.send(hex("89 02 61 62"));
    assert!(wait_for(
        || f.frames()
            .iter()
            .any(|fr| fr.raw == hex("8a 82 37 fa 21 3d 56 98")),
        2.0
    ));
    let t0 = Instant::now();
    let mut notes = Notes::new();
    let line = format!("TCI: connected to {} but no IQ arrived in 0.5 s", f.addr());
    assert!(
        wait_for(|| notes.pull(&mut src).has(&line), 3.0),
        "{:?}",
        notes.lines
    );
    // Without the reset the wait would be 11 x 0.5 s and end as a protocol fault.
    assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
    assert!(!notes.has("protocol: "), "{:?}", notes.lines);
    src.stop();
}

// ---- A stopped radio ----

#[test]
fn a_stopped_radio_gets_one_start() {
    let f = Fake::start(Cfg {
        radio_stopped: true,
        ..Default::default()
    });
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 2048, 4.0).len(), 2048);
    assert_eq!(f.count("start"), 1);
    assert!(f
        .frames()
        .iter()
        .any(|fr| fr.raw == hex("81 86 37 fa 21 3d 44 8e 40 4f 43 c1")));
    f.send(text("stop;"));
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(f.count("start"), 1);
    let mut notes = Notes::new();
    assert!(notes
        .pull(&mut src)
        .has("TCI: the server's radio is stopped, sending start"));
    assert_eq!(
        readings(&mut src).fields().get(&SERVER_RUNNING),
        Some(false)
    );
    src.stop();
    assert_only_receive_commands(&f);
}

// ---- Not a TCI server ----

#[test]
fn not_tci_servers_are_told_apart() {
    let f = Fake::start(Cfg {
        mode: Mode::Plain,
        ..Default::default()
    });
    assert_eq!(
        probe(&f).unwrap_err(),
        DriverError::Handshake(UpgradeError::NotHttp)
    );
    drop(f);
    let f = Fake::start(Cfg {
        mode: Mode::Silent,
        ..Default::default()
    });
    let t0 = Instant::now();
    let e = probe_at(&f.net, &f.addr(), Duration::from_millis(600)).unwrap_err();
    assert_eq!(e, DriverError::Net(NetError::TimedOut));
    assert!(e.retryable());
    assert!(t0.elapsed() < Duration::from_secs(3));
    drop(f);
    let e = probe_at(
        &FakeTransport::new(),
        "127.0.0.1:1",
        Duration::from_millis(500),
    )
    .unwrap_err();
    assert_eq!(e, DriverError::Net(NetError::Refused));
    assert!(e.retryable());
    let f = Fake::start(Cfg {
        init: vec![text("hello;")],
        ..Default::default()
    });
    let e = probe(&f).unwrap_err();
    assert_eq!(e, DriverError::Protocol(Protocol::NotThisRadio));
    assert!(!e.retryable());
    drop(f);
    let f = Fake::start(Cfg {
        init: vec![text("protocol:Other,2.0;")],
        ..Default::default()
    });
    let t0 = Instant::now();
    let i = probe_at(&f.net, &f.addr(), Duration::from_secs(3)).unwrap();
    assert!(t0.elapsed() >= Duration::from_millis(2900));
    assert_eq!(i.program.as_deref(), Some("Other"));
    assert!(!i.ready);
}

#[test]
fn bad_addresses_and_dead_ports_fail_as_they_should() {
    let net = FakeTransport::new();
    let d = TciDriver::with_timing(Arc::new(net.clone()), timing());
    let t = tune(48_000.0, 7.1e6, None);
    let e = d.open("h:x", &t).err().unwrap();
    assert_eq!(e, DriverError::Address);
    assert!(!e.retryable());
    let e = d.open("[::1", &t).err().unwrap();
    assert!(!e.retryable());
    assert_eq!(net.connects(), 0);
    let mut tm = timing();
    tm.connect = Duration::from_millis(300);
    let d = TciDriver::with_timing(Arc::new(net), tm);
    let e = d.open("127.0.0.1:1", &t).err().unwrap();
    assert!(e.retryable());
    assert_eq!(e, DriverError::Net(NetError::Refused));
    // A server that is not TCI is a final failure: one attempt only.
    let f = Fake::start(Cfg {
        init: vec![text("hello;")],
        ..Default::default()
    });
    let e = drv(&f).open(&f.addr(), &t).err().unwrap();
    assert_eq!(e, DriverError::Protocol(Protocol::NotThisRadio));
    assert_eq!(f.conns(), 1);
}

// ---- WebSocket framing ----

#[test]
fn pings_are_answered_and_the_stream_goes_on() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 1024, 3.0).len(), 1024);
    f.send(hex("89 02 61 62"));
    assert!(wait_for(
        || f.frames()
            .iter()
            .any(|fr| fr.raw == hex("8a 82 37 fa 21 3d 56 98")),
        3.0
    ));
    assert_eq!(read_n(&mut r, 2048, 3.0).len(), 2048);
    assert_eq!(f.conns(), 1);
    src.stop();
}

#[test]
fn a_server_close_is_answered_and_reconnected() {
    let f = Fake::start(Cfg {
        ignore_close: true,
        ..Default::default()
    });
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 1024, 3.0).len(), 1024);
    f.send(hex("88 02 03 e8"));
    assert!(wait_for(
        || f.frames()
            .iter()
            .any(|fr| fr.opcode == 8 && fr.payload == [3, 0xe8]),
        3.0
    ));
    f.cfg(|c| c.ignore_close = false);
    f.drop_client();
    assert!(wait_for(|| f.conns() >= 2, 6.0));
    src.stop();
}

fn bad_frame_drops(raw: Vec<u8>) {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 1024, 3.0).len(), 1024);
    f.send(raw.clone());
    let mut notes = Notes::new();
    let line = format!("TCI {}: protocol: ", f.addr());
    assert!(
        wait_for(|| notes.pull(&mut src).has(&line), 3.0),
        "{raw:02x?}: {:?}",
        notes.lines
    );
    assert!(wait_for(|| f.conns() >= 2, 6.0));
    src.stop();
}

#[test]
fn bad_frames_drop_the_connection() {
    bad_frame_drops(hex("81 85 01 02 03 04 72 67 62 60 3a"));
    bad_frame_drops(hex("c1 01 3b"));
    bad_frame_drops(hex("83 00"));
    bad_frame_drops(hex("8b 00"));
    bad_frame_drops(hex("80 02 79 3b"));
    bad_frame_drops(hex("09 00"));
    let mut big = hex("82 7f");
    big.extend_from_slice(&(2u64 << 20).to_be_bytes());
    bad_frame_drops(big);
}

#[test]
fn fragments_are_joined_with_a_ping_between() {
    let f = Fake::start(Cfg {
        init: vec![text(INIT1), hex("01 04 72 65 61 64"), hex("80 02 79 3b")],
        ..Default::default()
    });
    let i = probe(&f).unwrap();
    assert!(i.ready);
    let f = Fake::start(Cfg {
        init: vec![
            text(INIT1),
            hex("01 04 72 65 61 64"),
            hex("89 02 61 62"),
            hex("80 02 79 3b"),
        ],
        ..Default::default()
    });
    let t0 = Instant::now();
    let i = probe(&f).unwrap();
    assert!(i.ready);
    assert!(
        t0.elapsed() < Duration::from_secs(1),
        "ready ended the wait"
    );
    assert!(wait_for(
        || f.frames()
            .iter()
            .any(|fr| fr.raw == hex("8a 82 37 fa 21 3d 56 98")),
        2.0
    ));
}

#[test]
fn a_frame_split_over_slow_segments_is_read_whole() {
    let f = Fake::start(quiet());
    let mut tm = timing();
    tm.stall = Duration::from_secs(3);
    let d = TciDriver::with_timing(f.transport(), tm);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 16);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    let fr = binary(&iq_packet(0, 48_000, 3, 100, 0.5, -0.25));
    let (a, b) = fr.split_at(40);
    f.send(a.to_vec());
    std::thread::sleep(Duration::from_secs(1));
    f.send(b.to_vec());
    let s = read_n(&mut r, 100, 3.0);
    assert_eq!(s.len(), 100);
    assert!(s.iter().all(|c| *c == Complex32::new(0.5, -0.25)));
    assert_eq!(f.conns(), 1);
    src.stop();
}

#[test]
fn large_frames_use_the_long_lengths() {
    let f = Fake::start(quiet());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    // 16-bit and 64-bit lengths; a 1 MiB message is still accepted.
    f.send(binary(&iq_packet(0, 48_000, 3, 1000, 0.5, -0.25)));
    f.send(binary(&iq_packet(0, 48_000, 3, 10_000, 0.5, -0.25)));
    let n = (ws::MAX_MESSAGE - proto::HEADER) / 8;
    f.send(binary(&iq_packet(0, 48_000, 3, n, 0.5, -0.25)));
    assert_eq!(read_n(&mut r, 11_000 + n, 5.0).len(), 11_000 + n);
    assert_eq!(f.conns(), 1);
    src.stop();
}

// ---- Capabilities from a remembered server ----

#[test]
fn capabilities_come_from_the_server() {
    let schema = DESCRIPTOR.decl().schema().clone();
    let rx = schema.setting(receiver_id()).unwrap();
    assert!(rx.restart);
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let c = src.capabilities().clone();
    assert_eq!((c.min_freq_hz, c.max_freq_hz), (10e3, 30e6));
    assert_eq!(
        c.rates.ranges,
        [(48e3, 48e3), (96e3, 96e3), (192e3, 192e3), (384e3, 384e3)]
    );
    assert_eq!((c.rates.min_hz, c.rates.max_hz), (48e3, 384e3));
    let (lo, hi, _) = c.gain_db.unwrap();
    assert_eq!((lo, hi), (0.0, 0.0));
    let t = c.transmit.as_ref().unwrap();
    assert_eq!((t.min_freq_hz, t.max_freq_hz), (10e3, 30e6));
    assert_eq!((t.rates.min_hz, t.rates.max_hz), (48e3, 48e3));
    assert_eq!(t.bandwidth_hz, None);
    assert_eq!(readings(&mut src).fields().get(&RECEIVERS), Some(2));
    // The rate dropdown is exactly the four rates.
    let list = sdrkit_driver_core::rate::rate_list(&c.rates, 0.0);
    assert_eq!(list.len(), 4);
    src.stop();
    // A receive-only server has no transmitter.
    let f = Fake::start(Cfg {
        init: vec![text(
            "protocol:ExpertSDR3,1.9;trx_count:1;receive_only:true;ready;",
        )],
        ..Default::default()
    });
    let mut src = drv(&f)
        .open(&f.addr(), &tune(48_000.0, 7.1e6, None))
        .unwrap();
    assert!(src.capabilities().transmit.is_none());
    assert!(src.tx_schema().is_none());
    assert_eq!(readings(&mut src).fields().get(&RECEIVE_ONLY), Some(true));
    src.stop();
}

#[test]
fn a_connect_reports_the_server() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let mut notes = Notes::new();
    notes.pull(&mut src);
    assert_eq!(src.capabilities().max_freq_hz, 30e6);
    assert!(notes.has(&format!(
        "TCI: connected to SunSDR2DX at {} (ExpertSDR3 1.9)",
        f.addr()
    )));
    src.stop();
}

#[test]
fn the_descriptor() {
    let d = TciDriver::new(Arc::new(FakeTransport::new()));
    let k = d.descriptor();
    assert_eq!((k.key, k.name), ("tci", "TCI (network)"));
    assert_eq!(k.produces[0].sample, SampleType::Cf32);
    assert_eq!(
        parse_address("10.0.0.5").unwrap(),
        ("10.0.0.5".into(), 50001)
    );
    assert_eq!(parse_address("[::1]:7").unwrap(), ("::1".into(), 7));
}

#[test]
fn readings_and_throughput_are_shown() {
    let f = Fake::start(Cfg::default());
    let mut tm = timing();
    tm.window = Duration::from_millis(300);
    let d = TciDriver::with_timing(f.transport(), tm);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 1024, 3.0).len(), 1024);
    f.send(text("rx_smeter:0,0,-73;rx_channel_sensors:0,1,-10;"));
    assert!(wait_for(
        || readings(&mut src).fields().get(&SMETER) == Some(-73.0),
        3.0
    ));
    // The fake sends 10 240 sps for a rate of 48 000: flagged after 3 windows.
    let mut res = results();
    assert!(wait_for(
        || {
            src.poll(&mut res);
            res.fields().get(&LOW_THROUGHPUT) == Some(true)
        },
        5.0
    ));
    let fl = res.fields();
    let (arriving, mbit) = (fl.get(&ARRIVING).unwrap(), fl.get(&MBIT_S).unwrap());
    assert!(arriving > 5_000.0 && arriving < 20_000.0, "{arriving}");
    assert!(mbit > 0.3 && mbit < 1.5, "{mbit}");
    assert_eq!(fl.get(&SERVER), Some("ExpertSDR3 1.9"));
    assert_eq!(fl.get(&DEVICE), Some("SunSDR2DX"));
    assert_eq!(fl.get(&RECEIVERS), Some(2));
    assert_eq!(
        (fl.get(&VFO_MIN), fl.get(&VFO_MAX)),
        (Some(10e3), Some(30e6))
    );
    assert_eq!(fl.get(&RECEIVER_IN_USE), Some(0));
    assert_eq!(fl.get(&IQ_RATE), Some(48_000.0));
    assert_eq!(fl.get(&FORMAT), Some("float32"));
    src.stop();
}

// ---- Transmit ----

const CHRONO: &str = "82 40 00 00 00 00 80 bb 00 00 03 00 00 00 00 00
    00 00 00 00 00 00 04 00 00 00 03 00 00 00 02 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00";
const AUDIO_ANSWER: &str = "00 00 00 00 80 bb 00 00 03 00 00 00 00 00 00 00
    00 00 00 00 04 00 00 00 02 00 00 00 02 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    00 00 80 3e 00 00 80 3e 00 00 80 be 00 00 80 be";

fn tx_tune() -> Values {
    tune(48_000.0, 7.1e6, None)
}

/// The transmit values: the defaults of the transmit schema.
fn tx_values() -> Values {
    Values::defaults(&sdrkit_driver_core::tx_schema(
        &[],
        &sdrkit_driver_core::TxLimits::default(),
    ))
}

fn binaries(f: &Fake) -> Vec<fake::Frame> {
    f.frames().into_iter().filter(|fr| fr.opcode == 2).collect()
}

#[test]
fn transmit_answers_chrono_packets_with_audio() {
    let f = Fake::start(quiet());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tx_tune()).unwrap();
    assert!(src.capabilities().transmit.is_some());
    assert!(src.tx_schema().is_some());
    let (_, _r, _) = start(&mut src, 1 << 16);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    assert_eq!(f.count("trx:0,true,tci"), 0, "never keyed at open");
    let mut tx = src.transmitter(&tx_values()).unwrap();
    assert_eq!(tx.state(), TxState::Disarmed);
    tx.arm().unwrap();
    assert!(wait_for(|| f.count("trx:0,true,tci") == 1, 2.0));
    let t = texts_sent(&f);
    assert_eq!(
        t[4..8],
        [
            "audio_samplerate:48000;",
            "audio_stream_sample_type:float32;",
            "audio_stream_channels:2;",
            "trx:0,true,tci;"
        ]
    );
    assert_eq!(
        tx.push(
            &[Complex32::new(0.25, 0.0), Complex32::new(-0.25, 0.0)],
            Duration::ZERO
        )
        .unwrap(),
        2
    );
    f.send(hex(CHRONO));
    assert!(wait_for(|| !binaries(&f).is_empty(), 3.0));
    let b = &binaries(&f)[0];
    assert_eq!(b.payload, hex(AUDIO_ANSWER));
    assert_eq!(
        b.raw[..14],
        hex("82 d0 37 fa 21 3d 37 fa 21 3d b7 41 21 3d")
    );
    f.send(hex(CHRONO));
    assert!(wait_for(|| binaries(&f).len() == 2, 3.0));
    let b = &binaries(&f)[1];
    assert!(b.payload[64..].iter().all(|x| *x == 0));
    assert_eq!(b.payload.len(), 80);
    let s = tx.stats();
    assert_eq!((s.buffers, s.sent, s.underrun_samples), (2, 4, 2));
    // A chrono packet for another receiver is ignored.
    let mut other = hex(CHRONO);
    other[2] = 1;
    f.send(other);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(binaries(&f).len(), 2);
    tx.disarm();
    assert!(wait_for(|| f.count("trx:0,false") == 1, 2.0));
    f.send(hex(CHRONO));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(binaries(&f).len(), 2, "no answers once ended");
    // Dropping the connection while transmitting: the reconnect does not key again.
    tx.arm().unwrap();
    assert!(wait_for(|| f.count("trx:0,true,tci") == 2, 2.0));
    f.drop_client();
    assert!(wait_for(|| f.count("iq_start:0") == 2, 8.0));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(f.count("trx:0,true,tci"), 2);
    assert_eq!(tx.state(), TxState::Closed);
    assert!(tx.arm().is_err());
    drop(tx);
    src.stop();
}

#[test]
fn transmit_sends_nothing_until_armed_and_is_refused_where_the_server_has_none() {
    let f = Fake::start(quiet());
    let mut src = drv(&f).open(&f.addr(), &tx_tune()).unwrap();
    assert!(src.capabilities().transmit.is_some());
    // The server sets the drive: the power is fixed at 0 dB, so no attenuation is applied and nothing reads as muted.
    let schema = src.tx_schema().unwrap();
    let power = schema
        .setting(sdrkit_driver_core::TxSetting::Power.id())
        .unwrap();
    assert!(power.fixed);
    assert_eq!(power.default, Value::F64(0.0));
    let (_, _r, _) = start(&mut src, 1 << 16);
    let tx = src.transmitter(&tx_values()).unwrap();
    assert_eq!(tx.state(), TxState::Disarmed);
    std::thread::sleep(Duration::from_millis(200));
    drop(tx);
    src.stop();
    assert_only_receive_commands(&f);

    let f = Fake::start(Cfg {
        stream: false,
        init: vec![text("protocol:ExpertSDR3,1.9;receive_only:true;ready;")],
        ..Default::default()
    });
    let mut src = drv(&f).open(&f.addr(), &tx_tune()).unwrap();
    assert!(src.capabilities().transmit.is_none());
    let e = src.transmitter(&tx_values()).err().unwrap();
    assert_eq!(own(&e), Some(TciError::ReceiveOnly));
    src.stop();
    assert_only_receive_commands(&f);

    let f = Fake::start(quiet());
    let mut src = drv(&f).open(&f.addr(), &tx_tune()).unwrap();
    let (_, _r, _) = start(&mut src, 1 << 16);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    f.send(text("tx_enable:0,false;"));
    std::thread::sleep(Duration::from_millis(300));
    let mut tx = src.transmitter(&tx_values()).unwrap();
    let e = tx.arm().unwrap_err();
    assert_eq!(
        own(&e),
        Some(TciError::ReceiverMayNotTransmit { receiver: 0 })
    );
    assert_eq!(f.count("trx:0,true,tci"), 0);
    assert!(tx.play_cyclic(&[Complex32::default()]).is_err());
    assert!(
        tx.push(&[Complex32::default()], Duration::ZERO).is_err(),
        "not on air"
    );
    drop(tx);
    src.stop();
    assert_only_receive_commands(&f);
}

#[test]
fn stop_while_transmitting_unkeys_first() {
    let f = Fake::start(quiet());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tx_tune()).unwrap();
    let (_, _r, _) = start(&mut src, 1 << 16);
    assert!(wait_for(|| f.count("iq_start:0") == 1, 3.0));
    let mut tx = src.transmitter(&tx_values()).unwrap();
    tx.arm().unwrap();
    src.stop();
    assert!(wait_for(|| f.count("iq_stop:0") == 1, 3.0));
    let c = f.cmds();
    let off = c.iter().position(|x| x == "trx:0,false").unwrap();
    let stop = c.iter().position(|x| x == "iq_stop:0").unwrap();
    assert!(off < stop);
    assert_eq!(tx.state(), TxState::Closed);
}

// ---- Close ----

#[test]
fn stop_is_quick_even_when_the_server_never_closes() {
    let f = Fake::start(Cfg {
        ignore_close: true,
        interval: Duration::from_millis(20),
        ..Default::default()
    });
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 2048, 3.0).len(), 2048);
    let t0 = Instant::now();
    src.stop();
    assert!(
        t0.elapsed() < Duration::from_millis(600),
        "{:?}",
        t0.elapsed()
    );
    assert!(wait_for(|| f.count("iq_stop:0") == 1, 3.0));
    let frames = f.frames();
    let stop_at = frames
        .iter()
        .position(|fr| fr.payload == b"iq_stop:0;")
        .unwrap();
    let close_at = frames.iter().position(|fr| fr.opcode == 8).unwrap();
    assert!(stop_at < close_at);
    assert_eq!(frames[close_at].payload, [3, 0xe8]);
    assert!(wait_for(
        || !f.st.connected.load(std::sync::atomic::Ordering::Acquire),
        3.0
    ));
    // A second stop does nothing.
    src.stop();
    assert_eq!(f.count("iq_stop:0"), 1);
    assert!(src.apply(&tune(48_000.0, 7.2e6, None)).is_err());
}

#[test]
fn a_source_never_started_closes_without_iq_stop() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    src.stop();
    src.stop();
    assert!(wait_for(|| f.frames().iter().any(|fr| fr.opcode == 8), 2.0));
    assert_eq!(f.count("iq_stop:0"), 0);
    assert_eq!(f.count("iq_start:0"), 0);
    drop(src);
}

#[test]
fn a_stop_cuts_a_reconnect_short() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(48_000.0, 7.1e6, None)).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 1024, 3.0).len(), 1024);
    // The server goes silent in the handshake: the reconnect waits in it.
    f.cfg(|c| c.mode = Mode::Silent);
    f.drop_client();
    assert!(wait_for(|| f.conns() >= 2, 4.0));
    std::thread::sleep(Duration::from_millis(200));
    let t0 = Instant::now();
    src.stop();
    assert!(
        t0.elapsed() < Duration::from_millis(800),
        "{:?}",
        t0.elapsed()
    );
}
