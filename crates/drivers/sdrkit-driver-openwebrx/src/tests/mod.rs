//! The driver against a fake OpenWebRX server on a fake network. Every transcript goes byte for byte. The timings are short
//! (`fast`). `the_default_timings` checks the real ones.

mod fake;

use crate::adpcm::Ima;
use crate::view::{Profile, RunTime};
use crate::*;
use fake::{sframe, Fake, CONFIG_20M};
use sdrkit_core::net::{FakeTransport, NetError};
use sdrkit_core::{Choice, Owner, Record, Schema, SettingId, StateBlob, Value};
use sdrkit_driver_core::ws::tls::{RustlsTransport, Trust};
use sdrkit_driver_core::ws::UpgradeError;
use sdrkit_driver_core::{ring, DriverKind, Protocol, RadioSetting, Retuned, RingReader, Started};
use std::thread;
use std::time::Instant;

const KEY: [u8; 16] = *b"the sample nonce";
const DSP_FIRST: &str = r#"{"type":"dspcontrol","params":{"mod":"usb","offset_freq":74000,"low_cut":0,"high_cut":5400,"squelch_level":-150}}"#;
const S: Duration = Duration::from_secs(1);
/// The time of the fixed clock.
const T0: f64 = 1_700_000_000.0;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn fast() -> Timing {
    Timing {
        connect_remote: ms(2000),
        connect_local: ms(2000),
        upgrade: ms(1500),
        greeting: ms(1500),
        config: ms(1500),
        profile_wait: ms(600),
        centre_wait: ms(600),
        profiles_wait: ms(300),
        stall: ms(1000),
        no_audio: ms(800),
        keepalive: ms(700),
        close_wait: ms(1000),
        backoff_base: ms(300),
        backoff_max: ms(5000),
        tick: ms(20),
        window: ms(200),
        reconnect_gap: ms(200),
        retry_gap: ms(50),
        fixed_mask: Some([0; 4]),
        fixed_key: Some(KEY),
    }
}

fn drv_with(f: &Fake, t: Timing) -> OpenWebRxDriver {
    OpenWebRxDriver::with_timing(f.transport(), Clock::fixed(T0), t)
}

fn drv(f: &Fake) -> OpenWebRxDriver {
    drv_with(f, fast())
}

fn kind() -> DriverKind {
    DriverKind::new(Box::new(OpenWebRxDriver::new(
        Arc::new(FakeTransport::new()),
        Clock::fixed(T0),
    )))
}

/// The values of a radio at `f`, then `settings` by key.
fn tune(f: f64, settings: &[(&str, Value)]) -> Values {
    let k = kind();
    let schema = k.schema();
    let mut v = Values::defaults(schema);
    v.set(schema.id("centre_hz").unwrap(), &Value::Hz(f))
        .unwrap();
    for (key, x) in settings {
        v.set(schema.id(key).unwrap(), x).unwrap();
    }
    v
}

fn text(t: &str) -> Value {
    Value::Text(t.into())
}

fn choice(c: impl Choice) -> Value {
    Value::Choice(c.index())
}

fn audio() -> (&'static str, Value) {
    ("delivery", choice(Delivery::Audio))
}

fn refused(s: RunTime) -> DriverError {
    DriverError::Refused {
        id: view::run_time_id(s),
    }
}

/// `base` with the run-time settings `set` (setting key, choice key), from the lists that `d` learnt about the server at
/// `addr`.
fn with_rt(d: &OpenWebRxDriver, addr: &str, base: &Values, set: &[(&str, &str)]) -> Values {
    let key = conn::parse_address(addr).unwrap().key();
    let lists = lock(&d.addr_state(&key).view).lists();
    let mut decls = base.schema().settings().to_vec();
    decls.extend(lists.settings());
    let schema = Arc::new(Schema::new(decls).unwrap());
    let mut v = Values::defaults(&schema);
    for i in 0..base.schema().len() {
        let id = SettingId(i as u16);
        v.set(id, base.value(id).unwrap()).unwrap();
    }
    for (k, c) in set {
        let id = schema.id(k).unwrap();
        let decl = schema.setting(id).unwrap();
        let at = decl.field.choices.iter().position(|x| x.key == *c);
        v.set(id, &Value::Choice(at.expect("a listed choice") as u16))
            .unwrap();
    }
    v
}

/// Lets the driver see the mode list and the features of the server.
fn learn_modes(fake: &Fake, d: &OpenWebRxDriver) {
    let (mut src, mut r, _) = open_std(fake, d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    r.text(MODES.replace('\n', "").as_str());
    r.text(FEATURES);
    let mut w = Watch::new();
    assert!(wait_watch(&mut src, &mut w, |w| w.rows(&MODES_TABLE) == 6));
    src.stop();
}

fn refused_std(s: RadioSetting) -> DriverError {
    DriverError::Refused { id: s.id() }
}

/// The error of this crate inside `e`.
fn own(e: &DriverError) -> Option<OwrxError> {
    match e {
        DriverError::Driver(s) => s.downcast_ref::<OwrxError>().copied(),
        _ => None,
    }
}

type Opened = Result<Box<dyn Source>, DriverError>;

fn spawn_open(d: &OpenWebRxDriver, addr: &str, v: Values) -> thread::JoinHandle<Opened> {
    let (d, addr) = (d.clone(), addr.to_string());
    thread::spawn(move || d.open(&addr, &v))
}

/// Opens with the standard server script. Returns the source (not started) and the connection, after the first dspcontrol
/// and the start were read.
fn open_std(
    fake: &Fake,
    d: &OpenWebRxDriver,
    v: Values,
) -> (Box<dyn Source>, fake::Remote, fake::CFrame) {
    let h = spawn_open(d, &fake.addr(), v);
    let mut r = fake.accept();
    r.standard();
    let src = h.join().unwrap().expect("open");
    let first = r.data();
    let start = r.data();
    assert_eq!(start.text(), proto::DSP_START);
    (src, r, first)
}

fn start(src: &mut Box<dyn Source>) -> (Started, RingReader) {
    let (w, rd) = ring(1 << 18, src.sample_type());
    let s = src.start(w).expect("start");
    thread::sleep(ms(100));
    (s, rd)
}

/// The results of a source over several polls, as a host keeps them.
struct Watch {
    r: Results,
    lines: Vec<String>,
    restarts: usize,
    stored: Vec<(SettingId, Value)>,
}

impl Watch {
    fn new() -> Watch {
        Watch {
            r: Results::new(DESCRIPTOR.decl().results().clone()),
            lines: Vec::new(),
            restarts: 0,
            stored: Vec::new(),
        }
    }
    fn pull(&mut self, s: &mut Box<dyn Source>) -> &mut Watch {
        let p = s.poll(&mut self.r);
        self.restarts += usize::from(p.restart);
        self.stored.extend(p.store);
        self.lines.extend(self.r.log_lines().iter().cloned());
        self.r.clear_log();
        self
    }
    fn log(&self) -> String {
        self.lines.join("\n")
    }
    fn has(&self, t: &str) -> bool {
        self.lines.iter().any(|l| l.contains(t))
    }
    fn frequency_stored(&self) -> Vec<f64> {
        self.stored
            .iter()
            .filter(|(id, _)| *id == RadioSetting::Centre.id())
            .filter_map(|(_, v)| match v {
                Value::Hz(f) => Some(*f),
                _ => None,
            })
            .collect()
    }
    fn row(&self, t: &TableDef, key: &str) -> Option<Record> {
        let id = self.r.decl().table_id(t.key)?;
        self.r.table(id)?.row(key).cloned()
    }
    fn rows(&self, t: &TableDef) -> usize {
        let id = self.r.decl().table_id(t.key).unwrap();
        self.r.table(id).unwrap().len()
    }
}

/// Waits until `f` holds of the results of `src`.
fn wait_watch(src: &mut Box<dyn Source>, w: &mut Watch, mut f: impl FnMut(&Watch) -> bool) -> bool {
    let end = Instant::now() + S;
    loop {
        w.pull(src);
        if f(w) {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        thread::sleep(ms(10));
    }
}

/// The samples in the ring, as 16-bit values.
fn drain(rd: &mut RingReader) -> Vec<i16> {
    let mut bytes = Vec::new();
    while rd.read_with(1 << 16, |b| bytes.extend_from_slice(b)) > 0 {}
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c))
        .collect()
}

/// Reads `n` samples from the ring within a second.
fn take(rd: &mut RingReader, n: usize) -> Vec<i16> {
    let mut got = Vec::new();
    let end = Instant::now() + S;
    while got.len() < n && Instant::now() < end {
        rd.wait(1, ms(20));
        got.extend(drain(rd));
    }
    got
}

// ---- Plain rules ----

