//! The SoapyRemote server on a hub, on a fake network, against the SoapyRemote client driver: bit-exact CS16 and CF32 with
//! no loss over UDP and TCP (also on real loopback sockets), settings as requests, a refusal as the client's error, transmit
//! refused and delivered, a gap flagged, and the discovery answer. A bare TCP client checks back-pressure, the drop of a
//! stalled client, and a stream closed and opened again on one connection.

use sdrkit_core::net::{FakeTransport, Transport};
use sdrkit_core::{Choice, Complex32, Owner, Results, SettingSet, Value, Values};
use sdrkit_driver_core::{
    ring, tx_schema, DriverError, DriverKind, RingReader, Source, SourceDriver, TxLimits,
};
use sdrkit_driver_soapyremote::discovery::{ssdp_search, SSDP_V4};
use sdrkit_driver_soapyremote::rpc::{self, call, Packer, RX};
use sdrkit_driver_soapyremote::stream::{ack, ack_trigger, DgHeader, HEADER_LEN, OVERFLOW};
use sdrkit_driver_soapyremote::wire::Wire;
use sdrkit_driver_soapyremote::{
    SoapyRemoteDriver, SoapyTxSetting, StreamProt, WireChoice, OVERFLOWS,
};
use sdrkit_serve_core::{
    Answer, Block, ClientHandle, Config, Control, Event, Feeder, Hub, Leave, Listen, Request,
    SampleType, TxReader, TxRequest,
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
            agc: true,
            correction: true,
            bandwidths: Vec::new(),
            antennas: vec!["RX".into()],
        },
        tx: Some(Side {
            freqs: vec![Range::new(70e6, 6e9, 0.0)],
            rates: vec![Range::new(0.5e6, 20e6, 0.0)],
            gain: Range::new(-89.75, 0.0, 0.25),
            elements: vec![("PGA".into(), Range::new(-89.75, 0.0, 0.25))],
            agc: false,
            correction: false,
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
    net: Arc<dyn Transport>,
    port: u16,
}

fn config() -> Config {
    Config {
        block_bytes: 8 * 8192,
        ..Config::default()
    }
}

fn rig(format: SampleType) -> (Rig, Feeder) {
    rig_on(
        Arc::new(FakeTransport::new()),
        format,
        config(),
        DEFAULT_PORT,
    )
}

/// A hub on `net` serving SoapyRemote at `127.0.0.1:port` (0: a port the system chooses); TCP streams listen there too.
fn rig_on(net: Arc<dyn Transport>, format: SampleType, config: Config, port: u16) -> (Rig, Feeder) {
    rig_of(net, device(), format, config, port)
}

/// A hub on `net` serving `device` with SoapyRemote at `127.0.0.1:port`.
fn rig_of(
    net: Arc<dyn Transport>,
    device: Device,
    format: SampleType,
    config: Config,
    port: u16,
) -> (Rig, Feeder) {
    let (hub, feeder) = Hub::new(config, &block(format, 0)).unwrap();
    let owner = Arc::new(Owner2::default());
    let listen = Listen {
        host: "127.0.0.1".into(),
        port,
        allow: Vec::new(),
    };
    let server = SoapyRemote::new(net.clone(), device, "uuid-1").with_stream_host("127.0.0.1");
    let info = hub
        .listen(&net, &listen, Arc::new(server), owner.clone())
        .unwrap();
    let driver = SoapyRemoteDriver::new(net.clone());
    let rig = Rig {
        hub,
        owner,
        driver,
        net,
        port: info.addr.port(),
    };
    (rig, feeder)
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
            .open(&format!("127.0.0.1:{}", self.port), &self.values(set))
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

/// Opens the client with `format` over `prot`, streams about a second, and checks that every sample after the first
/// arrives in order.
fn bit_exact(source: SampleType, wire: WireChoice, prot: StreamProt) {
    let (rig, feeder) = rig(source);
    bit_exact_on(&rig, feeder, source, wire, prot);
}

fn bit_exact_on(rig: &Rig, feeder: Feeder, source: SampleType, wire: WireChoice, prot: StreamProt) {
    let stop = Arc::new(AtomicBool::new(false));
    let pushed = Arc::new(AtomicU64::new(0));
    let p = produce(feeder, source, stop.clone(), None, pushed.clone());
    let (mut src, mut r) = rig.open(&[
        ("format", Value::Choice(wire.index())),
        ("transport", Value::Choice(prot.index())),
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
    bit_exact(SampleType::Cs16, WireChoice::Cs16, StreamProt::Udp);
}

#[test]
fn cf32_arrives_bit_exact() {
    bit_exact(SampleType::Cf32, WireChoice::Cf32, StreamProt::Udp);
}

#[test]
fn cs16_over_tcp_arrives_bit_exact_with_its_settings() {
    bit_exact(SampleType::Cs16, WireChoice::Cs16, StreamProt::Tcp);
}

#[test]
fn cf32_over_tcp_arrives_bit_exact() {
    bit_exact(SampleType::Cf32, WireChoice::Cf32, StreamProt::Tcp);
}

#[test]
fn cs16_over_tcp_on_real_loopback_sockets_arrives_bit_exact() {
    let net: Arc<dyn Transport> = Arc::new(LoopbackOnly(sdrkit_core::net::StdNet));
    let (rig, feeder) = rig_on(net, SampleType::Cs16, config(), 0);
    bit_exact_on(
        &rig,
        feeder,
        SampleType::Cs16,
        WireChoice::Cs16,
        StreamProt::Tcp,
    );
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

/// A radio with three gain stages and no automatic gain, no frequency correction and no filter setting.
fn staged_device() -> Device {
    let mut d = device();
    d.rx.gain = Range::new(0.0, 116.0, 0.0);
    d.rx.elements = vec![
        ("LNA".into(), Range::new(0.0, 40.0, 8.0)),
        ("VGA".into(), Range::new(0.0, 62.0, 2.0)),
        ("AMP".into(), Range::new(0.0, 14.0, 14.0)),
    ];
    d.rx.agc = false;
    d.rx.correction = false;
    d
}

/// Our client opens a radio without automatic gain over UDP and over TCP: the server says it has no gain mode, the client
/// asks for none, and nothing for a setting that the radio lacks reaches the owner. A bare client that sets them anyway
/// gets no error and reads back manual gain; an overall gain spreads over the stages in order.
#[test]
fn a_radio_without_automatic_gain_opens_and_spreads_the_overall_gain() {
    for prot in [StreamProt::Udp, StreamProt::Tcp] {
        let (rig, feeder) = rig_of(
            Arc::new(FakeTransport::new()),
            staged_device(),
            SampleType::Cs16,
            config(),
            DEFAULT_PORT,
        );
        let stop = Arc::new(AtomicBool::new(false));
        let p = produce(
            feeder,
            SampleType::Cs16,
            stop.clone(),
            None,
            Arc::new(AtomicU64::new(0)),
        );
        let (mut src, _r) = rig.open(&[
            ("serverlog", Value::Bool(false)),
            ("transport", Value::Choice(prot.index())),
            ("ppm", Value::F64(1.5)),
        ]);
        assert!(!src.capabilities().has_agc);
        let seen = rig.owner.seen.lock().unwrap().clone();
        assert!(
            seen.iter()
                .all(|r| !matches!(r, Request::Agc { .. } | Request::Correction { .. })),
            "{seen:?}"
        );
        assert!(seen.contains(&Request::Gain { db: 30.0 }), "{seen:?}");
        src.stop();
        stop.store(true, Ordering::Relaxed);
        p.join().unwrap();
    }

    let (rig, _feeder) = rig_of(
        Arc::new(FakeTransport::new()),
        staged_device(),
        SampleType::Cs16,
        config(),
        DEFAULT_PORT,
    );
    let mut b = Bare::connect(&rig);
    let rx = |n: u32| {
        let mut p = Packer::call(n);
        p.dir_ch(RX, 0);
        p
    };
    let ask_bool = |b: &mut Bare, n: u32| b.ctl.call(&rx(n), WAIT, |u| u.bool()).unwrap();
    assert!(!ask_bool(&mut b, call::HAS_GAIN_MODE));
    assert!(!ask_bool(&mut b, call::HAS_FREQ_CORRECTION));
    let mut p = rx(call::SET_GAIN_MODE);
    p.bool(true);
    b.ctl.call_void(&p, WAIT).unwrap();
    assert!(!ask_bool(&mut b, call::GET_GAIN_MODE));
    let mut p = rx(call::SET_FREQ_CORRECTION);
    p.f64(2.0);
    b.ctl.call_void(&p, WAIT).unwrap();
    let mut p = rx(call::SET_BANDWIDTH);
    p.f64(1e6);
    b.ctl.call_void(&p, WAIT).unwrap();
    let mut p = rx(call::SET_GAIN);
    p.f64(50.0);
    b.ctl.call_void(&p, WAIT).unwrap();
    let element = |b: &mut Bare, name: &str| {
        let mut p = rx(call::GET_GAIN_ELEMENT);
        p.string(name);
        b.ctl.call(&p, WAIT, |u| u.f64()).unwrap()
    };
    assert_eq!(
        [
            element(&mut b, "LNA"),
            element(&mut b, "VGA"),
            element(&mut b, "AMP")
        ],
        [40.0, 10.0, 0.0]
    );
    // One stage set by hand: the overall gain reads back as the sum.
    let mut p = rx(call::SET_GAIN_ELEMENT);
    p.string("AMP").f64(14.0);
    b.ctl.call_void(&p, WAIT).unwrap();
    assert_eq!(
        b.ctl.call(&rx(call::GET_GAIN), WAIT, |u| u.f64()).unwrap(),
        64.0
    );
    let seen = rig.owner.seen.lock().unwrap().clone();
    assert!(
        seen.iter().all(|r| !matches!(
            r,
            Request::Agc { .. } | Request::Correction { .. } | Request::Bandwidth { .. }
        )),
        "{seen:?}"
    );
    assert!(seen.contains(&Request::Gain { db: 50.0 }), "{seen:?}");
}

/// An open radio holds the receive and the transmit settings in one schema: no key of the client is in both.
#[test]
fn the_receive_and_transmit_settings_of_the_client_have_distinct_keys() {
    let driver = SoapyRemoteDriver::new(Arc::new(FakeTransport::new()));
    let rx = DriverKind::new(Box::new(driver)).schema().clone();
    let tx = tx_schema(SoapyTxSetting::SETTINGS, &TxLimits::default());
    let both: Vec<&str> = tx
        .settings()
        .iter()
        .map(|d| d.field.key.as_ref())
        .filter(|k| rx.id(k).is_ok())
        .collect();
    assert!(both.is_empty(), "{both:?}");
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
    transmit(StreamProt::Udp);
}

#[test]
fn transmit_over_tcp_arrives_in_order() {
    transmit(StreamProt::Tcp);
}

fn transmit(prot: StreamProt) {
    let (rig, feeder) = rig(SampleType::Cs16);
    let stop = Arc::new(AtomicBool::new(false));
    let p = produce(
        feeder,
        SampleType::Cs16,
        stop.clone(),
        None,
        Arc::new(AtomicU64::new(0)),
    );
    let (mut src, _r) = rig.open(&[
        ("serverlog", Value::Bool(false)),
        ("transport", Value::Choice(prot.index())),
    ]);
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
    let _ssdp = Ssdp::start(&net, "uuid-7", "127.0.0.1", DEFAULT_PORT, "HackRF One").unwrap();
    let found = ssdp_search(net.as_ref(), &[SSDP_V4], Duration::from_millis(400)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        (
            found[0].uuid.as_str(),
            found[0].port,
            found[0].radio.as_str()
        ),
        ("uuid-7", DEFAULT_PORT, "HackRF One")
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
        "",
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

/// A bare SoapyRemote client of TCP streams, written from the protocol: the calls, the two connections, the messages and the
/// acknowledgements.
struct Bare {
    ctl: rpc::Conn,
    net: Arc<dyn Transport>,
    port: u16,
}

/// One TCP receive stream of a [`Bare`] client.
struct BareStream {
    id: i32,
    data: Wire,
    _status: Wire,
    buf: Vec<u8>,
    next: u32,
    acked: u32,
    window: u32,
    overflows: u64,
}

const WAIT: Duration = Duration::from_secs(3);

impl Bare {
    fn connect(rig: &Rig) -> Bare {
        let c = rig.net.connect("127.0.0.1", rig.port, false, WAIT).unwrap();
        Bare {
            ctl: rpc::Conn::new(c),
            net: rig.net.clone(),
            port: rig.port,
        }
    }

    /// Sets up a CS16 receive stream over TCP with a window of `window` bytes in messages of 4096 bytes, and activates it.
    fn open(&mut self, window: usize) -> BareStream {
        let mut kw = rpc::Kwargs::new();
        kw.insert("remote:prot".into(), "tcp".into());
        kw.insert("remote:mtu".into(), "4096".into());
        kw.insert("remote:window".into(), window.to_string());
        let mut p = Packer::call(call::SETUP_STREAM);
        p.char(RX)
            .string("CS16")
            .size_list(&[0])
            .kwargs(&kw)
            .string("")
            .string("");
        let port: u16 = self
            .ctl
            .call(&p, WAIT, |u| u.string())
            .unwrap()
            .parse()
            .unwrap();
        assert_ne!(port, self.port);
        let data = self.net.connect("127.0.0.1", port, false, WAIT).unwrap();
        let status = self.net.connect("127.0.0.1", port, false, WAIT).unwrap();
        let id = self
            .ctl
            .next(WAIT, |u| {
                let id = u.i32()?;
                u.string()?;
                Ok(id)
            })
            .unwrap();
        let mut data = Wire::tcp(data);
        data.set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let mut s = BareStream {
            id,
            data,
            _status: Wire::tcp(status),
            buf: vec![0; 65_536],
            next: 0,
            acked: 0,
            window: (window / 4096).max(1) as u32,
            overflows: 0,
        };
        s.ack();
        self.activate(id);
        s
    }

    fn activate(&mut self, id: i32) {
        let mut p = Packer::call(call::ACTIVATE_STREAM);
        p.i32(id).i32(0).i64(0).i32(0);
        assert_eq!(self.ctl.call(&p, WAIT, |u| u.i32()).unwrap(), 0);
    }

    fn deactivate(&mut self, id: i32) {
        let mut p = Packer::call(call::DEACTIVATE_STREAM);
        p.i32(id).i32(0).i64(0);
        assert_eq!(self.ctl.call(&p, WAIT, |u| u.i32()).unwrap(), 0);
    }

    fn close(&mut self, s: BareStream) {
        let mut p = Packer::call(call::CLOSE_STREAM);
        p.i32(s.id);
        self.ctl.call_void(&p, WAIT).unwrap();
    }
}

impl BareStream {
    fn ack(&mut self) {
        let a = ack(self.next, self.window);
        self.data.send(&a, &mut || false).unwrap();
        self.acked = self.next;
    }

    /// Reads messages until `want` samples have arrived or `secs` pass, sleeping `pause` after each message, and appends the
    /// samples to `got`. Every message must carry the next sequence.
    fn read(&mut self, got: &mut Vec<u8>, want: usize, pause: Duration, secs: f64) {
        let t = Instant::now();
        while got.len() < want * 4 && t.elapsed().as_secs_f64() < secs {
            let Some(n) = self.data.recv(&mut self.buf).unwrap() else {
                continue;
            };
            let h = DgHeader::parse(&self.buf[..n]).unwrap();
            assert_eq!(h.seq, self.next, "messages arrive in sequence");
            self.next = h.seq.wrapping_add(1);
            if h.elems == OVERFLOW {
                self.overflows += 1;
            } else {
                got.extend_from_slice(&self.buf[HEADER_LEN..n]);
            }
            if self.next.wrapping_sub(self.acked) >= ack_trigger(self.window) {
                self.ack();
            }
            if !pause.is_zero() {
                std::thread::sleep(pause);
            }
        }
    }
}

/// The index of the first sample in `got`, after checking that all samples follow it in order.
fn in_order(got: &[u8]) -> u64 {
    let k0 = index_of(SampleType::Cs16, &got[..4]);
    let bad = got
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .find(|(i, b)| b.as_slice() != sample(SampleType::Cs16, k0 + *i as u64).as_slice())
        .map(|(i, _)| i);
    assert_eq!(bad, None, "samples arrive in order and unchanged from {k0}");
    k0
}

/// Pushes paced blocks until `total` samples are pushed.
fn produce_until(feeder: Feeder, total: u64) -> (JoinHandle<()>, Arc<AtomicU64>) {
    let stop = Arc::new(AtomicBool::new(false));
    let pushed = Arc::new(AtomicU64::new(0));
    let (s2, p2) = (stop.clone(), pushed.clone());
    let h = produce(feeder, SampleType::Cs16, stop, None, pushed.clone());
    let w = std::thread::spawn(move || {
        while p2.load(Ordering::Relaxed) < total {
            std::thread::sleep(Duration::from_millis(1));
        }
        s2.store(true, Ordering::Relaxed);
        h.join().unwrap();
    });
    (w, pushed)
}

/// A reader slower than the source holds the server back through the window and TCP: within the hub's queue nothing is
/// lost and nothing is reordered.
#[test]
fn a_slow_tcp_reader_loses_and_reorders_nothing() {
    let (rig, feeder) = rig_on(
        Arc::new(FakeTransport::new()),
        SampleType::Cs16,
        Config {
            queue_ms: 4000,
            ..config()
        },
        DEFAULT_PORT,
    );
    let mut c = Bare::connect(&rig);
    let mut s = c.open(64 << 10);
    let (p, pushed) = produce_until(feeder, 1_200_000);
    let mut got = Vec::new();
    // About 1 000 samples a message and a pause of 1 ms: under 1 Msps against 2.4 Msps of source.
    s.read(&mut got, 100_000, Duration::from_millis(1), 10.0);
    let k0 = index_of(SampleType::Cs16, &got[..4]);
    let t = Instant::now();
    p.join().unwrap();
    let produced = t.elapsed();
    let total = pushed.load(Ordering::Relaxed);
    let want = (total - k0) as usize;
    s.read(&mut got, want, Duration::from_millis(1), 20.0);
    assert_eq!(in_order(&got), k0);
    assert_eq!(
        got.len() / 4,
        want,
        "every sample up to the last one pushed"
    );
    assert!(
        t.elapsed() > produced + Duration::from_millis(200),
        "the reader was the slower: {:?} after the source's {produced:?}",
        t.elapsed()
    );
    let cl = rig.hub.snapshot().clients;
    assert_eq!((cl.len(), cl[0].dropped_samples, s.overflows), (1, 0, 0));
    c.close(s);
}

/// A client that stops reading is dropped by the hub's loss rule once its loss has run for `drop_after_s`; what it read
/// before is in order.
#[test]
fn a_stalled_tcp_client_is_dropped_as_too_slow() {
    let (rig, feeder) = rig_on(
        Arc::new(FakeTransport::new()),
        SampleType::Cs16,
        Config {
            queue_ms: 200,
            drop_after_s: 0.3,
            ..config()
        },
        DEFAULT_PORT,
    );
    let mut c = Bare::connect(&rig);
    let mut s = c.open(64 << 10);
    let stop = Arc::new(AtomicBool::new(false));
    let p = produce(
        feeder,
        SampleType::Cs16,
        stop.clone(),
        None,
        Arc::new(AtomicU64::new(0)),
    );
    let mut got = Vec::new();
    s.read(&mut got, 50_000, Duration::ZERO, 5.0);
    in_order(&got);
    let mut events = Vec::new();
    let left = wait_for(
        || {
            events.extend(rig.hub.take_events());
            events.iter().any(|e| {
                matches!(
                    e,
                    Event::ClientLeft {
                        reason: Leave::TooSlow,
                        ..
                    }
                )
            })
        },
        5.0,
    );
    stop.store(true, Ordering::Relaxed);
    p.join().unwrap();
    assert!(left, "{events:?}");
}

/// On one RPC connection: a stream is deactivated and activated again (its sequence carries on), closed, and set up again
/// with new connections; each run arrives in order.
#[test]
fn a_tcp_stream_closes_and_opens_again_on_one_connection() {
    let (rig, feeder) = rig(SampleType::Cs16);
    let stop = Arc::new(AtomicBool::new(false));
    let p = produce(
        feeder,
        SampleType::Cs16,
        stop.clone(),
        None,
        Arc::new(AtomicU64::new(0)),
    );
    let mut c = Bare::connect(&rig);
    let mut s = c.open(256 << 10);
    let mut got = Vec::new();
    s.read(&mut got, 200_000, Duration::ZERO, 5.0);
    in_order(&got);
    c.deactivate(s.id);
    // What was under way drains; then nothing comes.
    let mut rest = Vec::new();
    s.read(&mut rest, usize::MAX / 8, Duration::ZERO, 0.3);
    let quiet = s.next;
    s.read(&mut rest, usize::MAX / 8, Duration::ZERO, 0.2);
    assert_eq!(s.next, quiet, "nothing after the deactivate");
    c.activate(s.id);
    let mut again = Vec::new();
    s.read(&mut again, 200_000, Duration::ZERO, 5.0);
    in_order(&again);
    c.deactivate(s.id);
    c.close(s);
    let mut s2 = c.open(256 << 10);
    let mut third = Vec::new();
    s2.read(&mut third, 200_000, Duration::ZERO, 5.0);
    assert!(third.len() >= 200_000 * 4);
    in_order(&third);
    c.close(s2);
    stop.store(true, Ordering::Relaxed);
    p.join().unwrap();
    let cl = rig.hub.snapshot().clients;
    assert_eq!(cl.len(), 1, "one client all along");
}
