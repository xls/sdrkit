//! Make, the errors of an open, and the open transcript.

use super::fake::{self, Fake, Script};
use super::*;
use crate::rpc::{self, call, Kwargs, Packer};
use crate::{SoapyError, PER_DATAGRAM, RPC_VERSION, SERVER_ID, WIRE_FORMAT};
use sdrkit_core::net::NetError;
use sdrkit_driver_core::{ring, DriverError, Protocol};
use sdrkit_graph::stream::SampleType;
use std::sync::atomic::Ordering;

#[test]
fn make_sends_the_arguments_and_the_no_deeper_key() {
    let f = Fake::start(Script::rtl());
    let src = open(
        &f,
        &[(
            "remote",
            Value::Text("driver=rtlsdr, serial=00000001".into()),
        )],
    )
    .unwrap();
    let reqs = f.control();
    assert_eq!(reqs[0].bytes, hex(SERVER_ID_REQ));
    assert_eq!(reqs[1].bytes, hex(MAKE));
    assert_eq!(reqs[1].bytes.len(), 112);
    drop(src);
}

#[test]
fn no_match_is_final_and_busy_is_retried() {
    let mut e = Packer::default();
    e.exception("SoapySDR::Device::make() no match");
    assert_eq!(rpc::packet(&e.buf, rpc::VERSION), hex(NO_MATCH));

    let rtl = [("remote", Value::Text("rtlsdr".into()))];
    let mut s = Script::rtl();
    s.make_fail = Some((99, "SoapySDR::Device::make() no match".into()));
    let f = Fake::start(s);
    let err = open(&f, &rtl).err().unwrap();
    assert_eq!(soapy(&err), Some(&SoapyError::NoMatch));
    assert_eq!(f.sh.makes.load(Ordering::SeqCst), 1, "no retry");
    assert!(
        f.wait(Duration::from_secs(2), |f| f.sh.open.load(Ordering::SeqCst)
            == 0)
    );
    assert_eq!(*f.calls().last().unwrap(), call::HANGUP);

    // An empty Remote driver: the server provides no radio.
    let mut s = Script::rtl();
    s.make_fail = Some((99, "SoapySDR::Device::make() no match".into()));
    let f = Fake::start(s);
    let err = open(&f, &[]).err().unwrap();
    assert_eq!(soapy(&err), Some(&SoapyError::NoRadio));

    let mut s = Script::rtl();
    s.make_fail = Some((99, "usb_claim_interface error -6".into()));
    let f = Fake::start(s);
    assert_eq!(open(&f, &rtl).err(), Some(DriverError::Busy));
    assert_eq!(f.sh.makes.load(Ordering::SeqCst), 4, "4 attempts in all");

    // Busy once, then free: the second attempt opens.
    let mut s = Script::rtl();
    s.make_fail = Some((1, "Device or resource busy".into()));
    let f = Fake::start(s);
    assert!(open(&f, &rtl).is_ok());
    assert_eq!(f.sh.makes.load(Ordering::SeqCst), 2);
}

#[test]
fn a_make_without_answer_times_out_and_sends_nothing_more_on_it() {
    let mut s = Script::rtl();
    s.silent.insert(call::MAKE);
    let f = Fake::start(s);
    let mut t = quick();
    t.make = Duration::from_millis(300);
    let d = crate::SoapyRemoteDriver::with_timing(f.transport(), t);
    let err = d
        .open(
            &f.addr(),
            &values(&d, &[("remote", Value::Text("rtlsdr".into()))]),
        )
        .err()
        .unwrap();
    assert_eq!(err, DriverError::Net(NetError::TimedOut));
    assert!(err.retryable());
    let reqs = f.control();
    for conn in 0..f.sh.conns.load(Ordering::SeqCst) {
        let on: Vec<u32> = reqs
            .iter()
            .filter(|r| r.conn == conn)
            .map(|r| r.call)
            .collect();
        if let Some(i) = on.iter().position(|c| *c == call::MAKE) {
            assert_eq!(
                i + 1,
                on.len(),
                "no request after the make on connection {conn}: {on:?}"
            );
        }
    }
    assert_eq!(Timing::default().make, Duration::from_secs(20));
}

