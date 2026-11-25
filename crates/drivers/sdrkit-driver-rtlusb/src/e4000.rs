//! The Elonics E4000 tuner (specification section 4.2): a zero-IF tuner from about 50 to 2200 MHz, with a gap near
//! 1100 to 1250 MHz that varies per unit. Its registers read back, so a masked write reads the register first and is
//! skipped when the masked bits already have the value.

use crate::chip::Chip;
use crate::r82xx::corrected;
use crate::RtlError;

/// The 8-bit I2C address.
pub const ADDR: u8 = 0xC8;

/// The lowest and the highest LO that the driver tunes.
pub const MIN_HZ: f64 = 50e6;
/// The highest LO that the driver tunes.
pub const MAX_HZ: f64 = 2200e6;

/// The gains that the tuner offers, in tenth dB.
pub const GAINS: [i32; 14] = [
    -10, 15, 40, 65, 90, 115, 140, 165, 190, 215, 240, 290, 340, 420,
];

/// The LNA gains in tenth dB and their codes in R14.
const LNA: [(i32, u8); 13] = [
    (-50, 0),
    (-25, 1),
    (0, 4),
    (25, 5),
    (50, 6),
    (75, 7),
    (100, 8),
    (125, 9),
    (150, 10),
    (175, 11),
    (200, 12),
    (250, 13),
    (300, 14),
];

/// The output divider by intended LO: below this LO in Hz, R and the R0D value.
const DIVIDERS: [(u64, u32, u8); 10] = [
    (72_400_000, 48, 0x0F),
    (81_200_000, 40, 0x0E),
    (108_300_000, 32, 0x0D),
    (162_500_000, 24, 0x0C),
    (216_600_000, 16, 0x0B),
    (325_000_000, 12, 0x0A),
    (350_000_000, 8, 0x09),
    (432_000_000, 8, 0x03),
    (667_000_000, 6, 0x02),
    (1_200_000_000, 4, 0x01),
];

/// The centres of the RF filter in MHz in the UHF band and in L band.
const UHF_FILTER: [u32; 16] = [
    360, 380, 405, 425, 450, 475, 505, 540, 575, 615, 670, 720, 760, 840, 890, 970,
];
const L_FILTER: [u32; 16] = [
    1300, 1320, 1360, 1410, 1445, 1460, 1490, 1530, 1560, 1590, 1640, 1660, 1680, 1700, 1720, 1750,
];

/// The widths of the mixer filter and of the IF RC filter in kHz, index 0 to 15, and of the channel filter, index 0
/// to 31.
const MIXER_FILTER: [u32; 16] = [
    27_000, 27_000, 27_000, 27_000, 27_000, 27_000, 27_000, 27_000, 4_600, 4_200, 3_800, 3_400,
    3_300, 2_700, 2_300, 1_900,
];
const RC_FILTER: [u32; 16] = [
    21_400, 21_000, 17_600, 14_700, 12_400, 10_600, 9_000, 7_700, 6_400, 5_300, 4_400, 3_400,
    2_600, 1_800, 1_200, 1_000,
];
const CHANNEL_FILTER: [u32; 32] = [
    5_500, 5_300, 5_000, 4_800, 4_600, 4_400, 4_300, 4_100, 3_900, 3_800, 3_700, 3_600, 3_400,
    3_300, 3_200, 3_100, 3_000, 2_950, 2_900, 2_800, 2_750, 2_700, 2_600, 2_550, 2_500, 2_450,
    2_400, 2_300, 2_280, 2_240, 2_200, 2_150,
];

/// The values that the synthesiser writes for an LO.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Synth {
    /// The R0D value: output divider and three-phase mixing.
    pub r0d: u8,
    /// The output divider R.
    pub r: u32,
    /// The integer divider Z.
    pub z: u8,
    /// The fraction X in 1/65536.
    pub x: u16,
    /// The LO that the values give, in Hz.
    pub actual_hz: f64,
}

/// The synthesiser values for the LO `lo_hz` with the reference `osc_hz`.
///
/// # Errors
///
/// [`RtlError::NoPllSetting`] if Z is above 255.
pub fn synth_for(lo_hz: u64, osc_hz: u32) -> Result<Synth, RtlError> {
    let (_, r, r0d) = DIVIDERS
        .iter()
        .copied()
        .find(|d| lo_hz < d.0)
        .unwrap_or((0, 2, 0x00));
    let osc = u64::from(osc_hz);
    let f = lo_hz * u64::from(r);
    let z = f / osc;
    if z > 255 {
        return Err(RtlError::NoPllSetting { hz: lo_hz });
    }
    let x = ((f - z * osc) * 65536) / osc;
    let actual = (osc as f64 * z as f64 + osc as f64 * x as f64 / 65536.0) / f64::from(r);
    Ok(Synth {
        r0d,
        r,
        z: z as u8,
        x: x as u16,
        actual_hz: actual,
    })
}

