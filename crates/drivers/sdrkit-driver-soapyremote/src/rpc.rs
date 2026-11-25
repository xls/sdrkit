//! The SoapyRemote RPC protocol: packet framing, the tagged value encoding, the call numbers, and a control connection that
//! sends one request and waits for its reply.
//!
//! Standard: the SoapyRemote protocol, RPC version 0x0400 (`rpc::VERSION`).

use sdrkit_core::net::Conn as Link;
use std::collections::BTreeMap;
use std::io::{ErrorKind, Write};
use std::time::{Duration, Instant};

/// The header word of every packet.
pub const HEADER: [u8; 4] = *b"SRPC";
/// The trailer word of every packet.
pub const TRAILER: [u8; 4] = *b"CPRS";
/// The RPC version sdrkit sends.
pub const VERSION: u32 = 0x0000_0400;
/// The first version whose ranges carry a step.
pub const RANGE_STEP_VERSION: u32 = 0x0000_0400;
/// The largest reply sdrkit accepts.
pub const MAX_PACKET: usize = 16 << 20;
/// Header (12) plus trailer (4).
pub const OVERHEAD: usize = 16;

/// Value type tags.
#[allow(missing_docs)]
pub mod tag {
    pub const CHAR: u8 = 0x00;
    pub const BOOL: u8 = 0x01;
    pub const INT32: u8 = 0x02;
    pub const INT64: u8 = 0x03;
    pub const FLOAT64: u8 = 0x04;
    pub const COMPLEX128: u8 = 0x05;
    pub const STRING: u8 = 0x06;
    pub const RANGE: u8 = 0x07;
    pub const RANGE_LIST: u8 = 0x08;
    pub const STRING_LIST: u8 = 0x09;
    pub const FLOAT64_LIST: u8 = 0x0A;
    pub const KWARGS: u8 = 0x0B;
    pub const KWARGS_LIST: u8 = 0x0C;
    pub const EXCEPTION: u8 = 0x0D;
    pub const VOID: u8 = 0x0E;
    pub const CALL: u8 = 0x0F;
    pub const SIZE_LIST: u8 = 0x10;
    pub const ARG_INFO: u8 = 0x11;
    pub const ARG_INFO_LIST: u8 = 0x12;
}

