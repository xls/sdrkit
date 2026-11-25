//! USB radios and shares without hardware: a scripted dongle runs the FM stack through `usb:rtl`, the table of USB
//! devices follows arrivals and removals, a radio that cannot be reached says why, and a share serves a radio to network
//! clients beside a local stack: bit exact, under each control, with a slow client, with transmit, and until its radio
//! closes. Every USB device is one of a scripted bus, and every socket one of the fake network.

mod common;

use common::*;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{
    Choice, Clock, Complex32, ConfigError, Event, Facilities, InputInfo, MemStorage, Output, Owner,
    Published, Random, Receiver, Results, ResultsDecl, SettingError, SettingId, StackDescriptor,
    Transport, Value, Values,
};
use sdrkit_driver_core::rtl_tcp::{cmd, command};
use sdrkit_driver_core::sample::{from_iq, to_iq};
use sdrkit_driver_core::{DriverError, Unavailable};
use sdrkit_driver_usb::{FakeUsb, UsbBus, UsbDeviceInfo, UsbId};
use sdrkit_graph::SampleType;
use sdrkit_host_engine::{
    split_address, ClientLeftEvent, ClientRow, DeviceArrivedEvent, DeviceLeftEvent, DeviceRow,
    DeviceState, Engine, EngineError, GrantArgs, Input, KickArgs, LeftReason, ListenArgs,
    ListenReply, ListenerRow, OwnerRef, RadioId, Setup, ShareControl, ShareFields, ShareId,
    ShareProtocol, ShareRow, CLIENTS, CLIENT_LEFT, DEVICES, DEVICE_ARRIVED, DEVICE_LEFT, GRANT,
    KICK, LISTEN, LISTENERS, SHARES, TRANSMIT_ALLOWED,
};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What the recorder stacks were fed.
static TAPE_A: Mutex<Vec<Complex32>> = Mutex::new(Vec::new());
static TAPE_B: Mutex<Vec<Complex32>> = Mutex::new(Vec::new());
/// The most samples a tape keeps.
const TAPE_MAX: usize = 4 << 20;

/// A stack that keeps the samples it is fed.
struct Recorder {
    tape: &'static Mutex<Vec<Complex32>>,
    results: Published,
    values: Values,
}

impl Receiver for Recorder {
    fn configure(&mut self, _: &InputInfo) -> Result<(), ConfigError> {
        Ok(())
    }
    fn feed(&mut self, iq: &[Complex32], _: &mut Output) {
        let mut t = self.tape.lock().unwrap();
        let room = TAPE_MAX.saturating_sub(t.len()).min(iq.len());
        t.extend_from_slice(&iq[..room]);
    }
}

impl Owner for Recorder {
    fn schema(&self) -> &Arc<sdrkit_core::Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.values.set(id, value)
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.results.results().decl()
    }
    fn results(&self) -> &Results {
        self.results.results()
    }
}

fn recorder(
    tape: &'static Mutex<Vec<Complex32>>,
    cfg: &sdrkit_core::StackConfig,
) -> Box<dyn Receiver> {
    Box::new(Recorder {
        tape,
        results: Published::from_decl(Arc::new(ResultsDecl::new())),
        values: cfg.settings.clone(),
    })
}

static REC_A: StackDescriptor = StackDescriptor {
    id: "rec_a",
    name: "Recorder A",
    submodes: &[],
    signal_types: &[],
    make_receiver: |cfg| Ok(recorder(&TAPE_A, cfg)),
    benchmarks: None,
    ..METER_DESC
};

static REC_B: StackDescriptor = StackDescriptor {
    id: "rec_b",
    name: "Recorder B",
    submodes: &[],
    signal_types: &[],
    make_receiver: |cfg| Ok(recorder(&TAPE_B, cfg)),
    benchmarks: None,
    ..METER_DESC
};

/// An engine on `net` with the USB bus `usb`, on a clock that runs.
fn engine_with(net: Arc<dyn Transport>, usb: Arc<dyn UsbBus>) -> Engine {
    let t0 = std::time::Instant::now();
    let mut stacks: Vec<&'static StackDescriptor> = vec![&METER, &TONE, &REC_A, &REC_B];
    stacks.push(&sdrkit_stack_fm::DESCRIPTOR);
    Engine::with_usb(
        Setup {
            facilities: Facilities {
                storage: Arc::new(MemStorage::new()),
                clock: Clock::new(move || 1.7e9 + t0.elapsed().as_secs_f64()),
                random: Random::fixed(7),
                codecs: Arc::default(),
            },
            transport: net,
            stacks,
            families: Vec::new(),
            components: sdrkit_stack_fm::graph::registry(),
        },
        usb,
    )
    .unwrap()
}

fn no_usb() -> Arc<dyn UsbBus> {
    Arc::new(sdrkit_driver_usb::NoUsb)
}

/// Sets setting `key` of owner `r`.
fn set(e: &Engine, r: OwnerRef, key: &str, v: Value) {
    e.owner(r, |o| {
        let id = o.schema().id(key).unwrap();
        sdrkit_core::owner::apply(o, id, &v)
    })
    .unwrap()
    .unwrap();
}

/// The rows of the table `devices`: bus position, address, driver, state and radio.
fn devices(e: &Engine) -> Vec<(String, String, String, DeviceState, Option<String>)> {
    e.owner(OwnerRef::Engine, |o| {
        let mut rows: Vec<_> = DEVICES
            .rows::<DeviceRow<'_>>(o.results())
            .map(|(key, r)| {
                assert_eq!(key, r.path);
                (
                    key.to_owned(),
                    r.address.to_owned(),
                    r.driver.to_owned(),
                    r.state,
                    r.radio.map(str::to_owned),
                )
            })
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows
    })
    .unwrap()
}

