//! The watch as an owner: watches set by commands and read back as a table, names resolved when a watch is set, alerts as
//! events, limits, and pushes to every channel against a fake HTTP server on this machine.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

use std::sync::{Arc, LazyLock};

use sdrkit_core::command::CommandError;
use sdrkit_core::net::StdNet;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::{field, Field};
use sdrkit_core::{Choice, Clock, Event, EventCatalog, EventDef, MemStorage, Record, Value};
use sdrkit_svc_events::{Context, Kinds};
use sdrkit_svc_watch::{Outcome, Watch, WatchWhat, WATCH_EVENTS};

const SQUAWK: Field<field::Text> = Field::new("squawk", "Squawk", "");
static PLANE: EventDef = EventDef::new(
    "air.plane",
    "Plane",
    &[
        &keys::SOURCE,
        &keys::FAMILY,
        &keys::URGENCY,
        &keys::TEXT,
        &SQUAWK,
    ],
);
static SHIP: EventDef = EventDef::new("sea.ship", "Ship", &[&keys::SOURCE, &keys::FAMILY]);
static CALL: EventDef = EventDef::new(
    "voice.call",
    "Call",
    &[&keys::SOURCE, &keys::TEXT, &keys::PRIVATE],
);

fn kinds() -> Arc<Kinds> {
    static K: LazyLock<Arc<Kinds>> = LazyLock::new(|| {
        let mut defs: Vec<&'static EventDef> = vec![&PLANE, &SHIP, &CALL];
        defs.extend(WATCH_EVENTS);
        Arc::new(Kinds::new(Arc::new(EventCatalog::new(&defs).unwrap())))
    });
    Arc::clone(&K)
}

fn st(mut e: Event) -> Event {
    kinds().catalog().stamp(&mut e);
    e
}

fn open(storage: &Arc<MemStorage>) -> Watch {
    Watch::open(
        kinds(),
        storage.clone(),
        Arc::new(StdNet),
        Clock::fixed(500.0),
    )
    .unwrap()
}

fn call(w: &mut Watch, cmd: &str, args: &[(&str, Value)]) -> Result<Option<Record>, CommandError> {
    let id = w.commands().id(cmd)?;
    let mut given = w.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let i = given.decl().id(k).unwrap();
        given.set_value(i, v).unwrap();
    }
    owner::invoke(w, id, &given)
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

fn what(w: WatchWhat) -> Value {
    Value::Choice(w.index())
}

fn check(w: &mut Watch, t: f64, e: &Event) -> Vec<Event> {
    let k = kinds();
    let c = Context {
        unix_time: t,
        receiver: None,
    };
    let o = k.observe(e, &c).unwrap();
    let mut out = Vec::new();
    w.check(&o, 0, &mut out);
    out
}

fn ship(mmsi: &str) -> Event {
    st(SHIP
        .event(0.0, 161.975e6)
        .set(&keys::SOURCE, mmsi)
        .set(&keys::FAMILY, Family::Vessel))
}

fn plane(id: &str, squawk: &str) -> Event {
    st(PLANE
        .event(0.0, 1090e6)
        .set(&keys::SOURCE, id)
        .set(&keys::FAMILY, Family::Aircraft)
        .set(&SQUAWK, squawk))
}

const OUTCOME: Field<field::Choice<Outcome>> = Field::new("outcome", "", "");
const TITLE: Field<field::Text> = Field::new("title", "", "");

#[test]
fn the_watch_is_an_owner() {
    let storage = Arc::new(MemStorage::new());
    let mut w = open(&storage);
    for t in ["watches", "alerts", "pushes"] {
        assert!(w.results_decl().table_id(t).is_some(), "{t}");
    }
    for c in [
        "add",
        "remove",
        "clear",
        "push.add",
        "push.remove",
        "alert.test",
    ] {
        assert!(w.commands().id(c).is_ok(), "{c}");
    }
    assert!(matches!(
        call(
            &mut w,
            "add",
            &[("row", text("w")), ("hz", Value::Hz(-1.0))]
        ),
        Err(CommandError::BadArgument { .. })
    ));
    assert!(matches!(
        call(&mut w, "add", &[]),
        Err(CommandError::MissingArgument { .. })
    ));
    assert!(matches!(
        call(&mut w, "remove", &[("row", text("nothing"))]),
        Err(CommandError::NoSuchRow { .. })
    ));
    call(
        &mut w,
        "add",
        &[("row", text("dad")), ("value", text("503123456"))],
    )
    .unwrap();
    let b = open(&storage);
    let t = b.results_decl().table_id("watches").unwrap();
    assert!(
        b.results().table(t).unwrap().row("dad").is_some(),
        "the watches survive a restart"
    );
}

