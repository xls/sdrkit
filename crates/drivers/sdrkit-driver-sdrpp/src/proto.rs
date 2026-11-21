//! The SDR++ server wire protocol: packet headers, commands, the UI_ACTION argument, sample bodies and error codes. Everything
//! on the wire is little endian.
//!
//! Standard: the SDR++ server protocol (`server_protocol.h`).

use sdrkit_core::Complex32;
use std::mem::MaybeUninit;

/// The server's default TCP port.
pub const DEFAULT_PORT: u16 = 5259;
/// The size of every packet header.
pub const HEADER_LEN: usize = 8;
/// The largest packet accepted, header included (32 MiB).
pub const MAX_PACKET: u32 = 32 << 20;
/// The longest id a UI_ACTION carries.
pub const MAX_ID: usize = 65_535;

/// Packet types (the first u32 of a header).
pub mod pkt {
    /// A command, both ways.
    pub const COMMAND: u32 = 0;
    /// The answer to a command.
    pub const COMMAND_ACK: u32 = 1;
    /// Samples.
    pub const BASEBAND: u32 = 2;
    /// Samples in one zstd frame.
    pub const BASEBAND_COMPRESSED: u32 = 3;
    /// Reserved, never requested.
    pub const VFO: u32 = 4;
    /// Reserved, never requested.
    pub const FFT: u32 = 5;
    /// An error code.
    pub const ERROR: u32 = 6;
    /// The highest valid type.
    pub const LAST: u32 = ERROR;
}

/// Commands (the first u32 of a COMMAND or COMMAND_ACK body).
pub mod cmd {
    /// Asks for the menu.
    pub const GET_UI: u32 = 0x00;
    /// Moves a control of the menu.
    pub const UI_ACTION: u32 = 0x01;
    /// Starts the source and the stream.
    pub const START: u32 = 0x02;
    /// Stops the source and the stream.
    pub const STOP: u32 = 0x03;
    /// The frequency, f64 Hz.
    pub const SET_FREQUENCY: u32 = 0x04;
    /// Asks for the rate (not used by this driver).
    pub const GET_SAMPLERATE: u32 = 0x05;
    /// The sample type, u8.
    pub const SET_SAMPLE_TYPE: u32 = 0x06;
    /// Compression on or off, u8.
    pub const SET_COMPRESSION: u32 = 0x07;
    /// From the server: its rate, f64 sps.
    pub const SET_SAMPLERATE: u32 = 0x80;
    /// From the server: it serves another client.
    pub const DISCONNECT: u32 = 0x81;
    /// The client commands, numbered 0 to this.
    pub const CLIENT_COUNT: usize = 8;
}

/// A packet header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The packet type ([`pkt`]).
    pub kind: u32,
    /// The packet size, header included.
    pub size: u32,
}

impl Header {
    /// Reads the 8 bytes.
    pub fn parse(b: &[u8; HEADER_LEN]) -> Header {
        Header {
            kind: u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            size: u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        }
    }
    /// Size 8 to 32 MiB and a type of at most 6.
    pub fn valid(&self) -> bool {
        (HEADER_LEN as u32..=MAX_PACKET).contains(&self.size) && self.kind <= pkt::LAST
    }
    /// The body length.
    pub fn body_len(&self) -> usize {
        self.size as usize - HEADER_LEN
    }
}

/// Appends a command packet: header, command, argument.
pub fn command(c: u32, arg: &[u8], out: &mut Vec<u8>) {
    let size = (HEADER_LEN + 4 + arg.len()) as u32;
    out.extend_from_slice(&pkt::COMMAND.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&c.to_le_bytes());
    out.extend_from_slice(arg);
}

/// A command packet as a new buffer.
pub fn command_bytes(c: u32, arg: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(HEADER_LEN + 4 + arg.len());
    command(c, arg, &mut v);
    v
}

/// A control's new value in a UI_ACTION.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActionValue {
    /// Check boxes, radio buttons.
    Bool(bool),
    /// Lists (the index), integer sliders and inputs.
    Int(i32),
    /// Float sliders.
    Float(f32),
}

impl std::fmt::Display for ActionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionValue::Bool(b) => write!(f, "{}", *b as u8),
            ActionValue::Int(i) => write!(f, "{i}"),
            ActionValue::Float(x) => write!(f, "{x}"),
        }
    }
}

