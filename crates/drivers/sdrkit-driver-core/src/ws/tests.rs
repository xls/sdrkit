//! Tests of the shared WebSocket client: known answers from RFC 6455, the handshake, the reader's rules, and TLS.

use super::*;
use sdrkit_core::net::{FakeTransport, Transport};
use std::io::{Read, Write};

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// A connected pair on a fake network: (client end, server end).
fn pair() -> (Box<dyn Conn>, Box<dyn Conn>) {
    let net = FakeTransport::new();
    let mut l = net.listen("127.0.0.1", 0).unwrap();
    let port = l.local_addr().port();
    let c = net
        .connect("127.0.0.1", port, false, Duration::from_secs(1))
        .unwrap();
    let s = l.accept().unwrap().unwrap();
    (c, s)
}

/// A plain link on `c` with a read tick of 50 ms.
fn plain(mut c: Box<dyn Conn>) -> Link {
    c.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    Link::new(c, false).unwrap()
}

/// A reader and writer on the client side, with a fixed zero mask and a short tick.
fn reader(c: Box<dyn Conn>, max: usize) -> Reader {
    let link = Arc::new(plain(c));
    let w = Arc::new(Writer::new(link.clone(), Some([0; 4])));
    Reader::new(link, w, max, Duration::from_millis(300))
}

/// Reads until a message comes, for at most 2 s.
fn next_msg(r: &mut Reader) -> Result<Vec<u8>, ReadError> {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(2) {
        if let Some(m) = r.poll_message()? {
            return Ok(m.bytes().to_vec());
        }
    }
    panic!("no message in 2 s");
}

// ---- Known answers ----

