//! rsp_tcp tests: the pure rules, and the client against a fake rsp_tcp server on a fake network.

mod fake;

use super::*;
use crate::model::{split_gain, Antenna, GainSplit, Model, Stages};
use crate::proto::{cmd, command, command_i, Head};
use fake::{Fake, FakeServer, Flow};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Choice, Owner, SettingError, Value};
use sdrkit_driver_core::{ring, DriverKind, Link, Retuned, RingReader, Started};

const M: f64 = 1e6;

// ---- Byte strings ----

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

fn cmds(list: &[&str]) -> Vec<[u8; 5]> {
    list.iter()
        .map(|s| {
            let v = hex(s);
            [v[0], v[1], v[2], v[3], v[4]]
        })
        .collect()
}

fn show(c: &[[u8; 5]]) -> String {
    c.iter()
        .map(|c| {
            c.iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// RSP1A, version 1.0, capabilities 0x2B, 8-bit.
const RSP1A: &str = "52 53 50 30 00 01 00 00 00 00 00 2B 00 00 00 00 00 00 00 FF 00 00 00 01";
/// RSP2, version 1.2, capabilities 0x1B.
const RSP2: &str = "52 53 50 30 00 01 00 02 00 00 00 1B 00 00 00 00 00 00 00 02 00 00 00 01";
/// RSPdx, capabilities 0.
const RSPDX: &str = "52 53 50 30 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 04 00 00 00 01";
/// RSP0 with version 0.
const VERSION_0: &str = "52 53 50 30 00 00 00 00 00 00 00 00";
/// A plain rtl_tcp greeting.
const RTL: &str = "52 54 4C 30 00 00 00 05 00 00 00 1D";
/// RSP1A, 16-bit.
const RSP1A_16: &str = "52 53 50 30 00 01 00 00 00 00 00 2B 00 00 00 00 00 00 00 FF 00 00 00 02";
/// RSP1A, sample format 9.
const FORMAT_9: &str = "52 53 50 30 00 01 00 00 00 00 00 2B 00 00 00 00 00 00 00 FF 00 00 00 09";
/// Not printable.
const BINARY: &str = "00 01 02 03 04 05 06 07 08 09 0A 0B";
/// A big word after a magic that is not RSP0.
const BIG_WORD: &str = "41 42 43 44 00 01 00 00 00 00 00 01";
/// Model 99.
const MODEL_99: &str = "52 53 50 30 00 01 00 00 00 00 00 2B 00 00 00 00 00 00 00 63 00 00 00 01";

/// The open of an RSP1A at 100 MHz, 2.048 Msps, gain 60, byte for byte.
const OPEN: &[&str] = &[
    "03 00 00 00 01",
    "22 00 00 00 00",
    "02 00 1F 40 00",
    "05 00 00 00 00",
    "01 05 F5 E1 00",
    "24 00 00 00 00",
    "20 00 00 00 03",
    "21 00 00 00 2D",
];

// ---- Helpers ----

fn driver(srv: &FakeServer) -> RspTcpDriver {
    RspTcpDriver::new(Arc::new(srv.net()))
}

fn timed(srv: &FakeServer, timing: Timing) -> RspTcpDriver {
    RspTcpDriver::with_timing(Arc::new(srv.net()), timing)
}

fn choice<C: Choice>(c: C) -> Value {
    Value::Choice(c.index())
}

/// The values of a radio at `centre`, the rate `rate`, manual gain `gain`, and the settings `set`.
fn tune(rate: Hz, centre: Hz, gain: f64, set: &[(&str, Value)]) -> Values {
    let schema = DESCRIPTOR.decl().schema().clone();
    let mut v = Values::defaults(&schema);
    let mut all = vec![
        ("centre_hz", Value::Hz(centre)),
        ("rate", Value::Hz(rate)),
        ("gain", Value::F64(gain)),
        ("agc", Value::Bool(false)),
    ];
    all.extend(set.iter().cloned());
    for (key, value) in &all {
        v.set(schema.id(key).unwrap(), value).unwrap();
    }
    v
}

fn base() -> Values {
    tune(2.048 * M, 100e6, 60.0, &[])
}

fn with(v: &Values, key: &str, value: Value) -> Values {
    let schema = DESCRIPTOR.decl().schema().clone();
    let mut v = v.clone();
    v.set(schema.id(key).unwrap(), &value).unwrap();
    v
}

fn results() -> Results {
    Results::new(DESCRIPTOR.decl().results().clone())
}

fn wait_for(mut what: impl FnMut() -> bool, secs: f64) -> bool {
    let t = Instant::now();
    while t.elapsed().as_secs_f64() < secs {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    what()
}

/// Waits for `n` commands on connection `i`, then a little more to be sure nothing else follows.
fn settle(srv: &FakeServer, i: usize, n: usize) -> Vec<[u8; 5]> {
    wait_for(|| srv.cmds(i).len() >= n, 3.0);
    std::thread::sleep(Duration::from_millis(150));
    srv.cmds(i)
}

/// Reads up to `n` bytes of the ring.
fn read_bytes(r: &mut RingReader, n: usize, secs: f64) -> Vec<u8> {
    let size = sdrkit_driver_core::sample::size(r.sample_type());
    let mut v = Vec::with_capacity(n);
    let t = Instant::now();
    while v.len() < n && t.elapsed().as_secs_f64() < secs {
        r.wait(1, Duration::from_millis(20));
        let want = (n - v.len()) / size;
        r.read_with(want, |b| v.extend_from_slice(b));
    }
    v
}

/// The log lines since the last call.
fn logs(src: &mut Box<dyn Source>, res: &mut Results) -> Vec<String> {
    src.poll(res);
    res.log_lines().iter().cloned().collect()
}

fn has(lines: &[String], text: &str) -> bool {
    lines.iter().any(|t| t.contains(text))
}

/// Opens and starts against `srv`, with a ring of the type of the radio.
fn run(drv: &RspTcpDriver, srv: &FakeServer, v: &Values) -> (Box<dyn Source>, RingReader, Started) {
    let mut src = drv.open(&srv.addr(), v).unwrap();
    let (w, r) = ring(1 << 22, src.sample_type());
    let s = src.start(w).unwrap();
    (src, r, s)
}

fn rsp_error(e: &DriverError) -> Option<RspError> {
    match e {
        DriverError::Driver(s) => s.downcast_ref::<RspError>().copied(),
        _ => None,
    }
}

fn refused(s: RspSetting) -> DriverError {
    DriverError::Refused {
        id: sdrkit_driver_core::own_id(s),
    }
}

// ---- Pure rules ----

#[test]
fn command_encoding() {
    let c = |c: u8, v: u32| command(c, v).to_vec();
    assert_eq!(c(cmd::FREQ, 433_920_000), hex("01 19 DD 18 00"));
    assert_eq!(c(cmd::FREQ, 2_000_000_000), hex("01 77 35 94 00"));
    assert_eq!(c(cmd::RATE, 2_000_000), hex("02 00 1E 84 80"));
    assert_eq!(c(cmd::PPM, 25), hex("05 00 00 00 19"));
    assert_eq!(command_i(cmd::PPM, -12).to_vec(), hex("05 FF FF FF F4"));
    assert_eq!(
        command_i(cmd::AGC_SETPOINT, -30).to_vec(),
        hex("23 FF FF FF E2")
    );
    assert_eq!(c(cmd::IFGR, 20), hex("21 00 00 00 14"));
    assert_eq!(c(cmd::IFGR, 59), hex("21 00 00 00 3B"));
    assert_eq!(c(cmd::ANTENNA, 2), hex("1F 00 00 00 02"));
    assert_eq!(c(cmd::REFOUT, 1), hex("26 00 00 00 01"));
}

#[test]
fn greeting_heads() {
    let h = |s: &str| {
        let v = hex(s);
        Head::parse(&v[..12].try_into().unwrap())
    };
    let g = h(RSP1A).unwrap();
    assert!(g.is_rsp() && g.version_ok() && g.w1 > 0xFFFF && g.w2 == 0x2B);
    let g = h(VERSION_0).unwrap();
    assert!(g.is_rsp() && !g.version_ok());
    assert!(!h(RTL).unwrap().is_rsp());
    assert_eq!(h(BINARY), None);
    assert_eq!(
        h(BIG_WORD),
        None,
        "big words after a magic that is not RSP0"
    );
    assert_eq!(proto::version(0x0001_0002), (1, 2));
}

type Row = (Model, bool, f64, f64, GainSplit, u32, u32, u32);

#[test]
fn the_split_table() {
    use GainSplit::*;
    let rows: &[Row] = &[
        (Model::Rsp1a, false, 100e6, 103.0, Sensitivity, 0, 0, 20),
        (Model::Rsp1a, false, 100e6, 90.0, Sensitivity, 0, 0, 33),
        (Model::Rsp1a, false, 100e6, 60.0, Sensitivity, 3, 18, 45),
        (Model::Rsp1a, false, 100e6, 60.4, Sensitivity, 3, 18, 45),
        (Model::Rsp1a, false, 100e6, 60.0, Linearity, 7, 38, 25),
        (Model::Rsp1a, false, 100e6, 0.0, Sensitivity, 9, 62, 59),
        (Model::Rsp1a, false, 100e6, 200.0, Sensitivity, 0, 0, 20),
        (Model::Rsp1a, false, 500e6, 60.0, Sensitivity, 2, 13, 50),
        (Model::Rsp1a, false, 1500e6, 30.0, Sensitivity, 7, 43, 50),
        (Model::Rsp1, false, 100e6, 50.0, Sensitivity, 2, 19, 33),
        (Model::Rsp2, false, 7e6, 40.0, Sensitivity, 5, 34, 49),
        (Model::Rsp2, true, 7e6, 40.0, Sensitivity, 1, 6, 50),
        (Model::RspDx, false, 100e6, 60.0, Sensitivity, 9, 33, 50),
    ];
    for &(m, hiz, f, g, s, lna, red, ifgr) in rows {
        assert_eq!(
            split_gain(m, hiz, f, g, s, -1, -1),
            Stages {
                lna_state: lna,
                lna_reduction: red,
                ifgr
            },
            "{m:?} hiz {hiz} {f} Hz gain {g} {s:?}"
        );
    }
    assert_eq!(Model::Rsp1.gmax(false), 82.0);
    assert_eq!(Model::Rsp1a.gmax(false), 103.0);
    assert_eq!(Model::Rsp1b.gmax(false), 103.0);
    assert_eq!(Model::Rsp2.gmax(false), 103.0);
    assert_eq!(Model::RspDuo.gmax(false), 103.0);
    assert_eq!(Model::Unknown(99).gmax(false), 103.0);
    assert_eq!(Model::Rsp2.gmax(true), 76.0);
    assert_eq!(Model::RspDuo.gmax(true), 76.0);
    assert_eq!(Model::RspDx.gmax(false), 123.0);
    assert_eq!(Model::RspDxR2.gmax(false), 123.0);
    // Band edges belong to the band above.
    assert_eq!(Model::Rsp1a.band(false, 60e6).len(), 10);
    assert_eq!(Model::Rsp1a.band(false, 59.999e6).len(), 7);
    assert_eq!(Model::Rsp1a.band(false, 420e6)[1], 7);
    assert_eq!(Model::Rsp1b.band(false, 50e6).len(), 10);
    // By hand: a state past the last of the band is the last. The IF is held to its range.
    assert_eq!(
        split_gain(Model::Rsp1a, false, 100e6, 60.0, Sensitivity, 12, 70),
        Stages {
            lna_state: 9,
            lna_reduction: 62,
            ifgr: 59
        }
    );
    assert_eq!(
        split_gain(Model::Rsp1a, false, 100e6, 60.0, Sensitivity, -1, 5).ifgr,
        20
    );
}

#[test]
fn rates_sent_and_refused() {
    let p = |rate: f64, exact: bool, set: &[(&str, Value)]| {
        let mut all = vec![("rate_exact", Value::Bool(exact))];
        all.extend(set.iter().cloned());
        let v = tune(rate, 100e6, 30.0, &all);
        plan_rate(&Options::from_values(&v), &v)
    };
    assert_eq!(p(10.0 * M, false, &[]), Ok(10_000_000));
    assert_eq!(p(8.0 * M, false, &[]), Ok(8_000_000));
    assert_eq!(p(2.4 * M, false, &[]), Ok(2_400_000));
    assert_eq!(p(0.05 * M, false, &[]), Ok(62_500));
    assert_eq!(p(0.0, false, &[]), Ok(2_048_000));
    assert_eq!(
        p(12.0 * M, false, &[]),
        Err(DriverError::Refused {
            id: RadioSetting::Rate.id()
        })
    );
    assert_eq!(p(12.0 * M, true, &[]), Ok(12_000_000));
    assert_eq!(
        p(12.0 * M, false, &[("server_rate", Value::Hz(2.4e6))]),
        Ok(2_400_000)
    );
}

#[test]
fn one_schema_for_every_server() {
    let kind = DriverKind::new(Box::new(RspTcpDriver::new(Arc::new(FakeTransport::new()))));
    let schema = kind.schema();
    for k in [
        "protocol",
        "ppm",
        "antenna",
        "gainsplit",
        "lnastate",
        "ifgr",
        "agcsetpoint",
        "rfnotch",
        "dabnotch",
        "amnotch",
        "refout",
        "server_rate",
    ] {
        assert!(schema.id(k).is_ok(), "{k}");
    }
    let info = |model: Model, caps: u32| ServerInfo {
        extended: true,
        magic: *b"RSP0",
        version: Some((1, 0)),
        model: Some(model),
        caps,
        format: SampleFormat::U8,
        tuner: 0x10000,
        gains: caps,
    };
    let c = info(Model::Rsp1a, 0x2B).controls();
    assert!(!c.antenna && c.dab_notch && !c.refout && !c.am_notch);
    let c = info(Model::Rsp2, 0x1B).controls();
    assert!(c.antenna && c.refout && !c.dab_notch);
    assert!(info(Model::RspDuo, 0).controls().am_notch);
    // The capability word removes a control the model has.
    let c = info(Model::RspDx, proto::caps::BIAS).controls();
    assert!(!c.antenna && !c.fm_notch && c.bias);
    // The 0x1F value is the position in the inputs of the model, and none for an input the model lacks.
    let o = |a: Antenna| Options::from_values(&tune(1e6, 1e6, 1.0, &[("antenna", choice(a))]));
    assert_eq!(o(Antenna::HiZ).antenna_index(Model::Rsp2), Some(2));
    assert_eq!(o(Antenna::HiZ).antenna_index(Model::RspDuo), None);
    assert_eq!(o(Antenna::First).antenna_index(Model::RspDuo), Some(0));
    assert_eq!(o(Antenna::C).antenna_index(Model::RspDx), Some(2));
}

#[test]
fn a_setting_out_of_range_is_refused_by_the_schema() {
    // The IF gain reduction runs to 59 dB: 70 is refused, never replaced by the default.
    let schema = DESCRIPTOR.decl().schema().clone();
    let mut v = Values::defaults(&schema);
    let e = v
        .set(own_id_of(RspSetting::Ifgr), &Value::I64(70))
        .unwrap_err();
    assert!(matches!(e, SettingError::OutOfRange { .. }), "{e:?}");
}

fn own_id_of(s: RspSetting) -> sdrkit_core::SettingId {
    sdrkit_driver_core::own_id(s)
}

#[test]
fn bad_addresses_are_final() {
    let drv = RspTcpDriver::new(Arc::new(FakeTransport::new()));
    let e = drv.open(":1234", &base()).err().unwrap();
    assert_eq!(e, DriverError::Address);
    assert!(!e.retryable());
    assert_eq!(drv.descriptor().key, "rsp_tcp");
}

// ---- Greetings and the commands used ----

#[test]
fn extended_rsp1a() {
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)));
    let drv = driver(&srv);
    let mut src = drv.open(&srv.addr(), &base()).unwrap();
    let i = drv.server_info(&srv.addr()).unwrap();
    assert!(i.extended);
    assert_eq!(
        (i.version, i.model, i.caps, i.format),
        (Some((1, 0)), Some(Model::Rsp1a), 0x2B, SampleFormat::U8)
    );
    let c = i.controls();
    assert!(!c.antenna && c.bias && c.fm_notch && c.dab_notch && !c.refout && !c.am_notch);
    let caps = src.capabilities().clone();
    assert_eq!(
        caps.name,
        format!("SDRplay RSP1A at {} (rsp_tcp v1.0)", srv.addr())
    );
    assert_eq!(caps.gain_db, Some((0.0, 103.0, 1.0)));
    assert_eq!((caps.min_freq_hz, caps.max_freq_hz), (1e3, 2e9));
    assert_eq!((caps.rates.min_hz, caps.rates.max_hz), (62_500.0, 10e6));
    assert!(caps.rates.ranges.is_empty());
    assert!(caps.has_bias_tee && caps.has_agc && caps.realtime && caps.transmit.is_none());
    assert_eq!(src.sample_type(), SampleType::Cu8);
    let mut res = results();
    src.poll(&mut res);
    let f = res.fields();
    assert_eq!(f.get(&MODEL), Some("RSP1A"));
    assert_eq!(f.get(&FORMAT), Some(SampleFormat::U8));
    assert_eq!(f.get(&VERSION), Some("1.0"));
    assert_eq!(f.get(&CONTROLS), Some("bias-T, FM/MW notch, DAB notch"));
    // An antenna the model does not have is refused.
    assert_eq!(
        src.apply(&tune(
            2.048 * M,
            100e6,
            60.0,
            &[("antenna", choice(Antenna::B))]
        )),
        Err(refused(RspSetting::Antenna))
    );
    src.stop();
}

#[test]
fn extended_rsp2() {
    let srv = FakeServer::start(Fake::new(&hex(RSP2)));
    let drv = driver(&srv);
    let mut src = drv.open(&srv.addr(), &base()).unwrap();
    let i = drv.server_info(&srv.addr()).unwrap();
    assert_eq!((i.version, i.model), (Some((1, 2)), Some(Model::Rsp2)));
    let c = i.controls();
    assert!(c.antenna && c.bias && c.fm_notch && c.refout && !c.dab_notch);
    assert!(src.capabilities().name.contains("(rsp_tcp v1.2)"));
    assert_eq!(
        src.apply(&with(&base(), "dabnotch", Value::Bool(true))),
        Err(refused(RspSetting::DabNotch))
    );
    src.stop();
}

#[test]
fn capabilities_zero_lets_the_model_decide() {
    let srv = FakeServer::start(Fake::new(&hex(RSPDX)));
    let drv = driver(&srv);
    let mut src = drv.open(&srv.addr(), &base()).unwrap();
    let i = drv.server_info(&srv.addr()).unwrap();
    assert_eq!(i.model, Some(Model::RspDx));
    let c = i.controls();
    assert!(c.antenna && c.bias && c.fm_notch && c.dab_notch && !c.refout && !c.am_notch);
    assert_eq!(src.capabilities().gain_db, Some((0.0, 123.0, 1.0)));
    src.stop();
}

#[test]
fn rsp0_version_0_falls_back_and_keeps_the_stream_aligned() {
    let mut f = Fake::new(&[hex(VERSION_0), hex("00 FF 80 7F")].concat());
    f.gap = Duration::ZERO;
    let srv = FakeServer::start(f);
    let drv = driver(&srv);
    let (mut src, mut r, _) = run(&drv, &srv, &tune(2.048 * M, 100e6, 30.0, &[]));
    assert_eq!(read_bytes(&mut r, 4, 2.0), hex("00 FF 80 7F"));
    let mut res = results();
    let n = logs(&mut src, &mut res);
    let notes = n
        .iter()
        .filter(|t| t.contains("not with the extended greeting"))
        .count();
    assert_eq!(notes, 1, "{n:?}");
    assert!(has(
        &n,
        "(version word 0x00000000): using rtl_tcp commands only"
    ));
    assert!(!drv.server_info(&srv.addr()).unwrap().extended);
    assert_eq!(
        src.capabilities().name,
        format!("rsp_tcp {} (rtl_tcp commands)", srv.addr())
    );
    // The rtl_tcp commands.
    let c = settle(&srv, 0, 6);
    assert_eq!(c[..2], cmds(&["03 00 00 00 01", "08 00 00 00 00"])[..]);
    src.stop();
    // A second open to the same address does not log the fallback again.
    let (mut src, _r, _) = run(&drv, &srv, &tune(2.048 * M, 100e6, 30.0, &[]));
    let mut res = results();
    assert!(!has(
        &logs(&mut src, &mut res),
        "not with the extended greeting"
    ));
    src.stop();
}

#[test]
fn default_mode_rsp_tcp_takes_the_rtl_tcp_commands() {
    let srv = FakeServer::start(Fake::new(&hex(RTL)));
    let drv = driver(&srv);
    let mut src = drv.open(&srv.addr(), &base()).unwrap();
    let i = drv.server_info(&srv.addr()).unwrap();
    assert!(!i.extended);
    assert_eq!((i.tuner, i.gains), (5, 29));
    let caps = src.capabilities();
    assert_eq!(caps.gain_db, Some((0.0, 49.6, 0.1)));
    assert!(caps.has_bias_tee && !caps.has_agc);
    assert_eq!(
        caps.name,
        format!("rsp_tcp {} (rtl_tcp commands)", srv.addr())
    );
    let mut res = results();
    src.poll(&mut res);
    assert_eq!(res.fields().get(&TUNER), Some(5));
    assert_eq!(res.fields().get(&GAINS), Some(29));
    // A control of the RSP commands is refused.
    assert_eq!(
        src.apply(&with(&base(), "lnastate", Value::I64(2))),
        Err(refused(RspSetting::LnaState))
    );
    src.stop();
}

#[test]
fn sixteen_bit_samples_go_to_the_ring_as_they_come() {
    let mut f = Fake::new(&hex(RSP1A_16));
    f.pieces = vec![hex("00 80 FF 7F 00 00 00 40")];
    let srv = FakeServer::start(f);
    let drv = driver(&srv);
    let (mut src, mut r, _) = run(&drv, &srv, &base());
    assert_eq!(
        drv.server_info(&srv.addr()).unwrap().format,
        SampleFormat::S16
    );
    assert_eq!(src.sample_type(), SampleType::Cs16);
    assert_eq!(read_bytes(&mut r, 8, 2.0), hex("00 80 FF 7F 00 00 00 40"));
    let mut res = results();
    src.poll(&mut res);
    assert_eq!(res.fields().get(&FORMAT), Some(SampleFormat::S16));
    src.stop();
}

#[test]
fn an_unknown_format_drops_the_24_bytes() {
    let mut f = Fake::new(&[hex(FORMAT_9), hex("00 FF")].concat());
    f.gap = Duration::ZERO;
    let srv = FakeServer::start(f);
    let drv = driver(&srv);
    let (mut src, mut r, _) = run(&drv, &srv, &base());
    assert_eq!(read_bytes(&mut r, 2, 2.0), hex("00 FF"));
    assert!(
        read_bytes(&mut r, 2, 0.3).is_empty(),
        "only one sample after the 24 bytes"
    );
    let mut res = results();
    assert!(has(&logs(&mut src, &mut res), "(sample format 9)"));
    assert!(!drv.server_info(&srv.addr()).unwrap().extended);
    src.stop();
}

#[test]
fn other_greetings_are_not_servers() {
    for g in [BINARY, BIG_WORD] {
        let srv = FakeServer::start(Fake::new(&hex(g)));
        let t = Instant::now();
        let e = driver(&srv).open(&srv.addr(), &base()).err().unwrap();
        assert_eq!(
            e,
            DriverError::Protocol(sdrkit_driver_core::Protocol::NotThisRadio)
        );
        assert!(!e.retryable());
        assert!(t.elapsed() < Duration::from_millis(400));
        assert_eq!(srv.n_conns(), 1, "no retry");
        assert!(wait_for(|| srv.closed(0), 1.0), "the connection is closed");
    }
}

#[test]
fn no_greeting_waits_10_s_and_can_be_retried() {
    let srv = FakeServer::start(Fake::new(&[]));
    let drv = timed(
        &srv,
        Timing {
            open_attempts: 1,
            ..Default::default()
        },
    );
    let t = Instant::now();
    let e = drv.open(&srv.addr(), &base()).err().unwrap();
    let secs = t.elapsed().as_secs_f64();
    assert!((9.9..11.0).contains(&secs), "{secs}");
    assert_eq!(e, DriverError::Busy);
    assert!(e.retryable());
    assert!(wait_for(|| srv.closed(0), 1.0));
}

#[test]
fn a_missing_greeting_is_tried_again() {
    let srv = FakeServer::start(Fake::new(&[]));
    let drv = timed(
        &srv,
        Timing {
            greeting: Duration::from_millis(200),
            ..Default::default()
        },
    );
    assert!(drv.open(&srv.addr(), &base()).is_err());
    assert_eq!(srv.n_conns(), 4);
}

#[test]
fn a_short_extended_greeting_waits_10_s_in_all() {
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)[..12]));
    let drv = timed(
        &srv,
        Timing {
            open_attempts: 1,
            ..Default::default()
        },
    );
    let t = Instant::now();
    let e = drv.open(&srv.addr(), &base()).err().unwrap();
    let secs = t.elapsed().as_secs_f64();
    assert!((9.9..11.0).contains(&secs), "{secs}");
    assert_eq!(e, DriverError::Busy);
}