#[test]
fn not_a_soapyremote_server() {
    let mut s = Script::rtl();
    s.header = *b"HTTP";
    let f = Fake::start(s);
    let err = open(&f, &[]).err().unwrap();
    assert_eq!(err, DriverError::Protocol(Protocol::NotThisRadio));
    assert_eq!(f.sh.conns.load(Ordering::SeqCst), 1, "final: no retry");

    let mut s = Script::rtl();
    s.close_after = Some(1);
    let f = Fake::start(s);
    let err = open(&f, &[]).err().unwrap();
    assert_eq!(err, DriverError::Protocol(Protocol::NotThisRadio));

    let mut s = Script::rtl();
    s.trailer = *b"XXXX";
    let f = Fake::start(s);
    let err = open(&f, &[]).err().unwrap();
    assert!(!err.retryable(), "{err}");

    // Nothing listening: a retry, refused.
    let f = Fake::start(Script::rtl());
    let d = drv(&f);
    let err = d.open("127.0.0.1:9", &values(&d, &[])).err().unwrap();
    assert!(err.retryable());
    assert_eq!(err, DriverError::Net(NetError::Refused));
}

#[test]
fn the_open_transcript_of_an_rtl_sdr() {
    // The reply bytes of the protocol example are what the encoder makes.
    assert_eq!(
        rpc::packet(&fake::f64_payload(2.4e6), rpc::VERSION),
        hex(GET_RATE_REPLY)
    );
    assert_eq!(
        rpc::packet(&fake::int(0), rpc::VERSION),
        hex(ACTIVATE_REPLY)
    );
    let mut p = Packer::default();
    p.i32(0).string("40000");
    assert_eq!(rpc::packet(&p.buf, rpc::VERSION), hex(SETUP_REPLY));
    let setup_with = |d: &str, s: &str| {
        let mut args = Kwargs::new();
        args.insert("remote:mtu".into(), "1500".into());
        args.insert("remote:prot".into(), "udp".into());
        args.insert("remote:window".into(), "4194304".into());
        let mut p = Packer::call(call::SETUP_STREAM);
        p.char(1)
            .string("CS8")
            .size_list(&[0])
            .kwargs(&args)
            .string(d)
            .string(s);
        rpc::packet(&p.buf, rpc::VERSION)
    };
    assert_eq!(setup_with("50000", "50001"), hex(SETUP));

    let f = Fake::start(Script::rtl());
    let mut src = open(
        &f,
        &[(
            "remote",
            Value::Text("driver=rtlsdr,serial=00000001".into()),
        )],
    )
    .unwrap();
    assert_eq!(src.sample_type(), SampleType::Cs8);
    let (w, r) = ring(1 << 20, src.sample_type());
    let started = src.start(w).unwrap();
    assert_eq!(started.rate_hz, 2_400_000.0);
    assert_eq!(started.centre_hz, 100e6);
    // The tuning and rate ranges are those that the server reports, not placeholders.
    let placeholders = sdrkit_driver_core::form::check_capabilities(src.capabilities());
    assert!(placeholders.is_empty(), "{placeholders:?}");
    let reqs = f.control();
    let bytes: Vec<Vec<u8>> = reqs.iter().map(|r| r.bytes.clone()).collect();
    // The reading phase, in order.
    let order = [
        SERVER_ID_REQ,
        MAKE,
        DRIVER_KEY,
        CHANNELS_RX,
        NATIVE_REQ,
        FORMATS_REQ,
        RATES_REQ,
        GAIN_REQ,
    ];
    let mut at = 0;
    for o in order {
        let pos = bytes[at..]
            .iter()
            .position(|b| *b == hex(o))
            .unwrap_or_else(|| panic!("missing {o}"));
        at += pos + 1;
    }
    for c in [
        call::LIST_GAINS,
        call::HAS_GAIN_MODE,
        call::HAS_FREQ_CORRECTION,
        call::LIST_ANTENNAS,
        call::SETTING_INFO,
        call::LIST_CLOCK_SOURCES,
        call::LIST_TIME_SOURCES,
        call::BANDWIDTH_RANGE,
        call::LIST_SENSORS,
    ] {
        assert!(reqs.iter().any(|r| r.call == c), "call {c} was not sent");
    }
    // Then exactly this.
    let i = reqs
        .iter()
        .position(|r| r.call == call::SET_SAMPLE_RATE)
        .unwrap();
    let rest: Vec<u32> = reqs[i..].iter().map(|r| r.call).collect();
    assert_eq!(
        rest,
        [
            call::SET_SAMPLE_RATE,
            call::GET_SAMPLE_RATE,
            call::SET_FREQUENCY,
            call::GET_FREQUENCY,
            call::SET_GAIN_MODE,
            call::SET_GAIN,
            call::SETUP_STREAM,
            call::ACTIVATE_STREAM
        ]
    );
    assert_eq!(reqs[i].bytes, hex(SET_RATE));
    assert_eq!(reqs[i + 1].bytes, hex(GET_RATE));
    assert_eq!(reqs[i + 2].bytes, hex(SET_FREQ));
    assert_eq!(reqs[i + 3].call, call::GET_FREQUENCY);
    assert_eq!(reqs[i + 4].bytes, hex(GAIN_MODE_OFF));
    assert_eq!(reqs[i + 5].bytes, hex(SET_GAIN));
    let st = f.stream().unwrap();
    let (dp, sp) = *st.ports.lock().unwrap();
    assert_eq!(
        reqs[i + 6].bytes,
        setup_with(&dp.to_string(), &sp.to_string())
    );
    assert_eq!(reqs[i + 7].bytes, hex(ACTIVATE));
    // The first acknowledgement came between set-up and activate, with the window of the requested buffer.
    let acks = st.acks.lock().unwrap().clone();
    assert_eq!(acks[0].1.to_vec(), hex(ACK0));
    // Samples arrive.
    assert!(r.wait(1000, Duration::from_secs(2)) >= 1000);
    let caps = src.capabilities().clone();
    assert_eq!(
        caps.rates.ranges,
        vec![(225_001.0, 300_000.0), (900_001.0, 3_200_000.0)]
    );
    assert_eq!(caps.gain_db, Some((0.0, 49.6, 0.0)));
    assert_eq!((caps.min_freq_hz, caps.max_freq_hz), (24e6, 1.766e9));
    assert!(caps.transmit.is_none() && !caps.has_bias_tee && caps.realtime && caps.has_agc);
    assert_eq!(
        caps.name,
        "SoapyRemote 127.0.0.1:55132: RTLSDR (Generic RTL2832U OEM)"
    );
    // The readings.
    let mut res = results();
    src.poll(&mut res);
    let fields = res.fields();
    assert_eq!(
        fields.get(&SERVER_ID),
        Some("2f9a3c10-1b2c-4d5e-8f90-a1b2c3d4e5f6")
    );
    assert_eq!(fields.get(&RPC_VERSION), Some("4.0"));
    assert_eq!(fields.get(&WIRE_FORMAT), Some("CS8"));
    assert_eq!(fields.get(&PER_DATAGRAM), Some(714));
    src.stop();
}

#[test]
fn a_bad_setting_is_refused_by_the_schema() {
    // The receive buffer runs from 0.25 to 32 MiB: 64 is refused, never replaced by the default.
    let schema = DESCRIPTOR.decl().schema().clone();
    let mut v = Values::defaults(&schema);
    assert!(v
        .set(schema.id("window").unwrap(), &Value::F64(64.0))
        .is_err());
}