/// Call numbers.
#[allow(missing_docs)]
pub mod call {
    pub const FIND: u32 = 0;
    pub const MAKE: u32 = 1;
    pub const UNMAKE: u32 = 2;
    pub const HANGUP: u32 = 3;
    pub const SERVER_ID: u32 = 20;
    pub const START_LOG: u32 = 21;
    pub const STOP_LOG: u32 = 22;
    pub const DRIVER_KEY: u32 = 100;
    pub const HARDWARE_KEY: u32 = 101;
    pub const HARDWARE_INFO: u32 = 102;
    pub const SET_FRONTEND_MAPPING: u32 = 200;
    pub const GET_FRONTEND_MAPPING: u32 = 201;
    pub const NUM_CHANNELS: u32 = 202;
    pub const FULL_DUPLEX: u32 = 203;
    pub const CHANNEL_INFO: u32 = 204;
    pub const SETUP_STREAM: u32 = 300;
    pub const CLOSE_STREAM: u32 = 301;
    pub const ACTIVATE_STREAM: u32 = 302;
    pub const DEACTIVATE_STREAM: u32 = 303;
    pub const STREAM_FORMATS: u32 = 304;
    pub const NATIVE_FORMAT: u32 = 305;
    pub const STREAM_ARG_INFO: u32 = 306;
    pub const SETUP_STREAM_BYPASS: u32 = 307;
    pub const LIST_ANTENNAS: u32 = 500;
    pub const SET_ANTENNA: u32 = 501;
    pub const GET_ANTENNA: u32 = 502;
    pub const HAS_FREQ_CORRECTION: u32 = 503;
    pub const SET_FREQ_CORRECTION: u32 = 504;
    pub const GET_FREQ_CORRECTION: u32 = 505;
    pub const HAS_DC_OFFSET_MODE: u32 = 600;
    pub const SET_DC_OFFSET_MODE: u32 = 601;
    pub const GET_DC_OFFSET_MODE: u32 = 602;
    pub const HAS_DC_OFFSET: u32 = 603;
    pub const SET_DC_OFFSET: u32 = 604;
    pub const GET_DC_OFFSET: u32 = 605;
    pub const HAS_IQ_BALANCE: u32 = 606;
    pub const SET_IQ_BALANCE: u32 = 607;
    pub const GET_IQ_BALANCE: u32 = 608;
    pub const HAS_IQ_BALANCE_MODE: u32 = 609;
    pub const SET_IQ_BALANCE_MODE: u32 = 610;
    pub const GET_IQ_BALANCE_MODE: u32 = 611;
    pub const LIST_GAINS: u32 = 700;
    pub const SET_GAIN_MODE: u32 = 701;
    pub const GET_GAIN_MODE: u32 = 702;
    pub const SET_GAIN: u32 = 703;
    pub const SET_GAIN_ELEMENT: u32 = 704;
    pub const GET_GAIN: u32 = 705;
    pub const GET_GAIN_ELEMENT: u32 = 706;
    pub const GAIN_RANGE: u32 = 707;
    pub const GAIN_RANGE_ELEMENT: u32 = 708;
    pub const HAS_GAIN_MODE: u32 = 709;
    pub const SET_FREQUENCY: u32 = 800;
    pub const SET_FREQUENCY_COMPONENT: u32 = 801;
    pub const GET_FREQUENCY: u32 = 802;
    pub const GET_FREQUENCY_COMPONENT: u32 = 803;
    pub const LIST_FREQUENCIES: u32 = 804;
    pub const FREQUENCY_RANGE: u32 = 805;
    pub const FREQUENCY_RANGE_COMPONENT: u32 = 806;
    pub const FREQUENCY_ARG_INFO: u32 = 807;
    pub const SET_SAMPLE_RATE: u32 = 900;
    pub const GET_SAMPLE_RATE: u32 = 901;
    pub const LIST_SAMPLE_RATES: u32 = 902;
    pub const SET_BANDWIDTH: u32 = 903;
    pub const GET_BANDWIDTH: u32 = 904;
    pub const LIST_BANDWIDTHS: u32 = 905;
    pub const BANDWIDTH_RANGE: u32 = 906;
    pub const SAMPLE_RATE_RANGE: u32 = 907;
    pub const SET_MASTER_CLOCK_RATE: u32 = 1000;
    pub const GET_MASTER_CLOCK_RATE: u32 = 1001;
    pub const LIST_CLOCK_SOURCES: u32 = 1002;
    pub const SET_CLOCK_SOURCE: u32 = 1003;
    pub const GET_CLOCK_SOURCE: u32 = 1004;
    pub const LIST_TIME_SOURCES: u32 = 1005;
    pub const SET_TIME_SOURCE: u32 = 1006;
    pub const GET_TIME_SOURCE: u32 = 1007;
    pub const MASTER_CLOCK_RATES: u32 = 1008;
    pub const SET_REF_CLOCK_RATE: u32 = 1009;
    pub const GET_REF_CLOCK_RATE: u32 = 1010;
    pub const REF_CLOCK_RATES: u32 = 1011;
    pub const HAS_HARDWARE_TIME: u32 = 1100;
    pub const GET_HARDWARE_TIME: u32 = 1101;
    pub const SET_HARDWARE_TIME: u32 = 1102;
    pub const SET_COMMAND_TIME: u32 = 1103;
    pub const LIST_SENSORS: u32 = 1200;
    pub const READ_SENSOR: u32 = 1201;
    pub const LIST_CHANNEL_SENSORS: u32 = 1202;
    pub const READ_CHANNEL_SENSOR: u32 = 1203;
    pub const SENSOR_INFO: u32 = 1204;
    pub const CHANNEL_SENSOR_INFO: u32 = 1205;
    pub const WRITE_SETTING: u32 = 1400;
    pub const READ_SETTING: u32 = 1401;
    pub const SETTING_INFO: u32 = 1402;
    pub const WRITE_CHANNEL_SETTING: u32 = 1403;
    pub const READ_CHANNEL_SETTING: u32 = 1404;
    pub const CHANNEL_SETTING_INFO: u32 = 1405;
}

