//! The USB protocol of the HackRF family: ids, request numbers, the firmware version rules, the byte layouts of the
//! requests and replies, and the rules that choose a rate ratio, a baseband filter and a gain split.

use crate::error::HackrfError;

/// The vendor id of every HackRF board.
pub const VENDOR_ID: u16 = 0x1D50;
/// The product id of the HackRF One and the HackRF Pro.
pub const PRODUCT_ONE: u16 = 0x6089;
/// The product id of the Jawbreaker.
pub const PRODUCT_JAWBREAKER: u16 = 0x604B;
/// The product id of the rad1o.
pub const PRODUCT_RAD1O: u16 = 0xCC15;
/// The vendor id of the boot loader of the chip (DFU mode).
pub const DFU_VENDOR_ID: u16 = 0x1FC9;
/// The product id of the boot loader of the chip (DFU mode).
pub const DFU_PRODUCT_ID: u16 = 0x000C;

/// The one interface.
pub const INTERFACE: u8 = 0;
/// The bulk IN endpoint: receive samples.
pub const EP_RX: u8 = 0x81;
/// The bulk OUT endpoint: transmit samples.
pub const EP_TX: u8 = 0x02;
/// The block the firmware moves on both bulk endpoints.
pub const BLOCK: usize = 16_384;
/// The device buffer of firmware below USB API 0x0112, and the bytes the device needs before it sends to the DAC.
pub const FIXED_BUFFER: usize = 32_768;
/// The device buffer of firmware from USB API 0x0112 on, when the driver has not read it.
pub const DRAIN_BUFFER: usize = 65_536;
/// The highest frequency the firmware takes.
pub const MAX_FREQ_HZ: u64 = 7_250_000_000;
/// The highest sample rate.
pub const MAX_RATE_HZ: f64 = 20e6;
/// The sample rate when the host asks for none.
pub const DEFAULT_RATE_HZ: f64 = 8e6;
/// The lowest rate the maker recommends.
pub const CLEAN_RATE_HZ: f64 = 8e6;

/// The vendor request numbers the driver uses.
pub mod request {
    /// Transceiver mode (value: [`super::Mode`]).
    pub const MODE: u8 = 1;
    /// Sample rate: frequency and divider.
    pub const SAMPLE_RATE: u8 = 6;
    /// Baseband filter bandwidth.
    pub const FILTER: u8 = 7;
    /// Board id.
    pub const BOARD_ID: u8 = 14;
    /// Firmware version string.
    pub const VERSION: u8 = 15;
    /// Frequency, simple form.
    pub const FREQUENCY: u8 = 16;
    /// RF amplifier on or off.
    pub const AMP: u8 = 17;
    /// Part id and serial number.
    pub const PART_SERIAL: u8 = 18;
    /// LNA gain (receive IF stage).
    pub const LNA: u8 = 19;
    /// VGA gain (receive baseband stage).
    pub const VGA: u8 = 20;
    /// TX VGA gain (transmit IF stage).
    pub const TX_VGA: u8 = 21;
    /// Antenna port power.
    pub const ANTENNA_POWER: u8 = 23;
    /// Clock output enable.
    pub const CLOCK_OUT: u8 = 32;
    /// Streaming state.
    pub const STREAM_STATE: u8 = 41;
    /// Clock input status.
    pub const CLOCK_IN: u8 = 44;
    /// Board revision.
    pub const BOARD_REVISION: u8 = 45;
    /// Supported platforms.
    pub const PLATFORMS: u8 = 46;
    /// Device buffer size.
    pub const BUFFER_SIZE: u8 = 61;
}

/// The standard GET_DESCRIPTOR request.
pub const GET_DESCRIPTOR: u8 = 6;

/// A transceiver mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum Mode {
    /// Idle: streaming stops, the RF path is off.
    Off = 0,
    /// Samples stream on endpoint 0x81.
    Receive = 1,
    /// Samples are taken on endpoint 0x02.
    Transmit = 2,
}

