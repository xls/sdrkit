//! The choice between the RDS and RBDS meanings of the programme type and the PI, and the RBDS call letters coded in a PI.
//!
//! Standard: IEC 62106 (RDS), NRSC-4-B Annex D (RBDS call letters).

use sdrkit_util_ids::pty::{self, PtyTable};

/// The name of programme type `code` (5 bits, so taken modulo 32) as a 16-character display shows it, in the RDS or the RBDS
/// table of `sdrkit-util-ids`. A code the table leaves unassigned returns an empty name.
pub fn pty_name(code: u8, rbds: bool) -> &'static str {
    let table = if rbds { PtyTable::Rbds } else { PtyTable::Rds };
    pty::pty(code % 32, table).map_or("", |p| p.short16.unwrap_or(p.name))
}

/// The RBDS call letters coded in a PI: K and three letters for 0x1000 to 0x54A7, W and three letters for 0x54A8 to 0x994F.
/// Any other PI returns `None`. Standard: NRSC-4-B Annex D.
pub fn call_letters(pi: u16) -> Option<String> {
    let (first, base) = match pi {
        0x1000..=0x54A7 => ('K', 0x1000u16),
        0x54A8..=0x994F => ('W', 0x54A8u16),
        _ => return None,
    };
    let n = pi - base;
    let letter = |v: u16| (b'A' + v as u8) as char;
    Some(
        [first, letter(n / 676), letter(n / 26 % 26), letter(n % 26)]
            .iter()
            .collect(),
    )
}

/// The PI of four-letter call letters that start with K or W (either case), or 0 for any other text.
pub fn pi_from_call_letters(call: &str) -> u16 {
    let c: Vec<u8> = call.bytes().map(|b| b.to_ascii_uppercase()).collect();
    if c.len() != 4 || !c[1..].iter().all(u8::is_ascii_uppercase) {
        return 0;
    }
    let base = match c[0] {
        b'K' => 0x1000u16,
        b'W' => 0x54A8u16,
        _ => return 0,
    };
    let v = |b: u8| (b - b'A') as u16;
    base + 676 * v(c[1]) + 26 * v(c[2]) + v(c[3])
}

sdrkit_core::choice_enum! {
    /// The `rds_variant` setting: which programme type table and call-letter rule apply.
    #[derive(Default)]
    pub enum Variant {
        /// Decided from the evidence ([`Evidence::rbds`]).
        #[default]
        Auto = ("auto", "Auto"),
        /// RDS (IEC 62106).
        Rds = ("rds", "RDS"),
        /// RBDS (NRSC-4).
        Rbds = ("rbds", "RBDS"),
    }
}

/// What the signal shows, for [`Variant::Auto`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Evidence {
    /// The tuned frequency, Hz.
    pub tuned_hz: f64,
    /// The station's extended country code (group 1A, variant 0).
    pub ecc: Option<u8>,
}

impl Evidence {
    /// Whether RBDS meanings apply under [`Variant::Auto`]. An extended country code outside the Americas rules RBDS
    /// out. Else RBDS applies on the RBDS raster (an odd tenth of a MHz from 87.9 to 107.9 MHz).
    pub fn rbds(&self) -> bool {
        if self.ecc.is_some_and(|e| !ecc_in_americas(e)) {
            return false;
        }
        on_us_raster(self.tuned_hz)
    }
}

impl Variant {
    /// Whether RBDS meanings (the RBDS programme types and the call letters) apply. A forced choice wins over the evidence.
    pub fn rbds_applies(self, e: &Evidence) -> bool {
        match self {
            Variant::Rds => false,
            Variant::Rbds => true,
            Variant::Auto => e.rbds(),
        }
    }
}

/// An extended country code of the Americas, 0xA0 to 0xA6 (IEC 62106 Annex D).
pub fn ecc_in_americas(ecc: u8) -> bool {
    (0xA0..=0xA6).contains(&ecc)
}

/// Whether `hz` is an odd tenth of a MHz between 87.9 and 107.9 MHz (101.1 MHz yes; 100.0, 98.2 and 98.25 MHz no).
pub fn on_us_raster(hz: f64) -> bool {
    let tenths = (hz / 100e3).round();
    if (hz - tenths * 100e3).abs() > 1e3 {
        return false;
    }
    let t = tenths as i64;
    (879..=1079).contains(&t) && t % 2 == 1
}
