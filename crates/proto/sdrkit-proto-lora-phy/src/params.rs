//! The parameters of a LoRa link, the sync-word symbols, and the time on air.
//!
//! Standard: Semtech SX1261/2 data sheet clause 6.1.4

use crate::error::{LoraError, Param, Result};
use serde::{Deserialize, Serialize};

/// The LoRa bandwidths of the Semtech radios, Hz (SX1276 data sheet, register `RegModemConfig1`). Each one is also the chip rate.
/// The receiver also accepts other bandwidths.
pub const BANDWIDTHS: [f64; 10] = [
    7_812.5,
    10_416.666_666_666_666,
    15_625.0,
    20_833.333_333_333_332,
    31_250.0,
    41_666.666_666_666_664,
    62_500.0,
    125_000.0,
    250_000.0,
    500_000.0,
];

/// Low data rate optimisation (LDRO). With LDRO on, each data symbol carries two bits less, and the receiver can tolerate more
/// clock drift.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ldro {
    /// On when the symbol time `2^SF / BW` is 16 ms or more (the rule of the common radio libraries).
    #[default]
    Auto,
    /// Always on.
    On,
    /// Always off.
    Off,
}

/// The coding layout of a frame: what the encoder and the decoder must agree on besides the header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Spreading factor, 5 to 12. A symbol has `2^SF` chips.
    pub sf: u8,
    /// Low data rate optimisation is in use.
    pub ldro: bool,
    /// The frame has no header. The length, the coding rate and the CRC flag are agreed in advance.
    pub implicit: bool,
}

impl Layout {
    /// A layout. If `sf` is not 5 to 12, this returns an error.
    pub fn new(sf: u8, ldro: bool, implicit: bool) -> Result<Self> {
        if !(5..=12).contains(&sf) {
            return Err(LoraError::BadParameter(Param::SpreadingFactor));
        }
        Ok(Layout { sf, ldro, implicit })
    }
}

/// One LoRa setting: all that a transmitter and a receiver must agree on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoraParams {
    /// Spreading factor, 5 to 12.
    pub sf: u8,
    /// Bandwidth, Hz. This is also the chip rate.
    pub bw: f64,
    /// Coding rate 5 to 8, for 4/5 to 4/8. An explicit header carries its own coding rate.
    pub cr: u8,
    /// Preamble length in up-chirps, 6 to 65535.
    pub preamble: u16,
    /// The sync word (0x12 for private networks, 0x34 for LoRaWAN).
    pub sync: u8,
    /// Low data rate optimisation.
    pub ldro: Ldro,
    /// Implicit header: no header on the air.
    pub implicit: bool,
    /// The payload CRC is on. An explicit header carries its own flag.
    pub crc: bool,
    /// The payload length for an implicit header.
    pub implicit_len: u8,
    /// Inverted IQ, as LoRaWAN downlinks use.
    pub invert_iq: bool,
}

impl Default for LoraParams {
    fn default() -> Self {
        LoraParams {
            sf: 7,
            bw: 125e3,
            cr: 5,
            preamble: 8,
            sync: 0x12,
            ldro: Ldro::Auto,
            implicit: false,
            crc: true,
            implicit_len: 16,
            invert_iq: false,
        }
    }
}

impl LoraParams {
    /// A setting with these values, an explicit header, the CRC on and LDRO automatic.
    pub fn new(sf: u8, bw: f64, cr: u8, preamble: u16, sync: u8) -> Self {
        LoraParams {
            sf,
            bw,
            cr,
            preamble,
            sync,
            ..Default::default()
        }
    }

    /// Makes sure that the setting is valid: SF 5 to 12, CR 5 to 8, a finite positive bandwidth, a preamble of 6 symbols or
    /// more, and an implicit length of 1 or more. If it is not valid, this returns an error that names the parameter.
    pub fn check(&self) -> Result<()> {
        if !(5..=12).contains(&self.sf) {
            return Err(LoraError::BadParameter(Param::SpreadingFactor));
        }
        if !(5..=8).contains(&self.cr) {
            return Err(LoraError::BadParameter(Param::CodingRate));
        }
        if !(self.bw.is_finite() && self.bw > 0.0) {
            return Err(LoraError::BadParameter(Param::Bandwidth));
        }
        if self.preamble < 6 {
            return Err(LoraError::BadParameter(Param::Preamble));
        }
        if self.implicit && self.implicit_len == 0 {
            return Err(LoraError::BadParameter(Param::ImplicitLength));
        }
        Ok(())
    }

    /// True if an SX1276/77/78/79 radio can send this setting. That radio has no SF5. At SF6 it needs an implicit header
    /// (SX1276 data sheet clause 4.1.1.2). The SX126x radios send SF5 and SF6 with either header.
    pub fn sx127x_can_send(&self) -> bool {
        self.sf >= 7 || (self.sf == 6 && self.implicit)
    }

    /// Chips per symbol, `2^SF`.
    pub fn n(&self) -> usize {
        1usize << self.sf.min(12)
    }

