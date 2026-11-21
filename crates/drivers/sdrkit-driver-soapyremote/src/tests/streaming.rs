//! Data, gaps, overflows, flow control and stalls.

use super::fake::{Fake, Script};
use super::*;
use crate::stream::{DgHeader, HEADER_LEN};
use crate::{SoapyError, SoapySource, ARRIVING, MBIT_S};
use sdrkit_core::Complex32;
use sdrkit_driver_core::{ring, RingReader};
use std::sync::atomic::Ordering;

fn datagram(seq: u32, elems: i32, payload: &[u8]) -> Vec<u8> {
    let mut v = DgHeader {
        bytes: (HEADER_LEN + payload.len()) as u32,
        seq,
        elems,
        flags: 0,
        time: 0,
    }
    .to_bytes()
    .to_vec();
    v.extend_from_slice(payload);
    v
}

fn rtl() -> Vec<(&'static str, Value)> {
    vec![("remote", Value::Text("rtlsdr".into()))]
}

fn open_with(s: Script, ring_size: usize) -> (Fake, SoapySource, RingReader) {
    let f = Fake::start(s);
    let d = drv(&f);
    let mut src = d.open_source(&f.addr(), &values(&d, &rtl())).unwrap();
    let (w, r) = ring(ring_size, src.sample_type());
    src.start(w).unwrap();
    (f, src, r)
}

fn read_all(r: &mut RingReader) -> Vec<Complex32> {
    let mut out = vec![Complex32::new(0.0, 0.0); r.available()];
    let n = r.read_iq(&mut out);
    out.truncate(n);
    out
}

#[test]
fn a_data_datagram_and_an_overflow_message() {
    let mut s = Script::rtl();
    s.stream.custom = vec![hex(DATA0), hex(OVERFLOW1)];
    s.stream.stop_after = Some(2);
    let (f, mut src, mut r) = open_with(s, 1 << 16);
    assert!(f.wait(Duration::from_secs(2), |_| src
        .ctx
        .stats
        .overflows
        .load(Ordering::Relaxed)
        == 1));
    // The ring holds the CS8 bytes as they came.
    let mut bytes = Vec::new();
    r.read_with(4, |b| bytes.extend_from_slice(b));
    assert_eq!(bytes, hex(DATA0)[24..]);
    // Read as complex samples, they are these.
    let mut r2 = {
        let mut s = Script::rtl();
        s.stream.custom = vec![hex(DATA0)];
        s.stream.stop_after = Some(1);
        open_with(s, 1 << 16)
    };
    assert!(r2.2.wait(4, Duration::from_secs(2)) >= 4);
    let got = read_all(&mut r2.2);
    r2.1.stop();
    assert_eq!(
        got,
        [
            Complex32::new(0.992_187_5, -1.0),
            Complex32::new(0.0, 0.007_812_5),
            Complex32::new(0.5, -0.5),
            Complex32::new(-0.007_812_5, 0.0)
        ]
    );
    let st = &src.ctx.stats;
    assert_eq!(st.lost_datagrams.load(Ordering::Relaxed), 0);
    assert_eq!(
        st.status(),
        "1 overflow at the server (it or its network is too slow)"
    );
    src.stop();
}

#[test]
fn the_acknowledgement_rhythm() {
    let mut s = Script::rtl();
    s.stream.rate = 50_000.0;
    s.stream.elems = 4;
    let (f, mut src, _r) = open_with(s, 1 << 22);
    let st = f.stream().unwrap();
    let window = crate::plan::window_packets(4 << 20, 1500);
    let trigger = crate::stream::ack_trigger(window);
    assert!(
        f.wait(Duration::from_secs(3), |_| st.acks.lock().unwrap().len()
            >= 2)
    );
    let acks = st.acks.lock().unwrap().clone();
    assert_eq!(acks[0].1, crate::stream::ack(0, window));
    assert_eq!(
        acks[1].1,
        crate::stream::ack(trigger, window),
        "no other acknowledgement while data keeps arriving"
    );
    assert_eq!(acks[1].1.to_vec(), hex(ACK349));
    src.stop();
}