#[test]
fn an_unknown_field_fails_when_the_watch_is_set() {
    let mut w = open(&Arc::new(MemStorage::new()));
    let r = call(
        &mut w,
        "add",
        &[
            ("row", text("s")),
            ("what", what(WatchWhat::Field)),
            ("kinds", text("sea.ship")),
            ("field", text("squawk")),
        ],
    );
    let Err(CommandError::Failed { source }) = r else {
        panic!("{r:?}")
    };
    assert!(matches!(
        source.downcast_ref::<sdrkit_svc_watch::WatchError>(),
        Some(sdrkit_svc_watch::WatchError::Name {
            arg: "field",
            error: sdrkit_svc_events::NameError::UnknownField { at: 0, .. }
        })
    ));
}

#[test]
fn watches_match_and_the_limits_hold() {
    let mut w = open(&Arc::new(MemStorage::new()));
    call(
        &mut w,
        "add",
        &[
            ("row", text("dad")),
            ("label", text("Dad's ship")),
            ("value", text("503-123 456")),
            ("families", text("vessel")),
        ],
    )
    .unwrap();
    call(
        &mut w,
        "add",
        &[
            ("row", text("sq")),
            ("what", what(WatchWhat::Field)),
            ("field", text("squawk")),
            ("value", text("75*")),
        ],
    )
    .unwrap();
    call(
        &mut w,
        "add",
        &[
            ("row", text("f")),
            ("what", what(WatchWhat::Frequency)),
            ("hz", Value::Hz(161.975e6)),
            ("tolerance_hz", Value::Hz(100.0)),
        ],
    )
    .unwrap();
    let a = check(&mut w, 0.0, &ship("503123456"));
    assert_eq!(a.len(), 2, "the identity and the frequency");
    assert_eq!(a[0].get(&TITLE), Some("Watch: Dad's ship"));
    assert_eq!(a[0].get(&OUTCOME), Some(Outcome::Alerted));
    assert!(check(&mut w, 1.0, &plane("7C6DB8", "7500")).len() == 1);
    assert!(check(&mut w, 2.0, &plane("7C6DB8", "7000")).is_empty());
    // The same thing within the cooldown is held back.
    let again = check(&mut w, 60.0, &ship("503123456"));
    assert_eq!(again[0].get(&OUTCOME), Some(Outcome::Cooldown));
    // An urgent event alerts without a watch, and private words are never read.
    let mayday = st(PLANE
        .event(0.0, 1090e6)
        .set(&keys::SOURCE, "7C0001")
        .set(&keys::URGENCY, Urgency::Emergency)
        .set(&keys::TEXT, "squawk 7700"));
    let e = check(&mut w, 70.0, &mayday);
    assert_eq!(e[0].get(&TITLE), Some("Emergency: squawk 7700"));
    call(
        &mut w,
        "add",
        &[
            ("row", text("word")),
            ("what", what(WatchWhat::Word)),
            ("value", text("Secret")),
        ],
    )
    .unwrap();
    let private = st(CALL
        .event(0.0, 1.0)
        .set(&keys::SOURCE, "100")
        .set(&keys::TEXT, "the secret")
        .set(&keys::PRIVATE, true));
    assert!(check(&mut w, 80.0, &private).is_empty());
    let open_call = st(CALL
        .event(0.0, 1.0)
        .set(&keys::SOURCE, "100")
        .set(&keys::TEXT, "the secret"));
    assert_eq!(check(&mut w, 81.0, &open_call).len(), 1);
    // The quiet hours keep an alert without a sound.
    let id = w.schema().id("quiet_from").unwrap();
    assert!(owner::apply(&mut w, id, &text("25:00")).is_err());
    owner::apply(&mut w, id, &text("00:00")).unwrap();
    let id = w.schema().id("quiet_to").unwrap();
    owner::apply(&mut w, id, &text("06:00")).unwrap();
    let q = check(
        &mut w,
        3600.0,
        &ship("503999999").set(&keys::SOURCE, "503999999"),
    );
    assert_eq!(q[0].get(&OUTCOME), Some(Outcome::Quiet));
    let t = w.results_decl().table_id("alerts").unwrap();
    assert!(w.results().table(t).unwrap().len() >= 6);
}

