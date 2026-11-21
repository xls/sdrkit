//! The SpyServer protocol: numbers, the command encoding, the message header, and the DEVICE_INFO and CLIENT_SYNC bodies.
//! Everything on the wire is little endian.
//!
//! Standard: Airspy SpyServer protocol description (`spyserver_protocol.h`), version 2.0.1700.

/// The protocol version this client announces: 2.0.1700, `(major << 24) | (minor << 16) | build`.
pub const PROTOCOL_VERSION: u32 = (2 << 24) | 1700;
/// The client name sent in HELLO.
pub const CLIENT_NAME: &str = "sdrkit";
/// The size of a server message header.
pub const HEADER_LEN: usize = 20;
/// The largest message body accepted (1 MiB); a larger size is not a SpyServer message.
pub const MAX_BODY: usize = 1 << 20;

/// Client command types.
pub mod cmd {
    /// HELLO: version, then the client name.
    pub const HELLO: u32 = 0;
    /// GET_SETTING (not used).
    pub const GET_SETTING: u32 = 1;
    /// SET_SETTING: id, value.
    pub const SET_SETTING: u32 = 2;
    /// PING (not used).
    pub const PING: u32 = 3;
}

/// Setting ids for SET_SETTING.
pub mod setting {
    /// A stream mode ([`super::stream`]).
    pub const STREAMING_MODE: u32 = 0;
    /// 0 off, 1 on.
    pub const STREAMING_ENABLED: u32 = 1;
    /// The gain index.
    pub const GAIN: u32 = 2;
    /// A stream format ([`super::format`]).
    pub const IQ_FORMAT: u32 = 100;
    /// The IQ centre, whole Hz.
    pub const IQ_FREQUENCY: u32 = 101;
    /// The decimation stage n: the IQ rate is the maximum rate / 2^n.
    pub const IQ_DECIMATION: u32 = 102;
    /// The server's digital gain on the IQ (not sent).
    pub const IQ_DIGITAL_GAIN: u32 = 103;
    /// FFT format (not sent).
    pub const FFT_FORMAT: u32 = 200;
    /// FFT centre (not sent).
    pub const FFT_FREQUENCY: u32 = 201;
    /// FFT decimation (not sent).
    pub const FFT_DECIMATION: u32 = 202;
    /// FFT dB offset (not sent).
    pub const FFT_DB_OFFSET: u32 = 203;
    /// FFT dB range (not sent).
    pub const FFT_DB_RANGE: u32 = 204;
    /// FFT width in bins (not sent).
    pub const FFT_DISPLAY_PIXELS: u32 = 205;
}

/// Server message types (the low 16 bits of the type word).
pub mod msg {
    /// The radio's description.
    pub const DEVICE_INFO: u32 = 0;
    /// This client's control state and the tuning.
    pub const CLIENT_SYNC: u32 = 1;
    /// Answer to PING.
    pub const PONG: u32 = 2;
    /// Answer to GET_SETTING.
    pub const READ_SETTING: u32 = 3;
    /// IQ, unsigned 8-bit.
    pub const UINT8_IQ: u32 = 100;
    /// IQ, signed 16-bit.
    pub const INT16_IQ: u32 = 101;
    /// IQ, signed 24-bit.
    pub const INT24_IQ: u32 = 102;
    /// IQ, 32-bit float.
    pub const FLOAT_IQ: u32 = 103;
    /// Audio, unsigned 8-bit.
    pub const UINT8_AF: u32 = 200;
    /// Audio, signed 16-bit.
    pub const INT16_AF: u32 = 201;
    /// Audio, signed 24-bit.
    pub const INT24_AF: u32 = 202;
    /// Audio, float.
    pub const FLOAT_AF: u32 = 203;
    /// FFT, two 4-bit bins a byte.
    pub const DINT4_FFT: u32 = 300;
    /// FFT, one byte a bin.
    pub const UINT8_FFT: u32 = 301;
}