#[test]
fn gaps_late_datagrams_and_short_datagrams() {
    let mut s = Script::rtl();
    let p = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let mut custom: Vec<Vec<u8>> = (0..10).map(|i| datagram(i, 4, &p)).collect();
    custom.push(datagram(13, 4, &p));
    custom.push(datagram(11, 4, &p));
    custom.push(datagram(14, 4, &p));
    // A datagram whose bytes field says 1500 but carries 32 bytes, twice.
    let mut short = datagram(15, 4, &p);
    short[..4].copy_from_slice(&1500u32.to_be_bytes());
    custom.push(short.clone());
    custom.push(short);
    s.stream.custom = custom;
    s.stream.stop_after = Some(15);
    let (f, mut src, mut r) = open_with(s, 1 << 16);
    assert!(f.wait(Duration::from_secs(2), |_| src
        .ctx
        .stats
        .truncated
        .load(Ordering::Relaxed)
        == 2));
    let st = src.ctx.stats.clone();
    assert_eq!(st.lost_datagrams.load(Ordering::Relaxed), 3);
    assert_eq!(st.lost_samples.load(Ordering::Relaxed), 12);
    assert_eq!(st.late.load(Ordering::Relaxed), 1);
    assert_eq!(read_all(&mut r).len(), 12 * 4, "0 to 9, 13 and 14");
    let t = notes(&mut src);
    let lost: Vec<&String> = t
        .iter()
        .filter(|l| l.contains("datagrams lost on the network"))
        .collect();
    assert_eq!(lost.len(), 1, "{t:?}");
    assert_eq!(
        lost[0],
        "3 datagrams lost on the network in the last 10 s (use a wired connection, a lower rate, or a smaller IQ format)"
    );
    assert_eq!(
        t.iter()
            .filter(|l| l.contains("MTU setting may be unachievable"))
            .count(),
        1,
        "{t:?}"
    );
    assert_eq!(st.status(), "3 datagrams lost on the network");
    src.stop();
}

#[test]
fn a_run_of_stream_errors_ends_the_loop_and_reconnects() {
    let mut s = Script::rtl();
    let mut custom = vec![hex(DATA0)];
    custom.extend((1..=51).map(|i| datagram(i, -2, &[])));
    s.stream.custom = custom;
    s.stream.stop_after = Some(52);
    let (f, mut src, _r) = open_with(s, 1 << 16);
    assert!(
        f.wait(Duration::from_secs(5), |f| f
            .sh
            .makes
            .load(Ordering::SeqCst)
            >= 2),
        "reconnected"
    );
    let (found, t) = wait_log(&mut src, 2.0, "the device gave error code -2");
    assert!(found, "{t:?}");
    src.stop();
}

#[test]
fn status_messages_are_counted() {
    let mut s = Script::rtl();
    s.stream.status = vec![-1, -4];
    s.stream.stop_after = Some(200);
    let (f, mut src, _r) = open_with(s, 1 << 20);
    assert!(f.wait(Duration::from_secs(2), |_| src
        .ctx
        .stats
        .errors
        .load(Ordering::Relaxed)
        == 1
        && src.ctx.stats.overflows.load(Ordering::Relaxed) == 1));
    let (found, t) = wait_log(&mut src, 1.0, "the server reports TIMEOUT");
    assert!(found, "{t:?}");
    src.stop();
}

#[test]
fn a_lost_first_acknowledgement_is_sent_again() {
    let mut s = Script::rtl();
    s.stream.ignore_first_ack = true;
    let f = Fake::start(s);
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &values(&d, &rtl())).unwrap();
    let (w, mut r) = ring(1 << 20, src.sample_type());
    let t0 = Instant::now();
    src.start(w).unwrap();
    assert!(
        t0.elapsed() < Duration::from_millis(300),
        "{:?}",
        t0.elapsed()
    );
    assert!(r.wait(100, Duration::from_millis(500)) >= 100);
    let _ = read_all(&mut r);
    assert!(f.stream().unwrap().acks.lock().unwrap().len() >= 2);
    src.stop();
}