#[test]
fn forced_extended_is_final() {
    for g in [VERSION_0, RTL] {
        let srv = FakeServer::start(Fake::new(&hex(g)));
        let e = driver(&srv)
            .open(
                &srv.addr(),
                &tune(
                    2.048 * M,
                    100e6,
                    60.0,
                    &[("protocol", choice(CommandSet::Extended))],
                ),
            )
            .err()
            .unwrap();
        assert_eq!(rsp_error(&e), Some(RspError::NotExtended), "{e:?}");
        assert!(!e.retryable());
        assert_eq!(srv.n_conns(), 1);
    }
}

#[test]
fn forced_compat_takes_the_rest_as_samples() {
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)));
    let drv = driver(&srv);
    let (mut src, mut r, _) = run(
        &drv,
        &srv,
        &tune(
            2.048 * M,
            100e6,
            30.0,
            &[("protocol", choice(CommandSet::Compat))],
        ),
    );
    assert_eq!(read_bytes(&mut r, 12, 2.0), hex(RSP1A)[12..]);
    assert!(!drv.server_info(&srv.addr()).unwrap().extended);
    let c = settle(&srv, 0, 6);
    assert_eq!(c[1], cmds(&["08 00 00 00 00"])[0]);
    src.stop();
}

#[test]
fn an_unknown_model() {
    let srv = FakeServer::start(Fake::new(&hex(MODEL_99)));
    let drv = driver(&srv);
    let (mut src, _r, _) = run(&drv, &srv, &base());
    let i = drv.server_info(&srv.addr()).unwrap();
    assert_eq!(i.model, Some(Model::Unknown(99)));
    assert!(src
        .capabilities()
        .name
        .starts_with("SDRplay RSP (model 99) at"));
    // The RSP1A tables: the same open.
    assert_eq!(settle(&srv, 0, 8), cmds(OPEN), "{}", show(&srv.cmds(0)));
    src.stop();
}