/// Stream types (header word 2) and stream modes (sets of stream types).
pub mod stream {
    /// Status messages.
    pub const STATUS: u32 = 0;
    /// IQ.
    pub const IQ: u32 = 1;
    /// Audio.
    pub const AF: u32 = 2;
    /// FFT.
    pub const FFT: u32 = 4;
    /// Mode: IQ only (the mode sdrkit requests).
    pub const MODE_IQ_ONLY: u32 = 1;
    /// Mode: audio only.
    pub const MODE_AF_ONLY: u32 = 2;
    /// Mode: FFT only.
    pub const MODE_FFT_ONLY: u32 = 4;
    /// Mode: FFT and IQ.
    pub const MODE_FFT_IQ: u32 = 5;
    /// Mode: FFT and audio.
    pub const MODE_FFT_AF: u32 = 6;
}

/// Stream formats.
pub mod format {
    /// None.
    pub const INVALID: u32 = 0;
    /// Unsigned 8-bit.
    pub const UINT8: u32 = 1;
    /// Signed 16-bit.
    pub const INT16: u32 = 2;
    /// Signed 24-bit.
    pub const INT24: u32 = 3;
    /// 32-bit float.
    pub const FLOAT: u32 = 4;
    /// Two 4-bit values a byte (FFT only).
    pub const DINT4: u32 = 5;

    /// Bytes per complex sample of an IQ format; 0 for one that is not an IQ format.
    pub fn bytes_per_sample(f: u32) -> usize {
        match f {
            UINT8 => 2,
            INT16 => 4,
            INT24 => 6,
            FLOAT => 8,
            _ => 0,
        }
    }

    /// A short name ("8-bit").
    pub fn name(f: u32) -> &'static str {
        match f {
            UINT8 => "8-bit",
            INT16 => "16-bit",
            INT24 => "24-bit",
            FLOAT => "float",
            _ => "none",
        }
    }
}

/// The IQ format of an IQ message type, or None for any other message.
pub fn iq_format_of(msg_type: u32) -> Option<u32> {
    match msg_type {
        msg::UINT8_IQ => Some(format::UINT8),
        msg::INT16_IQ => Some(format::INT16),
        msg::INT24_IQ => Some(format::INT24),
        msg::FLOAT_IQ => Some(format::FLOAT),
        _ => None,
    }
}

/// HELLO with a client name: type 0, body size 4 + the name's length, the version, the name (no NUL).
pub fn hello(name: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(12 + name.len());
    v.extend_from_slice(&cmd::HELLO.to_le_bytes());
    v.extend_from_slice(&(4 + name.len() as u32).to_le_bytes());
    v.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    v.extend_from_slice(name.as_bytes());
    v
}

/// A SET_SETTING command: always 16 bytes.
pub fn set_setting(id: u32, value: u32) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[0..4].copy_from_slice(&cmd::SET_SETTING.to_le_bytes());
    b[4..8].copy_from_slice(&8u32.to_le_bytes());
    b[8..12].copy_from_slice(&id.to_le_bytes());
    b[12..16].copy_from_slice(&value.to_le_bytes());
    b
}

/// A server message header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// The server's protocol version word (not checked).
    pub protocol: u32,
    /// The message type (low 16 bits of the type word).
    pub msg_type: u32,
    /// The flags (high 16 bits): for IQ, the digital gain in dB the server applied.
    pub flags: u32,
    /// The stream type.
    pub stream_type: u32,
    /// The sequence number: the stream fills the IQ messages it shows missing.
    pub sequence: u32,
    /// The body size in bytes.
    pub body_size: u32,
}