#[test]
fn the_accept_value_matches_rfc_6455() {
    // RFC 6455 section 1.3 and 4.2.2.
    assert_eq!(
        accept_for("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
    assert_eq!(key_text(b"the sample nonce"), "dGhlIHNhbXBsZSBub25jZQ==");
    let k = key_text(&random_key());
    assert_eq!(k.len(), 24);
    assert!(k.ends_with("=="));
    assert_ne!(random_key(), random_key());
}

#[test]
fn client_frames_match_the_rfc_6455_examples() {
    // RFC 6455 section 5.7: a masked text "Hello" with the key 37 fa 21 3d.
    let mut out = Vec::new();
    encode_frame(&mut out, op::TEXT, b"Hello", [0x37, 0xfa, 0x21, 0x3d]);
    assert_eq!(out, hex("81 85 37 fa 21 3d 7f 9f 4d 51 58"));
    // The masked pong "Hello" of the same section.
    out.clear();
    encode_frame(&mut out, op::PONG, b"Hello", [0x37, 0xfa, 0x21, 0x3d]);
    assert_eq!(out, hex("8a 85 37 fa 21 3d 7f 9f 4d 51 58"));
    // 256 bytes take the 16-bit length, 64 KiB the 64-bit length (section 5.7, the last two examples, masked here).
    out.clear();
    encode_frame(&mut out, op::BINARY, &[0u8; 256], [0; 4]);
    assert_eq!(&out[..4], &hex("82 fe 01 00")[..]);
    out.clear();
    encode_frame(&mut out, op::BINARY, &vec![0u8; 65536], [0; 4]);
    assert_eq!(&out[..10], &hex("82 ff 00 00 00 00 00 01 00 00")[..]);
    // The length edges: 125 is short, 126 and 65535 take 16 bits, 65536 takes 64 bits.
    for (n, b1) in [(125usize, 0xfd), (126, 0xfe), (65535, 0xfe), (65536, 0xff)] {
        out.clear();
        encode_frame(&mut out, op::BINARY, &vec![0u8; n], [0; 4]);
        assert_eq!(out[1], b1, "{n}");
    }
}

#[test]
fn server_frames_of_rfc_6455_read_as_messages() {
    let (c, mut s) = pair();
    let mut r = reader(c, MAX_MESSAGE);
    // Section 5.7: an unmasked "Hello", then "Hello" in two fragments, then 256 bytes with the 16-bit length.
    s.write_all(&hex("81 05 48 65 6c 6c 6f")).unwrap();
    s.write_all(&hex("01 03 48 65 6c 80 02 6c 6f")).unwrap();
    let mut long = hex("82 7e 01 00");
    long.extend((0..256u32).map(|i| i as u8));
    s.write_all(&long).unwrap();
    assert_eq!(next_msg(&mut r).unwrap(), b"Hello");
    assert_eq!(next_msg(&mut r).unwrap(), b"Hello");
    let m = next_msg(&mut r).unwrap();
    assert_eq!(m.len(), 256);
    assert_eq!(m[255], 255);
}

// ---- The handshake ----

#[test]
fn the_upgrade_request_is_exact() {
    assert_eq!(
        upgrade_request("/", "10.0.0.2:5454", "K", &[]),
        "GET / HTTP/1.1\r\nHost: 10.0.0.2:5454\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: K\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n"
    );
    assert!(upgrade_request("/x", "h", "K", &[("Origin", "http://h")])
        .ends_with("Sec-WebSocket-Version: 13\r\nOrigin: http://h\r\n\r\n"));
}

#[test]
fn answers_are_checked() {
    let key = "dGhlIHNhbXBsZSBub25jZQ==";
    let ok = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
               Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
    assert_eq!(check_answer(ok, key), Ok(()));
    let lower = b"HTTP/1.1 101 OK\r\nsec-websocket-accept:   s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
    assert_eq!(check_answer(lower, key), Ok(()));
    let wrong = b"HTTP/1.1 101 OK\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOx=";
    assert_eq!(check_answer(wrong, key), Err(UpgradeError::Mismatch));
    assert_eq!(
        check_answer(b"HTTP/1.1 101 OK", key),
        Err(UpgradeError::NoAccept)
    );
    for extra in [
        "Sec-WebSocket-Extensions: permessage-deflate",
        "Sec-WebSocket-Protocol: chat",
    ] {
        let a = format!(
            "HTTP/1.1 101 OK\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n{extra}"
        );
        assert_eq!(check_answer(a.as_bytes(), key), Err(UpgradeError::Unasked));
    }
    let empty = b"HTTP/1.1 101 OK\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\nSec-WebSocket-Extensions: ";
    assert_eq!(check_answer(empty, key), Ok(()));
    let e = check_answer(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0", key).unwrap_err();
    assert_eq!(e, UpgradeError::Refused { status: Some(404) });
    assert!(!e.retry());
    // The driver error keeps the kind of the fault.
    assert_eq!(
        DriverError::from(e),
        DriverError::Handshake(UpgradeError::Refused { status: Some(404) })
    );
    assert_eq!(
        DriverError::from(UpgradeError::NoAnswer),
        DriverError::Net(NetError::TimedOut)
    );
    assert_eq!(
        check_answer(b"SSH-2.0-OpenSSH", key),
        Err(UpgradeError::NotHttp)
    );
    assert!(UpgradeError::NoAnswer.retry() && UpgradeError::Dropped.retry());
}

/// A server that reads the request and answers `answer` (with `{accept}` replaced), then writes `after`.
fn serve(answer: &'static str, after: Vec<u8>) -> (Box<dyn Conn>, std::thread::JoinHandle<String>) {
    let (c, mut s) = pair();
    let t = std::thread::spawn(move || {
        let mut req = Vec::new();
        let mut b = [0u8; 1];
        while !req.ends_with(b"\r\n\r\n") {
            if s.read(&mut b).unwrap_or(0) == 0 {
                break;
            }
            req.push(b[0]);
        }
        let req = String::from_utf8(req).unwrap();
        let key = req
            .lines()
            .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
            .unwrap_or("")
            .to_string();
        let mut out = answer.replace("{accept}", &accept_for(&key)).into_bytes();
        out.extend_from_slice(&after);
        s.write_all(&out).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        req
    });
    (c, t)
}

const OK: &str = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n";

#[test]
fn a_handshake_keeps_the_bytes_after_the_answer() {
    let (c, t) = serve(OK, hex("81 02 68 69"));
    let up = Upgrade {
        host: "h:1",
        key: Some(*b"the sample nonce"),
        ..Default::default()
    };
    let (mut r, w) = handshake(plain(c), &up, &|| false).unwrap();
    assert_eq!(next_msg(&mut r).unwrap(), b"hi");
    assert!(!w.is_closed());
    let req = t.join().unwrap();
    assert_eq!(
        req,
        "GET / HTTP/1.1\r\nHost: h:1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n"
    );
}

#[test]
fn handshake_failures_say_why() {
    let quick = Upgrade {
        host: "h:1",
        wait: Duration::from_millis(300),
        ..Default::default()
    };
    let run = |answer: &'static str| {
        let (c, t) = serve(answer, Vec::new());
        let e = handshake(plain(c), &quick, &|| false).unwrap_err();
        t.join().unwrap();
        e
    };
    assert_eq!(run(""), UpgradeError::NoAnswer);
    assert_eq!(run("HTTP/1.1 101 OK\r\n"), UpgradeError::Dropped);
    assert_eq!(run("SSH-2.0-OpenSSH_9\r\n"), UpgradeError::NotHttp);
    assert_eq!(
        run("HTTP/1.1 404 Not Found\r\n\r\n"),
        UpgradeError::Refused { status: Some(404) }
    );
    let endless: &'static str =
        Box::leak(format!("HTTP/1.1 101 OK\r\n{}", "x".repeat(9000)).into_boxed_str());
    assert_eq!(run(endless), UpgradeError::NotHttp);
    // A stop ends the wait.
    let (c, t) = serve("", Vec::new());
    let e = handshake(plain(c), &Upgrade::default(), &|| true).unwrap_err();
    assert_eq!(e, UpgradeError::Stopped);
    t.join().unwrap();
}

// ---- The reader ----

#[test]
fn fragments_join_around_a_ping_and_the_ping_is_answered() {
    let (c, mut s) = pair();
    let mut r = reader(c, MAX_MESSAGE);
    // "ab", a ping "hi" between the fragments, then "c".
    s.write_all(&hex("02 02 61 62 89 02 68 69 80 01 63"))
        .unwrap();
    assert_eq!(next_msg(&mut r).unwrap(), b"abc");
    let mut pong = [0u8; 8];
    s.read_exact(&mut pong).unwrap();
    assert_eq!(pong, [0x8a, 0x82, 0, 0, 0, 0, b'h', b'i']);
    // A pong is dropped, and the next text frame is a message.
    s.write_all(&hex("8a 04 70 6f 6e 67 81 01 78")).unwrap();
    assert_eq!(next_msg(&mut r).unwrap(), b"x");
}

#[test]
fn a_close_is_answered_once_and_ends_the_connection() {
    let (c, mut s) = pair();
    let mut r = reader(c, MAX_MESSAGE);
    s.write_all(&hex("88 02 03 e8")).unwrap();
    assert_eq!(next_msg(&mut r).unwrap_err(), ReadError::Closed);
    let mut close = [0u8; 8];
    s.read_exact(&mut close).unwrap();
    assert_eq!(close, [0x88, 0x82, 0, 0, 0, 0, 0x03, 0xe8]);
    // No data frame and no second close go out after it.
    let w = r.writer().clone();
    assert!(w.is_closed());
    assert!(w.text("late").is_err());
    w.close();
    s.set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let mut rest = [0u8; 16];
    assert_eq!(s.read(&mut rest).unwrap_or(0), 0);
}

#[test]
fn bad_frames_end_the_connection() {
    for (bytes, want) in [
        ("82 81 00 00 00 00 01", ReadError::Bad(FrameFault::Masked)),
        ("c2 00", ReadError::Bad(FrameFault::Reserved)),
        ("80 00", ReadError::Bad(FrameFault::NothingToContinue)),
        (
            "02 00 02 00",
            ReadError::Bad(FrameFault::NewInsideFragmented),
        ),
        ("09 00", ReadError::Bad(FrameFault::FragmentedControl)),
        ("89 7e 00 7e", ReadError::Bad(FrameFault::ControlTooLong)),
        ("83 00", ReadError::Bad(FrameFault::UnknownOpcode)),
        ("8b 00", ReadError::Bad(FrameFault::UnknownOpcode)),
    ] {
        let (c, mut s) = pair();
        let mut r = reader(c, MAX_MESSAGE);
        s.write_all(&hex(bytes)).unwrap();
        assert_eq!(next_msg(&mut r).unwrap_err(), want, "{bytes}");
    }
    let (c, s) = pair();
    let mut r = reader(c, MAX_MESSAGE);
    drop(s);
    assert_eq!(next_msg(&mut r).unwrap_err(), ReadError::Closed);
}

#[test]
fn the_size_limit_holds_for_whole_and_fragmented_messages() {
    let (c, mut s) = pair();
    let mut r = reader(c, 1000);
    // Exactly the limit is accepted.
    let mut f = hex("82 7e 03 e8");
    f.extend(vec![7u8; 1000]);
    s.write_all(&f).unwrap();
    assert_eq!(next_msg(&mut r).unwrap().len(), 1000);
    // One byte over is refused, before the payload comes.
    s.write_all(&hex("82 7e 03 e9")).unwrap();
    assert_eq!(next_msg(&mut r).unwrap_err(), ReadError::TooBig);
    // Fragments that add up to more than the limit are refused too.
    let (c, mut s) = pair();
    let mut r = reader(c, 1000);
    let mut f = hex("02 7e 01 f4");
    f.extend(vec![1u8; 500]);
    f.extend(hex("80 7e 01 f5"));
    s.write_all(&f).unwrap();
    assert_eq!(next_msg(&mut r).unwrap_err(), ReadError::TooBig);
    // A 64-bit length far past the limit.
    let (c, mut s) = pair();
    let mut r = reader(c, MAX_MESSAGE);
    let mut f = hex("82 7f");
    f.extend_from_slice(&(2u64 << 20).to_be_bytes());
    s.write_all(&f).unwrap();
    assert_eq!(next_msg(&mut r).unwrap_err(), ReadError::TooBig);
}

#[test]
fn a_quiet_tick_is_not_an_error_but_a_cut_frame_is() {
    let (c, mut s) = pair();
    let mut r = reader(c, MAX_MESSAGE);
    assert!(r.poll_message().unwrap().is_none());
    assert!(!r.partial());
    // A frame split by 150 ms (less than the frame wait of 300 ms) is read whole.
    s.write_all(&hex("82 03 61")).unwrap();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        s.write_all(b"bc").unwrap();
        s
    });
    assert_eq!(next_msg(&mut r).unwrap(), b"abc");
    let mut s = t.join().unwrap();
    // A frame that stops for longer than the frame wait fails.
    s.write_all(&hex("82 03 61")).unwrap();
    assert_eq!(next_msg(&mut r).unwrap_err(), ReadError::Stalled);
}

