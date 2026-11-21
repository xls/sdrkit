//! Transmit against the fake server. The fake is the receiver.

use super::fake::{self, Fake, Script};
use super::*;
use crate::rpc::{self, call, Packer, Unpacker};
use crate::stream::END_BURST;
use crate::{SoapyError, SoapySource, SoapyTxSetting};
use sdrkit_core::{Complex32, SettingId, SettingSet};
use sdrkit_driver_core::{ring, tx_schema, Doing, DriverError, RingReader, TxSetting, TxState};
use std::sync::Arc;

fn tx_script(outputs: i32) -> Script {
    let mut s = Script::rtl();
    s.handler = Some(Arc::new(move |n, args| {
        let dir = Unpacker::new(args, rpc::VERSION).char().ok();
        match (n, dir) {
            (call::NUM_CHANNELS, Some(0)) => Some(fake::int(outputs)),
            (call::GAIN_RANGE, Some(0)) => Some(fake::range(-89.75, 0.0, 0.25)),
            (call::SAMPLE_RATE_RANGE, Some(0)) => Some(fake::ranges(&[(0.5e6, 20e6, 0.0)])),
            (call::FREQUENCY_RANGE, Some(0)) => Some(fake::ranges(&[(70e6, 6e9, 0.0)])),
            _ => None,
        }
    }));
    s
}

/// Transmit values: 433.92 MHz, 1 Msps, 10 dB below full power, then the output.
fn txtune(output: i64) -> Values {
    let limits = sdrkit_driver_core::TxLimits {
        power_db: (-89.75, 0.0, 0.25),
        ..Default::default()
    };
    let schema = tx_schema(SoapyTxSetting::SETTINGS, &limits);
    let mut v = Values::defaults(&schema);
    for (key, value) in [
        ("tx_frequency", Value::Hz(433.92e6)),
        ("tx_rate", Value::Hz(1e6)),
        ("tx_power", Value::F64(-10.0)),
        ("txoutput", Value::I64(output)),
    ] {
        v.set(schema.id(key).unwrap(), &value).unwrap();
    }
    v
}

fn output_id() -> SettingId {
    SettingId(TxSetting::SETTINGS.len() as u16 + SoapyTxSetting::Output.id().0)
}