/// The UI_ACTION argument: the sync byte, the id as a STRING element (cut to 65 535 bytes), the value as one element.
pub fn ui_action_arg(id: &str, value: ActionValue, sync: bool) -> Vec<u8> {
    let id = &id.as_bytes()[..id.len().min(MAX_ID)];
    let mut a = Vec::with_capacity(1 + 3 + id.len() + 5);
    a.push(sync as u8);
    a.push(crate::menu::elem::STRING);
    a.extend_from_slice(&(id.len() as u16).to_le_bytes());
    a.extend_from_slice(id);
    match value {
        ActionValue::Bool(b) => {
            a.push(crate::menu::elem::BOOL);
            a.push(b as u8);
        }
        ActionValue::Int(i) => {
            a.push(crate::menu::elem::INT);
            a.extend_from_slice(&i.to_le_bytes());
        }
        ActionValue::Float(x) => {
            a.push(crate::menu::elem::FLOAT);
            a.extend_from_slice(&x.to_le_bytes());
        }
    }
    a
}

/// The sample types of SET_SAMPLE_TYPE and BASEBAND.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleType {
    /// int8 I and Q.
    I8 = 0,
    /// int16 I and Q (the server's default).
    I16 = 1,
    /// float32 I and Q.
    F32 = 2,
}

impl SampleType {
    /// From the wire number.
    pub fn from_wire(n: u16) -> Option<SampleType> {
        match n {
            0 => Some(SampleType::I8),
            1 => Some(SampleType::I16),
            2 => Some(SampleType::F32),
            _ => None,
        }
    }
    /// Bytes per complex sample.
    pub fn bytes(self) -> usize {
        match self {
            SampleType::I8 => 2,
            SampleType::I16 => 4,
            SampleType::F32 => 8,
        }
    }
    /// The setting's value (`i8`, `i16`, `f32`).
    pub fn key(self) -> &'static str {
        match self {
            SampleType::I8 => "i8",
            SampleType::I16 => "i16",
            SampleType::F32 => "f32",
        }
    }
    /// From the setting's value; anything else is the default, int16.
    pub fn from_key(k: &str) -> SampleType {
        match k {
            "i8" => SampleType::I8,
            "f32" => SampleType::F32,
            _ => SampleType::I16,
        }
    }
    /// For the status line.
    pub fn describe(self) -> &'static str {
        match self {
            SampleType::I8 => "8-bit",
            SampleType::I16 => "16-bit",
            SampleType::F32 => "32-bit float",
        }
    }
}

/// A BASEBAND body, read in place: the sample type, the scale and the sample bytes.
#[derive(Clone, Copy, Debug)]
pub struct Baseband<'a> {
    /// The sample type.
    pub kind: SampleType,
    /// The packet's peak magnitude (not applied to float32).
    pub scale: f32,
    /// Interleaved I, Q in the sample type.
    pub data: &'a [u8],
}

impl<'a> Baseband<'a> {
    /// Reads a body. None for a body shorter than 8 bytes or an unknown sample type.
    pub fn parse(body: &'a [u8]) -> Option<Baseband<'a>> {
        if body.len() < 8 {
            return None;
        }
        let kind = SampleType::from_wire(u16::from_le_bytes([body[2], body[3]]))?;
        let scale = f32::from_le_bytes([body[4], body[5], body[6], body[7]]);
        Some(Baseband {
            kind,
            scale,
            data: &body[8..],
        })
    }
    /// Whole complex samples (a trailing part is ignored).
    pub fn len(&self) -> usize {
        self.data.len() / self.kind.bytes()
    }
    /// No whole sample.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Sample `i`.
    #[inline]
    pub fn sample(&self, i: usize) -> Complex32 {
        let d = self.data;
        match self.kind {
            SampleType::I8 => {
                let k = self.scale / 128.0;
                Complex32::new(d[2 * i] as i8 as f32 * k, d[2 * i + 1] as i8 as f32 * k)
            }
            SampleType::I16 => {
                let k = self.scale / 32768.0;
                let o = 4 * i;
                Complex32::new(
                    i16::from_le_bytes([d[o], d[o + 1]]) as f32 * k,
                    i16::from_le_bytes([d[o + 2], d[o + 3]]) as f32 * k,
                )
            }
            SampleType::F32 => {
                let o = 8 * i;
                Complex32::new(
                    f32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]),
                    f32::from_le_bytes([d[o + 4], d[o + 5], d[o + 6], d[o + 7]]),
                )
            }
        }
    }
    /// Writes samples `from..` into `dst` as `Cf32` ring bytes, with the scale of the packet applied: as many as `dst` holds.
    pub fn fill(&self, dst: &mut [MaybeUninit<u8>], from: usize) {
        for (i, d) in dst.as_chunks_mut::<8>().0.iter_mut().enumerate() {
            let c = self.sample(from + i);
            for (b, v) in d
                .iter_mut()
                .zip(c.re.to_le_bytes().into_iter().chain(c.im.to_le_bytes()))
            {
                b.write(v);
            }
        }
    }
}