#[test]
fn a_silent_server_is_acknowledged_then_given_up_and_reconnected() {
    let mut s = Script::rtl();
    s.stream.stop_after = Some(50);
    s.stream.elems = 4;
    let (f, mut src, _r) = open_with(s, 1 << 20);
    let st = f.stream().unwrap();
    assert!(
        f.wait(Duration::from_secs(2), |_| st.sent.load(Ordering::Relaxed)
            == 50)
    );
    let n = st.acks.lock().unwrap().len();
    std::thread::sleep(Duration::from_millis(250));
    let acks = st.acks.lock().unwrap().clone();
    assert!(acks.len() > n, "acknowledged again after 100 ms");
    let last = DgHeader::parse(&acks.last().unwrap().1).unwrap();
    assert_eq!(last.seq, 50, "the next expected");
    // After the stall time: lost, and a complete reconnect.
    assert!(f.wait(Duration::from_secs(5), |f| f
        .sh
        .streams
        .lock()
        .unwrap()
        .len()
        >= 2
        && f.stream().unwrap().active.load(Ordering::Acquire)));
    assert!(f.sh.makes.load(Ordering::SeqCst) >= 2);
    let stall = format!("SoapyRemote 127.0.0.1:{}: timed out", f.port);
    let (found, mut t) = wait_log(&mut src, 3.0, "reconnected after");
    t.extend(notes(&mut src));
    assert!(found, "{t:?}");
    assert!(t.iter().any(|l| l == &stall), "{t:?}");
    assert!(
        f.wait(Duration::from_secs(2), |_| src
            .ctx
            .stats
            .status()
            .contains("1 reconnect")),
        "{}",
        src.ctx.stats.status()
    );
    src.stop();
}

#[test]
fn a_server_that_never_sends_fails_the_start_with_the_firewall_text() {
    let mut s = Script::rtl();
    s.stream.never_send = true;
    let f = Fake::start(s);
    let mut t = quick();
    t.first_data = Duration::from_millis(300);
    let d = crate::SoapyRemoteDriver::with_timing(f.transport(), t);
    let mut src = d.open(&f.addr(), &values(&d, &rtl())).unwrap();
    let (w, _r) = ring(1 << 16, src.sample_type());
    let err = src.start(w).unwrap_err();
    assert_eq!(soapy(&err), Some(&SoapyError::NoSamples));
    assert_eq!(f.sh.streams.lock().unwrap().len(), 4, "tried again");
    let st = f.stream().unwrap();
    assert!(
        st.acks.lock().unwrap().len() >= 2,
        "acknowledged while waiting"
    );
    src.stop();
}

#[test]
fn a_full_ring_still_reads_and_acknowledges() {
    let mut s = Script::rtl();
    s.stream.rate = 5000.0;
    let (f, mut src, r) = open_with(s, 1000);
    let st = f.stream().unwrap();
    std::thread::sleep(Duration::from_millis(600));
    let a = st.acks.lock().unwrap().len();
    std::thread::sleep(Duration::from_millis(400));
    assert!(st.acks.lock().unwrap().len() > a, "still acknowledged");
    assert!(r.stats().dropped() > 0);
    assert!(r.stats().cpu_lost() > 0);
    // The throughput windows of the radio.
    assert!(
        f.wait(Duration::from_secs(3), |_| {
            crate::lock(&src.ctx.flow.last).is_some_and(|t| t.achieved_hz > 0.0 && t.mbit_s > 0.0)
        }),
        "a throughput window was published"
    );
    assert_eq!(crate::lock(&src.ctx.flow.last).unwrap().asked_hz, 2.4e6);
    let mut res = results();
    src.poll(&mut res);
    assert!(res.fields().get(&MBIT_S).is_some_and(|m| m > 0.0));
    assert!(res.fields().get(&ARRIVING).is_some_and(|a| a > 0.0));
    src.stop();
}

#[test]
fn twelve_bit_samples_in_cs16_reach_the_full_scale() {
    let mut s = Script::rtl();
    let mut native = crate::rpc::Packer::default();
    native.string("CS16").f64(2048.0);
    s.replies
        .insert(crate::rpc::call::NATIVE_FORMAT, native.buf);
    s.stream.custom = vec![datagram(
        0,
        2,
        &[0x00, 0x04, 0x00, 0xfc, 0xff, 0x07, 0x00, 0xf8],
    )];
    s.stream.stop_after = Some(1);
    let (_f, mut src, mut r) = open_with(s, 1 << 16);
    assert!(r.wait(2, Duration::from_secs(2)) >= 2);
    let mut bytes = Vec::new();
    r.read_with(2, |b| bytes.extend_from_slice(b));
    // 1024 and -1024, 2047 and -2048, shifted left by 4.
    assert_eq!(bytes, [0x00, 0x40, 0x00, 0xc0, 0xf0, 0x7f, 0x00, 0x80]);
    src.stop();
}