#[test]
fn the_default_timings() {
    let t = Timing::default();
    assert_eq!(
        (
            t.connect_remote,
            t.connect_local,
            t.upgrade,
            t.greeting,
            t.config
        ),
        (8 * S, 10 * S, 10 * S, 10 * S, 15 * S)
    );
    assert_eq!(
        (
            t.profile_wait,
            t.centre_wait,
            t.stall,
            t.no_audio,
            t.keepalive
        ),
        (5 * S, 3 * S, 10 * S, 5 * S, 15 * S)
    );
    assert_eq!(
        (t.close_wait, t.backoff_base, t.backoff_max, t.reconnect_gap),
        (S, 16 * S, 512 * S, S)
    );
    let st = view::AddrState::default();
    let waits: Vec<u64> = (0..8).map(|_| st.back_off(&t).as_secs()).collect();
    assert_eq!(waits, [16, 32, 64, 128, 256, 512, 512, 512]);
    st.reset_backoff();
    assert_eq!(st.back_off(&t).as_secs(), 16);
}

#[test]
fn the_descriptor() {
    let k = kind();
    let d = k.descriptor();
    assert_eq!(
        (d.key, d.produces[0].kind, d.produces[0].sample),
        ("openwebrx", Kind::Real, SampleType::I16)
    );
    let own: Vec<_> = OwrxSetting::schema()
        .settings()
        .iter()
        .map(|s| s.field.key.clone())
        .collect();
    assert_eq!(
        own,
        [
            "delivery",
            "passband",
            "squelch",
            "secondary_offset",
            "dmr_slot",
            "move_centre",
            "magic_key",
            "user",
            "password"
        ]
    );
    let restart: Vec<_> = OwrxSetting::schema()
        .settings()
        .iter()
        .filter(|s| s.restart)
        .map(|s| s.field.key.clone())
        .collect();
    assert_eq!(restart, ["delivery", "user", "password"]);
    let v = Values::defaults(k.schema());
    let o = radio::Opts::from_values(&v, &view::Lists::default());
    assert_eq!(
        (
            o.servermode.as_str(),
            o.dmr_slot,
            o.squelch,
            o.secondary,
            o.profile
        ),
        ("usb", DmrSlot::Both, -150.0, None, Profile::Keep)
    );
    // Before the server sent its lists, the open radio adds only the profile rule.
    let keys: Vec<_> = view::Lists::default()
        .settings()
        .iter()
        .map(|s| s.field.key.to_string())
        .collect();
    assert_eq!(keys, ["profile"]);
    assert!(k.decl().results().table_id("bookmarks").is_some());
    assert!(k.decl().results().raster_id("waterfall").is_some());
    assert!(conn::parse_address("http://").is_err());
}

// ---- The upgrade ----

#[test]
fn upgrade_request_byte_for_byte_and_a_bad_accept() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    assert_eq!(
        r.req,
        "GET /ws/ HTTP/1.1\r\nHost: 127.0.0.1:8073\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: \
         dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: http://127.0.0.1:8073\r\nUser-Agent: sdrkit\r\n\r\n"
    );
    src.stop();

    let h = spawn_open(&d, &fake.addr(), tune(14e6, &[]));
    let mut r = fake.accept();
    r.send(
        fake::ANSWER_101
            .replace("s3pPLMBiTxaQ9kYGzzhZRbK+xOo=", "AAAA")
            .as_bytes(),
    );
    let e = h.join().unwrap().err().unwrap();
    assert_eq!(e, DriverError::Handshake(UpgradeError::Mismatch));
    assert!(fake.try_accept(ms(300)).is_none(), "final: no second try");

    let h = spawn_open(
        &d,
        &fake.addr(),
        tune(14e6, &[("user", text("alice")), ("password", text("pw"))]),
    );
    let mut r = fake.accept();
    assert!(
        r.req.contains("\r\nAuthorization: Basic YWxpY2U6cHc=\r\n"),
        "{}",
        r.req
    );
    r.standard();
    h.join().unwrap().unwrap().stop();
}

// ---- The greeting ----

#[test]
fn greeting_and_properties_byte_for_byte() {
    let fake = Fake::new();
    let d = drv(&fake);
    let h = spawn_open(&d, &fake.addr(), tune(14_176_700.0, &[]));
    let mut r = fake.accept();
    r.upgrade();
    let (g, p) = r.greeting();
    let mut want = vec![0x81, 0xac, 0, 0, 0, 0];
    want.extend_from_slice(b"SERVER DE CLIENT client=sdrkit type=receiver");
    assert_eq!(g.raw, want);
    let mut want = vec![0x81, 0xd5, 0, 0, 0, 0];
    want.extend_from_slice(
        br#"{"type":"connectionproperties","params":{"output_rate":12000,"hd_output_rate":48000}}"#,
    );
    assert_eq!(p.raw, want);
    let mut hello = vec![0x81, 0x2f];
    hello.extend_from_slice(b"CLIENT DE SERVER server=openwebrx version=1.2.2");
    r.send(&hello);
    r.text(CONFIG_20M);
    let mut src = h.join().unwrap().unwrap();
    let mut w = Watch::new();
    w.pull(&mut src);
    assert_eq!(w.r.fields().get(&SERVER), Some("OpenWebRX 1.2.2"));
    src.stop();
}

fn greet_with(hello: &[u8]) -> (Opened, Watch) {
    let fake = Fake::new();
    let d = drv(&fake);
    let h = spawn_open(&d, &fake.addr(), tune(14_176_700.0, &[]));
    let mut r = fake.accept();
    r.upgrade();
    r.greeting();
    r.send(hello);
    r.text(CONFIG_20M);
    let mut res = h.join().unwrap();
    drop(r);
    let mut w = Watch::new();
    if let Ok(s) = res.as_mut() {
        w.pull(s);
        s.stop();
    }
    (res, w)
}

#[test]
fn greeting_variants() {
    let mut classic = vec![0x81, 0x1d];
    classic.extend_from_slice(b"CLIENT DE SERVER openwebrx.py");
    let (e, _) = greet_with(&classic);
    assert_eq!(e.err().unwrap(), DriverError::Version { got: 0, need: 18 });
    let (e, _) = greet_with(&sframe(
        1,
        b"CLIENT DE SERVER server=openwebrx version=0.17.3",
    ));
    let e = e.err().unwrap();
    assert!(!e.retryable());
    assert_eq!(e, DriverError::Version { got: 17, need: 18 });
    let (ok, _) = greet_with(&sframe(
        1,
        b"CLIENT DE SERVER server=openwebrx version=1.2.70-plus",
    ));
    assert!(ok.is_ok());
    let (e, _) = greet_with(&sframe(1, br#"{"type":"config","value":{}}"#));
    assert_eq!(
        e.err().unwrap(),
        DriverError::Protocol(Protocol::NotThisRadio)
    );
    // Another name: treated as OpenWebRX while its messages are JSON.
    let (ok, w) = greet_with(&sframe(1, b"CLIENT DE SERVER server=fork"));
    assert!(ok.is_ok());
    assert!(w.has("the server calls itself fork (version unknown): treated as OpenWebRX"));
    let fake = Fake::new();
    let d = drv(&fake);
    let h = spawn_open(&d, &fake.addr(), tune(1e6, &[]));
    let mut r = fake.accept();
    r.upgrade();
    r.greeting();
    r.text("CLIENT DE SERVER server=fork");
    r.text("MSG setup");
    assert_eq!(
        h.join().unwrap().err().unwrap(),
        DriverError::Protocol(Protocol::NotThisRadio)
    );
}

#[test]
fn a_silent_greeting_and_no_configuration_are_retried() {
    let fake = Fake::new();
    let mut t = fast();
    t.greeting = ms(200);
    let d = drv_with(&fake, t);
    let addr = fake.addr();
    let h = spawn_open(&d, &addr, tune(1e6, &[]));
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut r = fake.accept();
        r.upgrade();
        held.push(r);
    }
    let e = h.join().unwrap().err().unwrap();
    assert!(e.retryable());
    assert_eq!(e, DriverError::Net(NetError::TimedOut));
    let mut t = fast();
    t.config = ms(200);
    let d = drv_with(&fake, t);
    let h = spawn_open(&d, &addr, tune(1e6, &[]));
    for _ in 0..4 {
        let mut r = fake.accept();
        r.upgrade();
        r.greeting();
        r.hello("1.2.2");
        held.push(r);
    }
    let e = h.join().unwrap().err().unwrap();
    assert!(e.retryable());
    assert_eq!(e, DriverError::Net(NetError::TimedOut));
}

#[test]
fn sdr_errors_before_the_configuration_are_retried() {
    let fake = Fake::new();
    let d = drv(&fake);
    let h = spawn_open(&d, &fake.addr(), tune(1e6, &[]));
    for _ in 0..4 {
        let mut r = fake.accept();
        r.upgrade();
        r.greeting();
        r.hello("1.2.2");
        r.text(r#"{"type":"sdr_error","value":"No SDR Devices available"}"#);
    }
    let e = h.join().unwrap().err().unwrap();
    assert_eq!(e, DriverError::Busy);
    assert!(fake.try_accept(ms(200)).is_none());
}

// ---- The first configuration and the start ----

#[test]
fn first_configuration_and_start_byte_for_byte() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, r, first) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    assert_eq!(CONFIG_20M.len(), 292);
    assert_eq!(fake::PROFILES.len(), 97);
    let mut want = vec![0x81, 0xf1, 0, 0, 0, 0];
    want.extend_from_slice(DSP_FIRST.as_bytes());
    assert_eq!(first.raw, want);
    let (st, _rd) = start(&mut src);
    // The stream is the USB audio from the dial up: 0 Hz is the dial, B/2 below the tuned frequency.
    assert_eq!((st.rate_hz, st.centre_hz), (12_000.0, 14_174_000.0));
    assert_eq!(src.sample_type(), SampleType::I16);
    let caps = src.capabilities().clone();
    assert_eq!(
        (caps.min_freq_hz, caps.max_freq_hz),
        (13_975_000.0, 14_225_000.0)
    );
    assert_eq!((caps.rates.min_hz, caps.rates.max_hz), (12_000.0, 12_000.0));
    assert_eq!(caps.gain_db, None);
    assert!(caps.transmit.is_none());
    let mut w = Watch::new();
    w.pull(&mut src);
    assert!(w.has("set by its AGC"), "{}", w.log());
    let f = w.r.fields();
    assert_eq!(f.get(&PROFILE_NAME), Some("RTL 20m"));
    assert_eq!(f.get(&PROFILE_IN_USE), Some("rtl|20m"));
    assert_eq!(
        (f.get(&BAND_LOW), f.get(&BAND_HIGH)),
        (Some(13_975_000.0), Some(14_225_000.0))
    );
    assert_eq!(
        (f.get(&WATERFALL_MIN), f.get(&WATERFALL_MAX)),
        (Some(-88.0), Some(-20.0))
    );
    // The profiles of the server, with the band of the one seen.
    let p = w.row(&PROFILES, "rtl|20m").unwrap();
    assert_eq!(
        (p.get(&P_NAME), p.get(&P_LOW), p.get(&P_HIGH)),
        (Some("RTL 20m"), Some(13_975_000.0), Some(14_225_000.0))
    );
    let p = w.row(&PROFILES, "rtl|40m").unwrap();
    assert_eq!((p.get(&P_NAME), p.get(&P_LOW)), (Some("RTL 40m"), None));
    src.stop();
    assert!(r.closed(S));
}

