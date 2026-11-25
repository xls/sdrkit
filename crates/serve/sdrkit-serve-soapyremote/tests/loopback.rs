//! The SoapyRemote server on a hub, on a fake network, against the SoapyRemote client driver: bit-exact CS16 and CF32 with
//! no loss, settings as requests, a refusal as the client's error, transmit refused and delivered, a gap flagged, and the
//! discovery answer.

use sdrkit_core::net::{FakeTransport, Transport};
use sdrkit_core::{Choice, Complex32, Owner, Results, SettingSet, Value, Values};
use sdrkit_driver_core::{
    ring, tx_schema, DriverError, DriverKind, RingReader, Source, SourceDriver, TxLimits,
};
use sdrkit_driver_soapyremote::discovery::{ssdp_search, SSDP_V4};
use sdrkit_driver_soapyremote::{SoapyRemoteDriver, SoapyTxSetting, WireChoice, OVERFLOWS};
use sdrkit_serve_core::{
    Answer, Block, ClientHandle, Config, Control, Feeder, Hub, Listen, Request, SampleType,
    TxReader, TxRequest,
};
use sdrkit_serve_soapyremote::{Device, Kwargs, Range, Side, SoapyRemote, Ssdp, DEFAULT_PORT};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const CENTRE: f64 = 100e6;
const RATE: f64 = 2.4e6;

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

fn device() -> Device {
    let mut info = Kwargs::new();
    info.insert("serial".into(), "0001".into());
    Device {
        driver: "sdrkit".into(),
        hardware: "test radio".into(),
        info,
        rx: Side {
            freqs: vec![Range::new(24e6, 1.766e9, 0.0)],
            rates: vec![Range::new(0.25e6, 3.2e6, 0.0)],
            gain: Range::new(0.0, 49.6, 0.0),
            elements: vec![("TUNER".into(), Range::new(0.0, 49.6, 0.0))],
            bandwidths: Vec::new(),
            antennas: vec!["RX".into()],
        },
        tx: Some(Side {
            freqs: vec![Range::new(70e6, 6e9, 0.0)],
            rates: vec![Range::new(0.5e6, 20e6, 0.0)],
            gain: Range::new(-89.75, 0.0, 0.25),
            elements: vec![("PGA".into(), Range::new(-89.75, 0.0, 0.25))],
            bandwidths: Vec::new(),
            antennas: vec!["TX".into()],
        }),
        bias_tee: true,
    }
}

/// A control that records requests, refuses frequencies above 1 GHz, and accepts transmit when `transmit` is set.
#[derive(Default)]
struct Owner2 {
    seen: Mutex<Vec<Request>>,
    transmit: AtomicBool,
    tx: Mutex<Option<TxReader>>,
}

impl Control for Owner2 {
    fn request(&self, _client: &ClientHandle, request: &Request) -> Answer {
        self.seen.lock().unwrap().push(*request);
        match request {
            Request::Frequency { hz } if *hz > 1e9 => Answer::Refused,
            _ => Answer::Accepted,
        }
    }
    fn transmit(&self, _client: &ClientHandle, _request: &TxRequest, reader: TxReader) -> Answer {
        if !self.transmit.load(Ordering::Relaxed) {
            return Answer::Refused;
        }
        *self.tx.lock().unwrap() = Some(reader);
        Answer::Accepted
    }
}

struct Rig {
    hub: Hub,
    owner: Arc<Owner2>,
    driver: SoapyRemoteDriver,
}

fn rig(format: SampleType) -> (Rig, Feeder) {
    let net: Arc<dyn Transport> = Arc::new(FakeTransport::new());
    let (hub, feeder) = Hub::new(
        Config {
            block_bytes: 8 * 8192,
            ..Config::default()
        },
        &block(format, 0),
    )
    .unwrap();
    let owner = Arc::new(Owner2::default());
    let listen = Listen {
        host: "127.0.0.1".into(),
        port: DEFAULT_PORT,
        allow: Vec::new(),
    };
    hub.listen(
        &net,
        &listen,
        Arc::new(SoapyRemote::new(net.clone(), device(), "uuid-1")),
        owner.clone(),
    )
    .unwrap();
    let driver = SoapyRemoteDriver::new(net.clone());
    (Rig { hub, owner, driver }, feeder)
}

fn block(format: SampleType, lost: u64) -> Block {
    Block {
        format,
        centre_hz: CENTRE,
        rate_hz: RATE,
        lost,
    }
}