/// The lowest USB API version that has `request` (with `value` for the transceiver mode); 0 for a request present since
/// the first public firmware.
///
/// # Errors
///
/// [`HackrfError::Forbidden`] for a request the driver never sends: register, flash, logic and firmware access, the
/// reserved and the CPLD update modes, the antenna switch board, and the numbers with no request.
pub fn min_api(request: u8, value: u16) -> Result<u16, HackrfError> {
    let forbidden = Err(HackrfError::Forbidden { request, value });
    Ok(match request {
        request::MODE => match value {
            0..=2 => 0,
            5 => 0x0104,
            _ => return forbidden,
        },
        6 | 7 | 14..=21 | 23 | 24 => 0,
        26 | 29 | 30 => 0x0102,
        32 => 0x0103,
        37 => 0x0104,
        41..=46 => 0x0106,
        47 => 0x0107,
        48 => 0x0108,
        51 | 52 | 53 | 55 | 56 => 0x0109,
        61 => 0x0112,
        62 => 0x0113,
        _ => return forbidden,
    })
}

/// Checks `request` with `value` against the version rules for a device with USB API `api` and board `board`.
///
/// # Errors
///
/// [`HackrfError::Forbidden`], [`HackrfError::NotSupported`] when `api` is too low, [`HackrfError::ProOnly`] for a port
/// request on another board, [`HackrfError::NoAntennaPower`] for antenna power on a board without it.
pub fn check(request: u8, value: u16, api: u16, board: Board) -> Result<(), HackrfError> {
    let need = min_api(request, value)?;
    if api < need {
        return Err(HackrfError::NotSupported {
            request,
            need,
            have: api,
        });
    }
    if matches!(request, 49..=55) && board != Board::Pro {
        return Err(HackrfError::ProOnly { request });
    }
    if request == request::ANTENNA_POWER && !board.antenna_power() {
        return Err(HackrfError::NoAntennaPower);
    }
    Ok(())
}

/// The USB API version as text: 0x0110 is "1.10".
pub fn api_text(api: u16) -> String {
    format!("{:x}.{:02x}", api >> 8, api & 0xff)
}

/// A board of the family, from its board id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Board {
    /// Board id 1.
    Jawbreaker,
    /// Board id 2: HackRF One before r9.
    One,
    /// Board id 3.
    Rad1o,
    /// Board id 4: HackRF One r9 and later.
    OneR9,
    /// Board id 5, in its 8-bit sample mode.
    Pro,
    /// Board id 0xFE (unrecognized) or 0xFF (not detected), driven as a HackRF One.
    Unknown(u8),
}

impl Board {
    /// The board of board id `id`.
    ///
    /// # Errors
    ///
    /// [`HackrfError::Prototype`] for board id 0.
    pub fn from_id(id: u8) -> Result<Board, HackrfError> {
        Ok(match id {
            0 => return Err(HackrfError::Prototype),
            1 => Board::Jawbreaker,
            2 => Board::One,
            3 => Board::Rad1o,
            4 => Board::OneR9,
            5 => Board::Pro,
            n => Board::Unknown(n),
        })
    }

    /// The name of the board.
    pub fn name(self) -> &'static str {
        match self {
            Board::Jawbreaker => "HackRF Jawbreaker",
            Board::One => "HackRF One",
            Board::Rad1o => "rad1o",
            Board::OneR9 => "HackRF One r9",
            Board::Pro => "HackRF Pro (8-bit samples)",
            Board::Unknown(_) => "HackRF (board not recognized)",
        }
    }

    /// The board has antenna port power.
    pub fn antenna_power(self) -> bool {
        !matches!(self, Board::Jawbreaker | Board::Rad1o)
    }

    /// The board shows a DC spike at the tuned centre.
    pub fn dc_spike(self) -> bool {
        self != Board::Pro
    }

    /// The lowest sample rate of the board.
    pub fn min_rate_hz(self) -> f64 {
        if self == Board::Jawbreaker {
            8e6
        } else {
            2e6
        }
    }

    /// The range the board is specified for, in Hz. The firmware takes 0 to 7250 MHz with reduced performance outside it.
    pub fn specified_hz(self) -> (f64, f64) {
        match self {
            Board::Pro => (100e3, 6e9),
            Board::Jawbreaker => (30e6, 6e9),
            Board::Rad1o => (50e6, 4e9),
            Board::One | Board::OneR9 | Board::Unknown(_) => (1e6, 6e9),
        }
    }

    /// The typical maximum transmit power in dBm at increasing frequencies: lines through the bands of the maker's
    /// tables, the middle of a band whose table gives no direction. Empty where no figures are published.
    pub fn tx_output_dbm(self) -> Vec<(f64, f64)> {
        match self {
            Board::One | Board::OneR9 | Board::Unknown(_) => vec![
                (1e6, 5.0),
                (10e6, 15.0),
                (2_169e6, 5.0),
                (2_170e6, 14.0),
                (2_740e6, 14.0),
                (2_741e6, 5.0),
                (4_000e6, 0.0),
                (6_000e6, -10.0),
            ],
            Board::Jawbreaker => vec![
                (30e6, 10.0),
                (100e6, 10.0),
                (101e6, 5.0),
                (2_169e6, 5.0),
                (2_170e6, 12.5),
                (2_740e6, 12.5),
                (2_741e6, 0.0),
                (4_000e6, 0.0),
                (4_001e6, -7.5),
                (6_000e6, -7.5),
            ],
            Board::Pro | Board::Rad1o => Vec::new(),
        }
    }
}