/// The index of the entry of `table` (kHz) nearest `khz`; on a tie the first.
fn nearest(table: &[u32], khz: u32) -> usize {
    let mut best = 0;
    for (i, &w) in table.iter().enumerate() {
        if w.abs_diff(khz) < table[best].abs_diff(khz) {
            best = i;
        }
    }
    best
}

/// The mixer gain in dB and the LNA code for the gain `g` in tenth dB, which must be one of [`GAINS`].
pub fn gain_codes(g: i32) -> Option<(bool, u8)> {
    let high = g > 340;
    let lna = (g - if high { 120 } else { 40 }).min(300);
    LNA.iter().find(|e| e.0 == lna).map(|e| (high, e.1))
}

/// An E4000 tuner.
#[derive(Clone, Debug)]
pub struct E4000 {
    xtal: u32,
    osc_hz: u32,
    initialised: bool,
    locked: bool,
}

impl E4000 {
    /// A tuner with the reference `xtal` Hz.
    pub fn new(xtal: u32) -> E4000 {
        E4000 {
            xtal,
            osc_hz: xtal,
            initialised: false,
            locked: false,
        }
    }

    /// Sets the frequency correction of the reference: `ppm` parts per million.
    pub fn set_ppm(&mut self, ppm: f64) {
        self.osc_hz = corrected(self.xtal, ppm);
    }

    /// The last PLL lock state.
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// The tuner was initialised and is not in standby.
    pub fn initialised(&self) -> bool {
        self.initialised
    }

    fn write(c: &mut Chip, reg: u8, v: u8) -> Result<(), RtlError> {
        c.i2c_write(ADDR, &[reg, v])
    }

    fn read(c: &mut Chip, reg: u8) -> Result<u8, RtlError> {
        c.i2c_read_reg(ADDR, reg)
    }

    /// Reads `reg` and writes it with the bits of `mask` from `v`, unless they already have that value.
    fn mask(c: &mut Chip, reg: u8, v: u8, mask: u8) -> Result<(), RtlError> {
        let cur = Self::read(c, reg)?;
        if cur & mask == v & mask {
            return Ok(());
        }
        Self::write(c, reg, (cur & !mask) | (v & mask))
    }

