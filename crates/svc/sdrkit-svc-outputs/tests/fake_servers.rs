//! Every network output against a small fake server on this machine, and the outputs as an owner. Nothing here reaches a
//! real service.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, LazyLock};
use std::thread;
use std::time::{Duration, Instant};

use sdrkit_core::command::CommandError;
use sdrkit_core::net::StdNet;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::{field, Field};
use sdrkit_core::{
    Choice, Clock, Event, EventCatalog, EventDef, MemStorage, Record, SettingError, Storage,
    StorageError, StorePath, Value,
};
use sdrkit_svc_events::{Context, Kinds, NameError};
use sdrkit_svc_outputs::aprsis::passcode;
use sdrkit_svc_outputs::ax25::kiss_decode;
use sdrkit_svc_outputs::hub::{CONNECTED, DROPPED, ERRORS, LAST_ERROR, LISTEN, SENT};
use sdrkit_svc_outputs::mqtt::read_packet;
use sdrkit_svc_outputs::{LineFormat, OutputError, OutputKind, Outputs};

const ALTITUDE_FT: Field<field::I64> = Field::new("altitude_ft", "Altitude", "ft");
const TNC2: Field<field::Text> = Field::new("tnc2", "TNC2", "");
const SENTENCE: Field<field::Text> = Field::new("sentence", "Sentence", "");
static PLANE: EventDef = EventDef::new(
    "adsb.aircraft",
    "Aircraft",
    &[
        &keys::SOURCE,
        &keys::FAMILY,
        &keys::POSITION,
        &keys::CALLSIGN,
        &ALTITUDE_FT,
    ],
);
static PACKET: EventDef = EventDef::new(
    "aprs.packet",
    "Packet",
    &[&keys::SOURCE, &keys::FAMILY, &TNC2],
);
static NMEA: EventDef = EventDef::new("ais.nmea", "NMEA", &[&SENTENCE]);
static DECODE: EventDef = EventDef::new(
    "wsjt.decode",
    "Decode",
    &[
        &keys::CALLSIGN,
        &keys::LOCATOR,
        &keys::SNR_DB,
        &keys::SPOT_MODE,
    ],
);

fn kinds() -> Arc<Kinds> {
    static K: LazyLock<Arc<Kinds>> = LazyLock::new(|| {
        Arc::new(Kinds::new(Arc::new(
            EventCatalog::new(&[&PLANE, &PACKET, &NMEA, &DECODE]).unwrap(),
        )))
    });
    Arc::clone(&K)
}

fn st(mut e: Event) -> Event {
    kinds().catalog().stamp(&mut e);
    e
}

fn aircraft() -> Event {
    st(PLANE
        .event(0.0, 1090e6)
        .set(&keys::SOURCE, "7C6DB8")
        .set(&keys::FAMILY, Family::Aircraft)
        .set(&keys::CALLSIGN, "QFA1")
        .set(&ALTITUDE_FT, 3000)
        .set(&keys::POSITION, (-33.9, 151.2)))
}

fn aprs(tnc2: &str) -> Event {
    st(PACKET
        .event(0.0, 144.39e6)
        .set(&keys::SOURCE, tnc2.split('>').next().unwrap_or(""))
        .set(&keys::FAMILY, Family::Aprs)
        .set(&TNC2, tnc2))
}

const HEARD: &str = "VK2ABC-9>APRS,WIDE1-1*:!3351.00S/15112.00E>hello";

fn ctx() -> Context {
    Context {
        unix_time: 1_762_598_527.0,
        receiver: None,
    }
}

fn submit(h: &mut Outputs, e: &Event) {
    let k = kinds();
    let o = k.observe(e, &ctx()).unwrap();
    h.submit(&o);
}

fn send_to(h: &mut Outputs, row: &str, e: &Event) -> Result<(), OutputError> {
    let k = kinds();
    let o = k.observe(e, &ctx()).unwrap();
    h.send_to(row, &o)
}