fn rx(f: &Fake) -> (SoapySource, RingReader) {
    let d = drv(f);
    let mut src = d
        .open_source(
            &f.addr(),
            &values(&d, &[("remote", Value::Text("plutosdr".into()))]),
        )
        .unwrap();
    let (w, r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    (src, r)
}

fn dir_of(r: &fake::Req) -> Option<i8> {
    let p = &r.bytes[12..r.bytes.len() - 4];
    let mut u = Unpacker::new(p, rpc::VERSION);
    u.call().ok()?;
    u.char().ok()
}

#[test]
fn a_burst_is_sent_within_the_window_and_ends_with_end_of_burst() {
    let f = Fake::start(tx_script(1));
    let (mut src, _r) = rx(&f);
    let limits = src
        .capabilities()
        .transmit
        .clone()
        .expect("the transmit limits");
    assert_eq!((limits.min_freq_hz, limits.max_freq_hz), (70e6, 6e9));
    assert_eq!((limits.rates.min_hz, limits.rates.max_hz), (0.5e6, 20e6));
    assert_eq!(limits.bandwidth_hz, None);
    let schema = src.tx_schema().expect("the transmit settings");
    let power = schema.setting(TxSetting::Power.id()).unwrap();
    assert_eq!(power.range.map(|r| (r.min, r.max)), Some((-89.75, 0.0)));
    // The radio can drive an amplifier: it starts at the bottom of its range.
    assert_eq!(power.default, Value::F64(-89.75));
    let n = f.control().len();
    let mut tx = src.transmitter(&txtune(1)).unwrap();
    assert_eq!(tx.state(), TxState::Disarmed);
    assert_eq!(tx.capabilities().outputs, vec!["tx1"]);
    assert_eq!(tx.capabilities().limits, limits);
    assert_eq!(limits.power_db, (-89.75, 0.0, 0.25));
    assert_eq!(tx.capabilities().buffer_samples, 357);
    // Opening only reads: nothing set on the transmit side.
    let opened = f.since(n);
    assert!(opened.iter().all(|r| !(matches!(
        r.call,
        call::SET_FREQUENCY | call::SET_GAIN | call::SETUP_STREAM
    ) && dir_of(r) == Some(0))));
    assert!(opened.iter().any(|r| r.call == call::MAKE), "its own make");
    tx.configure(&txtune(1)).unwrap();
    let n = f.control().len();
    tx.arm().unwrap();
    assert_eq!(tx.state(), TxState::Armed);
    let armed = f.since(n);
    let calls: Vec<u32> = armed.iter().map(|r| r.call).collect();
    assert_eq!(
        calls,
        [
            call::SET_SAMPLE_RATE,
            call::GET_SAMPLE_RATE,
            call::SET_FREQUENCY,
            call::SET_GAIN,
            call::SETUP_STREAM,
            call::ACTIVATE_STREAM
        ]
    );
    let mut p = Packer::call(call::SET_GAIN);
    p.dir_ch(0, 0).f64(-10.0);
    assert_eq!(armed[3].bytes, rpc::packet(&p.buf, rpc::VERSION));
    let st = f.stream().unwrap();
    assert_eq!(*st.format.lock().unwrap(), "CS16");
    let iq = vec![Complex32::new(0.5, 0.25); 2000];
    assert_eq!(tx.push(&iq, Duration::from_secs(2)).unwrap(), 2000);
    tx.flush().unwrap();
    assert!(f.wait(Duration::from_secs(3), |_| st
        .received
        .lock()
        .unwrap()
        .iter()
        .map(|d| d.0.elems as u64)
        .sum::<u64>()
        == 2142));
    let got = st.received.lock().unwrap().clone();
    assert_eq!(got.len(), 6);
    assert!(got.iter().all(|d| d.0.elems == 357));
    assert_eq!(got.last().unwrap().0.flags & END_BURST, END_BURST);
    assert!(got[..5].iter().all(|d| d.0.flags == 0));
    assert_eq!(
        &got[0].1[..4],
        &[0x00, 0x40, 0x00, 0x20],
        "round(0.5 x 32767), round(0.25 x 32767)"
    );
    let s = tx.stats();
    assert_eq!((s.sent, s.padded, s.buffers), (2142, 142, 6));
    assert!(s.transmitting_since.is_some());
    // Pushing while disarmed is refused.
    let n = f.control().len();
    tx.disarm();
    assert_eq!(tx.state(), TxState::Disarmed);
    let calls: Vec<u32> = f.since(n).iter().map(|r| r.call).collect();
    assert_eq!(calls, [call::DEACTIVATE_STREAM, call::CLOSE_STREAM]);
    assert_eq!(
        tx.push(&iq, Duration::ZERO),
        Err(DriverError::NotNow {
            state: Doing::Disarmed
        })
    );
    assert_eq!(
        soapy(&tx.play_cyclic(&iq).unwrap_err()),
        Some(&SoapyError::NoCyclic)
    );
    let n = f.control().len();
    tx.close();
    let calls: Vec<u32> = f.since(n).iter().map(|r| r.call).collect();
    assert_eq!(calls, [call::UNMAKE, call::HANGUP]);
    assert_eq!(tx.state(), TxState::Closed);
    src.stop();
}

#[test]
fn transmit_refusals() {
    let f = Fake::start(tx_script(0));
    let (mut src, _r) = rx(&f);
    assert!(src.capabilities().transmit.is_none());
    assert!(src.tx_schema().is_none());
    let err = src.transmitter(&txtune(1)).err().unwrap();
    assert_eq!(err, DriverError::CannotTransmit);
    src.stop();

    let f = Fake::start(tx_script(1));
    let (mut src, _r) = rx(&f);
    let err = src.transmitter(&txtune(2)).err().unwrap();
    assert_eq!(err, DriverError::Refused { id: output_id() });
    src.stop();

    let f = Fake::start(tx_script(2));
    let (mut src, _r) = rx(&f);
    let err = src.transmitter(&txtune(3)).err().unwrap();
    assert_eq!(err, DriverError::Refused { id: output_id() });
    let tx = src.transmitter(&txtune(2)).unwrap();
    assert_eq!(tx.capabilities().outputs, vec!["tx1", "tx2"]);
    drop(tx);
    src.stop();

    // A refused carrier.
    let mut s = tx_script(1);
    let h = s.handler.clone().unwrap();
    s.handler = Some(Arc::new(move |n, args| {
        let dir = Unpacker::new(args, rpc::VERSION).char().ok();
        if n == call::SET_FREQUENCY && dir == Some(0) {
            return Some(fake::exception("out of range"));
        }
        h(n, args)
    }));
    let f = Fake::start(s);
    let (mut src, _r) = rx(&f);
    let mut tx = src.transmitter(&txtune(1)).unwrap();
    let err = tx.arm().unwrap_err();
    assert_eq!(
        err,
        DriverError::Refused {
            id: TxSetting::Frequency.id()
        }
    );
    assert_eq!(tx.state(), TxState::Disarmed);
    src.stop();
}

#[test]
fn a_refusing_or_silent_radio_ends_the_transmission() {
    let mut s = tx_script(1);
    s.stream.tx_status = vec![-2];
    let f = Fake::start(s);
    let (mut src, _r) = rx(&f);
    let mut tx = src.transmitter(&txtune(1)).unwrap();
    tx.arm().unwrap();
    let iq = vec![Complex32::new(0.1, 0.0); 4000];
    let end = Instant::now() + Duration::from_secs(3);
    let mut err = None;
    while Instant::now() < end {
        match tx.push(&iq, Duration::from_millis(50)) {
            Ok(_) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                err = Some(e);
                break;
            }
        }
    }
    assert_eq!(err, Some(DriverError::Device { code: -2 }));
    assert_eq!(tx.state(), TxState::Disarmed);
    src.stop();

    let mut s = tx_script(1);
    s.stream.tx_never_ack = true;
    let f = Fake::start(s);
    let (mut src, _r) = rx(&f);
    let mut tx = src.transmitter(&txtune(1)).unwrap();
    tx.arm().unwrap();
    tx.push(&iq, Duration::ZERO).unwrap();
    // quick(): 10 waits of 100 ms without an acknowledgement.
    std::thread::sleep(Duration::from_millis(1500));
    let err = tx.push(&iq, Duration::ZERO).unwrap_err();
    assert_eq!(soapy(&err), Some(&SoapyError::TxStalled));
    assert!(
        f.stream().unwrap().received.lock().unwrap().is_empty(),
        "nothing sent before an acknowledgement"
    );
    src.stop();
}
