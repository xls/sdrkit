//! The rtl_tcp server protocol on the serving core. A client gets a 12-byte greeting (four ASCII magic bytes, the tuner type
//! and the number of tuner gains, both 32-bit big endian), then unsigned 8-bit I/Q, interleaved, without end. A client sends
//! 5-byte commands: a command byte and a 32-bit big-endian value. Every command becomes a [`Request`]; a command byte that
//! is not known becomes [`Request::Other`] named `unknown`, and the connection stays.

use sdrkit_core::net::Conn;
use sdrkit_driver_core::rtl_tcp::{cmd, Greeting};
use sdrkit_serve_core::{ConnOutlet, Context, Parser, Protocol, Request, SampleType, Session};

pub use sdrkit_driver_core::rtl_tcp::DEFAULT_PORT;

/// The command bytes that the client driver does not send.
pub mod more_cmd {
    /// IF gain: the stage in the high 16 bits, the gain (tenths of a dB, signed) in the low 16.
    pub const IF_GAIN: u8 = 0x06;
    /// Test mode of the RTL2832 (a counter instead of samples).
    pub const TEST_MODE: u8 = 0x07;
    /// Offset tuning of the tuner.
    pub const OFFSET_TUNING: u8 = 0x0a;
    /// Crystal frequency of the RTL2832, Hz.
    pub const RTL_XTAL: u8 = 0x0b;
    /// Crystal frequency of the tuner, Hz.
    pub const TUNER_XTAL: u8 = 0x0c;
    /// Tuner gain by its index in the gain list.
    pub const GAIN_INDEX: u8 = 0x0d;
}

/// The rtl_tcp protocol with the greeting it announces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RtlTcp {
    magic: [u8; 4],
    tuner: u32,
    gains: Vec<i32>,
}

impl RtlTcp {
    /// A server that announces `magic`, tuner type `tuner` (1 E4000, 2 FC0012, 3 FC0013, 4 FC2580, 5 R820T, 6 R828D, others
    /// for other radios) and the tuner gains `gains_tenth_db` (tenths of a dB, ascending). The greeting states their number,
    /// and command 0x0d picks a gain from this list.
    pub fn new(magic: [u8; 4], tuner: u32, gains_tenth_db: &[i32]) -> RtlTcp {
        RtlTcp {
            magic,
            tuner,
            gains: gains_tenth_db.to_vec(),
        }
    }

    /// A server that announces itself as an RTL-SDR with tuner type `tuner` (1 to 6) and that tuner's gain list.
    pub fn rtl_sdr(tuner: u32) -> RtlTcp {
        let g = Greeting {
            magic: *b"RTL0",
            tuner,
            gains: 0,
        };
        RtlTcp::new(*b"RTL0", tuner, g.gain_table())
    }

    /// The 12 bytes sent first.
    pub fn greeting_bytes(&self) -> [u8; 12] {
        let mut b = [0u8; 12];
        b[..4].copy_from_slice(&self.magic);
        b[4..8].copy_from_slice(&self.tuner.to_be_bytes());
        b[8..].copy_from_slice(&(self.gains.len() as u32).to_be_bytes());
        b
    }

    /// The gain list, tenths of a dB.
    pub fn gains(&self) -> &[i32] {
        &self.gains
    }
}

impl Protocol for RtlTcp {
    fn name(&self) -> &'static str {
        "rtl_tcp"
    }
    fn session(&self, conn: &dyn Conn, _source: SampleType) -> Session {
        Session {
            greeting: self.greeting_bytes().to_vec(),
            outlet: conn
                .try_clone()
                .ok()
                .map(|c| Box::new(ConnOutlet::new(c, SampleType::Cu8)) as _),
            parser: Box::new(Commands {
                gains: self.gains.clone(),
                partial: [0; 5],
                have: 0,
            }),
        }
    }
}

/// The request for command byte `c` with value `v`. `gains` is the announced gain list (tenths of a dB), for command 0x0d.
pub fn request(c: u8, v: u32, gains: &[i32]) -> Request {
    let signed = v as i32;
    let other = |name: &'static str, a: i64, b: i64| Request::Other { name, args: [a, b] };
    match c {
        cmd::FREQ => Request::Frequency { hz: f64::from(v) },
        cmd::RATE => Request::Rate { hz: f64::from(v) },
        cmd::GAIN_MODE => Request::Agc { on: v == 0 },
        cmd::GAIN => Request::Gain {
            db: f64::from(signed) / 10.0,
        },
        cmd::PPM => Request::Correction {
            ppm: f64::from(signed),
        },
        more_cmd::IF_GAIN => other(
            "if_gain",
            i64::from(v >> 16),
            i64::from((v & 0xFFFF) as u16 as i16),
        ),
        more_cmd::TEST_MODE => other("test_mode", i64::from(v), 0),
        cmd::AGC => other("digital_agc", i64::from(v), 0),
        cmd::DIRECT => other("direct_sampling", i64::from(v), 0),
        more_cmd::OFFSET_TUNING => other("offset_tuning", i64::from(v), 0),
        more_cmd::RTL_XTAL => other("rtl_xtal", i64::from(v), 0),
        more_cmd::TUNER_XTAL => other("tuner_xtal", i64::from(v), 0),
        more_cmd::GAIN_INDEX => match gains.get(v as usize) {
            Some(g) => Request::Gain {
                db: f64::from(*g) / 10.0,
            },
            None => other("gain_index", i64::from(v), 0),
        },
        cmd::BIAS => Request::BiasTee { on: v != 0 },
        _ => other("unknown", i64::from(c), i64::from(v)),
    }
}