/// The text of an ERROR code.
pub fn error_text(code: u8) -> String {
    match code {
        0 => "none".into(),
        1 => "invalid packet".into(),
        2 => "invalid command".into(),
        3 => "invalid argument".into(),
        n => format!("error {n}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    #[test]
    fn commands_encode_byte_for_byte() {
        let c = |c, a: &[u8]| command_bytes(c, a);
        assert_eq!(
            c(cmd::GET_UI, &[]),
            hex("00 00 00 00 0c 00 00 00 00 00 00 00")
        );
        assert_eq!(
            c(cmd::START, &[]),
            hex("00 00 00 00 0c 00 00 00 02 00 00 00")
        );
        assert_eq!(
            c(cmd::STOP, &[]),
            hex("00 00 00 00 0c 00 00 00 03 00 00 00")
        );
        assert_eq!(
            c(cmd::SET_SAMPLE_TYPE, &[1]),
            hex("00 00 00 00 0d 00 00 00 06 00 00 00 01")
        );
        assert_eq!(
            c(cmd::SET_SAMPLE_TYPE, &[0]),
            hex("00 00 00 00 0d 00 00 00 06 00 00 00 00")
        );
        assert_eq!(
            c(cmd::SET_COMPRESSION, &[0]),
            hex("00 00 00 00 0d 00 00 00 07 00 00 00 00")
        );
        assert_eq!(
            c(cmd::SET_FREQUENCY, &100e6f64.to_le_bytes()),
            hex("00 00 00 00 14 00 00 00 04 00 00 00 00 00 00 00 84 d7 97 41")
        );
        let f: f64 = 1e9 / (1.0 + 10.0 * 1e-6);
        assert_eq!(
            c(cmd::SET_FREQUENCY, &f.to_le_bytes()),
            hex("00 00 00 00 14 00 00 00 04 00 00 00 c4 cc 0c 78 51 cd cd 41")
        );
    }

    #[test]
    fn ui_actions_encode_byte_for_byte() {
        let a = |id: &str, v| command_bytes(cmd::UI_ACTION, &ui_action_arg(id, v, true));
        let with = |head: &str, id: &str, tail: &str| {
            let mut v = hex(head);
            v.extend_from_slice(id.as_bytes());
            v.extend(hex(tail));
            v
        };
        let id = "##_airspy_sr_sel_Airspy Source";
        assert_eq!(
            a(id, ActionValue::Int(1)),
            with(
                "00 00 00 00 33 00 00 00 01 00 00 00 01 04 1e 00",
                id,
                "02 01 00 00 00"
            )
        );
        let id = "##_airspy_lin_gain_Airspy Source";
        assert_eq!(
            a(id, ActionValue::Int(10)),
            with(
                "00 00 00 00 35 00 00 00 01 00 00 00 01 04 20 00",
                id,
                "02 0a 00 00 00"
            )
        );
        let id = "Bias T##_airspy_Airspy Source";
        assert_eq!(
            a(id, ActionValue::Bool(true)),
            with(
                "00 00 00 00 2f 00 00 00 01 00 00 00 01 04 1d 00",
                id,
                "01 01"
            )
        );
        let id = "Free##_airspy_gm_Airspy Source";
        assert_eq!(
            a(id, ActionValue::Bool(true)),
            with(
                "00 00 00 00 30 00 00 00 01 00 00 00 01 04 1e 00",
                id,
                "01 01"
            )
        );
        assert_eq!(
            a("##_x_gain_Src", ActionValue::Float(12.5)),
            hex("00 00 00 00 22 00 00 00 01 00 00 00 01 04 0d 00 23 23 5f 78 5f 67 61 69 6e 5f 53 72 63 03 00 00 48 41")
        );
        let long = "a".repeat(65_540);
        let arg = ui_action_arg(&long, ActionValue::Bool(true), true);
        assert_eq!(&arg[..4], &hex("01 04 ff ff")[..]);
        assert_eq!(arg.len(), 1 + 3 + 65_535 + 2);
    }

    #[test]
    fn headers_are_checked() {
        let h = |kind, size| Header { kind, size }.valid();
        assert!(!h(7, 12));
        assert!(!h(0, 7));
        assert!(h(0, 8));
        assert!(h(2, 32 << 20));
        assert!(!h(2, (32 << 20) + 1));
        let p = Header::parse(&[0x14, 0, 0, 0, 0x14, 0, 0, 0]);
        assert_eq!(p, Header { kind: 20, size: 20 });
        assert!(!p.valid());
    }

    fn samples(packet: &str) -> Option<Vec<Complex32>> {
        let p = hex(packet);
        let h = Header::parse(p[..8].try_into().unwrap());
        assert!(h.valid());
        assert_eq!(h.body_len(), p.len() - 8);
        let b = Baseband::parse(&p[8..])?;
        Some((0..b.len()).map(|i| b.sample(i)).collect())
    }

    fn near(a: &[Complex32], b: &[(f32, f32)]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(x, y)| (x.re - y.0).abs() < 1e-5 && (x.im - y.1).abs() < 1e-5)
    }

    #[test]
    fn samples_decode() {
        let s = samples("02 00 00 00 18 00 00 00 00 00 01 00 00 00 80 3f 00 40 00 e0 00 80 ff 7f")
            .unwrap();
        assert!(near(&s, &[(0.5, -0.25), (-1.0, 0.999969)]), "{s:?}");
        let s = samples("02 00 00 00 14 00 00 00 00 00 00 00 00 00 00 3f 40 e0 80 7f").unwrap();
        assert!(near(&s, &[(0.25, -0.125), (-0.5, 0.496094)]), "{s:?}");
        let s = samples("02 00 00 00 18 00 00 00 00 00 02 00 00 00 e0 40 00 00 00 3f 00 00 80 be")
            .unwrap();
        assert!(near(&s, &[(0.5, -0.25)]), "{s:?}");
        assert!(samples("02 00 00 00 10 00 00 00 00 00 07 00 00 00 80 3f").is_none());
        assert!(samples("02 00 00 00 0c 00 00 00 00 00 01 00").is_none());
        // A trailing part of a sample is ignored.
        let raw = hex("00 00 01 00 00 00 80 3f 00 40 00 e0 01 02");
        let b = Baseband::parse(&raw).unwrap();
        assert_eq!(b.len(), 1);
        let mut dst = [MaybeUninit::uninit(); 8];
        b.fill(&mut dst, 0);
        // SAFETY: fill wrote the 8 bytes of the one sample.
        let v: [u8; 8] = dst.map(|d| unsafe { d.assume_init() });
        let re = f32::from_le_bytes([v[0], v[1], v[2], v[3]]);
        let im = f32::from_le_bytes([v[4], v[5], v[6], v[7]]);
        assert!(near(&[Complex32::new(re, im)], &[(0.5, -0.25)]));
    }

    #[test]
    fn server_commands_and_errors_decode() {
        let p = hex("00 00 00 00 14 00 00 00 80 00 00 00 00 00 00 00 d0 12 63 41");
        let h = Header::parse(p[..8].try_into().unwrap());
        assert_eq!((h.kind, h.body_len()), (pkt::COMMAND, 12));
        assert_eq!(
            u32::from_le_bytes(p[8..12].try_into().unwrap()),
            cmd::SET_SAMPLERATE
        );
        assert_eq!(f64::from_le_bytes(p[12..20].try_into().unwrap()), 10e6);
        let p = hex("00 00 00 00 14 00 00 00 80 00 00 00 00 00 00 00 d0 12 43 41");
        assert_eq!(f64::from_le_bytes(p[12..20].try_into().unwrap()), 2.5e6);
        assert_eq!(error_text(2), "invalid command");
        assert_eq!(error_text(9), "error 9");
        assert_eq!(ActionValue::Float(12.5).to_string(), "12.5");
        assert_eq!(ActionValue::Bool(true).to_string(), "1");
    }
}