/// SoapySDR directions.
pub const TX: i8 = 0;
/// SoapySDR directions.
pub const RX: i8 = 1;

/// ArgInfo types (SoapySDR).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgType {
    /// 0.
    Bool,
    /// 1.
    Int,
    /// 2.
    Float,
    /// 3 (and anything unknown).
    String,
}

impl ArgType {
    fn from_i32(v: i32) -> ArgType {
        match v {
            0 => ArgType::Bool,
            1 => ArgType::Int,
            2 => ArgType::Float,
            _ => ArgType::String,
        }
    }
    fn to_i32(self) -> i32 {
        match self {
            ArgType::Bool => 0,
            ArgType::Int => 1,
            ArgType::Float => 2,
            ArgType::String => 3,
        }
    }
}

/// A SoapySDR range: minimum, maximum, step (0 = continuous).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Range {
    /// Lowest value.
    pub min: f64,
    /// Highest value.
    pub max: f64,
    /// Step, 0 when continuous.
    pub step: f64,
}

impl Range {
    /// A range.
    pub fn new(min: f64, max: f64, step: f64) -> Range {
        Range { min, max, step }
    }
}

/// A SoapySDR ArgInfo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArgInfo {
    /// The key.
    pub key: String,
    /// The default value (as the driver states it).
    pub value: String,
    /// A display name.
    pub name: String,
    /// A description.
    pub description: String,
    /// Units.
    pub units: String,
    /// The type.
    pub kind: Option<ArgType>,
    /// The range of a number.
    pub range: Range,
    /// The options of a choice.
    pub options: Vec<String>,
    /// Display names of the options.
    pub option_names: Vec<String>,
}

/// Key-value arguments, sorted by key bytes (as they are written).
pub type Kwargs = BTreeMap<String, String>;

/// The argument key that names the SoapySDR driver of a radio.
pub const DRIVER: &str = "driver";
/// The argument key of a radio's serial number.
pub const SERIAL: &str = "serial";
/// The argument key of a radio's label.
pub const LABEL: &str = "label";

/// The value of `key` in `kw`, also when the server wrote it as `remote:<key>`.
pub fn arg<'a>(kw: &'a Kwargs, key: &str) -> Option<&'a str> {
    kw.get(&format!("remote:{key}"))
        .or_else(|| kw.get(key))
        .map(String::as_str)
}

// ---- FLOAT64 ----

/// Splits `v` as `m x 2^e` with `0.5 <= |m| < 1` (C `frexp`). 0 and values that are not finite give `(0, 0)`.
pub fn frexp(v: f64) -> (f64, i32) {
    if v == 0.0 || !v.is_finite() {
        return (0.0, 0);
    }
    let bits = v.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    if exp == 0 {
        // Subnormal: scale into the normal range first.
        let (m, e) = frexp(v * f64::from_bits(0x4350_0000_0000_0000)); // 2^54
        return (m, e - 54);
    }
    let m = f64::from_bits((bits & !(0x7ff_u64 << 52)) | (1022_u64 << 52));
    (m, exp - 1022)
}

/// The (exponent, mantissa) pair a FLOAT64 is sent as.
pub fn float_parts(v: f64) -> (i32, i64) {
    let (m, e) = frexp(v);
    (e, (m * 9_007_199_254_740_992.0) as i64) // 2^53, truncated toward zero
}

/// The double of an (exponent, mantissa) pair.
pub fn float_from_parts(exp: i32, mant: i64) -> f64 {
    // Two steps, so a large exponent difference neither overflows nor loses a subnormal result needlessly.
    let e = exp.saturating_sub(53);
    let half = e / 2;
    (mant as f64) * 2f64.powi(half) * 2f64.powi(e - half)
}

// ---- Packing ----

/// Builds a payload of tagged values.
#[derive(Clone, Debug, Default)]
pub struct Packer {
    /// The bytes so far.
    pub buf: Vec<u8>,
}

