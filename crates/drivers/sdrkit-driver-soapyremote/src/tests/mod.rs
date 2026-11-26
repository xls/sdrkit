//! Tests of the SoapyRemote driver: the encoding and the rules, then the driver against a fake server on a fake network.

mod discovery;
mod encoding;
pub mod fake;
mod lifecycle;
mod open;
mod pure;
mod streaming;
mod txtests;

use crate::{SoapyRemoteDriver, Timing, DESCRIPTOR};
use fake::Fake;
use sdrkit_core::{Results, Value, Values};
use sdrkit_driver_core::{Source, SourceDriver};
use std::time::{Duration, Instant};

/// Bytes from hexadecimal text.
pub fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

// Replies of the protocol example of an RTL-SDR.
pub const NATIVE: &str = "53 52 50 43 00 00 04 00 00 00 00 28 06 02 00 00 00 03 43 53 38 04 02 00 00 00 08 03 00 10 00 00 00 00 00 00 43 50 52 53";
pub const FORMATS: &str = "53 52 50 43 00 00 04 00 00 00 00 33 09 02 00 00 00 03 06 02 00 00 00 03 43 53 38 06 02 00 00 00 04 43 53 31 36 06 02 00 00 00 04 43 46 33 32 43 50 52 53";
pub const RATES: &str = "53 52 50 43 00 00 04 00 00 00 00 72 08 02 00 00 00 02 07 04 02 00 00 00 12 03 00 1b 77 48 00 00
  00 00 04 02 00 00 00 13 03 00 12 4f 80 00 00 00 00 04 02 00 00 00 00 03 00 00 00 00 00 00 00 00
  07 04 02 00 00 00 14 03 00 1b 77 42 00 00 00 00 04 02 00 00 00 16 03 00 18 6a 00 00 00 00 00 04
  02 00 00 00 00 03 00 00 00 00 00 00 00 00 43 50 52 53";
pub const GAIN: &str = "53 52 50 43 00 00 04 00 00 00 00 3e 07 04 02 00 00 00 00 03 00 00 00 00 00 00 00 00 04 02 00 00 00 06 03 00 18 cc cc cc cc cc cd 04 02 00 00 00 00 03 00 00 00 00 00 00 00 00 43 50 52 53";
pub const SETTING_INFO: &str = "53 52 50 43 00 00 04 00 00 00 00 df 12 02 00 00 00 01 11 06 02 00 00 00 0b 64 69 72 65 63 74 5f
  73 61 6d 70 06 02 00 00 00 01 30 06 02 00 00 00 0f 44 69 72 65 63 74 20 53 61 6d 70 6c 69 6e 67
  06 02 00 00 00 1c 52 54 4c 2d 53 44 52 20 44 69 72 65 63 74 20 53 61 6d 70 6c 69 6e 67 20 4d 6f
  64 65 06 02 00 00 00 00 02 00 00 00 03 07 04 02 00 00 00 00 03 00 00 00 00 00 00 00 00 04 02 00
  00 00 00 03 00 00 00 00 00 00 00 00 04 02 00 00 00 00 03 00 00 00 00 00 00 00 00 09 02 00 00 00
  03 06 02 00 00 00 01 30 06 02 00 00 00 01 31 06 02 00 00 00 01 32 09 02 00 00 00 03 06 02 00 00
  00 03 4f 66 66 06 02 00 00 00 05 49 2d 41 44 43 06 02 00 00 00 05 51 2d 41 44 43 43 50 52 53";

// Requests of the protocol example.
pub const HANGUP: &str = "53 52 50 43 00 00 04 00 00 00 00 16 0f 02 00 00 00 03 43 50 52 53";
pub const VOID_REPLY: &str = "53 52 50 43 00 00 04 00 00 00 00 11 0e 43 50 52 53";
pub const FIND_REPLY: &str = "53 52 50 43 00 00 04 00 00 00 00 b6 0c 02 00 00 00 02 0b 02 00 00 00 03 06 02 00 00 00 06 64 72
  69 76 65 72 06 02 00 00 00 06 72 74 6c 73 64 72 06 02 00 00 00 05 6c 61 62 65 6c 06 02 00 00 00
  20 47 65 6e 65 72 69 63 20 52 54 4c 32 38 33 32 55 20 4f 45 4d 20 3a 3a 20 30 30 30 30 30 30 30
  31 06 02 00 00 00 06 73 65 72 69 61 6c 06 02 00 00 00 08 30 30 30 30 30 30 30 31 0b 02 00 00 00
  02 06 02 00 00 00 06 64 72 69 76 65 72 06 02 00 00 00 05 61 75 64 69 6f 06 02 00 00 00 05 6c 61
  62 65 6c 06 02 00 00 00 09 48 44 41 20 49 6e 74 65 6c 43 50 52 53";
pub const MAKE: &str = "53 52 50 43 00 00 04 00 00 00 00 70 0f 02 00 00 00 01 0b 02 00 00 00 03 06 02 00 00 00 06 64 72
  69 76 65 72 06 02 00 00 00 06 72 74 6c 73 64 72 06 02 00 00 00 06 73 65 72 69 61 6c 06 02 00 00
  00 08 30 30 30 30 30 30 30 31 06 02 00 00 00 16 73 6f 61 70 79 5f 72 65 6d 6f 74 65 5f 6e 6f 5f
  64 65 65 70 65 72 06 02 00 00 00 00 43 50 52 53";