impl Rig {
    fn values(&self, set: &[(&str, Value)]) -> Values {
        let kind = DriverKind::new(Box::new(self.driver.clone()));
        let schema = kind.schema();
        let mut v = Values::defaults(schema);
        let base = [
            ("centre_hz", Value::Hz(CENTRE)),
            ("rate", Value::Hz(RATE)),
            ("gain", Value::F64(30.0)),
            ("agc", Value::Bool(false)),
        ];
        for (key, value) in base.iter().chain(set) {
            v.set(schema.id(key).unwrap(), value).unwrap();
        }
        v
    }

    fn open(&self, set: &[(&str, Value)]) -> (Box<dyn Source>, RingReader) {
        let mut src = self
            .driver
            .open(&format!("127.0.0.1:{DEFAULT_PORT}"), &self.values(set))
            .unwrap();
        let (w, r) = ring(1 << 24, src.sample_type());
        src.start(w).unwrap();
        (src, r)
    }
}

/// Sample `k` of the source as bytes: Cs16 `(k, k >> 16 ^ 0x5555)`, Cf32 `(k, -k)`.
fn sample(format: SampleType, k: u64) -> Vec<u8> {
    match format {
        SampleType::Cs16 => {
            let (i, q) = (k as u16, ((k >> 16) as u16) ^ 0x5555);
            [i.to_le_bytes(), q.to_le_bytes()].concat()
        }
        _ => [(k as f32).to_le_bytes(), (-(k as f32)).to_le_bytes()].concat(),
    }
}

/// The sample index whose bytes are `b`.
fn index_of(format: SampleType, b: &[u8]) -> u64 {
    match format {
        SampleType::Cs16 => {
            let i = u16::from_le_bytes([b[0], b[1]]);
            let q = u16::from_le_bytes([b[2], b[3]]) ^ 0x5555;
            u64::from(i) | u64::from(q) << 16
        }
        _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as u64,
    }
}

/// Pushes paced blocks of 8192 samples until `stop`; a block at `gap_at` reports `lost` samples before it.
fn produce(
    mut feeder: Feeder,
    format: SampleType,
    stop: Arc<AtomicBool>,
    gap_at: Option<(u64, u64)>,
    pushed: Arc<AtomicU64>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let per = 8192u64;
        let start = Instant::now();
        let mut k = 0u64;
        let mut blocks = 0u64;
        while !stop.load(Ordering::Relaxed) {
            let due = Duration::from_secs_f64(k as f64 / RATE);
            if let Some(w) = due.checked_sub(start.elapsed()) {
                std::thread::sleep(w);
            }
            let lost = gap_at.filter(|g| g.0 == blocks).map_or(0, |g| g.1);
            k += lost;
            let bytes: Vec<u8> = (k..k + per).flat_map(|n| sample(format, n)).collect();
            feeder.push(&bytes, &block(format, lost));
            k += per;
            blocks += 1;
            pushed.store(k, Ordering::Relaxed);
        }
    })
}

/// Opens the client with `format`, streams about a second, and checks that every sample after the first arrives in order.
fn bit_exact(source: SampleType, wire: WireChoice) {
    let (rig, feeder) = rig(source);
    let stop = Arc::new(AtomicBool::new(false));
    let pushed = Arc::new(AtomicU64::new(0));
    let p = produce(feeder, source, stop.clone(), None, pushed.clone());
    let (mut src, mut r) = rig.open(&[
        ("format", Value::Choice(wire.index())),
        ("serverlog", Value::Bool(false)),
    ]);
    assert_eq!(src.sample_type(), source);
    let ss = if source == SampleType::Cs16 { 4 } else { 8 };
    let mut got = Vec::new();
    let t = Instant::now();
    while got.len() < ss * 2_000_000 && t.elapsed() < Duration::from_secs(5) {
        r.wait(1, Duration::from_millis(20));
        r.read_with(1 << 20, |b| got.extend_from_slice(b));
    }
    stop.store(true, Ordering::Relaxed);
    p.join().unwrap();
    assert!(
        got.len() >= ss * 2_000_000,
        "{} samples arrived",
        got.len() / ss
    );
    let k0 = index_of(source, &got[..ss]);
    let bad = got
        .chunks_exact(ss)
        .enumerate()
        .find(|(i, b)| *b != sample(source, k0 + *i as u64).as_slice())
        .map(|(i, _)| i);
    assert_eq!(bad, None, "samples arrive in order and unchanged from {k0}");
    let c = rig
        .hub
        .snapshot()
        .clients
        .into_iter()
        .find(|c| c.wire.is_some())
        .unwrap();
    assert_eq!((c.dropped_samples, c.wire), (0, Some(source)));
    let seen = rig.owner.seen.lock().unwrap().clone();
    assert!(seen.contains(&Request::Rate { hz: RATE }), "{seen:?}");
    assert!(
        seen.contains(&Request::Frequency { hz: CENTRE }),
        "{seen:?}"
    );
    assert!(seen.contains(&Request::Agc { on: false }), "{seen:?}");
    assert!(
        seen.iter().any(|r| matches!(r, Request::Gain { .. })),
        "{seen:?}"
    );
    src.stop();
}