fn call(
    h: &mut Outputs,
    cmd: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = h.commands().id(cmd)?;
    let mut given = h.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let i = given.decl().id(k).unwrap();
        given.set_value(i, v).unwrap();
    }
    owner::invoke(h, id, &given)
}

fn hub_on(storage: Arc<MemStorage>) -> Outputs {
    let mut h = Outputs::open(
        kinds(),
        storage,
        Arc::new(StdNet),
        Clock::fixed(1_762_598_527.0),
    )
    .unwrap();
    call(&mut h, "start", &[]).unwrap();
    h
}

fn hub() -> Outputs {
    hub_on(Arc::new(MemStorage::new()))
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

fn choice<C: Choice>(c: C) -> Value {
    Value::Choice(c.index())
}

/// Adds output `name` of `kind`, on, with a queue of 100 and `args`.
fn add(
    h: &mut Outputs,
    name: &str,
    kind: OutputKind,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let mut all = vec![
        ("row", text(name)),
        ("kind", choice(kind)),
        ("enabled", Value::Bool(true)),
        ("queue", Value::I64(100)),
    ];
    all.extend(args.iter().cloned());
    call(h, "output.add", &all)
}

/// The row of output `name`, with the counters brought up to date.
fn row(h: &mut Outputs, name: &str) -> Record {
    h.tick();
    let t = h.results_decl().table_id("outputs").unwrap();
    h.results().table(t).unwrap().row(name).unwrap().clone()
}

fn n(r: &Record, f: &Field<field::I64>) -> i64 {
    r.get(f).unwrap_or(0)
}

/// `sentence` with its NMEA checksum.
fn with_checksum(sentence: &str) -> String {
    let sum = sdrkit_util_geo::nmea::checksum(&sentence.as_bytes()[1..]);
    format!("{sentence}*{sum:02X}")
}

fn wait_for(what: &str, mut f: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while !f() {
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "timed out waiting for {what}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

/// What the fake broker saw published: topic, payload, retain.
type Published = (String, Vec<u8>, bool);

/// A fake MQTT broker: answers CONNECT with `rc`, acknowledges QoS 1, and reports (topic, payload, retain) of each PUBLISH, and
/// the CONNECT body.
fn broker(rc: u8) -> (u16, Receiver<Published>, Receiver<Vec<u8>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let (tx, rx) = channel();
    let (ctx, crx) = channel();
    thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { return };
            let (tx, ctx) = (tx.clone(), ctx.clone());
            thread::spawn(move || {
                let Ok((h, body)) = read_packet(&mut s) else {
                    return;
                };
                assert_eq!(h, 0x10);
                let _ = ctx.send(body);
                s.write_all(&[0x20, 0x02, 0x00, rc]).unwrap();
                while let Ok((h, body)) = read_packet(&mut s) {
                    match h >> 4 {
                        3 => {
                            let qos = (h >> 1) & 3;
                            let tl = u16::from_be_bytes([body[0], body[1]]) as usize;
                            let topic = String::from_utf8(body[2..2 + tl].to_vec()).unwrap();
                            let mut at = 2 + tl;
                            if qos > 0 {
                                s.write_all(&[0x40, 0x02, body[at], body[at + 1]]).unwrap();
                                at += 2;
                            }
                            let _ = tx.send((topic, body[at..].to_vec(), h & 1 == 1));
                        }
                        12 => s.write_all(&[0xD0, 0x00]).unwrap(),
                        14 => return,
                        _ => {}
                    }
                }
            });
        }
    });
    (port, rx, crx)
}