#[test]
fn a_frequency_of_0_takes_the_servers_start() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, _r, first) = open_std(&fake, &d, tune(0.0, &[]));
    assert_eq!(first.text(), DSP_FIRST);
    let (st, _rd) = start(&mut src);
    assert_eq!(st.centre_hz, 14_174_000.0);
    let mut w = Watch::new();
    w.pull(&mut src);
    assert_eq!(w.frequency_stored(), [14_176_700.0]);
    assert!(w.has("tuned to the server's start, 14.1767 MHz"));
    src.stop();
    let (mut src, _r, first) = open_std(&fake, &d, tune(0.0, &[audio()]));
    assert_eq!(
        first.text(),
        r#"{"type":"dspcontrol","params":{"mod":"usb","offset_freq":74000,"low_cut":300,"high_cut":3000,"squelch_level":-150}}"#
    );
    let (st, _rd) = start(&mut src);
    assert_eq!(st.centre_hz, 14_174_000.0);
    let mut w = Watch::new();
    w.pull(&mut src);
    assert!(w.has("not RF"), "{}", w.log());
    src.stop();
}

#[test]
fn a_tune_outside_the_band_is_refused_at_the_open() {
    let fake = Fake::new();
    let d = drv(&fake);
    let h = spawn_open(&d, &fake.addr(), tune(7_076_700.0, &[]));
    let mut r = fake.accept();
    r.standard();
    let e = h.join().unwrap().err().unwrap();
    assert_eq!(e, refused_std(RadioSetting::Centre));
    assert!(fake.try_accept(ms(200)).is_none(), "a refusal is final");
}

// ---- Tuning ----

#[test]
fn live_retune_inside_the_band() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let res = src.apply(&tune(14_000_700.0, &[])).unwrap();
    assert!(matches!(res, Retuned::Live(s) if s.centre_hz == 13_998_000.0));
    let mut want = vec![0x81, 0xb6, 0, 0, 0, 0];
    want.extend_from_slice(br#"{"type":"dspcontrol","params":{"offset_freq":-102000}}"#);
    assert_eq!(r.data().raw, want);
    src.apply(&tune(14_000_700.0, &[])).unwrap();
    assert!(r.quiet(ms(200)));
    let e = src.apply(&tune(13_976_700.0, &[])).err().unwrap();
    assert_eq!(e, refused_std(RadioSetting::Centre));
    let mut w = Watch::new();
    w.pull(&mut src);
    assert!(w.has(
        "13.9767 MHz is outside this OpenWebRX band (13.975 to 14.225 MHz, profile RTL 20m): choose a profile that covers it"
    ));
    assert!(r.quiet(ms(200)));
    src.stop();
}