    /// The symbol time, seconds.
    pub fn symbol_s(&self) -> f64 {
        self.n() as f64 / self.bw
    }

    /// True if LDRO is in use.
    pub fn ldro_on(&self) -> bool {
        match self.ldro {
            Ldro::On => true,
            Ldro::Off => false,
            Ldro::Auto => auto_ldro(self.sf, self.bw),
        }
    }

    /// The coding layout of this setting.
    pub fn layout(&self) -> Layout {
        Layout {
            sf: self.sf,
            ldro: self.ldro_on(),
            implicit: self.implicit,
        }
    }

    /// The two sync symbols.
    pub fn sync_symbols(&self) -> [u16; 2] {
        sync_symbols(self.sync, self.sf)
    }

    /// The data symbols of a frame of `len` payload bytes, with coding rate `cr` and the CRC flag `crc`.
    pub fn data_symbols(&self, len: usize, cr: u8, crc: bool) -> usize {
        data_symbols(self.sf, cr, len, crc, self.implicit, self.ldro_on())
    }

    /// The symbols from the start of the preamble to the first data symbol. These are the preamble, 2 sync symbols, 2.25 symbols
    /// of start frame delimiter (SFD, the down-chirps that mark the frame start), and 2 more symbols at SF5 and SF6.
    pub fn header_symbols(&self) -> f64 {
        self.preamble as f64 + 4.25 + if self.sf < 7 { 2.0 } else { 0.0 }
    }

    /// The time on air of a frame of `len` bytes with the coding rate and the CRC flag of this setting, seconds.
    pub fn airtime(&self, len: usize) -> f64 {
        (self.header_symbols() + self.data_symbols(len, self.cr, self.crc) as f64) * self.symbol_s()
    }

    /// A short text such as "SF11 250 kHz 4/5".
    pub fn describe(&self) -> String {
        format!("SF{} {} kHz 4/{}", self.sf, fmt_khz(self.bw), self.cr)
    }
}

/// A bandwidth in kHz as people write it: "125", "62.5", "7.813".
pub fn fmt_khz(bw: f64) -> String {
    let k = bw / 1e3;
    if (k - k.round()).abs() < 1e-9 {
        format!("{}", k.round() as i64)
    } else {
        let s = format!("{k:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// The automatic LDRO rule: on when `2^SF / BW` is 16 ms or more.
pub fn auto_ldro(sf: u8, bw: f64) -> bool {
    (1u64 << sf.min(12)) as f64 / bw >= 0.016 - 1e-12
}

/// The two sync symbols of the sync word `sync` at `sf`. Each nibble `v` maps to the symbol `8 v`. If `8 v` is 64 or more, the
/// symbol is `8 v - 128`. The result is taken modulo `2^SF`. This is the mapping published by J. Tapparel et al., "An
/// open-source LoRa physical layer prototype on GNU Radio", 2020. Sync word 0x34 maps to the symbols 24 and 32.
pub fn sync_symbols(sync: u8, sf: u8) -> [u16; 2] {
    let n = 1i32 << sf.min(12);
    let f = |nib: u8| {
        let mut v = 8 * nib as i32;
        if v >= 64 {
            v -= 128;
        }
        v.rem_euclid(n) as u16
    };
    [f(sync >> 4), f(sync & 0xF)]
}

/// The sync symbols that a receiver accepts for `sync` at `sf`: those of [`sync_symbols`], and for a nibble of 8 or more
/// also the plain `8 v`, because radios differ on that nibble (sync word 0x2B is sent with a second symbol of either
/// `-40` or `88`, both heard in the field). Each pair is one form; a receiver takes the form whose first symbol it hears.
pub fn sync_symbol_forms(sync: u8, sf: u8) -> [[u16; 2]; 2] {
    let n = 1i32 << sf.min(12);
    let plain = |nib: u8| (8 * nib as i32).rem_euclid(n) as u16;
    [
        sync_symbols(sync, sf),
        [plain(sync >> 4), plain(sync & 0xF)],
    ]
}

/// The number of data symbols (SX1261/2 data sheet clause 6.1.4):
/// `8 + max(ceil((8 L - 4 SF + 28 + 16 CRC - 20 IH) / (4 (SF - 2 DE))), 0) (CR + 4)`.
/// At SF5 and SF6 the formula uses 20 in place of 28. `cr` is 5 to 8 here (the formula's `CR + 4`). If `sf` is out of range,
/// it counts as 12. If `ldro` is on at SF5, the divisor is at least 4.
pub fn data_symbols(sf: u8, cr: u8, len: usize, crc: bool, implicit: bool, ldro: bool) -> usize {
    let sf_i = sf.clamp(5, 12) as i64;
    let k = if sf_i < 7 { 20 } else { 28 };
    let num =
        8 * len as i64 - 4 * sf_i + k + if crc { 16 } else { 0 } - if implicit { 20 } else { 0 };
    let den = (4 * (sf_i - if ldro { 2 } else { 0 })).max(4);
    let blocks = if num <= 0 { 0 } else { (num + den - 1) / den };
    8 + blocks as usize * cr as usize
}