// ---- Opens with the RSP commands ----

#[test]
fn open_and_live_changes() {
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)));
    let drv = driver(&srv);
    let (mut src, _r, s) = run(&drv, &srv, &base());
    assert_eq!((s.rate_hz, s.centre_hz), (2.048e6, 100e6));
    let c = settle(&srv, 0, 8);
    assert_eq!(c, cmds(OPEN), "{}", show(&c));
    // 500 MHz.
    let t = with(&base(), "centre_hz", Value::Hz(500e6));
    assert!(matches!(src.apply(&t).unwrap(), Retuned::Live(s) if s.centre_hz == 500e6));
    let c = settle(&srv, 0, 11);
    assert_eq!(
        c[8..],
        cmds(&["01 1D CD 65 00", "20 00 00 00 02", "21 00 00 00 32"])[..],
        "{}",
        show(&c)
    );
    // 60.4 dB splits as 60.
    src.apply(&with(&t, "gain", Value::F64(60.4))).unwrap();
    assert_eq!(settle(&srv, 0, 11).len(), 11);
    src.stop();
    assert!(
        wait_for(|| srv.closed(0), 1.0),
        "stop closes the connection"
    );

    // Bias-T on, both notches, ppm -12.
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)));
    let drv = driver(&srv);
    let (mut src, _r, _) = run(&drv, &srv, &base());
    settle(&srv, 0, 8);
    let t = tune(
        2.048 * M,
        100e6,
        60.0,
        &[
            ("rfnotch", Value::Bool(true)),
            ("dabnotch", Value::Bool(true)),
            ("ppm", Value::I64(-12)),
            ("bias_tee", Value::Bool(true)),
        ],
    );
    src.apply(&t).unwrap();
    let c = settle(&srv, 0, 11);
    assert_eq!(
        c[8..],
        cmds(&["05 FF FF FF F4", "24 00 00 00 03", "25 00 00 00 01"])[..],
        "{}",
        show(&c)
    );
    src.stop();
    assert!(wait_for(|| srv.closed(0), 1.0));
    let c = srv.cmds(0);
    assert_eq!(c.len(), 12, "{}", show(&c));
    assert_eq!(c[11], cmds(&["25 00 00 00 00"])[0]);
    src.stop();
}