#[test]
fn read_waits_for_a_message_and_looks_at_the_stop() {
    let (c, mut s) = pair();
    let mut r = reader(c, MAX_MESSAGE);
    let t0 = Instant::now();
    assert_eq!(
        r.read(Duration::from_millis(200), &|| false).unwrap_err(),
        ReadError::Timeout
    );
    assert!(t0.elapsed() >= Duration::from_millis(200));
    assert_eq!(
        r.read(Duration::from_secs(5), &|| true).unwrap_err(),
        ReadError::Stopped
    );
    s.write_all(&hex("81 01 7a")).unwrap();
    assert_eq!(
        r.read(Duration::from_secs(2), &|| false).unwrap(),
        Message::Text(b"z")
    );
}

#[test]
fn a_large_message_grows_the_buffer_once() {
    let (c, mut s) = pair();
    let mut r = reader(c, MAX_MESSAGE);
    let mut f = hex("82 7f 00 00 00 00 00 01 11 70");
    f.extend((0..70_000u32).map(|i| i as u8));
    let t = std::thread::spawn(move || {
        for chunk in f.chunks(1500) {
            s.write_all(chunk).unwrap();
        }
        s
    });
    let m = next_msg(&mut r).unwrap();
    assert_eq!(m.len(), 70_000);
    assert_eq!(m[69_999], 69_999u32 as u8);
    assert_eq!(r.payload_in, 70_000);
    assert_eq!(r.bytes_in, 70_010);
    drop(t.join().unwrap());
}