    /// Initialises the tuner in automatic gain mode with the narrowest filters.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn init(&mut self, c: &mut Chip) -> Result<(), RtlError> {
        // The first transaction is not acknowledged.
        if let Err(e) = Self::read(c, 0x00) {
            if e.unplugged() {
                return Err(e);
            }
        }
        Self::write(c, 0x00, 0x07)?;
        Self::write(c, 0x05, 0x00)?;
        Self::write(c, 0x06, 0x00)?;
        Self::write(c, 0x7A, 0x96)?;
        for (reg, v) in [
            (0x7E, 0x01),
            (0x7F, 0xFE),
            (0x82, 0x00),
            (0x86, 0x50),
            (0x87, 0x20),
            (0x88, 0x01),
            (0x9F, 0x7F),
            (0xA0, 0x07),
        ] {
            Self::write(c, reg, v)?;
        }
        Self::write(c, 0x1D, 0x10)?;
        Self::write(c, 0x1E, 0x04)?;
        Self::write(c, 0x1F, 0x1A)?;
        Self::mask(c, 0x1A, 0x00, 0x0F)?;
        Self::mask(c, 0x20, 0x00, 0x01)?;
        self.set_auto_gain(c)?;
        // IF gains 6, 0, 0, 0, 9, 9 dB.
        Self::mask(c, 0x16, 0x01, 0x01)?;
        Self::mask(c, 0x16, 0x00, 0x06)?;
        Self::mask(c, 0x16, 0x00, 0x18)?;
        Self::mask(c, 0x16, 0x00, 0x60)?;
        Self::mask(c, 0x17, 0x02, 0x07)?;
        Self::mask(c, 0x17, 0x02 << 3, 0x38)?;
        // The narrowest filters, the channel filter on.
        Self::mask(c, 0x11, 15 << 4, 0xF0)?;
        Self::mask(c, 0x11, 15, 0x0F)?;
        Self::mask(c, 0x12, 31, 0x1F)?;
        Self::mask(c, 0x12, 0x00, 0x20)?;
        // Time-variant DC correction and the look-up tables off.
        Self::mask(c, 0x2D, 0x00, 0x03)?;
        Self::mask(c, 0x70, 0x00, 0x03)?;
        Self::mask(c, 0x71, 0x00, 0x03)?;
        self.initialised = true;
        Ok(())
    }

    /// Tunes the LO to `lo_hz` (the LO is the RF frequency) and checks the lock. `edge` is called with `true` just
    /// before the LO changes and with `false` after the lock check.
    ///
    /// # Errors
    ///
    /// [`RtlError::NoPllSetting`] or [`RtlError::PllNotLocked`], else the error of a transfer.
    pub fn set_frequency(
        &mut self,
        c: &mut Chip,
        lo_hz: u64,
        edge: &mut dyn FnMut(bool),
    ) -> Result<(), RtlError> {
        let s = synth_for(lo_hz, self.osc_hz)?;
        edge(true);
        let r = self.program(c, &s);
        edge(false);
        r?;
        if self.locked {
            Ok(())
        } else {
            Err(RtlError::PllNotLocked { hz: lo_hz })
        }
    }

    /// Writes the synthesiser, the band and the RF filter for `s`, and reads the lock.
    fn program(&mut self, c: &mut Chip, s: &Synth) -> Result<(), RtlError> {
        Self::write(c, 0x0D, s.r0d)?;
        Self::write(c, 0x09, s.z)?;
        Self::write(c, 0x0A, (s.x & 0xFF) as u8)?;
        Self::write(c, 0x0B, (s.x >> 8) as u8)?;
        let mhz = s.actual_hz / 1e6;
        let (band, bias, filter) = if mhz < 140.0 {
            (0u8, 0x03, None)
        } else if mhz < 350.0 {
            (1, 0x03, None)
        } else if mhz < 1135.0 {
            (2, 0x03, Some(&UHF_FILTER))
        } else {
            (3, 0x00, Some(&L_FILTER))
        };
        Self::write(c, 0x78, bias)?;
        Self::mask(c, 0x07, 0x00, 0x06)?;
        Self::mask(c, 0x07, band << 1, 0x06)?;
        let idx = filter.map_or(0, |t| nearest(t, (s.actual_hz / 1e6).round() as u32));
        Self::mask(c, 0x10, idx as u8, 0x0F)?;
        self.locked = Self::read(c, 0x07)? & 0x01 != 0;
        Ok(())
    }

    /// Sets the mixer, IF RC and channel filters to the entries nearest the bandwidth `bw` in Hz.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_bandwidth(&mut self, c: &mut Chip, bw: u32) -> Result<(), RtlError> {
        let khz = bw / 1000;
        let mix = nearest(&MIXER_FILTER, khz) as u8;
        let rc = nearest(&RC_FILTER, khz) as u8;
        let ch = nearest(&CHANNEL_FILTER, khz) as u8;
        Self::mask(c, 0x11, mix << 4, 0xF0)?;
        Self::mask(c, 0x11, rc, 0x0F)?;
        Self::mask(c, 0x12, ch, 0x1F)
    }

    /// Manual gain mode with the advertised gain nearest `g` tenth dB; returns the gain set.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_manual_gain(&mut self, c: &mut Chip, g: i32) -> Result<i32, RtlError> {
        Self::mask(c, 0x1A, 0x00, 0x0F)?;
        Self::mask(c, 0x20, 0x00, 0x01)?;
        let g = crate::nearest_gain(&GAINS, g);
        let (high, code) = gain_codes(g).ok_or(RtlError::Gain(g))?;
        Self::mask(c, 0x14, code, 0x0F)?;
        Self::mask(c, 0x15, u8::from(high), 0x01)?;
        Ok(g)
    }

    /// Automatic gain mode.
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn set_auto_gain(&mut self, c: &mut Chip) -> Result<(), RtlError> {
        Self::mask(c, 0x1A, 9, 0x0F)?;
        Self::mask(c, 0x20, 0x01, 0x01)?;
        Self::mask(c, 0x24, 0x00, 0x07)
    }

    /// Puts the tuner in standby. Leaving standby needs [`E4000::init`].
    ///
    /// # Errors
    ///
    /// The error of a transfer.
    pub fn standby(&mut self, c: &mut Chip) -> Result<(), RtlError> {
        if !self.initialised {
            return Ok(());
        }
        Self::mask(c, 0x00, 0x00, 0x02)?;
        self.initialised = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_advertised_gain_has_codes() {
        for g in GAINS {
            assert!(gain_codes(g).is_some(), "gain {g}");
        }
        assert_eq!(gain_codes(-10), Some((false, 0)));
        assert_eq!(gain_codes(340), Some((false, 14)));
        assert_eq!(gain_codes(420), Some((true, 14)));
        assert_eq!(gain_codes(100), None);
    }

    #[test]
    fn synthesiser_values() {
        // 100 MHz: R = 32, f = 3200 MHz, Z = 111, X = floor(0.1111.. * 65536) = 7281.
        let s = synth_for(100_000_000, 28_800_000).unwrap();
        assert_eq!((s.r0d, s.r, s.z, s.x), (0x0D, 32, 111, 7281));
        let s = synth_for(1_500_000_000, 28_800_000).unwrap();
        assert_eq!((s.r0d, s.r, s.z), (0x00, 2, 104));
        assert!((s.actual_hz - 1.5e9).abs() < 28.8e6 / 65536.0);
        let s = synth_for(340_000_000, 28_800_000).unwrap();
        assert_eq!((s.r0d, s.r), (0x09, 8));
    }

    #[test]
    fn nearest_filter_entries() {
        assert_eq!(nearest(&MIXER_FILTER, 2_400), 14);
        assert_eq!(nearest(&RC_FILTER, 2_400), 12);
        assert_eq!(nearest(&CHANNEL_FILTER, 2_400), 26);
        assert_eq!(nearest(&MIXER_FILTER, 30_000), 0);
    }
}