/// A fake HTTP server: answers each request with the next status, and reports (head, body).
fn http_server(statuses: Vec<u16>) -> (u16, Receiver<(String, String)>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let (tx, rx) = channel();
    thread::spawn(move || {
        let mut statuses = statuses.into_iter();
        for s in l.incoming() {
            let Ok(s) = s else { return };
            let mut r = BufReader::new(s);
            let mut head = String::new();
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                if line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let len = head
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut body = vec![0u8; len];
            r.read_exact(&mut body).unwrap();
            let status = statuses.next().unwrap_or(200);
            let reply = format!("HTTP/1.1 {status} X\r\nContent-Length: 2\r\n\r\nok");
            r.get_mut().write_all(reply.as_bytes()).unwrap();
            let _ = tx.send((head, String::from_utf8(body).unwrap()));
        }
    });
    (port, rx)
}

#[test]
fn the_outputs_are_an_owner() {
    let storage = Arc::new(MemStorage::new());
    let mut h = hub_on(storage.clone());
    assert!(h.results_decl().table_id("outputs").is_some());
    assert!(h.results_decl().table_id("problems").is_some());
    for c in [
        "output.add",
        "output.enable",
        "output.remove",
        "start",
        "stop",
    ] {
        assert!(h.commands().id(c).is_ok(), "{c}");
    }
    assert!(matches!(
        add(
            &mut h,
            "u",
            OutputKind::Udp,
            &[("max_per_s", Value::F64(-1.0))]
        ),
        Err(CommandError::BadArgument {
            error: SettingError::OutOfRange { .. },
            ..
        })
    ));
    assert!(matches!(
        call(
            &mut h,
            "output.enable",
            &[("row", text("nothing")), ("on", Value::Bool(true))]
        ),
        Err(CommandError::NoSuchRow { .. })
    ));
    assert!(matches!(
        call(&mut h, "output.remove", &[("row", text("nothing"))]),
        Err(CommandError::NoSuchRow { .. })
    ));
    // An address is checked when it is set.
    let Err(CommandError::Failed { source }) = add(
        &mut h,
        "u",
        OutputKind::Udp,
        &[("address", text("no port"))],
    ) else {
        panic!("a bad address is refused")
    };
    assert!(matches!(
        source.downcast_ref::<OutputError>(),
        Some(OutputError::Setting(
            sdrkit_svc_outputs::BadSetting::Address
        ))
    ));
    // The outputs survive a restart through the storage, and a password never shows.
    add(
        &mut h,
        "m",
        OutputKind::Mqtt,
        &[
            ("address", text("mqtt://127.0.0.1:9")),
            ("password", text("hunter2")),
            ("enabled", Value::Bool(false)),
        ],
    )
    .unwrap();
    let r = row(&mut h, "m");
    assert_eq!(r.get(&sdrkit_svc_outputs::hub::HAS_PASSWORD), Some(true));
    assert!(r
        .iter()
        .all(|(_, v)| !matches!(v, Value::Text(t) if t.contains("hunter2"))));
    let mut b = hub_on(storage.clone());
    assert_eq!(
        row(&mut b, "m").get(&sdrkit_svc_outputs::hub::HAS_PASSWORD),
        Some(true)
    );
    storage.fail_next(StorageError::NoSpace);
    let Err(CommandError::Failed { source }) = call(&mut b, "output.remove", &[("row", text("m"))])
    else {
        panic!("a failed write is an error")
    };
    assert!(matches!(
        source.downcast_ref::<OutputError>(),
        Some(OutputError::Storage {
            source: StorageError::NoSpace,
            ..
        })
    ));
}

#[test]
fn an_unknown_name_in_a_format_fails_with_its_position() {
    let mut h = hub();
    let r = add(
        &mut h,
        "t",
        OutputKind::Udp,
        &[
            ("address", text("127.0.0.1:9")),
            ("kinds", text("aprs.*")),
            ("format", choice(LineFormat::Text)),
            ("template", text("{source} {altitude_ft}")),
        ],
    );
    let Err(CommandError::Failed { source }) = r else {
        panic!("{r:?}")
    };
    assert!(matches!(
        source.downcast_ref::<OutputError>(),
        Some(OutputError::Name {
            arg: "template",
            error: NameError::UnknownField { at: 10, .. }
        })
    ));
    let r = add(
        &mut h,
        "t",
        OutputKind::Udp,
        &[("address", text("127.0.0.1:9")), ("kinds", text("nope.*"))],
    );
    let Err(CommandError::Failed { source }) = r else {
        panic!("{r:?}")
    };
    assert!(matches!(
        source.downcast_ref::<OutputError>(),
        Some(OutputError::Name {
            arg: "kinds",
            error: NameError::UnknownKind { at: 0, .. }
        })
    ));
}