pub const NO_MATCH: &str = "53 52 50 43 00 00 04 00 00 00 00 38 0d 06 02 00 00 00 21 53 6f 61 70 79 53 44 52 3a 3a 44 65 76
  69 63 65 3a 3a 6d 61 6b 65 28 29 20 6e 6f 20 6d 61 74 63 68 43 50 52 53";
pub const SERVER_ID_REQ: &str = "53 52 50 43 00 00 04 00 00 00 00 16 0f 02 00 00 00 14 43 50 52 53";
pub const DRIVER_KEY: &str = "53 52 50 43 00 00 04 00 00 00 00 16 0f 02 00 00 00 64 43 50 52 53";
pub const CHANNELS_RX: &str =
    "53 52 50 43 00 00 04 00 00 00 00 18 0f 02 00 00 00 ca 00 01 43 50 52 53";
pub const NATIVE_REQ: &str =
    "53 52 50 43 00 00 04 00 00 00 00 1d 0f 02 00 00 01 31 00 01 02 00 00 00 00 43 50 52 53";
pub const FORMATS_REQ: &str =
    "53 52 50 43 00 00 04 00 00 00 00 1d 0f 02 00 00 01 30 00 01 02 00 00 00 00 43 50 52 53";
pub const RATES_REQ: &str =
    "53 52 50 43 00 00 04 00 00 00 00 1d 0f 02 00 00 03 8b 00 01 02 00 00 00 00 43 50 52 53";
pub const GAIN_REQ: &str =
    "53 52 50 43 00 00 04 00 00 00 00 1d 0f 02 00 00 02 c3 00 01 02 00 00 00 00 43 50 52 53";
pub const GAIN_MODE_OFF: &str =
    "53 52 50 43 00 00 04 00 00 00 00 1f 0f 02 00 00 02 bd 00 01 02 00 00 00 00 01 00 43 50 52 53";
pub const SET_RATE: &str = "53 52 50 43 00 00 04 00 00 00 00 2c 0f 02 00 00 03 84 00 01 02 00 00 00 00 04 02 00 00 00 16 03 00 12 4f 80 00 00 00 00 43 50 52 53";
pub const GET_RATE: &str =
    "53 52 50 43 00 00 04 00 00 00 00 1d 0f 02 00 00 03 85 00 01 02 00 00 00 00 43 50 52 53";
pub const GET_RATE_REPLY: &str =
    "53 52 50 43 00 00 04 00 00 00 00 1f 04 02 00 00 00 16 03 00 12 4f 80 00 00 00 00 43 50 52 53";
pub const SET_FREQ: &str = "53 52 50 43 00 00 04 00 00 00 00 32 0f 02 00 00 03 20 00 01 02 00 00 00 00 04 02 00 00 00 1b 03 00 17 d7 84 00 00 00 00 0b 02 00 00 00 00 43 50 52 53";
pub const SET_GAIN: &str = "53 52 50 43 00 00 04 00 00 00 00 2c 0f 02 00 00 02 bf 00 01 02 00 00 00 00 04 02 00 00 00 05 03 00 1d b3 33 33 33 33 33 43 50 52 53";
pub const SETUP: &str = "53 52 50 43 00 00 04 00 00 00 00 9c 0f 02 00 00 01 2c 00 01 06 02 00 00 00 03 43 53 38 10 02 00
   00 00 01 02 00 00 00 00 0b 02 00 00 00 03 06 02 00 00 00 0a 72 65 6d 6f 74 65 3a 6d 74 75 06 02
   00 00 00 04 31 35 30 30 06 02 00 00 00 0b 72 65 6d 6f 74 65 3a 70 72 6f 74 06 02 00 00 00 03 75
   64 70 06 02 00 00 00 0d 72 65 6d 6f 74 65 3a 77 69 6e 64 6f 77 06 02 00 00 00 07 34 31 39 34 33
   30 34 06 02 00 00 00 05 35 30 30 30 30 06 02 00 00 00 05 35 30 30 30 31 43 50 52 53";
pub const SETUP_REPLY: &str = "53 52 50 43 00 00 04 00 00 00 00 20 02 00 00 00 00 06 02 00 00 00 05 34 30 30 30 30 43 50 52 53";
pub const ACK0: &str = "00 00 00 18 00 00 00 00 00 00 0a ec 00 00 00 00 00 00 00 00 00 00 00 00";
pub const ACTIVATE: &str = "53 52 50 43 00 00 04 00 00 00 00 2e 0f 02 00 00 01 2e 02 00 00 00 00 02 00 00 00 00 03 00 00 00 00 00 00 00 00 02 00 00 00 00 43 50 52 53";
pub const ACTIVATE_REPLY: &str = "53 52 50 43 00 00 04 00 00 00 00 15 02 00 00 00 00 43 50 52 53";
pub const DEACTIVATE: &str = "53 52 50 43 00 00 04 00 00 00 00 29 0f 02 00 00 01 2f 02 00 00 00 00 02 00 00 00 00 03 00 00 00 00 00 00 00 00 43 50 52 53";
pub const CLOSE_STREAM: &str =
    "53 52 50 43 00 00 04 00 00 00 00 1b 0f 02 00 00 01 2d 02 00 00 00 00 43 50 52 53";
