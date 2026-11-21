//! Framing and value encoding.

use super::fake::{self, Fake, Script};
use super::*;
use crate::rpc::{
    self, call, read_packet, ArgInfo, ArgType, Conn, Packer, PacketFault, Range, RpcError, Unpacker,
};
use sdrkit_core::net::{Conn as Link, FakeTransport, Transport};
use std::io::Write;

fn f64_bytes(v: f64) -> Vec<u8> {
    let mut p = Packer::default();
    p.f64(v);
    p.buf
}

#[test]
fn every_float_example_encodes_to_its_bytes_and_back() {
    let cases = [
        (0.0, "04 02 00 00 00 00 03 00 00 00 00 00 00 00 00"),
        (0.5, "04 02 00 00 00 00 03 00 10 00 00 00 00 00 00"),
        (1.0, "04 02 00 00 00 01 03 00 10 00 00 00 00 00 00"),
        (-3.0, "04 02 00 00 00 02 03 ff e8 00 00 00 00 00 00"),
        (49.6, "04 02 00 00 00 06 03 00 18 cc cc cc cc cc cd"),
        (128.0, "04 02 00 00 00 08 03 00 10 00 00 00 00 00 00"),
        (2_048_000.0, "04 02 00 00 00 15 03 00 1f 40 00 00 00 00 00"),
        (2_400_000.0, "04 02 00 00 00 16 03 00 12 4f 80 00 00 00 00"),
        (
            100_000_000.0,
            "04 02 00 00 00 1b 03 00 17 d7 84 00 00 00 00",
        ),
    ];
    for (v, h) in cases {
        let b = hex(h);
        assert_eq!(f64_bytes(v), b, "{v}");
        assert_eq!(Unpacker::new(&b, rpc::VERSION).f64(), Ok(v), "{v}");
    }
    // Exact for awkward doubles too.
    for v in [
        29.7,
        -1e-300,
        5e-324,
        1.7976931348623157e308,
        99_999_700.000_9,
        -0.1,
    ] {
        let b = f64_bytes(v);
        assert_eq!(Unpacker::new(&b, rpc::VERSION).f64(), Ok(v), "{v}");
    }
}

#[test]
fn hang_up_and_void_are_exact() {
    let p = rpc::packet(&Packer::call(call::HANGUP).buf, rpc::VERSION);
    assert_eq!(p, hex(HANGUP));
    assert_eq!(p.len(), 22);
    let v = rpc::packet(&[rpc::tag::VOID], rpc::VERSION);
    assert_eq!(v, hex(VOID_REPLY));
    assert_eq!(v.len(), 17);
}

#[test]
fn every_value_type_round_trips() {
    let mut kw = rpc::Kwargs::new();
    kw.insert("b".into(), "2".into());
    kw.insert("a".into(), "1".into());
    let info = ArgInfo {
        key: "k".into(),
        value: "v".into(),
        name: "n".into(),
        description: "d".into(),
        units: "u".into(),
        kind: Some(ArgType::Float),
        range: Range::new(0.0, 10.0, 0.5),
        options: vec!["x".into()],
        option_names: vec!["X".into()],
    };
    let mut p = Packer::default();
    p.char(-5)
        .bool(true)
        .i32(-7)
        .i64(1 << 40)
        .complex(1.5, -2.25)
        .string("héllo")
        .range_list(&[Range::new(1.0, 2.0, 0.25)], rpc::VERSION)
        .string_list(&["a", "b"])
        .f64_list(&[1.0, 2.0])
        .kwargs(&kw)
        .kwargs_list(std::slice::from_ref(&kw))
        .size_list(&[0, 3])
        .arg_info_list(std::slice::from_ref(&info), rpc::VERSION)
        .void();
    let mut u = Unpacker::new(&p.buf, rpc::VERSION);
    assert_eq!(u.char(), Ok(-5));
    assert_eq!(u.bool(), Ok(true));
    assert_eq!(u.i32(), Ok(-7));
    assert_eq!(u.i64(), Ok(1 << 40));
    assert_eq!(u.complex(), Ok((1.5, -2.25)));
    assert_eq!(u.string().unwrap(), "héllo");
    assert_eq!(u.range_list().unwrap(), vec![Range::new(1.0, 2.0, 0.25)]);
    assert_eq!(u.string_list().unwrap(), vec!["a", "b"]);
    assert_eq!(u.f64_list().unwrap(), vec![1.0, 2.0]);
    assert_eq!(u.kwargs().unwrap(), kw);
    assert_eq!(u.kwargs_list().unwrap(), vec![kw.clone()]);
    assert_eq!(u.size_list().unwrap(), vec![0, 3]);
    assert_eq!(u.arg_info_list().unwrap(), vec![info]);
    assert_eq!(u.void(), Ok(()));
    assert_eq!(u.remaining(), 0);
    // KWARGS keys are written in ascending byte order.
    let mut q = Packer::default();
    q.kwargs(&kw);
    let a = q.buf.windows(1).position(|w| w == b"a").unwrap();
    let b = q.buf.windows(1).position(|w| w == b"b").unwrap();
    assert!(a < b);
    // A repeated key: the last one wins. Any order is read.
    let mut r = Packer::default();
    r.buf.push(rpc::tag::KWARGS);
    r.i32(2).string("z").string("1").string("z").string("2");
    assert_eq!(
        Unpacker::new(&r.buf, rpc::VERSION).kwargs().unwrap()["z"],
        "2"
    );
    // A wrong tag is unexpected. A bool byte that is not zero is true.
    assert!(Unpacker::new(&[rpc::tag::INT32, 0, 0, 0, 1], rpc::VERSION)
        .string()
        .is_err());
    assert_eq!(
        Unpacker::new(&[rpc::tag::BOOL, 7], rpc::VERSION).bool(),
        Ok(true)
    );
}

