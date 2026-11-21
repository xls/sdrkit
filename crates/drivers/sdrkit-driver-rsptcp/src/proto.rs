//! The rsp_tcp wire format: the greeting, the extended greeting, and the 5-byte commands.

pub use sdrkit_driver_core::rtl_tcp::command;
use sdrkit_graph::stream::SampleType;

/// The rsp_tcp default port.
pub const DEFAULT_PORT: u16 = 1234;

/// The magic of an rsp_tcp server. Only this magic comes before an extended greeting.
pub const MAGIC_RSP: [u8; 4] = *b"RSP0";
/// The magic of an rtl_tcp server (and of rsp_tcp in its default mode).
pub const MAGIC_RTL: [u8; 4] = *b"RTL0";

/// The length of the extended greeting.
pub const EXTENDED_LEN: usize = 24;

/// The command bytes.
pub mod cmd {
    /// Centre frequency, Hz.
    pub const FREQ: u8 = 0x01;
    /// Sample rate, sps.
    pub const RATE: u8 = 0x02;
    /// Gain mode: 0 automatic, 1 manual.
    pub const GAIN_MODE: u8 = 0x03;
    /// Tuner gain, tenths of a dB on the R820T scale (compatible mode).
    pub const GAIN: u8 = 0x04;
    /// Frequency correction, whole ppm, signed.
    pub const PPM: u8 = 0x05;
    /// The rtl_tcp digital AGC (compatible mode).
    pub const RTL_AGC: u8 = 0x08;
    /// Bias tee in its rtl_tcp form (compatible mode).
    pub const RTL_BIAS: u8 = 0x0E;
    /// Antenna input: 0, 1 or 2.
    pub const ANTENNA: u8 = 0x1F;
    /// LNA state.
    pub const LNA: u8 = 0x20;
    /// IF gain reduction, 20 to 59 dB.
    pub const IFGR: u8 = 0x21;
    /// The radio's AGC: 0 off, 1 on.
    pub const AGC: u8 = 0x22;
    /// AGC set point, dBFS, signed.
    pub const AGC_SETPOINT: u8 = 0x23;
    /// Notch filters, a bit mask ([`super::notch`]).
    pub const NOTCH: u8 = 0x24;
    /// Bias-T (extended mode).
    pub const BIAS: u8 = 0x25;
    /// Clock output.
    pub const REFOUT: u8 = 0x26;
}

/// The bits of the notch mask (command 0x24).
pub mod notch {
    /// The FM/MW broadcast notch.
    pub const FM: u32 = 0x01;
    /// The DAB notch.
    pub const DAB: u32 = 0x02;
    /// The AM (MW) notch of the Hi-Z input.
    pub const AM: u32 = 0x04;
}

/// The capability bits of the extended greeting.
pub mod caps {
    /// The antenna input can be set (0x1F).
    pub const ANTENNA: u32 = 0x01;
    /// Bias-T (0x25).
    pub const BIAS: u32 = 0x02;
    /// The radio is a dual-tuner RSPduo.
    pub const DUO: u32 = 0x04;
    /// The FM/MW broadcast notch.
    pub const FM_NOTCH: u32 = 0x08;
    /// The clock output (0x26).
    pub const REFOUT: u32 = 0x10;
    /// The DAB notch.
    pub const DAB_NOTCH: u32 = 0x20;
    /// The AM notch of the Hi-Z input.
    pub const AM_NOTCH: u32 = 0x40;
}

/// A command with a signed value, sent as its 32-bit two's complement.
pub fn command_i(c: u8, value: i32) -> [u8; 5] {
    command(c, value as u32)
}

/// The first 12 bytes of every greeting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    /// 4 printable characters.
    pub magic: [u8; 4],
    /// The first word: the tuner type, or the version word of an extended greeting.
    pub w1: u32,
    /// The second word: the gain count, or the capability word of an extended greeting.
    pub w2: u32,
}

impl Head {
    /// Reads the 12 bytes. The magic must be printable ASCII. The words must each be at most 0xFFFF, except after `RSP0`,
    /// whose first word is a version word. `None`: not an rsp_tcp or rtl_tcp server.
    pub fn parse(b: &[u8; 12]) -> Option<Head> {
        let magic = [b[0], b[1], b[2], b[3]];
        let w1 = u32::from_be_bytes([b[4], b[5], b[6], b[7]]);
        let w2 = u32::from_be_bytes([b[8], b[9], b[10], b[11]]);
        let printable = magic.iter().all(|c| (0x20..=0x7E).contains(c));
        let small = w1 <= 0xFFFF && w2 <= 0xFFFF;
        (printable && (magic == MAGIC_RSP || small)).then_some(Head { magic, w1, w2 })
    }

    /// The magic is `RSP0`.
    pub fn is_rsp(&self) -> bool {
        self.magic == MAGIC_RSP
    }

    /// The version word has major 1 (any minor), as an extended greeting must.
    pub fn version_ok(&self) -> bool {
        self.w1 >> 16 == 1
    }

    /// The magic as text.
    pub fn magic_text(&self) -> String {
        String::from_utf8_lossy(&self.magic).into_owned()
    }
}

sdrkit_core::choice_enum! {
    /// The sample format on the link.
    pub enum SampleFormat {
        /// Unsigned 8-bit I/Q (format 1, and the rtl_tcp commands always).
        U8 = ("u8", "8-bit"),
        /// Signed 16-bit I/Q, little endian (format 2).
        S16 = ("s16", "16-bit"),
    }
}

impl SampleFormat {
    /// From the greeting's word: 1 or 2.
    pub fn from_word(w: u32) -> Option<SampleFormat> {
        match w {
            1 => Some(SampleFormat::U8),
            2 => Some(SampleFormat::S16),
            _ => None,
        }
    }
    /// Bytes per complex sample.
    pub fn bytes(&self) -> usize {
        match self {
            SampleFormat::U8 => 2,
            SampleFormat::S16 => 4,
        }
    }
    /// "8-bit" or "16-bit".
    pub fn name(&self) -> &'static str {
        match self {
            SampleFormat::U8 => "8-bit",
            SampleFormat::S16 => "16-bit",
        }
    }
    /// How one sample of this format is stored in the ring.
    pub fn sample_type(&self) -> SampleType {
        match self {
            SampleFormat::U8 => SampleType::Cu8,
            SampleFormat::S16 => SampleType::Cs16,
        }
    }
}

/// The 12 bytes after the head of an extended greeting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtendedTail {
    /// Reserved, 0.
    pub reserved: u32,
    /// The SDRplay API hardware version.
    pub hw: u32,
    /// The sample format word (1 or 2 valid).
    pub format: u32,
}

impl ExtendedTail {
    /// Reads bytes 12 to 23 of the extended greeting.
    pub fn parse(b: &[u8; 12]) -> ExtendedTail {
        let w = |i: usize| u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        ExtendedTail {
            reserved: w(0),
            hw: w(4),
            format: w(8),
        }
    }
}

/// (major, minor) of a version word.
pub fn version(w: u32) -> (u16, u16) {
    ((w >> 16) as u16, (w & 0xFFFF) as u16)
}