/// The board revision as text: "r9", "Pro 1.0", with the maker named when bit 7 is set.
pub fn revision_text(rev: u8) -> String {
    match rev {
        0xFE => return "unrecognized".into(),
        0xFF => return "not detected".into(),
        _ => {}
    }
    let name = match rev & 0x7F {
        0 => "older than r6".to_string(),
        1 => "r6".into(),
        2 => "r7".into(),
        3 => "r8".into(),
        4 => "r9".into(),
        5 => "r10".into(),
        6 => "Pro 0.1".into(),
        7 => "Pro 0.2".into(),
        8 => "Pro 0.3".into(),
        9 => "Pro 1.0".into(),
        10 => "Pro 1.1".into(),
        11 => "Pro 1.2".into(),
        n => format!("revision {n}"),
    };
    if rev & 0x80 != 0 {
        format!("{name}, built by Great Scott Gadgets")
    } else {
        name
    }
}

/// The names of the bits of the supported-platforms reply, from bit 0.
pub const PLATFORMS: [&str; 5] = [
    "Jawbreaker",
    "HackRF One before r9",
    "rad1o",
    "HackRF One r9",
    "HackRF Pro",
];

/// The platforms of the supported-platforms reply `mask` as a list.
pub fn platforms_text(mask: u32) -> String {
    let names: Vec<&str> = PLATFORMS
        .iter()
        .enumerate()
        .filter(|(i, _)| mask & (1 << i) != 0)
        .map(|(_, n)| *n)
        .collect();
    names.join(", ")
}

/// The USB API version (bcdDevice) of the device descriptor `d`.
pub fn api_from_descriptor(d: &[u8]) -> Option<u16> {
    (d.len() >= 14 && d[1] == 1).then(|| u16::from_le_bytes([d[12], d[13]]))
}

/// The reply of the part id and serial number request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartSerial {
    /// The two words of the chip's part number.
    pub part: [u32; 2],
    /// The four words of the serial number.
    pub serial: [u32; 4],
}

impl PartSerial {
    /// Reads the 24-byte reply.
    pub fn parse(b: &[u8]) -> Option<PartSerial> {
        if b.len() < 24 {
            return None;
        }
        let w = |i: usize| u32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
        Some(PartSerial {
            part: [w(0), w(1)],
            serial: [w(2), w(3), w(4), w(5)],
        })
    }

    /// The serial number as 32 hexadecimal digits, the words in reply order.
    pub fn serial_text(&self) -> String {
        let s = self.serial;
        format!("{:08x}{:08x}{:08x}{:08x}", s[0], s[1], s[2], s[3])
    }

    /// The part id as two words of 8 hexadecimal digits.
    pub fn part_text(&self) -> String {
        format!("{:08x} {:08x}", self.part[0], self.part[1])
    }
}

/// The parts of the streaming state reply that the driver reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamState {
    /// The active sampler mode.
    pub active: u32,
    /// Bytes the sampler moved (wraps at 2^32).
    pub sampled: u32,
    /// Shortfalls so far: overruns in receive, underruns in transmit.
    pub shortfalls: u32,
    /// The longest shortfall in bytes.
    pub longest: u32,
    /// The error that stopped the sampler (0 none).
    pub error: u32,
}