#[test]
fn mqtt_publishes_with_status_discovery_and_qos1() {
    let (port, rx, crx) = broker(0);
    let mut h = hub();
    add(
        &mut h,
        "mqtt",
        OutputKind::Mqtt,
        &[
            ("address", text(&format!("mqtt://127.0.0.1:{port}"))),
            ("client_id", text("test-client")),
            ("user", text("me")),
            ("password", text("secret")),
            ("qos", Value::I64(1)),
            ("discovery", Value::Bool(true)),
        ],
    )
    .unwrap();
    submit(&mut h, &aircraft());
    let connect = crx.recv_timeout(Duration::from_secs(10)).unwrap();
    let t = String::from_utf8_lossy(&connect);
    assert!(
        t.contains("test-client")
            && t.contains("sdrkit/status")
            && t.contains("offline")
            && t.contains("secret")
    );
    let mut got = Vec::new();
    while got.len() < 8 {
        got.push(rx.recv_timeout(Duration::from_secs(10)).unwrap());
    }
    assert_eq!(
        (got[0].0.as_str(), got[0].1.as_slice(), got[0].2),
        ("sdrkit/status", &b"online"[..], true)
    );
    let discovery: Vec<&str> = got[1..6].iter().map(|g| g.0.as_str()).collect();
    assert!(discovery.contains(&"homeassistant/sensor/sdrkit/last_aircraft/config"));
    assert!(String::from_utf8_lossy(&got[1].1).contains("\"availability_topic\":\"sdrkit/status\""));
    assert_eq!(got[6].0, "sdrkit/aircraft/adsb.aircraft/7C6DB8");
    assert!(String::from_utf8_lossy(&got[6].1).contains("\"identity\":\"7C6DB8\""));
    assert_eq!(
        (got[7].0.as_str(), got[7].2),
        ("sdrkit/last/aircraft", true)
    );
    wait_for("sent", || n(&row(&mut h, "mqtt"), &SENT) == 1);
    call(
        &mut h,
        "output.enable",
        &[("row", text("mqtt")), ("on", Value::Bool(false))],
    )
    .unwrap();
    // A clean goodbye publishes offline.
    let bye = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(
        (bye.0.as_str(), bye.1.as_slice()),
        ("sdrkit/status", &b"offline"[..])
    );
}

#[test]
fn mqtt_refusal_is_reported_without_the_password() {
    let (port, _rx, _crx) = broker(4);
    let mut h = hub();
    add(
        &mut h,
        "m",
        OutputKind::Mqtt,
        &[
            ("address", text(&format!("mqtt://127.0.0.1:{port}"))),
            ("user", text("me")),
            ("password", text("hunter2")),
        ],
    )
    .unwrap();
    submit(&mut h, &aircraft());
    wait_for("an error", || n(&row(&mut h, "m"), &ERRORS) > 0);
    let r = row(&mut h, "m");
    let e = r.get(&LAST_ERROR).unwrap().to_owned();
    assert!(e.contains("bad_login") && !e.contains("hunter2"), "{e}");
    assert_eq!(r.get(&CONNECTED), Some(false));
}