impl Packer {
    /// A request payload starting with CALL `n`.
    pub fn call(n: u32) -> Packer {
        let mut p = Packer::default();
        p.buf.push(tag::CALL);
        p.i32(n as i32);
        p
    }
    /// CHAR.
    pub fn char(&mut self, v: i8) -> &mut Self {
        self.buf.extend_from_slice(&[tag::CHAR, v as u8]);
        self
    }
    /// BOOL.
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.buf.extend_from_slice(&[tag::BOOL, v as u8]);
        self
    }
    /// INT32.
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.buf.push(tag::INT32);
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// INT64.
    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.buf.push(tag::INT64);
        self.buf.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// FLOAT64.
    pub fn f64(&mut self, v: f64) -> &mut Self {
        let (e, m) = float_parts(v);
        self.buf.push(tag::FLOAT64);
        self.i32(e);
        self.i64(m)
    }
    /// COMPLEX128.
    pub fn complex(&mut self, re: f64, im: f64) -> &mut Self {
        self.buf.push(tag::COMPLEX128);
        self.f64(re);
        self.f64(im)
    }
    /// STRING.
    pub fn string(&mut self, s: &str) -> &mut Self {
        self.buf.push(tag::STRING);
        self.i32(s.len() as i32);
        self.buf.extend_from_slice(s.as_bytes());
        self
    }
    /// RANGE. The step is written when `version` is 0x400 or more.
    pub fn range(&mut self, r: Range, version: u32) -> &mut Self {
        self.buf.push(tag::RANGE);
        self.f64(r.min);
        self.f64(r.max);
        if version >= RANGE_STEP_VERSION {
            self.f64(r.step);
        }
        self
    }
    /// RANGE_LIST.
    pub fn range_list(&mut self, rs: &[Range], version: u32) -> &mut Self {
        self.buf.push(tag::RANGE_LIST);
        self.i32(rs.len() as i32);
        for r in rs {
            self.range(*r, version);
        }
        self
    }
    /// STRING_LIST.
    pub fn string_list<S: AsRef<str>>(&mut self, v: &[S]) -> &mut Self {
        self.buf.push(tag::STRING_LIST);
        self.i32(v.len() as i32);
        for s in v {
            self.string(s.as_ref());
        }
        self
    }
    /// FLOAT64_LIST.
    pub fn f64_list(&mut self, v: &[f64]) -> &mut Self {
        self.buf.push(tag::FLOAT64_LIST);
        self.i32(v.len() as i32);
        for x in v {
            self.f64(*x);
        }
        self
    }
    /// KWARGS (keys in ascending byte order).
    pub fn kwargs(&mut self, kw: &Kwargs) -> &mut Self {
        self.buf.push(tag::KWARGS);
        self.i32(kw.len() as i32);
        for (k, v) in kw {
            self.string(k);
            self.string(v);
        }
        self
    }
    /// KWARGS_LIST.
    pub fn kwargs_list(&mut self, v: &[Kwargs]) -> &mut Self {
        self.buf.push(tag::KWARGS_LIST);
        self.i32(v.len() as i32);
        for kw in v {
            self.kwargs(kw);
        }
        self
    }
    /// EXCEPTION.
    pub fn exception(&mut self, msg: &str) -> &mut Self {
        self.buf.push(tag::EXCEPTION);
        self.string(msg)
    }
    /// VOID.
    pub fn void(&mut self) -> &mut Self {
        self.buf.push(tag::VOID);
        self
    }
    /// SIZE_LIST.
    pub fn size_list(&mut self, v: &[i32]) -> &mut Self {
        self.buf.push(tag::SIZE_LIST);
        self.i32(v.len() as i32);
        for x in v {
            self.i32(*x);
        }
        self
    }
    /// ARG_INFO.
    pub fn arg_info(&mut self, a: &ArgInfo, version: u32) -> &mut Self {
        self.buf.push(tag::ARG_INFO);
        self.string(&a.key);
        self.string(&a.value);
        self.string(&a.name);
        self.string(&a.description);
        self.string(&a.units);
        self.i32(a.kind.unwrap_or(ArgType::String).to_i32());
        self.range(a.range, version);
        self.string_list(&a.options);
        self.string_list(&a.option_names)
    }
    /// ARG_INFO_LIST.
    pub fn arg_info_list(&mut self, v: &[ArgInfo], version: u32) -> &mut Self {
        self.buf.push(tag::ARG_INFO_LIST);
        self.i32(v.len() as i32);
        for a in v {
            self.arg_info(a, version);
        }
        self
    }
    /// "dir, ch": CHAR direction then INT32 channel.
    pub fn dir_ch(&mut self, dir: i8, ch: i32) -> &mut Self {
        self.char(dir);
        self.i32(ch)
    }
}