fn engine_events(e: &Engine) -> Vec<Event> {
    let mut out = Vec::new();
    e.take_events(OwnerRef::Engine, &mut out).unwrap();
    out
}

#[test]
fn a_scripted_dongle_runs_the_fm_stack_through_usb_rtl() {
    let e = engine_with(Arc::new(FakeTransport::new()), Arc::new(dongle_bus()));
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(
        devices(&e),
        [(
            "1-3".to_owned(),
            "usb:rtl:0001".to_owned(),
            "rtlusb".to_owned(),
            DeviceState::Free,
            None
        )]
    );
    let (driver, selector) = split_address("usb:rtl:");
    assert_eq!((driver, selector), ("rtlusb", ""));
    let kind = e.find("driver.rtlusb").unwrap();
    let radio = e.open_radio(driver, selector, &defaults(&e, kind)).unwrap();
    let fm = &sdrkit_stack_fm::DESCRIPTOR;
    let s = e
        .start_stack(
            fm.id,
            None,
            &Values::defaults(&(fm.settings)()),
            Input::Radio(radio),
            Some(100e6),
        )
        .unwrap();
    let fed = wait_for(10.0, || {
        (number(&e, OwnerRef::Session(s), "session.samples") > 200_000.0).then_some(())
    });
    assert!(
        fed.is_some(),
        "the FM stack was fed no samples of the dongle"
    );
    assert_eq!(
        field(&e, OwnerRef::Session(s), "session.problem").filter(|v| !v.is_none()),
        None,
        "the FM stack took the samples"
    );
    // The row shows the radio that the device is open as, and the radio the selector of that one device.
    assert!(e.settle(Duration::from_secs(5)));
    let key = format!("radio.r{}", radio.to_u64());
    assert_eq!(devices(&e)[0].3, DeviceState::Open);
    assert_eq!(devices(&e)[0].4.as_deref(), Some(key.as_str()));
    e.stop_session(s).unwrap();
    e.close_session(s).unwrap();
    e.close_radio(radio).unwrap();
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(
        (devices(&e)[0].3, devices(&e)[0].4.clone()),
        (DeviceState::Free, None)
    );
}

#[test]
fn a_device_that_arrives_and_leaves_raises_events_and_changes_the_table() {
    let bus =
        FakeUsb::from_text(&format!("sdrkit-usb 1\nmode lenient\ndevice 1 {DONGLE}\n")).unwrap();
    let e = engine_with(Arc::new(FakeTransport::new()), Arc::new(bus.clone()));
    assert!(e.settle(Duration::from_secs(5)));
    // The devices that were attached at the start raise no event.
    assert!(engine_events(&e).is_empty());
    let mut hackrf = UsbDeviceInfo::new(UsbId(2), 0x1d50, 0x6089);
    hackrf.bus = "2".into();
    hackrf.ports = vec![1, 4];
    hackrf.product = Some("HackRF One".into());
    hackrf.serial = Some("0000000000000000A1B2C3D4E5F60718".into());
    bus.plug(hackrf);
    // A device that no driver takes changes nothing.
    let mut mouse = UsbDeviceInfo::new(UsbId(3), 0x046d, 0xc52b);
    mouse.bus = "2".into();
    mouse.ports = vec![2];
    bus.plug(mouse);
    let arrived = wait_for(5.0, || {
        let ev = engine_events(&e);
        (!ev.is_empty()).then_some(ev)
    })
    .expect("no event for the device that arrived");
    assert_eq!(arrived.len(), 1);
    let a: DeviceArrivedEvent<'_> = DEVICE_ARRIVED.read(&arrived[0]).unwrap();
    assert_eq!(
        (a.address, a.driver, a.name, a.path, a.state),
        (
            "usb:hackrf:0000000000000000a1b2c3d4e5f60718",
            "hackrf",
            "HackRF One",
            "2-1.4",
            DeviceState::Free
        )
    );
    assert_eq!(
        devices(&e).iter().map(|d| d.0.as_str()).collect::<Vec<_>>(),
        ["1-3", "2-1.4"]
    );
    bus.unplug(UsbId(1));
    let left = wait_for(5.0, || {
        let ev = engine_events(&e);
        (!ev.is_empty()).then_some(ev)
    })
    .expect("no event for the device that left");
    assert_eq!(left.len(), 1);
    let l: DeviceLeftEvent<'_> = DEVICE_LEFT.read(&left[0]).unwrap();
    assert_eq!(
        (l.address, l.driver, l.path),
        ("usb:rtl:0001", "rtlusb", "1-3")
    );
    assert_eq!(
        devices(&e).iter().map(|d| d.0.as_str()).collect::<Vec<_>>(),
        ["2-1.4"]
    );
    // Listing and watching opened no device.
    assert_eq!(bus.mismatch(), None);
}

/// The cell `available` and the key of `unavailable_why` of the row of driver `key`.
fn driver_available(e: &Engine, key: &str) -> (Option<Value>, Option<Value>) {
    e.owner(OwnerRef::Engine, |o| {
        let res = o.results();
        let t = res.table(res.decl().table_id("drivers").unwrap()).unwrap();
        let row = t.row(key).unwrap();
        let cell = |col: &str| {
            let id = row.decl().id(col).unwrap();
            row.value(id).cloned().filter(|v| *v != Value::None)
        };
        (cell("available"), cell("unavailable_why"))
    })
    .unwrap()
}