#[test]
fn webhook_posts_json_with_a_bearer_token_and_retries_a_server_error() {
    let (port, rx) = http_server(vec![503, 200]);
    let mut h = hub();
    add(
        &mut h,
        "hook",
        OutputKind::Webhook,
        &[
            ("address", text(&format!("http://127.0.0.1:{port}/hook"))),
            ("token", text("tok123")),
            ("timeout", Value::Duration(5.0)),
        ],
    )
    .unwrap();
    submit(&mut h, &aircraft());
    let (head1, _) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let (head2, body) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(head1.starts_with("POST /hook HTTP/1.1"));
    assert!(
        head2.contains("Authorization: Bearer tok123")
            && head2.contains("Content-Type: application/json")
    );
    assert!(body.contains("\"kind\":\"adsb.aircraft\""));
    wait_for("sent", || n(&row(&mut h, "hook"), &SENT) == 1);
    let r = row(&mut h, "hook");
    assert_eq!(n(&r, &ERRORS), 1);
    assert_eq!(r.get(&LAST_ERROR), Some("HTTP status 503"));
}

#[test]
fn udp_json_and_nmea() {
    let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
    rx.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let target = rx.local_addr().unwrap().to_string();
    let mut h = hub();
    add(
        &mut h,
        "udp",
        OutputKind::Udp,
        &[("address", text(&target))],
    )
    .unwrap();
    add(
        &mut h,
        "nmea",
        OutputKind::Udp,
        &[
            ("address", text(&target)),
            ("kinds", text("ais.nmea")),
            ("format", choice(LineFormat::Nmea)),
            ("template", text("{sentence}")),
        ],
    )
    .unwrap();
    submit(&mut h, &aircraft());
    let mut buf = [0u8; 65_536];
    let got = rx.recv(&mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf[..got]).contains("\"identity\":\"7C6DB8\""));
    let s1 =
        with_checksum("!AIVDM,2,1,3,B,55P5TL01VIaAL@7WKO@mBplU@<PDhh000000001S;AJ::4A80?4i@E53,0");
    let s2 = with_checksum("!AIVDM,2,2,3,B,1@0000000000000,2");
    call(
        &mut h,
        "output.enable",
        &[("row", text("udp")), ("on", Value::Bool(false))],
    )
    .unwrap();
    submit(
        &mut h,
        &st(NMEA
            .event(0.0, 162e6)
            .set(&SENTENCE, &format!("{s1}\r\n{s2}"))),
    );
    let got = rx.recv(&mut buf).unwrap();
    assert_eq!(&buf[..got], format!("{s1}\r\n").as_bytes());
    let got = rx.recv(&mut buf).unwrap();
    assert_eq!(&buf[..got], format!("{s2}\r\n").as_bytes());
}

#[test]
fn tcp_server_serves_kiss_and_sbs_and_stays_local() {
    let mut h = hub();
    let r = add(
        &mut h,
        "open",
        OutputKind::TcpServer,
        &[("address", text("0.0.0.0:0"))],
    );
    assert!(r.is_ok(), "the add itself works");
    let p = h.results_decl().table_id("problems").unwrap();
    assert!(
        h.results().table(p).unwrap().row("open").is_some(),
        "a start that fails is a problem row"
    );
    add(
        &mut h,
        "kiss",
        OutputKind::TcpServer,
        &[
            ("address", text("127.0.0.1:0")),
            ("kinds", text("aprs.packet")),
            ("format", choice(LineFormat::Kiss)),
            ("template", text("{tnc2}")),
        ],
    )
    .unwrap();
    add(
        &mut h,
        "sbs",
        OutputKind::TcpServer,
        &[
            ("address", text("127.0.0.1:0")),
            ("kinds", text("adsb.aircraft")),
            ("format", choice(LineFormat::Sbs)),
            ("fields", text("callsign=callsign, altitude=altitude_ft")),
        ],
    )
    .unwrap();
    let kiss_at = row(&mut h, "kiss").get(&LISTEN).unwrap().to_owned();
    let sbs_at = row(&mut h, "sbs").get(&LISTEN).unwrap().to_owned();
    let mut kiss = TcpStream::connect(kiss_at).unwrap();
    let sbs = TcpStream::connect(sbs_at).unwrap();
    kiss.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut sbs_r = BufReader::new(sbs);
    sbs_r
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let mut line = String::new();
    wait_for("an SBS line", || {
        submit(&mut h, &aircraft());
        sbs_r.read_line(&mut line).is_ok_and(|n| n > 0)
    });
    assert!(line.starts_with("MSG,1,1,1,7C6DB8,"), "{line}");
    submit(&mut h, &aprs(HEARD));
    let mut buf = vec![0u8; 512];
    let mut got = Vec::new();
    wait_for("a KISS frame", || {
        if let Ok(n) = kiss.read(&mut buf) {
            got.extend_from_slice(&buf[..n]);
        }
        got.iter().filter(|b| **b == 0xC0).count() >= 2
    });
    let frames = kiss_decode(&got);
    assert!(frames[0].ends_with(b"!3351.00S/15112.00E>hello"));
}