/// A complete packet around `payload`, with `version` in the header.
pub fn packet(payload: &[u8], version: u32) -> Vec<u8> {
    let len = payload.len() + OVERHEAD;
    let mut v = Vec::with_capacity(len);
    v.extend_from_slice(&HEADER);
    v.extend_from_slice(&version.to_be_bytes());
    v.extend_from_slice(&(len as u32).to_be_bytes());
    v.extend_from_slice(payload);
    v.extend_from_slice(&TRAILER);
    v
}

// ---- Unpacking ----

/// A value in a reply was not what the call returns (or the payload ended early).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unexpected;

/// Reads tagged values from a payload.
#[derive(Clone, Debug)]
pub struct Unpacker<'a> {
    buf: &'a [u8],
    pos: usize,
    /// The peer's RPC version (from the packet's header).
    pub version: u32,
}

type U<T> = Result<T, Unexpected>;

impl<'a> Unpacker<'a> {
    /// Reads `buf`, a payload from a packet with `version` in its header.
    pub fn new(buf: &'a [u8], version: u32) -> Self {
        Unpacker {
            buf,
            pos: 0,
            version,
        }
    }
    /// Bytes not read yet.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
    /// The tag of the next value, without reading it.
    pub fn peek(&self) -> Option<u8> {
        self.buf.get(self.pos).copied()
    }
    fn bytes(&mut self, n: usize) -> U<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(Unexpected)?;
        let b = self.buf.get(self.pos..end).ok_or(Unexpected)?;
        self.pos = end;
        Ok(b)
    }
    fn expect(&mut self, t: u8) -> U<()> {
        if self.bytes(1)?[0] == t {
            Ok(())
        } else {
            Err(Unexpected)
        }
    }
    fn count(&mut self) -> U<usize> {
        let n = self.i32()?;
        if n < 0 || n as usize > self.remaining() {
            return Err(Unexpected);
        }
        Ok(n as usize)
    }
    /// CHAR.
    pub fn char(&mut self) -> U<i8> {
        self.expect(tag::CHAR)?;
        Ok(self.bytes(1)?[0] as i8)
    }
    /// BOOL (any non-zero byte is true).
    pub fn bool(&mut self) -> U<bool> {
        self.expect(tag::BOOL)?;
        Ok(self.bytes(1)?[0] != 0)
    }
    /// INT32.
    pub fn i32(&mut self) -> U<i32> {
        self.expect(tag::INT32)?;
        let b = self.bytes(4)?;
        Ok(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    /// INT64.
    pub fn i64(&mut self) -> U<i64> {
        self.expect(tag::INT64)?;
        let b = self.bytes(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(i64::from_be_bytes(a))
    }
    /// FLOAT64.
    pub fn f64(&mut self) -> U<f64> {
        self.expect(tag::FLOAT64)?;
        let e = self.i32()?;
        let m = self.i64()?;
        Ok(float_from_parts(e, m))
    }
    /// COMPLEX128.
    pub fn complex(&mut self) -> U<(f64, f64)> {
        self.expect(tag::COMPLEX128)?;
        Ok((self.f64()?, self.f64()?))
    }
    /// STRING (invalid UTF-8 is replaced).
    pub fn string(&mut self) -> U<String> {
        self.expect(tag::STRING)?;
        let n = self.count()?;
        Ok(String::from_utf8_lossy(self.bytes(n)?).into_owned())
    }
    /// RANGE, with a step only when the peer's version is 0x400 or more.
    pub fn range(&mut self) -> U<Range> {
        self.expect(tag::RANGE)?;
        let min = self.f64()?;
        let max = self.f64()?;
        let step = if self.version >= RANGE_STEP_VERSION {
            self.f64()?
        } else {
            0.0
        };
        Ok(Range { min, max, step })
    }
    /// RANGE_LIST.
    pub fn range_list(&mut self) -> U<Vec<Range>> {
        self.expect(tag::RANGE_LIST)?;
        let n = self.count()?;
        (0..n).map(|_| self.range()).collect()
    }
    /// STRING_LIST.
    pub fn string_list(&mut self) -> U<Vec<String>> {
        self.expect(tag::STRING_LIST)?;
        let n = self.count()?;
        (0..n).map(|_| self.string()).collect()
    }
    /// FLOAT64_LIST.
    pub fn f64_list(&mut self) -> U<Vec<f64>> {
        self.expect(tag::FLOAT64_LIST)?;
        let n = self.count()?;
        (0..n).map(|_| self.f64()).collect()
    }
    /// KWARGS (a repeated key: the last one wins).
    pub fn kwargs(&mut self) -> U<Kwargs> {
        self.expect(tag::KWARGS)?;
        let n = self.count()?;
        let mut kw = Kwargs::new();
        for _ in 0..n {
            let k = self.string()?;
            let v = self.string()?;
            kw.insert(k, v);
        }
        Ok(kw)
    }
    /// KWARGS_LIST.
    pub fn kwargs_list(&mut self) -> U<Vec<Kwargs>> {
        self.expect(tag::KWARGS_LIST)?;
        let n = self.count()?;
        (0..n).map(|_| self.kwargs()).collect()
    }
    /// SIZE_LIST.
    pub fn size_list(&mut self) -> U<Vec<i32>> {
        self.expect(tag::SIZE_LIST)?;
        let n = self.count()?;
        (0..n).map(|_| self.i32()).collect()
    }
    /// VOID.
    pub fn void(&mut self) -> U<()> {
        self.expect(tag::VOID)
    }
    /// CALL: the call number of a request.
    pub fn call(&mut self) -> U<u32> {
        self.expect(tag::CALL)?;
        Ok(self.i32()? as u32)
    }
    /// ARG_INFO.
    pub fn arg_info(&mut self) -> U<ArgInfo> {
        self.expect(tag::ARG_INFO)?;
        Ok(ArgInfo {
            key: self.string()?,
            value: self.string()?,
            name: self.string()?,
            description: self.string()?,
            units: self.string()?,
            kind: Some(ArgType::from_i32(self.i32()?)),
            range: self.range()?,
            options: self.string_list()?,
            option_names: self.string_list()?,
        })
    }
    /// ARG_INFO_LIST.
    pub fn arg_info_list(&mut self) -> U<Vec<ArgInfo>> {
        self.expect(tag::ARG_INFO_LIST)?;
        let n = self.count()?;
        (0..n).map(|_| self.arg_info()).collect()
    }
    /// The message of an EXCEPTION value, when the next value is one.
    pub fn exception(&mut self) -> Option<String> {
        if self.peek() != Some(tag::EXCEPTION) {
            return None;
        }
        let mut probe = self.clone();
        probe.pos += 1;
        let msg = probe.string().ok()?;
        *self = probe;
        Some(msg)
    }
}

// ---- Reading packets ----

/// What is wrong with a packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketFault {
    /// The length field is at most the header and trailer.
    Length {
        /// The length the packet reported.
        len: usize,
    },
    /// The packet does not end with the trailer word.
    Trailer,
}

/// Why a request got no usable reply.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum RpcError {
    /// The call failed on the server, with the server's message. The connection stays usable.
    #[error("the server refused the call: {0}")]
    Exception(String),
    /// No reply within the time.
    #[error("no reply within {0:?}")]
    Timeout(Duration),
    /// The header word was not `SRPC`.
    #[error("not a SoapyRemote server")]
    NotSoapy,
    /// The connection closed (`before_reply`: before any reply was ever read on it).
    #[error("the connection closed")]
    Closed {
        /// No reply had been read on this connection yet.
        before_reply: bool,
    },
    /// The reply holds the wrong value for the call.
    #[error("the reply holds an unexpected value")]
    Unexpected,
    /// The length field says more than 16 MiB.
    #[error("the reply is too large")]
    TooLarge,
    /// The packet is malformed.
    #[error("malformed packet: {0:?}")]
    Malformed(PacketFault),
    /// The connection was already given up.
    #[error("the connection was given up")]
    Broken,
}