#[test]
fn cs16_arrives_bit_exact_with_its_settings() {
    bit_exact(SampleType::Cs16, WireChoice::Cs16);
}

#[test]
fn cf32_arrives_bit_exact() {
    bit_exact(SampleType::Cf32, WireChoice::Cf32);
}

#[test]
fn a_refused_setting_is_the_clients_error_and_a_gap_is_flagged() {
    let (rig, feeder) = rig(SampleType::Cs16);
    let stop = Arc::new(AtomicBool::new(false));
    let pushed = Arc::new(AtomicU64::new(0));
    let p = produce(
        feeder,
        SampleType::Cs16,
        stop.clone(),
        Some((60, 5000)),
        pushed.clone(),
    );
    let (mut src, mut r) = rig.open(&[("serverlog", Value::Bool(false))]);
    let far = rig.values(&[
        ("serverlog", Value::Bool(false)),
        ("centre_hz", Value::Hz(1.2e9)),
    ]);
    let e = src.apply(&far).err();
    assert!(matches!(e, Some(DriverError::Refused { .. })), "{e:?}");
    let decl = SourceDriver::descriptor(&rig.driver)
        .decl()
        .results()
        .clone();
    let mut res = Results::new(decl);
    let flagged = wait_for(
        || {
            r.read_with(1 << 22, |_| {});
            src.poll(&mut res);
            res.fields().get(&OVERFLOWS).is_some_and(|n| n >= 1)
        },
        5.0,
    );
    stop.store(true, Ordering::Relaxed);
    p.join().unwrap();
    assert!(flagged, "the client counts the gap as an overflow");
    assert!(rig
        .hub
        .snapshot()
        .clients
        .iter()
        .any(|c| c.refused_requests == 1));
    src.stop();
}

fn tx_values(src: &dyn Source) -> Values {
    let limits: TxLimits = src
        .capabilities()
        .transmit
        .clone()
        .expect("transmit limits");
    let schema = tx_schema(SoapyTxSetting::SETTINGS, &limits);
    let mut v = Values::defaults(&schema);
    for (key, value) in [
        ("tx_frequency", Value::Hz(433.92e6)),
        ("tx_rate", Value::Hz(1e6)),
        ("tx_power", Value::F64(-10.0)),
        ("txoutput", Value::I64(1)),
    ] {
        v.set(schema.id(key).unwrap(), &value).unwrap();
    }
    v
}