#[test]
fn rsp2_hiz_and_antenna_a() {
    let srv = FakeServer::start(Fake::new(&hex(RSP2)));
    let drv = driver(&srv);
    let t = tune(1.0 * M, 7.1e6, 40.0, &[("antenna", choice(Antenna::HiZ))]);
    let (mut src, _r, _) = run(&drv, &srv, &t);
    let c = settle(&srv, 0, 9);
    assert_eq!(
        c,
        cmds(&[
            "03 00 00 00 01",
            "22 00 00 00 00",
            "02 00 0F 42 40",
            "1F 00 00 00 02",
            "05 00 00 00 00",
            "01 00 6C 56 60",
            "24 00 00 00 00",
            "20 00 00 00 01",
            "21 00 00 00 32",
        ]),
        "{}",
        show(&c)
    );
    let mut res = results();
    src.poll(&mut res);
    assert_eq!(src.capabilities().gain_db, Some((0.0, 76.0, 1.0)));
    // 50 MHz on Hi-Z: the frequency and the note.
    src.apply(&with(&t, "centre_hz", Value::Hz(50e6))).unwrap();
    assert!(has(
        &logs(&mut src, &mut res),
        "the Hi-Z input is specified for 1 kHz to 30 MHz"
    ));
    let c = settle(&srv, 0, 10);
    assert_eq!(c[9..], cmds(&["01 02 FA F0 80"])[..], "{}", show(&c));
    // Antenna A at 7.1 MHz.
    let t3 = tune(1.0 * M, 7.1e6, 40.0, &[("antenna", choice(Antenna::A))]);
    src.apply(&t3).unwrap();
    let c = settle(&srv, 0, 14);
    assert_eq!(
        c[10..],
        cmds(&[
            "1F 00 00 00 00",
            "01 00 6C 56 60",
            "20 00 00 00 05",
            "21 00 00 00 31"
        ])[..],
        "{}",
        show(&c)
    );
    assert!(src.poll(&mut res).capabilities);
    assert_eq!(src.capabilities().gain_db, Some((0.0, 103.0, 1.0)));
    src.stop();
}