/// A connection to a server that writes `chunks` (byte by byte if `per_byte`), then waits.
fn serve_raw(chunks: Vec<Vec<u8>>, per_byte: bool) -> Box<dyn Link> {
    let net = FakeTransport::new();
    net.serve("127.0.0.1", 9, move |mut s| {
        for c in &chunks {
            if per_byte {
                for b in c {
                    if s.write_all(&[*b]).is_err() {
                        return;
                    }
                }
            } else if s.write_all(c).is_err() {
                return;
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    });
    net.connect("127.0.0.1", 9, false, Duration::from_secs(1))
        .unwrap()
}

#[test]
fn bad_packets_are_refused() {
    let t = Duration::from_secs(2);
    let mut http = hex(VOID_REPLY);
    http[..4].copy_from_slice(b"HTTP");
    assert_eq!(
        read_packet(serve_raw(vec![http], false).as_mut(), t),
        Err(RpcError::NotSoapy)
    );
    let mut bad = hex(VOID_REPLY);
    let n = bad.len();
    bad[n - 4..].copy_from_slice(b"XXXX");
    assert_eq!(
        read_packet(serve_raw(vec![bad], false).as_mut(), t),
        Err(RpcError::Malformed(PacketFault::Trailer))
    );
    let short = hex("53 52 50 43 00 00 04 00 00 00 00 10 43 50 52 53");
    assert_eq!(
        read_packet(serve_raw(vec![short], false).as_mut(), t),
        Err(RpcError::Malformed(PacketFault::Length { len: 16 }))
    );
    // 17 MiB: refused from the header alone, without waiting for a payload that never comes.
    let mut big = hex("53 52 50 43 00 00 04 00");
    big.extend_from_slice(&(17u32 << 20).to_be_bytes());
    let started = Instant::now();
    assert_eq!(
        read_packet(serve_raw(vec![big], false).as_mut(), t),
        Err(RpcError::TooLarge)
    );
    assert!(started.elapsed() < Duration::from_millis(500));
}

#[test]
fn a_reply_in_single_bytes_reads_the_same() {
    let reply = hex(super::FIND_REPLY);
    let whole = read_packet(
        serve_raw(vec![reply.clone()], false).as_mut(),
        Duration::from_secs(2),
    )
    .unwrap();
    let bytes = read_packet(
        serve_raw(vec![reply], true).as_mut(),
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(whole, bytes);
    // And through a fake server writing every reply byte by byte: the radio opens.
    let mut s = Script::rtl();
    s.split = true;
    let f = Fake::start(s);
    let mut src = open(&f, &[]).unwrap();
    assert_eq!(src.capabilities().max_freq_hz, 1.766e9);
    src.stop();
}

#[test]
fn a_range_has_a_step_only_from_version_0x400() {
    let mut p = Packer::default();
    p.range(Range::new(1.0, 2.0, 0.5), 0x300);
    assert_eq!(
        Unpacker::new(&p.buf, 0x300).range(),
        Ok(Range::new(1.0, 2.0, 0.0))
    );
    assert!(
        Unpacker::new(&p.buf, 0x400).range().is_err(),
        "a missing step"
    );
    // Over a connection: the reply's version word decides.
    let mut pl = Packer::default();
    pl.range(Range::new(0.0, 49.6, 0.0), 0x300);
    let old = rpc::packet(&pl.buf, 0x300);
    let new = rpc::packet(&pl.buf, 0x400);
    let mut c = Conn::new(serve_raw(vec![old], false));
    assert_eq!(
        c.call(
            &Packer::call(call::GAIN_RANGE),
            Duration::from_secs(2),
            |u| u.range()
        ),
        Ok(Range::new(0.0, 49.6, 0.0))
    );
    assert_eq!(c.peer_version, Some(0x300));
    let mut c = Conn::new(serve_raw(vec![new], false));
    assert_eq!(
        c.call(
            &Packer::call(call::GAIN_RANGE),
            Duration::from_secs(2),
            |u| u.range()
        ),
        Err(RpcError::Unexpected)
    );
    assert!(c.is_broken());
}

#[test]
fn an_exception_reply_leaves_the_connection_usable() {
    let mut e = Packer::default();
    e.exception("boom");
    let chunks = vec![
        rpc::packet(&e.buf, rpc::VERSION),
        rpc::packet(&fake::int(5), rpc::VERSION),
    ];
    let mut c = Conn::new(serve_raw(chunks, false));
    assert_eq!(
        c.call_void(&Packer::call(1), Duration::from_secs(2)),
        Err(RpcError::Exception("boom".into()))
    );
    assert!(!c.is_broken());
    assert_eq!(
        c.call(&Packer::call(202), Duration::from_secs(2), |u| u.i32()),
        Ok(5)
    );
    // A VOID reply to a call that expects a value is unexpected. Extra values after the result are ignored.
    let chunks = vec![rpc::packet(&[rpc::tag::VOID], rpc::VERSION)];
    let mut c = Conn::new(serve_raw(chunks, false));
    assert_eq!(
        c.call(&Packer::call(100), Duration::from_secs(2), |u| u.string()),
        Err(RpcError::Unexpected)
    );
    let mut two = fake::int(1);
    two.extend(fake::string("extra"));
    let mut c = Conn::new(serve_raw(vec![rpc::packet(&two, rpc::VERSION)], false));
    assert_eq!(
        c.call(&Packer::call(202), Duration::from_secs(2), |u| u.i32()),
        Ok(1)
    );
}