#[test]
fn file_rotates_by_size() {
    let storage = Arc::new(MemStorage::new());
    let mut h = hub_on(storage.clone());
    add(
        &mut h,
        "file",
        OutputKind::File,
        &[
            ("address", text("out/events.jsonl")),
            ("max_bytes", Value::I64(600)),
            ("keep", Value::I64(2)),
        ],
    )
    .unwrap();
    for _ in 0..10 {
        submit(&mut h, &aircraft());
    }
    wait_for("written", || n(&row(&mut h, "file"), &SENT) == 10);
    let p = |s: &str| StorePath::new(s).unwrap();
    assert!(storage.stat(&p("out/events.jsonl.1")).is_ok());
    assert!(
        storage.stat(&p("out/events.jsonl.3")).is_err(),
        "only two rotated files are kept"
    );
    let data = String::from_utf8(storage.read(&p("out/events.jsonl")).unwrap()).unwrap();
    assert!(data.len() <= 600);
    assert!(data.lines().all(|l| l.starts_with('{') && l.ends_with('}')));
    // A file that cannot open is an error, and the output tries again.
    add(
        &mut h,
        "f2",
        OutputKind::File,
        &[("address", text("out/second.jsonl"))],
    )
    .unwrap();
    storage.fail_next(StorageError::NoSpace);
    submit(&mut h, &aircraft());
    wait_for("an error and a retry", || {
        let r = row(&mut h, "f2");
        n(&r, &ERRORS) == 1 && n(&r, &SENT) == 1
    });
}

#[test]
fn aprs_is_gates_heard_packets_once() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let (tx, rx) = channel();
    thread::spawn(move || {
        let (s, _) = l.accept().unwrap();
        let mut r = BufReader::new(s);
        r.get_mut().write_all(b"# fake APRS-IS server\r\n").unwrap();
        let mut login = String::new();
        r.read_line(&mut login).unwrap();
        tx.send(login.clone()).unwrap();
        r.get_mut()
            .write_all(b"# logresp VK2XYZ-10 verified, server FAKE\r\n")
            .unwrap();
        loop {
            let mut line = String::new();
            if r.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            tx.send(line).unwrap();
        }
    });
    let mut h = hub();
    add(
        &mut h,
        "igate",
        OutputKind::AprsIs,
        &[
            ("address", text(&format!("127.0.0.1:{port}"))),
            ("callsign", text("VK2XYZ-10")),
            ("families", text("aprs")),
            ("format", choice(LineFormat::Text)),
            ("template", text("{tnc2}")),
        ],
    )
    .unwrap();
    submit(&mut h, &aircraft()); // not APRS: left out by the filter
    submit(&mut h, &aprs(HEARD));
    submit(&mut h, &aprs(HEARD)); // a repeat within 30 s
    submit(&mut h, &aprs("VK2DEF>APRS,TCPIP*:from the net"));
    submit(&mut h, &aprs("VK2GHI>APRS:second"));
    let login = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(
        login,
        format!(
            "user VK2XYZ-10 pass {} vers sdrkit {}\r\n",
            passcode("VK2XYZ"),
            env!("CARGO_PKG_VERSION")
        )
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(10)).unwrap(),
        "VK2ABC-9>APRS,WIDE1-1*,qAR,VK2XYZ-10:!3351.00S/15112.00E>hello\r\n"
    );
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(10)).unwrap(),
        "VK2GHI>APRS,qAR,VK2XYZ-10:second\r\n"
    );
    assert_eq!(passcode("N0CALL"), 13023);
}