#[test]
fn a_usb_radio_that_cannot_be_reached_says_why() {
    // No USB on this platform: the kinds are listed as not available, and an open says why.
    let e = engine_with(Arc::new(FakeTransport::new()), no_usb());
    let why = Unavailable::NoUsbHere;
    assert_eq!(
        driver_available(&e, "hackrf"),
        (Some(Value::Bool(false)), Some(Value::Choice(why.index())))
    );
    assert_eq!(driver_available(&e, "rtlusb").0, Some(Value::Bool(false)));
    assert_eq!(
        driver_available(&e, "rtl_tcp"),
        (Some(Value::Bool(true)), None)
    );
    for address in ["usb:hackrf:", "usb:rtl:", "usb:rtl:0001"] {
        let (driver, selector) = split_address(address);
        let kind = e.find(&format!("driver.{driver}")).unwrap();
        match e.open_radio(driver, selector, &defaults(&e, kind)) {
            Err(EngineError::Driver(DriverError::Unavailable { why })) => {
                assert_eq!(why, Unavailable::NoUsbHere, "{address}");
            }
            other => panic!("the open of {address} returned {other:?}"),
        }
    }
    assert!(devices(&e).is_empty());

    // A bus with USB: a device that Windows has not bound to WinUSB, one that another program holds, and no third one.
    let bus = FakeUsb::from_text(
        "sdrkit-usb 1\nmode lenient\n\
         device 1 1d50:6089 bus=\"1\" ports=1 serial=\"00000000000000000000000000000001\" blocked=needs_winusb\n\
         device 2 1d50:6089 bus=\"1\" ports=2 serial=\"00000000000000000000000000000002\"\n\
         open 2 -> !busy\n",
    )
    .unwrap();
    let e = engine_with(Arc::new(FakeTransport::new()), Arc::new(bus));
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(
        driver_available(&e, "hackrf"),
        (Some(Value::Bool(true)), None)
    );
    let states = |e: &Engine| devices(e).iter().map(|d| d.3).collect::<Vec<_>>();
    assert_eq!(states(&e), [DeviceState::NeedsDriver, DeviceState::Free]);
    let kind = e.find("driver.hackrf").unwrap();
    let v = defaults(&e, kind);
    // The empty selector passes over the device that needs a driver and finds the other one busy.
    match e.open_radio("hackrf", "", &v) {
        Err(EngineError::Driver(DriverError::Busy)) => {}
        other => panic!("the open of the busy device returned {other:?}"),
    }
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(states(&e), [DeviceState::NeedsDriver, DeviceState::Busy]);
    match e.open_radio("hackrf", "00000000000000000000000000000001", &v) {
        Err(EngineError::Driver(DriverError::Unavailable { why })) => {
            assert_eq!(why, Unavailable::NeedsWinUsb);
        }
        other => panic!("the open of the device without WinUSB returned {other:?}"),
    }
    match e.open_radio("hackrf", "@9-9", &v) {
        Err(EngineError::Driver(DriverError::Unavailable { why })) => {
            assert_eq!(why, Unavailable::NotFound);
        }
        other => panic!("the open of a device that is not attached returned {other:?}"),
    }
}

/// Opens the share of `radio` and returns it as an owner.
fn share(e: &Engine, radio: RadioId) -> (ShareId, OwnerRef) {
    let id = e.open_share(radio).unwrap();
    (id, OwnerRef::Share(id))
}

/// Starts a listener of `protocol` on 127.0.0.1 at a port that the network chooses, and returns its number and port.
fn listen(e: &Engine, share: OwnerRef, protocol: ShareProtocol) -> (i64, u16) {
    let reply = e
        .owner(share, |o| {
            LISTEN.call(
                o,
                &ListenArgs {
                    protocol,
                    port: 0,
                    host: Some("127.0.0.1"),
                },
            )
        })
        .unwrap()
        .unwrap()
        .unwrap();
    let r: ListenReply = LISTEN.read_reply(&reply).unwrap();
    (r.listener, u16::try_from(r.port).unwrap())
}

/// A copy of the state of a client row.
#[derive(Clone, Debug)]
struct Client {
    id: i64,
    dropped: i64,
    samples: i64,
    slice: Option<(f64, f64)>,
    steers: bool,
    may_transmit: bool,
}

fn clients(e: &Engine, share: OwnerRef) -> Vec<Client> {
    e.owner(share, |o| {
        CLIENTS
            .rows::<ClientRow<'_>>(o.results())
            .map(|(key, r)| Client {
                id: key.parse().unwrap(),
                dropped: r.dropped_samples,
                samples: r.samples,
                slice: r
                    .slice_centre_hz
                    .zip(r.slice_rate_hz)
                    .map(|(c, r)| (c.0, r.0)),
                steers: r.steers,
                may_transmit: r.may_transmit,
            })
            .collect()
    })
    .unwrap()
}

/// The share's count `key` of its result fields.
fn count(e: &Engine, share: OwnerRef, key: &str) -> i64 {
    number(e, share, key) as i64
}

/// Waits until the share has `n` clients and returns them.
fn wait_clients(e: &Engine, share: OwnerRef, n: usize) -> Vec<Client> {
    wait_for(10.0, || {
        let c = clients(e, share);
        (c.len() == n).then_some(c)
    })
    .unwrap_or_else(|| {
        panic!(
            "the share did not get to {n} clients: {:?}",
            clients(e, share)
        )
    })
}