impl RpcError {
    /// The error of a radio for this failure of call `call`.
    pub fn driver_error(self, call: u32) -> sdrkit_driver_core::DriverError {
        use sdrkit_core::NetError;
        use sdrkit_driver_core::{DriverError, Protocol};
        match self {
            RpcError::Exception(text) => {
                DriverError::driver(crate::SoapyError::Exception { call, text })
            }
            RpcError::Timeout(_) => DriverError::Net(NetError::TimedOut),
            RpcError::NotSoapy | RpcError::Closed { before_reply: true } => {
                DriverError::Protocol(Protocol::NotThisRadio)
            }
            RpcError::Closed {
                before_reply: false,
            }
            | RpcError::Broken => DriverError::Net(NetError::Closed),
            RpcError::Unexpected | RpcError::Malformed(_) => {
                DriverError::Protocol(Protocol::Malformed)
            }
            RpcError::TooLarge => DriverError::Protocol(Protocol::TooLong),
        }
    }
}

/// One packet as read: the peer's version and the payload.
#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    /// The version word of its header.
    pub version: u32,
    /// The payload (between header and trailer).
    pub payload: Vec<u8>,
}

fn fill(
    s: &mut dyn Link,
    buf: &mut [u8],
    deadline: Instant,
    timeout: Duration,
) -> Result<(), RpcError> {
    let mut got = 0;
    while got < buf.len() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(RpcError::Timeout(timeout));
        }
        let _ = s.set_read_timeout(Some(left.max(Duration::from_millis(1))));
        match s.read(&mut buf[got..]) {
            Ok(0) => {
                return Err(RpcError::Closed {
                    before_reply: false,
                })
            }
            Ok(n) => got += n,
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(_) => {
                return Err(RpcError::Closed {
                    before_reply: false,
                })
            }
        }
    }
    Ok(())
}