#[test]
fn transmit_is_refused_unless_the_owner_accepts_and_then_arrives_in_order() {
    let (rig, feeder) = rig(SampleType::Cs16);
    let stop = Arc::new(AtomicBool::new(false));
    let p = produce(
        feeder,
        SampleType::Cs16,
        stop.clone(),
        None,
        Arc::new(AtomicU64::new(0)),
    );
    let (mut src, _r) = rig.open(&[("serverlog", Value::Bool(false))]);
    let tv = tx_values(src.as_ref());
    let mut tx = src.transmitter(&tv).unwrap();
    tx.configure(&tv).unwrap();
    assert!(tx.arm().is_err(), "the owner refuses");
    rig.owner.transmit.store(true, Ordering::Relaxed);
    tx.arm().unwrap();
    let iq: Vec<Complex32> = (0..2000)
        .map(|k| Complex32::new(k as f32 / 4000.0, -(k as f32) / 4000.0))
        .collect();
    assert_eq!(tx.push(&iq, Duration::from_secs(2)).unwrap(), 2000);
    tx.flush().unwrap();
    let mut got = Vec::new();
    assert!(wait_for(
        || {
            if let Some(r) = rig.owner.tx.lock().unwrap().as_mut() {
                let mut b = vec![0u8; 1 << 16];
                let n = r.read(&mut b);
                got.extend_from_slice(&b[..n]);
            }
            got.len() >= 2000 * 4
        },
        5.0
    ));
    let want: Vec<u8> = iq
        .iter()
        .flat_map(|c| {
            let q = |x: f32| ((x * 32767.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes();
            [q(c.re), q(c.im)].concat()
        })
        .collect();
    assert_eq!(
        &got[..want.len()],
        &want[..],
        "transmit samples arrive in order"
    );
    let r = rig.owner.tx.lock().unwrap();
    assert_eq!(r.as_ref().unwrap().request().format, SampleType::Cs16);
    assert_eq!(r.as_ref().unwrap().request().rate_hz, 1e6);
    drop(r);
    tx.disarm();
    stop.store(true, Ordering::Relaxed);
    p.join().unwrap();
    src.stop();
}

#[test]
fn a_search_finds_the_server() {
    let net: Arc<dyn Transport> = Arc::new(FakeTransport::new());
    let _ssdp = Ssdp::start(&net, "uuid-7", "127.0.0.1", DEFAULT_PORT).unwrap();
    let found = ssdp_search(net.as_ref(), &[SSDP_V4], Duration::from_millis(400)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        (found[0].uuid.as_str(), found[0].port),
        ("uuid-7", DEFAULT_PORT)
    );
}

/// The network of the system with the loopback interface as its only interface, so that a search stays on this machine.
struct LoopbackOnly(sdrkit_core::net::StdNet);

fn loopback() -> sdrkit_core::net::Interface {
    sdrkit_core::net::Interface {
        name: "loopback".into(),
        index: 0,
        v4: Some(sdrkit_core::net::Ipv4Net {
            addr: std::net::Ipv4Addr::LOCALHOST,
            netmask: std::net::Ipv4Addr::new(255, 0, 0, 0),
        }),
        v6: false,
    }
}

impl Transport for LoopbackOnly {
    fn connect(
        &self,
        host: &str,
        port: u16,
        tls: bool,
        timeout: Duration,
    ) -> Result<Box<dyn sdrkit_core::net::Conn>, sdrkit_core::net::NetError> {
        self.0.connect(host, port, tls, timeout)
    }
    fn datagram(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Box<dyn sdrkit_core::net::Datagram>, sdrkit_core::net::NetError> {
        self.0.datagram(host, port)
    }
    fn bind(
        &self,
        options: &sdrkit_core::net::DatagramOptions,
    ) -> Result<Box<dyn sdrkit_core::net::Datagram>, sdrkit_core::net::NetError> {
        self.0.bind(options)
    }
    fn listen(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Box<dyn sdrkit_core::net::Listener>, sdrkit_core::net::NetError> {
        self.0.listen(host, port)
    }
    fn resolve(
        &self,
        host: &str,
        port: u16,
    ) -> Result<std::net::SocketAddr, sdrkit_core::net::NetError> {
        self.0.resolve(host, port)
    }
    fn interfaces(&self) -> Vec<sdrkit_core::net::Interface> {
        vec![loopback()]
    }
}

/// A search over real sockets on the loopback interface finds the server: the server's socket is a member of the group.
/// The port is one that the system chose, so the test meets no other SSDP program.
#[test]
fn a_search_over_real_loopback_sockets_finds_the_server() {
    let net: Arc<dyn Transport> = Arc::new(LoopbackOnly(sdrkit_core::net::StdNet));
    let port = {
        let probe = net
            .bind(&sdrkit_core::net::DatagramOptions::at(
                "127.0.0.1:0".parse().unwrap(),
            ))
            .unwrap();
        probe.local_addr().port()
    };
    let group = std::net::SocketAddr::new(SSDP_V4.ip(), port);
    let _ssdp = Ssdp::start_on(
        &net,
        "uuid-9",
        "127.0.0.1",
        DEFAULT_PORT,
        group,
        Some(loopback()),
    )
    .unwrap();
    let found = ssdp_search(net.as_ref(), &[group], Duration::from_millis(600)).unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        (found[0].uuid.as_str(), found[0].port),
        ("uuid-9", DEFAULT_PORT)
    );
}