#[test]
fn radio_agc() {
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)));
    let drv = driver(&srv);
    let t = tune(
        2.048 * M,
        100e6,
        60.0,
        &[("agc", Value::Bool(true)), ("agcsetpoint", Value::I64(-30))],
    );
    let (mut src, _r, _) = run(&drv, &srv, &t);
    let c = settle(&srv, 0, 8);
    assert_eq!(
        c,
        cmds(&[
            "03 00 00 00 01",
            "22 00 00 00 01",
            "23 FF FF FF E2",
            "02 00 1F 40 00",
            "05 00 00 00 00",
            "01 05 F5 E1 00",
            "24 00 00 00 00",
            "20 00 00 00 03",
        ]),
        "{}",
        show(&c)
    );
    src.apply(&base()).unwrap();
    let c = settle(&srv, 0, 10);
    assert_eq!(
        c[8..],
        cmds(&["22 00 00 00 00", "21 00 00 00 2D"])[..],
        "{}",
        show(&c)
    );
    src.stop();
}

#[test]
fn stages_by_hand() {
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)));
    let drv = driver(&srv);
    let t = tune(
        2.048 * M,
        100e6,
        60.0,
        &[("lnastate", Value::I64(9)), ("ifgr", Value::I64(59))],
    );
    let (mut src, _r, _) = run(&drv, &srv, &t);
    let c = settle(&srv, 0, 8);
    assert_eq!(
        c[6..],
        cmds(&["20 00 00 00 09", "21 00 00 00 3B"])[..],
        "{}",
        show(&c)
    );
    // The band at 100 MHz has 10 LNA states: state 12 is refused.
    assert_eq!(
        src.apply(&with(&t, "lnastate", Value::I64(12))),
        Err(refused(RspSetting::LnaState))
    );
    src.stop();
}

#[test]
fn a_reconnect_sends_everything_again() {
    let mut f = Fake::new(&hex(RSP1A));
    f.drop_first_after = Some(Duration::from_millis(300));
    f.flow = Flow::Obey;
    let srv = FakeServer::start(f);
    let drv = driver(&srv);
    let (mut src, mut r, _) = run(&drv, &srv, &base());
    assert!(wait_for(|| srv.cmds(1).len() >= 8, 4.0), "no reconnect");
    let c = settle(&srv, 1, 8);
    assert_eq!(c, cmds(OPEN), "{}", show(&c));
    r.skip_all();
    assert_eq!(read_bytes(&mut r, 20_000, 3.0).len(), 20_000);
    let mut res = results();
    let n = logs(&mut src, &mut res);
    assert!(has(&n, "reconnected after"), "{n:?}");
    let loss = src.loss().unwrap();
    assert!(loss.link_down > 0, "{loss:?}");
    src.stop();
    assert!(wait_for(|| srv.closed(1), 1.0));
}

// ---- Opens with the rtl_tcp commands ----