/// Lets the driver see the band of the 40 m profile: another listener moves the server there and back.
fn learn_40m(fake: &Fake, d: &OpenWebRxDriver) {
    let (mut src, mut r, _) = open_std(fake, d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    r.text(r#"{"type":"config","value":{"profile_id":"40m","center_freq":7100000}}"#);
    r.data();
    src.stop();
}

#[test]
fn retune_by_profile() {
    let fake = Fake::new();
    let d = drv(&fake);
    learn_40m(&fake, &d);
    let follow = |f: f64| with_rt(&d, &fake.addr(), &tune(f, &[]), &[("profile", "follow")]);
    let (mut src, mut r, _) = open_std(&fake, &d, follow(14_176_700.0));
    let (_st, _rd) = start(&mut src);
    let caps = src.capabilities().clone();
    assert_eq!(
        (caps.min_freq_hz, caps.max_freq_hz),
        (6_975_000.0, 14_225_000.0)
    );
    let mut w = Watch::new();
    w.pull(&mut src);
    src.apply(&follow(7_076_700.0)).unwrap();
    let mut want = vec![0x81, 0xb7, 0, 0, 0, 0];
    want.extend_from_slice(br#"{"type":"selectprofile","params":{"profile":"rtl|40m"}}"#);
    assert_eq!(r.data().raw, want);
    r.text(r#"{"type":"config","value":{"profile_id":"40m","center_freq":7100000}}"#);
    let mut want = vec![0x81, 0xb5, 0, 0, 0, 0];
    want.extend_from_slice(br#"{"type":"dspcontrol","params":{"offset_freq":-26000}}"#);
    assert_eq!(r.data().raw, want);
    assert!(wait_watch(&mut src, &mut w, |w| w.frequency_stored() == [7_076_700.0]));
    src.stop();

    // No config within the wait: the offset stays.
    let (mut src, r, _) = open_std(&fake, &d, follow(14_000_700.0));
    let (_st, _rd) = start(&mut src);
    let mut w = Watch::new();
    src.apply(&follow(7_076_700.0)).unwrap();
    assert_eq!(r.data().sent().ty, "selectprofile");
    assert!(wait_watch(&mut src, &mut w, |w| w
        .has("the server did not move to 7.0767 MHz")));
    assert!(r.quiet(ms(200)));
    src.stop();

    // Keep the profile of the server, no centre moves: refused at once.
    let (mut src, r, _) = open_std(&fake, &d, tune(14_000_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let e = src.apply(&tune(7_076_700.0, &[])).err().unwrap();
    assert_eq!(e, refused_std(RadioSetting::Centre));
    assert!(r.quiet(ms(200)));
    src.stop();
}

#[test]
fn a_named_profile_is_selected_at_the_open() {
    let fake = Fake::new();
    let d = drv(&fake);
    learn_modes(&fake, &d);
    let named = |id: &str| {
        with_rt(
            &d,
            &fake.addr(),
            &tune(7_076_700.0, &[]),
            &[("profile", id)],
        )
    };
    let h = spawn_open(&d, &fake.addr(), named("rtl|40m"));
    let mut r = fake.accept();
    r.standard();
    assert_eq!(
        r.data().text(),
        r#"{"type":"selectprofile","params":{"profile":"rtl|40m"}}"#
    );
    r.text(r#"{"type":"config","value":{"profile_id":"40m","center_freq":7100000}}"#);
    let mut src = h.join().unwrap().unwrap();
    assert_eq!(r.data().params().offset_freq, Some(-26000));
    assert_eq!(r.data().text(), proto::DSP_START);
    // Another one: sent at once.
    src.apply(&named("rtl|20m")).unwrap();
    assert_eq!(r.data().text(), proto::select_profile("rtl|20m"));
    src.stop();
}

#[test]
fn centre_move() {
    let fake = Fake::new();
    let d = drv(&fake);
    let set = [
        ("move_centre", Value::Bool(true)),
        ("magic_key", text("s3cret")),
    ];
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &set));
    let (_st, _rd) = start(&mut src);
    src.apply(&tune(7_102_700.0, &set)).unwrap();
    let mut want = vec![0x81, 0xc5, 0, 0, 0, 0];
    want.extend_from_slice(
        br#"{"type":"setfrequency","params":{"frequency":7102700,"key":"s3cret"}}"#,
    );
    assert_eq!(r.data().raw, want);
    r.text(r#"{"type":"config","value":{"center_freq":7100000}}"#);
    let mut want = vec![0x81, 0xb0, 0, 0, 0, 0];
    want.extend_from_slice(br#"{"type":"dspcontrol","params":{"offset_freq":0}}"#);
    assert_eq!(r.data().raw, want);
    // The server does not allow it.
    r.text(r#"{"type":"config","value":{"allow_center_freq_changes":false}}"#);
    thread::sleep(ms(100));
    assert!(src.apply(&tune(14_176_700.0, &set)).is_err());
    assert!(r.quiet(ms(200)));
    // No answer: the warning after the wait names the reasons.
    r.text(r#"{"type":"config","value":{"allow_center_freq_changes":true}}"#);
    thread::sleep(ms(100));
    let mut w = Watch::new();
    src.apply(&tune(14_176_700.0, &set)).unwrap();
    assert_eq!(r.data().sent().ty, "setfrequency");
    assert!(wait_watch(&mut src, &mut w, |w| {
        w.has(
        "the server did not move to 14.1767 MHz (it may not allow listeners to move the centre, or the magic key is wrong)"
    )
    }));
    src.stop();
}

#[test]
fn band_moved_by_someone_else() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let mut w = Watch::new();
    w.pull(&mut src);
    let mut c = vec![0x81, 0x32];
    c.extend_from_slice(br#"{"type":"config","value":{"center_freq":14150000}}"#);
    r.send(&c);
    let mut want = vec![0x81, 0xb4, 0, 0, 0, 0];
    want.extend_from_slice(br#"{"type":"dspcontrol","params":{"offset_freq":24000}}"#);
    assert_eq!(r.data().raw, want);
    assert!(wait_watch(&mut src, &mut w, |w| w.has(
        "another listener moved the server to 14.025 to 14.275 MHz"
    )));
    assert!(w.frequency_stored().is_empty());
    r.text(r#"{"type":"config","value":{"center_freq":7100000}}"#);
    let mut want = vec![0x81, 0xb5, 0, 0, 0, 0];
    want.extend_from_slice(br#"{"type":"dspcontrol","params":{"offset_freq":119600}}"#);
    assert_eq!(r.data().raw, want);
    assert!(wait_watch(&mut src, &mut w, |w| w.frequency_stored() == [7_222_300.0]));
    assert!(
        w.has("another listener moved the server to 6.975 to 7.225 MHz; now tuned to 7.2223 MHz")
    );
    src.stop();
}

// ---- Audio and waterfall ----

#[test]
fn adpcm_audio_with_sync_reaches_the_ring() {
    let fake = Fake::new();
    let d = drv(&fake);
    let want = [1302i16, 1402, 1494, 1410];
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_174_000.0, &[audio()]));
    let (_st, mut rd) = start(&mut src);
    r.send(&[
        0x82, 0x0b, 0x02, 0x53, 0x59, 0x4e, 0x43, 0x14, 0x00, 0xd2, 0x04, 0x55, 0xb3,
    ]);
    assert_eq!(take(&mut rd, 4), want);
    src.stop();

    // Split across frames: the same 4 samples.
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_174_000.0, &[audio()]));
    let (_st, mut rd) = start(&mut src);
    r.send(&[0x82, 0x03, 0x02, 0x53, 0x59]);
    r.send(&[
        0x82, 0x09, 0x02, 0x4e, 0x43, 0x14, 0x00, 0xd2, 0x04, 0x55, 0xb3,
    ]);
    assert_eq!(take(&mut rd, 4), want);
    // A resync after 1000 bytes: 2 bytes dropped, 4 samples lost.
    let mut f = vec![0x02];
    f.extend(vec![0x11u8; 998]);
    r.bin(&f);
    r.bin(&[
        0x02, 0x00, 0x00, 0x53, 0x59, 0x4e, 0x43, 0x14, 0x00, 0xd2, 0x04, 0x55, 0xb3,
    ]);
    let mut w = Watch::new();
    assert!(wait_watch(&mut src, &mut w, |w| w
        .r
        .fields()
        .get(&RESYNC_LOST)
        == Some(4)));
    assert!(src.loss().unwrap().radio_lost >= 4);
    assert!(w.has("the audio stream lost its sync: about 4 samples lost"));
    src.stop();
}

#[test]
fn uncompressed_audio_keeps_an_odd_byte() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_174_000.0, &[audio()]));
    let (_st, mut rd) = start(&mut src);
    r.text(r#"{"type":"config","value":{"audio_compression":"none"}}"#);
    r.send(&[0x82, 0x05, 0x02, 0x00, 0x80, 0xff, 0x7f]);
    r.send(&[0x82, 0x04, 0x02, 0x01, 0x00, 0x02]);
    r.send(&[0x82, 0x02, 0x02, 0x00]);
    assert_eq!(take(&mut rd, 4), [-32768, 32767, 1, 2]);
    src.stop();
}

#[test]
fn waterfall_lines_become_the_raster() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let mut w = Watch::new();
    let raster = |w: &Watch| {
        let id = w.r.decl().raster_id("waterfall").unwrap();
        w.r.raster(id).cloned()
    };
    r.text(r#"{"type":"config","value":{"fft_size":6}}"#);
    r.send(&[
        0x82, 0x09, 0x01, 0xff, 0xff, 0xff, 0xff, 0x0d, 0x08, 0x03, 0x8d,
    ]);
    assert!(wait_watch(&mut src, &mut w, |w| raster(w)
        .is_some_and(|r| r.height == 1)));
    let ras = raster(&w).unwrap();
    let step = 250_000.0 / 6.0;
    assert_eq!(ras.width, 6);
    assert!((ras.x.at(0) - (13_975_000.0 + step / 2.0)).abs() < 1e-6);
    assert!((ras.x.at(1) - ras.x.at(0) - step).abs() < 1e-6);
    let want = [-99.49f32, -94.46, -62.44, -58.29, -99.87, -105.4];
    for (a, b) in ras.row(0).unwrap().iter().zip(want) {
        assert!((a - b).abs() < 1e-3);
    }
    r.text(r#"{"type":"config","value":{"fft_compression":"none"}}"#);
    let mut line = vec![0x82, 0x19, 0x01];
    line.extend_from_slice(&[
        0x00, 0x00, 0xc8, 0xc2, 0x00, 0x00, 0xbf, 0xc2, 0x00, 0x00, 0x71, 0xc2, 0x00, 0x00, 0x71,
        0xc2, 0x66, 0x66, 0xc5, 0xc2, 0x00, 0x00, 0xca, 0xc2,
    ]);
    r.send(&line);
    assert!(wait_watch(&mut src, &mut w, |w| raster(w)
        .is_some_and(|r| r.height == 2)));
    assert_eq!(
        raster(&w).unwrap().row(1).unwrap(),
        [-100.0, -95.5, -60.25, -60.25, -98.7, -101.0]
    );
    // A 70 000-byte line (a 16-bit length is not enough: a 64-bit length).
    r.text(r#"{"type":"config","value":{"fft_size":17500}}"#);
    let mut big = vec![0x82, 0x7f, 0, 0, 0, 0, 0, 0x01, 0x11, 0x71, 0x01];
    big.extend((0..17_500).flat_map(|i| (-(i as f32) / 100.0).to_le_bytes()));
    r.send(&big);
    assert!(wait_watch(&mut src, &mut w, |w| raster(w)
        .is_some_and(|r| r.width == 17_500)));
    let ras = raster(&w).unwrap();
    assert_eq!(ras.row(2).unwrap()[17_499], -174.99);
    // The older lines are narrower: the rest of their rows is empty.
    assert!(ras.row(0).unwrap()[6].is_nan());
    src.stop();
}

// ---- Small messages ----

#[test]
fn smeter_and_small_messages() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let mut w = Watch::new();
    let mut sm = vec![0x81, 0x20];
    sm.extend_from_slice(br#"{"type":"smeter","value":0.0001}"#);
    r.send(&sm);
    assert!(wait_watch(&mut src, &mut w, |w| w.r.status().level_dbfs == Some(-40.0)));
    assert_eq!(w.r.fields().get(&SMETER), Some(-40.0));
    r.text(r#"{"type":"smeter","value":0}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w.r.status().level_dbfs
        == Some(-200.0)));
    r.text(r#"{"type":"clients","value":3}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w
        .r
        .fields()
        .get(&LISTENERS)
        == Some(3)));
    assert_eq!(w.r.fields().get(&MAX_LISTENERS), Some(20));
    r.text(r#"{"type":"clients","value":{"count":4}}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w
        .r
        .fields()
        .get(&LISTENERS)
        == Some(4)));
    r.text(r#"{"type":"cpuusage","value":0.234}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w
        .r
        .fields()
        .get(&CPU)
        .is_some_and(|c| (c - 23.4).abs() < 1e-9)));
    r.text(r#"{"type":"receiver_details","value":{"receiver_name":"Test RX","receiver_asl":120,"receiver_gps":{"lat":47.5,"lon":19.0}}}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w
        .r
        .fields()
        .get(&RECEIVER)
        == Some("Test RX")));
    let p = &w.r.points()["receiver"];
    assert_eq!((p.lat, p.lon), (47.5, 19.0));
    assert_eq!(w.r.fields().get(&HEIGHT), Some(120.0));
    r.text(r#"{"type":"bookmarks","value":[{"name":"Beacon","frequency":14100000,"modulation":"cw"}]}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w.rows(&BOOKMARKS) == 1));
    // In the `iq` delivery the tune frequency is B/2 above the bookmark.
    let b = w.row(&BOOKMARKS, "14100000|Beacon").unwrap();
    assert_eq!(
        (
            b.get(&B_NAME),
            b.get(&B_FREQ),
            b.get(&B_MODE),
            b.get(&B_TUNE)
        ),
        (
            Some("Beacon"),
            Some(14_100_000.0),
            Some("cw"),
            Some(14_102_700.0)
        )
    );
    r.text(r#"{"type":"dial_frequencies","value":[{"frequency":14074000,"mode":"ft8"}]}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w
        .row(&DIAL_FREQUENCIES, "FT8|14074000")
        .is_some_and(
            |d| d.get(&D_MODE) == Some("FT8") && d.get(&D_FREQ) == Some(14_074_000.0)
        )));
    r.text(r#"{"type":"secondary_demod","value":"CQ CQ"}"#);
    r.text(r#"{"type":"secondary_demod","value":" DE TEST"}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w
        .r
        .fields()
        .get(&DECODER_LINE)
        == Some("CQ CQ DE TEST")));
    r.text(r#"{"type":"secondary_demod","value":{"mode":"FT8","msg":"CQ TEST JN97","db":-12,"dt":0.2,"freq":1234}}"#);
    r.text(r#"{"type":"wsjt_message","value":{"mode":"WSPR","msg":"K1ABC FN42 37","db":-20,"dt":0.1,"freq":1500,"drift":0}}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w.rows(&DECODES) == 2));
    let row = w.row(&DECODES, "000000000001").unwrap();
    assert_eq!(
        (
            row.get(&C_TIME),
            row.get(&C_MODE),
            row.get(&C_FREQ),
            row.get(&C_LEVEL),
            row.get(&C_OFFSET),
            row.get(&C_MESSAGE)
        ),
        (
            Some(T0),
            Some("FT8"),
            Some(1234.0),
            Some(-12.0),
            Some(0.2),
            Some("CQ TEST JN97")
        )
    );
    assert_eq!(
        w.row(&DECODES, "000000000002").unwrap().get(&C_REST),
        Some(r#"{"drift":0}"#)
    );
    r.text(r#"{"type":"metadata","value":{"protocol":"DMR","slot":1}}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w
        .row(&METADATA, "protocol")
        .is_some_and(|m| m.get(&V_VALUE) == Some("DMR"))));
    r.text(r#"{"type":"nonsense","value":1}"#);
    r.text(r#"{"type":"nonsense","value":1}"#);
    r.text("not json");
    r.text(r#"{"type":"log_message","value":"hello"}"#);
    r.text(r##"{"type":"chat_message","name":"op","text":"hi all","color":"#fff"}"##);
    r.text(r#"{"type":"temperature","value":45.2}"#);
    assert!(wait_watch(&mut src, &mut w, |w| w
        .r
        .fields()
        .get(&TEMPERATURE)
        == Some(45.2)));
    let n = w.log();
    assert_eq!(
        n.matches("the server sent a message of type nonsense, not used")
            .count(),
        1
    );
    assert_eq!(n.matches("not JSON").count(), 1);
    assert!(
        n.contains("server: hello") && n.contains("chat op: hi all"),
        "{n}"
    );
    // Nothing of that was an error: the connection is still up.
    assert!(r.quiet(ms(100)));
    src.stop();
}

// ---- Backoff ----

#[test]
fn backoff_waits_double_and_reset() {
    let fake = Fake::new();
    let d = drv(&fake);
    let h = spawn_open(&d, &fake.addr(), tune(14_176_700.0, &[]));
    let mut times = Vec::new();
    for _ in 0..4 {
        let mut r = fake.accept();
        times.push(r.at);
        r.upgrade();
        r.greeting();
        r.hello("1.2.2");
        let mut b = vec![0x81, 0x2e];
        b.extend_from_slice(br#"{"type":"backoff","reason":"Too many clients"}"#);
        r.send(&b);
        r.send(&[0x88, 0x02, 0x03, 0xe8]);
    }
    let e = h.join().unwrap().err().unwrap();
    assert!(e.retryable());
    assert_eq!(e, DriverError::Busy);
    for (i, want) in [300u64, 600, 1200].iter().enumerate() {
        let gap = times[i + 1] - times[i];
        assert!(gap >= ms(*want - 10), "gap {i}: {gap:?}");
    }
    // The next open waits 2.4 s (the fourth backoff), then a config resets the wait.
    let h = spawn_open(&d, &fake.addr(), tune(14_176_700.0, &[]));
    let mut r = fake.accept();
    assert!(r.at - times[3] >= ms(2390));
    r.standard();
    let mut src = h.join().unwrap().unwrap();
    let (_st, _rd) = start(&mut src);
    r.data();
    r.data();
    // Lost, then a backoff on the reconnect: 300 ms again; then a stop during the 600 ms wait.
    drop(r);
    let mut r = fake.accept();
    r.upgrade();
    r.greeting();
    r.hello("1.2.2");
    r.text(r#"{"type":"backoff","reason":"Too many clients"}"#);
    let t0 = Instant::now();
    let mut r2 = fake.accept();
    assert!(t0.elapsed() >= ms(250), "{:?}", t0.elapsed());
    r2.upgrade();
    r2.greeting();
    r2.hello("1.2.2");
    r2.text(r#"{"type":"backoff","reason":"Too many clients"}"#);
    drop(r);
    thread::sleep(ms(100));
    let t = Instant::now();
    src.stop();
    assert!(t.elapsed() < ms(200), "stop took {:?}", t.elapsed());
    let mut w = Watch::new();
    w.pull(&mut src);
    assert!(w.has("busy: Too many clients"), "{}", w.log());
}

// ---- HTTP answers ----

fn http(answer: &str, set: &[(&str, Value)]) -> DriverError {
    let fake = Fake::new();
    let d = drv(&fake);
    let h = spawn_open(&d, &fake.addr(), tune(1e6, set));
    let mut r = fake.accept();
    r.send(answer.as_bytes());
    h.join().unwrap().err().expect("refused")
}

#[test]
fn http_answers() {
    let e = http("HTTP/1.1 404 Not Found\r\n\r\n", &[]);
    assert_eq!(own(&e), Some(OwrxError::NoWebSocket { status: 404 }));
    let e = http(
        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"x\"\r\n\r\n",
        &[],
    );
    assert_eq!(own(&e), Some(OwrxError::LoginNeeded));
    assert!(!e.retryable());
    let e = http(
        "HTTP/1.1 401 Unauthorized\r\n\r\n",
        &[("user", text("alice")), ("password", text("x"))],
    );
    assert_eq!(own(&e), Some(OwrxError::LoginRefused));
    let e = http("HTTP/1.1 403 Forbidden\r\n\r\n", &[]);
    assert_eq!(own(&e), Some(OwrxError::Http { status: 403 }));
    let e = http("SSH-2.0-OpenSSH_9.0\r\n", &[]);
    assert_eq!(e, DriverError::Protocol(Protocol::NotThisRadio));
}

#[test]
fn redirects_are_followed_twice() {
    let net = FakeTransport::new();
    let a = Fake::on(&net, 8073);
    let b = Fake::on(&net, 8074);
    let d = drv(&a);
    let h = spawn_open(&d, &a.addr(), tune(14_176_700.0, &[]));
    let mut r = a.accept();
    r.send(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:8074/sdr/\r\n\r\n");
    let mut r2 = b.accept();
    assert!(
        r2.req.starts_with("GET /sdr/ws/ HTTP/1.1\r\n"),
        "{}",
        r2.req
    );
    r2.standard();
    h.join().unwrap().unwrap().stop();

    let h = spawn_open(&d, &a.addr(), tune(1e6, &[]));
    for _ in 0..3 {
        let mut r = a.accept();
        r.send(b"HTTP/1.1 307 Moved\r\nLocation: http://127.0.0.1:8073/\r\n\r\n");
    }
    let e = h.join().unwrap().err().unwrap();
    assert_eq!(own(&e), Some(OwrxError::TooManyRedirects));
    assert!(!e.retryable());
}

#[test]
fn a_busy_http_answer_waits_the_backoff() {
    let fake = Fake::new();
    let d = drv(&fake);
    let h = spawn_open(&d, &fake.addr(), tune(14_176_700.0, &[]));
    let mut r = fake.accept();
    let t0 = r.at;
    r.send(b"HTTP/1.1 503 Service Unavailable\r\n\r\n");
    let mut r = fake.accept();
    assert!(r.at - t0 >= ms(290), "{:?}", r.at - t0);
    r.standard();
    let mut src = h.join().unwrap().unwrap();
    let mut w = Watch::new();
    w.pull(&mut src);
    assert!(w.has("the server is busy; waiting 0.3 s"), "{}", w.log());
    src.stop();
    // Other server errors are tried again at once.
    let h = spawn_open(&d, &fake.addr(), tune(14_176_700.0, &[]));
    let mut r = fake.accept();
    r.send(b"HTTP/1.1 502 Bad Gateway\r\n\r\n");
    let mut r = fake.accept();
    r.standard();
    h.join().unwrap().unwrap().stop();
}

// ---- Frames ----

#[test]
fn fragments_with_a_ping_between() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let mut msg = br#"{"type":"log_message","value":""#.to_vec();
    msg.extend(vec![b'x'; 300 - msg.len() - 2]);
    msg.extend_from_slice(br#""}"#);
    assert_eq!(msg.len(), 300);
    let mut f1 = vec![0x01, 100];
    f1.extend_from_slice(&msg[..100]);
    let mut f2 = vec![0x00, 100];
    f2.extend_from_slice(&msg[100..200]);
    let mut f3 = vec![0x80, 100];
    f3.extend_from_slice(&msg[200..]);
    r.send(&f1);
    r.send(&[0x89, 0x04, 0x74, 0x65, 0x73, 0x74]);
    let pong = r.next(S).unwrap();
    assert_eq!(pong.raw, [0x8a, 0x84, 0, 0, 0, 0, 0x74, 0x65, 0x73, 0x74]);
    r.send(&f2);
    r.send(&f3);
    let mut w = Watch::new();
    let xs = format!("server: {}", "x".repeat(267));
    assert!(wait_watch(&mut src, &mut w, |w| w
        .lines
        .iter()
        .any(|l| l.ends_with(&xs))));
    src.stop();
}

fn loss_by(frame: &[u8], want: &str) {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let mut w = Watch::new();
    r.send(frame);
    // The loop ends. The base connects again.
    let mut r2 = fake.accept();
    assert!(wait_watch(&mut src, &mut w, |w| w.has(want)), "{}", w.log());
    r2.standard();
    assert_eq!(r2.data().text(), DSP_FIRST);
    src.stop();
}

#[test]
fn bad_frames_end_the_loop() {
    loss_by(
        &[0x81, 0x82, 1, 2, 3, 4, 0, 0],
        "the server sent a bad WebSocket frame",
    );
    loss_by(
        &[0x82, 127, 0, 0, 0, 0, 0, 0x50, 0, 0],
        "the server sent a message over 4 MiB",
    );
    loss_by(
        &[0x88, 0x06, 0x03, 0xe9, b'b', b'y', b'e', b'!'],
        "the server closed the connection",
    );
}

// ---- Stall, keep-alive, stop ----

#[test]
fn a_silent_server_stalls_and_the_reconnect_starts_afresh() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, r, _) = open_std(&fake, &d, tune(14_000_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let t = Instant::now();
    let mut r2 = fake.accept();
    assert!(t.elapsed() >= ms(800), "{:?}", t.elapsed());
    assert!(r.closed(S), "the stalled connection is closed");
    let mut w = Watch::new();
    w.pull(&mut src);
    assert!(w.has("127.0.0.1:8073: no data for 1 s"), "{}", w.log());
    r2.standard();
    assert_eq!(
        r2.data().text(),
        r#"{"type":"dspcontrol","params":{"mod":"usb","offset_freq":-102000,"low_cut":0,"high_cut":5400,"squelch_level":-150}}"#
    );
    assert_eq!(r2.data().text(), proto::DSP_START);
    assert!(wait_watch(&mut src, &mut w, |w| w.has("reconnected after")));
    src.stop();
}

#[test]
fn waterfall_without_audio_and_keepalive() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let line = sframe(2, &[0x01, 0xff, 0xff, 0xff, 0xff, 0x0d, 0x08, 0x03, 0x8d]);
    let mut ping = None;
    let t = Instant::now();
    while t.elapsed() < ms(1500) {
        r.send(&line);
        while let Some(f) = r.next(ms(1)) {
            if f.op == 0x9 {
                ping = Some(f);
            }
        }
        thread::sleep(ms(100));
    }
    let p = ping.expect("a keep-alive ping");
    assert_eq!(
        p.raw,
        [0x89, 0x86, 0, 0, 0, 0, b's', b'd', b'r', b'k', b'i', b't']
    );
    let mut w = Watch::new();
    w.pull(&mut src);
    let n = w.log();
    assert_eq!(
        n.matches("the server sends a waterfall but no audio")
            .count(),
        1,
        "{n}"
    );
    assert!(!n.contains("no data for"), "{n}");
    src.stop();
}

#[test]
fn stop_sends_a_close_and_returns_without_an_answer() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let t = Instant::now();
    src.stop();
    assert!(t.elapsed() < ms(1200), "{:?}", t.elapsed());
    let c = r.next(S).unwrap();
    assert_eq!(c.raw, [0x88, 0x82, 0, 0, 0, 0, 0x03, 0xe8]);
    assert!(r.closed(S));
    src.stop();
    // With a server that answers, the stop is quick.
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    let h = thread::spawn(move || {
        let c = r.next(S).unwrap();
        assert_eq!(c.op, 0x8);
        r.send(&[0x88, 0x02, 0x03, 0xe8]);
        r.closed(S)
    });
    let t = Instant::now();
    src.stop();
    assert!(t.elapsed() < ms(500), "{:?}", t.elapsed());
    assert!(h.join().unwrap());
    // A stop before the start closes too.
    let (mut src, r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    src.stop();
    assert!(r.closed(ms(1500)));
}

// ---- Streaming at the audio rate ----

#[test]
fn streams_at_the_audio_rate() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, mut rd) = start(&mut src);
    // 6 frames a second of 2000 samples (a sync header and 1000 bytes each): 12 000 samples a second.
    let mut enc = Ima::default();
    let tone: Vec<i16> = (0..2000 * 12)
        .map(|k| {
            (8000.0 * (2.0 * std::f64::consts::PI * 1000.0 * k as f64 / 12_000.0).sin()) as i16
        })
        .collect();
    let t0 = Instant::now();
    let mut got = Vec::new();
    // The rate is read while the audio arrives: a window that closes after the last frame holds no audio.
    let mut w = Watch::new();
    let mut sps = 0.0f64;
    for (i, c) in tone.chunks(2000).enumerate() {
        let state = enc;
        let mut f = vec![0x02];
        f.extend_from_slice(b"SYNC");
        f.extend_from_slice(&(state.index as i16).to_le_bytes());
        f.extend_from_slice(&(state.pred as i16).to_le_bytes());
        f.extend(enc.encode_all(c));
        r.bin(&f);
        let due = t0 + Duration::from_secs_f64((i + 1) as f64 / 6.0);
        thread::sleep(due.saturating_duration_since(Instant::now()));
        got.extend(drain(&mut rd));
        w.pull(&mut src);
        sps = sps.max(w.r.fields().get(&AUDIO_SPS).unwrap_or(0.0));
    }
    thread::sleep(ms(100));
    got.extend(drain(&mut rd));
    assert_eq!(got.len(), 24_000);
    // The decoded tone follows the sent one within the ADPCM error.
    let err = got
        .iter()
        .zip(&tone)
        .skip(100)
        .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
        .max()
        .unwrap();
    assert!(err < 1500, "{err}");
    w.pull(&mut src);
    let f = w.r.fields();
    assert_eq!(f.get(&RESYNC_LOST), Some(0));
    assert!(sps > 6000.0 && sps < 24_000.0, "{sps}");
    assert_eq!(
        (
            f.get(&AUDIO_ADPCM),
            f.get(&AUDIO_SYNC),
            f.get(&WATERFALL_ADPCM)
        ),
        (Some(true), Some(true), Some(true))
    );
    assert!(f.get(&KBIT_S).unwrap() > 0.0);
    src.stop();
}

#[test]
fn a_slow_link_splits_frames_byte_by_byte() {
    let fake = Fake::new();
    let d = drv(&fake);
    let h = spawn_open(&d, &fake.addr(), tune(14_176_700.0, &[]));
    let mut r = fake.accept();
    r.upgrade();
    r.greeting();
    r.hello("1.2.2");
    for b in sframe(1, CONFIG_20M.as_bytes()) {
        r.send(&[b]);
        thread::sleep(Duration::from_micros(1500));
    }
    let mut src = h.join().unwrap().unwrap();
    assert_eq!(r.data().text(), DSP_FIRST);
    src.stop();
}

// ---- Settings while streaming ----

const MODES: &str = r#"{"type":"modes","value":[
 {"modulation":"usb","name":"USB","type":"analog","squelch":false,"bandpass":{"low_cut":300,"high_cut":3000}},
 {"modulation":"am","name":"AM","type":"analog","squelch":true,"bandpass":{"low_cut":-4000,"high_cut":4000}},
 {"modulation":"wfm","name":"WFM","type":"analog","squelch":true},
 {"modulation":"dmr","name":"DMR","type":"digital","requirements":["digital_voice_digiham"],"squelch":false},
 {"modulation":"ft8","name":"FT8","type":"digital","requirements":["wsjt-x"],"underlying":["usb"],"secondaryFft":true},
 {"modulation":"pocsag","name":"Pocsag","type":"digital","requirements":["pocsag"],"underlying":["nfm"]}]}"#;
const FEATURES: &str =
    r#"{"type":"features","value":{"digital_voice_digiham":true,"wsjt-x":true,"pocsag":false}}"#;

#[test]
fn settings_take_effect_live() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_174_000.0, &[audio()]));
    let (_st, _rd) = start(&mut src);
    r.text(MODES.replace('\n', "").as_str());
    r.text(FEATURES);
    thread::sleep(ms(100));
    let mut w = Watch::new();
    assert!(wait_watch(&mut src, &mut w, |w| w.rows(&MODES_TABLE) == 6));
    let set = |v: &[(&str, Value)]| {
        let mut s = vec![audio()];
        let rt: Vec<(&str, &str)> = v
            .iter()
            .filter_map(|(k, x)| match x {
                Value::Text(t) => Some((*k, t.as_str())),
                _ => None,
            })
            .collect();
        s.extend(
            v.iter()
                .filter(|(_, x)| !matches!(x, Value::Text(_)))
                .cloned(),
        );
        with_rt(&d, &fake.addr(), &tune(14_174_000.0, &s), &rt)
    };
    let am = ("servermode", text("am"));
    let sq = ("squelch", Value::F64(-50.0));
    src.apply(&set(std::slice::from_ref(&am))).unwrap();
    assert_eq!(
        r.data().text(),
        r#"{"type":"dspcontrol","params":{"mod":"am","low_cut":-4000,"high_cut":4000}}"#
    );
    src.apply(&set(&[am.clone(), sq.clone()])).unwrap();
    assert_eq!(
        r.data().text(),
        r#"{"type":"dspcontrol","params":{"squelch_level":-50}}"#
    );
    src.apply(&set(&[
        am.clone(),
        sq.clone(),
        ("passband", Value::Hz(6000.0)),
    ]))
    .unwrap();
    assert_eq!(
        r.data().text(),
        r#"{"type":"dspcontrol","params":{"low_cut":-3000,"high_cut":3000}}"#
    );
    let ft8 = ("secondary", text("ft8"));
    src.apply(&set(std::slice::from_ref(&ft8))).unwrap();
    assert_eq!(
        r.data().text(),
        r#"{"type":"dspcontrol","params":{"mod":"usb","low_cut":300,"squelch_level":-150,"secondary_mod":"ft8"}}"#
    );
    src.apply(&set(&[
        ft8.clone(),
        ("secondary_offset", Value::Hz(1500.0)),
    ]))
    .unwrap();
    assert_eq!(
        r.data().text(),
        r#"{"type":"dspcontrol","params":{"secondary_offset_freq":1500}}"#
    );
    src.apply(&set(&[])).unwrap();
    assert_eq!(
        r.data().text(),
        r#"{"type":"dspcontrol","params":{"secondary_mod":false}}"#
    );
    // A decoder the server cannot run is refused and not sent.
    assert_eq!(
        src.apply(&set(&[("secondary", text("pocsag"))])).err(),
        Some(refused(RunTime::Secondary))
    );
    assert!(r.quiet(ms(150)));
    let dmr = ("servermode", text("dmr"));
    src.apply(&set(std::slice::from_ref(&dmr))).unwrap();
    let p = r.data().params();
    assert_eq!(
        (p.modulation.as_deref(), p.dmr_filter),
        (Some("dmr"), Some(3))
    );
    src.apply(&set(&[dmr.clone(), ("dmr_slot", choice(DmrSlot::One))]))
        .unwrap();
    assert_eq!(
        r.data().text(),
        r#"{"type":"dspcontrol","params":{"dmr_filter":1}}"#
    );
    // The modes of the server are a table.
    assert!(wait_watch(&mut src, &mut w, |w| w.rows(&MODES_TABLE) == 6));
    let avail = |w: &Watch, id: &str| w.row(&MODES_TABLE, id).and_then(|m| m.get(&M_AVAILABLE));
    assert_eq!(
        (avail(&w, "usb"), avail(&w, "dmr"), avail(&w, "pocsag")),
        (Some(true), Some(true), Some(false))
    );
    assert_eq!(
        w.row(&MODES_TABLE, "ft8").unwrap().get(&M_UNDERLYING),
        Some("usb")
    );
    // A demodulator error marks the mode not available.
    r.text(r#"{"type":"demodulator_error","value":"dmr decoding failed"}"#);
    assert!(wait_watch(&mut src, &mut w, |w| avail(w, "dmr") == Some(false)));
    assert_eq!(w.r.fields().get(&SERVER_ERROR), Some("dmr decoding failed"));
    // The delivery and the login take a restart.
    assert!(matches!(
        src.apply(&tune(14_174_000.0, &[])),
        Ok(Retuned::NeedsRestart)
    ));
    src.stop();
}

use crate::MODES as MODES_TABLE;

#[test]
fn a_decoder_needing_another_demodulator_is_refused_in_iq() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let (_st, _rd) = start(&mut src);
    r.text(MODES.replace('\n', "").as_str());
    r.text(
        r#"{"type":"features","value":{"digital_voice_digiham":true,"wsjt-x":true,"pocsag":true}}"#,
    );
    let mut w = Watch::new();
    assert!(wait_watch(&mut src, &mut w, |w| w.rows(&MODES_TABLE) == 6));
    let decoder = |id: &str| {
        with_rt(
            &d,
            &fake.addr(),
            &tune(14_176_700.0, &[]),
            &[("secondary", id)],
        )
    };
    assert_eq!(
        src.apply(&decoder("pocsag")).err(),
        Some(refused(RunTime::Secondary))
    );
    assert!(r.quiet(ms(150)));
    src.apply(&decoder("ft8")).unwrap();
    assert_eq!(
        r.data().text(),
        r#"{"type":"dspcontrol","params":{"secondary_mod":"ft8"}}"#
    );
    src.stop();
}

#[test]
fn choices_the_server_does_not_offer_are_refused() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_174_000.0, &[audio()]));
    let (_st, _rd) = start(&mut src);
    r.text(MODES.replace('\n', "").as_str());
    r.text(FEATURES);
    r.text(
        r#"{"type":"bookmarks","value":[{"name":"Net","frequency":14300000,"modulation":"lsb"}]}"#,
    );
    let mut w = Watch::new();
    assert!(wait_watch(&mut src, &mut w, |w| w.rows(&BOOKMARKS) == 1
        && w.rows(&MODES_TABLE) == 6));
    // In the `audio` delivery the bookmark is the frequency to tune.
    assert_eq!(
        w.row(&BOOKMARKS, "14300000|Net").unwrap().get(&B_TUNE),
        Some(14_300_000.0)
    );
    // A demodulator that failed on the server, and a decoder whose requirement is missing.
    r.text(r#"{"type":"demodulator_error","value":"dmr decoding failed"}"#);
    let avail = |w: &Watch, id: &str| w.row(&MODES_TABLE, id).and_then(|m| m.get(&M_AVAILABLE));
    assert!(wait_watch(&mut src, &mut w, |w| avail(w, "dmr") == Some(false)));
    let one =
        |k: &str, c: &str| with_rt(&d, &fake.addr(), &tune(14_174_000.0, &[audio()]), &[(k, c)]);
    assert_eq!(
        src.apply(&one("servermode", "dmr")).err(),
        Some(refused(RunTime::ServerMode))
    );
    assert_eq!(
        src.apply(&one("secondary", "pocsag")).err(),
        Some(refused(RunTime::Secondary))
    );
    assert!(r.quiet(ms(150)));
    src.stop();
    // What the driver learnt refuses a later open before it connects.
    let e = d.open(&fake.addr(), &one("servermode", "dmr")).err();
    assert_eq!(e, Some(refused(RunTime::ServerMode)));
    assert!(fake.try_accept(ms(100)).is_none());
}

#[test]
fn a_rate_change_starts_a_new_throughput_window() {
    let fake = Fake::new();
    let d = drv(&fake);
    let (mut src, mut r, _) = open_std(&fake, &d, tune(14_174_000.0, &[audio()]));
    let (_st, _rd) = start(&mut src);
    // 3 x 1200 narrow samples and then the first HD frame, in one write: they come inside one window.
    let mut narrow = vec![0x02, 0x53, 0x59, 0x4e, 0x43, 0, 0, 0, 0];
    narrow.extend(vec![0x11u8; 600]);
    let mut burst = Vec::new();
    for _ in 0..3 {
        burst.extend(sframe(0x2, &narrow));
    }
    let mut hd = vec![0x04, 0x53, 0x59, 0x4e, 0x43, 0, 0, 0, 0];
    hd.extend(vec![0x11u8; 100]);
    burst.extend(sframe(0x2, &hd));
    r.send(&burst);
    let mut w = Watch::new();
    assert!(wait_watch(&mut src, &mut w, |w| w.restarts > 0));
    // The rate change starts a new window, so no window counts the narrow samples (3600 in 0.2 s would be 18 000 a second).
    let end = Instant::now() + ms(800);
    let mut most = 0f64;
    while Instant::now() < end {
        w.pull(&mut src);
        most = most.max(w.r.fields().get(&AUDIO_SPS).unwrap_or(0.0));
        thread::sleep(ms(10));
    }
    assert!(most < 5_000.0, "{most}");
    src.stop();
}

#[test]
fn hd_audio_changes_the_rate() {
    let fake = Fake::new();
    let d = drv(&fake);
    learn_modes(&fake, &d);
    let wfm = with_rt(
        &d,
        &fake.addr(),
        &tune(14_100_000.0, &[audio()]),
        &[("servermode", "wfm")],
    );
    let (mut src, mut r, first) = open_std(&fake, &d, wfm);
    assert_eq!(first.params().low_cut, Some(-75_000));
    let (st, mut rd) = start(&mut src);
    assert_eq!(st.rate_hz, 48_000.0);
    let mut w = Watch::new();
    w.pull(&mut src);
    r.bin(&[0x02, 0x53, 0x59, 0x4e, 0x43, 0, 0, 0, 0, 0x11]);
    assert!(wait_watch(&mut src, &mut w, |w| w.restarts == 1));
    assert_eq!(src.capabilities().rates.max_hz, 12_000.0);
    r.bin(&[
        0x04, 0x53, 0x59, 0x4e, 0x43, 0x14, 0x00, 0xd2, 0x04, 0x55, 0xb3,
    ]);
    assert!(wait_watch(&mut src, &mut w, |w| w.restarts == 2));
    assert_eq!(src.capabilities().rates.max_hz, 48_000.0);
    assert!(w.has("the server now sends audio at 48 kHz"));
    // The HD stream decodes with its own fresh state.
    let got = take(&mut rd, 6);
    assert_eq!(&got[2..6], [1302, 1402, 1494, 1410]);
    src.stop();
}

// ---- TLS (wss) ----

fn ca_der() -> Vec<u8> {
    use rustls::pki_types::pem::PemObject;
    rustls::pki_types::CertificateDer::from_pem_slice(fake::CA)
        .unwrap()
        .to_vec()
}

#[test]
fn wss_with_a_trusted_root_and_an_untrusted_certificate() {
    let net = FakeTransport::new();
    let srv = fake::tls_server(&net, 9443);
    let tls: Arc<dyn Transport> = Arc::new(
        RustlsTransport::new(Arc::new(net.clone()), &Trust::Roots(vec![ca_der()])).unwrap(),
    );
    let d = OpenWebRxDriver::with_timing(tls, Clock::fixed(T0), fast());
    let mut src = d
        .open("wss://127.0.0.1:9443/", &tune(14_176_700.0, &[]))
        .unwrap();
    src.stop();
    let got = srv
        .recv_timeout(5 * S)
        .unwrap()
        .expect("the TLS script ran");
    assert!(got[0].starts_with("GET /ws/ HTTP/1.1\r\n"));
    assert!(got[0].contains("Origin: https://127.0.0.1:9443\r\n"));
    assert_eq!(got[1], proto::GREETING);
    assert_eq!(got[3], DSP_FIRST);
    assert_eq!(got[4], proto::DSP_START);
    assert_eq!(got[5], "8");

    // Another trust refuses the certificate.
    let pinned: Arc<dyn Transport> =
        Arc::new(RustlsTransport::new(Arc::new(net.clone()), &Trust::Pinned([0; 32])).unwrap());
    let d = OpenWebRxDriver::with_timing(pinned, Clock::fixed(T0), fast());
    let e = d
        .open("wss://127.0.0.1:9443/", &tune(1e6, &[]))
        .err()
        .unwrap();
    assert_eq!(e, DriverError::Net(NetError::Untrusted));
    // Without TLS in the transport, wss is not possible.
    let d = OpenWebRxDriver::with_timing(Arc::new(net), Clock::fixed(T0), fast());
    let e = d
        .open("wss://127.0.0.1:9443/", &tune(1e6, &[]))
        .err()
        .unwrap();
    assert_eq!(e, DriverError::Net(NetError::TlsUnavailable));
}

#[test]
fn bad_settings_are_refused() {
    // The passband runs from 0 to 200 kHz: the schema refuses 300 kHz.
    let k = kind();
    let mut v = Values::defaults(k.schema());
    assert!(v
        .set(k.schema().id("passband").unwrap(), &Value::Hz(300_000.0))
        .is_err());
    // Settings the driver cannot follow are refused before a connect.
    let fake = Fake::new();
    let d = drv(&fake);
    let e = d.open(
        &fake.addr(),
        &tune(14e6, &[("bias_tee", Value::Bool(true))]),
    );
    assert_eq!(e.err(), Some(refused_std(RadioSetting::BiasTee)));
    let exact = tune(
        14e6,
        &[
            ("rate", Value::Hz(48_000.0)),
            ("rate_exact", Value::Bool(true)),
        ],
    );
    assert_eq!(
        d.open(&fake.addr(), &exact).err(),
        Some(refused_std(RadioSetting::Rate))
    );
    assert!(fake.try_accept(ms(100)).is_none());
    assert_eq!(
        d.open("h:99999", &tune(14e6, &[])).err(),
        Some(DriverError::Address)
    );
}

#[test]
fn an_open_radio_publishes_what_the_server_reports() {
    let fake = Fake::new();
    let k = DriverKind::new(Box::new(drv(&fake)));
    let addr = fake.addr();
    let v = tune(14_176_700.0, &[]);
    let h = thread::spawn(move || k.open(&addr, &v).map(|r| (k, r)));
    let mut r = fake.accept();
    r.standard();
    let (_k, mut radio) = h.join().unwrap().expect("open");
    let (w, _rd) = ring(1 << 16, radio.sample_type());
    radio.start(w).unwrap();
    let mut events = Vec::new();
    let end = Instant::now() + S;
    while radio.results().fields().get(&PROFILE_NAME) != Some("RTL 20m") && Instant::now() < end {
        thread::sleep(ms(10));
        radio.poll(T0, &mut events);
    }
    let res = radio.results();
    assert_eq!(res.fields().get(&PROFILE_NAME), Some("RTL 20m"));
    let id = res.decl().table_id(PROFILES.key).unwrap();
    assert_eq!(res.table(id).unwrap().len(), 2);
    radio.stop();
}

#[test]
fn the_learnt_bands_are_saved_state() {
    let fake = Fake::new();
    let d = drv(&fake);
    learn_40m(&fake, &d);
    let (mut src, _r, _) = open_std(&fake, &d, tune(14_176_700.0, &[]));
    let saved = src.save_state().expect("state");
    src.stop();
    // A driver that never saw the server learns the bands from the state.
    let other = Fake::new();
    let d2 = drv(&other);
    let (mut src, _r, _) = open_std(&other, &d2, tune(14_176_700.0, &[]));
    src.restore_state(&saved).unwrap();
    let key = conn::parse_address(&other.addr()).unwrap().key();
    let st = d2.addr_state(&key);
    let v = lock(&st.view);
    assert_eq!(
        v.bands.get("rtl|40m").map(|b| (b.lo(), b.hi())),
        Some((6_975_000.0, 7_225_000.0))
    );
    assert!(v.profiles.iter().any(|(id, _)| id == "rtl|40m"));
    drop(v);
    let wrong = StateBlob::new("kiwisdr", 1, Vec::new());
    assert!(matches!(
        src.restore_state(&wrong),
        Err(sdrkit_core::StateError::WrongOwner { .. })
    ));
    src.stop();
}