/// Reads one packet within `timeout`. The length is checked before the payload is read.
///
/// # Errors
///
/// [`RpcError`] if no whole packet arrives in time or the packet is not a SoapyRemote packet.
pub fn read_packet(s: &mut dyn Link, timeout: Duration) -> Result<Packet, RpcError> {
    let deadline = Instant::now() + timeout;
    let mut h = [0u8; 12];
    fill(s, &mut h, deadline, timeout)?;
    if h[..4] != HEADER {
        return Err(RpcError::NotSoapy);
    }
    let version = u32::from_be_bytes([h[4], h[5], h[6], h[7]]);
    let len = u32::from_be_bytes([h[8], h[9], h[10], h[11]]) as usize;
    if len > MAX_PACKET {
        return Err(RpcError::TooLarge);
    }
    if len <= OVERHEAD {
        return Err(RpcError::Malformed(PacketFault::Length { len }));
    }
    let mut rest = vec![0u8; len - 12];
    fill(s, &mut rest, deadline, timeout)?;
    if rest[rest.len() - 4..] != TRAILER {
        return Err(RpcError::Malformed(PacketFault::Trailer));
    }
    rest.truncate(rest.len() - 4);
    Ok(Packet {
        version,
        payload: rest,
    })
}

/// The control connection: one request, one reply, in order.
pub struct Conn {
    sock: Box<dyn Link>,
    /// The peer's RPC version, from its last reply.
    pub peer_version: Option<u32>,
    replies: u32,
    requests: u32,
    broken: bool,
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conn")
            .field("peer_version", &self.peer_version)
            .field("broken", &self.broken)
            .finish_non_exhaustive()
    }
}