impl StreamState {
    /// Reads the 40-byte reply.
    pub fn parse(b: &[u8]) -> Option<StreamState> {
        if b.len() < 40 {
            return None;
        }
        let w = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        Some(StreamState {
            active: w(4),
            sampled: w(8),
            shortfalls: w(16),
            longest: w(20),
            error: w(36),
        })
    }
}

/// The data of the simple frequency request: whole MHz, then the remaining Hz, each 32-bit LE.
pub fn frequency_data(hz: u64) -> [u8; 8] {
    let mhz = hz / 1_000_000;
    let rest = hz - mhz * 1_000_000;
    let mut d = [0u8; 8];
    d[..4].copy_from_slice(&(mhz as u32).to_le_bytes());
    d[4..].copy_from_slice(&(rest as u32).to_le_bytes());
    d
}

/// The frequency and divider for `rate`: the smallest divider from 1 to 31 that makes `rate` times it a whole number,
/// else the rate rounded to the Hz and divider 1.
pub fn rate_ratio(rate: f64) -> (u32, u32) {
    for d in 1..=31u32 {
        let x = rate * f64::from(d);
        let r = x.round();
        if (x - r).abs() <= r.abs() * 4.0 * f64::EPSILON {
            return (r as u32, d);
        }
    }
    (rate.round() as u32, 1)
}

/// The data of the sample rate request for `freq` and `divider`.
pub fn rate_data(freq: u32, divider: u32) -> [u8; 8] {
    let mut d = [0u8; 8];
    d[..4].copy_from_slice(&freq.to_le_bytes());
    d[4..].copy_from_slice(&divider.to_le_bytes());
    d
}

/// A 32-bit number as the value (low 16 bits) and index (high 16 bits) of a setup packet.
pub fn split_u32(x: u32) -> (u16, u16) {
    (x as u16, (x >> 16) as u16)
}

/// The baseband filter bandwidths, in Hz.
pub const FILTERS_HZ: [u32; 16] = [
    1_750_000, 2_500_000, 3_500_000, 5_000_000, 5_500_000, 6_000_000, 7_000_000, 8_000_000,
    9_000_000, 10_000_000, 12_000_000, 14_000_000, 15_000_000, 20_000_000, 24_000_000, 28_000_000,
];

/// The automatic filter for `rate`: the widest bandwidth no more than 0.75 times the rate, at least 1.75 MHz.
pub fn auto_filter(rate: f64) -> u32 {
    let limit = 0.75 * rate;
    FILTERS_HZ
        .iter()
        .rev()
        .find(|&&f| f64::from(f) <= limit)
        .copied()
        .unwrap_or(FILTERS_HZ[0])
}

/// The filter for a requested width `hz`: the narrowest bandwidth equal to or more than it, at most 28 MHz.
pub fn filter_at_least(hz: f64) -> u32 {
    FILTERS_HZ
        .iter()
        .find(|&&f| f64::from(f) >= hz)
        .copied()
        .unwrap_or(FILTERS_HZ[15])
}

/// The highest LNA gain.
pub const LNA_MAX: u32 = 40;
/// The highest VGA gain.
pub const VGA_MAX: u32 = 62;
/// The highest TX VGA gain.
pub const TX_VGA_MAX: u32 = 47;
/// The highest total receive gain of LNA and VGA.
pub const GAIN_MAX: f64 = (LNA_MAX + VGA_MAX) as f64;

/// The LNA gain for `db`, rounded down to its 8 dB step.
pub fn lna_step(db: u32) -> u32 {
    db.min(LNA_MAX) / 8 * 8
}

/// The VGA gain for `db`, rounded down to its 2 dB step.
pub fn vga_step(db: u32) -> u32 {
    db.min(VGA_MAX) / 2 * 2
}

/// The LNA and VGA gains for a total of `total_db`: the two stages raised by about the same amount, the LNA on its 8 dB
/// steps and the VGA on its 2 dB steps, the sum at most the total.
pub fn split_gain(total_db: f64) -> (u32, u32) {
    let t = if total_db.is_finite() {
        total_db.clamp(0.0, GAIN_MAX)
    } else {
        0.0
    };
    let mut lna = ((t / 16.0).round() * 8.0).min(f64::from(LNA_MAX));
    if t - lna > f64::from(VGA_MAX) {
        lna = (((t - f64::from(VGA_MAX)) / 8.0).ceil() * 8.0).min(f64::from(LNA_MAX));
    }
    if lna > t {
        lna = (t / 8.0).floor() * 8.0;
    }
    let vga = (((t - lna) / 2.0).floor() * 2.0).clamp(0.0, f64::from(VGA_MAX));
    (lna as u32, vga as u32)
}