#[test]
fn rtl_tcp_open_and_bias_tee() {
    let srv = FakeServer::start(Fake::new(&hex(RTL)));
    let drv = driver(&srv);
    let (mut src, _r, _) = run(&drv, &srv, &tune(2.048 * M, 100e6, 30.0, &[]));
    let c = settle(&srv, 0, 6);
    assert_eq!(
        c,
        cmds(&[
            "03 00 00 00 01",
            "08 00 00 00 00",
            "02 00 1F 40 00",
            "05 00 00 00 00",
            "01 05 F5 E1 00",
            "04 00 00 01 29",
        ]),
        "{}",
        show(&c)
    );
    // Bias-T on, then the gain list.
    let t = tune(2.048 * M, 100e6, 28.0, &[("bias_tee", Value::Bool(true))]);
    src.apply(&t).unwrap();
    src.apply(&with(&t, "gain", Value::F64(60.0))).unwrap();
    let c = settle(&srv, 0, 9);
    assert_eq!(
        c[6..],
        cmds(&["0E 00 00 00 01", "04 00 00 01 18", "04 00 00 01 F0"])[..],
        "{}",
        show(&c)
    );
    src.stop();
    assert!(wait_for(|| srv.closed(0), 1.0));
    let c = srv.cmds(0);
    assert_eq!(
        c.last(),
        Some(&cmds(&["0E 00 00 00 00"])[0]),
        "{}",
        show(&c)
    );
    assert!(c
        .iter()
        .all(|c| !(0x1F..=0x26).contains(&c[0]) && c[0] != 0x09));
}

// ---- Rates ----

#[test]
fn rates_on_the_wire() {
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)));
    let drv = driver(&srv);
    for (rate, exact, want) in [
        (10.0 * M, false, "02 00 98 96 80"),
        (8.0 * M, false, "02 00 7A 12 00"),
        (0.05 * M, false, "02 00 00 F4 24"),
        (12.0 * M, true, "02 00 B7 1B 00"),
    ] {
        let t = tune(rate, 100e6, 60.0, &[("rate_exact", Value::Bool(exact))]);
        let mut src = drv.open(&srv.addr(), &t).unwrap();
        let n = srv.n_conns() - 1;
        let c = settle(&srv, n, 8);
        assert_eq!(c[2], cmds(&[want])[0], "{}", show(&c));
        src.stop();
    }
    // Refused, final.
    let e = drv
        .open(&srv.addr(), &tune(12.0 * M, 100e6, 60.0, &[]))
        .err()
        .unwrap();
    assert_eq!(
        e,
        DriverError::Refused {
            id: RadioSetting::Rate.id()
        }
    );
    assert!(!e.retryable());
    // A live rate change while streaming.
    let mut f = Fake::new(&hex(RSP1A));
    f.flow = Flow::Obey;
    let srv = FakeServer::start(f);
    let drv = driver(&srv);
    let (mut src, mut r, _) = run(&drv, &srv, &base());
    settle(&srv, 0, 8);
    let t = tune(8.0 * M, 100e6, 60.0, &[]);
    assert!(matches!(src.apply(&t).unwrap(), Retuned::Live(s) if s.rate_hz == 8e6));
    let c = settle(&srv, 0, 9);
    assert_eq!(c[8..], cmds(&["02 00 7A 12 00"])[..], "{}", show(&c));
    r.skip_all();
    assert_eq!(read_bytes(&mut r, 200_000, 3.0).len(), 200_000);
    assert_eq!(srv.n_conns(), 1, "no restart");
    // A change to a refused rate is refused before anything is sent.
    assert!(matches!(
        src.apply(&tune(12.0 * M, 100e6, 60.0, &[])),
        Err(DriverError::Refused { .. })
    ));
    src.stop();
}

#[test]
fn a_server_that_ignores_the_rate_is_pinned() {
    let mut f = Fake::new(&hex(RSP1A));
    f.flow = Flow::Own(2.4 * M);
    let srv = FakeServer::start(f);
    let drv = driver(&srv);
    let (mut src, mut r, _) = run(&drv, &srv, &tune(8.0 * M, 100e6, 60.0, &[]));
    let mut res = results();
    let mut polled = Vec::new();
    let t = Instant::now();
    let found = wait_for(
        || {
            r.skip_all();
            polled.push(src.poll(&mut res));
            polled.iter().any(|p| p.store.is_some())
        },
        10.0,
    );
    assert!(found, "{polled:?}");
    assert!(t.elapsed() < Duration::from_secs(8), "{:?}", t.elapsed());
    let p = polled.iter().find(|p| p.store.is_some()).unwrap();
    assert_eq!(
        p.store,
        Some((own_id_of(RspSetting::ServerRate), Value::Hz(2.4 * M)))
    );
    assert!(p.restart);
    assert!(has(
        &res.log_lines().iter().cloned().collect::<Vec<_>>(),
        "the rsp_tcp server sends 2.4 Msps whatever it is asked: the radio is set to that rate"
    ));
    let rates = &src.capabilities().rates;
    assert_eq!(
        (rates.min_hz, rates.max_hz, rates.ranges.clone()),
        (2.4 * M, 2.4 * M, vec![(2.4 * M, 2.4 * M)])
    );
    src.stop();
    // The restart with the stored rate requests exactly that rate.
    let t = tune(8.0 * M, 100e6, 60.0, &[("server_rate", Value::Hz(2.4e6))]);
    let (mut src, _r, s) = run(&drv, &srv, &t);
    assert_eq!(s.rate_hz, 2.4 * M);
    let c = settle(&srv, 1, 8);
    assert_eq!(c[2], cmds(&["02 00 24 9F 00"])[0], "{}", show(&c));
    src.stop();
}

#[test]
fn throughput_and_the_link_note() {
    let mut f = Fake::new(&hex(RSP1A));
    f.flow = Flow::Obey;
    let srv = FakeServer::start(f);
    let drv = timed(
        &srv,
        Timing {
            window: Duration::from_millis(1000),
            ..Default::default()
        },
    );
    let (mut src, mut r, _) = run(&drv, &srv, &tune(8.0 * M, 100e6, 60.0, &[]));
    let mut res = results();
    assert!(has(
        &logs(&mut src, &mut res),
        "rsp_tcp at 8 Msps needs about 128 Mbit/s; a wired gigabit link is safest"
    ));
    let mut readings = Vec::new();
    wait_for(
        || {
            r.skip_all();
            if let Some(t) = drv.throughput(&srv.addr()) {
                if readings.last() != Some(&t) {
                    readings.push(t);
                }
            }
            readings.len() >= 3
        },
        6.0,
    );
    let tp = *readings.last().expect("no throughput");
    assert!((tp.achieved_hz / 8e6 - 1.0).abs() < 0.02, "{readings:?}");
    assert!((tp.mbit_s / 128.0 - 1.0).abs() < 0.02, "{readings:?}");
    assert_eq!(tp.asked_hz, 8e6);
    assert!(tp.total_bytes > 16_000_000);
    src.poll(&mut res);
    let status = &res.status().summary;
    assert!(
        status.starts_with("rsp_tcp: ")
            && status.contains("Mbit/s")
            && status.ends_with("of 8 Msps"),
        "{status}"
    );
    assert!(res.fields().get(&THROUGHPUT).is_some());
    src.stop();
}