impl Conn {
    /// Wraps a connection.
    pub fn new(sock: Box<dyn Link>) -> Conn {
        Conn {
            sock,
            peer_version: None,
            replies: 0,
            requests: 0,
            broken: false,
        }
    }
    /// The connection, to register with an abort handle.
    pub fn link(&self) -> &dyn Link {
        self.sock.as_ref()
    }
    /// The connection was given up (timeout, protocol error, closed).
    pub fn is_broken(&self) -> bool {
        self.broken
    }
    /// Sends `req` and reads its reply. An EXCEPTION reply is [`RpcError::Exception`]. Other failures break the connection.
    ///
    /// # Errors
    ///
    /// [`RpcError`] if the call fails.
    pub fn request(&mut self, req: &Packer, timeout: Duration) -> Result<Packet, RpcError> {
        if self.broken {
            return Err(RpcError::Broken);
        }
        let first = self.requests == 0;
        self.requests += 1;
        let pkt = packet(&req.buf, VERSION);
        if self.sock.write_all(&pkt).is_err() {
            self.broken = true;
            return Err(RpcError::Closed {
                before_reply: self.replies == 0,
            });
        }
        self.reply(timeout, first)
    }

    /// Reads the next reply. `first`: it answers the first request on the connection.
    fn reply(&mut self, timeout: Duration, first: bool) -> Result<Packet, RpcError> {
        match read_packet(self.sock.as_mut(), timeout) {
            Ok(p) => {
                self.replies += 1;
                self.peer_version = Some(p.version);
                let mut u = Unpacker::new(&p.payload, p.version);
                if let Some(msg) = u.exception() {
                    return Err(RpcError::Exception(msg));
                }
                Ok(p)
            }
            Err(e) => {
                self.broken = true;
                self.sock.shutdown();
                Err(match e {
                    RpcError::Closed { .. } => RpcError::Closed {
                        before_reply: self.replies == 0 && first,
                    },
                    e => e,
                })
            }
        }
    }
    /// A request whose reply `f` reads. A wrong value breaks the connection.
    ///
    /// # Errors
    ///
    /// [`RpcError`] if the call fails or the reply holds another value.
    pub fn call<T>(
        &mut self,
        req: &Packer,
        timeout: Duration,
        f: impl FnOnce(&mut Unpacker) -> Result<T, Unexpected>,
    ) -> Result<T, RpcError> {
        let p = self.request(req, timeout)?;
        let mut u = Unpacker::new(&p.payload, p.version);
        f(&mut u).map_err(|_| {
            self.broken = true;
            self.sock.shutdown();
            RpcError::Unexpected
        })
    }
    /// Reads one more reply to the last request, which `f` reads: a call that the server answers twice (the set-up of a TCP
    /// stream). A wrong value breaks the connection.
    ///
    /// # Errors
    ///
    /// [`RpcError`] if no reply comes, it is an exception, or it holds another value.
    pub fn next<T>(
        &mut self,
        timeout: Duration,
        f: impl FnOnce(&mut Unpacker) -> Result<T, Unexpected>,
    ) -> Result<T, RpcError> {
        if self.broken {
            return Err(RpcError::Broken);
        }
        let p = self.reply(timeout, false)?;
        let mut u = Unpacker::new(&p.payload, p.version);
        f(&mut u).map_err(|_| {
            self.broken = true;
            self.sock.shutdown();
            RpcError::Unexpected
        })
    }
    /// A request whose reply is VOID.
    ///
    /// # Errors
    ///
    /// [`RpcError`] if the call fails.
    pub fn call_void(&mut self, req: &Packer, timeout: Duration) -> Result<(), RpcError> {
        self.call(req, timeout, |u| u.void())
    }
    /// Shuts the connection down.
    pub fn shutdown(&mut self) {
        self.broken = true;
        self.sock.shutdown();
    }
}

/// An optional query: an EXCEPTION means "not offered". Other failures are passed on.
pub fn optional<T>(r: Result<T, RpcError>) -> Result<Option<T>, RpcError> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(RpcError::Exception(_)) => Ok(None),
        Err(e) => Err(e),
    }
}