#[test]
fn spots_are_batched_to_the_endpoint() {
    let (port, rx) = http_server(vec![]);
    let mut h = hub();
    add(
        &mut h,
        "spots",
        OutputKind::Spots,
        &[
            ("address", text(&format!("http://127.0.0.1:{port}/spots"))),
            ("callsign", text("VK2XYZ")),
            ("locator", text("QF56od")),
            ("batch", Value::Duration(3600.0)),
            ("max_batch", Value::I64(2)),
        ],
    )
    .unwrap();
    let spot = |call: &str| {
        st(DECODE
            .event(0.0, 14.074e6)
            .set(&keys::CALLSIGN, call)
            .set(&keys::SNR_DB, -10.0)
            .set(&keys::LOCATOR, "PM95")
            .set(&keys::SPOT_MODE, "ft8"))
    };
    submit(&mut h, &spot("JA1XYZ"));
    submit(&mut h, &spot("JA1XYZ")); // the same station and band: dropped
    submit(&mut h, &spot("W1AW"));
    let (_, body) = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(body.contains("\"callsign\":\"VK2XYZ\""), "{body}");
    let a = body.find("\"call\":\"JA1XYZ\"").unwrap();
    let b = body.find("\"call\":\"W1AW\"").unwrap();
    assert!(a < b && body.contains("\"mode\":\"FT8\""));
}

#[test]
fn off_outputs_send_nothing_and_filters_apply() {
    let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
    rx.set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    let mut h = hub();
    add(
        &mut h,
        "u",
        OutputKind::Udp,
        &[
            ("address", text(&rx.local_addr().unwrap().to_string())),
            ("families", text("aprs")),
            ("enabled", Value::Bool(false)),
        ],
    )
    .unwrap();
    submit(&mut h, &aprs(HEARD));
    let mut buf = [0u8; 4096];
    assert!(rx.recv(&mut buf).is_err(), "nothing while off");
    assert!(matches!(
        send_to(&mut h, "u", &aprs(HEARD)),
        Err(OutputError::Off)
    ));
    call(
        &mut h,
        "output.enable",
        &[("row", text("u")), ("on", Value::Bool(true))],
    )
    .unwrap();
    submit(&mut h, &aircraft()); // left out by the filter
    submit(&mut h, &aprs(HEARD));
    let got = rx.recv(&mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf[..got]).contains("\"family\":\"aprs\""));
    send_to(&mut h, "u", &aircraft()).unwrap();
    let got = rx.recv(&mut buf).unwrap();
    assert!(String::from_utf8_lossy(&buf[..got]).contains("adsb.aircraft"));
    call(&mut h, "output.remove", &[("row", text("u"))]).unwrap();
    assert!(matches!(
        call(
            &mut h,
            "output.enable",
            &[("row", text("u")), ("on", Value::Bool(true))]
        ),
        Err(CommandError::NoSuchRow { .. })
    ));
    assert!(matches!(
        send_to(&mut h, "u", &aircraft()),
        Err(OutputError::NoSuchOutput(_))
    ));
}

#[test]
fn tls_asked_for_is_never_sent_in_plain_text() {
    let mut h = hub();
    add(
        &mut h,
        "tls",
        OutputKind::Webhook,
        &[("address", text("https://127.0.0.1:9/hook"))],
    )
    .unwrap();
    submit(&mut h, &aircraft());
    wait_for("the refusal", || n(&row(&mut h, "tls"), &DROPPED) == 1);
    assert!(row(&mut h, "tls").get(&LAST_ERROR).unwrap().contains("TLS"));
}