pub const UNMAKE: &str = "53 52 50 43 00 00 04 00 00 00 00 16 0f 02 00 00 00 02 43 50 52 53";
pub const LOG_START: &str = "53 52 50 43 00 00 04 00 00 00 00 16 0f 02 00 00 00 15 43 50 52 53";
pub const LOG_STOP: &str = "53 52 50 43 00 00 04 00 00 00 00 16 0f 02 00 00 00 16 43 50 52 53";
pub const LOG_PACKET: &str = "53 52 50 43 00 00 04 00 00 00 00 2d 00 04 06 02 00 00 00 15 4f 76 65 72 66 6c 6f 77 20 6f 6e 20 74 68 65 20 72 61 64 69 6f 43 50 52 53";
pub const DATA0: &str = "00 00 00 20 00 00 00 00 00 00 00 04 00 00 00 00 00 00 00 00 00 00 00 00 7f 80 00 01 40 c0 ff 00";
pub const OVERFLOW1: &str =
    "00 00 00 18 00 00 00 01 ff ff ff fc 00 00 00 00 00 00 00 00 00 00 00 00";
pub const ACK349: &str = "00 00 00 18 00 00 01 5d 00 00 0a ec 00 00 00 00 00 00 00 00 00 00 00 00";

/// The [`crate::SoapyError`] inside `e`, if it holds one.
pub fn soapy(e: &sdrkit_driver_core::DriverError) -> Option<&crate::SoapyError> {
    match e {
        sdrkit_driver_core::DriverError::Driver(s) => s.downcast_ref::<crate::SoapyError>(),
        _ => None,
    }
}

/// Short timings for tests.
pub fn quick() -> Timing {
    Timing {
        call: Duration::from_secs(3),
        make: Duration::from_secs(3),
        setup: Duration::from_secs(3),
        close_call: Duration::from_millis(500),
        log_start: Duration::from_secs(1),
        first_data: Duration::from_millis(800),
        stall: Duration::from_secs(1),
        slice: Duration::from_millis(100),
        connect_remote: Duration::from_secs(2),
        connect_local: Duration::from_secs(2),
        throughput_window: Duration::from_millis(300),
        stream_attempts: 4,
        retry_gap: Duration::from_millis(100),
        tx_waits: 10,
        sensor_gap: Duration::from_secs(1),
    }
}

/// The values of a radio with the test defaults (100 MHz, 2.4 Msps, gain 29.7 dB, the server log off), then `set` by key.
pub fn values(driver: &dyn SourceDriver, set: &[(&str, Value)]) -> Values {
    let schema = driver.descriptor().decl().schema().clone();
    let mut v = Values::defaults(&schema);
    let defaults = [
        ("centre_hz", Value::Hz(100e6)),
        ("rate", Value::Hz(2.4e6)),
        ("agc", Value::Bool(false)),
        ("gain", Value::F64(29.7)),
        ("serverlog", Value::Bool(false)),
        // The tests compare the samples as they came: the removal of the DC offset is off.
        ("dc_removal", Value::Bool(false)),
        // The fake server streams over UDP; the default transport is TCP.
        (
            "transport",
            Value::Choice(sdrkit_core::Choice::index(crate::StreamProt::Udp)),
        ),
    ];
    for (key, value) in defaults.iter().chain(set) {
        v.set(schema.id(key).unwrap(), value).unwrap();
    }
    v
}

/// A driver with the quick timings on the network of `fake`.
pub fn drv(fake: &Fake) -> SoapyRemoteDriver {
    SoapyRemoteDriver::with_timing(fake.transport(), quick())
}

/// Opens the radio of `fake` with the test values and `set`.
pub fn open(
    fake: &Fake,
    set: &[(&str, Value)],
) -> Result<Box<dyn Source>, sdrkit_driver_core::DriverError> {
    let d = drv(fake);
    d.open(&fake.addr(), &values(&d, set))
}

/// Empty results of a SoapyRemote radio.
pub fn results() -> Results {
    Results::new(DESCRIPTOR.decl().results().clone())
}

/// The log lines since the last poll.
pub fn notes(src: &mut dyn Source) -> Vec<String> {
    let mut r = results();
    src.poll(&mut r);
    r.log_lines().iter().cloned().collect()
}

/// Polls until a log line holds `text`, within `secs`. It returns every line seen.
pub fn wait_log(src: &mut dyn Source, secs: f64, text: &str) -> (bool, Vec<String>) {
    let t = Instant::now();
    let mut seen = Vec::new();
    loop {
        seen.extend(notes(src));
        if seen.iter().any(|l| l.contains(text)) {
            return (true, seen);
        }
        if t.elapsed().as_secs_f64() > secs {
            return (false, seen);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
