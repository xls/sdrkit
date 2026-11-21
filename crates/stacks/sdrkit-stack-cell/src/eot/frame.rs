//! The end-of-train frame: bit sync, frame sync, 45 data bits and 18 check bits of a (63,45) cyclic code, and the fields of
//! the data bits. No public document confirms the frame, the code, the tones or the field positions.

use sdrkit_algo_fec::small::SmallCode;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The bit rate, bit/s.
pub const BIT_RATE: f64 = 1200.0;
/// The audio tone of a one, Hz.
pub const ONE_HZ: f64 = 1200.0;
/// The audio tone of a zero, Hz.
pub const ZERO_HZ: f64 = 1800.0;
/// The bits of bit sync that a transmitter sends, ending in `...1010`.
pub const BIT_SYNC_BITS: usize = 69;
/// The frame sync, 11 bits, first sent most significant.
pub const FRAME_SYNC: u32 = 0b111_0001_0010;
/// The frame sync bits.
pub const FRAME_SYNC_BITS: usize = 11;
/// What the search compares: the last 6 bits of the bit sync and the frame sync, 17 bits.
pub const SEARCH_WORD: u32 = (0b10_1010 << FRAME_SYNC_BITS) | FRAME_SYNC;
/// The bits of [`SEARCH_WORD`].
pub const SEARCH_BITS: usize = 17;
/// The bit sync sent before the search word that the receiver checks too: 16 bits, `1010...10`, first sent most significant.
pub const PREAMBLE_WORD: u32 = 0xAAAA;
/// The bits of [`PREAMBLE_WORD`].
pub const PREAMBLE_BITS: usize = 16;
/// The most wrong bits of [`PREAMBLE_WORD`]: noise passes 137 of 65 536 patterns, a frame at 6 dB C/N nearly always.
pub const PREAMBLE_MAX_ERRORS: u32 = 2;
/// The data bits d0 to d44.
pub const DATA_BITS: usize = 45;
/// The check bits c0 to c17.
pub const CHECK_BITS: usize = 18;
/// The data and check bits that follow the frame sync.
pub const BODY_BITS: usize = DATA_BITS + CHECK_BITS;
/// The most bit errors a frame may have repaired.
pub const MAX_REPAIRED: u8 = 2;
/// The most wrong bits of the sync and repaired bits of the body together: a sync error and two repaired bits are refused.
pub const MAX_ERRORS: u8 = 2;

/// The (63,45) code with its key, made once.
fn code() -> &'static SmallCode {
    static CODE: OnceLock<SmallCode> = OnceLock::new();
    CODE.get_or_init(SmallCode::eot_63_45)
}

/// The data bits d0 to d44 as the code's information word (d44 first sent, most significant).
fn info_of(data: &[u8; DATA_BITS]) -> u128 {
    data.iter()
        .rev()
        .fold(0u128, |w, &b| (w << 1) | u128::from(b & 1))
}

/// The check bits c0 to c17 of the data bits d0 to d44.
pub fn check_bits(data: &[u8; DATA_BITS]) -> [u8; CHECK_BITS] {
    let word = code().encode(info_of(data));
    std::array::from_fn(|i| ((word >> (CHECK_BITS - 1 - i)) & 1) as u8)
}

/// The body of a frame: d0 to d44, then c0 to c17.
pub fn body(data: &[u8; DATA_BITS]) -> [u8; BODY_BITS] {
    let c = check_bits(data);
    std::array::from_fn(|i| {
        if i < DATA_BITS {
            data[i]
        } else {
            c[i - DATA_BITS]
        }
    })
}

/// Checks a received body behind a sync with `sync_errors` wrong bits and repairs up to [`MAX_REPAIRED`] bits, and up to
/// [`MAX_ERRORS`] with the sync errors: the data bits and the number of bits repaired, or `None` for a frame that is refused.
pub fn check(body: &[u8; BODY_BITS], sync_errors: u8) -> Option<([u8; DATA_BITS], u8)> {
    let mut data = [0u8; DATA_BITS];
    data.copy_from_slice(&body[..DATA_BITS]);
    let checks = body[DATA_BITS..]
        .iter()
        .fold(0u128, |w, &b| (w << 1) | u128::from(b & 1));
    let word = (info_of(&data) << CHECK_BITS) | checks;
    let (fixed, n) = code().decode(word)?;
    if n > MAX_REPAIRED || n + sync_errors > MAX_ERRORS {
        return None;
    }
    let info = code().info(fixed);
    for (i, d) in data.iter_mut().enumerate() {
        *d = ((info >> i) & 1) as u8;
    }
    Some((data, n))
}

/// The link direction of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// End of train to head of train: telemetry.
    Eot,
    /// Head of train to end of train: commands.
    Hot,
}

impl Direction {
    /// The name for people.
    pub fn label(self) -> &'static str {
        match self {
            Direction::Eot => "End of train",
            Direction::Hot => "Head of train",
        }
    }
}

/// The fields of the data bits of a frame.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EotData {
    /// Chaining bits d0, d1.
    pub chaining: u8,
    /// Battery condition d2, d3: 0 not monitored, 1 very low, 2 low, 3 OK.
    pub battery: u8,
    /// Message type d4 to d6 (d4 most significant).
    pub message_type: u8,
    /// Unit address d7 to d23.
    pub unit: u32,
    /// Brake pipe pressure d24 to d30, psi.
    pub pressure_psi: u8,
    /// Battery charge d31 to d37, 0 to 127.
    pub charge: u8,
    /// Spare d38.
    pub spare: bool,
    /// Valve circuit d39: open.
    pub valve_open: bool,
    /// Confirmation d40 (message type 7: armed).
    pub confirmation: bool,
    /// Turbine status d41.
    pub turbine: bool,
    /// Motion detection d42: moving.
    pub moving: bool,
    /// Marker light battery d43.
    pub marker_battery: bool,
    /// Marker light status d44: on.
    pub marker_light: bool,
}