/// The size of one transmit write at `rate`: about 10 ms of samples in whole blocks, from 1 to 16 blocks.
pub fn tx_write_size(rate: f64) -> usize {
    let bytes = (rate.max(0.0) * 2.0 * 0.01) as usize;
    bytes.div_ceil(BLOCK).clamp(1, 16) * BLOCK
}

/// The byte of one transmit component `x` of full scale 1: `x` times 128, rounded and limited to -128 to 127.
pub fn tx_byte(x: f32) -> u8 {
    let v = (x * 128.0).round();
    let v = if v.is_nan() {
        0.0
    } else {
        v.clamp(-128.0, 127.0)
    };
    v as i8 as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filter_table_of_the_specification() {
        let table = [
            (2e6, 1_750_000),
            (4e6, 2_500_000),
            (8e6, 6_000_000),
            (10e6, 7_000_000),
            (12.5e6, 9_000_000),
            (16e6, 12_000_000),
            (20e6, 15_000_000),
        ];
        for (rate, filter) in table {
            assert_eq!(auto_filter(rate), filter, "{rate}");
        }
        assert_eq!(auto_filter(1e6), 1_750_000);
        assert_eq!(auto_filter(40e6), 28_000_000);
        assert_eq!(filter_at_least(6e6), 6_000_000);
        assert_eq!(filter_at_least(6.1e6), 7_000_000);
        assert_eq!(filter_at_least(1e6), 1_750_000);
        assert_eq!(filter_at_least(30e6), 28_000_000);
    }

    #[test]
    fn rates_become_a_frequency_and_the_smallest_divider() {
        assert_eq!(rate_ratio(8e6), (8_000_000, 1));
        assert_eq!(rate_ratio(25e6 / 3.0), (25_000_000, 3));
        assert_eq!(rate_ratio(12.5e6), (12_500_000, 1));
        assert_eq!(rate_ratio(10e6 / 7.0), (10_000_000, 7));
        assert_eq!(rate_ratio(20e6 / 31.0), (20_000_000, 31));
        // No divider up to 31 gives a whole number: the rate rounded, divider 1.
        assert_eq!(rate_ratio(8_000_000.123), (8_000_000, 1));
        assert_eq!(
            rate_data(8_000_000, 1),
            [0x00, 0x12, 0x7A, 0x00, 1, 0, 0, 0]
        );
    }

    #[test]
    fn frequencies_split_into_mhz_and_hz() {
        assert_eq!(
            frequency_data(433_920_000),
            [0xB1, 0x01, 0, 0, 0xC0, 0x09, 0x0E, 0]
        );
        assert_eq!(
            frequency_data(431_920_000),
            [0xAF, 0x01, 0, 0, 0xC0, 0x09, 0x0E, 0]
        );
        assert_eq!(split_u32(6_000_000), (0x8D80, 0x005B));
    }

    #[test]
    fn requests_follow_the_version_table() {
        assert_eq!(min_api(request::MODE, 1), Ok(0));
        assert_eq!(min_api(request::MODE, 5), Ok(0x0104));
        assert_eq!(min_api(request::CLOCK_OUT, 0), Ok(0x0103));
        assert_eq!(min_api(request::CLOCK_IN, 0), Ok(0x0106));
        assert_eq!(min_api(request::BUFFER_SIZE, 0), Ok(0x0112));
        assert_eq!(min_api(62, 0), Ok(0x0113));
        assert_eq!(
            check(request::CLOCK_IN, 0, 0x0105, Board::OneR9),
            Err(HackrfError::NotSupported {
                request: 44,
                need: 0x0106,
                have: 0x0105
            })
        );
        assert!(check(request::CLOCK_IN, 0, 0x0106, Board::OneR9).is_ok());
        assert_eq!(
            check(request::ANTENNA_POWER, 0, 0x0110, Board::Rad1o),
            Err(HackrfError::NoAntennaPower)
        );
        assert_eq!(
            check(52, 2, 0x0113, Board::OneR9),
            Err(HackrfError::ProOnly { request: 52 })
        );
        assert!(check(52, 2, 0x0113, Board::Pro).is_ok());
    }

    #[test]
    fn requests_that_write_firmware_flash_logic_or_registers_are_never_sent() {
        for r in [
            0, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13, 22, 25, 27, 28, 31, 33, 34, 35, 36, 38, 39, 40,
            49, 50, 54, 57, 58, 59, 60,
        ] {
            assert!(
                matches!(min_api(r, 0), Err(HackrfError::Forbidden { .. })),
                "{r}"
            );
        }
        for value in [3, 4, 6] {
            assert!(matches!(
                min_api(request::MODE, value),
                Err(HackrfError::Forbidden { .. })
            ));
        }
    }

    #[test]
    fn gains_split_and_round_to_their_steps() {
        assert_eq!(split_gain(32.0), (16, 16));
        assert_eq!(split_gain(36.0), (16, 20));
        assert_eq!(split_gain(0.0), (0, 0));
        assert_eq!(split_gain(102.0), (40, 62));
        assert_eq!(split_gain(200.0), (40, 62));
        assert_eq!(split_gain(5.0), (0, 4));
        for t in 0..=102 {
            let (l, v) = split_gain(f64::from(t));
            assert!(l % 8 == 0 && v % 2 == 0 && l <= 40 && v <= 62, "{t}");
            assert!(l + v <= t && t - (l + v) <= 1, "{t}: {l} {v}");
        }
        assert_eq!(lna_step(20), 16);
        assert_eq!(vga_step(21), 20);
        assert_eq!(lna_step(99), 40);
    }

    #[test]
    fn replies_parse() {
        let mut d = [0u8; 18];
        d[0] = 18;
        d[1] = 1;
        d[12] = 0x10;
        d[13] = 0x01;
        assert_eq!(api_from_descriptor(&d), Some(0x0110));
        assert_eq!(api_text(0x0110), "1.10");
        assert_eq!(api_text(0x0102), "1.02");
        let mut b = [0u8; 24];
        b[..4].copy_from_slice(&0xA000_CB3Cu32.to_le_bytes());
        b[8..12].copy_from_slice(&0x1234u32.to_le_bytes());
        b[20..24].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        let ps = PartSerial::parse(&b).unwrap();
        assert_eq!(ps.serial_text(), "000012340000000000000000deadbeef");
        assert_eq!(ps.part_text(), "a000cb3c 00000000");
        assert_eq!(revision_text(0x84), "r9, built by Great Scott Gadgets");
        assert_eq!(revision_text(9), "Pro 1.0");
        assert_eq!(
            platforms_text(0b11010),
            "HackRF One before r9, HackRF One r9, HackRF Pro"
        );
        let mut s = [0u8; 40];
        s[16] = 3;
        s[20] = 0x00;
        s[21] = 0x40;
        assert_eq!(
            StreamState::parse(&s),
            Some(StreamState {
                active: 0,
                sampled: 0,
                shortfalls: 3,
                longest: 0x4000,
                error: 0
            })
        );
    }

    #[test]
    fn transmit_sizes_and_bytes() {
        assert_eq!(tx_write_size(8e6), 10 * BLOCK);
        assert_eq!(tx_write_size(2e6), 3 * BLOCK);
        assert_eq!(tx_write_size(20e6), 16 * BLOCK);
        assert_eq!(tx_byte(1.0), 127);
        assert_eq!(tx_byte(-1.0), 0x80);
        assert_eq!(tx_byte(0.5), 64);
        assert_eq!(tx_byte(f32::NAN), 0);
    }

    #[test]
    fn boards_from_their_ids() {
        assert_eq!(Board::from_id(0), Err(HackrfError::Prototype));
        assert_eq!(Board::from_id(4), Ok(Board::OneR9));
        assert_eq!(Board::from_id(0xFE), Ok(Board::Unknown(0xFE)));
        assert!(!Board::Pro.dc_spike() && Board::OneR9.dc_spike());
        assert!(!Board::Jawbreaker.antenna_power() && Board::Pro.antenna_power());
        assert_eq!(Board::Jawbreaker.min_rate_hz(), 8e6);
    }
}