#[test]
fn the_link_note_counts_16_bit_samples() {
    let srv = FakeServer::start(Fake::new(&hex(RSP1A_16)));
    let (mut src, _r, _) = run(&driver(&srv), &srv, &tune(4.0 * M, 100e6, 60.0, &[]));
    let mut res = results();
    assert!(has(&logs(&mut src, &mut res), "needs about 128 Mbit/s"));
    src.stop();
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)));
    let (mut src, _r, _) = run(&driver(&srv), &srv, &tune(4.0 * M, 100e6, 60.0, &[]));
    let mut res = results();
    assert!(!has(&logs(&mut src, &mut res), "Mbit/s"));
    src.stop();
}

#[test]
fn frequencies_outside_the_range_are_sent_with_a_note() {
    let srv = FakeServer::start(Fake::new(&hex(RSP1A)));
    let (mut src, _r, _) = run(&driver(&srv), &srv, &base());
    settle(&srv, 0, 8);
    let mut res = results();
    logs(&mut src, &mut res);
    src.apply(&tune(2.048 * M, 2.5e9, 60.0, &[])).unwrap();
    let n = logs(&mut src, &mut res);
    assert!(
        has(
            &n,
            "SDRplay RSPs are specified for 1 kHz to 2 GHz; the server may not reach 2500000 kHz"
        ),
        "{n:?}"
    );
    let c = settle(&srv, 0, 9);
    assert_eq!(c[8], command(cmd::FREQ, 2_500_000_000));
    // Written once per open.
    let before = n.len();
    src.apply(&tune(2.048 * M, 2.6e9, 60.0, &[])).unwrap();
    let n = logs(&mut src, &mut res);
    assert!(
        !n[before..].iter().any(|t| t.contains("specified for")),
        "{n:?}"
    );
    src.stop();
}

// ---- Samples and streaming ----

#[test]
fn eight_bit_samples_go_to_the_ring_as_they_come() {
    let mut f = Fake::new(&hex(RTL));
    f.pieces = vec![hex("00 FF 80 7F")];
    let srv = FakeServer::start(f);
    let (mut src, mut r, _) = run(&driver(&srv), &srv, &base());
    assert_eq!(src.sample_type(), SampleType::Cu8);
    assert_eq!(read_bytes(&mut r, 4, 2.0), hex("00 FF 80 7F"));
    src.stop();
}

fn split_into(bytes: &[u8], sizes: &[usize]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut k = 0;
    while i < bytes.len() {
        let n = sizes[k % sizes.len()].min(bytes.len() - i);
        out.push(bytes[i..i + n].to_vec());
        i += n;
        k += 1;
    }
    out
}

#[test]
fn split_pairs_8_bit() {
    let bytes: Vec<u8> = (0..96u32).map(|i| (i * 37 % 256) as u8).collect();
    let mut f = Fake::new(&hex(RSP1A));
    f.pieces = split_into(&bytes, &[1, 3, 5, 7]);
    f.gap = Duration::from_millis(15);
    let srv = FakeServer::start(f);
    let (mut src, mut r, _) = run(&driver(&srv), &srv, &base());
    assert_eq!(read_bytes(&mut r, 96, 4.0), bytes);
    src.stop();
}

#[test]
fn split_samples_16_bit() {
    let bytes: Vec<u8> = (0..120u32).map(|i| (i * 53 % 256) as u8).collect();
    let mut f = Fake::new(&hex(RSP1A_16));
    f.pieces = split_into(&bytes, &[1, 2, 3, 5, 7]);
    f.gap = Duration::from_millis(15);
    let srv = FakeServer::start(f);
    let (mut src, mut r, _) = run(&driver(&srv), &srv, &base());
    assert_eq!(read_bytes(&mut r, 120, 4.0), bytes);
    src.stop();
}

#[test]
fn a_silent_remote_server_is_a_stall() {
    let mut f = Fake::new(&hex(RSP1A));
    f.pieces = vec![vec![0x80; 4096]];
    let srv = FakeServer::start(f);
    let drv = timed(
        &srv,
        Timing {
            local_as_remote: true,
            ..Default::default()
        },
    );
    let (mut src, _r, _) = run(&drv, &srv, &base());
    let t = Instant::now();
    assert!(wait_for(|| srv.n_conns() >= 2, 6.0), "no reconnect");
    let secs = t.elapsed().as_secs_f64();
    assert!((2.8..5.0).contains(&secs), "{secs}");
    assert!(
        wait_for(|| srv.closed(0), 1.0),
        "the stalled connection is closed"
    );
    src.stop();
}

#[test]
fn a_silent_local_server_is_waited_for() {
    let mut f = Fake::new(&hex(RSP1A));
    f.flow = Flow::Obey;
    f.silence = Some((Duration::from_millis(300), Duration::from_millis(4500)));
    let srv = FakeServer::start(f);
    let (mut src, mut r, _) = run(&driver(&srv), &srv, &tune(0.25 * M, 100e6, 60.0, &[]));
    std::thread::sleep(Duration::from_millis(5300));
    r.skip_all();
    assert_eq!(
        read_bytes(&mut r, 20_000, 2.0).len(),
        20_000,
        "samples resume"
    );
    assert_eq!(srv.n_conns(), 1, "the same connection is kept");
    let mut res = results();
    let n = logs(&mut src, &mut res);
    let waiting = n
        .iter()
        .filter(|t| {
            **t == format!(
                "rsp_tcp: no samples from {} for 3 s, still waiting",
                srv.addr()
            )
        })
        .count();
    assert_eq!(waiting, 1, "{n:?}");
    src.stop();
}

#[test]
fn a_stop_during_the_greeting_wait() {
    let mut f = Fake::new(&hex(RSP1A));
    f.drop_first_after = Some(Duration::from_millis(200));
    f.mute_after_first = true;
    let srv = FakeServer::start(f);
    let (mut src, _r, _) = run(&driver(&srv), &srv, &base());
    assert!(wait_for(|| srv.n_conns() >= 2, 3.0));
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    let r = src.apply(&with(&base(), "centre_hz", Value::Hz(433.92e6)));
    assert!(matches!(r, Ok(Retuned::Live(_))), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(1));
    assert_eq!(src.link(), Link::Away);
    let t = Instant::now();
    src.stop();
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
    assert!(
        wait_for(|| srv.closed(1), 1.0),
        "the pending connection is closed"
    );
}