/// `bits` least significant first from `d[from..]`.
fn lsb_first(d: &[u8], from: usize, bits: usize) -> u32 {
    (0..bits).fold(0, |v, i| v | (u32::from(d[from + i] & 1) << i))
}

fn put_lsb_first(d: &mut [u8], from: usize, bits: usize, v: u32) {
    for i in 0..bits {
        d[from + i] = ((v >> i) & 1) as u8;
    }
}

impl EotData {
    /// The fields of data bits d0 to d44.
    pub fn from_bits(d: &[u8; DATA_BITS]) -> EotData {
        EotData {
            chaining: lsb_first(d, 0, 2) as u8,
            battery: lsb_first(d, 2, 2) as u8,
            message_type: (d[4] << 2) | (d[5] << 1) | d[6],
            unit: lsb_first(d, 7, 17),
            pressure_psi: lsb_first(d, 24, 7) as u8,
            charge: lsb_first(d, 31, 7) as u8,
            spare: d[38] == 1,
            valve_open: d[39] == 1,
            confirmation: d[40] == 1,
            turbine: d[41] == 1,
            moving: d[42] == 1,
            marker_battery: d[43] == 1,
            marker_light: d[44] == 1,
        }
    }

    /// Data bits d0 to d44.
    pub fn to_bits(&self) -> [u8; DATA_BITS] {
        let mut d = [0u8; DATA_BITS];
        put_lsb_first(&mut d, 0, 2, u32::from(self.chaining));
        put_lsb_first(&mut d, 2, 2, u32::from(self.battery));
        d[4] = (self.message_type >> 2) & 1;
        d[5] = (self.message_type >> 1) & 1;
        d[6] = self.message_type & 1;
        put_lsb_first(&mut d, 7, 17, self.unit);
        put_lsb_first(&mut d, 24, 7, u32::from(self.pressure_psi));
        put_lsb_first(&mut d, 31, 7, u32::from(self.charge));
        for (i, b) in [
            self.spare,
            self.valve_open,
            self.confirmation,
            self.turbine,
            self.moving,
            self.marker_battery,
            self.marker_light,
        ]
        .into_iter()
        .enumerate()
        {
            d[38 + i] = u8::from(b);
        }
        d
    }

    /// The battery charge, percent (integer: value × 100 / 127).
    pub fn charge_percent(&self) -> u32 {
        u32::from(self.charge) * 100 / 127
    }

    /// The unit as shown: five digits, zero padded.
    pub fn unit_text(&self) -> String {
        format!("{:05}", self.unit)
    }

    /// What the frame says, for people. A head-of-train frame shows only its message type.
    pub fn text(&self, direction: Direction) -> String {
        if direction == Direction::Hot {
            return format!("command, message type {}", self.message_type);
        }
        let battery = match self.battery {
            0 => "not monitored",
            1 => "very low",
            2 => "low",
            _ => "OK",
        };
        let kind = match (self.message_type, self.confirmation) {
            (0, _) => "normal".to_string(),
            (7, true) => "armed".to_string(),
            (7, false) => "arming".to_string(),
            (t, _) => format!("message type {t}"),
        };
        let mut s = format!(
            "brake pipe {} psi, battery {battery} {}%, {}, {kind}, valve {}, marker light {}",
            self.pressure_psi,
            self.charge_percent(),
            if self.moving { "moving" } else { "stopped" },
            if self.valve_open { "open" } else { "closed" },
            if self.marker_light { "on" } else { "off" },
        );
        if self.turbine {
            s.push_str(", turbine bit set");
        }
        if self.marker_battery {
            s.push_str(", marker battery bit set");
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> EotData {
        EotData {
            battery: 3,
            unit: 12345,
            pressure_psi: 90,
            charge: 100,
            valve_open: true,
            moving: true,
            marker_light: true,
            ..EotData::default()
        }
    }

    fn bits(s: &str) -> Vec<u8> {
        s.bytes().map(|b| b - b'0').collect()
    }

    #[test]
    fn the_worked_example() {
        let d = example().to_bits();
        assert_eq!(
            d.to_vec(),
            bits("001100010011100000011000010110100100110100101")
        );
        assert_eq!(check_bits(&d).to_vec(), bits("100001000111110101"));
        assert_eq!(EotData::from_bits(&d), example());
        assert_eq!(
            example().text(Direction::Eot),
            "brake pipe 90 psi, battery OK 78%, moving, normal, valve open, marker light on"
        );
        assert_eq!(example().unit_text(), "12345");
    }

    #[test]
    fn two_errors_repaired_three_refused_or_wrong() {
        let d = example().to_bits();
        let mut b = body(&d);
        b[3] ^= 1;
        b[50] ^= 1;
        assert_eq!(check(&b, 0), Some((d, 2)));
        assert_eq!(check(&b, 1), None);
        b[20] ^= 1;
        // Three errors are beyond the repair limit: the code refuses the frame or repairs it to other data.
        match check(&b, 0) {
            None => println!("three errors: refused"),
            Some((other, n)) => {
                assert_ne!(other, d);
                println!("three errors: repaired to other data ({n} bits)");
            }
        }
    }
}