/// The command parser of one client: it keeps a partial command between reads.
struct Commands {
    gains: Vec<i32>,
    partial: [u8; 5],
    have: usize,
}

impl Commands {
    /// Takes the bytes of commands in any split and calls `submit` with each whole one.
    fn take(&mut self, bytes: &[u8], submit: &mut dyn FnMut(Request)) {
        for &b in bytes {
            self.partial[self.have] = b;
            self.have += 1;
            if self.have == 5 {
                self.have = 0;
                let p = self.partial;
                let v = u32::from_be_bytes([p[1], p[2], p[3], p[4]]);
                submit(request(p[0], v, &self.gains));
            }
        }
    }
}

impl Parser for Commands {
    fn feed(&mut self, bytes: &[u8], cx: &mut Context<'_>) {
        self.take(bytes, &mut |r| {
            cx.request(r);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_driver_core::rtl_tcp::command;

    #[test]
    fn the_greeting_states_what_the_caller_announces() {
        let r = RtlTcp::rtl_sdr(5);
        assert_eq!(r.greeting_bytes(), *b"RTL0\0\0\0\x05\0\0\0\x1d");
        let g = Greeting::parse(&r.greeting_bytes()).unwrap();
        assert_eq!(
            (g.tuner_name(), g.gains, g.is_other()),
            ("R820T", 29, false)
        );
        let o = RtlTcp::new(*b"RSP0", 9, &[0, 100, 200]);
        assert_eq!(o.greeting_bytes(), *b"RSP0\0\0\0\x09\0\0\0\x03");
        assert!(Greeting::parse(&o.greeting_bytes()).unwrap().is_other());
    }

    #[test]
    fn every_command_becomes_a_request() {
        let g = RtlTcp::rtl_sdr(5);
        let gains = g.gains();
        let o = |name, a, b| Request::Other { name, args: [a, b] };
        let cases: [(u8, u32, Request); 16] = [
            (0x01, 104_900_000, Request::Frequency { hz: 104.9e6 }),
            (0x02, 2_400_000, Request::Rate { hz: 2.4e6 }),
            (0x03, 0, Request::Agc { on: true }),
            (0x03, 1, Request::Agc { on: false }),
            (0x04, 297, Request::Gain { db: 29.7 }),
            (0x04, -10i32 as u32, Request::Gain { db: -1.0 }),
            (0x05, -5i32 as u32, Request::Correction { ppm: -5.0 }),
            (
                0x06,
                (3 << 16) | (-40i16 as u16 as u32),
                o("if_gain", 3, -40),
            ),
            (0x07, 1, o("test_mode", 1, 0)),
            (0x08, 1, o("digital_agc", 1, 0)),
            (0x09, 2, o("direct_sampling", 2, 0)),
            (0x0a, 1, o("offset_tuning", 1, 0)),
            (0x0b, 28_800_000, o("rtl_xtal", 28_800_000, 0)),
            (0x0c, 28_800_000, o("tuner_xtal", 28_800_000, 0)),
            (0x0d, 28, Request::Gain { db: 49.6 }),
            (0x0e, 1, Request::BiasTee { on: true }),
        ];
        for (c, v, want) in cases {
            assert_eq!(request(c, v, gains), want, "command {c:#04x}");
        }
        assert_eq!(request(0x0d, 29, gains), o("gain_index", 29, 0));
        assert_eq!(request(0x42, 7, gains), o("unknown", 0x42, 7));
    }

    #[test]
    fn commands_split_anywhere_are_parsed_whole() {
        let mut p = Commands {
            gains: RtlTcp::rtl_sdr(5).gains().to_vec(),
            partial: [0; 5],
            have: 0,
        };
        let mut bytes = Vec::new();
        bytes.extend(command(cmd::FREQ, 100_000_000));
        bytes.extend(command(0x42, 1));
        bytes.extend(command(cmd::BIAS, 0));
        let mut got = Vec::new();
        for piece in bytes.chunks(3) {
            p.take(piece, &mut |r| got.push(r));
        }
        assert_eq!(
            got,
            [
                Request::Frequency { hz: 100e6 },
                Request::Other {
                    name: "unknown",
                    args: [0x42, 1]
                },
                Request::BiasTee { on: false }
            ]
        );
    }
}