#[test]
fn a_change_while_away_is_applied_on_reconnect() {
    let mut srv = FakeServer::start({
        let mut f = Fake::new(&hex(RSP1A));
        f.drop_first_after = Some(Duration::from_millis(200));
        f.mute_after_first = true;
        f
    });
    let net = srv.net();
    let (mut src, _r, _) = run(&driver(&srv), &srv, &base());
    std::thread::sleep(Duration::from_millis(250));
    srv.shutdown();
    std::thread::sleep(Duration::from_millis(300));
    src.apply(&with(&base(), "centre_hz", Value::Hz(433.92e6)))
        .unwrap();
    let srv2 = FakeServer::start_on(net, Fake::new(&hex(RSP1A)));
    assert!(wait_for(|| srv2.cmds(0).len() >= 8, 5.0), "no reconnect");
    let c = settle(&srv2, 0, 8);
    assert!(c.contains(&command(cmd::FREQ, 433_920_000)), "{}", show(&c));
    src.stop();
}

#[test]
fn a_refused_connection_is_tried_four_times() {
    let drv = RspTcpDriver::new(Arc::new(FakeTransport::new()));
    let t = Instant::now();
    let e = drv.open("127.0.0.1:1234", &base()).err().unwrap();
    assert_eq!(e, DriverError::Net(sdrkit_core::NetError::Refused));
    let secs = t.elapsed().as_secs_f64();
    assert!((1.4..3.0).contains(&secs), "{secs}");
}

#[test]
fn refout_on_the_rsp2_and_its_close() {
    let srv = FakeServer::start(Fake::new(&hex(RSP2)));
    let drv = driver(&srv);
    let t = base();
    let (mut src, _r, _) = run(&drv, &srv, &t);
    settle(&srv, 0, 9);
    let t2 = tune(
        2.048 * M,
        100e6,
        60.0,
        &[
            ("refout", Value::Bool(true)),
            ("bias_tee", Value::Bool(true)),
        ],
    );
    src.apply(&t2).unwrap();
    let c = settle(&srv, 0, 11);
    assert_eq!(
        c[9..],
        cmds(&["26 00 00 00 01", "25 00 00 00 01"])[..],
        "{}",
        show(&c)
    );
    // Off again is sent once. Then the close has nothing to turn off.
    src.apply(&t).unwrap();
    let c = settle(&srv, 0, 13);
    assert_eq!(
        c[11..],
        cmds(&["26 00 00 00 00", "25 00 00 00 00"])[..],
        "{}",
        show(&c)
    );
    src.apply(&t2).unwrap();
    settle(&srv, 0, 15);
    src.stop();
    assert!(wait_for(|| srv.closed(0), 1.0));
    let c = srv.cmds(0);
    assert_eq!(
        c[15..],
        cmds(&["25 00 00 00 00", "26 00 00 00 00"])[..],
        "{}",
        show(&c)
    );
    src.stop();
}

#[test]
fn rspduo_notches_and_antenna() {
    // RSPduo, capabilities 0: antenna, bias-T, all three notches, clock output.
    let g = "52 53 50 30 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 03 00 00 00 01";
    let srv = FakeServer::start(Fake::new(&hex(g)));
    let drv = driver(&srv);
    let t = tune(
        2.048 * M,
        1e6,
        60.0,
        &[
            ("antenna", choice(Antenna::T1HiZ)),
            ("rfnotch", Value::Bool(true)),
            ("dabnotch", Value::Bool(true)),
            ("amnotch", Value::Bool(true)),
        ],
    );
    let (mut src, _r, _) = run(&drv, &srv, &t);
    let c = settle(&srv, 0, 9);
    assert!(c.contains(&command(cmd::ANTENNA, 2)), "{}", show(&c));
    assert!(c.contains(&command(cmd::NOTCH, 7)), "{}", show(&c));
    assert_eq!(src.capabilities().gain_db, Some((0.0, 76.0, 1.0)));
    let mut res = results();
    logs(&mut src, &mut res);
    assert!(res
        .fields()
        .get(&CONTROLS)
        .is_some_and(|c| c.contains("bias-T (Tuner 2 input only)")));
    // Bias-T on: sent, with the note that it powers Tuner 2 only, once per open.
    let tb = with(&t, "bias_tee", Value::Bool(true));
    src.apply(&tb).unwrap();
    let n = logs(&mut src, &mut res);
    assert!(
        has(&n, "on the RSPduo, bias-T powers the Tuner 2 input only"),
        "{n:?}"
    );
    let c = settle(&srv, 0, 10);
    assert_eq!(c[9], command(cmd::BIAS, 1), "{}", show(&c));
    let before = n.len();
    src.apply(&with(&tb, "centre_hz", Value::Hz(2e6))).unwrap();
    let n = logs(&mut src, &mut res);
    assert!(!n[before..].iter().any(|t| t.contains("Tuner 2")), "{n:?}");
    src.stop();
    // An RSP1 has no notch and no antenna: neither is sent, and a notch is refused.
    let g = "52 53 50 30 00 01 00 00 00 00 00 00 00 00 00 00 00 00 00 01 00 00 00 01";
    let srv = FakeServer::start(Fake::new(&hex(g)));
    let drv = driver(&srv);
    let (mut src, _r, _) = run(&drv, &srv, &tune(2.048 * M, 100e6, 50.0, &[]));
    let c = settle(&srv, 0, 7);
    assert_eq!(
        c,
        cmds(&[
            "03 00 00 00 01",
            "22 00 00 00 00",
            "02 00 1F 40 00",
            "05 00 00 00 00",
            "01 05 F5 E1 00",
            "20 00 00 00 02",
            "21 00 00 00 21",
        ]),
        "{}",
        show(&c)
    );
    assert_eq!(src.capabilities().gain_db, Some((0.0, 82.0, 1.0)));
    assert!(!src.capabilities().has_bias_tee);
    assert_eq!(src.capabilities().min_freq_hz, 10e3);
    assert_eq!(
        src.apply(&tune(
            2.048 * M,
            100e6,
            50.0,
            &[("rfnotch", Value::Bool(true))]
        )),
        Err(refused(RspSetting::RfNotch))
    );
    assert_eq!(
        src.apply(&tune(
            2.048 * M,
            100e6,
            50.0,
            &[("bias_tee", Value::Bool(true))]
        )),
        Err(DriverError::Refused {
            id: RadioSetting::BiasTee.id()
        })
    );
    src.stop();
}

#[test]
fn a_new_sample_format_on_reconnect_is_an_error() {
    let mut srv = FakeServer::start({
        let mut f = Fake::new(&hex(RSP1A));
        f.drop_first_after = Some(Duration::from_millis(200));
        f
    });
    let net = srv.net();
    let (mut src, _r, _) = run(&driver(&srv), &srv, &base());
    std::thread::sleep(Duration::from_millis(100));
    srv.shutdown();
    let srv2 = FakeServer::start_on(net, Fake::new(&hex(RSP1A_16)));
    assert!(wait_for(|| srv2.n_conns() >= 1, 5.0), "no reconnect");
    let mut res = results();
    assert!(wait_for(
        || has(&logs(&mut src, &mut res), "changed its sample format"),
        5.0
    ));
    assert_eq!(src.sample_type(), SampleType::Cu8);
    src.stop();
}