/// A fake HTTP server answering 200, reporting (request line and headers, body).
fn server() -> (u16, Receiver<(String, String)>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let (tx, rx) = channel();
    thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { return };
            let mut r = BufReader::new(s);
            let mut head = String::new();
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
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
            r.get_mut()
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
            let _ = tx.send((head, String::from_utf8(body).unwrap()));
        }
    });
    (port, rx)
}

#[test]
fn a_match_is_pushed_to_ntfy_telegram_and_a_webhook() {
    let (port, rx) = server();
    let base = format!("http://127.0.0.1:{port}");
    let mut w = open(&Arc::new(MemStorage::new()));
    let ch = |c: sdrkit_svc_watch::Channel| Value::Choice(c.index());
    call(
        &mut w,
        "push.add",
        &[
            ("row", text("n")),
            ("channel", ch(sdrkit_svc_watch::Channel::Ntfy)),
            ("address", text(&base)),
            ("topic", text("radio")),
            ("token", text("ntfy-token")),
        ],
    )
    .unwrap();
    call(
        &mut w,
        "push.add",
        &[
            ("row", text("t")),
            ("channel", ch(sdrkit_svc_watch::Channel::Telegram)),
            ("address", text(&base)),
            ("topic", text("42")),
            ("token", text("123:abc")),
        ],
    )
    .unwrap();
    call(
        &mut w,
        "push.add",
        &[
            ("row", text("w")),
            ("channel", ch(sdrkit_svc_watch::Channel::Webhook)),
            ("address", text(&format!("{base}/alert"))),
            ("token", text("hook")),
        ],
    )
    .unwrap();
    call(
        &mut w,
        "add",
        &[
            ("row", text("dad")),
            ("label", text("Dad's ship")),
            ("value", text("503123456")),
        ],
    )
    .unwrap();
    let alerts = check(&mut w, 0.0, &ship("503123456"));
    assert_eq!(alerts.len(), 1);
    let mut got: Vec<(String, String)> = (0..3)
        .map(|_| rx.recv_timeout(Duration::from_secs(10)).unwrap())
        .collect();
    got.sort();
    let ntfy = got.iter().find(|g| g.0.starts_with("POST /radio")).unwrap();
    assert!(
        ntfy.0.contains("Title: Watch: Dad's ship")
            && ntfy.0.contains("Authorization: Bearer ntfy-token")
    );
    assert!(ntfy.1.starts_with("503123456: 503123456 on 161.9750 MHz"));
    let tg = got
        .iter()
        .find(|g| g.0.starts_with("POST /bot123:abc/sendMessage"))
        .unwrap();
    assert!(tg.1.contains("\"chat_id\":\"42\""));
    let hook = got.iter().find(|g| g.0.starts_with("POST /alert")).unwrap();
    assert!(hook.1.contains("\"entry\":\"dad\""));
    // The tokens never show in the pushes table.
    w.tick();
    let t = w.results_decl().table_id("pushes").unwrap();
    let rows: Vec<_> = w
        .results()
        .table(t)
        .unwrap()
        .rows_newest_first()
        .map(|(_, r)| r.clone())
        .collect();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r
        .iter()
        .all(|(_, v)| !matches!(v, Value::Text(x) if x.contains("token") || x.contains("abc")))));
    // A cooldown is not pushed.
    check(&mut w, 1.0, &ship("503123456"));
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    // The test alert goes to every channel, and to the host as an event.
    let r = call(&mut w, "alert.test", &[]).unwrap().unwrap();
    assert!(r.iter().any(|(_, v)| matches!(v, Value::Text(_))));
    for _ in 0..3 {
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    let next = check(&mut w, 2.0, &plane("X", "1200"));
    assert_eq!(next[0].get(&TITLE), Some("sdrkit test alert"));
}