#[test]
fn the_writer_masks_every_frame_and_counts_its_bytes() {
    let (c, mut s) = pair();
    let link = Arc::new(plain(c));
    let w = Writer::new(link, None);
    w.text("Hello").unwrap();
    w.binary(&[1, 2, 3]).unwrap();
    let mut f = [0u8; 11 + 9];
    s.read_exact(&mut f).unwrap();
    assert_eq!(f[..2], [0x81, 0x85]);
    let mask = [f[2], f[3], f[4], f[5]];
    let text: Vec<u8> = f[6..11]
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ mask[i & 3])
        .collect();
    assert_eq!(text, b"Hello");
    assert_eq!(f[11..13], [0x82, 0x83]);
    assert_eq!(w.bytes_out.load(Ordering::Relaxed), 20);
    w.close();
    let mut close = [0u8; 8];
    s.read_exact(&mut close).unwrap();
    assert_eq!(close[..2], [0x88, 0x82]);
    let m = [close[2], close[3], close[4], close[5]];
    assert_eq!([close[6] ^ m[0], close[7] ^ m[1]], [0x03, 0xe8]);
}

// ---- TLS ----

#[cfg(feature = "tls")]
mod wss {
    use super::*;
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};

    const CA: &[u8] = include_bytes!("testdata/test-ca.pem");
    const CERT: &[u8] = include_bytes!("testdata/test-server.pem");
    const KEY: &[u8] = include_bytes!("testdata/test-server-key.pem");

    /// A TLS WebSocket server on 127.0.0.1 of a fake network that answers the upgrade, sends "hi", and returns the payload of
    /// the first client frame.
    fn server() -> (FakeTransport, u16, std::thread::JoinHandle<Option<Vec<u8>>>) {
        let net = FakeTransport::new();
        let mut l = net.listen("127.0.0.1", 0).unwrap();
        let port = l.local_addr().port();
        let t = std::thread::spawn(move || {
            let certs = vec![CertificateDer::from_pem_slice(CERT).unwrap()];
            let key = PrivateKeyDer::from_pem_slice(KEY).unwrap();
            let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap();
            let start = Instant::now();
            let mut sock = loop {
                match l.accept().ok()? {
                    Some(s) => break s,
                    None if start.elapsed() < Duration::from_secs(5) => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    None => return None,
                }
            };
            sock.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
            let conn = rustls::ServerConnection::new(Arc::new(cfg)).unwrap();
            let mut s = rustls::StreamOwned::new(conn, sock);
            let mut req = Vec::new();
            let mut b = [0u8; 1];
            while !req.ends_with(b"\r\n\r\n") {
                s.read_exact(&mut b).ok()?;
                req.push(b[0]);
            }
            let req = String::from_utf8(req).ok()?;
            let key = req
                .lines()
                .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))?
                .to_string();
            let mut out = OK.replace("{accept}", &accept_for(&key)).into_bytes();
            out.extend(hex("81 02 68 69"));
            s.write_all(&out).ok()?;
            s.flush().ok()?;
            let mut h = [0u8; 6];
            s.read_exact(&mut h).ok()?;
            let n = (h[1] & 0x7f) as usize;
            let mut p = vec![0u8; n];
            s.read_exact(&mut p).ok()?;
            Some(
                p.iter()
                    .enumerate()
                    .map(|(i, b)| b ^ h[2 + (i & 3)])
                    .collect(),
            )
        });
        (net, port, t)
    }

    fn open(
        net: &FakeTransport,
        port: u16,
        trust: &tls::Trust,
    ) -> Result<(Reader, Arc<Writer>), DriverError> {
        let t = tls::RustlsTransport::new(Arc::new(net.clone()), trust)?;
        let c = t.connect("127.0.0.1", port, true, Duration::from_secs(3))?;
        let link = plain_secure(c);
        assert!(link.secure());
        let host = format!("127.0.0.1:{port}");
        let up = Upgrade {
            host: &host,
            ..Default::default()
        };
        Ok(handshake(link, &up, &|| false)?)
    }

    fn plain_secure(mut c: Box<dyn Conn>) -> Link {
        c.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        Link::new(c, true).unwrap()
    }

    fn der(pem: &[u8]) -> Vec<u8> {
        CertificateDer::from_pem_slice(pem).unwrap().to_vec()
    }

    #[test]
    fn wss_works_with_a_trusted_root_and_with_a_pinned_certificate() {
        for trust in [
            tls::Trust::Roots(vec![der(CA)]),
            tls::Trust::Pinned(sdrkit_algo_crypto::sha256(&der(CERT))),
        ] {
            let (net, port, t) = server();
            let (mut r, w) = open(&net, port, &trust).unwrap();
            assert_eq!(next_msg(&mut r).unwrap(), b"hi");
            w.text("over tls").unwrap();
            assert_eq!(t.join().unwrap().unwrap(), b"over tls");
        }
    }

    #[test]
    fn wss_refuses_an_untrusted_certificate() {
        for trust in [
            tls::Trust::System,
            tls::Trust::Pinned([0; 32]),
            tls::Trust::Roots(Vec::new()),
        ] {
            let (net, port, _t) = server();
            assert_eq!(
                open(&net, port, &trust).err(),
                Some(DriverError::Net(NetError::Untrusted)),
                "{trust:?}"
            );
        }
    }

    #[test]
    fn pins_are_read_from_hexadecimal() {
        let d = sdrkit_algo_crypto::sha256(&der(CERT));
        let text: String = d.iter().map(|b| format!("{b:02X}:")).collect();
        assert_eq!(tls::parse_pin(text.trim_end_matches(':')), Some(d));
        let lower: String = d.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(tls::parse_pin(&lower), Some(d));
        assert!(tls::parse_pin(&lower[..62]).is_none());
        assert!(tls::parse_pin(&"zz".repeat(32)).is_none());
    }
}