/// The centre frequency of the samples of radio `r`.
fn centre(e: &Engine, r: RadioId) -> f64 {
    number(e, OwnerRef::Radio(r), "centre_hz")
}

const RATE: f64 = 1.024e6;

#[test]
fn a_share_serves_the_test_signal_bit_exact_beside_a_local_stack() {
    let net = FakeTransport::new();
    let e = engine_with(Arc::new(net.clone()), no_usb());
    let radio = tone_radio(&e, RATE);
    let local = e
        .start_stack(
            "rec_a",
            None,
            &Values::defaults(&(REC_A.settings)()),
            Input::Radio(radio),
            None,
        )
        .unwrap();
    let (id, sh) = share(&e, radio);
    let (_, port) = listen(&e, sh, ShareProtocol::RtlTcp);
    // The share is a row of the engine, and its session one of the sessions.
    let row = e
        .owner(OwnerRef::Engine, |o| {
            SHARES
                .rows::<ShareRow<'_>>(o.results())
                .map(|(k, r)| (k.to_owned(), r.radio.to_owned()))
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(
        row,
        [(
            format!("share.h{}", id.to_u64()),
            format!("radio.r{}", radio.to_u64())
        )]
    );
    assert_eq!(e.find(&row[0].0).unwrap(), sh);

    // Our own rtl_tcp driver reads the share, and a second stack records what it gives.
    let kind = e.find("driver.rtl_tcp").unwrap();
    let mut v = defaults(&e, kind);
    for (key, value) in [("rate", Value::Hz(RATE)), ("centre_hz", Value::Hz(100e6))] {
        let id = v.schema().id(key).unwrap();
        v.set(id, &value).unwrap();
    }
    let far = e
        .open_radio("rtl_tcp", &format!("127.0.0.1:{port}"), &v)
        .unwrap();
    let remote = e
        .start_stack(
            "rec_b",
            None,
            &Values::defaults(&(REC_B.settings)()),
            Input::Radio(far),
            None,
        )
        .unwrap();
    let got = wait_for(20.0, || {
        (TAPE_B.lock().unwrap().len() >= 400_000).then_some(())
    });
    assert!(
        got.is_some(),
        "the far stack got {} samples",
        TAPE_B.lock().unwrap().len()
    );
    // What the far side got is what the near side got, as unsigned 8-bit samples, sample for sample.
    let near = TAPE_A.lock().unwrap().clone();
    let far_tape = TAPE_B.lock().unwrap().clone();
    let mut bytes = Vec::new();
    from_iq(SampleType::Cu8, &near, &mut bytes);
    let mut want = vec![Complex32::default(); near.len()];
    to_iq(SampleType::Cu8, &bytes, &mut want);
    let head = &far_tape[..256];
    let at = want
        .windows(head.len())
        .position(|w| w == head)
        .expect("the start of the far stream is not in the near stream");
    let n = (want.len() - at).min(far_tape.len());
    assert!(n >= 300_000, "only {n} samples to compare");
    let differ = want[at..at + n]
        .iter()
        .zip(&far_tape[..n])
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(differ, 0, "of {n} samples");
    // Nobody lost a sample: not the radio, not the local stack, not the client.
    assert_eq!(number(&e, OwnerRef::Radio(radio), "dropped"), 0.0);
    assert_eq!(number(&e, OwnerRef::Session(local), "session.dropped"), 0.0);
    assert_eq!(
        number(&e, OwnerRef::Session(remote), "session.dropped"),
        0.0
    );
    let c = wait_clients(&e, sh, 1);
    assert_eq!(c[0].dropped, 0);
    let listeners = e
        .owner(sh, |o| {
            LISTENERS
                .rows::<ListenerRow<'_>>(o.results())
                .map(|(k, r)| (k.to_owned(), r.protocol, r.port, r.clients))
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(
        listeners,
        [("1".to_owned(), ShareProtocol::RtlTcp, i64::from(port), 1)]
    );
    let f = e
        .owner(sh, |o| {
            let f = ShareFields::read(o.results().fields()).unwrap();
            (f.format.to_owned(), f.rate_hz.0, f.clients, f.pool_misses)
        })
        .unwrap();
    // The test signal has float samples, and the share carries them as they are.
    assert_eq!(f, ("cf32".to_owned(), RATE, 1, 0));
}

/// Connects a raw rtl_tcp client to the share at `port` and reads its greeting.
fn raw_client(net: &FakeTransport, port: u16) -> Box<dyn sdrkit_core::net::Conn> {
    let mut c = net
        .connect("127.0.0.1", port, false, Duration::from_secs(2))
        .unwrap();
    let mut greeting = [0u8; 12];
    c.read_exact(&mut greeting).unwrap();
    assert_eq!(&greeting[..4], b"RTL0");
    c
}

/// Reads and drops what the share sends to `c`, on a thread, until the connection ends.
fn drain(mut c: Box<dyn sdrkit_core::net::Conn>) {
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 1 << 16];
        while matches!(c.read(&mut buf), Ok(n) if n > 0) {}
    });
}

#[test]
fn the_control_follow_leaves_the_radio_to_the_host() {
    let net = FakeTransport::new();
    let e = engine_with(Arc::new(net.clone()), no_usb());
    let radio = tone_radio(&e, RATE);
    let (_, sh) = share(&e, radio);
    let (_, port) = listen(&e, sh, ShareProtocol::RtlTcp);
    let mut c = raw_client(&net, port);
    drain(c.try_clone().unwrap());
    c.write_all(&command(cmd::FREQ, 101_000_000)).unwrap();
    c.write_all(&command(cmd::RATE, 2_048_000)).unwrap();
    c.write_all(&command(cmd::GAIN, 200)).unwrap();
    let refused = wait_for(5.0, || {
        (count(&e, sh, "requests_refused") == 3).then_some(())
    });
    assert!(
        refused.is_some(),
        "{} refused",
        count(&e, sh, "requests_refused")
    );
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(centre(&e, radio), 100e6);
    assert_eq!(number(&e, OwnerRef::Radio(radio), "rate_hz"), RATE);
    assert_eq!(count(&e, sh, "requests_done"), 0);
    let c1 = wait_clients(&e, sh, 1);
    assert!(!c1[0].steers);

    // A stack of the host moves the radio, with the share on it: the share holds the radio nowhere.
    let s = e
        .start_stack(
            "meter",
            None,
            &Values::defaults(&(METER.settings)()),
            Input::Radio(radio),
            Some(107.7e6),
        )
        .unwrap();
    assert_eq!(centre(&e, radio), 107.7e6);
    e.stop_session(s).unwrap();
    e.close_session(s).unwrap();
}

#[test]
fn the_control_remote_lets_one_client_steer_until_a_host_session_reads_the_radio() {
    let net = FakeTransport::new();
    let e = engine_with(Arc::new(net.clone()), no_usb());
    let radio = tone_radio(&e, RATE);
    let (_, sh) = share(&e, radio);
    set(
        &e,
        sh,
        "control",
        Value::Choice(ShareControl::Remote.index()),
    );
    let (_, port) = listen(&e, sh, ShareProtocol::RtlTcp);
    let mut first = raw_client(&net, port);
    drain(first.try_clone().unwrap());
    let c1 = wait_clients(&e, sh, 1);
    let mut second = raw_client(&net, port);
    drain(second.try_clone().unwrap());
    let c2 = wait_clients(&e, sh, 2);
    let other = c2.iter().find(|c| c.id != c1[0].id).unwrap().id;
    // The client that connected first steers.
    first.write_all(&command(cmd::FREQ, 101_000_000)).unwrap();
    let moved = wait_for(5.0, || (centre(&e, radio) == 101e6).then_some(()));
    assert!(moved.is_some(), "the radio is at {}", centre(&e, radio));
    second.write_all(&command(cmd::FREQ, 102_000_000)).unwrap();
    let refused = wait_for(5.0, || {
        (count(&e, sh, "requests_refused") == 1).then_some(())
    });
    assert!(refused.is_some());
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(centre(&e, radio), 101e6);
    // The command `grant` names the client that steers.
    e.owner(sh, |o| {
        GRANT.call(
            o,
            &GrantArgs {
                client: other,
                steer: Some(true),
                transmit: None,
            },
        )
    })
    .unwrap()
    .unwrap();
    let steers: Vec<_> = clients(&e, sh)
        .iter()
        .map(|c| (c.id == other, c.steers))
        .collect();
    assert!(
        steers.iter().all(|(is_other, steers)| is_other == steers),
        "{steers:?}"
    );
    second.write_all(&command(cmd::FREQ, 102_000_000)).unwrap();
    let moved = wait_for(5.0, || (centre(&e, radio) == 102e6).then_some(()));
    assert!(moved.is_some(), "the radio is at {}", centre(&e, radio));
    // The rate that a client sets stays on Automatic for the host: a stack still gets the rate it needs.
    second.write_all(&command(cmd::RATE, 2_048_000)).unwrap();
    // The test radio was opened with a rate of the host, so the client does not change it.
    let refused = wait_for(5.0, || {
        (count(&e, sh, "requests_refused") == 2).then_some(())
    });
    assert!(refused.is_some());

    // A session of the host on the radio: the client steers no more, and the radio goes where the host tunes.
    let s = e
        .start_stack(
            "meter",
            None,
            &Values::defaults(&(METER.settings)()),
            Input::Radio(radio),
            Some(107.7e6),
        )
        .unwrap();
    assert_eq!(centre(&e, radio), 107.7e6);
    assert!(e.settle(Duration::from_secs(5)));
    second.write_all(&command(cmd::FREQ, 103_000_000)).unwrap();
    let refused = wait_for(5.0, || {
        (count(&e, sh, "requests_refused") == 3).then_some(())
    });
    assert!(refused.is_some());
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(centre(&e, radio), 107.7e6);
    assert!(clients(&e, sh).iter().all(|c| !c.steers));
    e.stop_session(s).unwrap();
    e.close_session(s).unwrap();
}

#[test]
fn the_control_slice_gives_each_client_its_own_part_of_the_capture() {
    let net = FakeTransport::new();
    let e = engine_with(Arc::new(net.clone()), no_usb());
    let radio = tone_radio(&e, RATE);
    let (_, sh) = share(&e, radio);
    set(
        &e,
        sh,
        "control",
        Value::Choice(ShareControl::Slice.index()),
    );
    let (_, port) = listen(&e, sh, ShareProtocol::RtlTcp);
    let mut a = raw_client(&net, port);
    drain(a.try_clone().unwrap());
    let ca = wait_clients(&e, sh, 1)[0].id;
    let mut b = raw_client(&net, port);
    drain(b.try_clone().unwrap());
    let cb = wait_clients(&e, sh, 2)
        .iter()
        .find(|c| c.id != ca)
        .unwrap()
        .id;
    a.write_all(&command(cmd::RATE, 256_000)).unwrap();
    a.write_all(&command(cmd::FREQ, 100_200_000)).unwrap();
    b.write_all(&command(cmd::RATE, 256_000)).unwrap();
    b.write_all(&command(cmd::FREQ, 99_800_000)).unwrap();
    // A slice outside the capture is refused, and the slice stays.
    b.write_all(&command(cmd::FREQ, 105_000_000)).unwrap();
    let sliced = wait_for(5.0, || {
        let c = clients(&e, sh);
        let of = |id: i64| c.iter().find(|c| c.id == id).and_then(|c| c.slice);
        (of(ca) == Some((100.2e6, 256e3)) && of(cb) == Some((99.8e6, 256e3))).then_some(())
    });
    assert!(sliced.is_some(), "{:?}", clients(&e, sh));
    let counted = wait_for(5.0, || {
        (count(&e, sh, "requests_done") == 4 && count(&e, sh, "requests_refused") == 1)
            .then_some(())
    });
    assert!(counted.is_some());
    // The radio stayed where it was.
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(centre(&e, radio), 100e6);
    assert_eq!(number(&e, OwnerRef::Radio(radio), "rate_hz"), RATE);
    // Each client gets samples at the rate of its slice: about a quarter of the capture.
    let before = clients(&e, sh);
    std::thread::sleep(Duration::from_millis(1000));
    assert!(e.settle(Duration::from_secs(5)));
    let after = clients(&e, sh);
    for c in &after {
        let was = before.iter().find(|b| b.id == c.id).unwrap().samples;
        let per_s = (c.samples - was) as f64;
        assert!(
            (150e3..400e3).contains(&per_s),
            "client {} got {per_s} samples in a second",
            c.id
        );
    }
}

#[test]
fn a_client_that_does_not_read_loses_only_its_own_samples() {
    let net = FakeTransport::new();
    let e = engine_with(Arc::new(net.clone()), no_usb());
    let radio = tone_radio(&e, RATE);
    let local = e
        .start_stack(
            "meter",
            None,
            &Values::defaults(&(METER.settings)()),
            Input::Radio(radio),
            None,
        )
        .unwrap();
    let (_, sh) = share(&e, radio);
    set(&e, sh, "drop_after_s", Value::F64(1.0));
    let (_, port) = listen(&e, sh, ShareProtocol::RtlTcp);
    // One client reads, the other never does: its connection fills, then its queue, and the share drops for it alone.
    let good = raw_client(&net, port);
    drain(good);
    let good_id = wait_clients(&e, sh, 1)[0].id;
    let _stuck = raw_client(&net, port);
    wait_clients(&e, sh, 2);
    let mut worst = 0;
    let left = wait_for(30.0, || {
        for c in clients(&e, sh).iter().filter(|c| c.id != good_id) {
            worst = worst.max(c.dropped);
        }
        let mut ev = Vec::new();
        e.take_events(sh, &mut ev).unwrap();
        ev.iter()
            .find_map(|x| CLIENT_LEFT.read::<ClientLeftEvent<'_>>(x))
            .map(|l| (l.client, l.reason))
    })
    .expect("the client that does not read was not disconnected");
    assert_ne!(left.0, good_id);
    assert_eq!(left.1, LeftReason::TooSlow);
    assert!(worst > 0, "the slow client lost no sample before it left");
    let rest = wait_clients(&e, sh, 1);
    assert_eq!((rest[0].id, rest[0].dropped), (good_id, 0));
    // The radio and the local stack lost nothing.
    assert_eq!(number(&e, OwnerRef::Radio(radio), "dropped"), 0.0);
    assert_eq!(number(&e, OwnerRef::Radio(radio), "radio_lost"), 0.0);
    assert_eq!(number(&e, OwnerRef::Session(local), "session.dropped"), 0.0);
    let own = e
        .owner(sh, |o| {
            ShareFields::read(o.results().fields())
                .map(|f| f.session.to_owned())
                .unwrap()
        })
        .unwrap();
    let own = e.find(&own).unwrap();
    assert_eq!(number(&e, own, "session.dropped"), 0.0);
}

#[test]
fn a_client_is_kicked_and_the_share_closes_with_its_radio() {
    let net = FakeTransport::new();
    let e = engine_with(Arc::new(net.clone()), no_usb());
    let radio = tone_radio(&e, RATE);
    let (id, sh) = share(&e, radio);
    // A radio has one share.
    assert!(matches!(
        e.open_share(radio),
        Err(EngineError::ShareOpen { share, .. }) if share == id
    ));
    let (_, port) = listen(&e, sh, ShareProtocol::RtlTcp);
    let (_, soapy) = listen(&e, sh, ShareProtocol::SoapyRemote);
    assert_ne!(port, soapy);
    let a = raw_client(&net, port);
    drain(a);
    let ca = wait_clients(&e, sh, 1)[0].id;
    e.owner(sh, |o| KICK.call(o, &KickArgs { client: ca }))
        .unwrap()
        .unwrap();
    wait_clients(&e, sh, 0);
    let mut b = raw_client(&net, port);
    wait_clients(&e, sh, 1);
    // The radio closes with the share on it: the share goes first, and its client is disconnected.
    e.close_radio(radio).unwrap();
    assert!(matches!(
        e.find(&format!("share.h{}", id.to_u64())),
        Err(EngineError::NoSuchKey { .. })
    ));
    assert!(e.owner(sh, |_| ()).is_err());
    let rows = e
        .owner(OwnerRef::Engine, |o| {
            SHARES.rows::<ShareRow<'_>>(o.results()).count()
        })
        .unwrap();
    assert_eq!(rows, 0);
    let mut buf = vec![0u8; 1 << 16];
    let ended = wait_for(5.0, || match b.read(&mut buf) {
        Ok(0) | Err(_) => Some(()),
        Ok(_) => None,
    });
    assert!(
        ended.is_some(),
        "the client of the closed share still gets samples"
    );
    assert!(net
        .connect("127.0.0.1", port, false, Duration::from_millis(200))
        .is_err());
}

#[test]
fn the_share_makes_its_queues_again_for_a_new_format_and_a_new_rate() {
    let net = FakeTransport::new();
    let e = engine_with(Arc::new(net.clone()), no_usb());
    // The rate of the radio is on Automatic, so the engine and the host can change it.
    let kind = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:tone", &defaults(&e, kind))
        .unwrap();
    let rate0 = number(&e, OwnerRef::Radio(radio), "rate_hz");
    let (_, sh) = share(&e, radio);
    let (_, port) = listen(&e, sh, ShareProtocol::RtlTcp);
    let format = |e: &Engine| {
        e.owner(sh, |o| {
            let f = ShareFields::read(o.results().fields()).unwrap();
            (f.format.to_owned(), f.rate_hz.0)
        })
        .unwrap()
    };
    assert_eq!(format(&e), ("cf32".to_owned(), rate0));
    let mut a = raw_client(&net, port);
    wait_clients(&e, sh, 1);
    // Another format: the client is disconnected, and the listener is on its port again.
    set(
        &e,
        sh,
        "format",
        Value::Choice(sdrkit_host_engine::ShareFormat::Cu8.index()),
    );
    let left = wait_for(5.0, || {
        let mut ev = Vec::new();
        e.take_events(sh, &mut ev).unwrap();
        ev.iter()
            .find_map(|x| CLIENT_LEFT.read::<ClientLeftEvent<'_>>(x))
            .map(|l| l.reason)
    });
    assert_eq!(left, Some(LeftReason::Shutdown));
    let mut buf = vec![0u8; 1 << 16];
    let ended = wait_for(5.0, || match a.read(&mut buf) {
        Ok(0) | Err(_) => Some(()),
        Ok(_) => None,
    });
    assert!(ended.is_some());
    assert_eq!(format(&e).0, "cu8");
    let ports = e
        .owner(sh, |o| {
            LISTENERS
                .rows::<ListenerRow<'_>>(o.results())
                .map(|(_, r)| r.port)
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(ports, [i64::from(port)]);
    // Another rate with no client connected: the share follows, and a new client gets samples at that rate.
    set(&e, OwnerRef::Radio(radio), "rate", Value::Hz(96e3));
    let followed = wait_for(5.0, || (format(&e).1 == 96e3).then_some(()));
    assert!(followed.is_some(), "{:?}", format(&e));
    let b = raw_client(&net, port);
    drain(b);
    let c = wait_clients(&e, sh, 1);
    let before = c[0].samples;
    std::thread::sleep(Duration::from_millis(1000));
    assert!(e.settle(Duration::from_secs(5)));
    let per_s = (clients(&e, sh)[0].samples - before) as f64;
    assert!(
        (60e3..140e3).contains(&per_s),
        "{per_s} samples in a second"
    );
}

#[test]
fn a_soapyremote_listener_that_other_machines_reach_answers_a_search() {
    use sdrkit_driver_soapyremote::discovery::{ssdp_search, SSDP_V4};
    let net = FakeTransport::new();
    let e = engine_with(Arc::new(net.clone()), no_usb());
    let radio = tone_radio(&e, RATE);
    let (_, sh) = share(&e, radio);
    // A listener for this machine only does not answer.
    let (local, _) = listen(&e, sh, ShareProtocol::SoapyRemote);
    assert!(ssdp_search(&net, &[SSDP_V4], Duration::from_millis(300))
        .unwrap()
        .is_empty());
    e.owner(sh, |o| {
        sdrkit_host_engine::UNLISTEN.call(o, &sdrkit_host_engine::UnlistenArgs { listener: local })
    })
    .unwrap()
    .unwrap();
    // A listener on every interface joins the group of the search and answers with its port.
    let reply = e
        .owner(sh, |o| {
            LISTEN.call(
                o,
                &ListenArgs {
                    protocol: ShareProtocol::SoapyRemote,
                    port: 55132,
                    host: None,
                },
            )
        })
        .unwrap()
        .unwrap()
        .unwrap();
    let r: ListenReply = LISTEN.read_reply(&reply).unwrap();
    assert_eq!(r.port, 55132);
    let found = ssdp_search(&net, &[SSDP_V4], Duration::from_millis(400)).unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].port, 55132);
    // The share closes: nobody answers.
    e.close_radio(radio).unwrap();
    assert!(ssdp_search(&net, &[SSDP_V4], Duration::from_millis(300))
        .unwrap()
        .is_empty());
}

/// Sets the transmit switch of the engine.
fn switch(e: &Engine, on: bool) {
    set(e, OwnerRef::Engine, TRANSMIT_ALLOWED.key(), Value::Bool(on));
}

/// Lets every client of the share transmit, or none.
fn grant_all(e: &Engine, share: OwnerRef, on: bool) {
    for c in clients(e, share) {
        e.owner(share, |o| {
            GRANT.call(
                o,
                &GrantArgs {
                    client: c.id,
                    steer: None,
                    transmit: Some(on),
                },
            )
        })
        .unwrap()
        .unwrap();
    }
    assert!(clients(e, share).iter().all(|c| c.may_transmit == on));
}

#[test]
fn a_client_transmits_only_with_a_grant_and_the_transmit_switch() {
    use sdrkit_core::SettingSet;
    use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts as IiodOpts};
    use sdrkit_driver_core::{tx_schema, DriverKind, SourceDriver};
    use sdrkit_driver_soapyremote::{SoapyRemoteDriver, SoapyTxSetting};

    let net = FakeTransport::new();
    // A board that transmits, and plays its transmit samples at the sample rate.
    let board = FakeIiod::start_on(
        net.clone(),
        IiodOpts {
            tx_plays: true,
            ..IiodOpts::default()
        },
    );
    let e = engine_with(Arc::new(net.clone()), no_usb());
    let kind = e.find("driver.iiod").unwrap();
    let radio = e
        .open_radio("iiod", &board.addr(), &defaults(&e, kind))
        .unwrap();
    set(&e, OwnerRef::Radio(radio), "tx_power", Value::F64(-10.0));
    let (_, sh) = share(&e, radio);
    let (_, port) = listen(&e, sh, ShareProtocol::SoapyRemote);

    // A SoapyRemote client: our own driver, on the same network.
    let driver = SoapyRemoteDriver::new(Arc::new(net.clone()));
    let schema = DriverKind::new(Box::new(driver.clone())).schema().clone();
    let mut v = Values::defaults(&schema);
    v.set(schema.id("serverlog").unwrap(), &Value::Bool(false))
        .unwrap();
    let mut src = driver.open(&format!("127.0.0.1:{port}"), &v).unwrap();
    let limits = src
        .capabilities()
        .transmit
        .clone()
        .expect("the share offers transmit");
    let tx_schema = tx_schema(SoapyTxSetting::SETTINGS, &limits);
    let mut tv = Values::defaults(&tx_schema);
    for (key, value) in [
        ("tx_frequency", Value::Hz(433.92e6)),
        ("tx_rate", Value::Hz(2.5e6)),
        ("tx_power", Value::F64(-10.0)),
    ] {
        tv.set(tx_schema.id(key).unwrap(), &value).unwrap();
    }
    let mut tx = src.transmitter(&tv).unwrap();
    tx.configure(&tv).unwrap();
    // The driver has a connection for its calls and one for its transmit stream: each is a client of the share.
    let connected = wait_for(10.0, || (clients(&e, sh).len() >= 2).then_some(()));
    assert!(connected.is_some(), "{:?}", clients(&e, sh));

    // No grant and no switch: refused. Each of the two alone: refused.
    assert!(
        tx.arm().is_err(),
        "transmit without a grant and with the switch off"
    );
    switch(&e, true);
    assert!(tx.arm().is_err(), "transmit without a grant");
    switch(&e, false);
    grant_all(&e, sh, true);
    assert!(tx.arm().is_err(), "transmit with the switch off");
    // The counts of the share follow at the next tick.
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(count(&e, sh, "tx_taken"), 0);
    assert_eq!(count(&e, sh, "tx_refused"), 3);
    assert!(board.board.tx_bytes.lock().unwrap().is_empty());

    // The grant and the switch: the burst of the client goes out through the transmitter of the radio.
    switch(&e, true);
    tx.arm().unwrap();
    let n = 48_000usize;
    let iq: Vec<Complex32> = (0..n)
        .map(|k| Complex32::from_polar(0.5, k as f32 * 0.05))
        .collect();
    let mut sent = 0;
    while sent < n {
        sent += tx.push(&iq[sent..], Duration::from_secs(2)).unwrap();
    }
    // The flush sends what is queued, filled up to a whole packet: the transmitter is disarmed after it arrived.
    tx.flush().unwrap();
    let moved = wait_for(10.0, || {
        (count(&e, sh, "tx_samples") >= n as i64).then_some(())
    });
    assert!(
        moved.is_some(),
        "the share moved {} of {n} samples",
        count(&e, sh, "tx_samples")
    );
    tx.disarm();
    let own = e
        .owner(sh, |o| {
            ShareFields::read(o.results().fields())
                .map(|f| f.session.to_owned())
                .unwrap()
        })
        .unwrap();
    let own = e.find(&own).unwrap();
    let on_air = wait_for(20.0, || {
        (number(&e, own, "session.tx_sent") >= 1.0
            && board.board.tx_bytes.lock().unwrap().len() / 4 >= n)
            .then_some(())
    });
    let mut bursts = Vec::new();
    e.take_events(own, &mut bursts).unwrap();
    let why: Vec<String> = bursts
        .iter()
        .filter(|b| b.is(&sdrkit_host_engine::TRANSMIT))
        .map(|b| format!("{:?}", b.fields.get(&sdrkit_host_engine::BURST_REFUSED)))
        .collect();
    assert!(
        on_air.is_some(),
        "{} bursts sent, {} failed ({why:?}), the board got {} samples, the share moved {}: {:?}",
        number(&e, own, "session.tx_sent"),
        number(&e, own, "session.tx_failed"),
        board.board.tx_bytes.lock().unwrap().len() / 4,
        count(&e, sh, "tx_samples"),
        field(&e, own, "session.tx_blocked"),
    );
    assert!(e.settle(Duration::from_secs(5)));
    assert_eq!(count(&e, sh, "tx_taken"), 1);
    assert!(count(&e, sh, "tx_samples") < n as i64 + 2000);
    assert_eq!(number(&e, own, "session.tx_failed"), 0.0);
    src.stop();
}