fn word(b: &[u8], i: usize) -> u32 {
    b.get(i * 4..i * 4 + 4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .unwrap_or(0)
}

impl Header {
    /// Reads the 20 bytes.
    pub fn parse(b: &[u8; HEADER_LEN]) -> Header {
        let t = word(b, 1);
        Header {
            protocol: word(b, 0),
            msg_type: t & 0xFFFF,
            flags: t >> 16,
            stream_type: word(b, 2),
            sequence: word(b, 3),
            body_size: word(b, 4),
        }
    }

    /// The body is within the 1 MiB limit.
    pub fn size_ok(&self) -> bool {
        self.body_size as usize <= MAX_BODY
    }

    /// The header's bytes (for fake servers).
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        let w = [
            self.protocol,
            self.msg_type | (self.flags << 16),
            self.stream_type,
            self.sequence,
            self.body_size,
        ];
        for (i, v) in w.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

/// The device types of DEVICE_INFO.
pub mod device {
    /// No radio.
    pub const INVALID: u32 = 0;
    /// Airspy R2 or Mini.
    pub const AIRSPY_ONE: u32 = 1;
    /// Airspy HF+ family.
    pub const AIRSPY_HF: u32 = 2;
    /// RTL-SDR.
    pub const RTLSDR: u32 = 3;
}

/// DEVICE_INFO: what the server's radio is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// The device type ([`device`]).
    pub device_type: u32,
    /// The serial number.
    pub serial: u32,
    /// The radio's full IQ rate, sps.
    pub max_sample_rate: u32,
    /// The maximum bandwidth, Hz.
    pub max_bandwidth: u32,
    /// The decimation stage count.
    pub decimation_stages: u32,
    /// The gain stage count.
    pub gain_stages: u32,
    /// The maximum gain index.
    pub max_gain_index: u32,
    /// The minimum frequency, Hz.
    pub min_frequency: u32,
    /// The maximum frequency, Hz.
    pub max_frequency: u32,
    /// The resolution, bits.
    pub resolution: u32,
    /// The minimum IQ decimation.
    pub min_iq_decimation: u32,
    /// The forced IQ format: 0 none.
    pub forced_iq_format: u32,
}

impl DeviceInfo {
    /// Reads the 12 words; words past the end of a short body read as 0.
    pub fn parse(b: &[u8]) -> DeviceInfo {
        DeviceInfo {
            device_type: word(b, 0),
            serial: word(b, 1),
            max_sample_rate: word(b, 2),
            max_bandwidth: word(b, 3),
            decimation_stages: word(b, 4),
            gain_stages: word(b, 5),
            max_gain_index: word(b, 6),
            min_frequency: word(b, 7),
            max_frequency: word(b, 8),
            resolution: word(b, 9),
            min_iq_decimation: word(b, 10),
            forced_iq_format: word(b, 11),
        }
    }

    /// The 48-byte body (for fake servers).
    pub fn to_bytes(&self) -> Vec<u8> {
        [
            self.device_type,
            self.serial,
            self.max_sample_rate,
            self.max_bandwidth,
            self.decimation_stages,
            self.gain_stages,
            self.max_gain_index,
            self.min_frequency,
            self.max_frequency,
            self.resolution,
            self.min_iq_decimation,
            self.forced_iq_format,
        ]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect()
    }

    /// The radio's name for messages.
    pub fn device_name(&self) -> &'static str {
        match self.device_type {
            device::AIRSPY_ONE => "Airspy",
            device::AIRSPY_HF => "Airspy HF+",
            device::RTLSDR => "RTL-SDR",
            _ => "radio",
        }
    }

    /// No radio is plugged into the server.
    pub fn no_radio(&self) -> bool {
        self.device_type == device::INVALID || self.max_sample_rate == 0
    }

    /// What a server not seen yet is taken to be: an Airspy R2 (10 Msps, 9 stages, minimum 0, 21 gains, 24 to 1800 MHz).
    pub fn airspy_r2() -> DeviceInfo {
        DeviceInfo {
            device_type: device::AIRSPY_ONE,
            serial: 0,
            max_sample_rate: 10_000_000,
            max_bandwidth: 8_000_000,
            decimation_stages: 9,
            gain_stages: 1,
            max_gain_index: 21,
            min_frequency: 24_000_000,
            max_frequency: 1_800_000_000,
            resolution: 12,
            min_iq_decimation: 0,
            forced_iq_format: 0,
        }
    }
}

/// CLIENT_SYNC: this client's control state and the server's tuning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientSync {
    /// This client controls the radio.
    pub can_control: bool,
    /// The current gain index.
    pub gain: u32,
    /// The radio's tuner centre, Hz.
    pub device_center: u32,
    /// The IQ centre this client gets, Hz.
    pub iq_center: u32,
    /// The FFT centre, Hz.
    pub fft_center: u32,
    /// The lowest IQ centre allowed, Hz.
    pub min_iq_center: u32,
    /// The highest IQ centre allowed, Hz.
    pub max_iq_center: u32,
    /// The lowest FFT centre, Hz.
    pub min_fft_center: u32,
    /// The highest FFT centre, Hz.
    pub max_fft_center: u32,
}

impl ClientSync {
    /// Reads the 9 words; words past the end of a short body read as 0.
    pub fn parse(b: &[u8]) -> ClientSync {
        ClientSync {
            can_control: word(b, 0) != 0,
            gain: word(b, 1),
            device_center: word(b, 2),
            iq_center: word(b, 3),
            fft_center: word(b, 4),
            min_iq_center: word(b, 5),
            max_iq_center: word(b, 6),
            min_fft_center: word(b, 7),
            max_fft_center: word(b, 8),
        }
    }

    /// The 36-byte body (for fake servers).
    pub fn to_bytes(&self) -> Vec<u8> {
        [
            self.can_control as u32,
            self.gain,
            self.device_center,
            self.iq_center,
            self.fft_center,
            self.min_iq_center,
            self.max_iq_center,
            self.min_fft_center,
            self.max_fft_center,
        ]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect()
    }

    /// The IQ centre range is usable: its maximum is above 0 and at least its minimum.
    pub fn iq_range_valid(&self) -> bool {
        self.max_iq_center > 0 && self.max_iq_center >= self.min_iq_center
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_word_and_commands() {
        assert_eq!(PROTOCOL_VERSION, 0x020006A4);
        assert_eq!(
            hello("sdrkit"),
            [0, 0, 0, 0, 10, 0, 0, 0, 0xa4, 6, 0, 2, b's', b'd', b'r', b'k', b'i', b't']
        );
        assert_eq!(&hello("abcde")[4..8], &[9, 0, 0, 0]);
        assert_eq!(
            set_setting(setting::IQ_FREQUENCY, 100_000_000),
            [2, 0, 0, 0, 8, 0, 0, 0, 0x65, 0, 0, 0, 0x00, 0xe1, 0xf5, 0x05]
        );
    }

    #[test]
    fn headers_carry_type_and_flags() {
        let b = [
            0xa4, 6, 0, 2, 0x64, 0, 1, 0, 1, 0, 0, 0, 7, 0, 0, 0, 4, 0, 0, 0,
        ];
        let h = Header::parse(&b);
        assert_eq!(
            (h.msg_type, h.flags, h.stream_type, h.sequence, h.body_size),
            (100, 1, 1, 7, 4)
        );
        assert_eq!(h.to_bytes(), b);
        let mut h2 = h;
        h2.body_size = MAX_BODY as u32;
        assert!(h2.size_ok());
        h2.body_size += 1;
        assert!(!h2.size_ok());
    }

    #[test]
    fn short_bodies_read_zero_words() {
        let d = DeviceInfo {
            device_type: 2,
            serial: 7,
            max_sample_rate: 768_000,
            max_bandwidth: 600_000,
            decimation_stages: 6,
            gain_stages: 3,
            max_gain_index: 3,
            min_frequency: 100_000,
            max_frequency: 31_000_000,
            resolution: 16,
            min_iq_decimation: 1,
            forced_iq_format: 0,
        };
        let b = d.to_bytes();
        assert_eq!(DeviceInfo::parse(&b), d);
        let s = DeviceInfo::parse(&b[..20]);
        assert_eq!(
            (
                s.max_sample_rate,
                s.decimation_stages,
                s.gain_stages,
                s.max_frequency
            ),
            (768_000, 6, 0, 0)
        );
        assert_eq!(s.device_name(), "Airspy HF+");
        assert_eq!(
            ClientSync::parse(&[1, 0, 0, 0]),
            ClientSync {
                can_control: true,
                ..Default::default()
            }
        );
    }
}
